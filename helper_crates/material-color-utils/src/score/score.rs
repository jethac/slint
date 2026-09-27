// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT OR Apache-2.0

use alloc::vec::Vec;

use crate::Argb;
use crate::hct::Hct;
use crate::math::round_to_int;
use crate::utils::MathUtils;

const TARGET_CHROMA: f64 = 48.0; // A1 Chroma
const WEIGHT_PROPORTION: f64 = 0.7;
const WEIGHT_CHROMA_ABOVE: f64 = 0.3;
const WEIGHT_CHROMA_BELOW: f64 = 0.1;
const CUTOFF_CHROMA: f64 = 5.0;
const CUTOFF_EXCITED_PROPORTION: f64 = 0.01;

const DEFAULT_FALLBACK: Argb = 0xff4285f4;

/// Given a map with keys of colors and values of how often the color appears,
/// rank the colors based on suitability for being used for a UI theme.
///
/// The colors returned are the color map's keys, in descending order of score.
/// The algorithm selects the colors that best reflect the theme of the image.
/// There is no guarantee that a color will be within the colors provided.
///
/// `colors_to_population`: map with keys of colors and values of how often the
/// pixel appears.
///
/// Returns: colors sorted by suitability for a theme. The most suitable color
/// is the first item, the least suitable is the last. There will always be at
/// least one color returned. If all the input colors were not suitable for a
/// theme, a default color, `0xff4285f4`, is provided.
pub struct Score;

impl Score {
    /// Given a map with keys of colors and values of how often the color
    /// appears, rank the colors based on suitability for being used for a UI
    /// theme.
    ///
    /// The list returned is of length <= `desired`. The recommended color is
    /// the first item on the list. If no color is filtered to be of a
    /// reasonable contrast with the background, `fallback_color_argb` is
    /// returned.
    pub fn score(
        colors_to_population: &[(Argb, i64)],
        desired: usize,
        fallback_color_argb: Argb,
        filter: bool,
    ) -> Vec<Argb> {
        // Get the HCT color for each Argb value, while finding the per hue
        // count and total count.
        let mut colors_hct: Vec<Hct> = Vec::new();
        let mut hue_population = [0i64; 360];
        let mut population_sum = 0.0;
        for (key, value) in colors_to_population {
            let hct = Hct::from_int(*key);
            colors_hct.push(hct);
            let hue = crate::math::floor(hct.hue()) as i64;
            hue_population[hue as usize] += value;
            population_sum += *value as f64;
        }
        // Hues with more usage in neighboring 30 degree slice get a larger
        // number.
        let mut hue_excited_proportions = [0.0f64; 360];
        for (hue, &population) in hue_population.iter().enumerate() {
            let proportion = population as f64 / population_sum;
            for i in (hue as i64 - 14)..(hue as i64 + 16) {
                let neighbor_hue = MathUtils::sanitize_degrees_int(i);
                hue_excited_proportions[neighbor_hue as usize] += proportion;
            }
        }
        // Scores each HCT color based on usage and chroma, while optionally
        // filtering out values that do not have enough chroma or usage.
        let mut scored_hcts: Vec<(Hct, f64)> = Vec::new();
        for hct in colors_hct {
            let hue = MathUtils::sanitize_degrees_int(round_to_int(hct.hue()));
            let proportion = hue_excited_proportions[hue as usize];
            if filter && (hct.chroma() < CUTOFF_CHROMA || proportion <= CUTOFF_EXCITED_PROPORTION) {
                continue;
            }
            let proportion_score = proportion * 100.0 * WEIGHT_PROPORTION;
            let chroma_weight = if hct.chroma() < TARGET_CHROMA {
                WEIGHT_CHROMA_BELOW
            } else {
                WEIGHT_CHROMA_ABOVE
            };
            let chroma_score = (hct.chroma() - TARGET_CHROMA) * chroma_weight;
            let score = proportion_score + chroma_score;
            scored_hcts.push((hct, score));
        }
        // Sorted so that colors with higher scores come first. Stable sort to
        // match Java's Collections.sort.
        scored_hcts.sort_by(|a, b| b.1.total_cmp(&a.1));
        // Iterates through potential hue differences in degrees in order to
        // select the colors with the largest distribution of hues possible.
        // Starting at 90 degrees(maximum difference for 4 colors) then
        // decreasing down to a 15 degree minimum.
        let mut chosen_colors: Vec<Hct> = Vec::new();
        for difference_degrees in (15..=90).rev() {
            chosen_colors.clear();
            for (hct, _) in &scored_hcts {
                let mut has_duplicate_hue = false;
                for chosen_hct in &chosen_colors {
                    if MathUtils::difference_degrees(hct.hue(), chosen_hct.hue())
                        < difference_degrees as f64
                    {
                        has_duplicate_hue = true;
                        break;
                    }
                }
                if !has_duplicate_hue {
                    chosen_colors.push(*hct);
                }
                if chosen_colors.len() >= desired {
                    break;
                }
            }
            if chosen_colors.len() >= desired {
                break;
            }
        }
        let mut colors: Vec<Argb> = Vec::new();
        if chosen_colors.is_empty() {
            colors.push(fallback_color_argb);
        }
        for chosen_hct in chosen_colors {
            colors.push(chosen_hct.to_int());
        }
        colors
    }
}

/// Returns `score(colorsToPopulation, 4, 0xff4285f4, true)` — the convenience
/// overload used for seed-color selection.
pub fn score_colors(colors_to_population: &[(Argb, i64)]) -> Vec<Argb> {
    Score::score(colors_to_population, 4, DEFAULT_FALLBACK, true)
}
