// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT OR Apache-2.0

use alloc::rc::Rc;

use crate::hct::Hct;
use crate::palettes::TonalPalette;

use super::color_spec::{ColorSpec, SpecVersion};
use super::color_spec_2025::ColorSpec2025;
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
fn cc(f: impl Fn(&DynamicScheme) -> Option<ContrastCurve> + 'static) -> ContrastCurveFn {
    Rc::new(f)
}
fn tdp(f: impl Fn(&DynamicScheme) -> Option<ToneDeltaPair> + 'static) -> ToneDeltaPairFn {
    Rc::new(f)
}
fn fixed_color(color: DynamicColor) -> ColorFn {
    Rc::new(move |_| Some(color.clone()))
}
fn cm(f: impl Fn(&DynamicScheme) -> f64 + 'static) -> super::dynamic_color::ChromaMultiplierFn {
    Rc::new(f)
}

/// `ColorSpec` implementation for the 2026 spec.
#[derive(Clone, Copy)]
pub struct ColorSpec2026;

impl ColorSpec2026 {
    fn find_best_tone_for_chroma(
        hue: f64,
        chroma: f64,
        tone: f64,
        by_decreasing_tone: bool,
    ) -> f64 {
        let mut tone = tone;
        let mut answer = tone;
        let mut best_candidate = Hct::from(hue, chroma, answer);
        while best_candidate.chroma() < chroma {
            if tone < 0.0 || tone > 100.0 {
                break;
            }
            tone += if by_decreasing_tone { -1.0 } else { 1.0 };
            let new_candidate = Hct::from(hue, chroma, tone);
            if best_candidate.chroma() < new_candidate.chroma() {
                best_candidate = new_candidate;
                answer = tone;
            }
        }
        answer
    }

    fn t_max_c(
        palette: &TonalPalette,
        lower_bound: f64,
        upper_bound: f64,
        chroma_multiplier: f64,
    ) -> f64 {
        let answer = Self::find_best_tone_for_chroma(
            palette.hue,
            palette.chroma * chroma_multiplier,
            100.0,
            true,
        );
        answer.clamp(lower_bound, upper_bound)
    }

    fn t_min_c(palette: &TonalPalette, lower_bound: f64, upper_bound: f64) -> f64 {
        let answer = Self::find_best_tone_for_chroma(palette.hue, palette.chroma, 0.0, false);
        answer.clamp(lower_bound, upper_bound)
    }

    fn get_contrast_curve(default_contrast: f64) -> ContrastCurve {
        if default_contrast == 1.5 {
            ContrastCurve::new(1.5, 1.5, 3.0, 5.5)
        } else if default_contrast == 3.0 {
            ContrastCurve::new(3.0, 3.0, 4.5, 7.0)
        } else if default_contrast == 4.5 {
            ContrastCurve::new(4.5, 4.5, 7.0, 11.0)
        } else if default_contrast == 6.0 {
            ContrastCurve::new(6.0, 6.0, 7.0, 11.0)
        } else if default_contrast == 7.0 {
            ContrastCurve::new(7.0, 7.0, 11.0, 21.0)
        } else if default_contrast == 9.0 {
            ContrastCurve::new(9.0, 9.0, 11.0, 21.0)
        } else if default_contrast == 11.0 {
            ContrastCurve::new(11.0, 11.0, 21.0, 21.0)
        } else if default_contrast == 21.0 {
            ContrastCurve::new(21.0, 21.0, 21.0, 21.0)
        } else {
            ContrastCurve::new(default_contrast, default_contrast, 7.0, 21.0)
        }
    }

    /// `background = { highestSurface(it) }`
    fn highest_surface_background() -> ColorFn {
        bg(|s| Some(ColorSpec2026.highest_surface(s)))
    }

    /// `contrastCurve = { if (it.contrastLevel > 0) getContrastCurve(1.5) else null }`
    fn phone_contrast_only_curve() -> ContrastCurveFn {
        cc(|s| if s.contrast_level > 0.0 { Some(Self::get_contrast_curve(1.5)) } else { None })
    }
}

impl ColorSpec for ColorSpec2026 {
    // ////////////////////////////////////////////////////////////////
    // Not overridden in 2026 — forward to the 2025 spec //
    // ////////////////////////////////////////////////////////////////
    fn primary_palette_key_color(&self) -> DynamicColor {
        ColorSpec2025.primary_palette_key_color()
    }
    fn secondary_palette_key_color(&self) -> DynamicColor {
        ColorSpec2025.secondary_palette_key_color()
    }
    fn tertiary_palette_key_color(&self) -> DynamicColor {
        ColorSpec2025.tertiary_palette_key_color()
    }
    fn neutral_palette_key_color(&self) -> DynamicColor {
        ColorSpec2025.neutral_palette_key_color()
    }
    fn neutral_variant_palette_key_color(&self) -> DynamicColor {
        ColorSpec2025.neutral_variant_palette_key_color()
    }
    fn error_palette_key_color(&self) -> DynamicColor {
        ColorSpec2025.error_palette_key_color()
    }

