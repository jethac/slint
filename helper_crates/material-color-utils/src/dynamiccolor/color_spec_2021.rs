// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT OR Apache-2.0

use alloc::rc::Rc;

use crate::contrast::Contrast;
use crate::dislike::DislikeAnalyzer;
use crate::hct::Hct;
use crate::palettes::TonalPalette;
use crate::temperature::TemperatureCache;
use crate::utils::MathUtils;
use alloc::vec::Vec;

use super::color_spec::ColorSpec;
use super::color_spec_2026::ColorSpec2026;
use super::contrast_curve::ContrastCurve;
use super::dynamic_color::{
    ColorFn, ContrastCurveFn, DynamicColor, PaletteFn, ToneDeltaPairFn, ToneFn,
};
use super::dynamic_scheme::{DynamicScheme, Platform};
use super::tone_delta_pair::{DeltaConstraint, ToneDeltaPair, TonePolarity};
use super::variant::Variant;

fn pal(f: impl Fn(&DynamicScheme) -> Rc<TonalPalette> + 'static) -> PaletteFn {
    Rc::new(f)
}
fn bg(f: impl Fn(&DynamicScheme) -> Option<DynamicColor> + 'static) -> ColorFn {
    Rc::new(f)
}
fn tn(f: impl Fn(&DynamicScheme) -> f64 + 'static) -> ToneFn {
    Rc::new(f)
}
fn tdp(f: impl Fn(&DynamicScheme) -> Option<ToneDeltaPair> + 'static) -> ToneDeltaPairFn {
    Rc::new(f)
}
/// `background = { color }`: a closure returning `color` for any scheme.
fn fixed_color(color: DynamicColor) -> ColorFn {
    Rc::new(move |_| Some(color.clone()))
}
fn fixed_curve(curve: ContrastCurve) -> ContrastCurveFn {
    Rc::new(move |_| Some(curve))
}

/// `ColorSpec` implementation for the 2021 spec.
pub struct ColorSpec2021;

impl ColorSpec2021 {
    pub fn is_fidelity(scheme: &DynamicScheme) -> bool {
        scheme.variant == Variant::Fidelity || scheme.variant == Variant::Content
    }

    pub fn is_monochrome(scheme: &DynamicScheme) -> bool {
        scheme.variant == Variant::Monochrome
    }

    fn find_desired_chroma_by_tone(
        hue: f64,
        chroma: f64,
        tone: f64,
        by_decreasing_tone: bool,
    ) -> f64 {
        let mut answer = tone;
        let mut closest_to_chroma = Hct::from(hue, chroma, tone);
        if closest_to_chroma.chroma() < chroma {
            let mut chroma_peak = closest_to_chroma.chroma();
            while closest_to_chroma.chroma() < chroma {
                answer += if by_decreasing_tone { -1.0 } else { 1.0 };
                let potential_solution = Hct::from(hue, chroma, answer);
                if chroma_peak > potential_solution.chroma() {
                    break;
                }
                if (potential_solution.chroma() - chroma).abs() < 0.4 {
                    break;
                }
                let potential_delta = (potential_solution.chroma() - chroma).abs();
                let current_delta = (closest_to_chroma.chroma() - chroma).abs();
                if potential_delta < current_delta {
                    closest_to_chroma = potential_solution;
                }
                chroma_peak = chroma_peak.max(potential_solution.chroma());
            }
        }
        answer
    }
}

impl ColorSpec for ColorSpec2021 {
    // ////////////////////////////////////////////////////////////////
    // Main Palettes //
    // ////////////////////////////////////////////////////////////////
    fn primary_palette_key_color(&self) -> DynamicColor {
        DynamicColor::builder("primary_palette_key_color", pal(|s| s.primary_palette.clone()))
            .tone(tn(|s| s.primary_palette.key_color.tone()))
            .build()
    }

    fn secondary_palette_key_color(&self) -> DynamicColor {
        DynamicColor::builder("secondary_palette_key_color", pal(|s| s.secondary_palette.clone()))
            .tone(tn(|s| s.secondary_palette.key_color.tone()))
            .build()
    }

    fn tertiary_palette_key_color(&self) -> DynamicColor {
        DynamicColor::builder("tertiary_palette_key_color", pal(|s| s.tertiary_palette.clone()))
            .tone(tn(|s| s.tertiary_palette.key_color.tone()))
            .build()
    }

    fn neutral_palette_key_color(&self) -> DynamicColor {
        DynamicColor::builder("neutral_palette_key_color", pal(|s| s.neutral_palette.clone()))
            .tone(tn(|s| s.neutral_palette.key_color.tone()))
            .build()
    }

    fn neutral_variant_palette_key_color(&self) -> DynamicColor {
        DynamicColor::builder(
            "neutral_variant_palette_key_color",
            pal(|s| s.neutral_variant_palette.clone()),
        )
        .tone(tn(|s| s.neutral_variant_palette.key_color.tone()))
        .build()
    }

    fn error_palette_key_color(&self) -> DynamicColor {
        DynamicColor::builder("error_palette_key_color", pal(|s| s.error_palette.clone()))
            .tone(tn(|s| s.error_palette.key_color.tone()))
            .build()
    }

    // ////////////////////////////////////////////////////////////////
    // Surfaces [S] //
    // ////////////////////////////////////////////////////////////////
    fn background(&self) -> DynamicColor {
        DynamicColor::builder("background", pal(|s| s.neutral_palette.clone()))
            .tone(tn(|s| if s.is_dark { 6.0 } else { 98.0 }))
            .is_background(true)
            .build()
    }

    fn on_background(&self) -> DynamicColor {
        DynamicColor::builder("on_background", pal(|s| s.neutral_palette.clone()))
            .tone(tn(|s| if s.is_dark { 90.0 } else { 10.0 }))
            .background(fixed_color(ColorSpec2026.background()))
            .contrast_curve(fixed_curve(ContrastCurve::new(3.0, 3.0, 4.5, 7.0)))
            .build()
    }

    fn surface(&self) -> DynamicColor {
        DynamicColor::builder("surface", pal(|s| s.neutral_palette.clone()))
            .tone(tn(|s| if s.is_dark { 6.0 } else { 98.0 }))
            .is_background(true)
            .build()
    }

    fn surface_dim(&self) -> DynamicColor {
        DynamicColor::builder("surface_dim", pal(|s| s.neutral_palette.clone()))
            .tone(tn(|s| {
                if s.is_dark {
                    6.0
                } else {
                    ContrastCurve::new(87.0, 87.0, 80.0, 75.0).get(s.contrast_level)
                }
            }))
            .is_background(true)
            .build()
    }

