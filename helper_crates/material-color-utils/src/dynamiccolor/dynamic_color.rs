// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT OR Apache-2.0

use alloc::rc::Rc;
use alloc::string::String;

use crate::Argb;
use crate::contrast::Contrast;
use crate::hct::Hct;
use crate::math::round_to_int;
use crate::palettes::TonalPalette;

use super::color_spec::SpecVersion;
use super::color_specs::ColorSpecs;
use super::contrast_curve::ContrastCurve;
use super::dynamic_scheme::DynamicScheme;
use super::tone_delta_pair::ToneDeltaPair;

pub type PaletteFn = Rc<dyn Fn(&DynamicScheme) -> Rc<TonalPalette>>;
pub type ChromaMultiplierFn = Rc<dyn Fn(&DynamicScheme) -> f64>;
pub type ColorFn = Rc<dyn Fn(&DynamicScheme) -> Option<DynamicColor>>;
pub type ToneFn = Rc<dyn Fn(&DynamicScheme) -> f64>;
pub type ContrastCurveFn = Rc<dyn Fn(&DynamicScheme) -> Option<ContrastCurve>>;
pub type ToneDeltaPairFn = Rc<dyn Fn(&DynamicScheme) -> Option<ToneDeltaPair>>;
pub type OpacityFn = Rc<dyn Fn(&DynamicScheme) -> Option<f64>>;

/// A color that adjusts itself based on UI state, represented by DynamicScheme.
///
/// This color automatically adjusts to accommodate a desired contrast level, or
/// other adjustments such as differing in light mode versus dark mode, or what
/// the theme is, or what the color that produced the theme is, etc.
///
/// Colors without backgrounds do not change tone when contrast changes. Colors
/// with backgrounds become closer to their background as contrast lowers, and
/// further when contrast increases.
///
/// For example, the default behavior of adjust tone at max contrast to be at a
/// 7.0 ratio with its background is principled and matches accessibility
/// guidance. That does not mean it's the desired approach for _every_ design
/// system, and every color pairing, always, in every case.
///
/// Ultimately, each component necessary for calculating a color, adjusting it
/// for a desired contrast level, and ensuring it has a certain lightness/tone
/// difference from another color, is provided by a function that takes a
/// DynamicScheme and returns a value. This ensures ultimate flexibility, any
/// desired behavior of a color for any design system, but is usually
/// unnecessary.
#[derive(Clone)]
pub struct DynamicColor {
    /// The name of the dynamic color.
    pub name: String,
    /// Function that provides a TonalPalette given DynamicScheme. A
    /// TonalPalette is defined by a hue and chroma, so this replaces the need
    /// to specify hue/chroma. By providing a tonal palette, when contrast
    /// adjustments are made, intended chroma can be preserved.
    pub palette: PaletteFn,
    /// Whether this dynamic color is a background, with some other color as
    /// the foreground.
    pub is_background: bool,
    /// Function that provides a chroma multiplier, given a DynamicScheme.
    pub chroma_multiplier: Option<ChromaMultiplierFn>,
    /// Function that provides a background color, given a DynamicScheme.
    pub background: Option<ColorFn>,
    /// Function that provides a tone, given a DynamicScheme.
    pub tone: ToneFn,
    /// Function that provides a second background color, given a DynamicScheme.
    pub second_background: Option<ColorFn>,
    /// Function that provides a contrast curve, given a DynamicScheme.
    pub contrast_curve: Option<ContrastCurveFn>,
    /// Function that provides a tone delta pair, given a DynamicScheme.
    pub tone_delta_pair: Option<ToneDeltaPairFn>,
    /// Function that provides an opacity percentage, given a DynamicScheme.
    pub opacity: Option<OpacityFn>,
}

