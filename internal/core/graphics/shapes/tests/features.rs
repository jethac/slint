// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Port of upstream `FeaturesTest.kt`. Upstream's `assertThrows` maps to the
//! `Result::Err` the builders return.

use super::super::*;
use super::utils::*;
use alloc::vec;

#[test]
fn cannot_build_empty_features() {
    assert!(Feature::build_convex_corner(vec![]).is_err());
    assert!(Feature::build_concave_corner(vec![]).is_err());
    assert!(Feature::build_ignorable_feature(vec![]).is_err());
}

#[test]
fn cannot_build_non_continuous_features() {
    let cubic1 = Cubic::straight_line(0., 0., 1., 1.);
    let cubic2 = Cubic::straight_line(10., 10., 11., 11.);

    assert!(Feature::build_convex_corner(vec![cubic1, cubic2]).is_err());
    assert!(Feature::build_concave_corner(vec![cubic1, cubic2]).is_err());
    assert!(Feature::build_ignorable_feature(vec![cubic1, cubic2]).is_err());
}

#[test]
fn builds_concave_corner() {
    let cubic = Cubic::straight_line(0., 0., 1., 0.);
    let actual = Feature::build_concave_corner(vec![cubic]).unwrap();
    let expected = Feature::Corner { cubics: vec![cubic], convex: false };
    assert_features_equalish(&expected, &actual);
}

#[test]
fn builds_convex_corner() {
    let cubic = Cubic::straight_line(0., 0., 1., 0.);
    let actual = Feature::build_convex_corner(vec![cubic]).unwrap();
    let expected = Feature::Corner { cubics: vec![cubic], convex: true };
    assert_features_equalish(&expected, &actual);
}

#[test]
fn builds_edge() {
    let cubic = Cubic::straight_line(0., 0., 1., 0.);
    let actual = Feature::build_edge(cubic).unwrap();
    let expected = Feature::Edge(vec![cubic]);
    assert_features_equalish(&expected, &actual);
}

#[test]
fn builds_ignorable_as_edge() {
    let cubic = Cubic::straight_line(0., 0., 1., 0.);
    let actual = Feature::build_ignorable_feature(vec![cubic]).unwrap();
    let expected = Feature::Edge(vec![cubic]);
    assert_features_equalish(&expected, &actual);
}