    fn background(&self) -> DynamicColor {
        ColorSpec2025.background()
    }
    fn on_background(&self) -> DynamicColor {
        ColorSpec2025.on_background()
    }
    fn surface_variant(&self) -> DynamicColor {
        ColorSpec2025.surface_variant()
    }
    fn shadow(&self) -> DynamicColor {
        ColorSpec2025.shadow()
    }
    fn scrim(&self) -> DynamicColor {
        ColorSpec2025.scrim()
    }
    fn surface_tint(&self) -> DynamicColor {
        ColorSpec2025.surface_tint()
    }
    fn inverse_primary(&self) -> DynamicColor {
        ColorSpec2025.inverse_primary()
    }

    /// Java `ColorSpec2021.highestSurface`, invoked on `this` (2026 object):
    /// `surfaceBright`/`surfaceDim` dispatch to this spec's overrides.
    fn highest_surface(&self, s: &DynamicScheme) -> DynamicColor {
        if s.is_dark { self.surface_bright() } else { self.surface_dim() }
    }

    fn get_hct(&self, scheme: &DynamicScheme, color: &DynamicColor) -> Hct {
        ColorSpec2025.get_hct(scheme, color)
    }
    fn get_tone(&self, scheme: &DynamicScheme, color: &DynamicColor) -> f64 {
        ColorSpec2025.get_tone(scheme, color)
    }

    fn get_primary_palette(
        &self,
        variant: Variant,
        source_color_hct: Hct,
        is_dark: bool,
        platform: Platform,
        contrast_level: f64,
    ) -> TonalPalette {
        ColorSpec2025.get_primary_palette(
            variant,
            source_color_hct,
            is_dark,
            platform,
            contrast_level,
        )
    }
    fn get_secondary_palette(
        &self,
        variant: Variant,
        source_color_hct: Hct,
        is_dark: bool,
        platform: Platform,
        contrast_level: f64,
    ) -> TonalPalette {
        ColorSpec2025.get_secondary_palette(
            variant,
            source_color_hct,
            is_dark,
            platform,
            contrast_level,
        )
    }
    fn get_tertiary_palette(
        &self,
        variant: Variant,
        source_color_hct: Hct,
        is_dark: bool,
        platform: Platform,
        contrast_level: f64,
    ) -> TonalPalette {
        ColorSpec2025.get_tertiary_palette(
            variant,
            source_color_hct,
            is_dark,
            platform,
            contrast_level,
        )
    }
    fn get_neutral_palette(
        &self,
        variant: Variant,
        source_color_hct: Hct,
        is_dark: bool,
        platform: Platform,
        contrast_level: f64,
    ) -> TonalPalette {
        ColorSpec2025.get_neutral_palette(
            variant,
            source_color_hct,
            is_dark,
            platform,
            contrast_level,
        )
    }
    fn get_neutral_variant_palette(
        &self,
        variant: Variant,
        source_color_hct: Hct,
        is_dark: bool,
        platform: Platform,
        contrast_level: f64,
    ) -> TonalPalette {
        ColorSpec2025.get_neutral_variant_palette(
            variant,
            source_color_hct,
            is_dark,
            platform,
            contrast_level,
        )
    }
    fn get_error_palette(
        &self,
        variant: Variant,
        source_color_hct: Hct,
        is_dark: bool,
        platform: Platform,
        contrast_level: f64,
    ) -> TonalPalette {
        ColorSpec2025.get_error_palette(
            variant,
            source_color_hct,
            is_dark,
            platform,
            contrast_level,
        )
    }

    // ////////////////////////////////////////////////////////////////
    // Surfaces [S] //
    // ////////////////////////////////////////////////////////////////
    fn surface(&self) -> DynamicColor {
        let color2026 = DynamicColor::builder("surface", pal(|s| s.neutral_palette.clone()))
            .tone(tn(|s| {
                if s.variant == Variant::Cmf { if s.is_dark { 4.0 } else { 98.0 } } else { 0.0 }
            }))
            .is_background(true)
            .build();
        ColorSpec2025.surface().extend_spec_version(SpecVersion::Spec2026, &color2026)
    }

    fn surface_dim(&self) -> DynamicColor {
        let color2026 = DynamicColor::builder("surface_dim", pal(|s| s.neutral_palette.clone()))
            .tone(tn(|s| {
                if s.variant == Variant::Cmf { if s.is_dark { 4.0 } else { 87.0 } } else { 0.0 }
            }))
            .is_background(true)
            .chroma_multiplier(cm(|s| {
                if s.variant == Variant::Cmf { if s.is_dark { 1.0 } else { 1.7 } } else { 0.0 }
            }))
            .build();
        ColorSpec2025.surface_dim().extend_spec_version(SpecVersion::Spec2026, &color2026)
    }

