// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Benchmarks for the content-keyed morph cache, decomposed as R5 asks:
//! - `morph_lookup_hit`: a pure cache hit on the interned-id fast path —
//!   same endpoint values every iteration.
//! - `morph_lookup_verify`: a hit through the content-hash verify path —
//!   freshly constructed equal-content endpoints from a pool (what a
//!   re-evaluated binding produces), without measuring polygon construction.
//! - `morph_match`: `Morph::new` alone — the measure + feature-map cost a miss
//!   pays.
//! - `morph_lerp`: `as_cubics` on a prebuilt morph — the per-frame lerp cost a
//!   hit still pays.

use criterion::{Criterion, criterion_group, criterion_main};
use i_slint_core::graphics::shapes::{self, CornerRounding, MorphCache, Point};

const ROUNDING: CornerRounding = CornerRounding::new(0.1, 0.);

fn star() -> shapes::RoundedPolygon {
    shapes::star(4, 1., 0.5, ROUNDING, None, None, Point::ZERO).unwrap()
}

fn circle() -> shapes::RoundedPolygon {
    shapes::circle(8, 1., Point::ZERO).unwrap()
}

fn morph_lookup_hit(c: &mut Criterion) {
    let cache = MorphCache::new();
    let a = shapes::Shape::from_polygon(&star());
    let b = shapes::Shape::from_polygon(&circle());
    let mut iters = 0usize;
    c.bench_function("morph_lookup_hit", |bencher| {
        bencher.iter(|| {
            iters += 1;
            std::hint::black_box(cache.morph(std::hint::black_box(&a), std::hint::black_box(&b)))
        })
    });
    // Exactly one miss — every later lookup hit the id fast path.
    assert_eq!(cache.hits(), iters - 1);
}

fn morph_lookup_verify(c: &mut Criterion) {
    let cache = MorphCache::new();
    // Fresh `Shape`s with equal content but distinct interned ids: the lookup
    // pays the hash compare + full content verify, like a re-evaluated binding.
    // The pool cycles back every 8 iterations, so ~1 in 8 lookups matches the
    // stored entry's interned ids and takes the id fast path instead.
    let stars: Vec<shapes::Shape> = (0..8).map(|_| shapes::Shape::from_polygon(&star())).collect();
    let circles: Vec<shapes::Shape> =
        (0..8).map(|_| shapes::Shape::from_polygon(&circle())).collect();
    let mut iters = 0usize;
    c.bench_function("morph_lookup_verify", |bencher| {
        bencher.iter(|| {
            let i = iters % stars.len();
            iters += 1;
            std::hint::black_box(
                cache.morph(std::hint::black_box(&stars[i]), std::hint::black_box(&circles[i])),
            )
        })
    });
    assert_eq!(cache.hits(), iters - 1);
}

fn morph_match(c: &mut Criterion) {
    c.bench_function("morph_match", |bencher| {
        bencher.iter(|| {
            shapes::Morph::new(std::hint::black_box(star()), std::hint::black_box(circle()))
        })
    });
}

fn morph_lerp(c: &mut Criterion) {
    let morph = shapes::Morph::new(star(), circle());
    c.bench_function("morph_lerp", |bencher| {
        bencher.iter(|| morph.as_cubics(std::hint::black_box(0.37)))
    });
}

criterion_group!(benches, morph_lookup_hit, morph_lookup_verify, morph_match, morph_lerp);
criterion_main!(benches);
