// Copyright © The Android Open Source Project, SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: Apache-2.0

// cSpell: ignore androidx jetpack

//! Rounded polygon shapes: construction, measuring and morphing.
//!
//! This crate is a faithful `no_std` + `alloc` port of the
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

#![no_std]

extern crate alloc;
#[cfg(feature = "std")]
extern crate std;

/// Reports a recoverable misuse on stderr, where the Kotlin code logs or throws.
macro_rules! debug_log {
    ($($t:tt)*) => {{
        #[cfg(feature = "std")]
        std::eprintln!($($t)*);
    }};
}
pub(crate) use debug_log;

mod constructors;
mod corner;
mod cubic;
mod feature;
mod mapping;
mod measure;
mod morph;
mod rounded_polygon;
mod svg;
pub mod utils;

#[cfg(test)]
mod tests;

pub use constructors::{circle, custom_polygon, pill, pill_star, rectangle, star};
pub use corner::CornerRounding;
pub use cubic::Cubic;
pub use feature::{Feature, detect_features, detect_features_preserving};
pub use mapping::{DoubleMapper, ProgressableFeature};
pub use measure::{AngleMeasurer, LengthMeasurer, MeasuredCubic, MeasuredPolygon, Measurer};
pub use morph::Morph;
pub use rounded_polygon::RoundedPolygon;
pub use svg::{FeatureSerializer, SvgPathParser};
pub use utils::{Point, PointTransformer};

/// Errors produced by shape construction and parsing, mirroring the
/// `IllegalArgumentException`s thrown by the Kotlin implementation.
#[derive(Debug, Clone, PartialEq)]
pub struct ShapeError(alloc::string::String);

impl ShapeError {
    /// Creates an error with the given `message`.
    pub fn new(message: &str) -> Self {
        Self(message.into())
    }
}

impl core::fmt::Display for ShapeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(&self.0)
    }
}

impl core::error::Error for ShapeError {}
