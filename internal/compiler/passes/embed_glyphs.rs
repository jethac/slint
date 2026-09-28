// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore fsdm msdf msdfgen
use crate::CompilerConfiguration;
use crate::diagnostics::BuildDiagnostics;
#[cfg(not(target_arch = "wasm32"))]
use crate::embedded_resources::{BitmapFont, BitmapGlyph, BitmapGlyphs, CharacterMapEntry};
#[cfg(not(target_arch = "wasm32"))]
use crate::expression_tree::BuiltinFunction;
use crate::expression_tree::{Expression, Unit};
use crate::object_tree::*;
use std::collections::HashMap;
use std::collections::HashSet;
use std::rc::Rc;

use i_slint_common::sharedfontique::{self, fontique, skrifa};
#[cfg(not(target_arch = "wasm32"))]
use skrifa::MetadataProvider;

/// Axis tag constants as big-endian `u32` (matching `u32::from_be_bytes`).
const WDTH_TAG: u32 = u32::from_be_bytes(*b"wdth");
const OPSZ_TAG: u32 = u32::from_be_bytes(*b"opsz");

/// One axis value collected from a constant `font-variation-settings`,
/// `font-stretch` or `font-optical-sizing` binding. `FontDefault` resolves to
/// the source font's fvar default — used for `font-optical-sizing: none`, which
/// asks for `opsz` at its default instead of tracking the used size.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum CollectedAxisValue {
    Value(f32),
    FontDefault,
}

/// A normalized axis tuple: sorted by tag, one entry per tag.
pub type CollectedAxisTuple = Vec<(u32, CollectedAxisValue)>;

const EMPTY_TUPLE: CollectedAxisTuple = Vec::new();

/// The result of [`collect_font_axes_used`]: the constant axis tuples used in a
/// component (element settings already merged with every window default of that
/// component), plus the axis-property bindings that aren't constant and
/// therefore need runtime rasterization, which embedded bitmap fonts cannot
/// provide.
#[derive(Default)]
pub struct FontAxesUsed {
    /// The distinct constant tuples on text-like elements.
    pub tuples: Vec<CollectedAxisTuple>,
    /// The distinct constant `default-*` tuples on window elements. The runtime
    /// merges them into descendant requests, so embed_glyphs embeds every
    /// element tuple merged with every window tuple.
    pub window_tuples: Vec<CollectedAxisTuple>,
    /// `(property name, binding location)` of the non-constant axis bindings.
    pub dynamic: Vec<(smol_str::SmolStr, crate::diagnostics::SourceLocation)>,
    /// The families a dynamic axis or `font-family` binding can resolve to, so
    /// vector embedding covers only fonts actually used: the element's constant
    /// family, [`DynamicFamily::Default`] when none is set, or
    /// [`DynamicFamily::Any`] when `font-family` itself is non-constant.
    pub dynamic_families: HashSet<DynamicFamily>,
}

/// Which family a dynamically-bound axis request can come from at run time.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum DynamicFamily {
    /// The element names no family, so the run-time default applies: only the
    /// fonts in the default set need embedding.
    Default,
    /// `font-family` is bound dynamically — any used family can appear, so all
    /// variable fonts in the default and custom sets are embedded.
    Any,
    /// The element's `font-family` is this constant family name.
    Named(String),
}

impl FontAxesUsed {
    /// Every axis tuple bitmaps need to cover: each element tuple, each window
    /// tuple alone (for elements without their own settings), and every
    /// element tuple merged last-wins with every window tuple.
    pub fn tuples_to_embed(&self) -> Vec<CollectedAxisTuple> {
        let mut result = self.tuples.clone();
        for window in &self.window_tuples {
            for element in self.tuples.iter().map(Some).chain(core::iter::once(None)) {
                let mut tuple = window.clone();
                if let Some(element) = element {
                    for entry in element {
                        if let Some(existing) = tuple.iter_mut().find(|(t, _)| *t == entry.0) {
                            *existing = *entry;
                        } else {
                            tuple.push(*entry);
                        }
                    }
                    tuple.sort_by_key(|(tag, _)| *tag);
                }
                if !result.contains(&tuple) {
                    result.push(tuple);
                }
            }
        }
        result
    }
}

#[derive(Clone)]
struct Font {
    font: fontique::QueryFont,
}

/// The fontique collection shared by `embed_glyphs` and `embed_images`, together
/// with the imported fonts' file paths (the collection only knows them as in-memory
/// blobs, so the paths are tracked separately for embedding).
#[cfg(feature = "renderer-software")]
pub struct FontCollection {
    pub collection: sharedfontique::Collection,
    pub custom_font_paths: HashMap<fontique::FamilyId, std::path::PathBuf>,
    pub custom_fonts: HashMap<std::path::PathBuf, fontique::QueryFont>,
}

/// Built once and shared (by reference) between the font and image passes. The
/// `LazyLock` defers the system-font scan to the first lookup, so a build with no
/// glyphs or text SVGs to embed never scans.
#[cfg(feature = "renderer-software")]
pub type SharedFontCollection = std::sync::Arc<
    std::sync::LazyLock<
        std::sync::Mutex<FontCollection>,
        Box<dyn FnOnce() -> std::sync::Mutex<FontCollection> + Send + Sync>,
    >,
>;

/// Reads every imported (`import "...ttf"`) font file, reporting load errors with
/// their import span. The bytes feed [`shared_font_collection`].
#[cfg(feature = "renderer-software")]
pub fn read_custom_fonts<'a>(
    all_docs: impl Iterator<Item = &'a Document>,
    diag: &mut BuildDiagnostics,
) -> Vec<(std::path::PathBuf, Vec<u8>)> {
    let mut fonts = Vec::new();
    for doc in all_docs {
        for (font_path, import_token) in doc.custom_fonts.iter() {
            match std::fs::read(font_path.as_str()) {
                Err(e) => diag.push_error(format!("Error loading font: {e}"), import_token),
                Ok(bytes) => fonts.push((font_path.as_str().into(), bytes)),
            }
        }
    }
    fonts
}