    fn surface_bright(&self) -> DynamicColor {
        DynamicColor::builder("surface_bright", pal(|s| s.neutral_palette.clone()))
            .tone(tn(|s| {
                if s.is_dark {
                    ContrastCurve::new(24.0, 24.0, 29.0, 34.0).get(s.contrast_level)
                } else {
                    98.0
                }
            }))
            .is_background(true)
            .build()
    }

    fn surface_container_lowest(&self) -> DynamicColor {
        DynamicColor::builder("surface_container_lowest", pal(|s| s.neutral_palette.clone()))
            .tone(tn(|s| {
                if s.is_dark {
                    ContrastCurve::new(4.0, 4.0, 2.0, 0.0).get(s.contrast_level)
                } else {
                    100.0
                }
            }))
            .is_background(true)
            .build()
    }

    fn surface_container_low(&self) -> DynamicColor {
        DynamicColor::builder("surface_container_low", pal(|s| s.neutral_palette.clone()))
            .tone(tn(|s| {
                if s.is_dark {
                    ContrastCurve::new(10.0, 10.0, 11.0, 12.0).get(s.contrast_level)
                } else {
                    ContrastCurve::new(96.0, 96.0, 96.0, 95.0).get(s.contrast_level)
                }
            }))
            .is_background(true)
            .build()
    }

    fn surface_container(&self) -> DynamicColor {
        DynamicColor::builder("surface_container", pal(|s| s.neutral_palette.clone()))
            .tone(tn(|s| {
                if s.is_dark {
                    ContrastCurve::new(12.0, 12.0, 16.0, 20.0).get(s.contrast_level)
                } else {
                    ContrastCurve::new(94.0, 94.0, 92.0, 90.0).get(s.contrast_level)
                }
            }))
            .is_background(true)
            .build()
    }

    fn surface_container_high(&self) -> DynamicColor {
        DynamicColor::builder("surface_container_high", pal(|s| s.neutral_palette.clone()))
            .tone(tn(|s| {
                if s.is_dark {
                    ContrastCurve::new(17.0, 17.0, 21.0, 25.0).get(s.contrast_level)
                } else {
                    ContrastCurve::new(92.0, 92.0, 88.0, 85.0).get(s.contrast_level)
                }
            }))
            .is_background(true)
            .build()
    }

    fn surface_container_highest(&self) -> DynamicColor {
        DynamicColor::builder("surface_container_highest", pal(|s| s.neutral_palette.clone()))
            .tone(tn(|s| {
                if s.is_dark {
                    ContrastCurve::new(22.0, 22.0, 26.0, 30.0).get(s.contrast_level)
                } else {
                    ContrastCurve::new(90.0, 90.0, 84.0, 80.0).get(s.contrast_level)
                }
            }))
            .is_background(true)
            .build()
    }

    fn on_surface(&self) -> DynamicColor {
        DynamicColor::builder("on_surface", pal(|s| s.neutral_palette.clone()))
            .tone(tn(|s| if s.is_dark { 90.0 } else { 10.0 }))
            .background(bg(|s| Some(ColorSpec2026.highest_surface(s))))
            .contrast_curve(fixed_curve(ContrastCurve::new(4.5, 7.0, 11.0, 21.0)))
            .build()
    }

    fn surface_variant(&self) -> DynamicColor {
        DynamicColor::builder("surface_variant", pal(|s| s.neutral_variant_palette.clone()))
            .tone(tn(|s| if s.is_dark { 30.0 } else { 90.0 }))
            .is_background(true)
            .build()
    }

    fn on_surface_variant(&self) -> DynamicColor {
        DynamicColor::builder("on_surface_variant", pal(|s| s.neutral_variant_palette.clone()))
            .tone(tn(|s| if s.is_dark { 80.0 } else { 30.0 }))
            .background(bg(|s| Some(ColorSpec2026.highest_surface(s))))
            .contrast_curve(fixed_curve(ContrastCurve::new(3.0, 4.5, 7.0, 11.0)))
            .build()
    }

    fn inverse_surface(&self) -> DynamicColor {
        DynamicColor::builder("inverse_surface", pal(|s| s.neutral_palette.clone()))
            .tone(tn(|s| if s.is_dark { 90.0 } else { 20.0 }))
            .is_background(true)
            .build()
    }

    fn inverse_on_surface(&self) -> DynamicColor {
        DynamicColor::builder("inverse_on_surface", pal(|s| s.neutral_palette.clone()))
            .tone(tn(|s| if s.is_dark { 20.0 } else { 95.0 }))
            .background(fixed_color(ColorSpec2026.inverse_surface()))
            .contrast_curve(fixed_curve(ContrastCurve::new(4.5, 7.0, 11.0, 21.0)))
            .build()
    }

    fn outline(&self) -> DynamicColor {
        DynamicColor::builder("outline", pal(|s| s.neutral_variant_palette.clone()))
            .tone(tn(|s| if s.is_dark { 60.0 } else { 50.0 }))
            .background(bg(|s| Some(ColorSpec2026.highest_surface(s))))
            .contrast_curve(fixed_curve(ContrastCurve::new(1.5, 3.0, 4.5, 7.0)))
            .build()
    }

    fn outline_variant(&self) -> DynamicColor {
        DynamicColor::builder("outline_variant", pal(|s| s.neutral_variant_palette.clone()))
            .tone(tn(|s| if s.is_dark { 30.0 } else { 80.0 }))
            .background(bg(|s| Some(ColorSpec2026.highest_surface(s))))
            .contrast_curve(fixed_curve(ContrastCurve::new(1.0, 1.0, 3.0, 4.5)))
            .build()
    }

    fn shadow(&self) -> DynamicColor {
        DynamicColor::builder("shadow", pal(|s| s.neutral_palette.clone()))
            .tone(tn(|_| 0.0))
            .build()
    }

    fn scrim(&self) -> DynamicColor {
        DynamicColor::builder("scrim", pal(|s| s.neutral_palette.clone())).tone(tn(|_| 0.0)).build()
    }

    fn surface_tint(&self) -> DynamicColor {
        DynamicColor::builder("surface_tint", pal(|s| s.primary_palette.clone()))
            .tone(tn(|s| if s.is_dark { 80.0 } else { 40.0 }))
            .is_background(true)
            .build()
    }

