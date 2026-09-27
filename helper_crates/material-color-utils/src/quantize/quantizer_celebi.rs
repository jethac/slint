// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT OR Apache-2.0
use alloc::vec::Vec;

use crate::Argb;

use super::quantizer_result::QuantizerResult;
use super::quantizer_wsmeans::QuantizerWsmeans;
use super::quantizer_wu::QuantizerWu;

/// An image quantizer that uses the Wu quantizer to cluster points, then uses
/// a K-Means algorithm, with the clusters weighted by their importance in the
/// original image, to improve the quality of the final colors.
///
/// Algorithm was designed by M. Emre Celebi, and was found in 2011,
/// Improving the Performance of K-Means for Color Quantization.
/// <https://arxiv.org/abs/1101.0395>
pub struct QuantizerCelebi;

impl QuantizerCelebi {
    /// `pixels`: Pixels in the image as ARGB ints.
    /// `max_colors`: The maximum number of colors to return.
    ///
    /// Returns: Map of colors (as ARGB ints) to the number of times the color
    /// appears in the image.
    pub fn quantize(pixels: &[Argb], max_colors: i32) -> QuantizerResult {
        let mut wu = QuantizerWu::new();
        let wu_result = wu.quantize(pixels, max_colors);
        let wu_clusters: Vec<Argb> =
            wu_result.color_to_count.iter().map(|&(argb, _)| argb).collect();
        QuantizerResult {
            color_to_count: QuantizerWsmeans::quantize(pixels, &wu_clusters, max_colors),
        }
    }
}
