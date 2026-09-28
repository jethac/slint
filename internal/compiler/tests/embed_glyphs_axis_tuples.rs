// Copyright © Klarälvdalens Datakonsult AB, a KDAB Group company, info@kdab.com, author David Faure <david.faure@kdab.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore WDTX wdth opsz cmap subsetted subsetting

//! Under `EmbedForSoftwareRenderer` the compiler rasterizes one bitmap font per
//! constant axis tuple, embeds the vector font data for axis bindings that are
//! not compile-time constant, and reports them as errors only when vector fonts
//! are excluded from the build.

#![cfg(feature = "renderer-software")]

use i_slint_compiler::diagnostics::BuildDiagnostics;
use i_slint_compiler::embedded_resources::EmbeddedResourcesKind;
use i_slint_compiler::generator::OutputFormat;
use i_slint_compiler::object_tree::Document;
use i_slint_compiler::parser::parse;
use i_slint_compiler::{CompilerConfiguration, compile_syntax_node};

fn compile(source: &str, exclude_vector_fonts: bool) -> (Document, Vec<String>) {
    let (doc, diags, _) = compile_full(source, exclude_vector_fonts);
    (doc, diags)
}

fn compile_full(
    source: &str,
    exclude_vector_fonts: bool,
) -> (Document, Vec<String>, i_slint_compiler::typeloader::TypeLoader) {
    // SAFETY: single-threaded test binary usage pattern; the font collection is
    // initialized per compile, before any other test reads the variable.
    unsafe {
        std::env::set_var(
            "SLINT_FONT_PATH",
            concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/screenshots/fonts"),
        );
    }
    let mut diag = BuildDiagnostics::default();
    let syntax_node = parse(source.into(), Some(std::path::Path::new("main.slint")), &mut diag);
    let mut config = CompilerConfiguration::new(OutputFormat::Llr);
    config.embed_resources = i_slint_compiler::EmbedResourcesKind::EmbedTextures;
    config.exclude_vector_fonts = exclude_vector_fonts;
    let (doc, diag, loader) = spin_on::spin_on(compile_syntax_node(syntax_node, diag, config));
    (doc, diag.to_string_vec(), loader)
}

fn bitmap_variations(doc: &Document) -> Vec<(String, u16, Vec<(u32, f32)>)> {
    doc.embedded_file_resources
        .borrow()
        .iter()
        .filter_map(|r| match &r.kind {
            EmbeddedResourcesKind::BitmapFontData(font) => Some((
                font.family_name.clone(),
                font.weight,
                font.variations.iter().map(|v| (v.tag, v.value)).collect::<Vec<_>>(),
            )),
            _ => None,
        })
        .collect()
}

fn embedded_vector_font_paths(doc: &Document) -> Vec<String> {
    let mut paths: Vec<String> = doc
        .embedded_file_resources
        .borrow()
        .iter()
        .filter_map(|r| {
            matches!(
                &r.kind,
                EmbeddedResourcesKind::FileData | EmbeddedResourcesKind::DataUriPayload(..)
            )
            .then(|| r.path.as_ref().map(|p| p.to_string()))?
        })
        .collect();
    paths.sort();
    paths.dedup();
    paths
}

fn embedded_vector_font_bytes(doc: &Document) -> Vec<(String, Vec<u8>)> {
    doc.embedded_file_resources
        .borrow()
        .iter()
        .filter_map(|r| {
            let path = r.path.as_ref()?.to_string();
            match &r.kind {
                EmbeddedResourcesKind::DataUriPayload(bytes, _) => Some((path, bytes.clone())),
                EmbeddedResourcesKind::FileData => {
                    Some((path.clone(), std::fs::read(&path).unwrap()))
                }
                _ => None,
            }
        })
        .collect()
}

fn embedded_vector_fonts(doc: &Document) -> usize {
    embedded_vector_font_paths(doc).len()
}

const WDTX: u32 = u32::from_be_bytes(*b"wdth");

/// A bound `default-font-family` can't be pinned to a bitmap instance: const-
/// propagated literals still resolve, genuinely dynamic bindings take the
/// vector path (or error when vector fonts are excluded).
const SOURCE_BOUND_FAMILY: &str = r#"
export component Main inherits Window {
    in property <string> family;
    default-font-family: family;
    Text { text: "x"; }
}
"#;

#[test]
fn bound_default_font_family_embeds_vector_font() {
    let (doc, diags) = compile(SOURCE_BOUND_FAMILY, false);
    assert!(diags.iter().all(|d| !d.starts_with("error")), "{diags:?}");
    assert!(embedded_vector_fonts(&doc) > 0, "the font bytes must be embedded");
}

