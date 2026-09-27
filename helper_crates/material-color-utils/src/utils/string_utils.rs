// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Port of kotlin/utils/StringUtils.kt (material-color-utilities @ 5b3618b).

use crate::Argb;
use crate::utils::ColorUtils;
use alloc::string::String;

/// Utility methods for string representations of colors.
pub struct StringUtils;

impl StringUtils {
    /// Hex string representing color, ex. #ff0000 for red.
    pub fn hex_from_argb(argb: Argb) -> String {
        let red = ColorUtils::red_from_argb(argb);
        let blue = ColorUtils::blue_from_argb(argb);
        let green = ColorUtils::green_from_argb(argb);
        alloc::format!("#{red:02x}{green:02x}{blue:02x}")
    }
}
