// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Port of upstream `PolygonTest.kt`.

use super::super::*;
use super::utils::*;

use alloc::vec::Vec;

fn square() -> RoundedPolygon {
    RoundedPolygon::regular(4, 1., Point::ZERO, CornerRounding::UNROUNDED, None).unwrap()
}

fn rounded_square() -> RoundedPolygon {
    RoundedPolygon::regular(4, 1., Point::ZERO, CornerRounding::new(0.2, 0.), None).unwrap()
}

fn pentagon() -> RoundedPolygon {
    RoundedPolygon::regular(5, 1., Point::ZERO, CornerRounding::UNROUNDED, None).unwrap()
}

fn nonzero_cubics(original: &[Cubic]) -> Vec<Cubic> {
    original.iter().filter(|c| !c.zero_length()).cloned().collect()
}

#[test]
fn construction_test() {
    // We can't be too specific on how exactly the square is constructed, but
    // we can at least test whether all points are within the unit square
    let square = square();
    let mut min = pt(-1., -1.);
    let mut max = pt(1., 1.);
    assert_in_bounds(square.cubics(), min, max);

    let double_square =
        RoundedPolygon::regular(4, 2., Point::ZERO, CornerRounding::UNROUNDED, None).unwrap();
    min = min * 2.;
    max = max * 2.;
    assert_in_bounds(double_square.cubics(), min, max);

    let offset_square =
        RoundedPolygon::regular(4, 1., pt(1., 2.), CornerRounding::UNROUNDED, None).unwrap();
    min = pt(0., 1.);
    max = pt(2., 3.);
    assert_in_bounds(offset_square.cubics(), min, max);

    let square_copy = square.clone();
    min = pt(-1., -1.);
    max = pt(1., 1.);
    assert_in_bounds(square_copy.cubics(), min, max);

    let p0 = pt(1., 0.);
    let p1 = pt(0., 1.);
    let p2 = pt(-1., 0.);
    let p3 = pt(0., -1.);
    let manual_square = RoundedPolygon::from_vertices(
        &[p0.x, p0.y, p1.x, p1.y, p2.x, p2.y, p3.x, p3.y],
        CornerRounding::UNROUNDED,
        None,
        None,
    )
    .unwrap();
    min = pt(-1., -1.);
    max = pt(1., 1.);
    assert_in_bounds(manual_square.cubics(), min, max);

    let offset = pt(1., 2.);
    let p0_offset = p0 + offset;
    let p1_offset = p1 + offset;
    let p2_offset = p2 + offset;
    let p3_offset = p3 + offset;
    let manual_square_offset = RoundedPolygon::from_vertices(
        &[
            p0_offset.x,
            p0_offset.y,
            p1_offset.x,
            p1_offset.y,
            p2_offset.x,
            p2_offset.y,
            p3_offset.x,
            p3_offset.y,
        ],
        CornerRounding::UNROUNDED,
        None,
        Some(offset),
    )
    .unwrap();
    min = pt(0., 1.);
    max = pt(2., 3.);
    assert_in_bounds(manual_square_offset.cubics(), min, max);
}

#[test]
fn bounds_test() {
    let square = square();
    let rounded_square = rounded_square();
    let pentagon = pentagon();

    let mut bounds = square.calculate_bounds(true);
    assert_equalish(-1., bounds[0]); // Left
    assert_equalish(-1., bounds[1]); // Top
    assert_equalish(1., bounds[2]); // Right
    assert_equalish(1., bounds[3]); // Bottom

    let mut better_bounds = square.calculate_bounds(false);
    assert_equalish(-1., better_bounds[0]); // Left
    assert_equalish(-1., better_bounds[1]); // Top
    assert_equalish(1., better_bounds[2]); // Right
    assert_equalish(1., better_bounds[3]); // Bottom

    // roundedSquare's approximate bounds will be larger due to control points
    bounds = rounded_square.calculate_bounds(true);
    better_bounds = rounded_square.calculate_bounds(false);
    assert!(
        better_bounds[2] - better_bounds[0] < bounds[2] - bounds[0],
        "bounds {}, {}, {}, {}, betterBounds = {}, {}, {}, {}",
        bounds[0],
        bounds[1],
        bounds[2],
        bounds[3],
        better_bounds[0],
        better_bounds[1],
        better_bounds[2],
        better_bounds[3],
    );

    bounds = pentagon.calculate_bounds(true);
    let max_bounds = pentagon.calculate_max_bounds();
    assert!(max_bounds[2] - max_bounds[0] > bounds[2] - bounds[0]);
}