#[test]
fn bound_default_font_family_errors_when_vector_fonts_excluded() {
    let (_doc, diags) = compile(SOURCE_BOUND_FAMILY, true);
    assert!(diags.iter().any(|d| d.contains("'default-font-family'")), "{diags:?}");
}

#[test]
fn const_propagated_family_stays_bitmap() {
    let (doc, diags) = compile(
        r#"export component Main inherits Window {
            out property <string> family: "Noto Sans";
            default-font-family: family;
            Text { text: "x"; }
        }"#,
        true,
    );
    assert!(diags.iter().all(|d| !d.starts_with("error")), "{diags:?}");
    assert_eq!(embedded_vector_fonts(&doc), 0);
}

/// The test fonts have wght 100-900 and wdth 62.5-100 (no opsz).
const SOURCE_CONSTANT: &str = r#"
export component Main inherits Window {
    Text { font-family: "Noto Sans"; font-stretch: 75%; text: "narrow"; }
    Text {
        font-family: "Noto Sans";
        font-weight: 700;
        font-variation-settings: [{ tag: "wdth", value: 62.5 }, { tag: "GRAD", value: 0 }];
        text: "heavy";
    }
}
"#;

#[test]
fn constant_tuples_become_dedicated_bitmap_instances() {
    let (doc, diags) = compile(SOURCE_CONSTANT, false);
    assert!(diags.iter().all(|d| !d.starts_with("error")), "{diags:?}");
    let tuples = bitmap_variations(&doc);
    assert!(tuples.iter().any(|(_, _, t)| t.contains(&(WDTX, 75.0))), "{tuples:?}");
    assert!(
        tuples.iter().any(|(_, weight, t)| *weight == 700 && t.contains(&(WDTX, 62.5))),
        "the 700/62.5 text must produce its own tuple: {tuples:?}"
    );
    // font-stretch: 75% is the wdth value as written — not 0.75 and not ×100
    assert!(
        tuples.iter().all(|(_, _, t)| t.iter().all(|&(tag, v)| tag != WDTX || v <= 100.0)),
        "{tuples:?}"
    );
}

#[test]
fn font_stretch_zero_means_default_not_narrowest() {
    let (doc, diags) = compile(
        r#"export component Main inherits Window {
            Text { font-family: "Noto Sans"; font-stretch: 0%; text: "x"; }
        }"#,
        false,
    );
    assert!(diags.iter().all(|d| !d.starts_with("error")), "{diags:?}");
    // 0% is the "unset" sentinel: it must not pin wdth to the axis minimum
    // (62.5). Only the default-instance tuple (wdth 100) may appear.
    assert!(
        bitmap_variations(&doc)
            .iter()
            .flat_map(|(_, _, t)| t.iter())
            .all(|&(t, v)| t != WDTX || v == 100.0)
    );
}

const SOURCE_DYNAMIC: &str = r#"
export component Main inherits Window {
    in property <float> w;
    Text {
        font-family: "Noto Sans";
        font-variation-settings: [{ tag: "wght", value: w }];
        text: "sweep";
    }
    Text {
        font-family: "Noto Sans";
        text: "static";
        animate font-variation-settings { duration: 1s; }
    }
}
"#;

#[test]
fn non_constant_binding_embeds_vector_font() {
    let (doc, diags) = compile(SOURCE_DYNAMIC, false);
    assert!(diags.iter().all(|d| !d.starts_with("error")), "{diags:?}");
    assert!(embedded_vector_fonts(&doc) > 0, "the font bytes must be embedded");
}

#[test]
fn non_constant_binding_errors_only_when_vector_fonts_excluded() {
    let (_doc, diags) = compile(SOURCE_DYNAMIC, true);
    let errors: Vec<_> =
        diags.iter().filter(|d| d.contains("'font-variation-settings' is not constant")).collect();
    assert_eq!(errors.len(), 2, "the bound and the animated bindings must both error: {diags:?}");
}