    // ////////////////////////////////////////////////////////////////
    // Primaries [P] //
    // ////////////////////////////////////////////////////////////////
    fn primary(&self) -> DynamicColor {
        DynamicColor::builder("primary", pal(|s| s.primary_palette.clone()))
            .tone(tn(|s| {
                if Self::is_monochrome(s) {
                    if s.is_dark { 100.0 } else { 0.0 }
                } else {
                    if s.is_dark { 80.0 } else { 40.0 }
                }
            }))
            .is_background(true)
            .background(bg(|s| Some(ColorSpec2026.highest_surface(s))))
            .contrast_curve(fixed_curve(ContrastCurve::new(3.0, 4.5, 7.0, 7.0)))
            .tone_delta_pair(tdp(|_| {
                Some(ToneDeltaPair::new(
                    ColorSpec2026.primary_container(),
                    ColorSpec2026.primary(),
                    10.0,
                    TonePolarity::RelativeLighter,
                    false,
                    DeltaConstraint::Nearer,
                ))
            }))
            .build()
    }

    fn primary_dim(&self) -> Option<DynamicColor> {
        None
    }

    fn on_primary(&self) -> DynamicColor {
        DynamicColor::builder("on_primary", pal(|s| s.primary_palette.clone()))
            .tone(tn(|s| {
                if Self::is_monochrome(s) {
                    if s.is_dark { 10.0 } else { 90.0 }
                } else {
                    if s.is_dark { 20.0 } else { 100.0 }
                }
            }))
            .background(fixed_color(ColorSpec2026.primary()))
            .contrast_curve(fixed_curve(ContrastCurve::new(4.5, 7.0, 11.0, 21.0)))
            .build()
    }

    fn primary_container(&self) -> DynamicColor {
        DynamicColor::builder("primary_container", pal(|s| s.primary_palette.clone()))
            .tone(tn(|s| {
                if Self::is_fidelity(s) {
                    s.source_color_hct().tone()
                } else if Self::is_monochrome(s) {
                    if s.is_dark { 85.0 } else { 25.0 }
                } else {
                    if s.is_dark { 30.0 } else { 90.0 }
                }
            }))
            .is_background(true)
            .background(bg(|s| Some(ColorSpec2026.highest_surface(s))))
            .contrast_curve(fixed_curve(ContrastCurve::new(1.0, 1.0, 3.0, 4.5)))
            .tone_delta_pair(tdp(|_| {
                Some(ToneDeltaPair::new(
                    ColorSpec2026.primary_container(),
                    ColorSpec2026.primary(),
                    10.0,
                    TonePolarity::RelativeLighter,
                    false,
                    DeltaConstraint::Nearer,
                ))
            }))
            .build()
    }

    fn on_primary_container(&self) -> DynamicColor {
        DynamicColor::builder("on_primary_container", pal(|s| s.primary_palette.clone()))
            .tone(tn(|s| {
                if Self::is_fidelity(s) {
                    DynamicColor::foreground_tone((ColorSpec2026.primary_container().tone)(s), 4.5)
                } else if Self::is_monochrome(s) {
                    if s.is_dark { 0.0 } else { 100.0 }
                } else {
                    if s.is_dark { 90.0 } else { 30.0 }
                }
            }))
            .background(fixed_color(ColorSpec2026.primary_container()))
            .contrast_curve(fixed_curve(ContrastCurve::new(3.0, 4.5, 7.0, 11.0)))
            .build()
    }

    fn inverse_primary(&self) -> DynamicColor {
        DynamicColor::builder("inverse_primary", pal(|s| s.primary_palette.clone()))
            .tone(tn(|s| if s.is_dark { 40.0 } else { 80.0 }))
            .background(fixed_color(ColorSpec2026.inverse_surface()))
            .contrast_curve(fixed_curve(ContrastCurve::new(3.0, 4.5, 7.0, 7.0)))
            .build()
    }

    // ////////////////////////////////////////////////////////////////
    // Secondaries [Q] //
    // ////////////////////////////////////////////////////////////////
    fn secondary(&self) -> DynamicColor {
        DynamicColor::builder("secondary", pal(|s| s.secondary_palette.clone()))
            .tone(tn(|s| if s.is_dark { 80.0 } else { 40.0 }))
            .is_background(true)
            .background(bg(|s| Some(ColorSpec2026.highest_surface(s))))
            .contrast_curve(fixed_curve(ContrastCurve::new(3.0, 4.5, 7.0, 7.0)))
            .tone_delta_pair(tdp(|_| {
                Some(ToneDeltaPair::new(
                    ColorSpec2026.secondary_container(),
                    ColorSpec2026.secondary(),
                    10.0,
                    TonePolarity::RelativeLighter,
                    false,
                    DeltaConstraint::Nearer,
                ))
            }))
            .build()
    }

    fn secondary_dim(&self) -> Option<DynamicColor> {
        None
    }

    fn on_secondary(&self) -> DynamicColor {
        DynamicColor::builder("on_secondary", pal(|s| s.secondary_palette.clone()))
            .tone(tn(|s| {
                if Self::is_monochrome(s) {
                    if s.is_dark { 10.0 } else { 100.0 }
                } else {
                    if s.is_dark { 20.0 } else { 100.0 }
                }
            }))
            .background(fixed_color(ColorSpec2026.secondary()))
            .contrast_curve(fixed_curve(ContrastCurve::new(4.5, 7.0, 11.0, 21.0)))
            .build()
    }

    fn secondary_container(&self) -> DynamicColor {
        DynamicColor::builder("secondary_container", pal(|s| s.secondary_palette.clone()))
            .tone(tn(|s| {
                let initial_tone = if s.is_dark { 30.0 } else { 90.0 };
                if Self::is_monochrome(s) {
                    if s.is_dark { 30.0 } else { 85.0 }
                } else if !Self::is_fidelity(s) {
                    initial_tone
                } else {
                    Self::find_desired_chroma_by_tone(
                        s.secondary_palette.hue,
                        s.secondary_palette.chroma,
                        initial_tone,
                        !s.is_dark,
                    )
                }
            }))
            .is_background(true)
            .background(bg(|s| Some(ColorSpec2026.highest_surface(s))))
            .contrast_curve(fixed_curve(ContrastCurve::new(1.0, 1.0, 3.0, 4.5)))
            .tone_delta_pair(tdp(|_| {
                Some(ToneDeltaPair::new(
                    ColorSpec2026.secondary_container(),
                    ColorSpec2026.secondary(),
                    10.0,
                    TonePolarity::RelativeLighter,
                    false,
                    DeltaConstraint::Nearer,
                ))
            }))
            .build()
    }

