// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT OR Apache-2.0

use alloc::vec::Vec;

use crate::hct::Hct;
use crate::math::{atan2, cos, pow, round_to_int};
use crate::utils::ColorUtils;
use crate::utils::MathUtils;

/// Design utilities using color temperature theory.
///
/// Analogous colors, complementary color, and cache to efficiently, lazily,
/// generate data for calculations when needed.
pub struct TemperatureCache {
    input: Hct,
    /// All HCTs: `hcts_by_hue` (hues 0.0..=360.0) plus `input` appended.
    all_hcts: Vec<Hct>,
    /// Raw temperature of each entry in `all_hcts`, aligned by index.
    temps: Vec<f64>,
    /// Indices into `all_hcts` sorted by temperature.
    hcts_by_temp: Vec<usize>,
}

impl TemperatureCache {
    pub fn new(input: Hct) -> Self {
        let mut all_hcts = Vec::new();
        let mut hue = 0.0;
        while hue <= 360.0 {
            let color_at_hue = Hct::from(hue, input.chroma(), input.tone());
            all_hcts.push(color_at_hue);
            hue += 1.0;
        }
        all_hcts.push(input);
        let temps = all_hcts.iter().map(|hct| Self::raw_temperature(*hct)).collect::<Vec<f64>>();
        let mut hcts_by_temp: Vec<usize> = (0..all_hcts.len()).collect();
        hcts_by_temp.sort_by(|a, b| temps[*a].total_cmp(&temps[*b]));
        Self { input, all_hcts, temps, hcts_by_temp }
    }

    /// A color that complements the input color aesthetically.
    ///
    /// In art, this is usually described as being across the color wheel.
    /// History of this shows intent as a color that is just as cool-warm as the
    /// input color is warm-cool.
    pub fn complement(&self) -> Hct {
        let coldest_hue = self.coldest().hue();
        let coldest_temp = self.temp_of(self.coldest_index());
        let warmest_hue = self.warmest().hue();
        let warmest_temp = self.temp_of(self.warmest_index());
        let range = warmest_temp - coldest_temp;
        let start_hue_is_coldest_to_warmest =
            Self::is_between(self.input.hue(), coldest_hue, warmest_hue);
        let start_hue = if start_hue_is_coldest_to_warmest { warmest_hue } else { coldest_hue };
        let end_hue = if start_hue_is_coldest_to_warmest { coldest_hue } else { warmest_hue };
        let direction_of_rotation = 1.0;
        let mut smallest_error = 1000.0;
        let mut answer = self.hct_at(round_to_int(self.input.hue()) as usize);
        let complement_relative_temp = 1.0 - self.get_relative_temperature(self.input);
        // Find the color in the other section, closest to the inverse
        // percentile of the input color. This is the complement.
        let mut hue_addend = 0.0;
        while hue_addend <= 360.0 {
            let hue =
                MathUtils::sanitize_degrees_double(start_hue + direction_of_rotation * hue_addend);
            if !Self::is_between(hue, start_hue, end_hue) {
                hue_addend += 1.0;
                continue;
            }
            let possible_answer_index = round_to_int(hue) as usize;
            let relative_temp = (self.temp_of(possible_answer_index) - coldest_temp) / range;
            let error = (complement_relative_temp - relative_temp).abs();
            if error < smallest_error {
                smallest_error = error;
                answer = self.hct_at(possible_answer_index);
            }
            hue_addend += 1.0;
        }
        answer
    }

    /// 5 colors that pair well with the input color.
    ///
    /// The colors are equidistant in temperature and adjacent in hue.
    pub fn get_analogous_colors_default(&self) -> Vec<Hct> {
        self.get_analogous_colors(5, 12)
    }

