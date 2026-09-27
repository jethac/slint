// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Port of kotlin/palettes/TonalPalette.kt (material-color-utilities @ 5b3618b).

use crate::Argb;
use crate::hct::Hct;
use crate::math;
use alloc::collections::BTreeMap;
use core::cell::RefCell;

const MAX_CHROMA_VALUE: f64 = 200.0;

/// A convenience class for retrieving colors that are constant in hue and
/// chroma, but vary in tone.
///
/// TonalPalette is intended for use in a single thread due to its stateful
/// caching.
pub struct TonalPalette {
    /// The hue of the Tonal Palette, in HCT. Ranges from 0 to 360.
    pub hue: f64,
    /// The chroma of the Tonal Palette, in HCT. Ranges from 0 to ~130 (for
    /// sRGB gamut).
    pub chroma: f64,
    /// The key color is the first tone, starting from T50, that matches the
    /// palette's chroma.
    pub key_color: Hct,
    cache: RefCell<BTreeMap<i64, Argb>>,
}

impl TonalPalette {
    fn new(hue: f64, chroma: f64, key_color: Hct) -> TonalPalette {
        TonalPalette { hue, chroma, key_color, cache: RefCell::new(BTreeMap::new()) }
    }

    /// Create an ARGB color with HCT hue and chroma of this Tones instance,
    /// and the provided HCT tone.
    ///
    /// `tone`: HCT tone, measured from 0 to 100.
    /// Returns ARGB representation of a color with that tone.
    pub fn tone(&self, tone: i64) -> Argb {
        if let Some(color) = self.cache.borrow().get(&tone) {
            return *color;
        }
        let color = if tone == 99 && Hct::is_yellow(self.hue) {
            Self::average_argb(self.tone(98), self.tone(100))
        } else {
            Hct::from(self.hue, self.chroma, tone as f64).to_int()
        };
        self.cache.borrow_mut().insert(tone, color);
        color
    }

    /// Given a tone, use hue and chroma of palette to create a color, and
    /// return it as HCT.
    pub fn get_hct(&self, tone: f64) -> Hct {
        if tone == 99.0 && Hct::is_yellow(self.hue) {
            return Hct::from_int(self.tone(99));
        }
        Hct::from(self.hue, self.chroma, tone)
    }

    fn average_argb(argb1: Argb, argb2: Argb) -> Argb {
        let red1 = (argb1 >> 16) & 0xff;
        let green1 = (argb1 >> 8) & 0xff;
        let blue1 = argb1 & 0xff;
        let red2 = (argb2 >> 16) & 0xff;
        let green2 = (argb2 >> 8) & 0xff;
        let blue2 = argb2 & 0xff;
        let red = math::round_to_int((red1 + red2) as f64 / 2.0);
        let green = math::round_to_int((green1 + green2) as f64 / 2.0);
        let blue = math::round_to_int((blue1 + blue2) as f64 / 2.0);
        (255 << 24) | ((red & 255) << 16) | ((green & 255) << 8) | (blue & 255)
    }

    /// Create tones using the HCT hue and chroma from a color.
    ///
    /// `argb`: ARGB representation of a color.
    /// Returns tones matching that color's hue and chroma.
    pub fn from_int(argb: Argb) -> TonalPalette {
        Self::from_hct(Hct::from_int(argb))
    }

    /// Create tones using a HCT color.
    ///
    /// `hct`: HCT representation of a color.
    /// Returns tones matching that color's hue and chroma.
    pub fn from_hct(hct: Hct) -> TonalPalette {
        Self::new(hct.hue(), hct.chroma(), hct)
    }

    /// Create tones from a defined HCT hue and chroma.
    ///
    /// `hue`: HCT hue
    /// `chroma`: HCT chroma
    /// Returns tones matching hue and chroma.
    pub fn from_hue_and_chroma(hue: f64, chroma: f64) -> TonalPalette {
        let key_color = KeyColor::new(hue, chroma).create();
        Self::new(hue, chroma, key_color)
    }
}

/// Key color is a color that represents the hue and chroma of a tonal
/// palette.
struct KeyColor {
    hue: f64,
    requested_chroma: f64,
    // Cache that maps tone to max chroma to avoid duplicated HCT calculation.
    chroma_cache: RefCell<BTreeMap<i64, f64>>,
}

impl KeyColor {
    fn new(hue: f64, requested_chroma: f64) -> KeyColor {
        KeyColor { hue, requested_chroma, chroma_cache: RefCell::new(BTreeMap::new()) }
    }

    /// Creates a key color from a `hue` and a `chroma`. The key color is the
    /// first tone, starting from T50, matching the given hue and chroma.
    fn create(&self) -> Hct {
        // Pivot around T50 because T50 has the most chroma available, on
        // average. Thus it is most likely to have a direct answer.
        let pivot_tone: i64 = 50;
        let tone_step_size: i64 = 1;
        // Epsilon to accept values slightly higher than the requested chroma.
        let epsilon = 0.01;

        // Binary search to find the tone that can provide a chroma that is
        // closest to the requested chroma.
        let mut lower_tone: i64 = 0;
        let mut upper_tone: i64 = 100;
        while lower_tone < upper_tone {
            let mid_tone = (lower_tone + upper_tone) / 2;
            let is_ascending =
                self.max_chroma(mid_tone) < self.max_chroma(mid_tone + tone_step_size);
            let sufficient_chroma = self.max_chroma(mid_tone) >= self.requested_chroma - epsilon;
            if sufficient_chroma {
                // Either range [lowerTone, midTone] or [midTone, upperTone]
                // has the answer, so search in the range that is closer the
                // pivot tone.
                if math::abs((lower_tone - pivot_tone) as f64)
                    < math::abs((upper_tone - pivot_tone) as f64)
                {
                    upper_tone = mid_tone;
                } else {
                    if lower_tone == mid_tone {
                        return Hct::from(self.hue, self.requested_chroma, lower_tone as f64);
                    }
                    lower_tone = mid_tone;
                }
            } else {
                // As there is no sufficient chroma in the midTone, follow the
                // direction to the chroma peak.
                if is_ascending {
                    lower_tone = mid_tone + tone_step_size;
                } else {
                    // Keep midTone for potential chroma peak.
                    upper_tone = mid_tone;
                }
            }
        }
        Hct::from(self.hue, self.requested_chroma, lower_tone as f64)
    }

    // Find the maximum chroma for a given tone
    fn max_chroma(&self, tone: i64) -> f64 {
        if let Some(chroma) = self.chroma_cache.borrow().get(&tone) {
            return *chroma;
        }
        let chroma = Hct::from(self.hue, MAX_CHROMA_VALUE, tone as f64).chroma();
        self.chroma_cache.borrow_mut().insert(tone, chroma);
        chroma
    }
}
