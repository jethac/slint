// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! androidx.compose.ui.graphics colorspace math, ported at the androidx pin
//! (commit 23327507f7fc7d5b19d65fec4b090f60c970079b). Compose animates `Color`
//! in Oklab via `Color.convert(ColorSpaces.Oklab)`: sRGB is first adapted to
//! the D50 profile connection space (`Rgb.adapt`, Bradford), then converted by
//! `Oklab.fromXyz`, whose `M1` already carries the D50→D65 adaptation. All
//! matrices are 3x3 column-major and every multiply happens in the same order
//! — and the same f32 width — as `mul3x3Float3`/`mul3x3`/`inverse3x3` in
//! `ColorSpace.kt`.

#[cfg(not(feature = "std"))]
#[allow(unused_imports)]
use num_traits::Float;

const fn mul3x3f3(m: &[f32; 9], v: [f32; 3]) -> [f32; 3] {
    [
        m[0] * v[0] + m[3] * v[1] + m[6] * v[2],
        m[1] * v[0] + m[4] * v[1] + m[7] * v[2],
        m[2] * v[0] + m[5] * v[1] + m[8] * v[2],
    ]
}

const fn mul3x3(lhs: &[f32; 9], rhs: &[f32; 9]) -> [f32; 9] {
    [
        lhs[0] * rhs[0] + lhs[3] * rhs[1] + lhs[6] * rhs[2],
        lhs[1] * rhs[0] + lhs[4] * rhs[1] + lhs[7] * rhs[2],
        lhs[2] * rhs[0] + lhs[5] * rhs[1] + lhs[8] * rhs[2],
        lhs[0] * rhs[3] + lhs[3] * rhs[4] + lhs[6] * rhs[5],
        lhs[1] * rhs[3] + lhs[4] * rhs[4] + lhs[7] * rhs[5],
        lhs[2] * rhs[3] + lhs[5] * rhs[4] + lhs[8] * rhs[5],
        lhs[0] * rhs[6] + lhs[3] * rhs[7] + lhs[6] * rhs[8],
        lhs[1] * rhs[6] + lhs[4] * rhs[7] + lhs[7] * rhs[8],
        lhs[2] * rhs[6] + lhs[5] * rhs[7] + lhs[8] * rhs[8],
    ]
}

const fn inverse3x3(m: &[f32; 9]) -> [f32; 9] {
    let (a, b, c, d, e, f, g, h, i) = (m[0], m[3], m[6], m[1], m[4], m[7], m[2], m[5], m[8]);
    let xa = e * i - f * h;
    let xb = f * g - d * i;
    let xc = d * h - e * g;
    let det = a * xa + b * xb + c * xc;
    [
        xa / det,
        xb / det,
        xc / det,
        (c * h - b * i) / det,
        (a * i - c * g) / det,
        (b * g - a * h) / det,
        (b * f - c * e) / det,
        (c * d - a * f) / det,
        (a * e - b * d) / det,
    ]
}

const fn mul3x3_diag(lhs: [f32; 3], rhs: &[f32; 9]) -> [f32; 9] {
    [
        lhs[0] * rhs[0],
        lhs[1] * rhs[1],
        lhs[2] * rhs[2],
        lhs[0] * rhs[3],
        lhs[1] * rhs[4],
        lhs[2] * rhs[5],
        lhs[0] * rhs[6],
        lhs[1] * rhs[7],
        lhs[2] * rhs[8],
    ]
}

/// `ColorSpace.kt`'s `chromaticAdaptation(Adaptation.Bradford, src, dst)`.
const fn chromatic_adaptation(src_xyz: [f32; 3], dst_xyz: [f32; 3]) -> [f32; 9] {
    const BRADFORD: [f32; 9] = [
        0.8951, -0.7502, 0.0389, //
        0.2664, 1.7135, -0.0685, //
        -0.1614, 0.0367, 1.0296,
    ];
    let src_lms = mul3x3f3(&BRADFORD, src_xyz);
    let dst_lms = mul3x3f3(&BRADFORD, dst_xyz);
    let lms = [dst_lms[0] / src_lms[0], dst_lms[1] / src_lms[1], dst_lms[2] / src_lms[2]];
    mul3x3(&inverse3x3(&BRADFORD), &mul3x3_diag(lms, &BRADFORD))
}

/// `WhitePoint.toXyz()`: xyY (Y = 1) to XYZ.
const fn white_point_to_xyz(x: f32, y: f32) -> [f32; 3] {
    [x / y, 1.0, (1.0 - x - y) / y]
}

/// `Illuminant.D50` and `Illuminant.D65` as XYZ (from xy, like `toXyz()`).
const D50_XYZ: [f32; 3] = white_point_to_xyz(0.34567, 0.35850);
const D65_XYZ: [f32; 3] = white_point_to_xyz(0.31271, 0.32902);