    fn surface_bright(&self) -> DynamicColor {
        let color2026 = DynamicColor::builder("surface_bright", pal(|s| s.neutral_palette.clone()))
            .tone(tn(|s| {
                if s.variant == Variant::Cmf { if s.is_dark { 18.0 } else { 98.0 } } else { 0.0 }
            }))
            .is_background(true)
            .chroma_multiplier(cm(|s| {
                if s.variant == Variant::Cmf { if s.is_dark { 1.7 } else { 1.0 } } else { 0.0 }
            }))
            .build();
        ColorSpec2025.surface_bright().extend_spec_version(SpecVersion::Spec2026, &color2026)
    }

    fn surface_container_lowest(&self) -> DynamicColor {
        let color2026 =
            DynamicColor::builder("surface_container_lowest", pal(|s| s.neutral_palette.clone()))
                .tone(tn(|s| {
                    if s.variant == Variant::Cmf {
                        if s.is_dark { 0.0 } else { 100.0 }
                    } else {
                        0.0
                    }
                }))
                .is_background(true)
                .build();
        ColorSpec2025
            .surface_container_lowest()
            .extend_spec_version(SpecVersion::Spec2026, &color2026)
    }

    fn surface_container_low(&self) -> DynamicColor {
        let color2026 =
            DynamicColor::builder("surface_container_low", pal(|s| s.neutral_palette.clone()))
                .tone(tn(|s| {
                    if s.variant == Variant::Cmf { if s.is_dark { 6.0 } else { 96.0 } } else { 0.0 }
                }))
                .is_background(true)
                .chroma_multiplier(cm(|s| if s.variant == Variant::Cmf { 1.25 } else { 0.0 }))
                .build();
        ColorSpec2025.surface_container_low().extend_spec_version(SpecVersion::Spec2026, &color2026)
    }

    fn surface_container(&self) -> DynamicColor {
        let color2026 =
            DynamicColor::builder("surface_container", pal(|s| s.neutral_palette.clone()))
                .tone(tn(|s| {
                    if s.variant == Variant::Cmf { if s.is_dark { 9.0 } else { 94.0 } } else { 0.0 }
                }))
                .is_background(true)
                .chroma_multiplier(cm(|s| if s.variant == Variant::Cmf { 1.4 } else { 0.0 }))
                .build();
        ColorSpec2025.surface_container().extend_spec_version(SpecVersion::Spec2026, &color2026)
    }

    fn surface_container_high(&self) -> DynamicColor {
        let color2026 =
            DynamicColor::builder("surface_container_high", pal(|s| s.neutral_palette.clone()))
                .tone(tn(|s| {
                    if s.variant == Variant::Cmf {
                        if s.is_dark { 12.0 } else { 92.0 }
                    } else {
                        0.0
                    }
                }))
                .is_background(true)
                .chroma_multiplier(cm(|s| if s.variant == Variant::Cmf { 1.5 } else { 0.0 }))
                .build();
        ColorSpec2025
            .surface_container_high()
            .extend_spec_version(SpecVersion::Spec2026, &color2026)
    }

    fn surface_container_highest(&self) -> DynamicColor {
        let color2026 =
            DynamicColor::builder("surface_container_highest", pal(|s| s.neutral_palette.clone()))
                .tone(tn(|s| {
                    if s.variant == Variant::Cmf {
                        if s.is_dark { 15.0 } else { 90.0 }
                    } else {
                        0.0
                    }
                }))
                .is_background(true)
                .chroma_multiplier(cm(|s| if s.variant == Variant::Cmf { 1.7 } else { 0.0 }))
                .build();
        ColorSpec2025
            .surface_container_highest()
            .extend_spec_version(SpecVersion::Spec2026, &color2026)
    }

    fn on_surface(&self) -> DynamicColor {
        let color2026 = DynamicColor::builder("on_surface", pal(|s| s.neutral_palette.clone()))
            .chroma_multiplier(cm(|s| if s.variant == Variant::Cmf { 1.7 } else { 0.0 }))
            .background(Self::highest_surface_background())
            .contrast_curve(cc(|s| {
                Some(Self::get_contrast_curve(if s.is_dark { 11.0 } else { 9.0 }))
            }))
            .build();
        ColorSpec2025.on_surface().extend_spec_version(SpecVersion::Spec2026, &color2026)
    }

