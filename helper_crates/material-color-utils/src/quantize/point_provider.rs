// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::Argb;

/// An interface to allow use of different color spaces by quantizers.
pub trait PointProvider {
    /// The four components in the color space of an sRGB color.
    fn from_int(&self, argb: Argb) -> [f64; 4];
    /// The ARGB (i.e. hex code) representation of this color.
    fn to_int(&self, point: [f64; 4]) -> Argb;
    /// Squared distance between two points in the provider's color space.
    fn distance(&self, a: [f64; 4], b: [f64; 4]) -> f64;
}