/// `Rgb.kt`'s `computeXYZMatrix` for the sRGB primaries in `ColorSpaces.kt`
/// (`SrgbPrimaries`) under `Illuminant.D65`.
const fn srgb_to_xyz_d65() -> [f32; 9] {
    const PRIMARIES: [f32; 6] = [0.640, 0.330, 0.300, 0.600, 0.150, 0.060];
    let rx = PRIMARIES[0];
    let ry = PRIMARIES[1];
    let gx = PRIMARIES[2];
    let gy = PRIMARIES[3];
    let bx = PRIMARIES[4];
    let by = PRIMARIES[5];
    let wx = 0.31271f32;
    let wy = 0.32902f32;

    let one_rx_ry = (1.0 - rx) / ry;
    let one_gx_gy = (1.0 - gx) / gy;
    let one_bx_by = (1.0 - bx) / by;
    let one_wx_wy = (1.0 - wx) / wy;

    let rx_ry = rx / ry;
    let gx_gy = gx / gy;
    let bx_by = bx / by;
    let wx_wy = wx / wy;

    let by_numerator =
        (one_wx_wy - one_rx_ry) * (gx_gy - rx_ry) - (wx_wy - rx_ry) * (one_gx_gy - one_rx_ry);
    let by_denominator =
        (one_bx_by - one_rx_ry) * (gx_gy - rx_ry) - (bx_by - rx_ry) * (one_gx_gy - one_rx_ry);
    let by_lum = by_numerator / by_denominator;
    let gy_lum = (wx_wy - rx_ry - by_lum * (bx_by - rx_ry)) / (gx_gy - rx_ry);
    let ry_lum = 1.0 - gy_lum - by_lum;

    let r_ry = ry_lum / ry;
    let g_gy = gy_lum / gy;
    let b_by = by_lum / by;

    [
        r_ry * rx,
        ry_lum,
        r_ry * (1.0 - rx - ry),
        g_gy * gx,
        gy_lum,
        g_gy * (1.0 - gx - gy),
        b_by * bx,
        by_lum,
        b_by * (1.0 - bx - by),
    ]
}

/// `Srgb.adapt(Illuminant.D50).transform`: linear sRGB to XYZ D50.
const SRGB_TO_XYZ_D50: [f32; 9] =
    mul3x3(&chromatic_adaptation(D65_XYZ, D50_XYZ), &srgb_to_xyz_d65());
/// `Srgb.adapt(Illuminant.D50).inverseTransform`.
const SRGB_FROM_XYZ_D50: [f32; 9] = inverse3x3(&SRGB_TO_XYZ_D50);

/// `Oklab.kt`'s `M1`: raw Oklab M1 times Bradford D50→D65.
#[allow(clippy::excessive_precision)]
const OKLAB_M1: [f32; 9] = mul3x3(
    &[
        0.8189330101,
        0.0329845436,
        0.0482003018, //
        0.3618667424,
        0.9293118715,
        0.2643662691, //
        -0.1288597137,
        0.0361456387,
        0.6338517070,
    ],
    &chromatic_adaptation(D50_XYZ, D65_XYZ),
);
const OKLAB_INVERSE_M1: [f32; 9] = inverse3x3(&OKLAB_M1);
/// `Oklab.kt`'s `M2` and `InverseM2`.
#[allow(clippy::excessive_precision)]
const OKLAB_M2: [f32; 9] = [
    0.2104542553,
    1.9779984951,
    0.0259040371, //
    0.7936177850,
    -2.4285922050,
    0.7827717662, //
    -0.0040720468,
    0.4505937099,
    -0.8086757660,
];
const OKLAB_INVERSE_M2: [f32; 9] = inverse3x3(&OKLAB_M2);

/// Compose's eotf/oetf functions run on `Double` (`Rgb.eotfFunc`).
fn srgb_eotf(c: f32) -> f32 {
    let c = c as f64;
    (if c <= 0.04045 {
        c / 12.92
    } else {
        crate::graphics::math::pow((c + 0.055) / 1.055, 2.4)
    }) as f32
}

fn srgb_oetf(c: f32) -> f32 {
    let c = c as f64;
    (if c <= 0.0031308 {
        c * 12.92
    } else {
        1.055 * crate::graphics::math::pow(c, 1.0 / 2.4) - 0.055
    }) as f32
}

/// `Connector.transform` sRGB→Oklab, returning `(l, a, b)`.
pub(super) fn srgb_to_oklab(r: f32, g: f32, b: f32) -> [f32; 3] {
    let v = mul3x3f3(&SRGB_TO_XYZ_D50, [srgb_eotf(r), srgb_eotf(g), srgb_eotf(b)]);
    let v = mul3x3f3(&OKLAB_M1, v);
    let v = [
        crate::graphics::math::cbrtf(v[0]),
        crate::graphics::math::cbrtf(v[1]),
        crate::graphics::math::cbrtf(v[2]),
    ];
    mul3x3f3(&OKLAB_M2, v)
}

/// `Connector.transform` Oklab→sRGB for `(l, a, b)`. The channel clamps
/// live inside `Oklab.toXyz`/`toXy` in Compose.
pub(super) fn oklab_to_srgb(l: f32, a: f32, b: f32) -> [f32; 3] {
    let l = l.clamp(0.0, 1.0);
    let a = a.clamp(-0.5, 0.5);
    let b = b.clamp(-0.5, 0.5);
    let v = mul3x3f3(&OKLAB_INVERSE_M2, [l, a, b]);
    let v = [v[0] * v[0] * v[0], v[1] * v[1] * v[1], v[2] * v[2] * v[2]];
    let v = mul3x3f3(&OKLAB_INVERSE_M1, v);
    let v = mul3x3f3(&SRGB_FROM_XYZ_D50, v);
    [srgb_oetf(v[0]), srgb_oetf(v[1]), srgb_oetf(v[2])]
}