/// Each `<font>` span must produce its own bitmap instance: the span's axes
/// merge over the element's, they never leak into one shared tuple.
#[test]
fn markup_font_attributes_produce_one_tuple_per_span() {
    let (doc, diags) = compile(
        r#"export component Main inherits Window {
            default-font-stretch: 50%;
            StyledText {
                text: @markdown("normal <font font-stretch=\"75%\">narrow</font> <font font-variation-settings=\"'GRAD' 50\">graded</font>");
            }
        }"#,
        false,
    );
    assert!(diags.iter().all(|d| !d.starts_with("error")), "{diags:?}");
    // The recorded variations of every embedded Noto Sans instance,
    // deduplicated:
    // - wdth 62.5: the window's `default-font-stretch: 50%` clamped to the
    //   axis minimum — plain runs, the window tuple, and the GRAD-only span
    //   (GRAD is no fvar axis of Noto Sans) all rasterize to this instance,
    // - wdth 75: the `font-stretch="75%"` span's own tuple,
    // - wdth 100: the always-embedded default instance at the axis defaults.
    let mut seen: Vec<Vec<(u32, f32)>> = bitmap_variations(&doc)
        .into_iter()
        .filter(|(family, _, _)| family == "Noto Sans")
        .map(|(_, _, variations)| variations)
        .collect();
    seen.sort_by(|a, b| {
        a.iter().map(|(t, v)| (*t, v.to_bits())).cmp(b.iter().map(|(t, v)| (*t, v.to_bits())))
    });
    seen.dedup();
    assert_eq!(
        seen,
        vec![vec![(WDTX, 62.5)], vec![(WDTX, 75.0)], vec![(WDTX, 100.0)]],
        "one tuple per span (window wdth 50→62.5 / span wdth 75 / default wdth 100)"
    );
    // Roboto Flex declares GRAD: the `font-variation-settings` span must
    // rasterize its own instance with GRAD at 50 — proof the span isn't folded
    // into the element tuple.
    const GRAD: u32 = u32::from_be_bytes(*b"GRAD");
    // Roboto Flex declares GRAD: the `font-variation-settings` span must
    // rasterize its own instance with GRAD at 50 — proof the span isn't folded
    // into the element tuple. Exact check: the span tuple equals the default
    // instance's tuple (GRAD 0, wdth 100) with only GRAD and wdth replaced.
    let roboto_tuples: Vec<Vec<(u32, f32)>> = bitmap_variations(&doc)
        .into_iter()
        .filter(|(f, _, _)| f == "Roboto Flex")
        .map(|(_, _, v)| v)
        .collect();
    let default = roboto_tuples
        .iter()
        .find(|v| v.contains(&(GRAD, 0.0)) && v.contains(&(WDTX, 100.0)))
        .expect("the default Roboto Flex instance must be embedded");
    let expected: Vec<(u32, f32)> = default
        .iter()
        .map(|&(tag, value)| {
            let value = if tag == GRAD || tag == WDTX { 50.0 } else { value };
            (tag, value)
        })
        .collect();
    assert!(
        roboto_tuples.contains(&expected),
        "the GRAD span's exact tuple {expected:?} missing from {roboto_tuples:?}"
    );
}

/// Bound properties that constant propagation folds still rasterize bitmap
/// instances: an `out property` chain to a literal is a constant, so the
/// collectors see the same number or string the source chain resolves to.
#[test]
fn const_propagated_weight_family_and_size_stay_bitmap() {
    let (doc, diags) = compile(
        r#"export component Main inherits Window {
            property <int> w: 700;
            property <string> fam: "Noto Sans";
            property <length> s: 14px;
            Text { font-family: fam; font-weight: w; font-size: s; text: "x"; }
        }"#,
        true,
    );
    assert!(diags.iter().all(|d| !d.starts_with("error")), "{diags:?}");
    assert_eq!(embedded_vector_fonts(&doc), 0);
    assert!(
        bitmap_variations(&doc).iter().any(|(f, w, _)| f == "Noto Sans" && *w == 700),
        "the folded 700 weight must produce a bitmap instance: {:?}",
        bitmap_variations(&doc)
    );
}

/// A `font-weight`/`font-size` that isn't compile-time constant takes the
/// vector path like the axis bindings, and errors with the binding's property
/// name when vector fonts are excluded.
#[test]
fn non_constant_weight_and_size_take_the_vector_path() {
    const SOURCE: &str = r#"
export component Main inherits Window {
    in property <int> w;
    in property <length> s;
    Text { font-family: "Noto Sans"; font-weight: w; font-size: s; text: "x"; }
}
"#;
    let (_doc, diags) = compile(SOURCE, true);
    assert!(diags.iter().any(|d| d.contains("'font-weight'")), "{diags:?}");
    assert!(diags.iter().any(|d| d.contains("'font-size'")), "{diags:?}");

    let (doc, diags) = compile(SOURCE, false);
    assert!(diags.iter().all(|d| !d.starts_with("error")), "{diags:?}");
    assert!(embedded_vector_fonts(&doc) > 0, "the font bytes must be embedded");
}

