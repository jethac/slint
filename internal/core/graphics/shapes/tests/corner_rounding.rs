// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Port of upstream `CornerRoundingTest.kt`.

use super::super::*;

#[test]
fn corner_rounding_test() {
    let default_corner = CornerRounding::new(0., 0.);
    assert_eq!(default_corner.radius, 0.);
    assert_eq!(default_corner.smoothing, 0.);

    let unrounded = CornerRounding::UNROUNDED;
    assert_eq!(unrounded.radius, 0.);
    assert_eq!(unrounded.smoothing, 0.);

    let rounded = CornerRounding::new(5., 0.);
    assert_eq!(rounded.radius, 5.);
    assert_eq!(rounded.smoothing, 0.);

    let smoothed = CornerRounding::new(0., 0.5);
    assert_eq!(smoothed.radius, 0.);
    assert_eq!(smoothed.smoothing, 0.5);

    let rounded_and_smoothed = CornerRounding::new(5., 0.5);
    assert_eq!(rounded_and_smoothed.radius, 5.);
    assert_eq!(rounded_and_smoothed.smoothing, 0.5);
}
