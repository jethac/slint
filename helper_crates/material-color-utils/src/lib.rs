// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT OR Apache-2.0
//
// This crate is an exact port of the Kotlin implementation of
// material-color-utilities, pinned at commit
// 5b3618b16fdc3825e21d5679bafd144662088ea1 of
// material-foundation/material-color-utilities.
//
// Every function and data structure mirrors its Kotlin counterpart so that,
// given the same inputs, the outputs are bit-exact with the reference
// implementation.

//! Algorithms and utilities powering Material Design 3's color system.
//!
//! Provides the HCT and CAM16 color spaces, tonal palettes, dynamic color
//! schemes (`DynamicScheme` + `DynamicColor` with the 2021, 2025 and 2026
//! color specs, all variants and platforms), contrast curves, image
//! quantization (Celebi / Wu / WSMeans) and seed-color scoring.

#![cfg_attr(feature = "no_std", no_std)]

extern crate alloc;

/// Math functions: platform `std` implementations everywhere, except where
/// the reference implementations' fdlibm code is observably different —
/// `atan2` is routed through `libm` (a faithful fdlibm port, matching the
/// Kotlin/TypeScript reference bit-for-bit) on every platform. In `no_std`
/// builds all functions come from `libm`.
mod math {
    macro_rules! std_or_libm {
        ($(#[$meta:meta])* $fn:ident($($a:ident : f64),*) -> $std_e:expr, $libm_e:expr) => {
            #[cfg(feature = "no_std")]
            #[inline]
            pub(crate) fn $fn($($a: f64),*) -> f64 { $libm_e }
            #[cfg(not(feature = "no_std"))]
            #[inline]
            $(#[$meta])*
            pub(crate) fn $fn($($a: f64),*) -> f64 { $std_e }
        };
    }

    std_or_libm!(pow(x: f64, y: f64) -> x.powf(y), libm::pow(x, y));
    std_or_libm!(cbrt(x: f64) -> x.cbrt(), libm::cbrt(x));
    std_or_libm!(exp(x: f64) -> x.exp(), libm::exp(x));
    std_or_libm!(ln(x: f64) -> x.ln(), libm::log(x));
    std_or_libm!(sin(x: f64) -> x.sin(), libm::sin(x));
    std_or_libm!(cos(x: f64) -> x.cos(), libm::cos(x));

    /// `atan2` is the one transcendental where glibc's correctly-rounded
    /// result differs from the fdlibm `e_atan2` used by both Kotlin (Android
    /// fdlibm) and V8 — so always use `libm` (the musl/fdlibm port) for it.
    #[inline]
    pub(crate) fn atan2(y: f64, x: f64) -> f64 {
        libm::atan2(y, x)
    }

    // sqrt, abs, floor and ceil are IEEE-754 correctly-rounded basic
    // operations: every implementation returns identical results.
    #[inline]
    pub(crate) fn sqrt(x: f64) -> f64 {
        x.sqrt()
    }

    #[inline]
    pub(crate) fn abs(x: f64) -> f64 {
        x.abs()
    }

    #[inline]
    pub(crate) fn floor(x: f64) -> f64 {
        x.floor()
    }

    #[inline]
    pub(crate) fn ceil(x: f64) -> f64 {
        x.ceil()
    }

    /// Kotlin `sign` / JS `Math.sign` semantics.
    #[inline]
    pub(crate) fn signum(x: f64) -> f64 {
        if x.is_nan() {
            f64::NAN
        } else if x > 0.0 {
            1.0
        } else if x < 0.0 {
            -1.0
        } else {
            x
        }
    }

    /// Kotlin `Double.roundToInt` / JS `Math.round`: rounds half towards
    /// positive infinity.
    #[inline]
    pub(crate) fn round_to_int(x: f64) -> i64 {
        floor(x + 0.5) as i64
    }

    /// `floor(x)` truncated to an integer.
    #[inline]
    pub(crate) fn floor_to_int(x: f64) -> i64 {
        floor(x) as i64
    }

    /// `ceil(x)` truncated to an integer.
    #[inline]
    pub(crate) fn ceil_to_int(x: f64) -> i64 {
        ceil(x) as i64
    }
}

/// A color in ARGB format (alpha in the most significant byte), stored as a
/// positive value: `0xFF000000` is opaque black.
pub type Argb = i64;

pub mod utils {
    //! Color-science and math utilities.

    mod color_utils;
    mod math_utils;
    mod string_utils;

    pub use color_utils::ColorUtils;
    pub use math_utils::MathUtils;
    pub use string_utils::StringUtils;
}

pub mod contrast {
    //! Contrast ratio utilities.

    mod contrast;
    pub use contrast::Contrast;
}

pub mod hct {
    //! HCT and CAM16 color spaces, viewing conditions and the HCT solver.

    mod cam16;
    mod hct;
    mod hct_solver;
    mod viewing_conditions;

    pub use cam16::Cam16;
    pub use hct::Hct;
    pub use hct_solver::HctSolver;
    pub use viewing_conditions::ViewingConditions;
}

pub mod palettes {
    //! Tonal and core palettes.

    mod core_palettes;
    mod tonal_palette;

    pub use core_palettes::CorePalettes;
    pub use tonal_palette::TonalPalette;
}

pub mod dislike {
    //! Dislike analyzer: detects and fixes universally disliked colors.

    mod dislike_analyzer;
    pub use dislike_analyzer::DislikeAnalyzer;
}

pub mod temperature {
    //! Color temperature: analogous and complementary color computation.

    mod temperature_cache;
    pub use temperature_cache::TemperatureCache;
}

pub mod blend {
    //! Blending (harmonization) utilities.

    mod blend;
    pub use blend::Blend;
}

pub mod dynamiccolor {
    //! Dynamic colors and dynamic schemes (ColorSpec 2021/2025/2026).

    mod color_spec;
    mod color_spec_2021;
    mod color_spec_2025;
    mod color_spec_2026;
    mod color_specs;
    mod contrast_curve;
    mod dynamic_color;
    mod dynamic_scheme;
    mod material_dynamic_colors;
    mod tone_delta_pair;
    mod variant;

    pub use color_spec::{ColorSpec, SpecVersion};
    pub use color_spec_2021::ColorSpec2021;
    pub use color_spec_2025::ColorSpec2025;
    pub use color_spec_2026::ColorSpec2026;
    pub use color_specs::ColorSpecs;
    pub use contrast_curve::ContrastCurve;
    pub use dynamic_color::DynamicColor;
    pub use dynamic_scheme::{DynamicScheme, Platform};
    pub use material_dynamic_colors::MaterialDynamicColors;
    pub use tone_delta_pair::{DeltaConstraint, ToneDeltaPair, TonePolarity};
    pub use variant::Variant;
}

pub mod scheme {
    //! Named dynamic scheme variants (TonalSpot, Expressive, ...).

    mod scheme_cmf;
    mod scheme_content;
    mod scheme_expressive;
    mod scheme_fidelity;
    mod scheme_fruit_salad;
    mod scheme_monochrome;
    mod scheme_neutral;
    mod scheme_rainbow;
    mod scheme_tonal_spot;
    mod scheme_vibrant;

    pub use scheme_cmf::SchemeCmf;
    pub use scheme_content::SchemeContent;
    pub use scheme_expressive::SchemeExpressive;
    pub use scheme_fidelity::SchemeFidelity;
    pub use scheme_fruit_salad::SchemeFruitSalad;
    pub use scheme_monochrome::SchemeMonochrome;
    pub use scheme_neutral::SchemeNeutral;
    pub use scheme_rainbow::SchemeRainbow;
    pub use scheme_tonal_spot::SchemeTonalSpot;
    pub use scheme_vibrant::SchemeVibrant;
}

pub mod quantize {
    //! Image color quantization: Wu, WSMeans (Celebi) and a map quantizer.

    mod java_random;
    mod point_provider;
    mod point_provider_lab;
    mod quantizer;
    mod quantizer_celebi;
    mod quantizer_map;
    mod quantizer_result;
    mod quantizer_wsmeans;
    mod quantizer_wu;

    pub use point_provider::PointProvider;
    pub use point_provider_lab::PointProviderLab;
    pub use quantizer::Quantizer;
    pub use quantizer_celebi::QuantizerCelebi;
    pub use quantizer_map::QuantizerMap;
    pub use quantizer_result::QuantizerResult;
    pub use quantizer_wsmeans::QuantizerWsmeans;
    pub use quantizer_wu::QuantizerWu;
}

pub mod score {
    //! Seed-color scoring for seed-from-image.

    mod score;
    pub use score::{Score, score_colors};
}