/// Dynamic axis bindings embed the vector font data, but only for the families
/// the bindings can actually resolve to — flash on the MCU target is scarce.
#[test]
fn dynamic_binding_embeds_only_the_named_family() {
    let (doc, diags) = compile(
        r#"export component Main inherits Window {
            in property <float> w;
            Text {
                font-family: "Roboto Flex";
                font-variation-settings: [{ tag: "wght", value: w }];
                text: "flex";
            }
            Text { font-family: "Noto Sans"; text: "static"; }
        }"#,
        false,
    );
    assert!(diags.iter().all(|d| !d.starts_with("error")), "{diags:?}");
    let paths = embedded_vector_font_paths(&doc);
    assert_eq!(paths.len(), 1, "only Roboto Flex can be reached: {paths:?}");
    assert!(paths[0].ends_with("RobotoFlex.ttf"), "{paths:?}");
}

#[test]
fn dynamic_binding_without_family_embeds_the_default_set() {
    let (doc, diags) = compile(
        r#"export component Main inherits Window {
            in property <float> w;
            Text { font-variation-settings: [{ tag: "wght", value: w }]; text: "flex"; }
        }"#,
        false,
    );
    assert!(diags.iter().all(|d| !d.starts_with("error")), "{diags:?}");
    // No `font-family` binding: the default font set covers the request. The
    // named-family element from the previous test is absent, so nothing beyond
    // the default set may appear.
    assert!(embedded_vector_fonts(&doc) > 0, "default fonts must be embedded");
}

/// The embedded vector font is subsetted down to the coverage the app
/// collects: smaller than the source file, still parseable, with the full
/// variation space and the family name kept (the runtime matches by name).
#[test]
fn embedded_vector_font_is_subsetted_to_collected_coverage() {
    use skrifa::MetadataProvider as _;

    let (doc, diags) = compile(SOURCE_DYNAMIC, false);
    assert!(diags.iter().all(|d| !d.starts_with("error")), "{diags:?}");
    let embedded = embedded_vector_font_bytes(&doc);
    let subset = &embedded
        .iter()
        .find(|(path, _)| path.ends_with("NotoSans-Regular.ttf"))
        .expect("Noto Sans is embedded")
        .1;

    let source_path =
        concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/screenshots/fonts/NotoSans-Regular.ttf");
    let source = std::fs::read(source_path).unwrap();
    // Smaller, and the cmap check below proves it is the uncollected glyphs
    // that went away — not e.g. the name table.
    assert!(subset.len() < source.len(), "subset {}B vs source {}B", subset.len(), source.len());

    let face = skrifa::FontRef::new(subset).expect("subset must parse as a font");
    let source_face = skrifa::FontRef::new(&source).unwrap();
    // Same variation space: a dynamic binding can drive any axis.
    assert_eq!(face.axes().len(), source_face.axes().len(), "all axes must survive subsetting");
    // Family matching needs the name records.
    let family = face
        .localized_strings(skrifa::string::StringId::FAMILY_NAME)
        .next()
        .expect("subset must keep the family name");
    assert_eq!(family.to_string(), "Noto Sans");

    let charmap = face.charmap();
    // Collected: the strings in the source plus the always-added ASCII set.
    for c in "sweep".chars() {
        assert!(charmap.map(c).is_some(), "'{c}' must survive subsetting");
    }
    // A codepoint outside the collected coverage is gone.
    assert!(charmap.map('\u{4e2d}').is_none());
}

/// `slint!`-macro builds can't see the target, so generated code asserts on
/// the rasterizer feature instead of relying on build-time detection.
#[cfg(feature = "rust")]
#[test]
fn rust_codegen_emits_vector_rasterizer_assert() {
    let (doc, diags, loader) = compile_full(SOURCE_DYNAMIC, false);
    assert!(diags.iter().all(|d| !d.starts_with("error")), "{diags:?}");
    let mut out = Vec::new();
    i_slint_compiler::generator::generate(
        OutputFormat::Rust,
        &mut out,
        None,
        &doc,
        &loader.compiler_config,
    )
    .unwrap();
    let generated = String::from_utf8(out).unwrap();
    assert!(
        generated.contains("HAS_EMBEDDED_VECTOR_FONT_SUPPORT"),
        "the const assert guards user-crate builds without a rasterizer feature"
    );
}
