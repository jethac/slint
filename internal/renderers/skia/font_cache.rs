// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use clru::CLruCache;
use i_slint_common::sharedfontique::HashedBlob;
use i_slint_core::textlayout::sharedparley::{fontique, parley};
use std::cell::RefCell;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::num::NonZeroUsize;

const FONT_CACHE_CAPACITY: NonZeroUsize = NonZeroUsize::new(64).unwrap();
const GLYPH_PATH_CACHE_CAPACITY: NonZeroUsize = NonZeroUsize::new(256).unwrap();

/// Accumulates a glyph outline into a [`skia_safe::PathBuilder`], flipping
/// the font's y-up space to the canvas's y-down space.
struct SkiaPathPen {
    path: skia_safe::PathBuilder,
}

impl skrifa::outline::OutlinePen for SkiaPathPen {
    fn move_to(&mut self, x: f32, y: f32) {
        self.path.move_to((x, -y));
    }

    fn line_to(&mut self, x: f32, y: f32) {
        self.path.line_to((x, -y));
    }

    fn quad_to(&mut self, cx0: f32, cy0: f32, x: f32, y: f32) {
        self.path.quad_to((cx0, -cy0), (x, -y));
    }

    fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
        self.path.cubic_to((cx0, -cy0), (cx1, -cy1), (x, -y));
    }

    fn close(&mut self) {
        self.path.close();
    }
}

/// The user-space axis list (tag, value) the shaper consumed, hashed the same
/// way as [`FontCache::font_with_variations`]'s typeface key.
pub(super) fn variation_settings_hash(variation_settings: &[(u32, f32)]) -> u64 {
    if variation_settings.is_empty() {
        return 0;
    }
    let mut hasher = DefaultHasher::new();
    for &(tag, value) in variation_settings {
        tag.to_be_bytes().hash(&mut hasher);
        value.to_bits().hash(&mut hasher);
    }
    hasher.finish()
}

pub struct FontCache {
    font_mgr: skia_safe::FontMgr,
    // Use HashedBlob in key to keep strong reference to font data blob,
    // preventing eviction from fontique's shared cache (see commit 30a03cf).
    // The u64 is a hash of variation settings (0 for base typefaces).
    fonts: CLruCache<(HashedBlob, u32, u64), Option<skia_safe::Typeface>>,
    // Outline paths rasterized at exact variation coordinates, for fonts whose
    // typeface can't take variation arguments. Keyed by glyph + coords + size.
    glyph_paths: CLruCache<(HashedBlob, u32, u32, u32, u64), Option<skia_safe::Path>>,
    // Whether the face has an `opsz` fvar axis, per (blob, index).
    has_opsz_axis: std::collections::HashMap<(HashedBlob, u32), bool>,
}

impl Default for FontCache {
    fn default() -> Self {
        Self {
            font_mgr: skia_safe::FontMgr::new(),
            fonts: CLruCache::new(FONT_CACHE_CAPACITY),
            glyph_paths: CLruCache::new(GLYPH_PATH_CACHE_CAPACITY),
            has_opsz_axis: Default::default(),
        }
    }
}

impl FontCache {
    /// `variations` is the `FontVariations` axis list pushed to the shaper for
    /// the run's text range: `synthesis` followed by `variations` (later entries
    /// win per axis tag) is the user-space axis list the run was shaped with —
    /// the same list [`i_slint_core::textlayout::sharedparley::merged_variation_settings`]
    /// produces, which is what the typeface instance must match.
    pub fn font_with_variations(
        &mut self,
        font: &parley::FontData,
        synthesis: &fontique::Synthesis,
        variations: &[parley::style::FontVariation],
    ) -> Option<skia_safe::Typeface> {
        let mut variation_settings =
            i_slint_core::textlayout::sharedparley::merged_variation_settings(
                synthesis, variations,
            );
        self.without_unsupported_opsz(font, &mut variation_settings);
        let variations_hash = variation_settings_hash(&variation_settings);

        let key = (font.data.clone().into(), font.index, variations_hash);

        if let Some(cached) = self.fonts.get(&key) {
            return cached.clone();
        }

        let mut typeface = self.load_typeface_internal(font);

        if !variation_settings.is_empty() {
            // The run was shaped at these coordinates, so a typeface that can't
            // take the variation arguments would render the wrong shapes: the
            // caller rasterizes outlines at exact coordinates instead.
            typeface = typeface.and_then(|base| {
                let coords: Vec<skia_safe::font_arguments::variation_position::Coordinate> =
                    variation_settings
                        .iter()
                        .map(|&(tag, value)| {
                            skia_safe::font_arguments::variation_position::Coordinate {
                                axis: skia_safe::FourByteTag::new(tag),
                                value,
                            }
                        })
                        .collect();
                let position =
                    skia_safe::font_arguments::VariationPosition { coordinates: &coords };
                let args = skia_safe::FontArguments::new().set_variation_design_position(position);
                base.clone_with_arguments(&args)
            });
        }

        self.fonts.put(key, typeface.clone());
        typeface
    }

