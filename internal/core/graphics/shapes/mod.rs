// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore androidx jetpack

//! Rounded polygon shapes: construction, measuring and morphing.
//!
//! This module is a faithful `no_std` + `alloc` port of the
//! `androidx.graphics.shapes` Kotlin library, pinned to androidx commit
//! `23327507f7fc7d5b19d65fec4b090f60c970079b`
//! (`graphics/graphics-shapes/src/commonMain/kotlin/androidx/graphics/shapes/`),
//! plus the `customPolygon`/`doRepeat` helpers from Compose Material3's
//! `MaterialShapes.kt` at the same pin.
//!
//! Numeric parity notes:
//! - Pure arithmetic is evaluated in `f32` in the same order as the Kotlin code.
//! - `kotlin.math` functions on `Float` arguments widen to `Double` on the JVM;
//!   the `k_*` helpers in [`utils`] reproduce that by computing in `f64` and
//!   narrowing back to `f32`. `f64::sqrt` is correctly rounded and therefore
//!   bit-identical to `Math.sqrt`; `sin`/`cos`/`tan`/`atan2` may differ by one
//!   `f64` ulp from `StrictMath` (which the Rust `libm` feature resolves to the
//!   same fdlibm family), so parity tests apply a documented 1e-4 tolerance only
//!   to trig-derived values.
//! - `kotlin.math.min`/`max` propagate NaN like `Math.min`/`max`; use
//!   [`utils::k_min`]/[`utils::k_max`] (not `f32::min`/`max`) for parity.
//! - Kotlin's `Float.MIN_VALUE` is the smallest positive (subnormal) float —
//!   `f32::from_bits(1)` in Rust — and `Float.NaN`/`Float.MAX_VALUE` semantics
//!   are mirrored at the corresponding call sites.

mod api;
mod constructors;
mod corner;
mod cubic;
mod feature;
#[cfg(any(not(target_arch = "wasm32"), target_os = "emscripten"))]
pub(crate) mod ffi;
mod mapping;
mod measure;
mod morph;
mod rounded_polygon;
mod shape;
mod svg;
mod utils;

#[cfg(test)]
mod tests;

pub use api::{
    circle_shape, custom_shape, pill_shape, pill_star_shape, rectangle_shape, regular_polygon,
    regular_polygon_per_vertex, rounded_polygon_per_vertex, rounded_polygon_polygon, star_shape,
};
pub use constructors::{circle, custom_polygon, pill, pill_star, rectangle, star};
pub use corner::CornerRounding;
pub use cubic::Cubic;
pub use feature::Feature;
pub use mapping::{DoubleMapper, ProgressableFeature};
pub use measure::{LengthMeasurer, MeasuredCubic, MeasuredPolygon, Measurer};
pub use morph::Morph;
pub use rounded_polygon::RoundedPolygon;
pub use shape::{MorphCache, Shape, ShapeFeature, ShapeFeatureKind, ShapePoint};
pub use svg::{FeatureSerializer, SvgPathParser};
pub use utils::{Point, PointTransformer};

crate::thread_local! {
    /// The default morph cache used by the `Shapes.morph` builtin (and shape
    /// property animations): content-keyed, so identical endpoints share their
    /// measured feature-mapping regardless of how often they are reconstructed.
    pub static MORPH_CACHE: core::cell::RefCell<MorphCache> =
        const { core::cell::RefCell::new(MorphCache::new()) };

    /// The interned-id counter for [`Shape`]: every `Shape::new` takes the next
    /// id; `0` means "no id" (`Default`, FFI). Wraps at u64::MAX — unreachable.
    static SHAPE_ID_COUNTER: core::cell::Cell<u64> = const { core::cell::Cell::new(1) };
}

/// The next interned shape id. See [`Shape::id`].
pub(crate) fn next_shape_id() -> u64 {
    SHAPE_ID_COUNTER.with(|c| {
        let id = c.get();
        c.set(id.wrapping_add(1));
        id
    })
}

/// Morph between `from` and `to` at `progress` ∈ [0, 1] using the thread-local
/// default [`MorphCache`]. Equivalent to `from.morph(to, progress)`.
pub fn morph(from: &Shape, to: &Shape, progress: f32) -> Shape {
    from.morph(to, progress)
}

/// Errors produced by shape construction and parsing, mirroring the
/// `IllegalArgumentException`s thrown by the Kotlin implementation.
#[derive(Debug, Clone, PartialEq)]
pub struct ShapeError(alloc::string::String);

impl ShapeError {
    pub(crate) fn new(message: &str) -> Self {
        Self(message.into())
    }
}

impl core::fmt::Display for ShapeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(feature = "std")]
impl std::error::Error for ShapeError {}
