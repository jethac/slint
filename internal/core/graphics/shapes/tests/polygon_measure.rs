// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Port of upstream `PolygonMeasureTest.kt`.

use super::super::*;
use super::utils::*;

use alloc::rc::Rc;
use alloc::vec;

fn measurer() -> Rc<dyn Measurer> {
    Rc::new(LengthMeasurer::default())
}

fn regular_polygon_measure(sides: usize, rounding: CornerRounding) {
    let polygon = RoundedPolygon::regular(sides, 1., Point::ZERO, rounding, None).unwrap();
    irregular_polygon_measure(&polygon, &|measured_polygon| {
        assert_eq!(sides, measured_polygon.size());
        for index in 0..measured_polygon.size() {
            let measured_cubic = measured_polygon.get(index).unwrap();
            assert_equalish(index as f32 / sides as f32, measured_cubic.start_outline_progress);
        }
    });
}

fn custom_polygon_measure(polygon: &RoundedPolygon, progresses: &[f32]) {
    irregular_polygon_measure(polygon, &|measured_polygon| {
        assert_eq!(measured_polygon.size(), progresses.len());
        for (index, &expected) in progresses.iter().enumerate() {
            let measured_cubic = measured_polygon.get(index).unwrap();
            assert_equalish(
                expected,
                measured_cubic.end_outline_progress - measured_cubic.start_outline_progress,
            );
        }
    });
}

fn irregular_polygon_measure(polygon: &RoundedPolygon, extra_checks: &dyn Fn(&MeasuredPolygon)) {
    let measured_polygon = MeasuredPolygon::measure_polygon(measurer(), polygon).unwrap();

    assert_equalish(0., measured_polygon.get(0).unwrap().start_outline_progress);
    assert_equalish(
        1.,
        measured_polygon.get(measured_polygon.size() - 1).unwrap().end_outline_progress,
    );

    for index in 0..measured_polygon.size() {
        let measured_cubic = measured_polygon.get(index).unwrap();
        if index > 0 {
            assert_eq!(
                measured_polygon.get(index - 1).unwrap().end_outline_progress,
                measured_cubic.start_outline_progress,
            );
        }
        assert!(measured_cubic.end_outline_progress >= measured_cubic.start_outline_progress);
    }

    for (index, progressable_feature) in measured_polygon.features.iter().enumerate() {
        assert!(
            progressable_feature.progress() >= 0. && progressable_feature.progress() < 1.,
            "Feature #{index} has invalid progress: {}",
            progressable_feature.progress(),
        );
    }

    extra_checks(&measured_polygon);
}

fn no_extra_checks(_: &MeasuredPolygon) {}

#[test]
fn measure_sharp_triangle() {
    regular_polygon_measure(3, CornerRounding::UNROUNDED);
}

#[test]
fn measure_sharp_pentagon() {
    regular_polygon_measure(5, CornerRounding::UNROUNDED);
}

#[test]
fn measure_sharp_octagon() {
    regular_polygon_measure(8, CornerRounding::UNROUNDED);
}

#[test]
fn measure_sharp_dodecagon() {
    regular_polygon_measure(12, CornerRounding::UNROUNDED);
}

#[test]
fn measure_sharp_icosagon() {
    regular_polygon_measure(20, CornerRounding::UNROUNDED);
}

#[test]
fn measure_slightly_rounded_hexagon() {
    let polygon =
        RoundedPolygon::regular(6, 1., Point::ZERO, CornerRounding::new(0.15, 0.), None).unwrap();
    irregular_polygon_measure(&polygon, &no_extra_checks);
}

#[test]
fn measure_medium_rounded_hexagon() {
    let polygon =
        RoundedPolygon::regular(6, 1., Point::ZERO, CornerRounding::new(0.5, 0.), None).unwrap();
    irregular_polygon_measure(&polygon, &no_extra_checks);
}

#[test]
fn measure_maximum_rounded_hexagon() {
    let polygon =
        RoundedPolygon::regular(6, 1., Point::ZERO, CornerRounding::new(1., 0.), None).unwrap();
    irregular_polygon_measure(&polygon, &no_extra_checks);
}

#[test]
fn measure_circle() {
    // White box test: As the length measurer approximates arcs by linear segments,
    // this test validates if the chosen segment count approximates the arc length up to
    // an error of 1.5% from the true length
    let vertices = 4;
    let polygon = super::super::constructors::circle(vertices, 1., Point::ZERO).unwrap();

    let measurer = LengthMeasurer::default();
    let actual_length: f64 =
        polygon.cubics().iter().map(|c| measurer.measure_cubic(c) as f64).sum();
    let expected_length = 2. * core::f64::consts::PI;

    assert!(
        (expected_length - actual_length).abs() < 0.015 * expected_length,
        "Expected {expected_length}, actual {actual_length}"
    );
}

#[test]
fn irregular_triangle_angle_measure() {
    irregular_polygon_measure(
        &RoundedPolygon::from_vertices(
            &[0., -1., 1., 1., 0., 0.5, -1., 1.],
            CornerRounding::UNROUNDED,
            Some(&[
                CornerRounding::new(0.2, 0.5),
                CornerRounding::new(0.2, 0.5),
                CornerRounding::new(0.4, 0.),
                CornerRounding::new(0.2, 0.5),
            ]),
            None,
        )
        .unwrap(),
        &no_extra_checks,
    );
}

#[test]
fn quarter_angle_measure() {
    irregular_polygon_measure(
        &RoundedPolygon::from_vertices(
            &[-1., -1., 1., -1., 1., 1., -1., 1.],
            CornerRounding::UNROUNDED,
            Some(&[
                CornerRounding::UNROUNDED,
                CornerRounding::UNROUNDED,
                CornerRounding::new(0.5, 0.5),
                CornerRounding::UNROUNDED,
            ]),
            None,
        )
        .unwrap(),
        &no_extra_checks,
    );
}

#[test]
fn hour_glass_measure() {
    // Regression test: Legacy measurer (AngleMeasurer) would skip the diagonal sides
    // as they are 0 degrees from the center.
    let unit = 1f32;
    let coordinates = [
        // lower glass
        0., 0., unit, unit, -unit, unit, // upper glass
        0., 0., -unit, -unit, unit, -unit,
    ];

    let diagonal = (unit * unit + unit * unit).sqrt();
    let horizontal = 2. * unit;
    let total = 4. * diagonal + 2. * horizontal;

    let polygon =
        RoundedPolygon::from_vertices(&coordinates, CornerRounding::UNROUNDED, None, None).unwrap();
    custom_polygon_measure(
        &polygon,
        &[
            diagonal / total,
            horizontal / total,
            diagonal / total,
            diagonal / total,
            horizontal / total,
            diagonal / total,
        ],
    );
}

#[test]
fn handles_empty_feature_last() {
    let triangle = RoundedPolygon::from_features(
        vec![
            Feature::build_convex_corner(vec![Cubic::straight_line(0., 0., 1., 1.)]).unwrap(),
            Feature::build_convex_corner(vec![Cubic::straight_line(1., 1., 1., 0.)]).unwrap(),
            Feature::build_convex_corner(vec![Cubic::straight_line(1., 0., 0., 0.)]).unwrap(),
            // Empty feature at the end.
            Feature::build_convex_corner(vec![Cubic::straight_line(0., 0., 0., 0.)]).unwrap(),
        ],
        None,
    )
    .unwrap();

    irregular_polygon_measure(&triangle, &no_extra_checks);
}