/// Builder for DynamicColor; mirrors the Java class's named parameters.
pub struct DynamicColorBuilder {
    name: &'static str,
    palette: PaletteFn,
    is_background: bool,
    chroma_multiplier: Option<ChromaMultiplierFn>,
    background: Option<ColorFn>,
    tone: Option<ToneFn>,
    second_background: Option<ColorFn>,
    contrast_curve: Option<ContrastCurveFn>,
    tone_delta_pair: Option<ToneDeltaPairFn>,
    opacity: Option<OpacityFn>,
}

impl DynamicColor {
    pub fn builder(name: &'static str, palette: PaletteFn) -> DynamicColorBuilder {
        DynamicColorBuilder {
            name,
            palette,
            is_background: false,
            chroma_multiplier: None,
            background: None,
            tone: None,
            second_background: None,
            contrast_curve: None,
            tone_delta_pair: None,
            opacity: None,
        }
    }

    /// Returns an ARGB integer (i.e. a hex code).
    ///
    /// `scheme`: Defines the conditions of the user interface, for example,
    /// whether or not it is dark mode or light mode, and what the desired
    /// contrast level is.
    pub fn get_argb(&self, scheme: &DynamicScheme) -> Argb {
        let argb = self.get_hct(scheme).to_int();
        match self.opacity.as_ref().and_then(|f| f(scheme)) {
            None => argb,
            Some(opacity_percentage) => {
                let alpha = round_to_int(opacity_percentage * 255.0).clamp(0, 255);
                (argb & 0x00ffffff) | (alpha << 24)
            }
        }
    }

    /// Returns an HCT object.
    ///
    /// `scheme`: Defines the conditions of the user interface, for example,
    /// whether or not it is dark mode or light mode, and what the desired
    /// contrast level is.
    pub fn get_hct(&self, scheme: &DynamicScheme) -> Hct {
        ColorSpecs::get(scheme.spec_version).get_hct(scheme, self)
    }

    /// Returns the tone in HCT, ranging from 0 to 100, of the resolved color
    /// given scheme.
    pub fn get_tone(&self, scheme: &DynamicScheme) -> f64 {
        ColorSpecs::get(scheme.spec_version).get_tone(scheme, self)
    }

    /// Create a DynamicColor from an ARGB value.
    ///
    /// Result has no background; thus no support for increasing/decreasing
    /// contrast for a11y.
    ///
    /// `name`: The name of the dynamic color.
    /// `argb`: The source color from which to extract the hue and chroma.
    pub fn from_argb(name: &'static str, argb: Argb) -> Self {
        let hct = Hct::from_int(argb);
        let palette = Rc::new(TonalPalette::from_int(argb));
        let tone = hct.tone();
        Self::builder(name, Rc::new(move |_| palette.clone())).tone(Rc::new(move |_| tone)).build()
    }

    /// Given a background tone, find a foreground tone, while ensuring they
    /// reach a contrast ratio that is as close to ratio as possible.
    pub fn foreground_tone(bg_tone: f64, ratio: f64) -> f64 {
        let lighter_tone = Contrast::lighter_unsafe(bg_tone, ratio);
        let darker_tone = Contrast::darker_unsafe(bg_tone, ratio);
        let lighter_ratio = Contrast::ratio_of_tones(lighter_tone, bg_tone);
        let darker_ratio = Contrast::ratio_of_tones(darker_tone, bg_tone);
        let prefer_lighter = Self::tone_prefers_light_foreground(bg_tone);
        if prefer_lighter {
            // "Neglible difference" handles an edge case where the initial
            // contrast ratio is high (ex. 13.0), and the ratio passed to the
            // function is that high ratio, and both the lighter and darker
            // ratio fails to pass that ratio.
            //
            // This was observed with Tonal Spot's On Primary Container turning
            // black momentarily between high and max contrast in light mode.
            // PC's standard tone was T90, OPC's was T10, it was light mode,
            // and the contrast level was 0.6568521221032331.
            let negligible_difference = (lighter_ratio - darker_ratio).abs() < 0.1
                && lighter_ratio < ratio
                && darker_ratio < ratio;
            if lighter_ratio >= ratio || lighter_ratio >= darker_ratio || negligible_difference {
                lighter_tone
            } else {
                darker_tone
            }
        } else {
            if darker_ratio >= ratio || darker_ratio >= lighter_ratio {
                darker_tone
            } else {
                lighter_tone
            }
        }
    }