    fn on_surface_variant(&self) -> DynamicColor {
        let color2026 =
            DynamicColor::builder("on_surface_variant", pal(|s| s.neutral_palette.clone()))
                .chroma_multiplier(cm(|s| if s.variant == Variant::Cmf { 1.7 } else { 0.0 }))
                .background(Self::highest_surface_background())
                .contrast_curve(cc(|s| {
                    Some(Self::get_contrast_curve(if s.is_dark { 6.0 } else { 4.5 }))
                }))
                .build();
        ColorSpec2025.on_surface_variant().extend_spec_version(SpecVersion::Spec2026, &color2026)
    }

    fn outline(&self) -> DynamicColor {
        let color2026 = DynamicColor::builder("outline", pal(|s| s.neutral_palette.clone()))
            .chroma_multiplier(cm(|s| if s.variant == Variant::Cmf { 1.7 } else { 0.0 }))
            .background(Self::highest_surface_background())
            .contrast_curve(cc(|_| Some(Self::get_contrast_curve(3.0))))
            .build();
        ColorSpec2025.outline().extend_spec_version(SpecVersion::Spec2026, &color2026)
    }

    fn outline_variant(&self) -> DynamicColor {
        let color2026 =
            DynamicColor::builder("outline_variant", pal(|s| s.neutral_palette.clone()))
                .chroma_multiplier(cm(|s| if s.variant == Variant::Cmf { 1.7 } else { 0.0 }))
                .background(Self::highest_surface_background())
                .contrast_curve(cc(|_| Some(Self::get_contrast_curve(1.5))))
                .build();
        ColorSpec2025.outline_variant().extend_spec_version(SpecVersion::Spec2026, &color2026)
    }

    fn inverse_surface(&self) -> DynamicColor {
        let color2026 =
            DynamicColor::builder("inverse_surface", pal(|s| s.neutral_palette.clone()))
                .tone(tn(|s| if s.is_dark { 98.0 } else { 4.0 }))
                .is_background(true)
                .chroma_multiplier(cm(|s| if s.variant == Variant::Cmf { 1.7 } else { 0.0 }))
                .build();
        ColorSpec2025.inverse_surface().extend_spec_version(SpecVersion::Spec2026, &color2026)
    }

    fn inverse_on_surface(&self) -> DynamicColor {
        let color2026 =
            DynamicColor::builder("inverse_on_surface", pal(|s| s.neutral_palette.clone()))
                .background(fixed_color(ColorSpec2026.inverse_surface()))
                .contrast_curve(cc(|_| Some(Self::get_contrast_curve(7.0))))
                .build();
        ColorSpec2025.inverse_on_surface().extend_spec_version(SpecVersion::Spec2026, &color2026)
    }

    // ////////////////////////////////////////////////////////////////
    // Primaries [P] //
    // ////////////////////////////////////////////////////////////////
    fn primary(&self) -> DynamicColor {
        let color2026 = DynamicColor::builder("primary", pal(|s| s.primary_palette.clone()))
            .tone(tn(|s| {
                if s.source_color_hct().chroma() <= 12.0 {
                    if s.is_dark { 80.0 } else { 40.0 }
                } else {
                    s.source_color_hct().tone()
                }
            }))
            .is_background(true)
            .background(Self::highest_surface_background())
            .contrast_curve(cc(|_| Some(Self::get_contrast_curve(4.5))))
            .tone_delta_pair(tdp(|s| {
                if s.platform == Platform::Phone {
                    Some(ToneDeltaPair::new(
                        ColorSpec2026.primary_container(),
                        ColorSpec2026.primary(),
                        5.0,
                        TonePolarity::RelativeLighter,
                        true,
                        DeltaConstraint::Farther,
                    ))
                } else {
                    None
                }
            }))
            .build();
        ColorSpec2025.primary().extend_spec_version(SpecVersion::Spec2026, &color2026)
    }

    fn primary_dim(&self) -> Option<DynamicColor> {
        // Remapped to primary in 2026 spec.
        let mut color2026 = self.primary();
        color2026.name = "primary_dim".into();
        Some(
            ColorSpec2025
                .primary_dim()
                .unwrap()
                .extend_spec_version(SpecVersion::Spec2026, &color2026),
        )
    }

    fn on_primary(&self) -> DynamicColor {
        let color2026 = DynamicColor::builder("on_primary", pal(|s| s.primary_palette.clone()))
            .background(fixed_color(ColorSpec2026.primary()))
            .contrast_curve(cc(|_| Some(Self::get_contrast_curve(6.0))))
            .build();
        ColorSpec2025.on_primary().extend_spec_version(SpecVersion::Spec2026, &color2026)
    }

