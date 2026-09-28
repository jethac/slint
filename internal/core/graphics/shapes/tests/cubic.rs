// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Port of upstream `CubicTest.kt`.

use super::super::*;
use super::utils::*;

const ZERO: Point = Point { x: 0., y: 0. };
const P0: Point = Point { x: 1., y: 0. };
const P1: Point = Point { x: 1., y: 0.5 };
const P2: Point = Point { x: 0.5, y: 1. };
const P3: Point = Point { x: 0., y: 1. };

fn cubic() -> Cubic {
    Cubic::new(P0, P1, P2, P3)
}

#[test]
fn construction_test() {
    let cubic = cubic();
    assert_eq!(P0, pt(cubic.anchor0_x(), cubic.anchor0_y()));
    assert_eq!(P1, pt(cubic.control0_x(), cubic.control0_y()));
    assert_eq!(P2, pt(cubic.control1_x(), cubic.control1_y()));
    assert_eq!(P3, pt(cubic.anchor1_x(), cubic.anchor1_y()));
}

#[test]
fn circular_arc_test() {
    let arc_cubic = Cubic::circular_arc(ZERO.x, ZERO.y, P0.x, P0.y, P3.x, P3.y).unwrap();
    assert_eq!(P0, pt(arc_cubic.anchor0_x(), arc_cubic.anchor0_y()));
    assert_eq!(P3, pt(arc_cubic.anchor1_x(), arc_cubic.anchor1_y()));
}

#[test]
fn div_test() {
    let cubic = cubic();
    let div_cubic = cubic / 1.;
    assert_cubics_equalish(&cubic, &div_cubic);
    let div_cubic = cubic / 2.;
    assert_points_equalish(P0 / 2., pt(div_cubic.anchor0_x(), div_cubic.anchor0_y()));
    assert_points_equalish(P1 / 2., pt(div_cubic.control0_x(), div_cubic.control0_y()));
    assert_points_equalish(P2 / 2., pt(div_cubic.control1_x(), div_cubic.control1_y()));
    assert_points_equalish(P3 / 2., pt(div_cubic.anchor1_x(), div_cubic.anchor1_y()));
}

#[test]
fn times_test() {
    let cubic = cubic();
    let times_cubic = cubic * 1.;
    assert_eq!(P0, pt(times_cubic.anchor0_x(), times_cubic.anchor0_y()));
    assert_eq!(P1, pt(times_cubic.control0_x(), times_cubic.control0_y()));
    assert_eq!(P2, pt(times_cubic.control1_x(), times_cubic.control1_y()));
    assert_eq!(P3, pt(times_cubic.anchor1_x(), times_cubic.anchor1_y()));
    let times_cubic = cubic * 2.;
    assert_points_equalish(P0 * 2., pt(times_cubic.anchor0_x(), times_cubic.anchor0_y()));
    assert_points_equalish(P1 * 2., pt(times_cubic.control0_x(), times_cubic.control0_y()));
    assert_points_equalish(P2 * 2., pt(times_cubic.control1_x(), times_cubic.control1_y()));
    assert_points_equalish(P3 * 2., pt(times_cubic.anchor1_x(), times_cubic.anchor1_y()));
}

#[test]
fn plus_test() {
    let cubic = cubic();
    let offset_cubic = cubic * 2.;
    let plus_cubic = cubic + offset_cubic;
    assert_points_equalish(
        P0 + pt(offset_cubic.anchor0_x(), offset_cubic.anchor0_y()),
        pt(plus_cubic.anchor0_x(), plus_cubic.anchor0_y()),
    );
    assert_points_equalish(
        P1 + pt(offset_cubic.control0_x(), offset_cubic.control0_y()),
        pt(plus_cubic.control0_x(), plus_cubic.control0_y()),
    );
    assert_points_equalish(
        P2 + pt(offset_cubic.control1_x(), offset_cubic.control1_y()),
        pt(plus_cubic.control1_x(), plus_cubic.control1_y()),
    );
    assert_points_equalish(
        P3 + pt(offset_cubic.anchor1_x(), offset_cubic.anchor1_y()),
        pt(plus_cubic.anchor1_x(), plus_cubic.anchor1_y()),
    );
}