    /// Adjust a tone down such that white has 4.5 contrast, if the tone is
    /// reasonably close to supporting it.
    pub fn enable_light_foreground(tone: f64) -> f64 {
        if Self::tone_prefers_light_foreground(tone) && !Self::tone_allows_light_foreground(tone) {
            49.0
        } else {
            tone
        }
    }

    /// People prefer white foregrounds on ~T60-70. Observed over time, and
    /// also by Andrew Somers during research for APCA.
    ///
    /// T60 used as to create the smallest discontinuity possible when skipping
    /// down to T49 in order to ensure light foregrounds.
    ///
    /// Since `tertiaryContainer` in dark monochrome scheme requires a tone of
    /// 60, it should not be adjusted. Therefore, 60 is excluded here.
    pub fn tone_prefers_light_foreground(tone: f64) -> bool {
        round_to_int(tone) < 60
    }

    /// Tones less than ~T50 always permit white at 4.5 contrast.
    pub fn tone_allows_light_foreground(tone: f64) -> bool {
        round_to_int(tone) <= 49
    }

    pub fn initial_tone_from_background(background: Option<ColorFn>) -> ToneFn {
        match background {
            None => Rc::new(|_| 50.0),
            Some(background) => Rc::new(move |scheme| {
                background(scheme).map(|c| c.get_tone(scheme)).unwrap_or(50.0)
            }),
        }
    }

    /// Wraps this color and `extended_color`, so that each field delegates to
    /// `extended_color` when `scheme.spec_version >= spec_version`, and to
    /// `self` otherwise.
    pub fn extend_spec_version(
        &self,
        spec_version: SpecVersion,
        extended_color: &DynamicColor,
    ) -> DynamicColor {
        self.validate_extended_color(spec_version, extended_color);
        let this = self.clone();
        let extended = extended_color.clone();
        DynamicColor {
            name: self.name.clone(),
            is_background: self.is_background,
            palette: {
                let this = this.clone();
                let extended = extended.clone();
                Rc::new(move |scheme| {
                    if scheme.spec_version >= spec_version {
                        (extended.palette)(scheme)
                    } else {
                        (this.palette)(scheme)
                    }
                })
            },
            tone: {
                let this = this.clone();
                let extended = extended.clone();
                Rc::new(move |scheme| {
                    if scheme.spec_version >= spec_version {
                        (extended.tone)(scheme)
                    } else {
                        (this.tone)(scheme)
                    }
                })
            },
            chroma_multiplier: {
                let this = this.clone();
                let extended = extended.clone();
                Some(Rc::new(move |scheme| {
                    let f = if scheme.spec_version >= spec_version {
                        extended.chroma_multiplier.as_ref()
                    } else {
                        this.chroma_multiplier.as_ref()
                    };
                    f.map(|f| f(scheme)).unwrap_or(1.0)
                }))
            },
            background: {
                let this = this.clone();
                let extended = extended.clone();
                Some(Rc::new(move |scheme| {
                    let f = if scheme.spec_version >= spec_version {
                        extended.background.as_ref()
                    } else {
                        this.background.as_ref()
                    };
                    f.and_then(|f| f(scheme))
                }))
            },
            second_background: {
                let this = this.clone();
                let extended = extended.clone();
                Some(Rc::new(move |scheme| {
                    let f = if scheme.spec_version >= spec_version {
                        extended.second_background.as_ref()
                    } else {
                        this.second_background.as_ref()
                    };
                    f.and_then(|f| f(scheme))
                }))
            },
            contrast_curve: {
                let this = this.clone();
                let extended = extended.clone();
                Some(Rc::new(move |scheme| {
                    let f = if scheme.spec_version >= spec_version {
                        extended.contrast_curve.as_ref()
                    } else {
                        this.contrast_curve.as_ref()
                    };
                    f.and_then(|f| f(scheme))
                }))
            },
            tone_delta_pair: {
                let this = this.clone();
                let extended = extended.clone();
                Some(Rc::new(move |scheme| {
                    let f = if scheme.spec_version >= spec_version {
                        extended.tone_delta_pair.as_ref()
                    } else {
                        this.tone_delta_pair.as_ref()
                    };
                    f.and_then(|f| f(scheme))
                }))
            },
            opacity: {
                let this = this.clone();
                let extended = extended.clone();
                Some(Rc::new(move |scheme| {
                    let f = if scheme.spec_version >= spec_version {
                        extended.opacity.as_ref()
                    } else {
                        this.opacity.as_ref()
                    };
                    f.and_then(|f| f(scheme))
                }))
            },
        }
    }