/// Wraps the system fonts plus the imported `custom_fonts` into a [`SharedFontCollection`].
#[cfg(feature = "renderer-software")]
pub fn shared_font_collection(
    custom_fonts: Vec<(std::path::PathBuf, Vec<u8>)>,
) -> SharedFontCollection {
    let init: Box<dyn FnOnce() -> std::sync::Mutex<FontCollection> + Send + Sync> =
        Box::new(move || {
            let mut collection = sharedfontique::create_collection(true);
            let mut custom_font_paths = HashMap::new();
            let mut custom_font_map = HashMap::new();
            for (path, bytes) in custom_fonts {
                if let Some(font) = collection
                    .register_fonts(bytes.into(), None)
                    .first()
                    .and_then(|(id, infos)| collection.get_font_for_info(*id, infos.first()?))
                {
                    custom_font_paths.insert(font.family.0, path.clone());
                    custom_font_map.insert(path, font);
                }
            }
            std::sync::Mutex::new(FontCollection {
                collection,
                custom_font_paths,
                custom_fonts: custom_font_map,
            })
        });
    std::sync::Arc::new(std::sync::LazyLock::new(init))
}

fn swash_font_ref(font: &Font) -> swash::FontRef<'_> {
    swash::FontRef::from_index(font.font.blob.data(), font.font.index as usize).unwrap()
}

#[cfg(target_arch = "wasm32")]
pub fn embed_glyphs<'a>(
    _component: &Document,
    _compiler_config: &CompilerConfiguration,
    _scale_factor: f64,
    _pixel_sizes: Vec<i16>,
    _font_weights: Vec<u16>,
    _axis_tuples: Vec<CollectedAxisTuple>,
    _characters_seen: HashSet<char>,
    _all_docs: impl Iterator<Item = &'a crate::object_tree::Document> + 'a,
    _diag: &mut BuildDiagnostics,
) -> bool {
    false
}