    fn on_secondary_container(&self) -> DynamicColor {
        DynamicColor::builder("on_secondary_container", pal(|s| s.secondary_palette.clone()))
            .tone(tn(|s| {
                if Self::is_monochrome(s) {
                    if s.is_dark { 90.0 } else { 10.0 }
                } else if !Self::is_fidelity(s) {
                    if s.is_dark { 90.0 } else { 30.0 }
                } else {
                    DynamicColor::foreground_tone(
                        (ColorSpec2026.secondary_container().tone)(s),
                        4.5,
                    )
                }
            }))
            .background(fixed_color(ColorSpec2026.secondary_container()))
            .contrast_curve(fixed_curve(ContrastCurve::new(3.0, 4.5, 7.0, 11.0)))
            .build()
    }

    // ////////////////////////////////////////////////////////////////
    // Tertiaries [T] //
    // ////////////////////////////////////////////////////////////////
    fn tertiary(&self) -> DynamicColor {
        DynamicColor::builder("tertiary", pal(|s| s.tertiary_palette.clone()))
            .tone(tn(|s| {
                if Self::is_monochrome(s) {
                    if s.is_dark { 90.0 } else { 25.0 }
                } else {
                    if s.is_dark { 80.0 } else { 40.0 }
                }
            }))
            .is_background(true)
            .background(bg(|s| Some(ColorSpec2026.highest_surface(s))))
            .contrast_curve(fixed_curve(ContrastCurve::new(3.0, 4.5, 7.0, 7.0)))
            .tone_delta_pair(tdp(|_| {
                Some(ToneDeltaPair::new(
                    ColorSpec2026.tertiary_container(),
                    ColorSpec2026.tertiary(),
                    10.0,
                    TonePolarity::RelativeLighter,
                    false,
                    DeltaConstraint::Nearer,
                ))
            }))
            .build()
    }

    fn tertiary_dim(&self) -> Option<DynamicColor> {
        None
    }

    fn on_tertiary(&self) -> DynamicColor {
        DynamicColor::builder("on_tertiary", pal(|s| s.tertiary_palette.clone()))
            .tone(tn(|s| {
                if Self::is_monochrome(s) {
                    if s.is_dark { 10.0 } else { 90.0 }
                } else {
                    if s.is_dark { 20.0 } else { 100.0 }
                }
            }))
            .background(fixed_color(ColorSpec2026.tertiary()))
            .contrast_curve(fixed_curve(ContrastCurve::new(4.5, 7.0, 11.0, 21.0)))
            .build()
    }

    fn tertiary_container(&self) -> DynamicColor {
        DynamicColor::builder("tertiary_container", pal(|s| s.tertiary_palette.clone()))
            .tone(tn(|s| {
                if Self::is_monochrome(s) {
                    if s.is_dark { 60.0 } else { 49.0 }
                } else if !Self::is_fidelity(s) {
                    if s.is_dark { 30.0 } else { 90.0 }
                } else {
                    let proposed_hct = s.tertiary_palette.get_hct(s.source_color_hct().tone());
                    DislikeAnalyzer::fix_if_disliked(proposed_hct).tone()
                }
            }))
            .is_background(true)
            .background(bg(|s| Some(ColorSpec2026.highest_surface(s))))
            .contrast_curve(fixed_curve(ContrastCurve::new(1.0, 1.0, 3.0, 4.5)))
            .tone_delta_pair(tdp(|_| {
                Some(ToneDeltaPair::new(
                    ColorSpec2026.tertiary_container(),
                    ColorSpec2026.tertiary(),
                    10.0,
                    TonePolarity::RelativeLighter,
                    false,
                    DeltaConstraint::Nearer,
                ))
            }))
            .build()
    }

    fn on_tertiary_container(&self) -> DynamicColor {
        DynamicColor::builder("on_tertiary_container", pal(|s| s.tertiary_palette.clone()))
            .tone(tn(|s| {
                if Self::is_monochrome(s) {
                    if s.is_dark { 0.0 } else { 100.0 }
                } else if !Self::is_fidelity(s) {
                    if s.is_dark { 90.0 } else { 30.0 }
                } else {
                    DynamicColor::foreground_tone((ColorSpec2026.tertiary_container().tone)(s), 4.5)
                }
            }))
            .background(fixed_color(ColorSpec2026.tertiary_container()))
            .contrast_curve(fixed_curve(ContrastCurve::new(3.0, 4.5, 7.0, 11.0)))
            .build()
    }

    // ////////////////////////////////////////////////////////////////
    // Errors [E] //
    // ////////////////////////////////////////////////////////////////
    fn error(&self) -> DynamicColor {
        DynamicColor::builder("error", pal(|s| s.error_palette.clone()))
            .tone(tn(|s| if s.is_dark { 80.0 } else { 40.0 }))
            .is_background(true)
            .background(bg(|s| Some(ColorSpec2026.highest_surface(s))))
            .contrast_curve(fixed_curve(ContrastCurve::new(3.0, 4.5, 7.0, 7.0)))
            .tone_delta_pair(tdp(|_| {
                Some(ToneDeltaPair::new(
                    ColorSpec2026.error_container(),
                    ColorSpec2026.error(),
                    10.0,
                    TonePolarity::RelativeLighter,
                    false,
                    DeltaConstraint::Nearer,
                ))
            }))
            .build()
    }

    fn error_dim(&self) -> Option<DynamicColor> {
        None
    }

    fn on_error(&self) -> DynamicColor {
        DynamicColor::builder("on_error", pal(|s| s.error_palette.clone()))
            .tone(tn(|s| if s.is_dark { 20.0 } else { 100.0 }))
            .background(fixed_color(ColorSpec2026.error()))
            .contrast_curve(fixed_curve(ContrastCurve::new(4.5, 7.0, 11.0, 21.0)))
            .build()
    }

    fn error_container(&self) -> DynamicColor {
        DynamicColor::builder("error_container", pal(|s| s.error_palette.clone()))
            .tone(tn(|s| if s.is_dark { 30.0 } else { 90.0 }))
            .is_background(true)
            .background(bg(|s| Some(ColorSpec2026.highest_surface(s))))
            .contrast_curve(fixed_curve(ContrastCurve::new(1.0, 1.0, 3.0, 4.5)))
            .tone_delta_pair(tdp(|_| {
                Some(ToneDeltaPair::new(
                    ColorSpec2026.error_container(),
                    ColorSpec2026.error(),
                    10.0,
                    TonePolarity::RelativeLighter,
                    false,
                    DeltaConstraint::Nearer,
                ))
            }))
            .build()
    }

    fn on_error_container(&self) -> DynamicColor {
        DynamicColor::builder("on_error_container", pal(|s| s.error_palette.clone()))
            .tone(tn(|s| {
                if Self::is_monochrome(s) {
                    if s.is_dark { 90.0 } else { 10.0 }
                } else {
                    if s.is_dark { 90.0 } else { 30.0 }
                }
            }))
            .background(fixed_color(ColorSpec2026.error_container()))
            .contrast_curve(fixed_curve(ContrastCurve::new(3.0, 4.5, 7.0, 11.0)))
            .build()
    }

