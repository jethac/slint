// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Port of upstream `RoundedPolygonTest.kt`.

use super::super::constructors;
use super::super::*;
use super::utils::*;

use alloc::format;
use alloc::vec;
use alloc::vec::Vec;

const ROUNDING: CornerRounding = CornerRounding { radius: 0.1, smoothing: 0. };
const PER_VTX_ROUNDED: [CornerRounding; 4] = [ROUNDING, ROUNDING, ROUNDING, ROUNDING];

fn points_to_floats(points: &[Point]) -> Vec<f32> {
    let mut result = Vec::with_capacity(points.len() * 2);
    for point in points {
        result.push(point.x);
        result.push(point.y);
    }
    result
}

#[test]
fn num_verts_constructor_test() {
    // `RoundedPolygon(2)` throws upstream.
    assert!(RoundedPolygon::regular(2, 1., Point::ZERO, CornerRounding::UNROUNDED, None).is_err());

    let square =
        RoundedPolygon::regular(4, 1., Point::ZERO, CornerRounding::UNROUNDED, None).unwrap();
    let mut min = pt(-1., -1.);
    let mut max = pt(1., 1.);
    assert_in_bounds(square.cubics(), min, max);

    let double_square =
        RoundedPolygon::regular(4, 2., Point::ZERO, CornerRounding::UNROUNDED, None).unwrap();
    min = min * 2.;
    max = max * 2.;
    assert_in_bounds(double_square.cubics(), min, max);

    let square_rounded = RoundedPolygon::regular(4, 1., Point::ZERO, ROUNDING, None).unwrap();
    min = pt(-1., -1.);
    max = pt(1., 1.);
    assert_in_bounds(square_rounded.cubics(), min, max);

    let square_pv_rounded = RoundedPolygon::regular(
        4,
        1.,
        Point::ZERO,
        CornerRounding::UNROUNDED,
        Some(&PER_VTX_ROUNDED),
    )
    .unwrap();
    assert_in_bounds(square_pv_rounded.cubics(), min, max);
}

#[test]
fn vertices_constructor_test() {
    let p0 = pt(1., 0.);
    let p1 = pt(0., 1.);
    let p2 = pt(-1., 0.);
    let p3 = pt(0., -1.);
    let verts = [p0.x, p0.y, p1.x, p1.y, p2.x, p2.y, p3.x, p3.y];

    assert!(
        RoundedPolygon::from_vertices(
            &[p0.x, p0.y, p1.x, p1.y],
            CornerRounding::UNROUNDED,
            None,
            None
        )
        .is_err()
    );

    let manual_square =
        RoundedPolygon::from_vertices(&verts, CornerRounding::UNROUNDED, None, None).unwrap();
    let mut min = pt(-1., -1.);
    let mut max = pt(1., 1.);
    assert_in_bounds(manual_square.cubics(), min, max);

    let offset = pt(1., 2.);
    let offset_verts = [
        p0.x + offset.x,
        p0.y + offset.y,
        p1.x + offset.x,
        p1.y + offset.y,
        p2.x + offset.x,
        p2.y + offset.y,
        p3.x + offset.x,
        p3.y + offset.y,
    ];
    let manual_square_offset =
        RoundedPolygon::from_vertices(&offset_verts, CornerRounding::UNROUNDED, None, Some(offset))
            .unwrap();
    min = pt(0., 1.);
    max = pt(2., 3.);
    assert_in_bounds(manual_square_offset.cubics(), min, max);

    let manual_square_rounded =
        RoundedPolygon::from_vertices(&verts, ROUNDING, None, None).unwrap();
    min = pt(-1., -1.);
    max = pt(1., 1.);
    assert_in_bounds(manual_square_rounded.cubics(), min, max);

    let manual_square_pv_rounded = RoundedPolygon::from_vertices(
        &verts,
        CornerRounding::UNROUNDED,
        Some(&PER_VTX_ROUNDED),
        None,
    )
    .unwrap();
    assert_in_bounds(manual_square_pv_rounded.cubics(), min, max);
}

#[test]
fn features_constructor_throws_for_too_few_features() {
    assert!(RoundedPolygon::from_features(vec![], None).is_err());
    let corner = Feature::Corner { cubics: vec![Cubic::empty(0., 0.)], convex: true };
    assert!(RoundedPolygon::from_features(vec![corner], None).is_err());
}

#[test]
fn features_constructor_throws_for_non_continuous_features() {
    let cubic1 = Cubic::straight_line(0., 0., 1., 0.);
    let cubic2 = Cubic::straight_line(10., 10., 20., 20.);
    assert!(
        RoundedPolygon::from_features(
            vec![Feature::build_edge(cubic1).unwrap(), Feature::build_edge(cubic2).unwrap()],
            None,
        )
        .is_err()
    );
}

#[test]
fn features_constructor_reconstructs_square() {
    let base =
        constructors::rectangle(2., 2., CornerRounding::UNROUNDED, None, Point::ZERO).unwrap();
    let actual = RoundedPolygon::from_features(base.features().to_vec(), None).unwrap();
    assert_polygons_equalish(&base, &actual);
}

