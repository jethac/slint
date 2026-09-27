// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT OR Apache-2.0

use alloc::rc::Rc;
use alloc::vec::Vec;

use crate::contrast::Contrast;
use crate::hct::Hct;
use crate::palettes::TonalPalette;

use super::color_spec::{ColorSpec, SpecVersion};
use super::color_spec_2021::ColorSpec2021;
use super::color_spec_2026::ColorSpec2026;
use super::contrast_curve::ContrastCurve;
use super::dynamic_color::{
    ChromaMultiplierFn, ColorFn, ContrastCurveFn, DynamicColor, PaletteFn, ToneDeltaPairFn, ToneFn,
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
fn cm(f: impl Fn(&DynamicScheme) -> f64 + 'static) -> ChromaMultiplierFn {
    Rc::new(f)
}
fn fixed_color(color: DynamicColor) -> ColorFn {
    Rc::new(move |_| Some(color.clone()))
}

/// `ColorSpec` implementation for the 2025 spec.
#[derive(Clone, Copy)]
pub struct ColorSpec2025;

impl ColorSpec2025 {
    /// The 2025 surface container / dim-bright background used by many 2025
    /// roles: `surfaceDim`/`surfaceBright` on phones, `surfaceContainerHigh`
    /// on watches.
    fn default_background() -> ColorFn {
        bg(|s| {
            if s.platform == Platform::Phone {
                if s.is_dark {
                    Some(ColorSpec2026.surface_bright())
                } else {
                    Some(ColorSpec2026.surface_dim())
                }
            } else {
                Some(ColorSpec2026.surface_container_high())
            }
        })
    }

    /// `background = { phone ? (dark ? surfaceBright : surfaceDim) : null }`.
    fn phone_or_null_background() -> ColorFn {
        bg(|s| {
            if s.platform == Platform::Phone {
                if s.is_dark {
                    Some(ColorSpec2026.surface_bright())
                } else {
                    Some(ColorSpec2026.surface_dim())
                }
            } else {
                None
            }
        })
    }

    /// `if (isDark == dark) variantTable else 1.0` — the neutral-palette chroma
    /// multiplier table used by `surfaceDim`/`surfaceBright` (applied on
    /// opposite modes).
    fn surface_chroma_multiplier(dark: bool) -> ChromaMultiplierFn {
        cm(move |s| {
            if s.is_dark == dark {
                match s.variant {
                    Variant::Neutral => 2.5,
                    Variant::TonalSpot => 1.7,
                    Variant::Expressive => {
                        if Hct::is_yellow(s.neutral_palette.hue) {
                            2.7
                        } else {
                            1.75
                        }
                    }
                    Variant::Vibrant => 1.36,
                    _ => 1.0,
                }
            } else {
                1.0
            }
        })
    }

    /// The neutral-palette chroma multiplier table shared by `onSurface`,
    /// `onSurfaceVariant`, `outline` and `outlineVariant` on phones.
    fn neutral_chroma_multiplier() -> ChromaMultiplierFn {
        cm(|s| {
            if s.platform == Platform::Phone {
                match s.variant {
                    Variant::Neutral => 2.2,
                    Variant::TonalSpot => 1.7,
                    Variant::Expressive => {
                        if Hct::is_yellow(s.neutral_palette.hue) {
                            if s.is_dark { 3.0 } else { 2.3 }
                        } else {
                            1.6
                        }
                    }
                    _ => 1.0,
                }
            } else {
                1.0
            }
        })
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

    fn get_expressive_neutral_hue(source_color_hct: Hct) -> f64 {
        DynamicScheme::get_rotated_hue(
            source_color_hct,
            &[0.0, 71.0, 124.0, 253.0, 278.0, 300.0, 360.0],
            &[10.0, 0.0, 10.0, 0.0, 10.0, 0.0],
        )
    }

    fn get_expressive_neutral_chroma(
        source_color_hct: Hct,
        is_dark: bool,
        platform: Platform,
    ) -> f64 {
        let neutral_hue = Self::get_expressive_neutral_hue(source_color_hct);
        if platform == Platform::Phone {
            if is_dark { if Hct::is_yellow(neutral_hue) { 6.0 } else { 14.0 } } else { 18.0 }
        } else {
            12.0
        }
    }

    fn get_vibrant_neutral_hue(source_color_hct: Hct) -> f64 {
        DynamicScheme::get_rotated_hue(
            source_color_hct,
            &[0.0, 38.0, 105.0, 140.0, 333.0, 360.0],
            &[-14.0, 10.0, -14.0, 10.0, -14.0],
        )
    }

    fn get_vibrant_neutral_chroma(source_color_hct: Hct, platform: Platform) -> f64 {
        let neutral_hue = Self::get_vibrant_neutral_hue(source_color_hct);
        if platform == Platform::Phone {
            28.0
        } else if Hct::is_blue(neutral_hue) {
            28.0
        } else {
            20.0
        }
    }
}

impl ColorSpec for ColorSpec2025 {
    // ////////////////////////////////////////////////////////////////
    // Main Palettes — not overridden in 2025 //
    // ////////////////////////////////////////////////////////////////
    fn primary_palette_key_color(&self) -> DynamicColor {
        ColorSpec2021.primary_palette_key_color()
    }
    fn secondary_palette_key_color(&self) -> DynamicColor {
        ColorSpec2021.secondary_palette_key_color()
    }
    fn tertiary_palette_key_color(&self) -> DynamicColor {
        ColorSpec2021.tertiary_palette_key_color()
    }
    fn neutral_palette_key_color(&self) -> DynamicColor {
        ColorSpec2021.neutral_palette_key_color()
    }
    fn neutral_variant_palette_key_color(&self) -> DynamicColor {
        ColorSpec2021.neutral_variant_palette_key_color()
    }
    fn error_palette_key_color(&self) -> DynamicColor {
        ColorSpec2021.error_palette_key_color()
    }

    fn shadow(&self) -> DynamicColor {
        ColorSpec2021.shadow()
    }
    fn scrim(&self) -> DynamicColor {
        ColorSpec2021.scrim()
    }

    /// Kotlin `ColorSpec2021.highestSurface`, invoked on `this` (2025 object):
    /// `surfaceBright`/`surfaceDim` dispatch to this spec's overrides.
    fn highest_surface(&self, s: &DynamicScheme) -> DynamicColor {
        if s.is_dark { ColorSpec2026.surface_bright() } else { ColorSpec2026.surface_dim() }
    }

    // ////////////////////////////////////////////////////////////////
    // Surfaces [S] //
    // ////////////////////////////////////////////////////////////////
    fn background(&self) -> DynamicColor {
        // Remapped to surface for 2025 spec.
        let mut color2025 = ColorSpec2026.surface();
        color2025.name = "background".into();
        ColorSpec2021.background().extend_spec_version(SpecVersion::Spec2025, &color2025)
    }

    fn on_background(&self) -> DynamicColor {
        // Remapped to onSurface for 2025 spec.
        let on_surface = ColorSpec2026.on_surface();
        let mut color2025 = on_surface.clone();
        color2025.name = "on_background".into();
        color2025.tone =
            tn(move |s| if s.platform == Platform::Watch { 100.0 } else { on_surface.get_tone(s) });
        ColorSpec2021.on_background().extend_spec_version(SpecVersion::Spec2025, &color2025)
    }

    fn surface(&self) -> DynamicColor {
        let color2025 = DynamicColor::builder("surface", pal(|s| s.neutral_palette.clone()))
            .tone(tn(|s| {
                if s.platform == Platform::Phone {
                    if s.is_dark {
                        4.0
                    } else if Hct::is_yellow(s.neutral_palette.hue) {
                        99.0
                    } else if s.variant == Variant::Vibrant {
                        97.0
                    } else {
                        98.0
                    }
                } else {
                    0.0
                }
            }))
            .is_background(true)
            .build();
        ColorSpec2021.surface().extend_spec_version(SpecVersion::Spec2025, &color2025)
    }

    fn surface_dim(&self) -> DynamicColor {
        let color2025 = DynamicColor::builder("surface_dim", pal(|s| s.neutral_palette.clone()))
            .tone(tn(|s| {
                if s.is_dark {
                    4.0
                } else if Hct::is_yellow(s.neutral_palette.hue) {
                    90.0
                } else if s.variant == Variant::Vibrant {
                    85.0
                } else {
                    87.0
                }
            }))
            .is_background(true)
            .chroma_multiplier(Self::surface_chroma_multiplier(false))
            .build();
        ColorSpec2021.surface_dim().extend_spec_version(SpecVersion::Spec2025, &color2025)
    }

    fn surface_bright(&self) -> DynamicColor {
        let color2025 = DynamicColor::builder("surface_bright", pal(|s| s.neutral_palette.clone()))
            .tone(tn(|s| {
                if s.is_dark {
                    18.0
                } else if Hct::is_yellow(s.neutral_palette.hue) {
                    99.0
                } else if s.variant == Variant::Vibrant {
                    97.0
                } else {
                    98.0
                }
            }))
            .is_background(true)
            .chroma_multiplier(Self::surface_chroma_multiplier(true))
            .build();
        ColorSpec2021.surface_bright().extend_spec_version(SpecVersion::Spec2025, &color2025)
    }

    fn surface_container_lowest(&self) -> DynamicColor {
        let color2025 =
            DynamicColor::builder("surface_container_lowest", pal(|s| s.neutral_palette.clone()))
                .tone(tn(|s| if s.is_dark { 0.0 } else { 100.0 }))
                .is_background(true)
                .build();
        ColorSpec2021
            .surface_container_lowest()
            .extend_spec_version(SpecVersion::Spec2025, &color2025)
    }

    fn surface_container_low(&self) -> DynamicColor {
        let color2025 =
            DynamicColor::builder("surface_container_low", pal(|s| s.neutral_palette.clone()))
                .tone(tn(|s| {
                    if s.platform == Platform::Phone {
                        if s.is_dark {
                            6.0
                        } else if Hct::is_yellow(s.neutral_palette.hue) {
                            98.0
                        } else if s.variant == Variant::Vibrant {
                            95.0
                        } else {
                            96.0
                        }
                    } else {
                        15.0
                    }
                }))
                .is_background(true)
                .chroma_multiplier(cm(|s| {
                    if s.platform == Platform::Phone {
                        match s.variant {
                            Variant::Neutral => 1.3,
                            Variant::TonalSpot => 1.25,
                            Variant::Expressive => {
                                if Hct::is_yellow(s.neutral_palette.hue) {
                                    1.3
                                } else {
                                    1.15
                                }
                            }
                            Variant::Vibrant => 1.08,
                            _ => 1.0,
                        }
                    } else {
                        1.0
                    }
                }))
                .build();
        ColorSpec2021.surface_container_low().extend_spec_version(SpecVersion::Spec2025, &color2025)
    }

    fn surface_container(&self) -> DynamicColor {
        let color2025 =
            DynamicColor::builder("surface_container", pal(|s| s.neutral_palette.clone()))
                .tone(tn(|s| {
                    if s.platform == Platform::Phone {
                        if s.is_dark {
                            9.0
                        } else if Hct::is_yellow(s.neutral_palette.hue) {
                            96.0
                        } else if s.variant == Variant::Vibrant {
                            92.0
                        } else {
                            94.0
                        }
                    } else {
                        20.0
                    }
                }))
                .is_background(true)
                .chroma_multiplier(cm(|s| {
                    if s.platform == Platform::Phone {
                        match s.variant {
                            Variant::Neutral => 1.6,
                            Variant::TonalSpot => 1.4,
                            Variant::Expressive => {
                                if Hct::is_yellow(s.neutral_palette.hue) {
                                    1.6
                                } else {
                                    1.3
                                }
                            }
                            Variant::Vibrant => 1.15,
                            _ => 1.0,
                        }
                    } else {
                        1.0
                    }
                }))
                .build();
        ColorSpec2021.surface_container().extend_spec_version(SpecVersion::Spec2025, &color2025)
    }

    fn surface_container_high(&self) -> DynamicColor {
        let color2025 =
            DynamicColor::builder("surface_container_high", pal(|s| s.neutral_palette.clone()))
                .tone(tn(|s| {
                    if s.platform == Platform::Phone {
                        if s.is_dark {
                            12.0
                        } else if Hct::is_yellow(s.neutral_palette.hue) {
                            94.0
                        } else if s.variant == Variant::Vibrant {
                            90.0
                        } else {
                            92.0
                        }
                    } else {
                        25.0
                    }
                }))
                .is_background(true)
                .chroma_multiplier(cm(|s| {
                    if s.platform == Platform::Phone {
                        match s.variant {
                            Variant::Neutral => 1.9,
                            Variant::TonalSpot => 1.5,
                            Variant::Expressive => {
                                if Hct::is_yellow(s.neutral_palette.hue) {
                                    1.95
                                } else {
                                    1.45
                                }
                            }
                            Variant::Vibrant => 1.22,
                            _ => 1.0,
                        }
                    } else {
                        1.0
                    }
                }))
                .build();
        ColorSpec2021
            .surface_container_high()
            .extend_spec_version(SpecVersion::Spec2025, &color2025)
    }

    fn surface_container_highest(&self) -> DynamicColor {
        let color2025 =
            DynamicColor::builder("surface_container_highest", pal(|s| s.neutral_palette.clone()))
                .tone(tn(|s| {
                    if s.is_dark {
                        15.0
                    } else if Hct::is_yellow(s.neutral_palette.hue) {
                        92.0
                    } else if s.variant == Variant::Vibrant {
                        88.0
                    } else {
                        90.0
                    }
                }))
                .is_background(true)
                .chroma_multiplier(cm(|s| match s.variant {
                    Variant::Neutral => 2.2,
                    Variant::TonalSpot => 1.7,
                    Variant::Expressive => {
                        if Hct::is_yellow(s.neutral_palette.hue) {
                            2.3
                        } else {
                            1.6
                        }
                    }
                    Variant::Vibrant => 1.29,
                    _ => 1.0,
                }))
                .build();
        ColorSpec2021
            .surface_container_highest()
            .extend_spec_version(SpecVersion::Spec2025, &color2025)
    }

    fn on_surface(&self) -> DynamicColor {
        let background = Self::default_background();
        let color2025 = DynamicColor::builder("on_surface", pal(|s| s.neutral_palette.clone()))
            .tone(tn(move |s| {
                if s.variant == Variant::Vibrant {
                    Self::t_max_c(&s.neutral_palette, 0.0, 100.0, 1.1)
                } else {
                    DynamicColor::initial_tone_from_background(Some(background.clone()))(s)
                }
            }))
            .chroma_multiplier(Self::neutral_chroma_multiplier())
            .background(Self::default_background())
            .contrast_curve(cc(|s| {
                if s.is_dark && s.platform == Platform::Phone {
                    Some(Self::get_contrast_curve(11.0))
                } else {
                    Some(Self::get_contrast_curve(9.0))
                }
            }))
            .build();
        ColorSpec2021.on_surface().extend_spec_version(SpecVersion::Spec2025, &color2025)
    }

    fn surface_variant(&self) -> DynamicColor {
        // Remapped to surfaceContainerHighest for 2025 spec.
        let mut color2025 = ColorSpec2026.surface_container_highest();
        color2025.name = "surface_variant".into();
        ColorSpec2021.surface_variant().extend_spec_version(SpecVersion::Spec2025, &color2025)
    }

    fn on_surface_variant(&self) -> DynamicColor {
        let color2025 =
            DynamicColor::builder("on_surface_variant", pal(|s| s.neutral_palette.clone()))
                .chroma_multiplier(Self::neutral_chroma_multiplier())
                .background(Self::default_background())
                .contrast_curve(cc(|s| {
                    if s.platform == Platform::Phone {
                        if s.is_dark {
                            Some(Self::get_contrast_curve(6.0))
                        } else {
                            Some(Self::get_contrast_curve(4.5))
                        }
                    } else {
                        Some(Self::get_contrast_curve(7.0))
                    }
                }))
                .build();
        ColorSpec2021.on_surface_variant().extend_spec_version(SpecVersion::Spec2025, &color2025)
    }

    fn inverse_surface(&self) -> DynamicColor {
        let color2025 =
            DynamicColor::builder("inverse_surface", pal(|s| s.neutral_palette.clone()))
                .tone(tn(|s| if s.is_dark { 98.0 } else { 4.0 }))
                .is_background(true)
                .build();
        ColorSpec2021.inverse_surface().extend_spec_version(SpecVersion::Spec2025, &color2025)
    }

    fn inverse_on_surface(&self) -> DynamicColor {
        let color2025 =
            DynamicColor::builder("inverse_on_surface", pal(|s| s.neutral_palette.clone()))
                .background(fixed_color(ColorSpec2026.inverse_surface()))
                .contrast_curve(cc(|_| Some(Self::get_contrast_curve(7.0))))
                .build();
        ColorSpec2021.inverse_on_surface().extend_spec_version(SpecVersion::Spec2025, &color2025)
    }

    fn outline(&self) -> DynamicColor {
        let color2025 = DynamicColor::builder("outline", pal(|s| s.neutral_palette.clone()))
            .chroma_multiplier(Self::neutral_chroma_multiplier())
            .background(Self::default_background())
            .contrast_curve(cc(|s| {
                if s.platform == Platform::Phone {
                    Some(Self::get_contrast_curve(3.0))
                } else {
                    Some(Self::get_contrast_curve(4.5))
                }
            }))
            .build();
        ColorSpec2021.outline().extend_spec_version(SpecVersion::Spec2025, &color2025)
    }

    fn outline_variant(&self) -> DynamicColor {
        let color2025 =
            DynamicColor::builder("outline_variant", pal(|s| s.neutral_palette.clone()))
                .chroma_multiplier(Self::neutral_chroma_multiplier())
                .background(Self::default_background())
                .contrast_curve(cc(|s| {
                    if s.platform == Platform::Phone {
                        Some(Self::get_contrast_curve(1.5))
                    } else {
                        Some(Self::get_contrast_curve(3.0))
                    }
                }))
                .build();
        ColorSpec2021.outline_variant().extend_spec_version(SpecVersion::Spec2025, &color2025)
    }

    fn surface_tint(&self) -> DynamicColor {
        // Remapped to primary for 2025 spec.
        let mut color2025 = ColorSpec2026.primary();
        color2025.name = "surface_tint".into();
        ColorSpec2021.surface_tint().extend_spec_version(SpecVersion::Spec2025, &color2025)
    }

    // ////////////////////////////////////////////////////////////////
    // Primaries [P] //
    // ////////////////////////////////////////////////////////////////
    fn primary(&self) -> DynamicColor {
        let color2025 = DynamicColor::builder("primary", pal(|s| s.primary_palette.clone()))
            .tone(tn(|s| {
                match s.variant {
                    Variant::Neutral => {
                        if s.platform == Platform::Phone {
                            if s.is_dark { 80.0 } else { 40.0 }
                        } else {
                            90.0
                        }
                    }
                    Variant::TonalSpot => {
                        if s.platform == Platform::Phone {
                            if s.is_dark {
                                80.0
                            } else {
                                Self::t_max_c(&s.primary_palette, 0.0, 100.0, 1.0)
                            }
                        } else {
                            Self::t_max_c(&s.primary_palette, 0.0, 90.0, 1.0)
                        }
                    }
                    Variant::Expressive => {
                        if s.platform == Platform::Phone {
                            Self::t_max_c(
                                &s.primary_palette,
                                0.0,
                                if s.is_dark {
                                    if Hct::is_cyan(s.primary_palette.hue) { 88.0 } else { 98.0 }
                                } else {
                                    if Hct::is_yellow(s.primary_palette.hue) { 25.0 } else { 98.0 }
                                },
                                1.0,
                            )
                        } else {
                            // WATCH
                            Self::t_max_c(&s.primary_palette, 0.0, 100.0, 1.0)
                        }
                    }
                    _ => {
                        // VIBRANT
                        if s.platform == Platform::Phone {
                            Self::t_max_c(
                                &s.primary_palette,
                                0.0,
                                if Hct::is_cyan(s.primary_palette.hue) { 88.0 } else { 98.0 },
                                1.0,
                            )
                        } else {
                            // WATCH
                            Self::t_max_c(&s.primary_palette, 0.0, 100.0, 1.0)
                        }
                    }
                }
            }))
            .is_background(true)
            .background(Self::default_background())
            .contrast_curve(cc(|s| {
                if s.platform == Platform::Phone {
                    Some(Self::get_contrast_curve(4.5))
                } else {
                    Some(Self::get_contrast_curve(7.0))
                }
            }))
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
        ColorSpec2021.primary().extend_spec_version(SpecVersion::Spec2025, &color2025)
    }

    fn primary_dim(&self) -> Option<DynamicColor> {
        Some(
            DynamicColor::builder("primary_dim", pal(|s| s.primary_palette.clone()))
                .tone(tn(|s| match s.variant {
                    Variant::Neutral => 85.0,
                    Variant::TonalSpot => Self::t_max_c(&s.primary_palette, 0.0, 90.0, 1.0),
                    _ => Self::t_max_c(&s.primary_palette, 0.0, 100.0, 1.0),
                }))
                .is_background(true)
                .background(fixed_color(ColorSpec2026.surface_container_high()))
                .contrast_curve(cc(|_| Some(Self::get_contrast_curve(4.5))))
                .tone_delta_pair(tdp(|_| {
                    Some(ToneDeltaPair::new(
                        ColorSpec2026.primary_dim().unwrap(),
                        ColorSpec2026.primary(),
                        5.0,
                        TonePolarity::Darker,
                        true,
                        DeltaConstraint::Farther,
                    ))
                }))
                .build(),
        )
    }

    fn on_primary(&self) -> DynamicColor {
        let color2025 = DynamicColor::builder("on_primary", pal(|s| s.primary_palette.clone()))
            .background(bg(|s| {
                if s.platform == Platform::Phone {
                    Some(ColorSpec2026.primary())
                } else {
                    ColorSpec2026.primary_dim()
                }
            }))
            .contrast_curve(cc(|s| {
                if s.platform == Platform::Phone {
                    Some(Self::get_contrast_curve(6.0))
                } else {
                    Some(Self::get_contrast_curve(7.0))
                }
            }))
            .build();
        ColorSpec2021.on_primary().extend_spec_version(SpecVersion::Spec2025, &color2025)
    }

    fn primary_container(&self) -> DynamicColor {
        let color2025 =
            DynamicColor::builder("primary_container", pal(|s| s.primary_palette.clone()))
                .tone(tn(|s| {
                    if s.platform == Platform::Watch {
                        30.0
                    } else if s.variant == Variant::Neutral {
                        if s.is_dark { 30.0 } else { 90.0 }
                    } else if s.variant == Variant::TonalSpot {
                        if s.is_dark {
                            Self::t_min_c(&s.primary_palette, 35.0, 93.0)
                        } else {
                            Self::t_max_c(&s.primary_palette, 0.0, 90.0, 1.0)
                        }
                    } else if s.variant == Variant::Expressive {
                        if s.is_dark {
                            Self::t_min_c(&s.primary_palette, 30.0, 93.0)
                        } else {
                            Self::t_max_c(
                                &s.primary_palette,
                                78.0,
                                if Hct::is_cyan(s.primary_palette.hue) { 88.0 } else { 90.0 },
                                1.0,
                            )
                        }
                    } else {
                        // VIBRANT
                        if s.is_dark {
                            Self::t_min_c(&s.primary_palette, 66.0, 93.0)
                        } else {
                            Self::t_max_c(
                                &s.primary_palette,
                                66.0,
                                if Hct::is_cyan(s.primary_palette.hue) { 88.0 } else { 93.0 },
                                1.0,
                            )
                        }
                    }
                }))
                .is_background(true)
                .background(Self::phone_or_null_background())
                .tone_delta_pair(tdp(|s| {
                    if s.platform == Platform::Watch {
                        Some(ToneDeltaPair::new(
                            ColorSpec2026.primary_container(),
                            ColorSpec2026.primary_dim().unwrap(),
                            10.0,
                            TonePolarity::Darker,
                            true,
                            DeltaConstraint::Farther,
                        ))
                    } else {
                        None
                    }
                }))
                .contrast_curve(cc(|s| {
                    if s.platform == Platform::Phone && s.contrast_level > 0.0 {
                        Some(Self::get_contrast_curve(1.5))
                    } else {
                        None
                    }
                }))
                .build();
        ColorSpec2021.primary_container().extend_spec_version(SpecVersion::Spec2025, &color2025)
    }

    fn on_primary_container(&self) -> DynamicColor {
        let color2025 =
            DynamicColor::builder("on_primary_container", pal(|s| s.primary_palette.clone()))
                .background(fixed_color(ColorSpec2026.primary_container()))
                .contrast_curve(cc(|s| {
                    if s.platform == Platform::Phone {
                        Some(Self::get_contrast_curve(6.0))
                    } else {
                        Some(Self::get_contrast_curve(7.0))
                    }
                }))
                .build();
        ColorSpec2021.on_primary_container().extend_spec_version(SpecVersion::Spec2025, &color2025)
    }

    fn inverse_primary(&self) -> DynamicColor {
        let color2025 =
            DynamicColor::builder("inverse_primary", pal(|s| s.primary_palette.clone()))
                .tone(tn(|s| Self::t_max_c(&s.primary_palette, 0.0, 100.0, 1.0)))
                .background(fixed_color(ColorSpec2026.inverse_surface()))
                .contrast_curve(cc(|s| {
                    if s.platform == Platform::Phone {
                        Some(Self::get_contrast_curve(6.0))
                    } else {
                        Some(Self::get_contrast_curve(7.0))
                    }
                }))
                .build();
        ColorSpec2021.inverse_primary().extend_spec_version(SpecVersion::Spec2025, &color2025)
    }

    // ////////////////////////////////////////////////////////////////
    // Secondaries [Q] //
    // ////////////////////////////////////////////////////////////////
    fn secondary(&self) -> DynamicColor {
        let color2025 = DynamicColor::builder("secondary", pal(|s| s.secondary_palette.clone()))
            .tone(tn(|s| {
                if s.platform == Platform::Watch {
                    if s.variant == Variant::Neutral {
                        90.0
                    } else {
                        Self::t_max_c(&s.secondary_palette, 0.0, 90.0, 1.0)
                    }
                } else if s.variant == Variant::Neutral {
                    if s.is_dark {
                        Self::t_min_c(&s.secondary_palette, 0.0, 98.0)
                    } else {
                        Self::t_max_c(&s.secondary_palette, 0.0, 100.0, 1.0)
                    }
                } else if s.variant == Variant::Vibrant {
                    Self::t_max_c(
                        &s.secondary_palette,
                        0.0,
                        if s.is_dark { 90.0 } else { 98.0 },
                        1.0,
                    )
                } else {
                    // EXPRESSIVE and TONAL_SPOT
                    if s.is_dark {
                        80.0
                    } else {
                        Self::t_max_c(&s.secondary_palette, 0.0, 100.0, 1.0)
                    }
                }
            }))
            .is_background(true)
            .background(Self::default_background())
            .contrast_curve(cc(|s| {
                if s.platform == Platform::Phone {
                    Some(Self::get_contrast_curve(4.5))
                } else {
                    Some(Self::get_contrast_curve(7.0))
                }
            }))
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
        ColorSpec2021.secondary().extend_spec_version(SpecVersion::Spec2025, &color2025)
    }

    fn secondary_dim(&self) -> Option<DynamicColor> {
        Some(
            DynamicColor::builder("secondary_dim", pal(|s| s.secondary_palette.clone()))
                .tone(tn(|s| {
                    if s.variant == Variant::Neutral {
                        85.0
                    } else {
                        Self::t_max_c(&s.secondary_palette, 0.0, 90.0, 1.0)
                    }
                }))
                .is_background(true)
                .background(fixed_color(ColorSpec2026.surface_container_high()))
                .contrast_curve(cc(|_| Some(Self::get_contrast_curve(4.5))))
                .tone_delta_pair(tdp(|_| {
                    Some(ToneDeltaPair::new(
                        ColorSpec2026.secondary_dim().unwrap(),
                        ColorSpec2026.secondary(),
                        5.0,
                        TonePolarity::Darker,
                        true,
                        DeltaConstraint::Farther,
                    ))
                }))
                .build(),
        )
    }

    fn on_secondary(&self) -> DynamicColor {
        let color2025 = DynamicColor::builder("on_secondary", pal(|s| s.secondary_palette.clone()))
            .background(bg(|s| {
                if s.platform == Platform::Phone {
                    Some(ColorSpec2026.secondary())
                } else {
                    ColorSpec2026.secondary_dim()
                }
            }))
            .contrast_curve(cc(|s| {
                if s.platform == Platform::Phone {
                    Some(Self::get_contrast_curve(6.0))
                } else {
                    Some(Self::get_contrast_curve(7.0))
                }
            }))
            .build();
        ColorSpec2021.on_secondary().extend_spec_version(SpecVersion::Spec2025, &color2025)
    }

    fn secondary_container(&self) -> DynamicColor {
        let color2025 =
            DynamicColor::builder("secondary_container", pal(|s| s.secondary_palette.clone()))
                .tone(tn(|s| {
                    if s.platform == Platform::Watch {
                        30.0
                    } else if s.variant == Variant::Vibrant {
                        if s.is_dark {
                            Self::t_min_c(&s.secondary_palette, 30.0, 40.0)
                        } else {
                            Self::t_max_c(&s.secondary_palette, 84.0, 90.0, 1.0)
                        }
                    } else if s.variant == Variant::Expressive {
                        if s.is_dark {
                            15.0
                        } else {
                            Self::t_max_c(&s.secondary_palette, 90.0, 95.0, 1.0)
                        }
                    } else {
                        if s.is_dark { 25.0 } else { 90.0 }
                    }
                }))
                .is_background(true)
                .background(Self::phone_or_null_background())
                .tone_delta_pair(tdp(|s| {
                    if s.platform == Platform::Watch {
                        Some(ToneDeltaPair::new(
                            ColorSpec2026.secondary_container(),
                            ColorSpec2026.secondary_dim().unwrap(),
                            10.0,
                            TonePolarity::Darker,
                            true,
                            DeltaConstraint::Farther,
                        ))
                    } else {
                        None
                    }
                }))
                .contrast_curve(cc(|s| {
                    if s.platform == Platform::Phone && s.contrast_level > 0.0 {
                        Some(Self::get_contrast_curve(1.5))
                    } else {
                        None
                    }
                }))
                .build();
        ColorSpec2021.secondary_container().extend_spec_version(SpecVersion::Spec2025, &color2025)
    }

    fn on_secondary_container(&self) -> DynamicColor {
        let color2025 =
            DynamicColor::builder("on_secondary_container", pal(|s| s.secondary_palette.clone()))
                .background(fixed_color(ColorSpec2026.secondary_container()))
                .contrast_curve(cc(|s| {
                    if s.platform == Platform::Phone {
                        Some(Self::get_contrast_curve(6.0))
                    } else {
                        Some(Self::get_contrast_curve(7.0))
                    }
                }))
                .build();
        ColorSpec2021
            .on_secondary_container()
            .extend_spec_version(SpecVersion::Spec2025, &color2025)
    }

    // ////////////////////////////////////////////////////////////////
    // Tertiaries [T] //
    // ////////////////////////////////////////////////////////////////
    fn tertiary(&self) -> DynamicColor {
        let color2025 = DynamicColor::builder("tertiary", pal(|s| s.tertiary_palette.clone()))
            .tone(tn(|s| {
                if s.platform == Platform::Watch {
                    if s.variant == Variant::TonalSpot {
                        Self::t_max_c(&s.tertiary_palette, 0.0, 90.0, 1.0)
                    } else {
                        Self::t_max_c(&s.tertiary_palette, 0.0, 100.0, 1.0)
                    }
                } else if s.variant == Variant::Expressive || s.variant == Variant::Vibrant {
                    Self::t_max_c(
                        &s.tertiary_palette,
                        0.0,
                        if Hct::is_cyan(s.tertiary_palette.hue) {
                            88.0
                        } else if s.is_dark {
                            98.0
                        } else {
                            100.0
                        },
                        1.0,
                    )
                } else {
                    // NEUTRAL and TONAL_SPOT
                    if s.is_dark {
                        Self::t_max_c(&s.tertiary_palette, 0.0, 98.0, 1.0)
                    } else {
                        Self::t_max_c(&s.tertiary_palette, 0.0, 100.0, 1.0)
                    }
                }
            }))
            .is_background(true)
            .background(Self::default_background())
            .contrast_curve(cc(|s| {
                if s.platform == Platform::Phone {
                    Some(Self::get_contrast_curve(4.5))
                } else {
                    Some(Self::get_contrast_curve(7.0))
                }
            }))
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
        ColorSpec2021.tertiary().extend_spec_version(SpecVersion::Spec2025, &color2025)
    }

    fn tertiary_dim(&self) -> Option<DynamicColor> {
        Some(
            DynamicColor::builder("tertiary_dim", pal(|s| s.tertiary_palette.clone()))
                .tone(tn(|s| {
                    if s.variant == Variant::TonalSpot {
                        Self::t_max_c(&s.tertiary_palette, 0.0, 90.0, 1.0)
                    } else {
                        Self::t_max_c(&s.tertiary_palette, 0.0, 100.0, 1.0)
                    }
                }))
                .is_background(true)
                .background(fixed_color(ColorSpec2026.surface_container_high()))
                .contrast_curve(cc(|_| Some(Self::get_contrast_curve(4.5))))
                .tone_delta_pair(tdp(|_| {
                    Some(ToneDeltaPair::new(
                        ColorSpec2026.tertiary_dim().unwrap(),
                        ColorSpec2026.tertiary(),
                        5.0,
                        TonePolarity::Darker,
                        true,
                        DeltaConstraint::Farther,
                    ))
                }))
                .build(),
        )
    }

    fn on_tertiary(&self) -> DynamicColor {
        let color2025 = DynamicColor::builder("on_tertiary", pal(|s| s.tertiary_palette.clone()))
            .background(bg(|s| {
                if s.platform == Platform::Phone {
                    Some(ColorSpec2026.tertiary())
                } else {
                    ColorSpec2026.tertiary_dim()
                }
            }))
            .contrast_curve(cc(|s| {
                if s.platform == Platform::Phone {
                    Some(Self::get_contrast_curve(6.0))
                } else {
                    Some(Self::get_contrast_curve(7.0))
                }
            }))
            .build();
        ColorSpec2021.on_tertiary().extend_spec_version(SpecVersion::Spec2025, &color2025)
    }

    fn tertiary_container(&self) -> DynamicColor {
        let color2025 =
            DynamicColor::builder("tertiary_container", pal(|s| s.tertiary_palette.clone()))
                .tone(tn(|s| {
                    if s.platform == Platform::Watch {
                        if s.variant == Variant::TonalSpot {
                            Self::t_max_c(&s.tertiary_palette, 0.0, 90.0, 1.0)
                        } else {
                            Self::t_max_c(&s.tertiary_palette, 0.0, 100.0, 1.0)
                        }
                    } else if s.variant == Variant::Neutral {
                        if s.is_dark {
                            Self::t_max_c(&s.tertiary_palette, 0.0, 93.0, 1.0)
                        } else {
                            Self::t_max_c(&s.tertiary_palette, 0.0, 96.0, 1.0)
                        }
                    } else if s.variant == Variant::TonalSpot {
                        Self::t_max_c(
                            &s.tertiary_palette,
                            0.0,
                            if s.is_dark { 93.0 } else { 100.0 },
                            1.0,
                        )
                    } else if s.variant == Variant::Expressive {
                        Self::t_max_c(
                            &s.tertiary_palette,
                            75.0,
                            if Hct::is_cyan(s.tertiary_palette.hue) {
                                88.0
                            } else if s.is_dark {
                                93.0
                            } else {
                                100.0
                            },
                            1.0,
                        )
                    } else {
                        // VIBRANT
                        if s.is_dark {
                            Self::t_max_c(&s.tertiary_palette, 0.0, 93.0, 1.0)
                        } else {
                            Self::t_max_c(&s.tertiary_palette, 72.0, 100.0, 1.0)
                        }
                    }
                }))
                .is_background(true)
                .background(Self::phone_or_null_background())
                .tone_delta_pair(tdp(|s| {
                    if s.platform == Platform::Watch {
                        Some(ToneDeltaPair::new(
                            ColorSpec2026.tertiary_container(),
                            ColorSpec2026.tertiary_dim().unwrap(),
                            10.0,
                            TonePolarity::Darker,
                            true,
                            DeltaConstraint::Farther,
                        ))
                    } else {
                        None
                    }
                }))
                .contrast_curve(cc(|s| {
                    if s.platform == Platform::Phone && s.contrast_level > 0.0 {
                        Some(Self::get_contrast_curve(1.5))
                    } else {
                        None
                    }
                }))
                .build();
        ColorSpec2021.tertiary_container().extend_spec_version(SpecVersion::Spec2025, &color2025)
    }

    fn on_tertiary_container(&self) -> DynamicColor {
        let color2025 =
            DynamicColor::builder("on_tertiary_container", pal(|s| s.tertiary_palette.clone()))
                .background(fixed_color(ColorSpec2026.tertiary_container()))
                .contrast_curve(cc(|s| {
                    if s.platform == Platform::Phone {
                        Some(Self::get_contrast_curve(6.0))
                    } else {
                        Some(Self::get_contrast_curve(7.0))
                    }
                }))
                .build();
        ColorSpec2021.on_tertiary_container().extend_spec_version(SpecVersion::Spec2025, &color2025)
    }

    // ////////////////////////////////////////////////////////////////
    // Errors [E] //
    // ////////////////////////////////////////////////////////////////
    fn error(&self) -> DynamicColor {
        let color2025 = DynamicColor::builder("error", pal(|s| s.error_palette.clone()))
            .tone(tn(|s| {
                if s.platform == Platform::Phone {
                    if s.is_dark {
                        Self::t_min_c(&s.error_palette, 0.0, 98.0)
                    } else {
                        Self::t_max_c(&s.error_palette, 0.0, 100.0, 1.0)
                    }
                } else {
                    Self::t_min_c(&s.error_palette, 0.0, 100.0)
                }
            }))
            .is_background(true)
            .background(Self::default_background())
            .contrast_curve(cc(|s| {
                if s.platform == Platform::Phone {
                    Some(Self::get_contrast_curve(4.5))
                } else {
                    Some(Self::get_contrast_curve(7.0))
                }
            }))
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
        ColorSpec2021.error().extend_spec_version(SpecVersion::Spec2025, &color2025)
    }

    fn error_dim(&self) -> Option<DynamicColor> {
        Some(
            DynamicColor::builder("error_dim", pal(|s| s.error_palette.clone()))
                .tone(tn(|s| Self::t_min_c(&s.error_palette, 0.0, 100.0)))
                .is_background(true)
                .background(fixed_color(ColorSpec2026.surface_container_high()))
                .contrast_curve(cc(|_| Some(Self::get_contrast_curve(4.5))))
                .tone_delta_pair(tdp(|_| {
                    Some(ToneDeltaPair::new(
                        ColorSpec2026.error_dim().unwrap(),
                        ColorSpec2026.error(),
                        5.0,
                        TonePolarity::Darker,
                        true,
                        DeltaConstraint::Farther,
                    ))
                }))
                .build(),
        )
    }

    fn on_error(&self) -> DynamicColor {
        let color2025 = DynamicColor::builder("on_error", pal(|s| s.error_palette.clone()))
            .background(bg(|s| {
                if s.platform == Platform::Phone {
                    Some(ColorSpec2026.error())
                } else {
                    ColorSpec2026.error_dim()
                }
            }))
            .contrast_curve(cc(|s| {
                if s.platform == Platform::Phone {
                    Some(Self::get_contrast_curve(6.0))
                } else {
                    Some(Self::get_contrast_curve(7.0))
                }
            }))
            .build();
        ColorSpec2021.on_error().extend_spec_version(SpecVersion::Spec2025, &color2025)
    }

    fn error_container(&self) -> DynamicColor {
        let color2025 = DynamicColor::builder("error_container", pal(|s| s.error_palette.clone()))
            .tone(tn(|s| {
                if s.platform == Platform::Watch {
                    30.0
                } else {
                    if s.is_dark {
                        Self::t_min_c(&s.error_palette, 30.0, 93.0)
                    } else {
                        Self::t_max_c(&s.error_palette, 0.0, 90.0, 1.0)
                    }
                }
            }))
            .is_background(true)
            .background(Self::phone_or_null_background())
            .tone_delta_pair(tdp(|s| {
                if s.platform == Platform::Watch {
                    Some(ToneDeltaPair::new(
                        ColorSpec2026.error_container(),
                        ColorSpec2026.error_dim().unwrap(),
                        10.0,
                        TonePolarity::Darker,
                        true,
                        DeltaConstraint::Farther,
                    ))
                } else {
                    None
                }
            }))
            .contrast_curve(cc(|s| {
                if s.platform == Platform::Phone && s.contrast_level > 0.0 {
                    Some(Self::get_contrast_curve(1.5))
                } else {
                    None
                }
            }))
            .build();
        ColorSpec2021.error_container().extend_spec_version(SpecVersion::Spec2025, &color2025)
    }

    fn on_error_container(&self) -> DynamicColor {
        let color2025 =
            DynamicColor::builder("on_error_container", pal(|s| s.error_palette.clone()))
                .background(fixed_color(ColorSpec2026.error_container()))
                .contrast_curve(cc(|s| {
                    if s.platform == Platform::Phone {
                        Some(Self::get_contrast_curve(4.5))
                    } else {
                        Some(Self::get_contrast_curve(7.0))
                    }
                }))
                .build();
        ColorSpec2021.on_error_container().extend_spec_version(SpecVersion::Spec2025, &color2025)
    }

    // ////////////////////////////////////////////////////////////////
    // Primary Fixed Colors [PF] //
    // ////////////////////////////////////////////////////////////////
    fn primary_fixed(&self) -> DynamicColor {
        let color2025 = DynamicColor::builder("primary_fixed", pal(|s| s.primary_palette.clone()))
            .tone(tn(|s| {
                let temp_s = DynamicScheme::from_with_contrast(s, false, 0.0);
                ColorSpec2026.primary_container().get_tone(&temp_s)
            }))
            .is_background(true)
            .background(Self::phone_or_null_background())
            .contrast_curve(cc(|s| {
                if s.platform == Platform::Phone && s.contrast_level > 0.0 {
                    Some(Self::get_contrast_curve(1.5))
                } else {
                    None
                }
            }))
            .build();
        ColorSpec2021.primary_fixed().extend_spec_version(SpecVersion::Spec2025, &color2025)
    }

    fn primary_fixed_dim(&self) -> DynamicColor {
        let color2025 =
            DynamicColor::builder("primary_fixed_dim", pal(|s| s.primary_palette.clone()))
                .tone(tn(|s| ColorSpec2026.primary_fixed().get_tone(s)))
                .is_background(true)
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
                .build();
        ColorSpec2021.primary_fixed_dim().extend_spec_version(SpecVersion::Spec2025, &color2025)
    }

    fn on_primary_fixed(&self) -> DynamicColor {
        let color2025 =
            DynamicColor::builder("on_primary_fixed", pal(|s| s.primary_palette.clone()))
                .background(fixed_color(ColorSpec2026.primary_fixed_dim()))
                .contrast_curve(cc(|_| Some(Self::get_contrast_curve(7.0))))
                .build();
        ColorSpec2021.on_primary_fixed().extend_spec_version(SpecVersion::Spec2025, &color2025)
    }

    fn on_primary_fixed_variant(&self) -> DynamicColor {
        let color2025 =
            DynamicColor::builder("on_primary_fixed_variant", pal(|s| s.primary_palette.clone()))
                .background(fixed_color(ColorSpec2026.primary_fixed_dim()))
                .contrast_curve(cc(|_| Some(Self::get_contrast_curve(4.5))))
                .build();
        ColorSpec2021
            .on_primary_fixed_variant()
            .extend_spec_version(SpecVersion::Spec2025, &color2025)
    }

    // ////////////////////////////////////////////////////////////////
    // Secondary Fixed Colors [QF] //
    // ////////////////////////////////////////////////////////////////
    fn secondary_fixed(&self) -> DynamicColor {
        let color2025 =
            DynamicColor::builder("secondary_fixed", pal(|s| s.secondary_palette.clone()))
                .tone(tn(|s| {
                    let temp_s = DynamicScheme::from_with_contrast(s, false, 0.0);
                    ColorSpec2026.secondary_container().get_tone(&temp_s)
                }))
                .is_background(true)
                .background(Self::phone_or_null_background())
                .contrast_curve(cc(|s| {
                    if s.platform == Platform::Phone && s.contrast_level > 0.0 {
                        Some(Self::get_contrast_curve(1.5))
                    } else {
                        None
                    }
                }))
                .build();
        ColorSpec2021.secondary_fixed().extend_spec_version(SpecVersion::Spec2025, &color2025)
    }

    fn secondary_fixed_dim(&self) -> DynamicColor {
        let color2025 =
            DynamicColor::builder("secondary_fixed_dim", pal(|s| s.secondary_palette.clone()))
                .tone(tn(|s| ColorSpec2026.secondary_fixed().get_tone(s)))
                .is_background(true)
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
                .build();
        ColorSpec2021.secondary_fixed_dim().extend_spec_version(SpecVersion::Spec2025, &color2025)
    }

    fn on_secondary_fixed(&self) -> DynamicColor {
        let color2025 =
            DynamicColor::builder("on_secondary_fixed", pal(|s| s.secondary_palette.clone()))
                .background(fixed_color(ColorSpec2026.secondary_fixed_dim()))
                .contrast_curve(cc(|_| Some(Self::get_contrast_curve(7.0))))
                .build();
        ColorSpec2021.on_secondary_fixed().extend_spec_version(SpecVersion::Spec2025, &color2025)
    }

    fn on_secondary_fixed_variant(&self) -> DynamicColor {
        let color2025 = DynamicColor::builder(
            "on_secondary_fixed_variant",
            pal(|s| s.secondary_palette.clone()),
        )
        .background(fixed_color(ColorSpec2026.secondary_fixed_dim()))
        .contrast_curve(cc(|_| Some(Self::get_contrast_curve(4.5))))
        .build();
        ColorSpec2021
            .on_secondary_fixed_variant()
            .extend_spec_version(SpecVersion::Spec2025, &color2025)
    }

    // ////////////////////////////////////////////////////////////////
    // Tertiary Fixed Colors [TF] //
    // ////////////////////////////////////////////////////////////////
    fn tertiary_fixed(&self) -> DynamicColor {
        let color2025 =
            DynamicColor::builder("tertiary_fixed", pal(|s| s.tertiary_palette.clone()))
                .tone(tn(|s| {
                    let temp_s = DynamicScheme::from_with_contrast(s, false, 0.0);
                    ColorSpec2026.tertiary_container().get_tone(&temp_s)
                }))
                .is_background(true)
                .background(Self::phone_or_null_background())
                .contrast_curve(cc(|s| {
                    if s.platform == Platform::Phone && s.contrast_level > 0.0 {
                        Some(Self::get_contrast_curve(1.5))
                    } else {
                        None
                    }
                }))
                .build();
        ColorSpec2021.tertiary_fixed().extend_spec_version(SpecVersion::Spec2025, &color2025)
    }

    fn tertiary_fixed_dim(&self) -> DynamicColor {
        let color2025 =
            DynamicColor::builder("tertiary_fixed_dim", pal(|s| s.tertiary_palette.clone()))
                .tone(tn(|s| ColorSpec2026.tertiary_fixed().get_tone(s)))
                .is_background(true)
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
                .build();
        ColorSpec2021.tertiary_fixed_dim().extend_spec_version(SpecVersion::Spec2025, &color2025)
    }

    fn on_tertiary_fixed(&self) -> DynamicColor {
        let color2025 =
            DynamicColor::builder("on_tertiary_fixed", pal(|s| s.tertiary_palette.clone()))
                .background(fixed_color(ColorSpec2026.tertiary_fixed_dim()))
                .contrast_curve(cc(|_| Some(Self::get_contrast_curve(7.0))))
                .build();
        ColorSpec2021.on_tertiary_fixed().extend_spec_version(SpecVersion::Spec2025, &color2025)
    }

    fn on_tertiary_fixed_variant(&self) -> DynamicColor {
        let color2025 =
            DynamicColor::builder("on_tertiary_fixed_variant", pal(|s| s.tertiary_palette.clone()))
                .background(fixed_color(ColorSpec2026.tertiary_fixed_dim()))
                .contrast_curve(cc(|_| Some(Self::get_contrast_curve(4.5))))
                .build();
        ColorSpec2021
            .on_tertiary_fixed_variant()
            .extend_spec_version(SpecVersion::Spec2025, &color2025)
    }

    // /////////////////////////////////////////////////////////////////
    // Color value calculations //
    // /////////////////////////////////////////////////////////////////
    fn get_hct(&self, scheme: &DynamicScheme, color: &DynamicColor) -> Hct {
        // This is crucial for aesthetics: we aren't simply taking the standard
        // color and changing its tone for contrast. Rather, we find the tone
        // for contrast, then use the specified chroma from the palette to
        // construct a new color.
        //
        // For example, this enables colors with standard tone of T90, which
        // has limited chroma, to "recover" intended chroma as contrast
        // increases.
        let palette = (color.palette)(scheme);
        let tone = self.get_tone(scheme, color);
        let chroma_multiplier = color.chroma_multiplier.as_ref().map(|f| f(scheme)).unwrap_or(1.0);
        if chroma_multiplier == 1.0 {
            return palette.get_hct(tone);
        }

        let chroma = palette.chroma * chroma_multiplier;
        if tone == 99.0 && Hct::is_yellow(palette.hue) {
            return TonalPalette::from_hue_and_chroma(palette.hue, chroma).get_hct(tone);
        }
        Hct::from(palette.hue, chroma, tone)
    }

    fn get_tone(&self, scheme: &DynamicScheme, color: &DynamicColor) -> f64 {
        let tone_delta_pair = color.tone_delta_pair.as_ref().and_then(|f| f(scheme));

        // Case 0: tone delta pair.
        if let Some(tone_delta_pair) = tone_delta_pair {
            let role_a = tone_delta_pair.role_a.clone();
            let role_b = tone_delta_pair.role_b.clone();
            let polarity = tone_delta_pair.polarity;
            let constraint = tone_delta_pair.constraint;
            let absolute_delta = if polarity == TonePolarity::Darker
                || (polarity == TonePolarity::RelativeLighter && scheme.is_dark)
                || (polarity == TonePolarity::RelativeDarker && !scheme.is_dark)
            {
                -tone_delta_pair.delta
            } else {
                tone_delta_pair.delta
            };
            let am_role_a = color.name == role_a.name;
            let self_role = if am_role_a { &role_a } else { &role_b };
            let reference_role = if am_role_a { &role_b } else { &role_a };
            let mut self_tone = (self_role.tone)(scheme);
            let reference_tone = reference_role.get_tone(scheme);
            let relative_delta = absolute_delta * (if am_role_a { 1.0 } else { -1.0 });
            match constraint {
                DeltaConstraint::Exact => {
                    self_tone = (reference_tone + relative_delta).clamp(0.0, 100.0);
                }
                DeltaConstraint::Nearer => {
                    if relative_delta > 0.0 {
                        self_tone = self_tone
                            .clamp(reference_tone, reference_tone + relative_delta)
                            .clamp(0.0, 100.0);
                    } else {
                        self_tone = self_tone
                            .clamp(reference_tone + relative_delta, reference_tone)
                            .clamp(0.0, 100.0);
                    }
                }
                DeltaConstraint::Farther => {
                    if relative_delta > 0.0 {
                        self_tone = self_tone.clamp(reference_tone + relative_delta, 100.0);
                    } else {
                        self_tone = self_tone.clamp(0.0, reference_tone + relative_delta);
                    }
                }
            }
            let background = color.background.as_ref().and_then(|f| f(scheme));
            let contrast_curve = color.contrast_curve.as_ref().and_then(|f| f(scheme));
            if let (Some(background), Some(contrast_curve)) = (background, contrast_curve) {
                let bg_tone = background.get_tone(scheme);
                let self_contrast = contrast_curve.get(scheme.contrast_level);
                self_tone = if Contrast::ratio_of_tones(bg_tone, self_tone) >= self_contrast
                    && scheme.contrast_level >= 0.0
                {
                    self_tone
                } else {
                    DynamicColor::foreground_tone(bg_tone, self_contrast)
                };
            }

            // This can avoid the awkward tones for background colors including
            // the access fixed colors. Accent fixed dim colors should not be
            // adjusted.
            if color.is_background && !color.name.ends_with("_fixed_dim") {
                self_tone = if self_tone >= 57.0 {
                    self_tone.clamp(65.0, 100.0)
                } else {
                    self_tone.clamp(0.0, 49.0)
                };
            }
            return self_tone;
        }

        // Case 1: No tone delta pair; just solve for itself.
        let mut answer = (color.tone)(scheme);
        let background = color.background.as_ref().and_then(|f| f(scheme));
        let contrast_curve = color.contrast_curve.as_ref().and_then(|f| f(scheme));
        if background.is_none() || contrast_curve.is_none() {
            return answer; // No adjustment for colors with no background.
        }
        let background = background.unwrap();
        let bg_tone = background.get_tone(scheme);
        let desired_ratio = contrast_curve.unwrap().get(scheme.contrast_level);

        // Recalculate the tone from desired contrast ratio if the current
        // contrast ratio is not enough or desired contrast level is decreasing
        // (<0).
        answer = if Contrast::ratio_of_tones(bg_tone, answer) >= desired_ratio
            && scheme.contrast_level >= 0.0
        {
            answer
        } else {
            DynamicColor::foreground_tone(bg_tone, desired_ratio)
        };

        // This can avoid the awkward tones for background colors including the
        // access fixed colors. Accent fixed dim colors should not be adjusted.
        if color.is_background && !color.name.ends_with("_fixed_dim") {
            answer =
                if answer >= 57.0 { answer.clamp(65.0, 100.0) } else { answer.clamp(0.0, 49.0) };
        }
        let second_background = color.second_background.as_ref().and_then(|f| f(scheme));
        if second_background.is_none() {
            return answer;
        }
        let second_background = second_background.unwrap();

        // Case 2: Adjust for dual backgrounds.
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
        is_dark: bool,
        platform: Platform,
        contrast_level: f64,
    ) -> TonalPalette {
        match variant {
            Variant::Neutral => TonalPalette::from_hue_and_chroma(
                source_color_hct.hue(),
                if platform == Platform::Phone {
                    if Hct::is_blue(source_color_hct.hue()) { 12.0 } else { 8.0 }
                } else if Hct::is_blue(source_color_hct.hue()) {
                    16.0
                } else {
                    12.0
                },
            ),
            Variant::TonalSpot => TonalPalette::from_hue_and_chroma(
                source_color_hct.hue(),
                if platform == Platform::Phone && is_dark { 26.0 } else { 32.0 },
            ),
            Variant::Expressive => TonalPalette::from_hue_and_chroma(
                source_color_hct.hue(),
                if platform == Platform::Phone { if is_dark { 36.0 } else { 48.0 } } else { 40.0 },
            ),
            Variant::Vibrant => TonalPalette::from_hue_and_chroma(
                source_color_hct.hue(),
                if platform == Platform::Phone { 74.0 } else { 56.0 },
            ),
            _ => ColorSpec2021.get_primary_palette(
                variant,
                source_color_hct,
                is_dark,
                platform,
                contrast_level,
            ),
        }
    }

    fn get_secondary_palette(
        &self,
        variant: Variant,
        source_color_hct: Hct,
        is_dark: bool,
        platform: Platform,
        contrast_level: f64,
    ) -> TonalPalette {
        match variant {
            Variant::Neutral => TonalPalette::from_hue_and_chroma(
                source_color_hct.hue(),
                if platform == Platform::Phone {
                    if Hct::is_blue(source_color_hct.hue()) { 6.0 } else { 4.0 }
                } else if Hct::is_blue(source_color_hct.hue()) {
                    10.0
                } else {
                    6.0
                },
            ),
            Variant::TonalSpot => TonalPalette::from_hue_and_chroma(source_color_hct.hue(), 16.0),
            Variant::Expressive => TonalPalette::from_hue_and_chroma(
                DynamicScheme::get_rotated_hue(
                    source_color_hct,
                    &[0.0, 105.0, 140.0, 204.0, 253.0, 278.0, 300.0, 333.0, 360.0],
                    &[-160.0, 155.0, -100.0, 96.0, -96.0, -156.0, -165.0, -160.0],
                ),
                if platform == Platform::Phone { if is_dark { 16.0 } else { 24.0 } } else { 24.0 },
            ),
            Variant::Vibrant => TonalPalette::from_hue_and_chroma(
                DynamicScheme::get_rotated_hue(
                    source_color_hct,
                    &[0.0, 38.0, 105.0, 140.0, 333.0, 360.0],
                    &[-14.0, 10.0, -14.0, 10.0, -14.0],
                ),
                if platform == Platform::Phone { 56.0 } else { 36.0 },
            ),
            _ => ColorSpec2021.get_secondary_palette(
                variant,
                source_color_hct,
                is_dark,
                platform,
                contrast_level,
            ),
        }
    }

    fn get_tertiary_palette(
        &self,
        variant: Variant,
        source_color_hct: Hct,
        is_dark: bool,
        platform: Platform,
        contrast_level: f64,
    ) -> TonalPalette {
        match variant {
            Variant::Neutral => TonalPalette::from_hue_and_chroma(
                DynamicScheme::get_rotated_hue(
                    source_color_hct,
                    &[0.0, 38.0, 105.0, 161.0, 204.0, 278.0, 333.0, 360.0],
                    &[-32.0, 26.0, 10.0, -39.0, 24.0, -15.0, -32.0],
                ),
                if platform == Platform::Phone { 20.0 } else { 36.0 },
            ),
            Variant::TonalSpot => TonalPalette::from_hue_and_chroma(
                DynamicScheme::get_rotated_hue(
                    source_color_hct,
                    &[0.0, 20.0, 71.0, 161.0, 333.0, 360.0],
                    &[-40.0, 48.0, -32.0, 40.0, -32.0],
                ),
                if platform == Platform::Phone { 28.0 } else { 32.0 },
            ),
            Variant::Expressive => TonalPalette::from_hue_and_chroma(
                DynamicScheme::get_rotated_hue(
                    source_color_hct,
                    &[0.0, 105.0, 140.0, 204.0, 253.0, 278.0, 300.0, 333.0, 360.0],
                    &[-165.0, 160.0, -105.0, 101.0, -101.0, -160.0, -170.0, -165.0],
                ),
                48.0,
            ),
            Variant::Vibrant => TonalPalette::from_hue_and_chroma(
                DynamicScheme::get_rotated_hue(
                    source_color_hct,
                    &[0.0, 38.0, 71.0, 105.0, 140.0, 161.0, 253.0, 333.0, 360.0],
                    &[-72.0, 35.0, 24.0, -24.0, 62.0, 50.0, 62.0, -72.0],
                ),
                56.0,
            ),
            _ => ColorSpec2021.get_tertiary_palette(
                variant,
                source_color_hct,
                is_dark,
                platform,
                contrast_level,
            ),
        }
    }

    fn get_neutral_palette(
        &self,
        variant: Variant,
        source_color_hct: Hct,
        is_dark: bool,
        platform: Platform,
        contrast_level: f64,
    ) -> TonalPalette {
        match variant {
            Variant::Neutral => TonalPalette::from_hue_and_chroma(
                source_color_hct.hue(),
                if platform == Platform::Phone { 1.4 } else { 6.0 },
            ),
            Variant::TonalSpot => TonalPalette::from_hue_and_chroma(
                source_color_hct.hue(),
                if platform == Platform::Phone { 5.0 } else { 10.0 },
            ),
            Variant::Expressive => TonalPalette::from_hue_and_chroma(
                Self::get_expressive_neutral_hue(source_color_hct),
                Self::get_expressive_neutral_chroma(source_color_hct, is_dark, platform),
            ),
            Variant::Vibrant => TonalPalette::from_hue_and_chroma(
                Self::get_vibrant_neutral_hue(source_color_hct),
                Self::get_vibrant_neutral_chroma(source_color_hct, platform),
            ),
            _ => ColorSpec2021.get_neutral_palette(
                variant,
                source_color_hct,
                is_dark,
                platform,
                contrast_level,
            ),
        }
    }

    fn get_neutral_variant_palette(
        &self,
        variant: Variant,
        source_color_hct: Hct,
        is_dark: bool,
        platform: Platform,
        contrast_level: f64,
    ) -> TonalPalette {
        match variant {
            Variant::Neutral => TonalPalette::from_hue_and_chroma(
                source_color_hct.hue(),
                (if platform == Platform::Phone { 1.4 } else { 6.0 }) * 2.2,
            ),
            Variant::TonalSpot => TonalPalette::from_hue_and_chroma(
                source_color_hct.hue(),
                (if platform == Platform::Phone { 5.0 } else { 10.0 }) * 1.7,
            ),
            Variant::Expressive => {
                let expressive_neutral_hue = Self::get_expressive_neutral_hue(source_color_hct);
                let expressive_neutral_chroma =
                    Self::get_expressive_neutral_chroma(source_color_hct, is_dark, platform);
                TonalPalette::from_hue_and_chroma(
                    expressive_neutral_hue,
                    expressive_neutral_chroma
                        * if expressive_neutral_hue >= 105.0 && expressive_neutral_hue < 125.0 {
                            1.6
                        } else {
                            2.3
                        },
                )
            }
            Variant::Vibrant => {
                let vibrant_neutral_hue = Self::get_vibrant_neutral_hue(source_color_hct);
                let vibrant_neutral_chroma =
                    Self::get_vibrant_neutral_chroma(source_color_hct, platform);
                TonalPalette::from_hue_and_chroma(
                    vibrant_neutral_hue,
                    vibrant_neutral_chroma * 1.29,
                )
            }
            _ => ColorSpec2021.get_neutral_variant_palette(
                variant,
                source_color_hct,
                is_dark,
                platform,
                contrast_level,
            ),
        }
    }

    fn get_error_palette(
        &self,
        variant: Variant,
        source_color_hct: Hct,
        is_dark: bool,
        platform: Platform,
        contrast_level: f64,
    ) -> TonalPalette {
        let error_hue = DynamicScheme::get_piecewise_value(
            source_color_hct,
            &[0.0, 3.0, 13.0, 23.0, 33.0, 43.0, 153.0, 273.0, 360.0],
            &[12.0, 22.0, 32.0, 12.0, 22.0, 32.0, 22.0, 12.0],
        );
        match variant {
            Variant::Neutral => TonalPalette::from_hue_and_chroma(
                error_hue,
                if platform == Platform::Phone { 50.0 } else { 40.0 },
            ),
            Variant::TonalSpot => TonalPalette::from_hue_and_chroma(
                error_hue,
                if platform == Platform::Phone { 60.0 } else { 48.0 },
            ),
            Variant::Expressive => TonalPalette::from_hue_and_chroma(
                error_hue,
                if platform == Platform::Phone { 64.0 } else { 48.0 },
            ),
            Variant::Vibrant => TonalPalette::from_hue_and_chroma(
                error_hue,
                if platform == Platform::Phone { 80.0 } else { 60.0 },
            ),
            _ => ColorSpec2021.get_error_palette(
                variant,
                source_color_hct,
                is_dark,
                platform,
                contrast_level,
            ),
        }
    }
}
