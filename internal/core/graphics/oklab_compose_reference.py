#!/usr/bin/env python3
# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

"""Regenerates the `(l, a, b)` reference cases in `color.rs`'s
`test_oklab_compose_reference`.

Ports the pinned androidx conversion path verbatim in float32:
`Rgb.eotfFunc` → `Srgb.adapt(Illuminant.D50)` (`Connector.transform`) →
`Oklab.fromXyz` (commit 23327507f7fc7d5b19d65fec4b090f60c970079b). Same
operation order and f32 width as `compose_oklab` in `color.rs`.
"""

import numpy as np

f32 = np.float32


def mul3x3f3(m, v):
    m = np.asarray(m, dtype=f32)
    return np.array(
        [
            m[0] * v[0] + m[3] * v[1] + m[6] * v[2],
            m[1] * v[0] + m[4] * v[1] + m[7] * v[2],
            m[2] * v[0] + m[5] * v[1] + m[8] * v[2],
        ],
        dtype=f32,
    )


def mul3x3(lhs, rhs):
    lhs = np.asarray(lhs, dtype=f32)
    rhs = np.asarray(rhs, dtype=f32)
    out = np.empty(9, dtype=f32)
    for col in range(3):
        for row in range(3):
            out[col * 3 + row] = (
                lhs[row] * rhs[col * 3]
                + lhs[3 + row] * rhs[col * 3 + 1]
                + lhs[6 + row] * rhs[col * 3 + 2]
            )
    return out


def inverse3x3(m):
    m = np.asarray(m, dtype=f32)
    a, b, c, d, e, f, g, h, i = m[0], m[3], m[6], m[1], m[4], m[7], m[2], m[5], m[8]
    xa = e * i - f * h
    xb = f * g - d * i
    xc = d * h - e * g
    det = a * xa + b * xb + c * xc
    return np.array(
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
        ],
        dtype=f32,
    )


def mul3x3_diag(lhs, rhs):
    lhs = np.asarray(lhs, dtype=f32)
    rhs = np.asarray(rhs, dtype=f32)
    out = np.empty(9, dtype=f32)
    for col in range(3):
        for row in range(3):
            out[col * 3 + row] = lhs[row] * rhs[col * 3 + row]
    return out


BRADFORD = np.array(
    [0.8951, -0.7502, 0.0389, 0.2664, 1.7135, -0.0685, -0.1614, 0.0367, 1.0296],
    dtype=f32,
)


def chromatic_adaptation(src_xyz, dst_xyz):
    """`ColorSpace.kt`'s `chromaticAdaptation(Adaptation.Bradford, src, dst)`."""
    src_lms = mul3x3f3(BRADFORD, src_xyz)
    dst_lms = mul3x3f3(BRADFORD, dst_xyz)
    lms = dst_lms / src_lms
    return mul3x3(inverse3x3(BRADFORD), mul3x3_diag(lms, BRADFORD))


def white_point_to_xyz(x, y):
    return np.array([x / y, 1.0, (1.0 - x - y) / y], dtype=f32)


D50_XYZ = white_point_to_xyz(f32(0.34567), f32(0.35850))
D65_XYZ = white_point_to_xyz(f32(0.31271), f32(0.32902))


def srgb_to_xyz_d65():
    """`Rgb.kt`'s `computeXYZMatrix` for the sRGB primaries under D65."""
    rx, ry, gx, gy, bx, by = map(f32, (0.640, 0.330, 0.300, 0.600, 0.150, 0.060))
    wx, wy = f32(0.31271), f32(0.32902)
    one_rx_ry = (1.0 - rx) / ry
    one_gx_gy = (1.0 - gx) / gy
    one_bx_by = (1.0 - bx) / by
    one_wx_wy = (1.0 - wx) / wy
    rx_ry, gx_gy, bx_by, wx_wy = rx / ry, gx / gy, bx / by, wx / wy
    by_lum = (
        (one_wx_wy - one_rx_ry) * (gx_gy - rx_ry)
        - (wx_wy - rx_ry) * (one_gx_gy - one_rx_ry)
    ) / (
        (one_bx_by - one_rx_ry) * (gx_gy - rx_ry)
        - (bx_by - rx_ry) * (one_gx_gy - one_rx_ry)
    )
    gy_lum = (wx_wy - rx_ry - by_lum * (bx_by - rx_ry)) / (gx_gy - rx_ry)
    ry_lum = 1.0 - gy_lum - by_lum
    r_ry, g_gy, b_by = ry_lum / ry, gy_lum / gy, by_lum / by
    return np.array(
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
        ],
        dtype=f32,
    )


SRGB_TO_XYZ_D50 = mul3x3(chromatic_adaptation(D65_XYZ, D50_XYZ), srgb_to_xyz_d65())

# `Oklab.kt`'s `M1`: raw Oklab M1 times Bradford D50→D65.
OKLAB_M1 = mul3x3(
    np.array(
        [
            0.8189330101,
            0.0329845436,
            0.0482003018,
            0.3618667424,
            0.9293118715,
            0.2643662691,
            -0.1288597137,
            0.0361456387,
            0.6338517070,
        ],
        dtype=f32,
    ),
    chromatic_adaptation(D50_XYZ, D65_XYZ),
)

OKLAB_M2 = np.array(
    [
        0.2104542553,
        1.9779984951,
        0.0259040371,
        0.7936177850,
        -2.4285922050,
        0.7827717662,
        -0.0040720468,
        0.4505937099,
        -0.8086757660,
    ],
    dtype=f32,
)


def srgb_eotf(c):
    """`Rgb.kt`'s sRGB EOTF (runs on `Double` in Compose)."""
    c = float(c)
    return f32(c / 12.92 if c <= 0.04045 else ((c + 0.055) / 1.055) ** 2.4)


def srgb_to_oklab(r, g, b):
    """`Connector.transform` sRGB→Oklab, returning `(l, a, b)`."""
    v = mul3x3f3(
        SRGB_TO_XYZ_D50, np.array([srgb_eotf(r), srgb_eotf(g), srgb_eotf(b)], dtype=f32)
    )
    v = mul3x3f3(OKLAB_M1, v)
    v = np.cbrt(v).astype(f32)
    return mul3x3f3(OKLAB_M2, v)


CASES = [
    (1.0, 0.0, 0.0),
    (0.0, 1.0, 0.0),
    (0.0, 0.0, 1.0),
    (1.0, 1.0, 1.0),
    (0.4, 0.2, 0.9),
    (0.5, 0.5, 0.5),
    (0.0, 1.0, 1.0),
    (1.0, 1.0, 0.0),
]


def main():
    print("let cases = [")
    for r, g, b in CASES:
        lab = srgb_to_oklab(r, g, b)
        print(
            f"    ([{r:.1f}, {g:.1f}, {b:.1f}], "
            f"[{float(lab[0]):.6f}, {float(lab[1]):.6f}, {float(lab[2]):.6f}]),"
        )
    print("];")


if __name__ == "__main__":
    main()
