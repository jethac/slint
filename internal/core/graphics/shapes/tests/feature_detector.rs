// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Port of upstream `FeatureDetectorTest.kt`.

use super::super::*;
use super::utils::*;
use alloc::vec;
use alloc::vec::Vec;

fn pill_star_polygon(rounding: CornerRounding) -> RoundedPolygon {
    // Upstream `RoundedPolygon.pillStar()` defaults.
    super::super::constructors::pill_star(
        2.,
        1.,
        8,
        0.5,
        rounding,
        None,
        None,
        0.5,
        0.,
        Point::ZERO,
    )
    .unwrap()
}

#[test]
fn recognizes_straightness() {
    assert!(Cubic::straight_line(0., 0., 1., 0.).straight_ish());
}

#[test]
fn recognizes_straightness_ish() {
    let slightly_not_straight_cubic =
        Cubic::from_floats(323.508, 201.759, 317.35, 192.008, 311.193, 182.227, 305.035, 172.475);
    assert!(slightly_not_straight_cubic.straight_ish());
}

#[test]
fn recognizes_curvature() {
    let round_cubic = Cubic::from_floats(0., 0., 0.5, 0.5, 0.5, 0.5, 1., 0.);
    assert!(!round_cubic.straight_ish());
}

#[test]
fn recognizes_smoothness_for_curved_cubic() {
    let base_cubic = Cubic::from_floats(0., 0., 0., 10., 10., 10., 10., 0.);
    let smooth_continuation = Cubic::from_floats(10., 0., 10., -10., 20., -10., 20., 0.);

    assert!(base_cubic.smoothes_into_ish(&smooth_continuation));
}

#[test]
fn recognizes_smoothness_for_straight_cubic() {
    let base_cubic = Cubic::straight_line(0., 0., 10., 0.);
    let smooth_continuation = Cubic::straight_line(10., 0., 20., 0.);

    assert!(base_cubic.smoothes_into_ish(&smooth_continuation));
}

#[test]
fn recognizes_smoothness_within_relative_tolerance() {
    // These two cubics are from the edge of an imported shape. Even though they don't
    // count as smooth within the absolute distance epsilon, relatively seen they should count.
    let base_cubic =
        Cubic::from_floats(323.508, 201.759, 317.35, 192.008, 311.193, 182.227, 305.008, 172.475);
    let smooth_continuation =
        Cubic::from_floats(305.008, 172.475, 290.812, 149.962, 276.617, 127.42, 262.422, 104.907);

    assert!(base_cubic.smoothes_into_ish(&smooth_continuation));
}

#[test]
fn empty_cubics_are_not_straight_ish() {
    assert!(!Cubic::empty(10., 10.).straight_ish());
}

#[test]
fn recognizes_alignment_for_straight_lines() {
    let base_cubic = Cubic::straight_line(0., 0., 10., 0.);
    let smooth_continuation = Cubic::straight_line(10., 0., 20., 0.);

    assert!(base_cubic.aligns_ish_with(&smooth_continuation));
}

#[test]
fn recognizes_alignment_within_relative_tolerance() {
    // These two cubics are from the edge of an imported shape. Even though the second edge
    // is very small within the given scale, it is not empty. However, even the length of
    // 0.027 is so relatively tiny in the given range of coordinates, that it should be seen as
    // an empty cubic. Therefore, the second can be seen as an extend of the first.
    let base_cubic =
        Cubic::from_floats(323.508, 201.759, 317.35, 192.008, 311.193, 182.227, 305.035, 172.475);
    let smooth_continuation = Cubic::straight_line(305.035, 172.475, 305.008, 172.475);

    assert!(base_cubic.aligns_ish_with(&smooth_continuation));
}

#[test]
fn includes_alignment_for_empty_cubics() {
    let base = Cubic::straight_line(0., 0., 10., 0.);
    let empty = Cubic::empty(10., 0.);

    assert!(base.aligns_ish_with(&empty));
    assert!(empty.aligns_ish_with(&base));
}

#[test]
fn converts_straight_cubic_to_edge() {
    let cubic = Cubic::straight_line(0., 0., 10., 0.);
    let following_cubic = Cubic::straight_line(10., 0., 20., 0.);

    let converted = cubic.as_feature(&following_cubic);
    let expected = Feature::Edge(vec![cubic]);

    assert!(matches!(converted, Feature::Edge(_)));
    assert_features_equalish(&expected, &converted);
}

