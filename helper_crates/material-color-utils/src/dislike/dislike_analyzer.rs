// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::hct::Hct;
use crate::math::round_to_int;

/// Check and/or fix universally disliked colors.
///
/// Color science studies researched before 2016 are inclusive of ~4% of the
/// population. Those who dislike these colors have the common denominator of
/// relatively high chroma, and are often described as liking muted colors, or
/// stating that the color is "muddy" or "washed out", but is "too vivid".
///
/// Design team noticed that if a color is used as a background, it should be
/// toned to a minimum of 30% luminance. Otherwise, it appears more as a
/// foreground color. From testing, the minimum luminance is 70 for texts on
/// background colors.
///
/// Design preferences on color, stated in https://m3.material.io, do not use
/// these colors.
pub struct DislikeAnalyzer;

impl DislikeAnalyzer {
    /// Returns true if a color is disliked.
    ///
    /// Disliked is defined as a dark yellow-green that is not neutral.
    /// Specifically:
    /// - Hue 90 to 111
    /// - Chroma greater than 16
    /// - Tone less than 65
    ///
    /// This mainly refers to CAM16-UCS hue, but the hue is not restricted to
    /// exactly 90-111.
    pub fn is_disliked(hct: Hct) -> bool {
        let hue_passes = round_to_int(hct.hue()) >= 90 && round_to_int(hct.hue()) <= 111;
        let chroma_passes = round_to_int(hct.chroma()) > 16;
        let tone_passes = round_to_int(hct.tone()) < 65;
        hue_passes && chroma_passes && tone_passes
    }

    /// If color is disliked, lighten it to make it likable.
    pub fn fix_if_disliked(hct: Hct) -> Hct {
        if Self::is_disliked(hct) { Hct::from(hct.hue(), hct.chroma(), 70.0) } else { hct }
    }
}