    /// Rasterizes `glyph` to an outline path at the exact `variation_settings`
    /// coordinates (the `(tag, value)` list from
    /// [`i_slint_core::textlayout::sharedparley::merged_variation_settings`]),
    /// in `pixel_size` pixels. This is the fallback for fonts whose typeface
    /// Skia can't clone with variation arguments — it draws the same shapes the
    /// shaper produced instead of silently dropping the requested axes.
    pub fn glyph_path(
        &mut self,
        font: &parley::FontData,
        glyph: u32,
        pixel_size: f32,
        variation_settings: &[(u32, f32)],
    ) -> Option<skia_safe::Path> {
        let mut variation_settings = variation_settings.to_vec();
        self.without_unsupported_opsz(font, &mut variation_settings);
        let key = (
            font.data.clone().into(),
            font.index,
            glyph,
            pixel_size.to_bits(),
            variation_settings_hash(&variation_settings),
        );
        if let Some(cached) = self.glyph_paths.get(&key) {
            return cached.clone();
        }

        let path = (|| {
            use skrifa::MetadataProvider;
            let font_ref = skrifa::FontRef::from_index(font.data.as_ref(), font.index).ok()?;
            let location =
                font_ref.axes().location(variation_settings.iter().map(|&(tag, value)| {
                    skrifa::setting::VariationSetting::from((
                        skrifa::Tag::new(&tag.to_be_bytes()),
                        value,
                    ))
                }));
            let outline = font_ref.outline_glyphs().get(skrifa::GlyphId::new(glyph))?;
            let mut pen = SkiaPathPen { path: skia_safe::PathBuilder::new() };
            outline
                .draw(
                    skrifa::outline::DrawSettings::unhinted(
                        skrifa::instance::Size::new(pixel_size),
                        &location,
                    ),
                    &mut pen,
                )
                .ok()?;
            Some(pen.path.detach())
        })();

        self.glyph_paths.put(key, path.clone());
        path
    }

    /// Drops the automatically injected `opsz` pair when the face has no
    /// `opsz` fvar axis: `font-optical-sizing: auto` on such a font must keep
    /// the base instance rather than clone a typeface carrying an axis it
    /// can't take (which also splits the cache per pixel size).
    fn without_unsupported_opsz(
        &mut self,
        font: &parley::FontData,
        variation_settings: &mut Vec<(u32, f32)>,
    ) {
        const OPSZ: u32 = u32::from_be_bytes(*b"opsz");
        if !variation_settings.iter().any(|&(tag, _)| tag == OPSZ) {
            return;
        }
        let has_opsz = *self
            .has_opsz_axis
            .entry((font.data.clone().into(), font.index))
            .or_insert_with(|| {
                use skrifa::MetadataProvider;
                skrifa::FontRef::from_index(font.data.as_ref(), font.index)
                    .map(|font_ref| {
                        font_ref.axes().iter().any(|axis| axis.tag() == skrifa::Tag::new(b"opsz"))
                    })
                    .unwrap_or_default()
            });
        if !has_opsz {
            variation_settings.retain(|&(tag, _)| tag != OPSZ);
        }
    }

