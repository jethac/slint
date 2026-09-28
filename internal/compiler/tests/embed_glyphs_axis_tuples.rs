// Copyright © Klarälvdalens Datakonsult AB, a KDAB Group company, info@kdab.com, author David Faure <david.faure@kdab.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

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
    let (doc, diag, _loader) = spin_on::spin_on(compile_syntax_node(syntax_node, diag, config));
    (doc, diag.to_string_vec())
}

fn bitmap_variations(doc: &Document) -> Vec<(u16, Vec<(u32, f32)>)> {
    doc.embedded_file_resources
        .borrow()
        .iter()
        .filter_map(|r| match &r.kind {
            EmbeddedResourcesKind::BitmapFontData(font) => Some((
                font.weight,
                font.variations.iter().map(|v| (v.tag, v.value)).collect::<Vec<_>>(),
            )),
            _ => None,
        })
        .collect()
}

fn embedded_vector_fonts(doc: &Document) -> usize {
    doc.embedded_file_resources
        .borrow()
        .iter()
        .filter(|r| matches!(&r.kind, EmbeddedResourcesKind::FileData))
        .count()
}

const WDTX: u32 = u32::from_be_bytes(*b"wdth");

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
    assert!(tuples.iter().any(|(_, t)| t.contains(&(WDTX, 75.0))), "{tuples:?}");
    assert!(
        tuples.iter().any(|(weight, t)| *weight == 700 && t.contains(&(WDTX, 62.5))),
        "the 700/62.5 text must produce its own tuple: {tuples:?}"
    );
    // font-stretch: 75% is the wdth value as written — not 0.75 and not ×100
    assert!(
        tuples.iter().all(|(_, t)| t.iter().all(|&(tag, v)| tag != WDTX || v <= 100.0)),
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
            .flat_map(|(_, t)| t.iter())
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

#[test]
fn markup_font_attributes_are_collected() {
    let (doc, diags) = compile(
        r#"export component Main inherits Window {
            StyledText {
                text: "normal <font font-stretch=\"75%\">narrow</font> <font font-variation-settings=\"'GRAD' 50\">graded</font>";
            }
        }"#,
        false,
    );
    assert!(diags.iter().all(|d| !d.starts_with("error")), "{diags:?}");
    let tuples = bitmap_variations(&doc);
    // font-stretch="75%" in a <font> tag collects wdth 75 (the test font has
    // no GRAD axis, so unknown axes don't reach the rasterized variations).
    assert!(tuples.iter().any(|(_, t)| t.contains(&(WDTX, 75.0))), "{tuples:?}");
}
