// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::Argb;

use super::quantizer_result::QuantizerResult;

/// An interface to allow use of different quantizers over image data.
pub trait Quantizer {
    /// Reduce the image to `max_colors` colors and return the result.
    fn quantize(&mut self, pixels: &[Argb], max_colors: i32) -> QuantizerResult;
}