    fn primary_container(&self) -> DynamicColor {
        let color2026 =
            DynamicColor::builder("primary_container", pal(|s| s.primary_palette.clone()))
                .tone(tn(|s| {
                    if !s.is_dark && s.source_color_hct().chroma() <= 12.0 {
                        90.0
                    } else if s.source_color_hct().tone() > 55.0 {
                        s.source_color_hct().tone().clamp(61.0, 90.0)
                    } else {
                        s.source_color_hct().tone().clamp(30.0, 49.0)
                    }
                }))
                .is_background(true)
                .background(Self::highest_surface_background())
                .contrast_curve(Self::phone_contrast_only_curve())
                .build();
        ColorSpec2025.primary_container().extend_spec_version(SpecVersion::Spec2026, &color2026)
    }

    fn on_primary_container(&self) -> DynamicColor {
        let color2026 =
            DynamicColor::builder("on_primary_container", pal(|s| s.primary_palette.clone()))
                .background(fixed_color(ColorSpec2026.primary_container()))
                .contrast_curve(cc(|_| Some(Self::get_contrast_curve(6.0))))
                .build();
        ColorSpec2025.on_primary_container().extend_spec_version(SpecVersion::Spec2026, &color2026)
    }

    fn primary_fixed(&self) -> DynamicColor {
        let color2026 = DynamicColor::builder("primary_fixed", pal(|s| s.primary_palette.clone()))
            .tone(tn(|s| {
                let temp_s = DynamicScheme::from_with_contrast(s, false, 0.0);
                ColorSpec2026.primary_container().get_tone(&temp_s)
            }))
            .is_background(true)
            .background(Self::highest_surface_background())
            .contrast_curve(Self::phone_contrast_only_curve())
            .build();
        ColorSpec2025.primary_fixed().extend_spec_version(SpecVersion::Spec2026, &color2026)
    }

    fn primary_fixed_dim(&self) -> DynamicColor {
        let color2026 =
            DynamicColor::builder("primary_fixed_dim", pal(|s| s.primary_palette.clone()))
                .tone(tn(|s| ColorSpec2026.primary_fixed().get_tone(s)))
                .is_background(true)
                .background(Self::highest_surface_background())
                .tone_delta_pair(tdp(|_| {
                    Some(ToneDeltaPair::new(
                        ColorSpec2026.primary_fixed_dim(),
                        ColorSpec2026.primary_fixed(),
                        5.0,
                        TonePolarity::Darker,
                        true,
                        DeltaConstraint::Exact,
                    ))
                }))
                .contrast_curve(Self::phone_contrast_only_curve())
                .build();
        ColorSpec2025.primary_fixed_dim().extend_spec_version(SpecVersion::Spec2026, &color2026)
    }

    fn on_primary_fixed(&self) -> DynamicColor {
        let color2026 =
            DynamicColor::builder("on_primary_fixed", pal(|s| s.primary_palette.clone()))
                .background(bg(|s| {
                    if ColorSpec2026.primary_fixed().get_tone(s) > 57.0 {
                        Some(ColorSpec2026.primary_fixed_dim())
                    } else {
                        Some(ColorSpec2026.primary_fixed())
                    }
                }))
                .contrast_curve(cc(|_| Some(Self::get_contrast_curve(7.0))))
                .build();
        ColorSpec2025.on_primary_fixed().extend_spec_version(SpecVersion::Spec2026, &color2026)
    }

    fn on_primary_fixed_variant(&self) -> DynamicColor {
        let color2026 =
            DynamicColor::builder("on_primary_fixed_variant", pal(|s| s.primary_palette.clone()))
                .background(bg(|s| {
                    if ColorSpec2026.primary_fixed().get_tone(s) > 57.0 {
                        Some(ColorSpec2026.primary_fixed_dim())
                    } else {
                        Some(ColorSpec2026.primary_fixed())
                    }
                }))
                .contrast_curve(cc(|_| Some(Self::get_contrast_curve(4.5))))
                .build();
        ColorSpec2025
            .on_primary_fixed_variant()
            .extend_spec_version(SpecVersion::Spec2026, &color2026)
    }

    // ////////////////////////////////////////////////////////////////
    // Secondaries [Q] //
    // ////////////////////////////////////////////////////////////////
    fn secondary(&self) -> DynamicColor {
        let color2026 = DynamicColor::builder("secondary", pal(|s| s.secondary_palette.clone()))
            .tone(tn(|s| {
                if s.is_dark {
                    Self::t_min_c(&s.secondary_palette, 0.0, 100.0)
                } else {
                    Self::t_max_c(&s.secondary_palette, 0.0, 100.0, 1.0)
                }
            }))
            .is_background(true)
            .background(Self::highest_surface_background())
            .contrast_curve(cc(|_| Some(Self::get_contrast_curve(4.5))))
            .tone_delta_pair(tdp(|s| {
                if s.platform == Platform::Phone {
                    Some(ToneDeltaPair::new(
                        ColorSpec2026.secondary_container(),
                        ColorSpec2026.secondary(),
                        5.0,
                        TonePolarity::RelativeLighter,
                        true,
                        DeltaConstraint::Farther,
                    ))
                } else {
                    None
                }
            }))
            .build();
        ColorSpec2025.secondary().extend_spec_version(SpecVersion::Spec2026, &color2026)
    }