    // ////////////////////////////////////////////////////////////////
    // Primary Fixed Colors [PF] //
    // ////////////////////////////////////////////////////////////////
    fn primary_fixed(&self) -> DynamicColor {
        DynamicColor::builder("primary_fixed", pal(|s| s.primary_palette.clone()))
            .tone(tn(|s| if Self::is_monochrome(s) { 40.0 } else { 90.0 }))
            .is_background(true)
            .background(bg(|s| Some(ColorSpec2026.highest_surface(s))))
            .contrast_curve(fixed_curve(ContrastCurve::new(1.0, 1.0, 3.0, 4.5)))
            .tone_delta_pair(tdp(|_| {
                Some(ToneDeltaPair::new(
                    ColorSpec2026.primary_fixed(),
                    ColorSpec2026.primary_fixed_dim(),
                    10.0,
                    TonePolarity::Lighter,
                    true,
                    DeltaConstraint::Exact,
                ))
            }))
            .build()
    }

    fn primary_fixed_dim(&self) -> DynamicColor {
        DynamicColor::builder("primary_fixed_dim", pal(|s| s.primary_palette.clone()))
            .tone(tn(|s| if Self::is_monochrome(s) { 30.0 } else { 80.0 }))
            .is_background(true)
            .background(bg(|s| Some(ColorSpec2026.highest_surface(s))))
            .contrast_curve(fixed_curve(ContrastCurve::new(1.0, 1.0, 3.0, 4.5)))
            .tone_delta_pair(tdp(|_| {
                Some(ToneDeltaPair::new(
                    ColorSpec2026.primary_fixed(),
                    ColorSpec2026.primary_fixed_dim(),
                    10.0,
                    TonePolarity::Lighter,
                    true,
                    DeltaConstraint::Exact,
                ))
            }))
            .build()
    }

    fn on_primary_fixed(&self) -> DynamicColor {
        DynamicColor::builder("on_primary_fixed", pal(|s| s.primary_palette.clone()))
            .tone(tn(|s| if Self::is_monochrome(s) { 100.0 } else { 10.0 }))
            .background(fixed_color(ColorSpec2026.primary_fixed_dim()))
            .second_background(fixed_color(ColorSpec2026.primary_fixed()))
            .contrast_curve(fixed_curve(ContrastCurve::new(4.5, 7.0, 11.0, 21.0)))
            .build()
    }

    fn on_primary_fixed_variant(&self) -> DynamicColor {
        DynamicColor::builder("on_primary_fixed_variant", pal(|s| s.primary_palette.clone()))
            .tone(tn(|s| if Self::is_monochrome(s) { 90.0 } else { 30.0 }))
            .background(fixed_color(ColorSpec2026.primary_fixed_dim()))
            .second_background(fixed_color(ColorSpec2026.primary_fixed()))
            .contrast_curve(fixed_curve(ContrastCurve::new(3.0, 4.5, 7.0, 11.0)))
            .build()
    }

    // ////////////////////////////////////////////////////////////////
    // Secondary Fixed Colors [QF] //
    // ////////////////////////////////////////////////////////////////
    fn secondary_fixed(&self) -> DynamicColor {
        DynamicColor::builder("secondary_fixed", pal(|s| s.secondary_palette.clone()))
            .tone(tn(|s| if Self::is_monochrome(s) { 80.0 } else { 90.0 }))
            .is_background(true)
            .background(bg(|s| Some(ColorSpec2026.highest_surface(s))))
            .contrast_curve(fixed_curve(ContrastCurve::new(1.0, 1.0, 3.0, 4.5)))
            .tone_delta_pair(tdp(|_| {
                Some(ToneDeltaPair::new(
                    ColorSpec2026.secondary_fixed(),
                    ColorSpec2026.secondary_fixed_dim(),
                    10.0,
                    TonePolarity::Lighter,
                    true,
                    DeltaConstraint::Exact,
                ))
            }))
            .build()
    }

    fn secondary_fixed_dim(&self) -> DynamicColor {
        DynamicColor::builder("secondary_fixed_dim", pal(|s| s.secondary_palette.clone()))
            .tone(tn(|s| if Self::is_monochrome(s) { 70.0 } else { 80.0 }))
            .is_background(true)
            .background(bg(|s| Some(ColorSpec2026.highest_surface(s))))
            .contrast_curve(fixed_curve(ContrastCurve::new(1.0, 1.0, 3.0, 4.5)))
            .tone_delta_pair(tdp(|_| {
                Some(ToneDeltaPair::new(
                    ColorSpec2026.secondary_fixed(),
                    ColorSpec2026.secondary_fixed_dim(),
                    10.0,
                    TonePolarity::Lighter,
                    true,
                    DeltaConstraint::Exact,
                ))
            }))
            .build()
    }

    fn on_secondary_fixed(&self) -> DynamicColor {
        DynamicColor::builder("on_secondary_fixed", pal(|s| s.secondary_palette.clone()))
            .tone(tn(|_| 10.0))
            .background(fixed_color(ColorSpec2026.secondary_fixed_dim()))
            .second_background(fixed_color(ColorSpec2026.secondary_fixed()))
            .contrast_curve(fixed_curve(ContrastCurve::new(4.5, 7.0, 11.0, 21.0)))
            .build()
    }

    fn on_secondary_fixed_variant(&self) -> DynamicColor {
        DynamicColor::builder("on_secondary_fixed_variant", pal(|s| s.secondary_palette.clone()))
            .tone(tn(|s| if Self::is_monochrome(s) { 25.0 } else { 30.0 }))
            .background(fixed_color(ColorSpec2026.secondary_fixed_dim()))
            .second_background(fixed_color(ColorSpec2026.secondary_fixed()))
            .contrast_curve(fixed_curve(ContrastCurve::new(3.0, 4.5, 7.0, 11.0)))
            .build()
    }

    // ////////////////////////////////////////////////////////////////
    // Tertiary Fixed Colors [TF] //
    // ////////////////////////////////////////////////////////////////
    fn tertiary_fixed(&self) -> DynamicColor {
        DynamicColor::builder("tertiary_fixed", pal(|s| s.tertiary_palette.clone()))
            .tone(tn(|s| if Self::is_monochrome(s) { 40.0 } else { 90.0 }))
            .is_background(true)
            .background(bg(|s| Some(ColorSpec2026.highest_surface(s))))
            .contrast_curve(fixed_curve(ContrastCurve::new(1.0, 1.0, 3.0, 4.5)))
            .tone_delta_pair(tdp(|_| {
                Some(ToneDeltaPair::new(
                    ColorSpec2026.tertiary_fixed(),
                    ColorSpec2026.tertiary_fixed_dim(),
                    10.0,
                    TonePolarity::Lighter,
                    true,
                    DeltaConstraint::Exact,
                ))
            }))
            .build()
    }

