// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Transcendental math routed through the `libm` crate.
//!
//! `std`'s float functions forward to the platform libm, whose results differ
//! by an ulp between glibc, MSVC's CRT and Apple's libm — enough to shift a
//! pixel on a rasterized edge and break the bit-identical-golden assumption
//! of the vello_cpu screenshot tests (and no_std targets, where `f32::sin` and
//! friends don't exist at all). `libm` is a pure-Rust fdlibm port: identical
//! results on every platform and in `core`-only builds. `sqrt` and `powi` are
//! exempt — IEEE-754 requires them correctly rounded everywhere.

/// `f64::sin` via libm.
#[inline(always)]
pub fn sin(x: f64) -> f64 {
    libm::sin(x)
}

/// `f64::cos` via libm.
#[inline(always)]
pub fn cos(x: f64) -> f64 {
    libm::cos(x)
}

/// `f64::tan` via libm.
#[inline(always)]
pub fn tan(x: f64) -> f64 {
    libm::tan(x)
}

/// `f64::atan2` via libm.
#[inline(always)]
pub fn atan2(y: f64, x: f64) -> f64 {
    libm::atan2(y, x)
}

/// `f64::pow` via libm.
#[inline(always)]
pub fn pow(x: f64, y: f64) -> f64 {
    libm::pow(x, y)
}

/// `f32::sin` via libm.
#[inline(always)]
pub fn sinf(x: f32) -> f32 {
    libm::sinf(x)
}

/// `f32::cos` via libm.
#[inline(always)]
pub fn cosf(x: f32) -> f32 {
    libm::cosf(x)
}

/// `f32::tan` via libm.
#[inline(always)]
pub fn tanf(x: f32) -> f32 {
    libm::tanf(x)
}

/// `f32::asin` via libm.
#[inline(always)]
pub fn asinf(x: f32) -> f32 {
    libm::asinf(x)
}

/// `f32::acos` via libm.
#[inline(always)]
pub fn acosf(x: f32) -> f32 {
    libm::acosf(x)
}

/// `f32::atan2` via libm.
#[inline(always)]
pub fn atan2f(y: f32, x: f32) -> f32 {
    libm::atan2f(y, x)
}

/// `f32::exp` via libm.
#[inline(always)]
pub fn expf(x: f32) -> f32 {
    libm::expf(x)
}

/// `f32::cbrt` via libm.
#[inline(always)]
pub fn cbrtf(x: f32) -> f32 {
    libm::cbrtf(x)
}
