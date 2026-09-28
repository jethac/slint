// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Port of upstream `MorphTest.kt`.

use super::super::*;
use super::utils::*;

fn poly1() -> RoundedPolygon {
    RoundedPolygon::regular(3, 1., Point { x: 0.5, y: 0.5 }, CornerRounding::UNROUNDED, None)
        .unwrap()
}

/// Simple test to verify that a Morph with the same start and end shape has
/// curves equivalent to those in that shape.
#[test]
fn cubics_test() {
    let poly1 = poly1();
    let morph11 = Morph::new(poly1.clone(), poly1.clone());
    let p1_cubics = poly1.cubics();
    let cubics11 = morph11.as_cubics(0.);
    assert!(!cubics11.is_empty());
    // The structure of a morph and its component shapes may not match exactly, because morph
    // calculations may optimize some of the zero-length curves out. But in general, every
    // curve in the morph *should* exist somewhere in the shape it is based on, so we
    // do an exhaustive search for such existence. Note that this assertion only works because
    // we constructed the Morph from/to the same shape. A Morph between different shapes
    // may not have the curves replicated exactly.
    for morph_cubic in &cubics11 {
        let mut matched = false;
        for p1_cubic in p1_cubics {
            if cubics_equalish(morph_cubic, p1_cubic) {
                matched = true;
                continue;
            }
        }
        assert!(matched);
    }
}