    fn load_typeface_internal(&self, font: &parley::FontData) -> Option<skia_safe::Typeface> {
        let typeface = self.font_mgr.new_from_data(
            skia_safe::Data::new_copy(font.data.as_ref()),
            if font.index > 0 { Some(font.index as _) } else { None },
        );

        // Due to  https://issues.skia.org/issues/310510989, fonts from true type collections
        // with an index > 0 fail to load on macOS. As a workaround, we manually extract the font from the
        // collection and load it as a single font.
        #[cfg(target_vendor = "apple")]
        if font.index > 0
            && typeface.is_none()
            && let Some(typeface) = read_fonts::CollectionRef::new(font.data.as_ref())
                .ok()
                .and_then(|ttc| ttc.get(font.index).ok())
                .map(|ttf| write_fonts::FontBuilder::new().copy_missing_tables(ttf).build())
                .and_then(|new_ttf| {
                    self.font_mgr.new_from_data(skia_safe::Data::new_copy(&new_ttf), None)
                })
        {
            return Some(typeface);
        }

        typeface
    }
}

thread_local! {
    pub static FONT_CACHE: RefCell<FontCache> = RefCell::new(Default::default())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::time::Instant;

    fn inter_variable() -> parley::FontData {
        let data: &[u8] = include_bytes!("../../common/sharedfontique/Inter-VariableFont.ttf");
        parley::FontData::new(fontique::Blob::new(Arc::new(data)), 0)
    }

    fn wght(wght: f32) -> Vec<parley::style::FontVariation> {
        vec![parley::style::FontVariation::new(parley::setting::Tag::new(b"wght"), wght)]
    }

    /// The `wght` 100→900 sweep the design note budgets a frame around: every
    /// distinct axis combination misses the typeface cache once, then hits.
    /// Prints timings so `cargo test -- --nocapture` shows them.
    #[test]
    fn variation_sweep_uses_the_cache() {
        let mut cache = FontCache::default();
        let font = inter_variable();
        let synthesis = fontique::Synthesis::default();
        let steps: Vec<f32> = (0..=100).map(|i| 100. + i as f32 * 8.).collect();
        assert!(steps.len() > FONT_CACHE_CAPACITY.get());

        let cold = {
            let t0 = Instant::now();
            for &w in &steps {
                assert!(cache.font_with_variations(&font, &synthesis, &wght(w)).is_some());
            }
            t0.elapsed()
        };

        // An animation bouncing between a handful of axis settings is the hot
        // path: once seen, every frame must be a cache hit.
        let bounce: Vec<f32> =
            [100., 300., 500., 700., 900.].into_iter().cycle().take(200).collect();
        let warm = {
            let t0 = Instant::now();
            for &w in &bounce {
                assert!(cache.font_with_variations(&font, &synthesis, &wght(w)).is_some());
            }
            t0.elapsed()
        };
        eprintln!(
            "skia typeface cache: {} unique settings — cold {:?} ({:?}/step), \
             200-frame bounce {:?} ({:?}/frame)",
            steps.len(),
            cold,
            cold / steps.len() as u32,
            warm,
            warm / 200,
        );
        // Cache hits must be far cheaper than a typeface clone.
        assert!(warm < cold / 2 || warm < std::time::Duration::from_millis(10));
    }

    #[test]
    fn outline_fallback_paths_differ_per_axis() {
        let mut cache = FontCache::default();
        let font = inter_variable();
        // 'g' (glyph id arbitrary — look up a real one via charmap).
        let font_ref = skrifa::FontRef::from_index(font.data.as_ref(), 0).unwrap();
        let gid = {
            use skrifa::MetadataProvider;
            font_ref.charmap().map('g' as u32).unwrap().to_u32()
        };
        let thin = cache.glyph_path(&font, gid, 20., &[(u32::from_be_bytes(*b"wght"), 100.)]);
        let black = cache.glyph_path(&font, gid, 20., &[(u32::from_be_bytes(*b"wght"), 900.)]);
        assert!(thin.is_some() && black.is_some());
        assert_ne!(thin, black);
        // Same inputs hit the path cache (identical Path value returned).
        assert_eq!(
            cache.glyph_path(&font, gid, 20., &[(u32::from_be_bytes(*b"wght"), 100.)]),
            thin
        );
    }
}
