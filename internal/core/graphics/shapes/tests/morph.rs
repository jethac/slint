// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore maxd segs

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

/// Diagnostic: the start of a retarget morph must trace the `from` outline:
/// every anchor of the matched start cut lies on the `from` shape's outline
/// (cubic cutting preserves the curve exactly).
#[test]
fn retarget_zero_progress_matches_from_outline() {
    let circle = crate::graphics::shapes::circle_shape(8);
    let tri = crate::graphics::shapes::regular_polygon(3, CornerRounding::UNROUNDED);
    let star = crate::graphics::shapes::star_shape(
        5,
        0.4,
        CornerRounding::UNROUNDED,
        CornerRounding::UNROUNDED,
    );
    let mid = circle.morph(&tri, 0.35);
    let back = mid.morph(&star, 0.);

    // Flatten `mid` into a dense polyline, then ask for each `back` anchor the
    // distance to its nearest segment.
    let segs: alloc::vec::Vec<(Point, Point)> = mid
        .cubics()
        .as_chunks::<8>()
        .0
        .iter()
        .map(|p| Cubic { points: *p })
        .collect::<alloc::vec::Vec<Cubic>>()
        .iter()
        .flat_map(|c| {
            (0..32)
                .map(|k| (c.point_on_curve(k as f32 / 32.), c.point_on_curve((k + 1) as f32 / 32.)))
        })
        .collect();
    let dist = |p: Point, (a, b): (Point, Point)| {
        let ab = b - a;
        let t = (((p - a).x * ab.x + (p - a).y * ab.y) / (ab.x * ab.x + ab.y * ab.y).max(1e-12))
            .clamp(0., 1.);
        ((p - a).x - t * ab.x).hypot((p - a).y - t * ab.y)
    };
    let mut maxd = 0f32;
    for c in back.cubics().as_chunks::<8>().0.iter().map(|p| Cubic { points: *p }) {
        let a = Point { x: c.anchor0_x(), y: c.anchor0_y() };
        maxd = maxd.max(segs.iter().map(|s| dist(a, *s)).fold(f32::MAX, f32::min));
    }
    assert!(maxd <= 1e-4, "retarget start anchor off the outline by {maxd}");
}