#[test]
fn reverse_test() {
    let reverse_cubic = cubic().reverse();
    assert_eq!(P3, pt(reverse_cubic.anchor0_x(), reverse_cubic.anchor0_y()));
    assert_eq!(P2, pt(reverse_cubic.control0_x(), reverse_cubic.control0_y()));
    assert_eq!(P1, pt(reverse_cubic.control1_x(), reverse_cubic.control1_y()));
    assert_eq!(P0, pt(reverse_cubic.anchor1_x(), reverse_cubic.anchor1_y()));
}

#[track_caller]
fn assert_between(end0: Point, end1: Point, actual: Point) {
    let min_x = end0.x.min(end1.x);
    let min_y = end0.y.min(end1.y);
    let max_x = end0.x.max(end1.x);
    let max_y = end0.y.max(end1.y);
    assert!(min_x <= actual.x);
    assert!(min_y <= actual.y);
    assert!(max_x >= actual.x);
    assert!(max_y >= actual.y);
}

#[test]
fn straight_line_test() {
    let line_cubic = Cubic::straight_line(P0.x, P0.y, P3.x, P3.y);
    assert_eq!(P0, pt(line_cubic.anchor0_x(), line_cubic.anchor0_y()));
    assert_eq!(P3, pt(line_cubic.anchor1_x(), line_cubic.anchor1_y()));
    assert_between(P0, P3, pt(line_cubic.control0_x(), line_cubic.control0_y()));
    assert_between(P0, P3, pt(line_cubic.control1_x(), line_cubic.control1_y()));
}

#[test]
fn split_test() {
    let cubic = cubic();
    let (split0, split1) = cubic.split(0.5);
    assert_eq!(
        pt(cubic.anchor0_x(), cubic.anchor0_y()),
        pt(split0.anchor0_x(), split0.anchor0_y())
    );
    assert_eq!(
        pt(cubic.anchor1_x(), cubic.anchor1_y()),
        pt(split1.anchor1_x(), split1.anchor1_y())
    );
    assert_between(
        pt(cubic.anchor0_x(), cubic.anchor0_y()),
        pt(cubic.anchor1_x(), cubic.anchor1_y()),
        pt(split0.anchor1_x(), split0.anchor1_y()),
    );
    assert_between(
        pt(cubic.anchor0_x(), cubic.anchor0_y()),
        pt(cubic.anchor1_x(), cubic.anchor1_y()),
        pt(split1.anchor0_x(), split1.anchor0_y()),
    );
}

#[test]
fn point_on_curve_test() {
    let cubic = cubic();
    let halfway = cubic.point_on_curve(0.5);
    assert_between(
        pt(cubic.anchor0_x(), cubic.anchor0_y()),
        pt(cubic.anchor1_x(), cubic.anchor1_y()),
        halfway,
    );
    let straight_line_cubic = Cubic::straight_line(P0.x, P0.y, P3.x, P3.y);
    let halfway = straight_line_cubic.point_on_curve(0.5);
    let computed_halfway = Point { x: P0.x + 0.5 * (P3.x - P0.x), y: P0.y + 0.5 * (P3.y - P0.y) };
    assert_points_equalish(computed_halfway, halfway);
}

#[test]
fn transform_test() {
    let cubic = cubic();
    let transformed_cubic = cubic.transformed(&identity_transform());
    assert_cubics_equalish(&cubic, &transformed_cubic);

    let transformed_cubic = cubic.transformed(&scale_transform(3., 3.));
    assert_cubics_equalish(&(cubic * 3.), &transformed_cubic);

    let tx = 200.;
    let ty = 300.;
    let translation_vector = Point { x: tx, y: ty };
    let transformed_cubic = cubic.transformed(&translate_transform(tx, ty));
    assert_points_equalish(
        pt(cubic.anchor0_x(), cubic.anchor0_y()) + translation_vector,
        pt(transformed_cubic.anchor0_x(), transformed_cubic.anchor0_y()),
    );
    assert_points_equalish(
        pt(cubic.control0_x(), cubic.control0_y()) + translation_vector,
        pt(transformed_cubic.control0_x(), transformed_cubic.control0_y()),
    );
    assert_points_equalish(
        pt(cubic.control1_x(), cubic.control1_y()) + translation_vector,
        pt(transformed_cubic.control1_x(), transformed_cubic.control1_y()),
    );
    assert_points_equalish(
        pt(cubic.anchor1_x(), cubic.anchor1_y()) + translation_vector,
        pt(transformed_cubic.anchor1_x(), transformed_cubic.anchor1_y()),
    );
}

#[test]
fn empty_cubic_has_zero_length() {
    assert!(Cubic::empty(10., 10.).zero_length());
}