    fn secondary_dim(&self) -> Option<DynamicColor> {
        // Remapped to secondary in 2026 spec.
        let mut color2026 = self.secondary();
        color2026.name = "secondary_dim".into();
        Some(
            ColorSpec2025
                .secondary_dim()
                .unwrap()
                .extend_spec_version(SpecVersion::Spec2026, &color2026),
        )
    }

    fn on_secondary(&self) -> DynamicColor {
        let color2026 = DynamicColor::builder("on_secondary", pal(|s| s.secondary_palette.clone()))
            .background(fixed_color(ColorSpec2026.secondary()))
            .contrast_curve(cc(|_| Some(Self::get_contrast_curve(6.0))))
            .build();
        ColorSpec2025.on_secondary().extend_spec_version(SpecVersion::Spec2026, &color2026)
    }

    fn secondary_container(&self) -> DynamicColor {
        let color2026 =
            DynamicColor::builder("secondary_container", pal(|s| s.secondary_palette.clone()))
                .tone(tn(|s| {
                    if s.is_dark {
                        Self::t_min_c(&s.secondary_palette, 20.0, 49.0)
                    } else {
                        Self::t_max_c(&s.secondary_palette, 61.0, 90.0, 1.0)
                    }
                }))
                .is_background(true)
                .background(Self::highest_surface_background())
                .contrast_curve(Self::phone_contrast_only_curve())
                .build();
        ColorSpec2025.secondary_container().extend_spec_version(SpecVersion::Spec2026, &color2026)
    }

    fn on_secondary_container(&self) -> DynamicColor {
        let color2026 =
            DynamicColor::builder("on_secondary_container", pal(|s| s.secondary_palette.clone()))
                .background(fixed_color(ColorSpec2026.secondary_container()))
                .contrast_curve(cc(|_| Some(Self::get_contrast_curve(6.0))))
                .build();
        ColorSpec2025
            .on_secondary_container()
            .extend_spec_version(SpecVersion::Spec2026, &color2026)
    }

    fn secondary_fixed(&self) -> DynamicColor {
        let color2026 =
            DynamicColor::builder("secondary_fixed", pal(|s| s.secondary_palette.clone()))
                .tone(tn(|s| {
                    let temp_s = DynamicScheme::from_with_contrast(s, false, 0.0);
                    ColorSpec2026.secondary_container().get_tone(&temp_s)
                }))
                .is_background(true)
                .background(Self::highest_surface_background())
                .contrast_curve(Self::phone_contrast_only_curve())
                .build();
        ColorSpec2025.secondary_fixed().extend_spec_version(SpecVersion::Spec2026, &color2026)
    }

    fn secondary_fixed_dim(&self) -> DynamicColor {
        let color2026 =
            DynamicColor::builder("secondary_fixed_dim", pal(|s| s.secondary_palette.clone()))
                .tone(tn(|s| ColorSpec2026.secondary_fixed().get_tone(s)))
                .is_background(true)
                .background(Self::highest_surface_background())
                .tone_delta_pair(tdp(|_| {
                    Some(ToneDeltaPair::new(
                        ColorSpec2026.secondary_fixed_dim(),
                        ColorSpec2026.secondary_fixed(),
                        5.0,
                        TonePolarity::Darker,
                        true,
                        DeltaConstraint::Exact,
                    ))
                }))
                .contrast_curve(Self::phone_contrast_only_curve())
                .build();
        ColorSpec2025.secondary_fixed_dim().extend_spec_version(SpecVersion::Spec2026, &color2026)
    }

    fn on_secondary_fixed(&self) -> DynamicColor {
        let color2026 =
            DynamicColor::builder("on_secondary_fixed", pal(|s| s.secondary_palette.clone()))
                .background(bg(|s| {
                    if ColorSpec2026.secondary_fixed().get_tone(s) > 57.0 {
                        Some(ColorSpec2026.secondary_fixed_dim())
                    } else {
                        Some(ColorSpec2026.secondary_fixed())
                    }
                }))
                .contrast_curve(cc(|_| Some(Self::get_contrast_curve(7.0))))
                .build();
        ColorSpec2025.on_secondary_fixed().extend_spec_version(SpecVersion::Spec2026, &color2026)
    }

    fn on_secondary_fixed_variant(&self) -> DynamicColor {
        let color2026 = DynamicColor::builder(
            "on_secondary_fixed_variant",
            pal(|s| s.secondary_palette.clone()),
        )
        .background(bg(|s| {
            if ColorSpec2026.secondary_fixed().get_tone(s) > 57.0 {
                Some(ColorSpec2026.secondary_fixed_dim())
            } else {
                Some(ColorSpec2026.secondary_fixed())
            }
        }))
        .contrast_curve(cc(|_| Some(Self::get_contrast_curve(4.5))))
        .build();
        ColorSpec2025
            .on_secondary_fixed_variant()
            .extend_spec_version(SpecVersion::Spec2026, &color2026)
    }

