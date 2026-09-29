// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Parity test for the generated `MaterialShapes` global: evaluates every one
//! of the 35 `Shapes.*` expressions the token generator emitted into
//! `ui-libraries/material/src/ui/styling/generated/material_shapes.slint` and
//! compares the resulting outlines against the Kotlin goldens in
//! `tests/shapes/golden/material_shapes.json` (pinned androidx commit
//! `23327507f7fc7d5b19d65fec4b090f60c970079b`, f32 values stored as IEEE-754
//! bit patterns).
//!
//! Comparison is bit-exact by default. Shapes that go through
//! `Shapes.rotated`/`Shapes.scaled` may differ in the last ulp from Kotlin's
//! `Matrix().apply { rotateZ(..) }`/`scale(..)` path (the matrix composition
//! differs, and the transform is then re-normalized); those names are listed
//! in `TOLERANT_SHAPES` below and held to a 1e-4 absolute bound, mirroring the
//! tolerance table in `tests/shapes/golden/README.md`.

use i_slint_core::graphics::shapes::Cubic;
use slint_interpreter::{Compiler, Value};

/// `(slint property, golden key)` pairs — all 35 `MaterialShapes` members.
const SHAPES: &[(&str, &str)] = &[
    ("circle", "circle"),
    ("square", "square"),
    ("slanted", "slanted"),
    ("arch", "arch"),
    ("fan", "fan"),
    ("arrow", "arrow"),
    ("semi-circle", "semi_circle"),
    ("oval", "oval"),
    ("pill", "pill"),
    ("triangle", "triangle"),
    ("diamond", "diamond"),
    ("clam-shell", "clam_shell"),
    ("pentagon", "pentagon"),
    ("gem", "gem"),
    ("sunny", "sunny"),
    ("very-sunny", "very_sunny"),
    ("cookie-4-sided", "cookie_4_sided"),
    ("cookie-6-sided", "cookie_6_sided"),
    ("cookie-7-sided", "cookie_7_sided"),
    ("cookie-9-sided", "cookie_9_sided"),
    ("cookie-12-sided", "cookie_12_sided"),
    ("ghostish", "ghostish"),
    ("clover-4-leaf", "clover_4_leaf"),
    ("clover-8-leaf", "clover_8_leaf"),
    ("burst", "burst"),
    ("soft-burst", "soft_burst"),
    ("boom", "boom"),
    ("soft-boom", "soft_boom"),
    ("flower", "flower"),
    ("puffy", "puffy"),
    ("puffy-diamond", "puffy_diamond"),
    ("pixel-circle", "pixel_circle"),
    ("pixel-triangle", "pixel_triangle"),
    ("bun", "bun"),
    ("heart", "heart"),
];

/// Members allowed a 1e-4 absolute tolerance instead of bit-exact equality —
/// see the module doc. Keep in sync with `tests/shapes/golden/README.md`.
const TOLERANT_SHAPES: &[&str] =
    &["triangle", "puffy", "cookie_7_sided", "cookie_9_sided", "cookie_12_sided"];

fn f32_bits(v: &serde_json::Value) -> f32 {
    f32::from_bits(v.as_u64().unwrap() as u32)
}

#[test]
fn material_shapes_match_golden() {
    i_slint_backend_testing::init_no_event_loop();

    let golden: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/shapes/golden/material_shapes.json"
        ))
        .unwrap(),
    )
    .unwrap();
    let shapes = &golden["shapes"];

    let mut properties = String::new();
    for (slint_name, _) in SHAPES {
        properties += &format!("out property <shape> {slint_name}: MaterialShapes.{slint_name};\n");
    }
    let code = format!(
        "import {{ MaterialShapes }} from \"material_shapes.slint\";\n\
         export component TestCase {{\n{properties}}}\n"
    );

    let mut compiler = Compiler::default();
    compiler.set_include_paths(std::vec::Vec::from([std::path::PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../ui-libraries/material/src/ui/styling/generated"
    ))]));
    let result = spin_on::spin_on(compiler.build_from_source(code.into(), Default::default()));
    assert!(!result.has_errors(), "{:?}", result.diagnostics().collect::<Vec<_>>());
    let instance = result.component("TestCase").unwrap().create().unwrap();

    for (slint_name, golden_name) in SHAPES {
        let label = format!("{golden_name} (MaterialShapes.{slint_name})");
        let Value::Shape(shape) = instance.get_property(slint_name).unwrap() else {
            panic!("{label}: not a shape");
        };
        let polygon = shape.polygon().unwrap();
        let entry = &shapes[golden_name];

        let cubics: &[Cubic] = polygon.cubics();
        let expected = entry["cubics"].as_array().unwrap();
        assert_eq!(expected.len(), cubics.len(), "{label}: cubic count");

        let tolerant = TOLERANT_SHAPES.contains(golden_name);
        let mut mismatched = 0;
        let mut max_diff = 0f32;
        for (cubic, e) in cubics.iter().zip(expected.iter()) {
            for (j, a) in cubic.points.iter().enumerate() {
                let expected = f32_bits(&e[j]);
                if expected.to_bits() != a.to_bits() {
                    mismatched += 1;
                    max_diff = max_diff.max((expected - a).abs());
                    if !tolerant {
                        panic!(
                            "{label}: cubic[{j}..] not bit-exact: expected {expected} \
                             ({:#x}) got {a} ({:#x}) — if this is a last-ulp libm \
                             difference, add the member to TOLERANT_SHAPES",
                            expected.to_bits(),
                            a.to_bits()
                        );
                    }
                }
            }
        }
        if tolerant {
            assert!(
                max_diff <= 1e-4,
                "{label}: {mismatched} cubic components differ by more than 1e-4 \
                 (max {max_diff})"
            );
        }

        let center = entry["center"].as_array().unwrap();
        assert_eq!(
            f32_bits(&center[0]).to_bits(),
            polygon.center_x().to_bits(),
            "{label}: centerX"
        );
        assert_eq!(
            f32_bits(&center[1]).to_bits(),
            polygon.center_y().to_bits(),
            "{label}: centerY"
        );
    }
}