#[cfg(not(target_arch = "wasm32"))]
pub fn embed_glyphs(
    doc: &Document,
    compiler_config: &CompilerConfiguration,
    mut pixel_sizes: Vec<i16>,
    font_weights: Vec<u16>,
    font_axes: FontAxesUsed,
    mut characters_seen: HashSet<char>,
    font_collection: &SharedFontCollection,
    diag: &mut BuildDiagnostics,
) {
    use crate::diagnostics::Spanned;

    let generic_diag_location = doc.node.as_ref().map(|n| n.to_source_location());
    let scale_factor = compiler_config.const_scale_factor.unwrap_or(1.);

    // Embedded bitmap glyphs are rasterized at a fixed axis tuple, so any
    // axis property that can change at runtime (animated or computed binding)
    // has nothing to rasterize against. When the build configuration excludes
    // vector fonts, report the offending bindings rather than silently
    // rendering the font's default instance; otherwise the font's vector data
    // is embedded below so the runtime rasterizes the requested instance.
    if compiler_config.exclude_vector_fonts {
        for (property_name, span) in &font_axes.dynamic {
            diag.push_error_with_span(
                format!(
                    "'{property_name}' is not constant, but this build rasterizes glyphs \
                     at fixed axis values and vector fonts are excluded — give the \
                     property a constant value, enable the software renderer's \
                     `embedded-vector-fonts` feature (Rust builds; clear \
                     `SLINT_EXCLUDE_VECTOR_FONTS` if it was set automatically for a \
                     no-std target), or disable glyph embedding (SLINT_EMBED_RESOURCES) \
                     so the variable font data is used directly"
                ),
                span.clone(),
            );
        }
        if diag.has_errors() {
            return;
        }
    }

    let tuples_to_embed = font_axes.tuples_to_embed();

    characters_seen.extend(
        ('a'..='z')
            .chain('A'..='Z')
            .chain('0'..='9')
            .chain(" '!\"#$%&()*+,-./:;<=>?@\\[]{}^_|~".chars())
            .chain(std::iter::once('●'))
            .chain(std::iter::once('…')),
    );

    if let Ok(sizes_str) = std::env::var("SLINT_FONT_SIZES") {
        for custom_size_str in sizes_str.split(',') {
            let custom_size = if let Ok(custom_size) = custom_size_str
                .parse::<f32>()
                .map(|size_as_float| (size_as_float * scale_factor) as i16)
            {
                custom_size
            } else {
                diag.push_error(
                    format!(
                        "Invalid font size '{custom_size_str}' specified in `SLINT_FONT_SIZES`"
                    ),
                    &generic_diag_location,
                );
                return;
            };

            if let Err(pos) = pixel_sizes.binary_search(&custom_size) {
                pixel_sizes.insert(pos, custom_size)
            }
        }
    }

    let fallback_fonts = get_fallback_fonts();

    // The collection (system fonts + imported fonts) is built once and shared with
    // `embed_images`; the imported-font paths come with it.
    let mut shared = font_collection.lock().unwrap();
    let FontCollection { collection, custom_font_paths: font_paths, custom_fonts } = &mut *shared;

    let mut custom_face_error = false;

    let default_fonts: Vec<(std::path::PathBuf, fontique::QueryFont)> = if !collection
        .default_fonts
        .is_empty()
    {
        collection.default_fonts.as_ref().clone()
    } else {
        let mut default_fonts: Vec<(std::path::PathBuf, fontique::QueryFont)> = Vec::new();

        for c in doc.exported_roots() {
            let (family, source_location) = c
                .root_element
                .borrow()
                .binding("default-font-family")
                .and_then(|binding| match binding.value_expression() {
                    Expression::StringLiteral(family) => {
                        Some((Some(family.clone()), binding.span.clone()))
                    }
                    _ => None,
                })
                .unwrap_or_default();

            let font = {
                let mut query = collection.query();

                query.set_families(
                    family
                        .as_ref()
                        .map(|family| fontique::QueryFamily::from(family.as_str()))
                        .into_iter()
                        .chain(
                            sharedfontique::FALLBACK_FAMILIES
                                .into_iter()
                                .map(fontique::QueryFamily::Generic),
                        ),
                );

                let mut font = None;

                query.matches_with(|queried_font| {
                    font = Some(queried_font.clone());
                    fontique::QueryStatus::Stop
                });
                font
            };

            match font {
                None => {
                    if let Some(source_location) = source_location {
                        diag.push_error_with_span("could not find font that provides specified family, falling back to Sans-Serif".to_string(), source_location);
                    } else {
                        diag.push_error(
                            "internal error: could not determine a default font for sans-serif"
                                .to_string(),
                            &generic_diag_location,
                        );
                    };
                }
                Some(query_font) => {
                    if let Some(font_info) = collection
                        .family(query_font.family.0)
                        .and_then(|family_info| family_info.fonts().first().cloned())
                    {
                        let path = if let Some(path) = font_paths.get(&query_font.family.0) {
                            path.clone()
                        } else {
                            match &font_info.source().kind {
                                fontique::SourceKind::Path(path) => path.to_path_buf(),
                                fontique::SourceKind::Memory(_) => {
                                    diag.push_error(
                                    "internal error: memory fonts are not supported in the compiler"
                                        .to_string(),
                                    &generic_diag_location,
                                );
                                    custom_face_error = true;
                                    continue;
                                }
                            }
                        };
                        font_paths.insert(query_font.family.0, path.clone());
                        default_fonts.push((path.clone(), query_font));
                    }
                }
            }
        }

        default_fonts
    };

    if custom_face_error {
        return;
    }

    let register_embedded_font = |path: &std::path::Path, embedded_bitmap_font: BitmapFont| {
        let resource_id = doc.embedded_file_resources.borrow_mut().push_and_get_key(
            crate::embedded_resources::EmbeddedResources {
                path: Some(path.to_string_lossy().as_ref().into()),
                kind: crate::embedded_resources::EmbeddedResourcesKind::BitmapFontData(
                    embedded_bitmap_font,
                ),
            },
        );

        for c in doc.exported_roots() {
            c.init_code.borrow_mut().font_registration_code.push(Expression::FunctionCall {
                function: BuiltinFunction::RegisterBitmapFont.into(),
                arguments: vec![Expression::NumberLiteral(resource_id.0 as _, Unit::None)],
                source_location: None,
            });
        }
    };

    let mut embed_font_by_path = |path: &std::path::Path, font: &fontique::QueryFont| {
        let Some(family_name) = collection.family_name(font.family.0).to_owned() else {
            diag.push_error(
                format!(
                    "internal error: TrueType font without family name encountered: {}",
                    path.display()
                ),
                &generic_diag_location,
            );
            return;
        };

        let Some(font_ref) = skrifa::FontRef::from_index(font.blob.data(), font.index).ok() else {
            diag.push_error(
                format!("internal error: failed to parse font: {}", path.display()),
                &generic_diag_location,
            );
            return;
        };
        let axes = font_ref.axes();
        let wght_axis = axes.iter().find(|axis| axis.tag() == skrifa::Tag::new(b"wght"));

        if axes.iter().next().is_some() {
            // Variable font: embed one BitmapFont per (axis tuple, weight)
            // combination used in the .slint sources, so every constant
            // axis setting renders its own pre-rasterized instance.
            for tuple in tuples_to_embed
                .iter()
                .filter(|t| !t.is_empty())
                .chain(core::iter::once(&EMPTY_TUPLE))
            {
                // Resolve the tuple against this font's fvar axes; axes the font
                // doesn't declare are inert and dropped from the rasterization.
                let mut tuple_settings: Vec<(skrifa::Tag, f32)> = Vec::new();
                for (tag_bits, value) in tuple {
                    let tag = skrifa::Tag::new(&tag_bits.to_be_bytes());
                    let Some(axis) = axes.iter().find(|axis| axis.tag() == tag) else {
                        continue;
                    };
                    let value = match value {
                        CollectedAxisValue::Value(v) => *v,
                        CollectedAxisValue::FontDefault => axis.default_value(),
                    };
                    tuple_settings.push((tag, value.clamp(axis.min_value(), axis.max_value())));
                }
                let tuple_wght = tuple_settings
                    .iter()
                    .find(|(tag, _)| *tag == skrifa::Tag::new(b"wght"))
                    .map(|(_, value)| *value as u16);
                // `opsz` set by a tuple disables the per-size optical sizing;
                // otherwise the axis tracks the glyph set's used size.
                let auto_opsz = axes.iter().any(|axis| axis.tag() == skrifa::Tag::new(b"opsz"))
                    && !tuple_settings.iter().any(|(tag, _)| *tag == skrifa::Tag::new(b"opsz"));

                let weights: Vec<u16> = if let Some(weight) = tuple_wght {
                    vec![weight]
                } else if font_weights.is_empty() {
                    vec![fontique::FontWeight::NORMAL.value() as u16]
                } else {
                    font_weights.clone()
                };

                for &weight in &weights {
                    let mut settings = tuple_settings.clone();
                    if let Some(wght_axis) = &wght_axis {
                        let clamped =
                            (weight as f32).clamp(wght_axis.min_value(), wght_axis.max_value());
                        if let Some(existing) =
                            settings.iter_mut().find(|(tag, _)| *tag == wght_axis.tag())
                        {
                            existing.1 = clamped;
                        } else {
                            settings.push((wght_axis.tag(), clamped));
                        }
                    }
                    let location = axes.location(settings.iter().copied());

                    // Record the resolved tuple on the embedded font so the
                    // runtime can score it against a request: every fvar axis
                    // with its used and default value, except `wght` (carried
                    // by `weight`) and `opsz` while it tracks the size.
                    let recorded: Vec<crate::embedded_resources::BitmapFontVariation> = axes
                        .iter()
                        .filter_map(|axis| {
                            let tag = axis.tag();
                            if tag == skrifa::Tag::new(b"wght")
                                || (auto_opsz && tag == skrifa::Tag::new(b"opsz"))
                            {
                                return None;
                            }
                            let used = settings
                                .iter()
                                .find(|(t, _)| *t == tag)
                                .map(|(_, v)| *v)
                                .unwrap_or_else(|| axis.default_value());
                            Some(crate::embedded_resources::BitmapFontVariation {
                                tag: u32::from_be_bytes(tag.to_be_bytes()),
                                value: used,
                                default_value: axis.default_value(),
                            })
                        })
                        .collect();

                    let embedded = embed_font(
                        family_name.to_owned(),
                        Font { font: font.clone() },
                        &pixel_sizes,
                        characters_seen.iter().cloned(),
                        &fallback_fonts,
                        compiler_config,
                        location.coords(),
                        &settings,
                        Some(weight),
                        recorded,
                        if auto_opsz {
                            axes.iter()
                                .find(|axis| axis.tag() == skrifa::Tag::new(b"opsz"))
                                .map(|axis| (axis, scale_factor))
                        } else {
                            None
                        },
                    );
                    register_embedded_font(path, embedded);
                }
            }
        } else {
            // Static font: embed once
            let embedded = embed_font(
                family_name.to_owned(),
                Font { font: font.clone() },
                &pixel_sizes,
                characters_seen.iter().cloned(),
                &fallback_fonts,
                compiler_config,
                &[],
                &[],
                None,
                Vec::new(),
                None,
            );
            register_embedded_font(path, embedded);
        }
    };

    // default_fonts is in primary-first order (set up by sharedfontique from
    // SLINT_DEFAULT_FONT then SLINT_FONT_PATH); preserve it.
    for (path, font) in default_fonts.iter() {
        custom_fonts.remove(path);
        embed_font_by_path(path, font);
    }

    for (path, font) in custom_fonts.iter() {
        embed_font_by_path(path, font);
    }

    // Non-constant axis bindings (animated or computed) rasterize through the
    // vector path: embed the raw font data and register it at run time, like
    // `collect_custom_fonts` does for non-`EmbedTextures` builds. Only
    // variable fonts can produce distinct instances; a static font satisfies
    // every axis request with its default instance. (This branch is only
    // reached when `exclude_vector_fonts` is unset — see the error loop
    // above.)
    if !font_axes.dynamic.is_empty() && !compiler_config.exclude_vector_fonts {
        let mut embedded_paths: HashSet<std::path::PathBuf> = HashSet::new();
        let is_default_set = |path: &std::path::Path| default_fonts.iter().any(|(p, _)| p == path);
        for (path, font) in default_fonts
            .iter()
            .map(|(p, f)| (p, f))
            .chain(custom_fonts.iter().map(|(p, f)| (p, f)))
        {
            if !embedded_paths.insert(path.clone()) {
                continue;
            }
            // Embed only fonts a dynamic binding can resolve to — every
            // embedded vector font costs flash on the MCU target.
            let family_name = collection.family_name(font.family.0);
            let used = font_axes.dynamic_families.iter().any(|family| match family {
                DynamicFamily::Any => true,
                DynamicFamily::Default => is_default_set(path),
                DynamicFamily::Named(name) => family_name.is_some_and(|f| f == name.as_str()),
            });
            if !used {
                continue;
            }
            let Ok(font_ref) = skrifa::FontRef::from_index(font.blob.data(), font.index) else {
                continue;
            };
            if font_ref.axes().iter().next().is_none() {
                continue;
            }
            let resource_id = doc.embedded_file_resources.borrow_mut().push_and_get_key(
                crate::embedded_resources::EmbeddedResources {
                    path: Some(path.to_string_lossy().as_ref().into()),
                    kind: crate::embedded_resources::EmbeddedResourcesKind::FileData,
                },
            );
            for c in doc.exported_roots() {
                c.init_code.borrow_mut().font_registration_code.push(Expression::FunctionCall {
                    function: BuiltinFunction::RegisterCustomFontByMemory.into(),
                    arguments: vec![Expression::NumberLiteral(resource_id.0 as _, Unit::None)],
                    source_location: None,
                });
            }
        }
    }
}