    /// A set of colors with differing hues, equidistant in temperature.
    ///
    /// In art, this is usually described as a set of 5 colors on a color wheel
    /// divided into 12 sections. This method allows provision of either of
    /// those values.
    ///
    /// Behavior is undefined when `count` or `divisions` is 0. When divisions <
    /// count, colors repeat.
    ///
    /// `count`: The number of colors to return, includes the input color.
    /// `divisions`: The number of divisions on the color wheel.
    ///
    /// Returns: A list of `count` HCTs.
    pub fn get_analogous_colors(&self, count: i32, divisions: i32) -> Vec<Hct> {
        // The starting hue is the hue of the input color.
        let start_hue = round_to_int(self.input.hue());
        let start_hct = self.hct_at(start_hue as usize);
        let mut last_temp = self.get_relative_temperature_by_index(start_hue as usize);
        let mut all_colors: Vec<Hct> = Vec::new();
        all_colors.push(start_hct);
        let mut absolute_total_temp_delta: f32 = 0.0;
        for i in 0..360 {
            let hue = MathUtils::sanitize_degrees_int(start_hue + i);
            let hct_index = hue as usize;
            let temp = self.get_relative_temperature_by_index(hct_index);
            let temp_delta = (temp - last_temp).abs();
            last_temp = temp;
            absolute_total_temp_delta += temp_delta as f32;
        }
        let mut hue_addend = 1;
        let temp_step = absolute_total_temp_delta as f64 / divisions as f64;
        let mut total_temp_delta = 0.0;
        last_temp = self.get_relative_temperature_by_index(start_hue as usize);
        while all_colors.len() < divisions as usize {
            let hue = MathUtils::sanitize_degrees_int(start_hue + hue_addend);
            let hct_index = hue as usize;
            let hct = self.hct_at(hct_index);
            let temp = self.get_relative_temperature_by_index(hct_index);
            let temp_delta = (temp - last_temp).abs();
            total_temp_delta += temp_delta;
            let mut desired_total_temp_delta_for_index = all_colors.len() as f64 * temp_step;
            let mut index_satisfied = total_temp_delta >= desired_total_temp_delta_for_index;
            let mut index_addend = 1;
            // Keep adding this hue to the answers until its temperature is
            // insufficient. This ensures consistent behavior when there aren't
            // `divisions` discrete steps between 0 and 360 in hue with
            // `tempStep` delta in temperature between them.
            //
            // For example, white and black have no analogues: there are no
            // other colors at T100/T0. Therefore, they should just be added to
            // the array as answers.
            while index_satisfied && all_colors.len() < divisions as usize {
                all_colors.push(hct);
                desired_total_temp_delta_for_index =
                    (all_colors.len() + index_addend) as f64 * temp_step;
                index_satisfied = total_temp_delta >= desired_total_temp_delta_for_index;
                index_addend += 1;
            }
            last_temp = temp;
            hue_addend += 1;
            if hue_addend > 360 {
                while all_colors.len() < divisions as usize {
                    all_colors.push(hct);
                }
                break;
            }
        }
        let mut answers: Vec<Hct> = Vec::new();
        answers.push(self.input);
        let ccw_count = ((count as f64 - 1.0) / 2.0).floor() as i32;
        for i in 1..(ccw_count + 1) {
            let mut index = 0 - i;
            while index < 0 {
                index = all_colors.len() as i32 + index;
            }
            if index >= all_colors.len() as i32 {
                index %= all_colors.len() as i32;
            }
            answers.insert(0, all_colors[index as usize]);
        }
        let cw_count = count - ccw_count - 1;
        for i in 1..(cw_count + 1) {
            let mut index = i;
            while index < 0 {
                index = all_colors.len() as i32 + index;
            }
            if index >= all_colors.len() as i32 {
                index %= all_colors.len() as i32;
            }
            answers.push(all_colors[index as usize]);
        }
        answers
    }

    /// Temperature relative to all colors with the same chroma and tone as the
    /// input color, on a scale from 0 to 1.
    pub fn get_relative_temperature(&self, hct: Hct) -> f64 {
        let range = self.temp_of(self.warmest_index()) - self.temp_of(self.coldest_index());
        let difference_from_coldest =
            Self::raw_temperature(hct) - self.temp_of(self.coldest_index());
        // Handle when there's no difference in temperature between warmest and
        // coldest: for example, at T100, only one color is available, white.
        if range == 0.0 { 0.5 } else { difference_from_coldest / range }
    }

    fn get_relative_temperature_by_index(&self, index: usize) -> f64 {
        self.get_relative_temperature(self.all_hcts[index])
    }

    /// Coldest color with same chroma and tone as input.
    fn coldest(&self) -> Hct {
        self.all_hcts[self.hcts_by_temp[0]]
    }

    fn coldest_index(&self) -> usize {
        self.hcts_by_temp[0]
    }

    /// Warmest color with same chroma and tone as input.
    fn warmest(&self) -> Hct {
        self.all_hcts[*self.hcts_by_temp.last().unwrap()]
    }

    fn warmest_index(&self) -> usize {
        *self.hcts_by_temp.last().unwrap()
    }

    /// The HCT at the given index of `hcts_by_hue` (hues 0..=360).
    fn hct_at(&self, index: usize) -> Hct {
        self.all_hcts[index]
    }

    fn temp_of(&self, index: usize) -> f64 {
        self.temps[index]
    }

    /// Value representing cool-warm factor of a color. Values below 0 are
    /// considered cool, above, warm.
    ///
    /// Color science has researched emotion and harmony, which art uses to
    /// select colors. Warm-cool is the foundation of analogous and
    /// complementary colors. See:
    /// - Li-Chen Ou's "Color Emotion and Color Harmony in Color Images"
    /// - Jose Antonio Camacho-Olguin's "Aesthetic-Based Multi-Objective
    ///   Optimization of the Color in Art Images" (2015)
    /// - https://www.sensationalcolor.com/color-temperature/
    /// - https://en.wikipedia.org/wiki/Color_temperature
    ///
    /// This is the data set used to determine cool-warm factor.
    ///
    /// `color`: The color to calculate the cool-warm factor of.
    ///
    /// Returns: The cool-warm factor of the color.
    pub fn raw_temperature(color: Hct) -> f64 {
        let lab = ColorUtils::lab_from_argb(color.to_int());
        let hue = MathUtils::sanitize_degrees_double(
            atan2(lab[2], lab[1]) * 180.0 / core::f64::consts::PI,
        );
        let chroma = crate::math::sqrt(lab[1] * lab[1] + lab[2] * lab[2]);
        -0.5 + 0.02
            * pow(chroma, 1.07)
            * cos(MathUtils::sanitize_degrees_double(hue - 50.0) * core::f64::consts::PI / 180.0)
    }

    /// Determines if an angle is between two other angles, rotating clockwise.
    fn is_between(angle: f64, a: f64, b: f64) -> bool {
        if a < b { a <= angle && angle <= b } else { a <= angle || angle <= b }
    }
}
