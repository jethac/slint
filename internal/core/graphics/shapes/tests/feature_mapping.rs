// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Port of upstream `FeatureMappingTest.kt`.

use super::super::mapping::{do_mapping, feature_dist_squared};
use super::super::*;
use super::utils::*;

use alloc::rc::Rc;
use alloc::vec::Vec;

fn regular(num_vertices: usize, rounding: CornerRounding) -> RoundedPolygon {
    RoundedPolygon::regular(num_vertices, 1., Point::ZERO, rounding, None).unwrap()
}

fn verify_mapping(p1: &RoundedPolygon, p2: &RoundedPolygon, validator: impl Fn(Vec<f32>)) {
    let measurer: Rc<dyn Measurer> = Rc::new(LengthMeasurer::default());
    let f1 = MeasuredPolygon::measure_polygon(measurer.clone(), p1).unwrap().features;
    let f2 = MeasuredPolygon::measure_polygon(measurer, p2).unwrap().features;

    // Maps progress in p1 to progress in p2
    let map = do_mapping(&f1, &f2).unwrap();

    // See which features where actually mapped and the distance between their representative
    // points
    let mut distances = Vec::new();
    for (progress1, progress2) in &map {
        let feature1 = f1.iter().find(|f| f.progress() == *progress1).unwrap();
        let feature2 = f2.iter().find(|f| f.progress() == *progress2).unwrap();
        distances.push(feature_dist_squared(feature1.feature(), feature2.feature()));
    }

    distances.sort_by(|a, b| b.total_cmp(a));
    validator(distances);
}

#[test]
fn feature_mapping_triangles() {
    let triangle_with_roundings = regular(3, CornerRounding::new(0.2, 0.));
    let triangle = regular(3, CornerRounding::UNROUNDED);
    verify_mapping(&triangle_with_roundings, &triangle, |distances| {
        distances.iter().for_each(|d| assert!(*d < 0.1))
    });
}

#[test]
fn feature_mapping_triangle_to_square() {
    let triangle = regular(3, CornerRounding::UNROUNDED);
    let square = regular(4, CornerRounding::UNROUNDED);
    verify_mapping(&triangle, &square, |distances| {
        // We have one exact match (both have points at 0 degrees), and 2 close ones
        assert_eq!(3, distances.len());
        assert_equalish(distances[0], distances[1]);
        assert!(distances[0] < 0.3);
        assert!(distances[2] < 1e-6);
    });
}

#[test]
fn feature_mapping_square_to_triangle() {
    let triangle = regular(3, CornerRounding::UNROUNDED);
    let square = regular(4, CornerRounding::UNROUNDED);
    verify_mapping(&square, &triangle, |distances| {
        // We have one exact match (both have points at 0 degrees), and 2 close ones
        assert_eq!(3, distances.len());
        assert_equalish(distances[0], distances[1]);
        assert!(distances[0] < 0.3);
        assert!(distances[2] < 1e-6);
    });
}

#[test]
fn feature_mapping_does_not_crash() {
    // Verify that complicated shapes can be matched (this used to crash before).
    let checkmark = RoundedPolygon::from_vertices(
        &[
            400., -304., 240., -464., 296., -520., 400., -416., 664., -680., 720., -624., 400.,
            -304.,
        ],
        CornerRounding::UNROUNDED,
        None,
        None,
    )
    .unwrap()
    .normalized()
    .unwrap();
    let very_sunny = super::super::constructors::star(
        8,
        1.,
        0.65,
        CornerRounding::new(0.15, 0.),
        None,
        None,
        Point::ZERO,
    )
    .unwrap()
    .normalized()
    .unwrap();
    verify_mapping(&checkmark, &very_sunny, |distances| {
        // Most vertices on the checkmark map to a feature in the second shape.
        assert!(distances.len() >= 6);

        // And they are close enough
        assert!(distances[0] < 0.15);
    });
}