    fn tertiary_fixed_dim(&self) -> DynamicColor {
        DynamicColor::builder("tertiary_fixed_dim", pal(|s| s.tertiary_palette.clone()))
            .tone(tn(|s| if Self::is_monochrome(s) { 30.0 } else { 80.0 }))
            .is_background(true)
            .background(bg(|s| Some(ColorSpec2026.highest_surface(s))))
            .contrast_curve(fixed_curve(ContrastCurve::new(1.0, 1.0, 3.0, 4.5)))
            .tone_delta_pair(tdp(|_| {
                Some(ToneDeltaPair::new(
                    ColorSpec2026.tertiary_fixed(),
                    ColorSpec2026.tertiary_fixed_dim(),
                    10.0,
                    TonePolarity::Lighter,
                    true,
                    DeltaConstraint::Exact,
                ))
            }))
            .build()
    }

    fn on_tertiary_fixed(&self) -> DynamicColor {
        DynamicColor::builder("on_tertiary_fixed", pal(|s| s.tertiary_palette.clone()))
            .tone(tn(|s| if Self::is_monochrome(s) { 100.0 } else { 10.0 }))
            .background(fixed_color(ColorSpec2026.tertiary_fixed_dim()))
            .second_background(fixed_color(ColorSpec2026.tertiary_fixed()))
            .contrast_curve(fixed_curve(ContrastCurve::new(4.5, 7.0, 11.0, 21.0)))
            .build()
    }

    fn on_tertiary_fixed_variant(&self) -> DynamicColor {
        DynamicColor::builder("on_tertiary_fixed_variant", pal(|s| s.tertiary_palette.clone()))
            .tone(tn(|s| if Self::is_monochrome(s) { 90.0 } else { 30.0 }))
            .background(fixed_color(ColorSpec2026.tertiary_fixed_dim()))
            .second_background(fixed_color(ColorSpec2026.tertiary_fixed()))
            .contrast_curve(fixed_curve(ContrastCurve::new(3.0, 4.5, 7.0, 11.0)))
            .build()
    }

    // ////////////////////////////////////////////////////////////////
    // Other //
    // ////////////////////////////////////////////////////////////////
    fn highest_surface(&self, scheme: &DynamicScheme) -> DynamicColor {
        if scheme.is_dark { ColorSpec2026.surface_bright() } else { ColorSpec2026.surface_dim() }
    }

    // ////////////////////////////////////////////////////////////////
    // Color value calculations //
    // ////////////////////////////////////////////////////////////////
    fn get_hct(&self, scheme: &DynamicScheme, color: &DynamicColor) -> Hct {
        // This is crucial for aesthetics: we aren't simply taking the standard
        // color and changing its tone for contrast. Rather, we find the tone
        // for contrast, then use the specified chroma from the palette to
        // construct a new color.
        //
        // For example, this enables colors with standard tone of T90, which
        // has limited chroma, to "recover" intended chroma as contrast
        // increases.
        let tone = self.get_tone(scheme, color);
        (color.palette)(scheme).get_hct(tone)
    }