#[test]
fn features_constructor_reconstructs_rounded_square() {
    let base =
        constructors::rectangle(2., 2., CornerRounding::new(0.5, 0.2), None, Point::ZERO).unwrap();
    let actual = RoundedPolygon::from_features(base.features().to_vec(), None).unwrap();
    assert_polygons_equalish(&base, &actual);
}

#[test]
fn features_constructor_reconstructs_circles() {
    for i in 3..=20 {
        let base = constructors::circle(i, 1., Point::ZERO).unwrap();
        let actual = RoundedPolygon::from_features(base.features().to_vec(), None).unwrap();
        assert_polygons_equalish(&base, &actual);
    }
}

#[test]
fn features_constructor_reconstructs_stars() {
    for i in 3..=20 {
        let base =
            constructors::star(i, 1., 0.5, CornerRounding::UNROUNDED, None, None, Point::ZERO)
                .unwrap();
        let actual = RoundedPolygon::from_features(base.features().to_vec(), None).unwrap();
        assert_polygons_equalish(&base, &actual);
    }
}

#[test]
fn features_constructor_reconstructs_rounded_stars() {
    for i in 3..=20 {
        let base =
            constructors::star(i, 1., 0.5, CornerRounding::new(0.5, 0.2), None, None, Point::ZERO)
                .unwrap();
        let actual = RoundedPolygon::from_features(base.features().to_vec(), None).unwrap();
        assert_polygons_equalish(&base, &actual);
    }
}

#[test]
fn features_constructor_reconstructs_pill() {
    let base = constructors::pill(2., 1., 0., Point::ZERO).unwrap();
    let actual = RoundedPolygon::from_features(base.features().to_vec(), None).unwrap();
    assert_polygons_equalish(&base, &actual);
}

#[test]
fn features_constructor_reconstructs_pill_star() {
    let base = constructors::pill_star(
        2.,
        1.,
        8,
        0.5,
        CornerRounding::new(0.5, 0.2),
        None,
        None,
        0.5,
        0.,
        Point::ZERO,
    )
    .unwrap();
    let actual = RoundedPolygon::from_features(base.features().to_vec(), None).unwrap();
    assert_polygons_equalish(&base, &actual);
}

#[test]
fn compute_center_test() {
    let polygon = RoundedPolygon::from_vertices(
        &[0., 0., 1., 0., 0., 1., 1., 1.],
        CornerRounding::UNROUNDED,
        None,
        None,
    )
    .unwrap();

    assert_equalish(0.5, polygon.center_x());
    assert_equalish(0.5, polygon.center_y());
}

#[test]
fn rounding_space_usage_test() {
    let p0 = pt(0., 0.);
    let p1 = pt(1., 0.);
    let p2 = pt(0.5, 1.);
    let pv_rounding =
        [CornerRounding::new(1., 0.), CornerRounding::new(1., 1.), CornerRounding::UNROUNDED];
    let polygon = RoundedPolygon::from_vertices(
        &points_to_floats(&[p0, p1, p2]),
        CornerRounding::UNROUNDED,
        Some(&pv_rounding),
        None,
    )
    .unwrap();

    // Since there is not enough room in the p0 -> p1 side even for the roundings, we shouldn't
    // take smoothing into account, so the corners should end in the middle point.
    let lower_edge_feature =
        polygon.features().iter().find(|f| matches!(f, Feature::Edge(_))).unwrap();
    assert_eq!(1, lower_edge_feature.cubics().len());

    let lower_edge = &lower_edge_feature.cubics()[0];
    assert_equalish(0.5, lower_edge.anchor0_x());
    assert_equalish(0.0, lower_edge.anchor0_y());
    assert_equalish(0.5, lower_edge.anchor1_x());
    assert_equalish(0.0, lower_edge.anchor1_y());
}

/*
 * In the following tests, we check how much was cut for the top left (vertex 0) and bottom
 * left corner (vertex 3).
 * In particular, both vertex are competing for space in the left side.
 *
 *   Vertex 0            Vertex 1
 *      *---------------------*
 *      |                     |
 *      *---------------------*
 *   Vertex 3            Vertex 2
 */
const POINTS: usize = 20;