#[inline(never)] // workaround https://github.com/rust-lang/rust/issues/104099
fn get_fallback_fonts() -> Vec<Font> {
    let mut fallback_fonts = Vec::new();

    let mut collection = sharedfontique::create_collection(false);
    let mut query = collection.query();
    query.set_families(
        sharedfontique::FALLBACK_FAMILIES.into_iter().map(fontique::QueryFamily::Generic).chain(
            core::iter::once(fontique::QueryFamily::Generic(fontique::GenericFamily::Emoji)),
        ),
    );

    query.matches_with(|query_font| {
        fallback_fonts.push(Font { font: query_font.clone() });
        fontique::QueryStatus::Continue
    });

    fallback_fonts
}

#[cfg(not(target_arch = "wasm32"))]
fn embed_font(
    family_name: String,
    font: Font,
    pixel_sizes: &[i16],
    character_coverage: impl Iterator<Item = char>,
    fallback_fonts: &[Font],
    _compiler_config: &CompilerConfiguration,
    normalized_coords: &[skrifa::instance::NormalizedCoord],
    _variations: &[(skrifa::Tag, f32)],
    override_weight: Option<u16>,
    recorded_variations: Vec<crate::embedded_resources::BitmapFontVariation>,
    // `Some((axis, scale_factor))` rasterizes every glyph set with `opsz` equal
    // to its used (logical) size — automatic optical sizing on the bitmap path.
    auto_opsz: Option<(skrifa::Axis, f32)>,
) -> BitmapFont {
    let coords_i16: Vec<i16> = normalized_coords.iter().map(|c| c.to_bits()).collect();

    let mut character_map: Vec<CharacterMapEntry> = character_coverage
        .filter(|code_point| {
            core::iter::once(&font)
                .chain(fallback_fonts.iter())
                .any(|font| swash_font_ref(font).charmap().map(*code_point) != 0)
        })
        .enumerate()
        .map(|(glyph_index, code_point)| CharacterMapEntry {
            code_point,
            glyph_index: u16::try_from(glyph_index)
                .expect("more than 65535 glyphs are not supported"),
        })
        .collect();

    let auto_opsz = auto_opsz.map(|(axis, scale_factor)| {
        let index = skrifa::FontRef::from_index(font.font.blob.data(), font.font.index)
            .expect("embed_font is only called with parseable fonts")
            .axes()
            .iter()
            .position(|a| a.tag() == axis.tag())
            .expect("auto_opsz is only set when the font declares an opsz axis");
        (axis, index, scale_factor)
    });
    let has_auto_opsz = auto_opsz.is_some();

    #[cfg(feature = "sdf-fonts")]
    let glyphs = if _compiler_config.use_sdf_fonts {
        embed_sdf_glyphs(pixel_sizes, &character_map, &font, fallback_fonts, _variations, auto_opsz)
    } else {
        embed_alpha_map_glyphs(
            pixel_sizes,
            &character_map,
            &font,
            fallback_fonts,
            &coords_i16,
            auto_opsz,
        )
    };
    #[cfg(not(feature = "sdf-fonts"))]
    let glyphs = embed_alpha_map_glyphs(
        pixel_sizes,
        &character_map,
        &font,
        fallback_fonts,
        &coords_i16,
        auto_opsz,
    );

    character_map.sort_by_key(|entry| entry.code_point);

    let font_ref = skrifa::FontRef::from_index(font.font.blob.data(), font.font.index).unwrap();
    let location = skrifa::instance::LocationRef::new(normalized_coords);
    let metrics =
        skrifa::metrics::Metrics::new(&font_ref, skrifa::instance::Size::unscaled(), location);
    let attrs = skrifa::attribute::Attributes::new(&font_ref);

    BitmapFont {
        family_name,
        character_map,
        units_per_em: metrics.units_per_em as f32,
        ascent: metrics.ascent,
        descent: metrics.descent,
        x_height: metrics.x_height.unwrap_or_default(),
        cap_height: metrics.cap_height.unwrap_or_default(),
        glyphs,
        weight: override_weight.unwrap_or(attrs.weight.value() as u16),
        italic: attrs.style != skrifa::attribute::Style::Normal,
        #[cfg(feature = "sdf-fonts")]
        sdf: _compiler_config.use_sdf_fonts,
        #[cfg(not(feature = "sdf-fonts"))]
        sdf: false,
        variations: recorded_variations,
        auto_opsz: has_auto_opsz,
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn embed_alpha_map_glyphs(
    pixel_sizes: &[i16],
    character_map: &Vec<CharacterMapEntry>,
    font: &Font,
    fallback_fonts: &[Font],
    normalized_coords: &[i16],
    // `Some((axis, index, scale_factor))` overrides the `opsz` coordinate of each
    // glyph set with that set's used (logical) size.
    auto_opsz: Option<(skrifa::Axis, usize, f32)>,
) -> Vec<BitmapGlyphs> {
    use rayon::prelude::*;
    use std::cell::RefCell;

    thread_local! {
        static SCALE_CONTEXT: RefCell<swash::scale::ScaleContext> =
            RefCell::new(swash::scale::ScaleContext::new());
    }

    pixel_sizes
        .par_iter()
        .map(|pixel_size| {
            // With automatic optical sizing the opsz coordinate follows each
            // glyph set's used (logical) size — physical size divided by the
            // constant scale factor.
            let coords_storage;
            let normalized_coords: &[i16] =
                if let Some((axis, index, scale_factor)) = auto_opsz.as_ref() {
                    coords_storage = normalized_coords
                        .iter()
                        .enumerate()
                        .map(|(i, c)| {
                            if i == *index {
                                axis.normalize(*pixel_size as f32 / scale_factor).to_bits()
                            } else {
                                *c
                            }
                        })
                        .collect::<Vec<_>>();
                    &coords_storage
                } else {
                    normalized_coords
                };

            let glyph_data = character_map
                .par_iter()
                .map(|CharacterMapEntry { code_point, .. }| {
                    let font_to_use = core::iter::once(font)
                        .chain(fallback_fonts.iter())
                        .find(|f| swash_font_ref(f).charmap().map(*code_point) != 0)
                        .unwrap_or(font);

                    let font_ref = swash_font_ref(font_to_use);
                    let glyph_id = font_ref.charmap().map(*code_point);
                    let gm = font_ref.glyph_metrics(normalized_coords);
                    let fm = font_ref.metrics(normalized_coords);
                    let scale = *pixel_size as f32 / fm.units_per_em as f32;
                    let advance_width = gm.advance_width(glyph_id) * scale;

                    SCALE_CONTEXT.with(|ctx| {
                        let font_ref = swash_font_ref(font_to_use);
                        let mut ctx = ctx.borrow_mut();
                        let mut scaler = ctx
                            .builder(font_ref)
                            .size(*pixel_size as f32)
                            .normalized_coords(normalized_coords)
                            .build();
                        let image = swash::scale::Render::new(&[swash::scale::Source::Outline])
                            .format(swash::zeno::Format::Alpha)
                            .render(&mut scaler, glyph_id);

                        match image {
                            Some(image) => {
                                let p = image.placement;
                                BitmapGlyph {
                                    x: i16::try_from(p.left * 64)
                                        .expect("large glyph x coordinate"),
                                    y: i16::try_from((p.top - p.height as i32) * 64)
                                        .expect("large glyph y coordinate"),
                                    width: i16::try_from(p.width).expect("large width"),
                                    height: i16::try_from(p.height).expect("large height"),
                                    x_advance: i16::try_from((advance_width * 64.) as i64)
                                        .expect("large advance width"),
                                    data: image.data,
                                }
                            }
                            None => BitmapGlyph {
                                x: 0,
                                y: 0,
                                width: 0,
                                height: 0,
                                x_advance: i16::try_from((advance_width * 64.) as i64)
                                    .expect("large advance width"),
                                data: vec![],
                            },
                        }
                    })
                })
                .collect();

            BitmapGlyphs { pixel_size: *pixel_size, glyph_data }
        })
        .collect()
}

#[cfg(all(not(target_arch = "wasm32"), feature = "sdf-fonts"))]
fn embed_sdf_glyphs(
    pixel_sizes: &[i16],
    character_map: &Vec<CharacterMapEntry>,
    font: &Font,
    fallback_fonts: &[Font],
    variations: &[(skrifa::Tag, f32)],
    // `Some((axis, _index, scale_factor))` rasterizes the SDF with `opsz` equal
    // to the target glyph set's used (logical) size.
    auto_opsz: Option<(skrifa::Axis, usize, f32)>,
) -> Vec<BitmapGlyphs> {
    use rayon::prelude::*;

    const RANGE: f64 = 6.;

    let Some(max_size) = pixel_sizes.iter().max() else {
        return Vec::new();
    };
    let min_size = pixel_sizes.iter().min().expect("we have a 'max' so the vector is not empty");
    let target_pixel_size = (max_size * 2 / 3).max(16).min(RANGE as i16 * min_size);

    let auto_opsz_setting = auto_opsz
        .map(|(axis, _, scale_factor)| (axis.tag(), target_pixel_size as f32 / scale_factor));
    let variations_storage;
    let variations = if let Some(opsz_setting) = auto_opsz_setting {
        variations_storage =
            variations.iter().copied().chain(core::iter::once(opsz_setting)).collect::<Vec<_>>();
        variations_storage.as_slice()
    } else {
        variations
    };

    let glyph_data = character_map
        .par_iter()
        .map(|CharacterMapEntry { code_point, .. }| {
            core::iter::once(font)
                .chain(fallback_fonts.iter())
                .find_map(|font| {
                    (swash_font_ref(font).charmap().map(*code_point) != 0).then(|| {
                        generate_sdf_for_glyph(
                            font,
                            *code_point,
                            target_pixel_size,
                            RANGE,
                            variations,
                        )
                    })
                })
                .unwrap_or_else(|| {
                    generate_sdf_for_glyph(font, *code_point, target_pixel_size, RANGE, variations)
                })
                .unwrap_or_default()
        })
        .collect::<Vec<_>>();

    vec![BitmapGlyphs { pixel_size: target_pixel_size, glyph_data }]
}

#[cfg(all(not(target_arch = "wasm32"), feature = "sdf-fonts"))]
fn generate_sdf_for_glyph(
    font: &Font,
    code_point: char,
    target_pixel_size: i16,
    range: f64,
    variations: &[(skrifa::Tag, f32)],
) -> Option<BitmapGlyph> {
    use fdsm::transform::Transform;
    use nalgebra::{Affine2, Similarity2, Vector2};

    let mut face =
        fdsm_ttf_parser::ttf_parser::Face::parse(font.font.blob.data(), font.font.index).unwrap();
    for &(tag, value) in variations {
        face.set_variation(
            fdsm_ttf_parser::ttf_parser::Tag(u32::from_be_bytes(tag.to_be_bytes())),
            value,
        );
    }
    let glyph_id = face.glyph_index(code_point).unwrap_or_default();

    let font_ref = skrifa::FontRef::from_index(font.font.blob.data(), font.font.index).unwrap();
    let variation_settings: Vec<_> =
        variations.iter().map(|&(tag, value)| (tag, value)).collect::<Vec<_>>();
    let location = font_ref.axes().location(variation_settings);
    let metrics = skrifa::metrics::Metrics::new(
        &font_ref,
        skrifa::instance::Size::unscaled(),
        skrifa::instance::LocationRef::from(&location),
    );
    let target_pixel_size = target_pixel_size as f64;
    let scale = target_pixel_size / metrics.units_per_em as f64;

    // TODO: handle bitmap glyphs (emojis)
    let Some(bbox) = face.glyph_bounding_box(glyph_id) else {
        // For example, for space
        return Some(BitmapGlyph {
            x_advance: (face.glyph_hor_advance(glyph_id).unwrap_or(0) as f64 * scale * 64.) as i16,
            ..Default::default()
        });
    };

    let mut shape = fdsm_ttf_parser::load_shape_from_face(&face, glyph_id)?;

    let width = ((bbox.x_max as f64 - bbox.x_min as f64) * scale + 2.).ceil() as u32;
    let height = ((bbox.y_max as f64 - bbox.y_min as f64) * scale + 2.).ceil() as u32;
    let transformation = nalgebra::convert::<_, Affine2<f64>>(Similarity2::new(
        Vector2::new(1. - bbox.x_min as f64 * scale, 1. - bbox.y_min as f64 * scale),
        0.,
        scale,
    ));

    // Unlike msdfgen, the transformation is not passed into the
    // `generate_msdf` function – the coordinates of the control points
    // must be expressed in terms of pixels on the distance field. To get
    // the correct units, we pre-transform the shape:

    shape.transform(&transformation);

    let prepared_shape = shape.prepare();

    // Set up the resulting image and generate the distance field:

    let mut sdf = image::GrayImage::new(width, height);
    fdsm::generate::generate_sdf(&prepared_shape, range, &mut sdf);
    fdsm::render::correct_sign_sdf(
        &mut sdf,
        &prepared_shape,
        fdsm::bezier::scanline::FillRule::Nonzero,
    );

    let mut glyph_data = sdf.into_raw();

    // normalize around 0
    for x in &mut glyph_data {
        *x = x.wrapping_sub(128);
    }

    // invert the y coordinate (as the fsdm crate has the y axis inverted)
    let (w, h) = (width as usize, height as usize);
    for idx in 0..glyph_data.len() / 2 {
        glyph_data.swap(idx, (h - idx / w - 1) * w + idx % w);
    }

    // Add a "0" so that we can always access pos+1 without going out of bound
    // (so that the last row will look like `data[len-1]*1 + data[len]*0`)
    glyph_data.push(0);

    let bg = BitmapGlyph {
        x: i16::try_from((-(1. - bbox.x_min as f64 * scale) * 64.).ceil() as i32)
            .expect("large glyph x coordinate"),
        y: i16::try_from((-(1. - bbox.y_min as f64 * scale) * 64.).ceil() as i32)
            .expect("large glyph y coordinate"),
        width: i16::try_from(width).expect("large width"),
        height: i16::try_from(height).expect("large height"),
        x_advance: i16::try_from(
            (face.glyph_hor_advance(glyph_id).unwrap() as f64 * scale * 64.).round() as i32,
        )
        .expect("large advance width"),
        data: glyph_data,
    };

    Some(bg)
}

fn try_extract_literal_from_element(
    elem: &ElementRc,
    property_name: &str,
    unit: Unit,
) -> Option<f64> {
    elem.borrow().binding(property_name).and_then(|binding| match binding.value_expression() {
        Expression::NumberLiteral(value, u) if *u == unit => Some(*value),
        Expression::Cast { from, .. } => match from.as_ref() {
            Expression::NumberLiteral(value, u) if *u == unit => Some(*value),
            _ => None,
        },
        _ => None,
    })
}

pub fn collect_font_sizes_used(
    component: &Rc<Component>,
    scale_factor: f64,
    sizes_seen: &mut Vec<i16>,
) {
    let mut add_font_size = |logical_size: f64| {
        let pixel_size = (logical_size * scale_factor) as i16;
        match sizes_seen.binary_search(&pixel_size) {
            Ok(_) => {}
            Err(pos) => sizes_seen.insert(pos, pixel_size),
        }
    };

    recurse_elem_including_sub_components(component, &(), &mut |elem, _| match elem
        .borrow()
        .base_type
        .to_string()
        .as_str()
    {
        "TextInput" | "Text" | "SimpleText" | "ComplexText" | "StyledText" | "StyledTextItem" => {
            if let Some(font_size) = try_extract_literal_from_element(elem, "font-size", Unit::Px) {
                add_font_size(font_size)
            }
        }
        "Dialog" | "Window" | "WindowItem" => {
            if let Some(font_size) =
                try_extract_literal_from_element(elem, "default-font-size", Unit::Px)
            {
                add_font_size(font_size)
            }
        }
        _ => {}
    });
}

pub fn collect_font_weights_used(component: &Rc<Component>, weights_seen: &mut Vec<u16>) {
    let mut add_weight = |weight: f64| {
        let weight = weight as u16;
        if let Err(pos) = weights_seen.binary_search(&weight) {
            weights_seen.insert(pos, weight);
        }
    };

    recurse_elem_including_sub_components(component, &(), &mut |elem, _| match elem
        .borrow()
        .base_type
        .to_string()
        .as_str()
    {
        "TextInput" | "Text" | "SimpleText" | "ComplexText" | "StyledText" | "StyledTextItem" => {
            if let Some(weight) = try_extract_literal_from_element(elem, "font-weight", Unit::None)
            {
                add_weight(weight)
            }
        }
        "Dialog" | "Window" | "WindowItem" => {
            if let Some(weight) =
                try_extract_literal_from_element(elem, "default-font-weight", Unit::None)
            {
                add_weight(weight)
            }
        }
        _ => {}
    });
}

pub fn scan_string_literals(component: &Rc<Component>, characters_seen: &mut HashSet<char>) {
    visit_all_expressions(component, |expr, _| {
        expr.visit_recursive(&mut |expr| {
            if let Expression::StringLiteral(string) = expr {
                characters_seen.extend(string.chars());
            }
        })
    })
}

/// Collects the constant font-axis tuples used by a component's text and window
/// elements. `font-stretch` contributes a `wdth` entry, `font-optical-sizing:
/// none` an `opsz` entry at the font's default, and `font-variation-settings`
/// its literal list. Element-level tuples are merged last-wins with every
/// `default-*` tuple declared on a window of the same component, since the
/// runtime merges them the same way into the request.
///
/// Bindings that aren't constant (expressions or `animate`d properties) land in
/// `seen.dynamic` — embedded bitmap fonts can't express runtime-varying axes.
pub fn collect_font_axes_used(component: &Rc<Component>, seen: &mut FontAxesUsed) {
    fn number_value(expr: &Expression) -> Option<f64> {
        match expr {
            Expression::NumberLiteral(value, _) => Some(*value),
            Expression::Cast { from, .. } => number_value(from),
            _ => None,
        }
    }

    /// Reads one axis-related binding: pushes constant entries into `tuple`,
    /// records non-constant ones in `dynamic`. Returns nothing when the
    /// property isn't bound at all.
    fn collect_binding(
        elem: &ElementRc,
        property: &str,
        tuple: &mut CollectedAxisTuple,
        dynamic: &mut Vec<(smol_str::SmolStr, crate::diagnostics::SourceLocation)>,
    ) {
        let element = elem.borrow();
        let Some(binding) = element.binding(property) else { return };
        let span = || binding.span.clone().unwrap_or_default();
        if binding.animation.is_some() {
            dynamic.push((property.into(), span()));
            return;
        }
        match binding.value_expression() {
            Expression::Array { values, .. } => {
                let mut entries: CollectedAxisTuple = Vec::new();
                let mut all_constant = true;
                for entry in values {
                    let constant_entry = match entry {
                        Expression::Struct { values, .. } => match (
                            values.get("tag"),
                            values.get("value").and_then(|e| number_value(e)),
                        ) {
                            (Some(Expression::StringLiteral(tag)), Some(value))
                                if tag.len() == 4
                                    && tag.bytes().all(|b| (0x20..=0x7e).contains(&b)) =>
                            {
                                Some((
                                    u32::from_be_bytes(tag.as_bytes().try_into().unwrap()),
                                    CollectedAxisValue::Value(value as f32),
                                ))
                            }
                            _ => None,
                        },
                        _ => None,
                    };
                    let Some(entry) = constant_entry else {
                        all_constant = false;
                        break;
                    };
                    entries.push(entry);
                }
                if !all_constant {
                    dynamic.push((property.into(), span()));
                    return;
                }
                for entry in entries {
                    if let Some(existing) = tuple.iter_mut().find(|(t, _)| *t == entry.0) {
                        *existing = entry;
                    } else {
                        tuple.push(entry);
                    }
                }
            }
            Expression::EnumerationValue(value) => {
                // `font-optical-sizing`: Inherit resolves to the surrounding
                // default (auto), Auto tracks the used size, None pins `opsz`
                // to the font's default.
                if matches!(value.enumeration.values[value.value].as_str(), "none")
                    && !tuple.iter().any(|(t, _)| *t == OPSZ_TAG)
                {
                    tuple.push((OPSZ_TAG, CollectedAxisValue::FontDefault));
                }
            }
            Expression::NumberLiteral(value, unit) if *unit == Unit::Percent => {
                // `font-stretch` percent ↔ the `wdth` axis value: the literal
                // already holds the percentage number (75% → 75). 0 is the
                // "unset" sentinel — the runtime resolves the window's
                // `default-font-stretch` in that case (see
                // `WindowItem::resolved_font_request`), so it contributes no
                // entry and the window tuple supplies one.
                if *value == 0. {
                    return;
                }
                if let Some(existing) = tuple.iter_mut().find(|(t, _)| *t == WDTH_TAG) {
                    *existing = (WDTH_TAG, CollectedAxisValue::Value(*value as f32));
                } else {
                    tuple.push((WDTH_TAG, CollectedAxisValue::Value(*value as f32)));
                }
            }
            _ => dynamic.push((property.into(), span())),
        }
    }

    let mut element_tuples: Vec<CollectedAxisTuple> = Vec::new();
    let mut window_tuples: Vec<CollectedAxisTuple> = Vec::new();

    recurse_elem_including_sub_components(component, &(), &mut |elem, _| {
        let base = elem.borrow().base_type.to_string();
        let (is_text, is_window) = match base.as_str() {
            "TextInput" | "Text" | "SimpleText" | "ComplexText" | "StyledText"
            | "StyledTextItem" => (true, false),
            "Dialog" | "Window" | "WindowItem" | "PopupWindow" => (false, true),
            _ => (false, false),
        };
        if !is_text && !is_window {
            return;
        }
        // Dynamic bindings found on this element; merged into `seen.dynamic`
        // after the loop, together with the element's family for
        // `dynamic_families`.
        let mut element_dynamic: Vec<(smol_str::SmolStr, crate::diagnostics::SourceLocation)> =
            Vec::new();
        let prefix = if is_window { "default-" } else { "" };
        let mut tuple: CollectedAxisTuple = Vec::new();
        for suffix in ["font-stretch", "font-optical-sizing", "font-variation-settings"] {
            collect_binding(
                elem,
                format!("{prefix}{suffix}").as_str(),
                &mut tuple,
                &mut element_dynamic,
            );
        }

        // `font-weight` feeds the `wght` axis through the `font_weights`
        // collection, which only understands literals. A bound or animated
        // weight would silently snap to the nearest embedded instance, so it
        // takes the same route as other non-constant axis inputs.
        if let Some(binding) = elem.borrow().binding(format!("{prefix}font-weight").as_str()) {
            if binding.animation.is_some()
                || try_extract_literal_from_element(
                    elem,
                    &format!("{prefix}font-weight"),
                    Unit::None,
                )
                .is_none()
            {
                element_dynamic.push((
                    format!("{prefix}font-weight").into(),
                    binding.span.clone().unwrap_or_default(),
                ));
            }
        }

        // A bound `font-family` can name a family that no bitmap instance was
        // rasterized for — the request would silently match the fallback. The
        // dynamic path embeds every used variable font so the vector rasterizer
        // resolves it at run time (or the binding errors out when vector fonts
        // are excluded).
        let family_property = format!("{prefix}font-family");
        // The family this element's dynamic bindings resolve to at run time,
        // used to embed only the vector fonts that can actually be requested.
        let (dynamic_family, dynamic_family_span) = {
            let element = elem.borrow();
            match element.binding(family_property.as_str()) {
                Some(binding) => match binding.value_expression() {
                    Expression::StringLiteral(family) => {
                        (DynamicFamily::Named(family.to_string()), None)
                    }
                    _ => (DynamicFamily::Any, Some(binding.span.clone().unwrap_or_default())),
                },
                None => (DynamicFamily::Default, None),
            }
        };
        if let Some(span) = dynamic_family_span {
            element_dynamic.push((family_property.into(), span));
        }

        // `<font>` tags in styled-text markup carry axis attributes of their
        // own; the markup source is a compile-time literal, so every value is
        // constant. Each span's attributes merge over the element's own axes,
        // producing one tuple per distinct span configuration.
        if is_text && (base == "StyledTextItem" || base == "StyledText") {
            for span_tuple in collect_markup_axes(elem, &tuple, &mut element_dynamic) {
                if !element_tuples.contains(&span_tuple) {
                    element_tuples.push(span_tuple);
                }
            }
        }
        if !element_dynamic.is_empty() {
            seen.dynamic_families.insert(dynamic_family);
            seen.dynamic.extend(element_dynamic);
        }
        if !tuple.is_empty() {
            tuple.sort_by_key(|(tag, _)| *tag);
            if is_window {
                if !window_tuples.contains(&tuple) {
                    window_tuples.push(tuple);
                }
            } else if !element_tuples.contains(&tuple) {
                element_tuples.push(tuple);
            }
        }
    });

    for tuple in element_tuples {
        if !seen.tuples.contains(&tuple) {
            seen.tuples.push(tuple);
        }
    }
    for tuple in window_tuples {
        if !seen.window_tuples.contains(&tuple) {
            seen.window_tuples.push(tuple);
        }
    }
}

/// Reads the `<font>` tag attributes out of a styled-text `text` binding's
/// markup and merges them into `tuple`. A literal markup string (or the format
/// string of `ParseMarkdown` for `@markdown{}`) is fully known at compile time.
/// Markup that isn't a literal can carry `<font>` axis attributes we can't see,
/// so it is non-constant for collection purposes and takes the dynamic path
/// like any other non-literal axis input.
/// Reads the `<font>` tag attributes out of a styled-text `text` binding's
/// markup. Each span that declares axis attributes produces one tuple — the
/// element's own axes (`element_tuple`) overridden by the span's, matching how
/// the run's font request merges them. Spans without axis attributes inherit
/// the element settings and need no tuple of their own.
///
/// A literal markup string (or the format string of `ParseMarkdown` for
/// `@markdown{}`) is fully known at compile time. Markup that isn't a literal
/// can carry `<font>` axis attributes we can't see, so it is non-constant for
/// collection purposes and takes the dynamic path like any other non-literal
/// axis input.
fn collect_markup_axes(
    elem: &ElementRc,
    element_tuple: &CollectedAxisTuple,
    dynamic: &mut Vec<(smol_str::SmolStr, crate::diagnostics::SourceLocation)>,
) -> Vec<CollectedAxisTuple> {
    let (markup, span) = {
        let elem = elem.borrow();
        let Some(binding) = elem.binding("text") else { return Vec::new() };
        let markup = match binding.value_expression() {
            Expression::StringLiteral(markup) => Some(markup.clone()),
            Expression::FunctionCall { function, arguments, .. }
                if matches!(
                    function,
                    crate::expression_tree::Callable::Builtin(BuiltinFunction::ParseMarkdown)
                ) =>
            {
                match arguments.first() {
                    Some(Expression::StringLiteral(markup)) => Some(markup.clone()),
                    _ => None,
                }
            }
            _ => None,
        };
        (markup, binding.span.clone().unwrap_or_default())
    };
    let Some(markup) = markup else {
        dynamic.push(("text (styled markup)".into(), span));
        return Vec::new();
    };
    let (paragraphs, _errors) = i_slint_common::styled_text::parse_interpolated::<
        &[i_slint_common::styled_text::StyledTextParagraph],
    >(markup.as_str(), &[]);

    let mut span_tuples = Vec::new();
    for paragraph in &paragraphs {
        for span in &paragraph.formatting {
            let i_slint_common::styled_text::Style::FontTag(font_tag) = &span.style else {
                continue;
            };
            let mut span_axes: CollectedAxisTuple = Vec::new();
            if let Some(stretch) = font_tag.font_stretch {
                span_axes.push((WDTH_TAG, CollectedAxisValue::Value(stretch)));
            }
            if font_tag.font_optical_sizing == Some(false) {
                span_axes.push((OPSZ_TAG, CollectedAxisValue::FontDefault));
            }
            for (tag, value) in font_tag.font_variation_settings.iter().flatten() {
                if tag.len() == 4 && tag.bytes().all(|b| (0x20..=0x7e).contains(&b)) {
                    span_axes.push((
                        u32::from_be_bytes(tag.as_bytes().try_into().unwrap()),
                        CollectedAxisValue::Value(*value),
                    ));
                }
            }
            if !span_axes.is_empty() {
                // The span inherits the element's axes and overrides the tags
                // it declares — the same merge the runtime applies per run.
                let mut merged = element_tuple.clone();
                for entry in span_axes {
                    if let Some(existing) = merged.iter_mut().find(|(t, _)| *t == entry.0) {
                        *existing = entry;
                    } else {
                        merged.push(entry);
                    }
                }
                merged.sort_by_key(|(tag, _)| *tag);
                if !span_tuples.contains(&merged) {
                    span_tuples.push(merged);
                }
            }
        }
    }
    span_tuples
}