    fn get_tone(&self, scheme: &DynamicScheme, color: &DynamicColor) -> f64 {
        let decreasing_contrast = scheme.contrast_level < 0.0;
        let tone_delta_pair = color.tone_delta_pair.as_ref().and_then(|f| f(scheme));

        // Case 1: dual foreground, pair of colors with delta constraint.
        if let Some(tone_delta_pair) = tone_delta_pair {
            let role_a = tone_delta_pair.role_a.clone();
            let role_b = tone_delta_pair.role_b.clone();
            let delta = tone_delta_pair.delta;
            let polarity = tone_delta_pair.polarity;
            let stay_together = tone_delta_pair.stay_together;
            // The TypeScript reference checks `darker && scheme.isDark`
            // while the Kotlin one has `DARKER && !scheme.isDark`.
            let a_is_nearer = tone_delta_pair.constraint == DeltaConstraint::Nearer
                || (polarity == TonePolarity::Lighter && !scheme.is_dark)
                || (polarity == TonePolarity::Darker && scheme.is_dark);
            let nearer = if a_is_nearer { &role_a } else { &role_b };
            let farther = if a_is_nearer { &role_b } else { &role_a };
            let am_nearer = color.name == nearer.name;
            let expansion_dir = if scheme.is_dark { 1.0 } else { -1.0 };
            let mut n_tone = (nearer.tone)(scheme);
            let mut f_tone = (farther.tone)(scheme);

            // 1st round: solve to min, each
            let background = &color.background;
            let n_contrast_curve = nearer.contrast_curve.as_ref().and_then(|f| f(scheme));
            let f_contrast_curve = farther.contrast_curve.as_ref().and_then(|f| f(scheme));
            if background.is_some()
                && nearer.contrast_curve.is_some()
                && farther.contrast_curve.is_some()
                && n_contrast_curve.is_some()
                && f_contrast_curve.is_some()
            {
                if let Some(bg) = background.as_ref().unwrap()(scheme) {
                    let n_contrast = n_contrast_curve.unwrap().get(scheme.contrast_level);
                    let f_contrast = f_contrast_curve.unwrap().get(scheme.contrast_level);
                    let bg_tone = bg.get_tone(scheme);

                    // If a color is good enough, it is not adjusted.
                    // Initial and adjusted tones for `nearer`
                    if Contrast::ratio_of_tones(bg_tone, n_tone) < n_contrast {
                        n_tone = DynamicColor::foreground_tone(bg_tone, n_contrast);
                    }
                    // Initial and adjusted tones for `farther`
                    if Contrast::ratio_of_tones(bg_tone, f_tone) < f_contrast {
                        f_tone = DynamicColor::foreground_tone(bg_tone, f_contrast);
                    }
                    if decreasing_contrast {
                        // If decreasing contrast, adjust color to the "bare
                        // minimum" that satisfies contrast.
                        n_tone = DynamicColor::foreground_tone(bg_tone, n_contrast);
                        f_tone = DynamicColor::foreground_tone(bg_tone, f_contrast);
                    }
                }
            }

            // If constraint is not satisfied, try another round.
            if (f_tone - n_tone) * expansion_dir < delta {
                // 2nd round: expand farther to match delta.
                f_tone = (n_tone + delta * expansion_dir).clamp(0.0, 100.0);
                // If constraint is not satisfied, try another round.
                if (f_tone - n_tone) * expansion_dir < delta {
                    // 3rd round: contract nearer to match delta.
                    n_tone = (f_tone - delta * expansion_dir).clamp(0.0, 100.0);
                }
            }

            // Avoids the 50-59 awkward zone.
            if (50.0..60.0).contains(&n_tone) {
                // If `nearer` is in the awkward zone, move it away, together
                // with `farther`.
                if expansion_dir > 0.0 {
                    n_tone = 60.0;
                    f_tone = f_tone.max(n_tone + delta * expansion_dir);
                } else {
                    n_tone = 49.0;
                    f_tone = f_tone.min(n_tone + delta * expansion_dir);
                }
            } else if (50.0..60.0).contains(&f_tone) {
                if stay_together {
                    // Fixes both, to avoid two colors on opposite sides of the
                    // "awkward zone".
                    if expansion_dir > 0.0 {
                        n_tone = 60.0;
                        f_tone = f_tone.max(n_tone + delta * expansion_dir);
                    } else {
                        n_tone = 49.0;
                        f_tone = f_tone.min(n_tone + delta * expansion_dir);
                    }
                } else {
                    // Not required to stay together; fixes just one.
                    if expansion_dir > 0.0 {
                        f_tone = 60.0;
                    } else {
                        f_tone = 49.0;
                    }
                }
            }

            // Returns `n_tone` if this color is `nearer`, otherwise `f_tone`.
            return if am_nearer { n_tone } else { f_tone };
        }

        // Case 2: No contrast pair; just solve for itself.
        let mut answer = (color.tone)(scheme);
        let background = color.background.as_ref().and_then(|f| f(scheme));
        let contrast_curve = color.contrast_curve.as_ref().and_then(|f| f(scheme));
        if background.is_none() || contrast_curve.is_none() {
            return answer; // No adjustment for colors with no background.
        }
        let background = background.unwrap();
        let desired_ratio = contrast_curve.unwrap().get(scheme.contrast_level);
        let bg_tone = background.get_tone(scheme);
        if Contrast::ratio_of_tones(bg_tone, answer) >= desired_ratio {
            // Don't "improve" what's good enough.
        } else {
            // Rough improvement.
            answer = DynamicColor::foreground_tone(bg_tone, desired_ratio);
        }
        if decreasing_contrast {
            answer = DynamicColor::foreground_tone(bg_tone, desired_ratio);
        }
        if color.is_background && (50.0..60.0).contains(&answer) {
            // Must adjust
            answer =
                if Contrast::ratio_of_tones(49.0, bg_tone) >= desired_ratio { 49.0 } else { 60.0 };
        }
        let second_background = color.second_background.as_ref().and_then(|f| f(scheme));
        if second_background.is_none() {
            return answer;
        }
        let second_background = second_background.unwrap();

        // Case 3: Adjust for dual backgrounds.
        let bg_tone1 = background.get_tone(scheme);
        let bg_tone2 = second_background.get_tone(scheme);
        let upper = bg_tone1.max(bg_tone2);
        let lower = bg_tone1.min(bg_tone2);
        if Contrast::ratio_of_tones(upper, answer) >= desired_ratio
            && Contrast::ratio_of_tones(lower, answer) >= desired_ratio
        {
            return answer;
        }

        // The darkest light tone that satisfies the desired ratio,
        // or -1 if such ratio cannot be reached.
        let light_option = Contrast::lighter(upper, desired_ratio);

        // The lightest dark tone that satisfies the desired ratio,
        // or -1 if such ratio cannot be reached.
        let dark_option = Contrast::darker(lower, desired_ratio);

        // Tones suitable for the foreground.
        let mut availables = Vec::new();
        if let Some(light_option) = light_option {
            availables.push(light_option);
        }
        if let Some(dark_option) = dark_option {
            availables.push(dark_option);
        }
        let prefers_light = DynamicColor::tone_prefers_light_foreground(bg_tone1)
            || DynamicColor::tone_prefers_light_foreground(bg_tone2);
        if prefers_light {
            return light_option.unwrap_or(100.0);
        }
        if availables.len() == 1 {
            availables[0]
        } else if dark_option.is_none() {
            0.0
        } else {
            dark_option.unwrap()
        }
    }

    // ////////////////////////////////////////////////////////////////
    // Scheme Palettes //
    // ////////////////////////////////////////////////////////////////
    fn get_primary_palette(
        &self,
        variant: Variant,
        source_color_hct: Hct,
        _is_dark: bool,
        _platform: Platform,
        _contrast_level: f64,
    ) -> TonalPalette {
        match variant {
            Variant::Content | Variant::Fidelity => {
                TonalPalette::from_hue_and_chroma(source_color_hct.hue(), source_color_hct.chroma())
            }
            Variant::FruitSalad => TonalPalette::from_hue_and_chroma(
                MathUtils::sanitize_degrees_double(source_color_hct.hue() - 50.0),
                48.0,
            ),
            Variant::Monochrome => TonalPalette::from_hue_and_chroma(source_color_hct.hue(), 0.0),
            Variant::Neutral => TonalPalette::from_hue_and_chroma(source_color_hct.hue(), 12.0),
            Variant::Rainbow => TonalPalette::from_hue_and_chroma(source_color_hct.hue(), 48.0),
            Variant::TonalSpot => TonalPalette::from_hue_and_chroma(source_color_hct.hue(), 36.0),
            Variant::Expressive => TonalPalette::from_hue_and_chroma(
                MathUtils::sanitize_degrees_double(source_color_hct.hue() + 240.0),
                40.0,
            ),
            Variant::Vibrant => TonalPalette::from_hue_and_chroma(source_color_hct.hue(), 200.0),
            _ => panic!("{variant:?} variant is not supported in current spec."),
        }
    }

