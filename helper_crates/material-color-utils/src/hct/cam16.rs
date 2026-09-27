// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Port of kotlin/hct/Cam16.kt (material-color-utilities @ 5b3618b).

use crate::Argb;
use crate::hct::ViewingConditions;
use crate::math;
use crate::utils::ColorUtils;
use crate::utils::MathUtils;

/// CAM16, a color appearance model. Colors are not just defined by their hex
/// code, but rather, a hex code and viewing conditions.
///
/// CAM16 instances also have coordinates in the CAM16-UCS space, called J*,
/// a*, b*, or jstar, astar, bstar in code. CAM16-UCS is included in the CAM16
/// specification, and should be used when measuring distances between colors.
///
/// In traditional color spaces, a color can be identified solely by the
/// observer's measurement of the color. Color appearance models such as CAM16
/// also use information about the environment where the color was observed,
/// known as the viewing conditions.
///
/// For example, white under the traditional assumption of a midday sun white
/// point is accurately measured as a slightly chromatic blue by CAM16.
/// (roughly, hue 203, chroma 3, lightness 100)
#[derive(Clone, Copy, Debug)]
pub struct Cam16 {
    /// Hue in CAM16.
    pub hue: f64,
    /// Chroma in CAM16.
    pub chroma: f64,
    /// Lightness in CAM16.
    pub j: f64,
    /// Brightness in CAM16.
    ///
    /// Prefer lightness, brightness is an absolute quantity. For example, a
    /// sheet of white paper is much brighter viewed in sunlight than in
    /// indoor light, but it is the lightest object under any lighting.
    pub q: f64,
    /// Colorfulness in CAM16.
    ///
    /// Prefer chroma, colorfulness is an absolute quantity. For example, a
    /// yellow toy car is much more colorful outside than inside, but it has
    /// the same chroma in both environments.
    pub m: f64,
    /// Saturation in CAM16.
    ///
    /// Colorfulness in proportion to brightness. Prefer chroma, saturation
    /// measures colorfulness relative to the color's own brightness, where
    /// chroma is colorfulness relative to white.
    pub s: f64,
    /// Lightness coordinate in CAM16-UCS.
    pub jstar: f64,
    /// a* coordinate in CAM16-UCS.
    pub astar: f64,
    /// b* coordinate in CAM16-UCS.
    pub bstar: f64,
}

impl Cam16 {
    /// Transforms XYZ color space coordinates to 'cone'/'RGB' responses in
    /// CAM16.
    pub(crate) const XYZ_TO_CAM16RGB: [[f64; 3]; 3] = [
        [0.401288, 0.650173, -0.051461],
        [-0.250268, 1.204414, 0.045854],
        [-0.002079, 0.048952, 0.953127],
    ];

    /// Transforms 'cone'/'RGB' responses in CAM16 to XYZ color space
    /// coordinates.
    pub(crate) const CAM16RGB_TO_XYZ: [[f64; 3]; 3] = [
        [1.8620678, -1.0112547, 0.14918678],
        [0.38752654, 0.62144744, -0.00897398],
        [-0.0158415, -0.03412294, 1.0499644],
    ];

    /// The color's distance to `other` in CAM16-UCS space.
    pub fn distance(&self, other: &Cam16) -> f64 {
        let d_j = self.jstar - other.jstar;
        let d_a = self.astar - other.astar;
        let d_b = self.bstar - other.bstar;
        let d_e_prime = math::sqrt(d_j * d_j + d_a * d_a + d_b * d_b);
        1.41 * math::pow(d_e_prime, 0.63)
    }

    /// ARGB representation of the color. Assumes the color was viewed in
    /// default viewing conditions, which are near-identical to the default
    /// viewing conditions for sRGB.
    pub fn to_int(&self) -> Argb {
        self.viewed(&ViewingConditions::default())
    }

