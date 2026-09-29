// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Benchmarks for the elevation-shadow rasterizers, as the span-per-scanline
//! contract asks: `shadow_span_row` sweeps one scanline of a tessellated
//! mesh (what the software renderer composites per line), and
//! `shadow_full_mask` rasterizes the whole mask (what texture renderers
//! upload) — both on the same ~140px caster at z 8.

use criterion::{Criterion, criterion_group, criterion_main};
use i_slint_core::graphics::ElementOutline;
use i_slint_core::graphics::shadow;

fn mesh_at_z8() -> (shadow::ShadowMesh, euclid::Vector2D<f32, euclid::UnknownUnit>) {
    let outline = ElementOutline::Rectangle(i_slint_core::graphics::BorderRadius::new_uniform(12.));
    let rect = euclid::rect(0f32, 0f32, 96f32, 48f32);
    let ctm = shadow::Affine::scale_translate(1., 1., 40., 60.);
    let mut mesh = None;
    let mut offset = euclid::vec2(0., 0.);
    // Tessellate through the public path: the ambient mesh of this rounded
    // rect always exists.
    let layers =
        shadow::elevation_shadow_layers(&outline, rect, &ctm, 8., [200., 0., 600.], 800., false);
    if let Some(shadow::ElevationLayer::Mesh(m)) = layers.ambient {
        mesh = Some((*m.mesh).clone());
        offset = m.offset;
    }
    (mesh.expect("rounded-rect ambient mesh should tessellate"), offset)
}

fn shadow_span_row(c: &mut Criterion) {
    let (mesh, offset) = mesh_at_z8();
    let b: euclid::Rect<f32, euclid::UnknownUnit> = {
        // bounds = mesh bounds at draw position (same grid as the mask path)
        let mb = shadow::mesh_bounds(&mesh);
        euclid::rect(
            (mb.origin.x + offset.x).floor(),
            (mb.origin.y + offset.y).floor(),
            (mb.origin.x + mb.size.width + offset.x).ceil() - (mb.origin.x + offset.x).floor(),
            (mb.origin.y + mb.size.height + offset.y).ceil() - (mb.origin.y + offset.y).floor(),
        )
    };
    let w = b.width().ceil() as usize;
    let mut scratch = shadow::ShadowRowScratch::default();
    let mut row = vec![0u8; w];
    c.bench_function("shadow_span_row", |bencher| {
        bencher.iter(|| {
            shadow::rasterize_shadow_mesh_row(
                &mesh,
                b.cast_unit(),
                offset,
                b.origin.y as i32 + b.height() as i32 / 2,
                b.origin.x as i32,
                &mut row,
                &mut scratch,
            );
        })
    });
}

fn shadow_full_mask(c: &mut Criterion) {
    let (mesh, offset) = mesh_at_z8();
    let mb = shadow::mesh_bounds(&mesh);
    let b: euclid::Rect<f32, euclid::UnknownUnit> = euclid::rect(
        (mb.origin.x + offset.x).floor(),
        (mb.origin.y + offset.y).floor(),
        (mb.origin.x + mb.size.width + offset.x).ceil() - (mb.origin.x + offset.x).floor(),
        (mb.origin.y + mb.size.height + offset.y).ceil() - (mb.origin.y + offset.y).floor(),
    );
    c.bench_function("shadow_full_mask", |bencher| {
        bencher.iter(|| shadow::rasterize_shadow_mesh_at(&mesh, b.cast_unit(), offset))
    });
}

criterion_group!(benches, shadow_span_row, shadow_full_mask);
criterion_main!(benches);
