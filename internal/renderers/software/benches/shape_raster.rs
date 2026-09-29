// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use criterion::{Criterion, criterion_group, criterion_main};
use i_slint_core::items::{FillRule, LineCap, LineJoin};
use i_slint_renderer_software::shape_raster::*;

/// A rounded-rect-like closed contour with a slanted edge — the kind of
/// polyline the flattener emits for a fitted shape.
fn sample_contours() -> Vec<Contour> {
    let mut c = Vec::new();
    let n = 40;
    for i in 0..n {
        let a = i as f32 / n as f32 * core::f32::consts::TAU;
        c.push(Point::new(160. + a.cos() * 120., 120. + a.sin() * 90.));
    }
    vec![c]
}

fn rasterize_fill(c: &mut Criterion) {
    let contours = sample_contours();
    c.bench_function("rasterize_fill_320x240", |b| {
        b.iter(|| {
            let mut r = Rasterizer::default();
            r.begin(std::hint::black_box(&contours));
            let mut row = [0u8; 320];
            for y in 0..240 {
                r.rasterize_row(y, 0, &mut row, FillRule::Nonzero);
            }
            std::hint::black_box(row);
        })
    });
}

fn stroke_outline(c: &mut Criterion) {
    let contours = sample_contours();
    c.bench_function("stroke_to_fill_40pt", |b| {
        b.iter(|| {
            std::hint::black_box(stroke_to_fill(
                std::hint::black_box(&contours),
                4.,
                LineCap::Butt,
                LineJoin::Round,
                4.,
            ))
        })
    });
}

fn spread_mask(c: &mut Criterion) {
    let contours = sample_contours();
    c.bench_function("spread_mask_320x240", |b| {
        b.iter(|| {
            std::hint::black_box(rasterize_spread_mask(
                std::hint::black_box(&contours),
                6.,
                euclid::point2(0, 0),
                euclid::size2(320, 240),
                FillRule::Nonzero,
            ))
        })
    });
}

criterion_group!(benches, rasterize_fill, stroke_outline, spread_mask);
criterion_main!(benches);