fn do_uneven_smooth_test(
    // Corner rounding parameter for vertex 0 (top left)
    rounding0: CornerRounding,
    expected_v0_sx: f32, // Expected total cut from vertex 0 towards vertex 1
    expected_v0_sy: f32, // Expected total cut from vertex 0 towards vertex 3
    expected_v3_sy: f32, // Expected total cut from vertex 3 towards vertex 0
    // Corner rounding parameter for vertex 3 (bottom left)
    rounding3: CornerRounding,
) {
    let p0 = pt(0., 0.);
    let p1 = pt(5., 0.);
    let p2 = pt(5., 1.);
    let p3 = pt(0., 1.);

    let pv_rounding = [rounding0, CornerRounding::UNROUNDED, CornerRounding::UNROUNDED, rounding3];
    let polygon = RoundedPolygon::from_vertices(
        &points_to_floats(&[p0, p1, p2, p3]),
        CornerRounding::UNROUNDED,
        Some(&pv_rounding),
        None,
    )
    .unwrap();
    let edges: Vec<&Feature> =
        polygon.features().iter().filter(|f| matches!(f, Feature::Edge(_))).collect();
    assert_eq!(edges.len(), 4);
    let (e01, e30) = (edges[0], edges[3]);
    let msg = format!(
        "r0 = (r={}, s={}), r3 = (r={}, s={})",
        rounding0.radius, rounding0.smoothing, rounding3.radius, rounding3.smoothing
    );
    assert_equalish_msg(expected_v0_sx, e01.cubics()[0].anchor0_x(), &msg);
    assert_equalish_msg(expected_v0_sy, e30.cubics()[0].anchor1_y(), &msg);
    assert_equalish_msg(expected_v3_sy, 1. - e30.cubics()[0].anchor0_y(), &msg);
}

#[test]
fn uneven_smoothing_test() {
    // Vertex 3 has the default 0.5 radius, 0 smoothing.
    // Vertex 0 has 0.4 radius, and smoothing varying from 0 to 1.
    for i in 0..=POINTS {
        let smooth = i as f32 / POINTS as f32;
        do_uneven_smooth_test(
            CornerRounding::new(0.4, smooth),
            0.4 * (1. + smooth),
            (0.4 * (1. + smooth)).min(0.5),
            0.5,
            CornerRounding::new(0.5, 0.),
        );
    }
}

#[test]
fn uneven_smoothing_test2() {
    // Vertex 3 has 0.2f radius and 0.2f smoothing, so it takes at most 0.4f
    // Vertex 0 has 0.4f radius and smoothing varies from 0 to 1, when it reaches 0.5 it starts
    // competing with vertex 3 for space.
    for i in 0..=POINTS {
        let smooth = i as f32 / POINTS as f32;

        let smooth_wanted_v0 = 0.4 * smooth;
        let smooth_wanted_v3 = 0.2f32;

        // There is 0.4f room for smoothing
        let factor = (0.4 / (smooth_wanted_v0 + smooth_wanted_v3)).min(1.);
        do_uneven_smooth_test(
            CornerRounding::new(0.4, smooth),
            0.4 * (1. + smooth),
            0.4 + factor * smooth_wanted_v0,
            0.2 + factor * smooth_wanted_v3,
            CornerRounding::new(0.2, 1.),
        );
    }
}

#[test]
fn uneven_smoothing_test3() {
    // Vertex 3 has 0.6f radius.
    // Vertex 0 has 0.4f radius and smoothing varies from 0 to 1. There is no room for smoothing
    // on the segment between these vertices, but vertex 0 can still have smoothing on the top
    // side.
    for i in 0..=POINTS {
        let smooth = i as f32 / POINTS as f32;

        do_uneven_smooth_test(
            CornerRounding::new(0.4, smooth),
            0.4 * (1. + smooth),
            0.4,
            0.6,
            CornerRounding::new(0.6, 0.),
        );
    }
}

#[test]
fn creating_full_size_test() {
    let radius = 400.;
    let inner_radius_factor = 0.35;
    let inner_radius = radius * inner_radius_factor;
    let rounding_factor = 0.32;

    let full_size_shape = constructors::star(
        4,
        radius,
        inner_radius,
        CornerRounding::new(radius * rounding_factor, 0.),
        Some(CornerRounding::new(radius * rounding_factor, 0.)),
        None,
        Point { x: radius, y: radius },
    )
    .unwrap()
    .transformed(&move |x: f32, y: f32| Point {
        x: (x - radius) / radius,
        y: (y - radius) / radius,
    })
    .unwrap();

    let canonical_shape = constructors::star(
        4,
        1.,
        inner_radius_factor,
        CornerRounding::new(rounding_factor, 0.),
        Some(CornerRounding::new(rounding_factor, 0.)),
        None,
        Point::ZERO,
    )
    .unwrap();

    let cubics = canonical_shape.cubics();
    let cubics1 = full_size_shape.cubics();
    assert_eq!(cubics.len(), cubics1.len());
    for (cubic, cubic1) in cubics.iter().zip(cubics1.iter()) {
        assert_equalish(cubic.anchor0_x(), cubic1.anchor0_x());
        assert_equalish(cubic.anchor0_y(), cubic1.anchor0_y());
        assert_equalish(cubic.anchor1_x(), cubic1.anchor1_x());
        assert_equalish(cubic.anchor1_y(), cubic1.anchor1_y());
        assert_equalish(cubic.control0_x(), cubic1.control0_x());
        assert_equalish(cubic.control0_y(), cubic1.control0_y());
        assert_equalish(cubic.control1_x(), cubic1.control1_x());
        assert_equalish(cubic.control1_y(), cubic1.control1_y());
    }
}
