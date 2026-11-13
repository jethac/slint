// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! The `shape` type: rounded polygon outlines and their morphing,
//! built on the `rounded-polygon` crate.

mod api;
#[cfg(any(not(target_arch = "wasm32"), target_os = "emscripten"))]
pub(crate) mod ffi;
mod shape;

#[cfg(test)]
mod tests;

pub use crate::items::CornerRounding;
pub use api::{
    circle_shape, custom_shape, pill_shape, pill_star_shape, rectangle_shape, regular_polygon,
    regular_polygon_per_vertex, rounded_polygon_per_vertex, rounded_polygon_polygon, star_shape,
};
pub use rounded_polygon::CornerRounding as PolygonCornerRounding;
pub use rounded_polygon::{
    AngleMeasurer, Cubic, DoubleMapper, Feature, FeatureSerializer, LengthMeasurer, MeasuredCubic,
    MeasuredPolygon, Measurer, Morph, Point, PointTransformer, ProgressableFeature,
    RoundedPolygon, ShapeError, SvgPathParser, circle, custom_polygon, pill, pill_star, rectangle,
    star,
};
pub use shape::{MorphCache, Shape, ShapeFeature, ShapeFeatureKind, ShapePoint};


crate::thread_local! {
    /// The default morph cache used by the `Shapes.morph` builtin (and shape
    /// property animations): content-keyed, so identical endpoints share their
    /// measured feature-mapping regardless of how often they are reconstructed.
    pub static MORPH_CACHE: core::cell::RefCell<MorphCache> =
        const { core::cell::RefCell::new(MorphCache::new()) };
}

/// The interned-id counter for [`Shape`]: every `Shape::new` takes the next
/// id; `0` means "no id" (`Default`, FFI). Global, not per-thread: `Shape` is
/// `Send`, so per-thread counters would hand equal ids to different shapes on
/// different threads and let a shared [`MorphCache`] alias their morphs.
/// `portable_atomic` covers targets without 64-bit atomics. Wraps at u64::MAX
/// — unreachable.
static SHAPE_ID_COUNTER: portable_atomic::AtomicU64 = portable_atomic::AtomicU64::new(1);

/// The next interned shape id. See [`Shape::id`].
pub(crate) fn next_shape_id() -> u64 {
    SHAPE_ID_COUNTER.fetch_add(1, portable_atomic::Ordering::Relaxed)
}

/// Morph between `from` and `to` at `progress` ∈ [0, 1] using the thread-local
/// default [`MorphCache`]. Equivalent to `from.morph(to, progress)`.
pub fn morph(from: &Shape, to: &Shape, progress: f32) -> Shape {
    from.morph(to, progress)
}

