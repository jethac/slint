// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Port of upstream `TestUtils.kt`.

use super::super::*;
use alloc::format;

pub const EPSILON: f32 = 1e-4;

pub fn pt(x: f32, y: f32) -> Point {
    Point { x, y }
}

/// Test equality within EPSILON.
#[track_caller]
pub fn assert_points_equalish(expected: Point, actual: Point) {
    let msg = format!("{expected:?} vs. {actual:?}");
    assert_equalish_msg(expected.x, actual.x, &msg);
    assert_equalish_msg(expected.y, actual.y, &msg);
}

pub fn equalish(f0: f32, f1: f32, epsilon: f32) -> bool {
    (f0 - f1).abs() < epsilon
}

pub fn points_equalish(p0: Point, p1: Point) -> bool {
    equalish(p0.x, p1.x, EPSILON) && equalish(p0.y, p1.y, EPSILON)
}

#[track_caller]
pub fn cubics_equalish(c0: &Cubic, c1: &Cubic) -> bool {
    points_equalish(pt(c0.anchor0_x(), c0.anchor0_y()), pt(c1.anchor0_x(), c1.anchor0_y()))
        && points_equalish(pt(c0.anchor1_x(), c0.anchor1_y()), pt(c1.anchor1_x(), c1.anchor1_y()))
        && points_equalish(
            pt(c0.control0_x(), c0.control0_y()),
            pt(c1.control0_x(), c1.control0_y()),
        )
        && points_equalish(
            pt(c0.control1_x(), c0.control1_y()),
            pt(c1.control1_x(), c1.control1_y()),
        )
}

#[track_caller]
pub fn assert_cubics_equalish(expected: &Cubic, actual: &Cubic) {
    assert_points_equalish(
        pt(expected.anchor0_x(), expected.anchor0_y()),
        pt(actual.anchor0_x(), actual.anchor0_y()),
    );
    assert_points_equalish(
        pt(expected.control0_x(), expected.control0_y()),
        pt(actual.control0_x(), actual.control0_y()),
    );
    assert_points_equalish(
        pt(expected.control1_x(), expected.control1_y()),
        pt(actual.control1_x(), actual.control1_y()),
    );
    assert_points_equalish(
        pt(expected.anchor1_x(), expected.anchor1_y()),
        pt(actual.anchor1_x(), actual.anchor1_y()),
    );
}

#[track_caller]
pub fn assert_cubic_lists_equalish(expected: &[Cubic], actual: &[Cubic]) {
    assert_eq!(expected.len(), actual.len());
    for i in 0..expected.len() {
        assert_cubics_equalish(&expected[i], &actual[i]);
    }
}

#[track_caller]
pub fn assert_features_equalish(expected: &Feature, actual: &Feature) {
    assert_cubic_lists_equalish(expected.cubics(), actual.cubics());
    assert_eq!(core::mem::discriminant(expected), core::mem::discriminant(actual));

    if let (
        Feature::Corner { convex: expected_convex, .. },
        Feature::Corner { convex: actual_convex, .. },
    ) = (expected, actual)
    {
        assert_eq!(expected_convex, actual_convex);
    }
}

#[track_caller]
pub fn assert_polygons_equalish(expected: &RoundedPolygon, actual: &RoundedPolygon) {
    assert_cubic_lists_equalish(expected.cubics(), actual.cubics());

    assert_eq!(expected.features().len(), actual.features().len());
    for i in 0..expected.features().len() {
        assert_features_equalish(&expected.features()[i], &actual.features()[i]);
    }
}

#[track_caller]
pub fn assert_point_greaterish(expected: Point, actual: Point) {
    assert!(actual.x >= expected.x - EPSILON);
    assert!(actual.y >= expected.y - EPSILON);
}

#[track_caller]
pub fn assert_point_lessish(expected: Point, actual: Point) {
    assert!(actual.x <= expected.x + EPSILON);
    assert!(actual.y <= expected.y + EPSILON);
}

#[track_caller]
pub fn assert_equalish_msg(expected: f32, actual: f32, message: &str) {
    assert!(
        equalish(expected, actual, EPSILON),
        "Expected <{expected}>, actual <{actual}>. {message}"
    );
}

#[track_caller]
pub fn assert_equalish(expected: f32, actual: f32) {
    assert!(equalish(expected, actual, EPSILON), "Expected <{expected}>, actual <{actual}>.");
}

#[track_caller]
pub fn assert_in_bounds(shape: &[Cubic], min_point: Point, max_point: Point) {
    for cubic in shape {
        assert_point_greaterish(min_point, pt(cubic.anchor0_x(), cubic.anchor0_y()));
        assert_point_lessish(max_point, pt(cubic.anchor0_x(), cubic.anchor0_y()));
        assert_point_greaterish(min_point, pt(cubic.control0_x(), cubic.control0_y()));
        assert_point_lessish(max_point, pt(cubic.control0_x(), cubic.control0_y()));
        assert_point_greaterish(min_point, pt(cubic.control1_x(), cubic.control1_y()));
        assert_point_lessish(max_point, pt(cubic.control1_x(), cubic.control1_y()));
        assert_point_greaterish(min_point, pt(cubic.anchor1_x(), cubic.anchor1_y()));
        assert_point_lessish(max_point, pt(cubic.anchor1_x(), cubic.anchor1_y()));
    }
}

pub fn identity_transform() -> impl Fn(f32, f32) -> Point {
    |x, y| Point { x, y }
}

pub fn scale_transform(sx: f32, sy: f32) -> impl Fn(f32, f32) -> Point {
    move |x, y| Point { x: x * sx, y: y * sy }
}

pub fn translate_transform(dx: f32, dy: f32) -> impl Fn(f32, f32) -> Point {
    move |x, y| Point { x: x + dx, y: y + dy }
}
