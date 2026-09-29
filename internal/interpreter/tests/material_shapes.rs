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

use i_slint_core::graphics::shapes::{Cubic, Feature, LengthMeasurer, MeasuredPolygon, Measurer};
use slint_interpreter::{Compiler, Value};
use std::rc::Rc;

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

        // The feature segmentation (corners + edges, each with its cubic
        // count) — this is what morph feature matching groups on.
        let expected_features = entry["features"].as_array().unwrap();
        let features = polygon.features();
        assert_eq!(expected_features.len(), features.len(), "{label}: feature count");
        for (i, (feature, expected)) in features.iter().zip(expected_features.iter()).enumerate() {
            let (kind, count, convex) = match feature {
                Feature::Edge(cubics) => ("edge", cubics.len(), None),
                Feature::Corner { cubics, convex } => ("corner", cubics.len(), Some(*convex)),
            };
            assert_eq!(expected["kind"].as_str().unwrap(), kind, "{label}: feature[{i}] kind");
            assert_eq!(
                expected["count"].as_u64().unwrap() as usize,
                count,
                "{label}: feature[{i}] count"
            );
            if let Some(convex) = convex {
                assert_eq!(
                    expected["convex"].as_bool().unwrap(),
                    convex,
                    "{label}: feature[{i}] convex"
                );
            }
        }

        // The measured corner progress along the outline — the morph
        // mapper's input. `start_offset` is the feature's progress in
        // [0, 1); for trig-derived shapes the arc-length progression may
        // differ in the last ulp, so TOLERANT_SHAPES gets the 1e-4 bound.
        let measured = MeasuredPolygon::measure_polygon(
            Rc::new(LengthMeasurer::default()) as Rc<dyn Measurer>,
            &polygon,
        )
        .expect("{label}: measure_polygon");
        let expected_measured = entry["measured_features"].as_array().unwrap();
        assert_eq!(
            expected_measured.len(),
            measured.features.len(),
            "{label}: measured feature count"
        );
        for (i, (pf, expected)) in
            measured.features.iter().zip(expected_measured.iter()).enumerate()
        {
            let Feature::Corner { cubics, convex } = pf.feature() else {
                panic!("{label}: measured feature[{i}] is not a corner")
            };
            assert_eq!(
                expected["convex"].as_bool().unwrap(),
                *convex,
                "{label}: measured feature[{i}] convex"
            );
            assert_eq!(
                expected["count"].as_u64().unwrap() as usize,
                cubics.len(),
                "{label}: measured feature[{i}] count"
            );
            let expected_offset = f32_bits(&expected["start_offset"]);
            if expected_offset.to_bits() != pf.progress().to_bits() {
                let diff = (expected_offset - pf.progress()).abs();
                assert!(
                    tolerant && diff <= 1e-4,
                    "{label}: measured feature[{i}] start_offset: expected \
                     {expected_offset} ({:#x}) got {} ({:#x})",
                    expected_offset.to_bits(),
                    pf.progress(),
                    pf.progress().to_bits()
                );
            }
        }
    }
}
