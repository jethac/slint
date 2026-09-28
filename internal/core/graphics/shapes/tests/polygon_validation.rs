// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Port of upstream `PolygonValidationTest.kt` (`PolygonValidator.fix` is
//! `fix_polygon_orientation` here).

use super::super::rounded_polygon::fix_polygon_orientation;
use super::super::*;
use super::utils::*;

const PENTAGON_POINTS: [f32; 10] = [0.2, 0., 0.8, 0., 1., 0.6, 0.5, 1., 0., 0.6];
const REVERSE_ORIENTED_PENTAGON_POINTS: [f32; 10] = [0.2, 0., 0., 0.6, 0.5, 1., 1., 0.6, 0.8, 0.];

fn stays_unchanged(polygon: &RoundedPolygon) {
    let copy = polygon.clone();
    let fixed_polygon = fix_polygon_orientation(polygon).unwrap();

    // Kotlin returns the identical instance (`polygon === fixedPolygon`); this port
    // returns a clone, so value equality is the equivalent check.
    assert_polygons_equalish(&copy, polygon);
    assert_polygons_equalish(&copy, &fixed_polygon);
}

fn fixes(broken: &RoundedPolygon, expected: &RoundedPolygon) {
    let fixed = fix_polygon_orientation(broken).unwrap();

    assert!(*broken != fixed);
    assert_polygons_equalish(expected, &fixed);
}

#[test]
fn does_not_fix_valid_sharp_polygon() {
    stays_unchanged(&regular(5));
}

#[test]
fn does_not_fix_valid_round_polygon() {
    stays_unchanged(&regular_rounded(5));
}

#[test]
fn fixes_anti_clockwise_oriented_polygon() {
    let valid =
        RoundedPolygon::from_vertices(&PENTAGON_POINTS, CornerRounding::UNROUNDED, None, None)
            .unwrap();

    let broken = RoundedPolygon::from_vertices(
        &REVERSE_ORIENTED_PENTAGON_POINTS,
        CornerRounding::UNROUNDED,
        None,
        None,
    )
    .unwrap();

    fixes(&broken, &valid);
}

#[test]
fn fixes_anti_clockwise_oriented_rounded_polygon() {
    let rounding = CornerRounding::new(0.5, 0.);
    let valid = RoundedPolygon::from_vertices(&PENTAGON_POINTS, rounding, None, None).unwrap();

    let broken =
        RoundedPolygon::from_vertices(&REVERSE_ORIENTED_PENTAGON_POINTS, rounding, None, None)
            .unwrap();

    fixes(&broken, &valid);
}

fn regular(n: usize) -> RoundedPolygon {
    RoundedPolygon::regular(n, 1., Point::ZERO, CornerRounding::UNROUNDED, None).unwrap()
}

fn regular_rounded(n: usize) -> RoundedPolygon {
    RoundedPolygon::regular(n, 1., Point::ZERO, CornerRounding::new(0.5, 0.), None).unwrap()
}
