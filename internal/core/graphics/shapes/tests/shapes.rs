// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Port of upstream `ShapesTest.kt` — tests the utility shape-creating
//! functions like Circle and Star.

use super::super::constructors;
use super::super::*;
use super::utils::*;
use alloc::vec::Vec;

const ZERO: Point = Point { x: 0., y: 0. };
const EPSILON: f32 = 0.01;

fn distance(start: Point, end: Point) -> f32 {
    let vector = end - start;
    (vector.x * vector.x + vector.y * vector.y).sqrt()
}

/// Test that the given point is radius distance away from `center`. If two radii are
/// provided it is sufficient to lie on either one (used for testing points on stars).
#[track_caller]
fn assert_point_on_radii(point: Point, radius1: f32, radius2: f32, center: Point) {
    let dist = distance(center, point);
    if !equalish(radius1, dist, EPSILON) {
        assert!(equalish(radius2, dist, EPSILON), "Expected {radius1} or {radius2}, actual {dist}");
    }
}

#[track_caller]
fn assert_cubic_on_radii(cubic: &Cubic, radius1: f32, radius2: f32, center: Point) {
    assert_point_on_radii(pt(cubic.anchor0_x(), cubic.anchor0_y()), radius1, radius2, center);
    assert_point_on_radii(pt(cubic.anchor1_x(), cubic.anchor1_y()), radius1, radius2, center);
}

/// Tests points along the curve of the cubic by comparing the distance from that point to the
/// center, compared to the requested radius. The test is very lenient since the Circle shape is
/// only a 4x cubic approximation of the circle and varies from the true circle.
#[track_caller]
fn assert_circular_cubic(cubic: &Cubic, radius: f32, center: Point) {
    let mut t = 0f32;
    while t <= 1. {
        let point_on_curve = cubic.point_on_curve(t);
        let distance_to_point = distance(center, point_on_curve);
        assert!(
            equalish(radius, distance_to_point, EPSILON),
            "Expected {radius}, actual {distance_to_point}"
        );
        t += 0.1;
    }
}

#[track_caller]
fn assert_circle_shape(shape: &[Cubic], radius: f32, center: Point) {
    for cubic in shape {
        assert_circular_cubic(cubic, radius, center);
    }
}

#[test]
fn circle_test() {
    assert!(constructors::circle(2, 1., ZERO).is_err());

    let circle = constructors::circle(8, 1., ZERO).unwrap();
    assert_circle_shape(circle.cubics(), 1., ZERO);

    let simple_circle = constructors::circle(3, 1., ZERO).unwrap();
    assert_circle_shape(simple_circle.cubics(), 1., ZERO);

    let complex_circle = constructors::circle(20, 1., ZERO).unwrap();
    assert_circle_shape(complex_circle.cubics(), 1., ZERO);

    let big_circle = constructors::circle(8, 3., ZERO).unwrap();
    assert_circle_shape(big_circle.cubics(), 3., ZERO);

    let center = pt(1., 2.);
    let offset_circle = constructors::circle(8, 1., center).unwrap();
    assert_circle_shape(offset_circle.cubics(), 1., center);
}

/// Stars are complicated. For the unrounded version, we can check whether the vertices are the
/// right distance from the center. For the rounded versions, just check that the shape is within
/// the appropriate bounds.
#[test]
fn star_test() {
    let mut radius = 1f32;
    let mut inner_radius = 0.5f32;
    let mut star =
        constructors::star(4, radius, inner_radius, CornerRounding::UNROUNDED, None, None, ZERO)
            .unwrap();
    for cubic in star.cubics() {
        assert_cubic_on_radii(cubic, radius, inner_radius, ZERO);
    }

    let center = pt(1., 2.);
    star =
        constructors::star(4, radius, inner_radius, CornerRounding::UNROUNDED, None, None, center)
            .unwrap();
    for cubic in star.cubics() {
        assert_cubic_on_radii(cubic, radius, inner_radius, center);
    }

    radius = 4.;
    inner_radius = 2.;
    star = constructors::star(4, radius, inner_radius, CornerRounding::UNROUNDED, None, None, ZERO)
        .unwrap();
    for cubic in star.cubics() {
        assert_cubic_on_radii(cubic, radius, inner_radius, ZERO);
    }
}

#[test]
fn rounded_star_test() {
    let rounding = CornerRounding::new(0.1, 0.);
    let inner_rounding = CornerRounding::new(0.2, 0.);
    let per_vtx_rounded: Vec<CornerRounding> = [
        rounding,
        inner_rounding,
        rounding,
        inner_rounding,
        rounding,
        inner_rounding,
        rounding,
        inner_rounding,
    ]
    .into();

    let min = pt(-1., -1.);
    let max = pt(1., 1.);

    let star = constructors::star(4, 1., 0.5, rounding, None, None, ZERO).unwrap();
    assert_in_bounds(star.cubics(), min, max);

    let star =
        constructors::star(4, 1., 0.5, CornerRounding::UNROUNDED, Some(inner_rounding), None, ZERO)
            .unwrap();
    assert_in_bounds(star.cubics(), min, max);

    let star = constructors::star(4, 1., 0.5, rounding, Some(inner_rounding), None, ZERO).unwrap();
    assert_in_bounds(star.cubics(), min, max);

    let star = constructors::star(
        4,
        1.,
        0.5,
        CornerRounding::UNROUNDED,
        None,
        Some(per_vtx_rounded.clone()),
        ZERO,
    )
    .unwrap();
    assert_in_bounds(star.cubics(), min, max);

    assert!(
        constructors::star(
            6,
            1.,
            0.5,
            CornerRounding::UNROUNDED,
            None,
            Some(per_vtx_rounded),
            ZERO,
        )
        .is_err()
    );
}
