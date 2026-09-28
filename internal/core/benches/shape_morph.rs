// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Benchmark for the content-keyed morph cache: a binding like
//! `Shapes.morph(Shapes.star(4), Shapes.circle(8), t)` re-runs the shape
//! constructors on every re-evaluation, so morphing between *fresh* endpoint
//! values is the common path. `morph_cached` measures the lookup + lerp; a hit
//! reuses the measured feature match and only the per-frame cubic lerp runs.

use criterion::{Criterion, criterion_group, criterion_main};
use i_slint_core::graphics::shapes::{self, CornerRounding, MorphCache, Point};

const ROUNDING: CornerRounding = CornerRounding::new(0.1, 0.);

fn star() -> shapes::RoundedPolygon {
    shapes::star(4, 1., 0.5, ROUNDING, None, None, Point::ZERO).unwrap()
}

fn circle() -> shapes::RoundedPolygon {
    shapes::circle(8, 1., Point::ZERO).unwrap()
}

fn morph_cached(c: &mut Criterion) {
    let cache = MorphCache::new();
    let a = shapes::Shape::from_polygon(&star());
    let b = shapes::Shape::from_polygon(&circle());
    c.bench_function("morph_cached", |bencher| {
        bencher.iter(|| {
            // Rebuild the endpoints like a re-evaluated binding does: equal
            // content, fresh construction ids.
            let a = shapes::Shape::from_polygon(&star());
            let b = shapes::Shape::from_polygon(&circle());
            let morph = cache.morph(&a, &b);
            morph.as_cubics(0.37)
        })
    });
    // Every lookup after the first lands on the same content hash.
    assert!(cache.hits() > 0);
    drop((a, b));
}

fn morph_uncached(c: &mut Criterion) {
    c.bench_function("morph_uncached", |bencher| {
        bencher.iter(|| shapes::Morph::new(star(), circle()).as_cubics(0.37))
    });
}

criterion_group!(benches, morph_cached, morph_uncached);
criterion_main!(benches);
