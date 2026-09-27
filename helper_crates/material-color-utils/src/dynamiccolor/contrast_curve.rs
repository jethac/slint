// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::utils::MathUtils;

/// Documents a constraint between two DynamicColors, in which their tone
/// distance must be maintained.
#[derive(Debug, Clone, Copy)]
pub struct ContrastCurve {
    /// Contrast requirement for contrast level -1.0
    pub low: f64,
    /// Contrast requirement for contrast level 0.0
    pub normal: f64,
    /// Contrast requirement for contrast level 0.5
    pub medium: f64,
    /// Contrast requirement for contrast level 1.0
    pub high: f64,
}

impl ContrastCurve {
    pub fn new(low: f64, normal: f64, medium: f64, high: f64) -> Self {
        Self { low, normal, medium, high }
    }

    /// Returns the contrast ratio at a given contrast level.
    ///
    /// `contrast_level`: The contrast level. 0.0 is the normal amount; higher
    /// values increase contrast; values decrease until -1.0.
    ///
    /// Returns: the contrast ratio, a number between 1.0 and 21.0.
    pub fn get(&self, contrast_level: f64) -> f64 {
        if contrast_level <= -1.0 {
            self.low
        } else if contrast_level < 0.0 {
            MathUtils::lerp(self.low, self.normal, contrast_level + 1.0)
        } else if contrast_level < 0.5 {
            MathUtils::lerp(self.normal, self.medium, contrast_level / 0.5)
        } else if contrast_level < 1.0 {
            MathUtils::lerp(self.medium, self.high, (contrast_level - 0.5) / 0.5)
        } else {
            self.high
        }
    }
}