    /// ARGB representation of the color, in defined viewing conditions.
    pub(crate) fn viewed(&self, viewing_conditions: &ViewingConditions) -> Argb {
        let xyz = self.xyz_in_viewing_conditions(viewing_conditions);
        ColorUtils::argb_from_xyz(xyz[0], xyz[1], xyz[2])
    }

    pub(crate) fn xyz_in_viewing_conditions(
        &self,
        viewing_conditions: &ViewingConditions,
    ) -> [f64; 3] {
        let alpha = if self.chroma == 0.0 || self.j == 0.0 {
            0.0
        } else {
            self.chroma / math::sqrt(self.j / 100.0)
        };
        let t = math::pow(
            alpha / math::pow(1.64 - math::pow(0.29, viewing_conditions.n), 0.73),
            1.0 / 0.9,
        );
        let h_rad = math::to_radians(self.hue);
        let e_hue = 0.25 * (math::cos(h_rad + 2.0) + 3.8);
        let ac = viewing_conditions.aw
            * math::pow(self.j / 100.0, 1.0 / viewing_conditions.c / viewing_conditions.z);
        let p1 = e_hue * (50000.0 / 13.0) * viewing_conditions.nc * viewing_conditions.ncb;
        let p2 = ac / viewing_conditions.nbb;
        let h_sin = math::sin(h_rad);
        let h_cos = math::cos(h_rad);
        let gamma = 23.0 * (p2 + 0.305) * t / (23.0 * p1 + 11.0 * t * h_cos + 108.0 * t * h_sin);
        let a = gamma * h_cos;
        let b = gamma * h_sin;
        let r_a = (460.0 * p2 + 451.0 * a + 288.0 * b) / 1403.0;
        let g_a = (460.0 * p2 - 891.0 * a - 261.0 * b) / 1403.0;
        let b_a = (460.0 * p2 - 220.0 * a - 6300.0 * b) / 1403.0;
        let r_c_base = (27.13 * math::abs(r_a) / (400.0 - math::abs(r_a))).max(0.0);
        let r_c =
            math::signum(r_a) * (100.0 / viewing_conditions.fl) * math::pow(r_c_base, 1.0 / 0.42);
        let g_c_base = (27.13 * math::abs(g_a) / (400.0 - math::abs(g_a))).max(0.0);
        let g_c =
            math::signum(g_a) * (100.0 / viewing_conditions.fl) * math::pow(g_c_base, 1.0 / 0.42);
        let b_c_base = (27.13 * math::abs(b_a) / (400.0 - math::abs(b_a))).max(0.0);
        let b_c =
            math::signum(b_a) * (100.0 / viewing_conditions.fl) * math::pow(b_c_base, 1.0 / 0.42);
        let r_f = r_c / viewing_conditions.rgb_d[0];
        let g_f = g_c / viewing_conditions.rgb_d[1];
        let b_f = b_c / viewing_conditions.rgb_d[2];
        let matrix = Self::CAM16RGB_TO_XYZ;
        let x = r_f * matrix[0][0] + g_f * matrix[0][1] + b_f * matrix[0][2];
        let y = r_f * matrix[1][0] + g_f * matrix[1][1] + b_f * matrix[1][2];
        let z = r_f * matrix[2][0] + g_f * matrix[2][1] + b_f * matrix[2][2];
        [x, y, z]
    }

    /// Create a CAM16 color from a color, assuming the color was viewed in
    /// default viewing conditions.
    pub fn from_int(argb: Argb) -> Cam16 {
        Self::from_int_in_viewing_conditions(argb, &ViewingConditions::default())
    }

