// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Port of upstream `SvgPathParserTest.kt`.

use super::super::svg::SvgPathParser;
use super::super::*;
use super::utils::*;

fn parsing_is_equalish(path_a: &str, path_b: &str) {
    let path_a_result = SvgPathParser::parse_cubics(path_a).unwrap();
    let path_b_result = SvgPathParser::parse_cubics(path_b).unwrap();

    assert_cubic_lists_equalish(&path_a_result, &path_b_result);
}

fn parsing_matches(path: &str, expected: &[Cubic]) {
    let actual = SvgPathParser::parse_cubics(path).unwrap();

    assert_cubic_lists_equalish(expected, &actual);
}

#[test]
fn handles_empty_input() {
    let path = "";
    let expected: [Cubic; 0] = [];
    parsing_matches(path, &expected);
}

#[test]
fn parses_both_parameter_splits_equally() {
    let comma_path = "M 10,10 L 30,10 L 20,30 z";
    let space_path = "M 10 10 L 30 10 L 20 30 z";

    parsing_is_equalish(comma_path, space_path);
}

#[test]
fn parses_whitespaces_equally() {
    let spaced = "M 10,10  L    30,10    L 20,30 z";
    let touching = "M10,10L30,10L20,30z";

    parsing_is_equalish(spaced, touching);
}

#[test]
fn parses_absolute_multiple_paths() {
    let path = "M 10,10 30,10 20,30 z M 20,20 50,10 40,30 z";
    let expected = [
        Cubic::straight_line(10., 10., 30., 10.),
        Cubic::straight_line(30., 10., 20., 30.),
        Cubic::straight_line(20., 30., 10., 10.),
        Cubic::straight_line(20., 20., 50., 10.),
        Cubic::straight_line(50., 10., 40., 30.),
        Cubic::straight_line(40., 30., 20., 20.),
    ];
    parsing_matches(path, &expected);
}

#[test]
fn parses_relative_multiple_paths() {
    let absolute = "M 10,10 30,10 20,30 z M 20,20 50,10 40,30 z";
    let relative = "m 10,10 20,0 -10,20 z m 10,10, 30,-10, -10,20 z";

    parsing_is_equalish(absolute, relative);
}

#[test]
fn parses_multiple_absolute_move_tos() {
    let path = "M 10,10 30,10 20,30 z";
    let expected = [
        Cubic::straight_line(10., 10., 30., 10.),
        Cubic::straight_line(30., 10., 20., 30.),
        Cubic::straight_line(20., 30., 10., 10.),
    ];
    parsing_matches(path, &expected);
}

#[test]
fn parses_multiple_relative_move_tos_like_absolute() {
    let absolute = "M 10,10 30,10 20,30 z";
    let relative = "m 10,10 20,0 -10,20 z";

    parsing_is_equalish(absolute, relative);
}

#[test]
fn parses_absolute_line() {
    let path = "M 10,10 L 30,10 20,30 z";
    let expected = [
        Cubic::straight_line(10., 10., 30., 10.),
        Cubic::straight_line(30., 10., 20., 30.),
        Cubic::straight_line(20., 30., 10., 10.),
    ];
    parsing_matches(path, &expected);
}

#[test]
fn parses_relative_line_like_absolute() {
    let absolute = "M 10,10 L 30,10 20,30 z";
    let relative = "m 10,10 l 20,0 l -10,20 z";

    parsing_is_equalish(absolute, relative);
}

#[test]
fn parses_negative_floating_point_parameters() {
    let path = "M-10.5555,10.5L-30.5555,10.5L-20.5555,30z";
    let expected = [
        Cubic::straight_line(-10.5555, 10.5, -30.5555, 10.5),
        Cubic::straight_line(-30.5555, 10.5, -20.5555, 30.),
        Cubic::straight_line(-20.5555, 30., -10.5555, 10.5),
    ];
    parsing_matches(path, &expected);
}

#[test]
fn parses_absolute_horizontal_like_line() {
    let line_path = "M 10,10 L 30,10 L 40,10 z";
    let horizontal_equivalent = "M 10,10 H 30 40 z";

    parsing_is_equalish(line_path, horizontal_equivalent);
}

#[test]
fn parses_relative_horizontal_like_absolute() {
    let line_path = "M 10,10 L 30,10 L 40,10 z";
    let horizontal_equivalent = "M 10,10 h 20 10 z";

    parsing_is_equalish(line_path, horizontal_equivalent);
}

#[test]
fn parses_absolute_vertical_like_line() {
    let line_path = "M 10,10 L 10,30 L 10,40 z";
    let vertical_equivalent = "M 10,10 V 30 40 z";

    parsing_is_equalish(line_path, vertical_equivalent);
}

#[test]
fn parses_relative_vertical_like_absolute() {
    let line_path = "M 10,10 L 10,30 L 10,40 z";
    let vertical_equivalent = "M 10,10 v 20 10 z";

    parsing_is_equalish(line_path, vertical_equivalent);
}

#[test]
fn parses_absolute_cubics() {
    let path = "M 0,0 C 10,10 30,20, 10,10 C 23, 23 48,40 20,20 z";
    let expected = [
        Cubic::from_floats(0., 0., 10., 10., 30., 20., 10., 10.),
        Cubic::from_floats(10., 10., 23., 23., 48., 40., 20., 20.),
        Cubic::straight_line(20., 20., 0., 0.),
    ];

    parsing_matches(path, &expected);
}