#[test]
fn converts_curved_cubic_to_corner() {
    let cubic = Cubic::from_floats(0., 0., 0.5, 0.5, 0.5, 0.5, 1., 0.);
    let following_cubic = Cubic::from_floats(1., 0., 1.5, 1.5, 1.5, 1.5, 2., 0.);

    let converted = cubic.as_feature(&following_cubic);
    let expected = Feature::Corner { cubics: vec![cubic], convex: false };

    assert!(matches!(converted, Feature::Corner { .. }));
    assert_features_equalish(&expected, &converted);
}

#[test]
fn converts_empty_cubic_to_corner() {
    let cubic = Cubic::empty(1., 0.);
    let following_cubic = Cubic::from_floats(1., 0., 1.5, 1.5, 1.5, 1.5, 2., 0.);

    let converted = cubic.as_feature(&following_cubic);
    let expected = Feature::Corner { cubics: vec![cubic], convex: false };

    assert!(matches!(converted, Feature::Corner { .. }));
    assert_features_equalish(&expected, &converted);
}

#[test]
fn reconstructs_pill_star() {
    let original_polygon = pill_star_polygon(CornerRounding::UNROUNDED);
    let split_cubics: Vec<Cubic> = original_polygon
        .cubics()
        .iter()
        .flat_map(|cubic| {
            let (a, b) = cubic.split(0.5);
            [a, b]
        })
        .collect();

    let created_polygon = RoundedPolygon::from_features(
        super::super::feature::detect_features(&split_cubics),
        Some(original_polygon.center()),
    )
    .unwrap();

    // It's okay if the cubics' control points aren't the same, as long as the shape is the same
    assert_eq!(original_polygon.cubics().len(), created_polygon.cubics().len());
    for (i, new) in created_polygon.cubics().iter().enumerate() {
        let original = &original_polygon.cubics()[i];

        // pillStar has no roundings, so the created cubics shouldn't be as well
        assert!(new.straight_ish());
        assert!(original.straight_ish());

        assert_points_equalish(
            pt(new.anchor0_x(), new.anchor0_y()),
            pt(original.anchor0_x(), original.anchor0_y()),
        );
        assert_points_equalish(
            pt(new.anchor1_x(), new.anchor1_y()),
            pt(original.anchor1_x(), original.anchor1_y()),
        );
    }

    // The order of the features can be different, as long as they describe the same shape
    assert_eq!(original_polygon.features().len(), created_polygon.features().len());
    assert_eq!(
        original_polygon.features().iter().filter(|f| matches!(f, Feature::Corner { .. })).count(),
        created_polygon.features().iter().filter(|f| matches!(f, Feature::Corner { .. })).count(),
    );
    assert_eq!(
        original_polygon.features().iter().filter(|f| matches!(f, Feature::Edge(_))).count(),
        created_polygon.features().iter().filter(|f| matches!(f, Feature::Edge(_))).count(),
    );
    assert!(created_polygon.features().windows(2).all(|w| {
        matches!((&w[0], &w[1]), (Feature::Edge(_), Feature::Corner { .. }))
            || matches!((&w[0], &w[1]), (Feature::Corner { .. }, Feature::Edge(_)))
    }));
    assert!(
        created_polygon
            .features()
            .iter()
            .filter(|f| matches!(f, Feature::Corner { .. }))
            .all(|f| f.cubics().len() == 1 && f.cubics()[0].zero_length())
    );
}

#[test]
fn reconstructs_rounded_pill_star_close_enough() {
    // This test aims to ensure that our distance epsilon is not set too high that
    // the roundings of pill star gets pointy as they are small in the [0,1] space
    let original_polygon = pill_star_polygon(CornerRounding::new(0.2, 0.));
    let created_polygon = RoundedPolygon::from_features(
        super::super::feature::detect_features(original_polygon.cubics()),
        Some(original_polygon.center()),
    )
    .unwrap();

    assert_eq!(original_polygon.cubics().len(), created_polygon.cubics().len());
    // Allow up to one difference...
    assert_eq!(
        (original_polygon.features().len() as i64 - created_polygon.features().len() as i64).abs(),
        1
    );
    // ...as long as the edge - corner pattern persists
    assert!(created_polygon.features().windows(2).all(|w| {
        matches!((&w[0], &w[1]), (Feature::Edge(_), Feature::Corner { .. }))
            || matches!((&w[0], &w[1]), (Feature::Corner { .. }, Feature::Edge(_)))
    }));
}