    /// Create a CAM16 color from a color in defined viewing conditions.
    ///
    /// The RGB => XYZ conversion matrix elements are derived scientific
    /// constants. While the values may differ at runtime due to floating
    /// point imprecision, keeping the values the same, and accurate, across
    /// implementations takes precedence.
    pub(crate) fn from_int_in_viewing_conditions(
        argb: Argb,
        viewing_conditions: &ViewingConditions,
    ) -> Cam16 {
        // Transform ARGB int to XYZ
        let red = (argb & 0x00ff0000) >> 16;
        let green = (argb & 0x0000ff00) >> 8;
        let blue = argb & 0x000000ff;
        let red_l = ColorUtils::linearized(red);
        let green_l = ColorUtils::linearized(green);
        let blue_l = ColorUtils::linearized(blue);
        let x = 0.41233895 * red_l + 0.35762064 * green_l + 0.18051042 * blue_l;
        let y = 0.2126 * red_l + 0.7152 * green_l + 0.0722 * blue_l;
        let z = 0.01932141 * red_l + 0.11916382 * green_l + 0.95034478 * blue_l;
        Self::from_xyz_in_viewing_conditions(x, y, z, viewing_conditions)
    }

    pub(crate) fn from_xyz_in_viewing_conditions(
        x: f64,
        y: f64,
        z: f64,
        viewing_conditions: &ViewingConditions,
    ) -> Cam16 {
        // Transform XYZ to 'cone'/'rgb' responses
        let matrix = Self::XYZ_TO_CAM16RGB;
        let r_t = x * matrix[0][0] + y * matrix[0][1] + z * matrix[0][2];
        let g_t = x * matrix[1][0] + y * matrix[1][1] + z * matrix[1][2];
        let b_t = x * matrix[2][0] + y * matrix[2][1] + z * matrix[2][2];

        // Discount illuminant
        let r_d = viewing_conditions.rgb_d[0] * r_t;
        let g_d = viewing_conditions.rgb_d[1] * g_t;
        let b_d = viewing_conditions.rgb_d[2] * b_t;

        // Chromatic adaptation
        let r_af = math::pow(viewing_conditions.fl * math::abs(r_d) / 100.0, 0.42);
        let g_af = math::pow(viewing_conditions.fl * math::abs(g_d) / 100.0, 0.42);
        let b_af = math::pow(viewing_conditions.fl * math::abs(b_d) / 100.0, 0.42);
        let r_a = math::signum(r_d) * 400.0 * r_af / (r_af + 27.13);
        let g_a = math::signum(g_d) * 400.0 * g_af / (g_af + 27.13);
        let b_a = math::signum(b_d) * 400.0 * b_af / (b_af + 27.13);

        // redness-greenness
        let a = (11.0 * r_a + -12.0 * g_a + b_a) / 11.0;
        // yellowness-blueness
        let b = (r_a + g_a - 2.0 * b_a) / 9.0;

        // auxiliary components
        let u = (20.0 * r_a + 20.0 * g_a + 21.0 * b_a) / 20.0;
        let p2 = (40.0 * r_a + 20.0 * g_a + b_a) / 20.0;

        // hue
        let atan2 = math::atan2(b, a);
        let atan_degrees = math::to_degrees(atan2);
        let hue = MathUtils::sanitize_degrees_double(atan_degrees);
        let hue_radians = math::to_radians(hue);

        // achromatic response to color
        let ac = p2 * viewing_conditions.nbb;

        // CAM16 lightness and brightness
        let j = 100.0
            * math::pow(ac / viewing_conditions.aw, viewing_conditions.c * viewing_conditions.z);
        let q = 4.0 / viewing_conditions.c
            * math::sqrt(j / 100.0)
            * (viewing_conditions.aw + 4.0)
            * viewing_conditions.fl_root;

        // CAM16 chroma, colorfulness, and saturation.
        let hue_prime = if hue < 20.14 { hue + 360.0 } else { hue };
        let e_hue = 0.25 * (math::cos(math::to_radians(hue_prime) + 2.0) + 3.8);
        let p1 = 50000.0 / 13.0 * e_hue * viewing_conditions.nc * viewing_conditions.ncb;
        let t = p1 * math::hypot(a, b) / (u + 0.305);
        let alpha =
            math::pow(t, 0.9) * math::pow(1.64 - math::pow(0.29, viewing_conditions.n), 0.73);
        // CAM16 chroma, colorfulness, saturation
        let c = alpha * math::sqrt(j / 100.0);
        let m = c * viewing_conditions.fl_root;
        let s = 50.0 * math::sqrt(alpha * viewing_conditions.c / (viewing_conditions.aw + 4.0));

        // CAM16-UCS components
        let jstar = (1.0 + 100.0 * 0.007) * j / (1.0 + 0.007 * j);
        let mstar = 1.0 / 0.0228 * math::log1p(0.0228 * m);
        let astar = mstar * math::cos(hue_radians);
        let bstar = mstar * math::sin(hue_radians);
        Cam16 { hue, chroma: c, j, q, m, s, jstar, astar, bstar }
    }