#[test]
fn center_test() {
    assert_points_equalish(pt(0., 0.), pt(square().center_x(), square().center_y()));
}

#[test]
fn transform_test() {
    let square = square();
    // First, make sure the shape doesn't change when transformed by the identity
    let square_copy = square.transformed(&identity_transform()).unwrap();
    let n = square.cubics().len();

    assert_eq!(n, square_copy.cubics().len());
    for i in 0..n {
        assert_cubics_equalish(&square.cubics()[i], &square_copy.cubics()[i]);
    }

    // Now create a function which translates points by (1, 2) and make sure
    // the shape is translated similarly by it
    let offset = pt(1., 2.);
    let square_cubics = square.cubics();
    let translated_square_cubics =
        square.transformed(&translate_transform(offset.x, offset.y)).unwrap();

    for (i, cubic) in square_cubics.iter().enumerate() {
        let translated_cubic = &translated_square_cubics.cubics()[i];
        assert_points_equalish(
            pt(cubic.anchor0_x(), cubic.anchor0_y()) + offset,
            pt(translated_cubic.anchor0_x(), translated_cubic.anchor0_y()),
        );
        assert_points_equalish(
            pt(cubic.control0_x(), cubic.control0_y()) + offset,
            pt(translated_cubic.control0_x(), translated_cubic.control0_y()),
        );
        assert_points_equalish(
            pt(cubic.control1_x(), cubic.control1_y()) + offset,
            pt(translated_cubic.control1_x(), translated_cubic.control1_y()),
        );
        assert_points_equalish(
            pt(cubic.anchor1_x(), cubic.anchor1_y()) + offset,
            pt(translated_cubic.anchor1_x(), translated_cubic.anchor1_y()),
        );
    }
}

#[test]
fn features_test() {
    let square = square();
    let square_features = square.features();

    // Verify that cubics of polygon == nonzero cubics of features of that polygon
    // Note the Equalish test since some points may be adjusted in conversion from raw
    // cubics in the feature to the cubics list for the shape
    let flat: Vec<Cubic> =
        square_features.iter().flat_map(|f| f.cubics().iter().cloned()).collect();
    let mut nonzero = nonzero_cubics(&flat);
    assert_cubic_lists_equalish(square.cubics(), &nonzero);

    // Same as the first polygon test, but with a copy of that polygon
    let square_copy = square.clone();
    let square_copy_features = square_copy.features();
    let flat: Vec<Cubic> =
        square_copy_features.iter().flat_map(|f| f.cubics().iter().cloned()).collect();
    nonzero = nonzero_cubics(&flat);
    assert_cubic_lists_equalish(square_copy.cubics(), &nonzero);
}

#[test]
fn empty_polygon_test() {
    let poly =
        RoundedPolygon::regular(6, 0., Point::ZERO, CornerRounding::new(0.1, 0.), None).unwrap();
    assert_eq!(1, poly.cubics().len());

    let still_empty = poly.transformed(&scale_transform(10., 20.)).unwrap();
    assert_eq!(1, still_empty.cubics().len());
    assert!(still_empty.cubics()[0].zero_length());
}

#[test]
fn empty_side_test() {
    // Triangle with one point repeated
    let poly1 = RoundedPolygon::from_vertices(
        &[0., 0., 1., 0., 1., 0., 0., 1.],
        CornerRounding::UNROUNDED,
        None,
        None,
    )
    .unwrap();
    // Triangle
    let poly2 = RoundedPolygon::from_vertices(
        &[0., 0., 1., 0., 0., 1.],
        CornerRounding::UNROUNDED,
        None,
        None,
    )
    .unwrap();
    assert_cubic_lists_equalish(poly1.cubics(), poly2.cubics());
}