    fn get_secondary_palette(
        &self,
        variant: Variant,
        source_color_hct: Hct,
        _is_dark: bool,
        _platform: Platform,
        _contrast_level: f64,
    ) -> TonalPalette {
        match variant {
            Variant::Content | Variant::Fidelity => TonalPalette::from_hue_and_chroma(
                source_color_hct.hue(),
                (source_color_hct.chroma() - 32.0).max(source_color_hct.chroma() * 0.5),
            ),
            Variant::FruitSalad => TonalPalette::from_hue_and_chroma(
                MathUtils::sanitize_degrees_double(source_color_hct.hue() - 50.0),
                36.0,
            ),
            Variant::Monochrome => TonalPalette::from_hue_and_chroma(source_color_hct.hue(), 0.0),
            Variant::Neutral => TonalPalette::from_hue_and_chroma(source_color_hct.hue(), 8.0),
            Variant::Rainbow => TonalPalette::from_hue_and_chroma(source_color_hct.hue(), 16.0),
            Variant::TonalSpot => TonalPalette::from_hue_and_chroma(source_color_hct.hue(), 16.0),
            Variant::Expressive => TonalPalette::from_hue_and_chroma(
                DynamicScheme::get_rotated_hue(
                    source_color_hct,
                    &[0.0, 21.0, 51.0, 121.0, 151.0, 191.0, 271.0, 321.0, 360.0],
                    &[45.0, 95.0, 45.0, 20.0, 45.0, 90.0, 45.0, 45.0, 45.0],
                ),
                24.0,
            ),
            Variant::Vibrant => TonalPalette::from_hue_and_chroma(
                DynamicScheme::get_rotated_hue(
                    source_color_hct,
                    &[0.0, 41.0, 61.0, 101.0, 131.0, 181.0, 251.0, 301.0, 360.0],
                    &[18.0, 15.0, 10.0, 12.0, 15.0, 18.0, 15.0, 12.0, 12.0],
                ),
                24.0,
            ),
            _ => panic!("{variant:?} variant is not supported in current spec."),
        }
    }

    fn get_tertiary_palette(
        &self,
        variant: Variant,
        source_color_hct: Hct,
        _is_dark: bool,
        _platform: Platform,
        _contrast_level: f64,
    ) -> TonalPalette {
        match variant {
            Variant::Content => TonalPalette::from_hct(DislikeAnalyzer::fix_if_disliked(
                TemperatureCache::new(source_color_hct).get_analogous_colors(3, 6)[2],
            )),
            Variant::Fidelity => TonalPalette::from_hct(DislikeAnalyzer::fix_if_disliked(
                TemperatureCache::new(source_color_hct).complement(),
            )),
            Variant::FruitSalad => TonalPalette::from_hue_and_chroma(source_color_hct.hue(), 36.0),
            Variant::Monochrome => TonalPalette::from_hue_and_chroma(source_color_hct.hue(), 0.0),
            Variant::Neutral => TonalPalette::from_hue_and_chroma(source_color_hct.hue(), 16.0),
            Variant::Rainbow | Variant::TonalSpot => TonalPalette::from_hue_and_chroma(
                MathUtils::sanitize_degrees_double(source_color_hct.hue() + 60.0),
                24.0,
            ),
            Variant::Expressive => TonalPalette::from_hue_and_chroma(
                DynamicScheme::get_rotated_hue(
                    source_color_hct,
                    &[0.0, 21.0, 51.0, 121.0, 151.0, 191.0, 271.0, 321.0, 360.0],
                    &[120.0, 120.0, 20.0, 45.0, 20.0, 15.0, 20.0, 120.0, 120.0],
                ),
                32.0,
            ),
            Variant::Vibrant => TonalPalette::from_hue_and_chroma(
                DynamicScheme::get_rotated_hue(
                    source_color_hct,
                    &[0.0, 41.0, 61.0, 101.0, 131.0, 181.0, 251.0, 301.0, 360.0],
                    &[35.0, 30.0, 20.0, 25.0, 30.0, 35.0, 30.0, 25.0, 25.0],
                ),
                32.0,
            ),
            _ => panic!("{variant:?} variant is not supported in current spec."),
        }
    }

    fn get_neutral_palette(
        &self,
        variant: Variant,
        source_color_hct: Hct,
        _is_dark: bool,
        _platform: Platform,
        _contrast_level: f64,
    ) -> TonalPalette {
        match variant {
            Variant::Content | Variant::Fidelity => TonalPalette::from_hue_and_chroma(
                source_color_hct.hue(),
                source_color_hct.chroma() / 8.0,
            ),
            Variant::FruitSalad => TonalPalette::from_hue_and_chroma(source_color_hct.hue(), 10.0),
            Variant::Monochrome => TonalPalette::from_hue_and_chroma(source_color_hct.hue(), 0.0),
            Variant::Neutral => TonalPalette::from_hue_and_chroma(source_color_hct.hue(), 2.0),
            Variant::Rainbow => TonalPalette::from_hue_and_chroma(source_color_hct.hue(), 0.0),
            Variant::TonalSpot => TonalPalette::from_hue_and_chroma(source_color_hct.hue(), 6.0),
            Variant::Expressive => TonalPalette::from_hue_and_chroma(
                MathUtils::sanitize_degrees_double(source_color_hct.hue() + 15.0),
                8.0,
            ),
            Variant::Vibrant => TonalPalette::from_hue_and_chroma(source_color_hct.hue(), 10.0),
            _ => panic!("{variant:?} variant is not supported in current spec."),
        }
    }

    fn get_neutral_variant_palette(
        &self,
        variant: Variant,
        source_color_hct: Hct,
        _is_dark: bool,
        _platform: Platform,
        _contrast_level: f64,
    ) -> TonalPalette {
        match variant {
            Variant::Content | Variant::Fidelity => TonalPalette::from_hue_and_chroma(
                source_color_hct.hue(),
                (source_color_hct.chroma() / 8.0) + 4.0,
            ),
            Variant::FruitSalad => TonalPalette::from_hue_and_chroma(source_color_hct.hue(), 16.0),
            Variant::Monochrome => TonalPalette::from_hue_and_chroma(source_color_hct.hue(), 0.0),
            Variant::Neutral => TonalPalette::from_hue_and_chroma(source_color_hct.hue(), 2.0),
            Variant::Rainbow => TonalPalette::from_hue_and_chroma(source_color_hct.hue(), 0.0),
            Variant::TonalSpot => TonalPalette::from_hue_and_chroma(source_color_hct.hue(), 8.0),
            Variant::Expressive => TonalPalette::from_hue_and_chroma(
                MathUtils::sanitize_degrees_double(source_color_hct.hue() + 15.0),
                12.0,
            ),
            Variant::Vibrant => TonalPalette::from_hue_and_chroma(source_color_hct.hue(), 12.0),
            _ => panic!("{variant:?} variant is not supported in current spec."),
        }
    }

    fn get_error_palette(
        &self,
        _variant: Variant,
        _source_color_hct: Hct,
        _is_dark: bool,
        _platform: Platform,
        _contrast_level: f64,
    ) -> TonalPalette {
        TonalPalette::from_hue_and_chroma(25.0, 84.0)
    }
}