    /// Create a CAM16 color from lightness, chroma, hue.
    pub fn from_jch(j: f64, c: f64, h: f64) -> Cam16 {
        Self::from_jch_in_viewing_conditions(j, c, h, &ViewingConditions::default())
    }

    /// Create a CAM16 color from lightness, chroma, hue in defined viewing
    /// conditions.
    fn from_jch_in_viewing_conditions(
        j: f64,
        c: f64,
        h: f64,
        viewing_conditions: &ViewingConditions,
    ) -> Cam16 {
        let q = 4.0 / viewing_conditions.c
            * math::sqrt(j / 100.0)
            * (viewing_conditions.aw + 4.0)
            * viewing_conditions.fl_root;
        let m = c * viewing_conditions.fl_root;
        let alpha = c / math::sqrt(j / 100.0);
        let s = 50.0 * math::sqrt(alpha * viewing_conditions.c / (viewing_conditions.aw + 4.0));
        let hue_radians = math::to_radians(h);
        let jstar = (1.0 + 100.0 * 0.007) * j / (1.0 + 0.007 * j);
        let mstar = 1.0 / 0.0228 * math::log1p(0.0228 * m);
        let astar = mstar * math::cos(hue_radians);
        let bstar = mstar * math::sin(hue_radians);
        Cam16 { hue: h, chroma: c, j, q, m, s, jstar, astar, bstar }
    }

    /// Create a CAM16 color from CAM16-UCS coordinates.
    ///
    /// `jstar` CAM16-UCS lightness. `astar` CAM16-UCS a dimension. Like a* in
    /// L*a*b*, it is a Cartesian coordinate on the Y axis. `bstar` CAM16-UCS
    /// b dimension. Like a* in L*a*b*, it is a Cartesian coordinate on the X
    /// axis.
    pub fn from_ucs(jstar: f64, astar: f64, bstar: f64) -> Cam16 {
        Self::from_ucs_in_viewing_conditions(jstar, astar, bstar, &ViewingConditions::default())
    }

    /// Create a CAM16 color from CAM16-UCS coordinates in defined viewing
    /// conditions.
    ///
    /// `jstar` CAM16-UCS lightness. `astar` CAM16-UCS a dimension. Like a* in
    /// L*a*b*, it is a Cartesian coordinate on the Y axis. `bstar` CAM16-UCS
    /// b dimension. Like a* in L*a*b*, it is a Cartesian coordinate on the X
    /// axis.
    /// `viewing_conditions` are information about the environment where the
    /// color was observed.
    pub fn from_ucs_in_viewing_conditions(
        jstar: f64,
        astar: f64,
        bstar: f64,
        viewing_conditions: &ViewingConditions,
    ) -> Cam16 {
        let m = math::hypot(astar, bstar);
        let m2 = math::expm1(m * 0.0228) / 0.0228;
        let c = m2 / viewing_conditions.fl_root;
        let mut h = math::to_degrees(math::atan2(bstar, astar));
        if h < 0.0 {
            h += 360.0;
        }
        let j = jstar / (1.0 - (jstar - 100.0) * 0.007);
        Self::from_jch_in_viewing_conditions(j, c, h, viewing_conditions)
    }
}
