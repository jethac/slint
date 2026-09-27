// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Port of kotlin/hct/ViewingConditions.kt (material-color-utilities @ 5b3618b).

use crate::hct::Cam16;
use crate::math;
use crate::utils::ColorUtils;
use crate::utils::MathUtils;
use core::f64::consts::PI;

/// In traditional color spaces, a color can be identified solely by the
/// observer's measurement of the color. Color appearance models such as CAM16
/// also use information about the environment where the color was observed,
/// known as the viewing conditions.
///
/// For example, white under the traditional assumption of a midday sun white
/// point is accurately measured as a slightly chromatic blue by CAM16.
/// (roughly, hue 203, chroma 3, lightness 100)
///
/// This class caches intermediate values of the CAM16 conversion process that
/// depend only on viewing conditions, enabling speed ups.
#[derive(Clone, Debug)]
pub struct ViewingConditions {
    /// Intermediate value.
    pub n: f64,
    /// Intermediate value.
    pub aw: f64,
    /// Intermediate value.
    pub nbb: f64,
    pub(crate) ncb: f64,
    pub(crate) c: f64,
    pub(crate) nc: f64,
    /// Intermediate value.
    pub rgb_d: [f64; 3],
    pub(crate) fl: f64,
    /// Intermediate value.
    pub fl_root: f64,
    pub(crate) z: f64,
}

impl ViewingConditions {
    /// sRGB-like viewing conditions.
    ///
    /// The Kotlin implementation shares a single lazily-initialized instance;
    /// the values are pure math, so computing them on each call yields
    /// identical results.
    pub fn default() -> ViewingConditions {
        Self::default_with_background_lstar(50.0)
    }

    /// Create ViewingConditions from a simple, physically relevant, set of
    /// parameters.
    ///
    /// `white_point`: White point, measured in the XYZ color space. default =
    ///   D65, or sunny day afternoon.
    /// `adapting_luminance`: The luminance of the adapting field. Informally,
    ///   how bright it is in the room where the color is viewed. Can be
    ///   calculated from lux by multiplying lux by 0.0586. default = 11.72,
    ///   or 200 lux.
    /// `background_lstar`: The lightness of the area surrounding the color,
    ///   measured by L* in L*a*b*. default = 50.0.
    /// `surround`: A general description of the lighting surrounding the
    ///   color. 0 is pitch dark, like watching a movie in a theater. 1.0 is a
    ///   dimly lit room, like watching TV at home at night. 2.0 means there is
    ///   no difference between the lighting on the color and around it.
    ///   default = 2.0.
    /// `discounting_illuminant`: Whether the eye accounts for the tint of the
    ///   ambient lighting, such as knowing an apple is still red in green
    ///   light. default = false, the eye does not perform this process on
    ///   self-luminous objects like displays.
    pub fn make(
        white_point: [f64; 3],
        adapting_luminance: f64,
        background_lstar: f64,
        surround: f64,
        discounting_illuminant: bool,
    ) -> ViewingConditions {
        // A background of pure black is non-physical and leads to infinities
        // that represent the idea that any color viewed in pure black can't
        // be seen.
        let background_lstar = background_lstar.max(0.1);
        // Transform white point XYZ to 'cone'/'rgb' responses
        let matrix = Cam16::XYZ_TO_CAM16RGB;
        let xyz = white_point;
        let r_w = xyz[0] * matrix[0][0] + xyz[1] * matrix[0][1] + xyz[2] * matrix[0][2];
        let g_w = xyz[0] * matrix[1][0] + xyz[1] * matrix[1][1] + xyz[2] * matrix[1][2];
        let b_w = xyz[0] * matrix[2][0] + xyz[1] * matrix[2][1] + xyz[2] * matrix[2][2];
        let f = 0.8 + surround / 10.0;
        let c = if f >= 0.9 {
            MathUtils::lerp(0.59, 0.69, (f - 0.9) * 10.0)
        } else {
            MathUtils::lerp(0.525, 0.59, (f - 0.8) * 10.0)
        };
        let mut d = if discounting_illuminant {
            1.0
        } else {
            f * (1.0 - 1.0 / 3.6 * math::exp((-adapting_luminance - 42.0) / 92.0))
        };
        d = d.clamp(0.0, 1.0);
        let nc = f;
        let rgb_d =
            [d * (100.0 / r_w) + 1.0 - d, d * (100.0 / g_w) + 1.0 - d, d * (100.0 / b_w) + 1.0 - d];
        let k = 1.0 / (5.0 * adapting_luminance + 1.0);
        let k4 = k * k * k * k;
        let k4_f = 1.0 - k4;
        let fl = k4 * adapting_luminance + 0.1 * k4_f * k4_f * math::cbrt(5.0 * adapting_luminance);
        let n = ColorUtils::y_from_lstar(background_lstar) / white_point[1];
        let z = 1.48 + math::sqrt(n);
        let nbb = 0.725 / math::pow(n, 0.2);
        let ncb = nbb;
        let rgb_a_factors = [
            math::pow(fl * rgb_d[0] * r_w / 100.0, 0.42),
            math::pow(fl * rgb_d[1] * g_w / 100.0, 0.42),
            math::pow(fl * rgb_d[2] * b_w / 100.0, 0.42),
        ];
        let rgb_a = [
            400.0 * rgb_a_factors[0] / (rgb_a_factors[0] + 27.13),
            400.0 * rgb_a_factors[1] / (rgb_a_factors[1] + 27.13),
            400.0 * rgb_a_factors[2] / (rgb_a_factors[2] + 27.13),
        ];
        let aw = (2.0 * rgb_a[0] + rgb_a[1] + 0.05 * rgb_a[2]) * nbb;
        ViewingConditions { n, aw, nbb, ncb, c, nc, rgb_d, fl, fl_root: math::pow(fl, 0.25), z }
    }

    /// Create sRGB-like viewing conditions with a custom background lstar.
    ///
    /// Default viewing conditions have an lstar of 50, midgray.
    pub fn default_with_background_lstar(lstar: f64) -> ViewingConditions {
        Self::make(
            ColorUtils::white_point_d65(),
            200.0 / PI * ColorUtils::y_from_lstar(50.0) / 100.0,
            lstar,
            2.0,
            false,
        )
    }
}
