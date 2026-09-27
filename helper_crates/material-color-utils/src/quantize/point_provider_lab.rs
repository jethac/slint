// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::Argb;
use crate::utils::ColorUtils;

use super::point_provider::PointProvider;

/// Provides conversions needed for K-Means quantization. Converting input to
/// points, and converting the final state of the K-Means algorithm to colors.
pub struct PointProviderLab;

impl PointProvider for PointProviderLab {
    /// Convert a color represented in ARGB to a 3-element array of L*a*b*
    /// coordinates of the color.
    fn from_int(&self, argb: Argb) -> [f64; 4] {
        let lab = ColorUtils::lab_from_argb(argb);
        [lab[0], lab[1], lab[2], (lab[0] + lab[1] + lab[2])]
    }

    /// Convert a 3-element array to a color represented in ARGB.
    fn to_int(&self, point: [f64; 4]) -> Argb {
        ColorUtils::argb_from_lab(point[0], point[1], point[2])
    }

    /// Standard CIE 1976 delta E formula also takes the square root, unneeded
    /// here. This method is used by quantization algorithms to compare
    /// distance, and the relative ordering is the same, with or without a
    /// square root.
    ///
    /// This relatively minor optimization is helpful because this method is
    /// called at least once per pixel in an image.
    fn distance(&self, a: [f64; 4], b: [f64; 4]) -> f64 {
        let d_l = a[0] - b[0];
        let d_a = a[1] - b[1];
        let d_b = a[2] - b[2];
        d_l * d_l + d_a * d_a + d_b * d_b
    }
}