#[test]
fn parses_relative_cubics_like_absolute() {
    let absolute = "M 0,0 C 10,10 30,20, 10,10 C 23, 23 48,40 20,20 z";
    let relative_equivalent = "M 0,0 c 10,10 30,20, 10,10 c 13, 13 38,30 10,10 z";

    parsing_is_equalish(absolute, relative_equivalent);
}

#[test]
fn parses_absolute_smooth_curve_like_cubic() {
    let smooth = "M 0,0 C 0,10 10,10 10,0 S 20,-10 20,0 z";
    let cubic_equivalent = "M 0,0 C 0,10 10,10 10,0 C 10,-10 20,-10 20,0 z";

    parsing_is_equalish(smooth, cubic_equivalent);
}

#[test]
fn parses_relative_smooth_curve_like_absolute() {
    let absolute = "M 0,0 C 0,10 10,10 10,0 S 20,-10 20,0 z";
    let relative_equivalent = "M 0,0 C 0,10 10,10 10,0 s 10,-10 10,0 z";

    parsing_is_equalish(absolute, relative_equivalent);
}

#[test]
fn parses_smooth_curve_with_current_position_if_no_predecessor() {
    let path = "M 10,10 S 20,-10, 20,0";
    let expected = [Cubic::from_floats(10., 10., 10., 10., 20., -10., 20., 0.)];
    parsing_matches(path, &expected);
}

#[test]
fn parses_absolute_quadratic_curve_like_cubic() {
    let q_path = "M 0,0 Q 5,10 10,0 z";
    let curve_equivalent = "M 0,0 C 5,10 5,10 10,0 z";

    parsing_is_equalish(q_path, curve_equivalent);
}

#[test]
fn parses_relative_quadratic_curve_like_absolute() {
    let absolute = "M 10,10 Q 15,20 20,10 z";
    let relative = "M 10,10 q 5, 10 10,0 z";

    parsing_is_equalish(absolute, relative);
}

#[test]
fn parses_absolute_smooth_quadratic_like_cubic() {
    let t_path = "M 0,0 Q 5,10 10,0 T 20,0 z";
    let curve_equivalent = "M 0,0 Q 5,10 10,0 C 15,-10 15,-10 20,0 z";

    parsing_is_equalish(t_path, curve_equivalent);
}

#[test]
fn parses_relative_smooth_quadratic_like_absolute() {
    let absolute = "M 0,0 Q 5,10 10,0 T 20,0 z";
    let relative_equivalent = "M 0,0 Q 5,10 10,0 t 10,0 z";

    parsing_is_equalish(absolute, relative_equivalent);
}

#[test]
fn parses_smooth_quadratic_with_current_position_if_no_predecessor() {
    let path = "M 10,10 T 20,0";
    let expected = [Cubic::from_floats(10., 10., 10., 10., 10., 10., 20., 0.)];
    parsing_matches(path, &expected);
}

#[test]
fn parses_absolute_arc() {
    // A 1/4 segment of a pie chart
    let path = "M300,200 v-150 A150,150 0 0,0 150,200 z";
    let expected = [
        Cubic::straight_line(300., 200., 300., 50.),
        Cubic::from_floats(300., 50., 273.67145, 50., 247.80118, 56.93192, 225., 70.09619),
        Cubic::from_floats(
            225., 70.09619, 202.19882, 83.26046, 183.26047, 102.198814, 170.09619, 125.,
        ),
        Cubic::from_floats(170.09619, 125., 156.93192, 147.80118, 150., 173.67145, 150., 200.),
        Cubic::straight_line(150., 200., 300., 200.),
    ];

    parsing_matches(path, &expected);
}

#[test]
fn parses_relative_arc_like_absolute() {
    // A 1/4 segment of a pie chart
    let absolute = "M300,200 v-150 A150,150 0 0,0 150,200 z";
    let relative = "M300,200 v-150 a150,150 0 0,0 -150,150 z";

    parsing_is_equalish(absolute, relative);
}

#[test]
fn parses_material_three_favorite() {
    // https://fonts.google.com/icons?selected=Material+Symbols+Outlined:favorite:FILL
    let path = "m 480 -120
l -58 -52
q -101 -91 -167 -157
T 150 -447.5
Q 111 -500 95.5 -544
T 80 -634 q 0 -94 63 -157
t 157 -63
q 52 0 99 22
t 81 62
q 34 -40 81 -62
t 99 -22
q 94 0 157 63
t 63 157
q 0 46 -15.5 90
T 810 -447.5
Q 771 -395 705 -329
T 538 -172
l -58 52
Z";
    let result = SvgPathParser::parse_cubics(path).unwrap();

    assert_eq!(result.len(), 19);
}

#[test]
fn parses_material_three_eco() {
    // https://fonts.google.com/icons?selected=Material+Symbols+Outlined:eco:FILL
    let path = "M 450 -80
q -33 0 -66.5 -7.5
T 315 -109
q 12 -121 70 -226
t 149 -185
q -110 56 -190.5 148
T 231 -162
q -4 -3 -7.5 -6.5
L 216 -176
q -47 -47 -71.5 -105
T 120 -402
q 0 -68 27 -130
t 75 -110
q 81 -81 210 -105.5
t 362 -4.5
q 18 239 -6 364.5
T 684 -182
q -49 49 -109.5 75.5
T 450 -80
Z";

    let result = SvgPathParser::parse_cubics(path).unwrap();

    assert_eq!(result.len(), 19);
}