    // ////////////////////////////////////////////////////////////////
    // Tertiaries [T] //
    // ////////////////////////////////////////////////////////////////
    fn tertiary(&self) -> DynamicColor {
        let color2026 = DynamicColor::builder("tertiary", pal(|s| s.tertiary_palette.clone()))
            .tone(tn(|s| {
                s.source_color_hct_list
                    .get(1)
                    .map(|h| h.tone())
                    .unwrap_or_else(|| s.source_color_hct().tone())
            }))
            .is_background(true)
            .background(Self::highest_surface_background())
            .contrast_curve(cc(|_| Some(Self::get_contrast_curve(4.5))))
            .tone_delta_pair(tdp(|s| {
                if s.platform == Platform::Phone {
                    Some(ToneDeltaPair::new(
                        ColorSpec2026.tertiary_container(),
                        ColorSpec2026.tertiary(),
                        5.0,
                        TonePolarity::RelativeLighter,
                        true,
                        DeltaConstraint::Farther,
                    ))
                } else {
                    None
                }
            }))
            .build();
        ColorSpec2025.tertiary().extend_spec_version(SpecVersion::Spec2026, &color2026)
    }

    fn tertiary_dim(&self) -> Option<DynamicColor> {
        // Remapped to tertiary in 2026 spec.
        let mut color2026 = self.tertiary();
        color2026.name = "tertiary_dim".into();
        Some(
            ColorSpec2025
                .tertiary_dim()
                .unwrap()
                .extend_spec_version(SpecVersion::Spec2026, &color2026),
        )
    }

    fn on_tertiary(&self) -> DynamicColor {
        let color2026 = DynamicColor::builder("on_tertiary", pal(|s| s.tertiary_palette.clone()))
            .background(fixed_color(ColorSpec2026.tertiary()))
            .contrast_curve(cc(|_| Some(Self::get_contrast_curve(6.0))))
            .build();
        ColorSpec2025.on_tertiary().extend_spec_version(SpecVersion::Spec2026, &color2026)
    }

    fn tertiary_container(&self) -> DynamicColor {
        let color2026 =
            DynamicColor::builder("tertiary_container", pal(|s| s.tertiary_palette.clone()))
                .tone(tn(|s| {
                    let secondary_source_color_hct = s
                        .source_color_hct_list
                        .get(1)
                        .copied()
                        .unwrap_or_else(|| s.source_color_hct());
                    if secondary_source_color_hct.tone() > 55.0 {
                        secondary_source_color_hct.tone().clamp(61.0, 90.0)
                    } else {
                        secondary_source_color_hct.tone().clamp(20.0, 49.0)
                    }
                }))
                .is_background(true)
                .background(Self::highest_surface_background())
                .contrast_curve(Self::phone_contrast_only_curve())
                .build();
        ColorSpec2025.tertiary_container().extend_spec_version(SpecVersion::Spec2026, &color2026)
    }

    fn on_tertiary_container(&self) -> DynamicColor {
        let color2026 =
            DynamicColor::builder("on_tertiary_container", pal(|s| s.tertiary_palette.clone()))
                .background(fixed_color(ColorSpec2026.tertiary_container()))
                .contrast_curve(cc(|_| Some(Self::get_contrast_curve(6.0))))
                .build();
        ColorSpec2025.on_tertiary_container().extend_spec_version(SpecVersion::Spec2026, &color2026)
    }

    fn tertiary_fixed(&self) -> DynamicColor {
        let color2026 =
            DynamicColor::builder("tertiary_fixed", pal(|s| s.tertiary_palette.clone()))
                .tone(tn(|s| {
                    let temp_s = DynamicScheme::from_with_contrast(s, false, 0.0);
                    ColorSpec2026.tertiary_container().get_tone(&temp_s)
                }))
                .is_background(true)
                .background(Self::highest_surface_background())
                .contrast_curve(Self::phone_contrast_only_curve())
                .build();
        ColorSpec2025.tertiary_fixed().extend_spec_version(SpecVersion::Spec2026, &color2026)
    }

    fn tertiary_fixed_dim(&self) -> DynamicColor {
        let color2026 =
            DynamicColor::builder("tertiary_fixed_dim", pal(|s| s.tertiary_palette.clone()))
                .tone(tn(|s| ColorSpec2026.tertiary_fixed().get_tone(s)))
                .is_background(true)
                .background(Self::highest_surface_background())
                .tone_delta_pair(tdp(|_| {
                    Some(ToneDeltaPair::new(
                        ColorSpec2026.tertiary_fixed_dim(),
                        ColorSpec2026.tertiary_fixed(),
                        5.0,
                        TonePolarity::Darker,
                        true,
                        DeltaConstraint::Exact,
                    ))
                }))
                .contrast_curve(Self::phone_contrast_only_curve())
                .build();
        ColorSpec2025.tertiary_fixed_dim().extend_spec_version(SpecVersion::Spec2026, &color2026)
    }