    fn validate_extended_color(&self, spec_version: SpecVersion, extended_color: &DynamicColor) {
        assert!(
            self.name == extended_color.name,
            "Attempting to extend color {} with color {} of different name for spec version {spec_version:?}.",
            self.name,
            extended_color.name,
        );
        assert!(
            self.is_background == extended_color.is_background,
            "Attempting to extend color {} as a {} with color {} as a {} for spec version {spec_version:?}.",
            self.name,
            if self.is_background { "background" } else { "foreground" },
            extended_color.name,
            if extended_color.is_background { "background" } else { "foreground" },
        );
    }
}

impl DynamicColorBuilder {
    #[allow(clippy::wrong_self_convention)] // Java: `setIsBackground`
    pub fn is_background(mut self, is_background: bool) -> Self {
        self.is_background = is_background;
        self
    }
    pub fn chroma_multiplier(mut self, f: ChromaMultiplierFn) -> Self {
        self.chroma_multiplier = Some(f);
        self
    }
    pub fn background(mut self, f: ColorFn) -> Self {
        self.background = Some(f);
        self
    }
    pub fn tone(mut self, f: ToneFn) -> Self {
        self.tone = Some(f);
        self
    }
    pub fn second_background(mut self, f: ColorFn) -> Self {
        self.second_background = Some(f);
        self
    }
    pub fn contrast_curve(mut self, f: ContrastCurveFn) -> Self {
        self.contrast_curve = Some(f);
        self
    }
    pub fn tone_delta_pair(mut self, f: ToneDeltaPairFn) -> Self {
        self.tone_delta_pair = Some(f);
        self
    }
    pub fn opacity(mut self, f: OpacityFn) -> Self {
        self.opacity = Some(f);
        self
    }

    pub fn build(self) -> DynamicColor {
        if self.background.is_none() && self.second_background.is_some() {
            panic!(
                "Color {} has second_background defined, but background is not defined.",
                self.name
            );
        }
        if self.background.is_none() && self.contrast_curve.is_some() {
            panic!(
                "Color {} has contrast_curve defined, but background is not defined.",
                self.name
            );
        }
        if self.background.is_some() && self.contrast_curve.is_none() {
            panic!(
                "Color {} has background defined, but contrast_curve is not defined.",
                self.name
            );
        }
        let tone = self
            .tone
            .unwrap_or_else(|| DynamicColor::initial_tone_from_background(self.background.clone()));
        DynamicColor {
            name: String::from(self.name),
            palette: self.palette,
            is_background: self.is_background,
            chroma_multiplier: self.chroma_multiplier,
            background: self.background,
            tone,
            second_background: self.second_background,
            contrast_curve: self.contrast_curve,
            tone_delta_pair: self.tone_delta_pair,
            opacity: self.opacity,
        }
    }
}