    fn on_tertiary_fixed(&self) -> DynamicColor {
        let color2026 =
            DynamicColor::builder("on_tertiary_fixed", pal(|s| s.tertiary_palette.clone()))
                .background(bg(|s| {
                    if ColorSpec2026.tertiary_fixed().get_tone(s) > 57.0 {
                        Some(ColorSpec2026.tertiary_fixed_dim())
                    } else {
                        Some(ColorSpec2026.tertiary_fixed())
                    }
                }))
                .contrast_curve(cc(|_| Some(Self::get_contrast_curve(7.0))))
                .build();
        ColorSpec2025.on_tertiary_fixed().extend_spec_version(SpecVersion::Spec2026, &color2026)
    }

    fn on_tertiary_fixed_variant(&self) -> DynamicColor {
        let color2026 =
            DynamicColor::builder("on_tertiary_fixed_variant", pal(|s| s.tertiary_palette.clone()))
                .background(bg(|s| {
                    if ColorSpec2026.tertiary_fixed().get_tone(s) > 57.0 {
                        Some(ColorSpec2026.tertiary_fixed_dim())
                    } else {
                        Some(ColorSpec2026.tertiary_fixed())
                    }
                }))
                .contrast_curve(cc(|_| Some(Self::get_contrast_curve(4.5))))
                .build();
        ColorSpec2025
            .on_tertiary_fixed_variant()
            .extend_spec_version(SpecVersion::Spec2026, &color2026)
    }

    // ////////////////////////////////////////////////////////////////
    // Errors [E] //
    // ////////////////////////////////////////////////////////////////
    fn error(&self) -> DynamicColor {
        let color2026 = DynamicColor::builder("error", pal(|s| s.error_palette.clone()))
            .tone(tn(|s| Self::t_max_c(&s.error_palette, 0.0, 100.0, 1.0)))
            .is_background(true)
            .background(Self::highest_surface_background())
            .contrast_curve(cc(|_| Some(Self::get_contrast_curve(4.5))))
            .tone_delta_pair(tdp(|s| {
                if s.platform == Platform::Phone {
                    Some(ToneDeltaPair::new(
                        ColorSpec2026.error_container(),
                        ColorSpec2026.error(),
                        5.0,
                        TonePolarity::RelativeLighter,
                        true,
                        DeltaConstraint::Farther,
                    ))
                } else {
                    None
                }
            }))
            .build();
        ColorSpec2025.error().extend_spec_version(SpecVersion::Spec2026, &color2026)
    }

    fn error_dim(&self) -> Option<DynamicColor> {
        // Remapped to error in 2026 spec.
        let mut color2026 = self.error();
        color2026.name = "error_dim".into();
        Some(
            ColorSpec2025
                .error_dim()
                .unwrap()
                .extend_spec_version(SpecVersion::Spec2026, &color2026),
        )
    }

    fn on_error(&self) -> DynamicColor {
        let color2026 = DynamicColor::builder("on_error", pal(|s| s.error_palette.clone()))
            .background(fixed_color(ColorSpec2026.error()))
            .contrast_curve(cc(|_| Some(Self::get_contrast_curve(6.0))))
            .build();
        ColorSpec2025.on_error().extend_spec_version(SpecVersion::Spec2026, &color2026)
    }

    fn error_container(&self) -> DynamicColor {
        let color2026 = DynamicColor::builder("error_container", pal(|s| s.error_palette.clone()))
            .tone(tn(|s| {
                if s.is_dark {
                    Self::t_min_c(&s.error_palette, 0.0, 100.0)
                } else {
                    Self::t_max_c(&s.error_palette, 0.0, 100.0, 1.0)
                }
            }))
            .is_background(true)
            .background(Self::highest_surface_background())
            .contrast_curve(Self::phone_contrast_only_curve())
            .build();
        ColorSpec2025.error_container().extend_spec_version(SpecVersion::Spec2026, &color2026)
    }

    fn on_error_container(&self) -> DynamicColor {
        let color2026 =
            DynamicColor::builder("on_error_container", pal(|s| s.error_palette.clone()))
                .background(fixed_color(ColorSpec2026.error_container()))
                .contrast_curve(cc(|_| Some(Self::get_contrast_curve(6.0))))
                .build();
        ColorSpec2025.on_error_container().extend_spec_version(SpecVersion::Spec2026, &color2026)
    }
}
