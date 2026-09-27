// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Material Design dynamic color schemes, computed at run time with the
//! `material-color-utils` crate (a port of Google's material-color-utilities).

use alloc::vec::Vec;

use crate::graphics::{Color, Image};
use crate::items::{
    MaterialColorScheme, MaterialSchemePlatform, MaterialSchemeVariant, MaterialSpecVersion,
};
use material_color_utils::dynamiccolor::{
    DynamicScheme, MaterialDynamicColors, Platform, SpecVersion,
};
use material_color_utils::hct::Hct;
use material_color_utils::quantize::QuantizerCelebi;
use material_color_utils::scheme;

fn argb(color: Color) -> i64 {
    color.as_argb_encoded() as i64
}

fn color(argb: i64) -> Color {
    Color::from_argb_encoded(argb as u32)
}

fn to_spec_version(spec_version: MaterialSpecVersion) -> SpecVersion {
    match spec_version {
        MaterialSpecVersion::Spec2021 => SpecVersion::Spec2021,
        MaterialSpecVersion::Spec2025 => SpecVersion::Spec2025,
        MaterialSpecVersion::Spec2026 => SpecVersion::Spec2026,
    }
}

fn to_platform(platform: MaterialSchemePlatform) -> Platform {
    match platform {
        MaterialSchemePlatform::Phone => Platform::Phone,
        MaterialSchemePlatform::Watch => Platform::Watch,
    }
}

/// Generates a Material Design dynamic color scheme from a seed color, like
/// `new SchemeTonalSpot(sourceColorHct, isDark, contrastLevel, specVersion, platform)`
/// in material-color-utilities.
pub fn color_scheme(
    seed_color: Color,
    variant: MaterialSchemeVariant,
    spec_version: MaterialSpecVersion,
    platform: MaterialSchemePlatform,
    is_dark: bool,
    contrast_level: f32,
) -> MaterialColorScheme {
    let seeds = Vec::from([Hct::from_int(argb(seed_color))]);
    let spec_version = to_spec_version(spec_version);
    let platform = to_platform(platform);
    let contrast_level = contrast_level.clamp(-1.0, 1.0) as f64;
    let scheme = match variant {
        MaterialSchemeVariant::Monochrome => {
            scheme::SchemeMonochrome::new(seeds, is_dark, contrast_level, spec_version, platform)
        }
        MaterialSchemeVariant::Neutral => {
            scheme::SchemeNeutral::new(seeds, is_dark, contrast_level, spec_version, platform)
        }
        MaterialSchemeVariant::TonalSpot => {
            scheme::SchemeTonalSpot::new(seeds, is_dark, contrast_level, spec_version, platform)
        }
        MaterialSchemeVariant::Vibrant => {
            scheme::SchemeVibrant::new(seeds, is_dark, contrast_level, spec_version, platform)
        }
        MaterialSchemeVariant::Expressive => {
            scheme::SchemeExpressive::new(seeds, is_dark, contrast_level, spec_version, platform)
        }
        MaterialSchemeVariant::Fidelity => {
            scheme::SchemeFidelity::new(seeds, is_dark, contrast_level, spec_version, platform)
        }
        MaterialSchemeVariant::Content => {
            scheme::SchemeContent::new(seeds, is_dark, contrast_level, spec_version, platform)
        }
        MaterialSchemeVariant::Rainbow => {
            scheme::SchemeRainbow::new(seeds, is_dark, contrast_level, spec_version, platform)
        }
        MaterialSchemeVariant::FruitSalad => {
            scheme::SchemeFruitSalad::new(seeds, is_dark, contrast_level, spec_version, platform)
        }
        MaterialSchemeVariant::Cmf => {
            scheme::SchemeCmf::new(seeds, is_dark, contrast_level, spec_version, platform)
        }
    };
    color_scheme_to_struct(&scheme)
}

fn color_scheme_to_struct(scheme: &DynamicScheme) -> MaterialColorScheme {
    let colors = MaterialDynamicColors::new();
    let get = |dynamic_color: &material_color_utils::dynamiccolor::DynamicColor| {
        color(scheme.get_argb(dynamic_color))
    };
    MaterialColorScheme {
        primary: get(&colors.primary()),
        surface_tint: get(&colors.surface_tint()),
        on_primary: get(&colors.on_primary()),
        primary_container: get(&colors.primary_container()),
        on_primary_container: get(&colors.on_primary_container()),
        secondary: get(&colors.secondary()),
        on_secondary: get(&colors.on_secondary()),
        secondary_container: get(&colors.secondary_container()),
        on_secondary_container: get(&colors.on_secondary_container()),
        tertiary: get(&colors.tertiary()),
        on_tertiary: get(&colors.on_tertiary()),
        tertiary_container: get(&colors.tertiary_container()),
        on_tertiary_container: get(&colors.on_tertiary_container()),
        error: get(&colors.error()),
        on_error: get(&colors.on_error()),
        error_container: get(&colors.error_container()),
        on_error_container: get(&colors.on_error_container()),
        background: get(&colors.background()),
        on_background: get(&colors.on_background()),
        surface: get(&colors.surface()),
        on_surface: get(&colors.on_surface()),
        surface_variant: get(&colors.surface_variant()),
        on_surface_variant: get(&colors.on_surface_variant()),
        outline: get(&colors.outline()),
        outline_variant: get(&colors.outline_variant()),
        shadow: get(&colors.shadow()),
        scrim: get(&colors.scrim()),
        inverse_surface: get(&colors.inverse_surface()),
        inverse_on_surface: get(&colors.inverse_on_surface()),
        inverse_primary: get(&colors.inverse_primary()),
        primary_fixed: get(&colors.primary_fixed()),
        on_primary_fixed: get(&colors.on_primary_fixed()),
        primary_fixed_dim: get(&colors.primary_fixed_dim()),
        on_primary_fixed_variant: get(&colors.on_primary_fixed_variant()),
        secondary_fixed: get(&colors.secondary_fixed()),
        on_secondary_fixed: get(&colors.on_secondary_fixed()),
        secondary_fixed_dim: get(&colors.secondary_fixed_dim()),
        on_secondary_fixed_variant: get(&colors.on_secondary_fixed_variant()),
        tertiary_fixed: get(&colors.tertiary_fixed()),
        on_tertiary_fixed: get(&colors.on_tertiary_fixed()),
        tertiary_fixed_dim: get(&colors.tertiary_fixed_dim()),
        on_tertiary_fixed_variant: get(&colors.on_tertiary_fixed_variant()),
        surface_dim: get(&colors.surface_dim()),
        surface_bright: get(&colors.surface_bright()),
        surface_container_lowest: get(&colors.surface_container_lowest()),
        surface_container_low: get(&colors.surface_container_low()),
        surface_container: get(&colors.surface_container()),
        surface_container_high: get(&colors.surface_container_high()),
        surface_container_highest: get(&colors.surface_container_highest()),
    }
}

/// The Material Design dynamic color scheme the platform provides: on Android
/// 12+ the exact system scheme the OS derives from the user's wallpaper (the
/// equivalent of `dynamicLightColorScheme(context)`/`dynamicDarkColorScheme(context)`
/// in androidx `DynamicTonalPalette.android.kt`). On platforms that only expose
/// an accent color, a scheme generated from it with the given parameters;
/// when the platform reports neither accent nor schemes, a scheme generated
/// from the Material baseline seed.
pub fn platform_color_scheme(
    root: &crate::item_tree::ItemTreeRc,
    variant: MaterialSchemeVariant,
    spec_version: MaterialSpecVersion,
    platform: MaterialSchemePlatform,
    is_dark: bool,
    contrast_level: f32,
) -> MaterialColorScheme {
    platform_color_scheme_for_context(
        crate::window::context_for_root(root).as_ref(),
        variant,
        spec_version,
        platform,
        is_dark,
        contrast_level,
    )
}

/// `platform_color_scheme` for a `SlintContext` directly. `None` behaves like
/// a platform that reports nothing: generate from the baseline seed.
pub fn platform_color_scheme_for_context(
    ctx: Option<&crate::SlintContext>,
    variant: MaterialSchemeVariant,
    spec_version: MaterialSpecVersion,
    platform: MaterialSchemePlatform,
    is_dark: bool,
    contrast_level: f32,
) -> MaterialColorScheme {
    if let Some((light, dark)) = ctx.and_then(|c| c.platform_schemes()) {
        return if is_dark { dark } else { light };
    }
    let accent = ctx.map(|c| c.accent_color()).unwrap_or_default();
    let seed = if accent.alpha() == 0 { Color::from_argb_encoded(0xff6750a4) } else { accent };
    color_scheme(seed, variant, spec_version, platform, is_dark, contrast_level)
}

/// Builds the light and dark `MaterialColorScheme`s from the Android system
/// colors the backend's `system_color_schemes()` JNI call packs, porting
/// `dynamicTonalPalette()` + `dynamicLightColorScheme31()`/
/// `dynamicDarkColorScheme31()` (API 31–33) and `dynamicLightColorScheme34()`/
/// `dynamicDarkColorScheme34()` (API 34+) from androidx
/// `DynamicTonalPalette.android.kt` at the token pin
/// (ui-libraries/material/TOKENS_SOURCE).
///
/// Wire format: 65 ints = 5 tonal palettes × 13 tones (accent1, accent2,
/// accent3, neutral1, neutral2 palettes at tones 100, 99, 95, 90, 80, 70, 60,
/// 50, 40, 30, 20, 10, 0 — the `mc3_palette` styleable order in
/// res/values-v31/styles.xml), or 86 ints = the 43 color roles for light then
/// dark (API 34+, the `lightColorScheme()`/`darkColorScheme()` argument order
/// of `dynamic*ColorScheme34`). Other lengths return `None`.
pub fn android_system_schemes(data: &[i32]) -> Option<(MaterialColorScheme, MaterialColorScheme)> {
    match data.len() {
        65 => {
            let p = |i: usize| &data[i * 13..i * 13 + 13];
            let pal = SystemPalettes {
                primary: p(0).try_into().ok()?,
                secondary: p(1).try_into().ok()?,
                tertiary: p(2).try_into().ok()?,
                neutral_variant: p(4).try_into().ok()?,
            };
            Some((android_scheme31(&pal, false), android_scheme31(&pal, true)))
        }
        86 => Some((android_scheme34(&data[..43], false), android_scheme34(&data[43..], true))),
        _ => None,
    }
}

/// `Color.setLuminance` in DynamicTonalPalette.android.kt: keep hue and chroma,
/// move to the given tone.
fn set_luminance(argb: i32, tone: f64) -> i32 {
    if !(0.0001..=99.9999).contains(&tone) {
        return material_color_utils::utils::ColorUtils::argb_from_lstar(tone) as i32;
    }
    let hct = Hct::from_int(argb as i64);
    Hct::from(hct.hue(), hct.chroma(), tone).to_int() as i32
}

/// One of the five tonal palettes Android publishes (`system_accent1/2/3`,
/// `system_neutral1/2`). `tones` holds the 13 published tones in the order
/// [100, 99, 95, 90, 80, 70, 60, 50, 40, 30, 20, 10, 0].
struct SystemTonalPalette<'a> {
    tones: &'a [i32; 13],
}

impl SystemTonalPalette<'_> {
    fn tone(&self, tone: u8) -> i32 {
        const PUBLISHED: [u8; 13] = [100, 99, 95, 90, 80, 70, 60, 50, 40, 30, 20, 10, 0];
        match PUBLISHED.iter().position(|&t| t == tone) {
            Some(i) => self.tones[i],
            // Tones Android doesn't publish are derived like
            // `(*_40).setLuminance(t)` in `dynamicTonalPalette()`.
            None => set_luminance(self.tones[8], tone as f64),
        }
    }
}

/// The system palettes `dynamicLightColorScheme31()`/`dynamicDarkColorScheme31()`
/// map into color roles. Like upstream, the neutral palette (`system_neutral1_*`)
/// is unused: the 31-33 system neutral tones have too little chroma, so the
/// neutral-variant palette backs the neutral roles instead.
struct SystemPalettes<'a> {
    primary: &'a [i32; 13],
    secondary: &'a [i32; 13],
    tertiary: &'a [i32; 13],
    neutral_variant: &'a [i32; 13],
}

/// `dynamicLightColorScheme31`/`dynamicDarkColorScheme31` in
/// DynamicTonalPalette.android.kt at the pin.
fn android_scheme31(palettes: &SystemPalettes, dark: bool) -> MaterialColorScheme {
    let (p, s, tt, nv) = (
        SystemTonalPalette { tones: palettes.primary },
        SystemTonalPalette { tones: palettes.secondary },
        SystemTonalPalette { tones: palettes.tertiary },
        SystemTonalPalette { tones: palettes.neutral_variant },
    );
    let t = |pal: &SystemTonalPalette, tone: u8| color(pal.tone(tone) as i64);
    let (error, on_error, error_container, on_error_container) = android_baseline_error(dark);
    if !dark {
        MaterialColorScheme {
            primary: t(&p, 40),
            surface_tint: t(&p, 40),
            on_primary: t(&p, 100),
            primary_container: t(&p, 90),
            on_primary_container: t(&p, 10),
            secondary: t(&s, 40),
            on_secondary: t(&s, 100),
            secondary_container: t(&s, 90),
            on_secondary_container: t(&s, 10),
            tertiary: t(&tt, 40),
            on_tertiary: t(&tt, 100),
            tertiary_container: t(&tt, 90),
            on_tertiary_container: t(&tt, 10),
            error,
            on_error,
            error_container,
            on_error_container,
            background: t(&nv, 98),
            on_background: t(&nv, 10),
            surface: t(&nv, 98),
            on_surface: t(&nv, 10),
            surface_variant: t(&nv, 90),
            on_surface_variant: t(&nv, 30),
            outline: t(&nv, 50),
            outline_variant: t(&nv, 80),
            shadow: Color::from_argb_encoded(0xff000000),
            scrim: t(&nv, 0),
            inverse_surface: t(&nv, 20),
            inverse_on_surface: t(&nv, 95),
            inverse_primary: t(&p, 80),
            primary_fixed: t(&p, 90),
            on_primary_fixed: t(&p, 10),
            primary_fixed_dim: t(&p, 80),
            on_primary_fixed_variant: t(&p, 30),
            secondary_fixed: t(&s, 90),
            on_secondary_fixed: t(&s, 10),
            secondary_fixed_dim: t(&s, 80),
            on_secondary_fixed_variant: t(&s, 30),
            tertiary_fixed: t(&tt, 90),
            on_tertiary_fixed: t(&tt, 10),
            tertiary_fixed_dim: t(&tt, 80),
            on_tertiary_fixed_variant: t(&tt, 30),
            surface_dim: t(&nv, 87),
            surface_bright: t(&nv, 98),
            surface_container_lowest: t(&nv, 100),
            surface_container_low: t(&nv, 96),
            surface_container: t(&nv, 94),
            surface_container_high: t(&nv, 92),
            surface_container_highest: t(&nv, 90),
        }
    } else {
        MaterialColorScheme {
            primary: t(&p, 80),
            surface_tint: t(&p, 80),
            on_primary: t(&p, 20),
            primary_container: t(&p, 30),
            on_primary_container: t(&p, 90),
            secondary: t(&s, 80),
            on_secondary: t(&s, 20),
            secondary_container: t(&s, 30),
            on_secondary_container: t(&s, 90),
            tertiary: t(&tt, 80),
            on_tertiary: t(&tt, 20),
            tertiary_container: t(&tt, 30),
            on_tertiary_container: t(&tt, 90),
            error,
            on_error,
            error_container,
            on_error_container,
            background: t(&nv, 6),
            on_background: t(&nv, 90),
            surface: t(&nv, 6),
            on_surface: t(&nv, 90),
            surface_variant: t(&nv, 30),
            on_surface_variant: t(&nv, 80),
            outline: t(&nv, 60),
            outline_variant: t(&nv, 30),
            shadow: Color::from_argb_encoded(0xff000000),
            scrim: t(&nv, 0),
            inverse_surface: t(&nv, 90),
            inverse_on_surface: t(&nv, 20),
            inverse_primary: t(&p, 40),
            primary_fixed: t(&p, 90),
            on_primary_fixed: t(&p, 10),
            primary_fixed_dim: t(&p, 80),
            on_primary_fixed_variant: t(&p, 30),
            secondary_fixed: t(&s, 90),
            on_secondary_fixed: t(&s, 10),
            secondary_fixed_dim: t(&s, 80),
            on_secondary_fixed_variant: t(&s, 30),
            tertiary_fixed: t(&tt, 90),
            on_tertiary_fixed: t(&tt, 10),
            tertiary_fixed_dim: t(&tt, 80),
            on_tertiary_fixed_variant: t(&tt, 30),
            surface_dim: t(&nv, 6),
            surface_bright: t(&nv, 24),
            surface_container_lowest: t(&nv, 4),
            surface_container_low: t(&nv, 10),
            surface_container: t(&nv, 12),
            surface_container_high: t(&nv, 17),
            surface_container_highest: t(&nv, 22),
        }
    }
}

/// `dynamicLightColorScheme34`/`dynamicDarkColorScheme34` in
/// DynamicTonalPalette.android.kt at the pin: `colors` are the 43 role
/// resources in `lightColorScheme()`/`darkColorScheme()` argument order
/// (primary … onTertiaryFixedVariant, matching res/values-v34/styles.xml).
/// The error family, scrim and shadow aren't published by the system and stay
/// at the baseline `lightColorScheme()`/`darkColorScheme()` defaults
/// (ColorLightTokens.kt/ColorDarkTokens.kt; `shadow = Color.Black` in
/// ColorScheme.kt).
fn android_scheme34(colors: &[i32], dark: bool) -> MaterialColorScheme {
    debug_assert_eq!(colors.len(), 43);
    let mut roles = colors.iter().map(|&argb| color(argb as i64));
    let (error, on_error, error_container, on_error_container) = android_baseline_error(dark);
    MaterialColorScheme {
        primary: roles.next().unwrap_or_default(),
        on_primary: roles.next().unwrap_or_default(),
        primary_container: roles.next().unwrap_or_default(),
        on_primary_container: roles.next().unwrap_or_default(),
        inverse_primary: roles.next().unwrap_or_default(),
        secondary: roles.next().unwrap_or_default(),
        on_secondary: roles.next().unwrap_or_default(),
        secondary_container: roles.next().unwrap_or_default(),
        on_secondary_container: roles.next().unwrap_or_default(),
        tertiary: roles.next().unwrap_or_default(),
        on_tertiary: roles.next().unwrap_or_default(),
        tertiary_container: roles.next().unwrap_or_default(),
        on_tertiary_container: roles.next().unwrap_or_default(),
        background: roles.next().unwrap_or_default(),
        on_background: roles.next().unwrap_or_default(),
        surface: roles.next().unwrap_or_default(),
        on_surface: roles.next().unwrap_or_default(),
        surface_variant: roles.next().unwrap_or_default(),
        on_surface_variant: roles.next().unwrap_or_default(),
        inverse_surface: roles.next().unwrap_or_default(),
        inverse_on_surface: roles.next().unwrap_or_default(),
        outline: roles.next().unwrap_or_default(),
        outline_variant: roles.next().unwrap_or_default(),
        surface_bright: roles.next().unwrap_or_default(),
        surface_dim: roles.next().unwrap_or_default(),
        surface_container: roles.next().unwrap_or_default(),
        surface_container_high: roles.next().unwrap_or_default(),
        surface_container_highest: roles.next().unwrap_or_default(),
        surface_container_low: roles.next().unwrap_or_default(),
        surface_container_lowest: roles.next().unwrap_or_default(),
        surface_tint: roles.next().unwrap_or_default(),
        primary_fixed: roles.next().unwrap_or_default(),
        primary_fixed_dim: roles.next().unwrap_or_default(),
        on_primary_fixed: roles.next().unwrap_or_default(),
        on_primary_fixed_variant: roles.next().unwrap_or_default(),
        secondary_fixed: roles.next().unwrap_or_default(),
        secondary_fixed_dim: roles.next().unwrap_or_default(),
        on_secondary_fixed: roles.next().unwrap_or_default(),
        on_secondary_fixed_variant: roles.next().unwrap_or_default(),
        tertiary_fixed: roles.next().unwrap_or_default(),
        tertiary_fixed_dim: roles.next().unwrap_or_default(),
        on_tertiary_fixed: roles.next().unwrap_or_default(),
        on_tertiary_fixed_variant: roles.next().unwrap_or_default(),
        error,
        on_error,
        error_container,
        on_error_container,
        shadow: Color::from_argb_encoded(0xff000000),
        scrim: Color::from_argb_encoded(0xff000000),
    }
}

/// The baseline error family Compose leaves in place for system schemes
/// (ColorLightTokens.kt/ColorDarkTokens.kt at the pin).
fn android_baseline_error(dark: bool) -> (Color, Color, Color, Color) {
    let (error, on_error, error_container, on_error_container) = if dark {
        (0xfff2b8b5, 0xff601410, 0xff8c1d18, 0xfff9dedc)
    } else {
        (0xffb3261e, 0xffffffff, 0xfff9dedc, 0xff410e0b)
    };
    (
        Color::from_argb_encoded(error),
        Color::from_argb_encoded(on_error),
        Color::from_argb_encoded(error_container),
        Color::from_argb_encoded(on_error_container),
    )
}

/// Picks a seed color for a Material Design scheme from the pixels of an image,
/// with `QuantizerCelebi` and `Score` from material-color-utilities, like the
/// dynamic color scheme Android derives from the wallpaper.
///
/// Returns `None` when the image has no readable pixels.
pub fn seed_from_image(image: &Image) -> Option<Color> {
    let buffer = image.to_rgba8()?;
    let pixels: Vec<i64> = buffer
        .data
        .as_slice()
        .iter()
        .map(|p| ((p.a as i64) << 24) | ((p.r as i64) << 16) | ((p.g as i64) << 8) | p.b as i64)
        .collect();
    let result = QuantizerCelebi::quantize(&pixels, 128);
    material_color_utils::score::score_colors(&result.color_to_count)
        .first()
        .map(|&argb| color(argb))
}

/// `(kebab-case field name, color)` accessor table for `MaterialColorScheme`, in the
/// order the fields are declared. Used by language runtimes that enumerate the
/// roles dynamically.
const ROLE_COLORS: [(&str, fn(&MaterialColorScheme) -> Color); 49] = [
    ("primary", |s| s.primary),
    ("surface-tint", |s| s.surface_tint),
    ("on-primary", |s| s.on_primary),
    ("primary-container", |s| s.primary_container),
    ("on-primary-container", |s| s.on_primary_container),
    ("secondary", |s| s.secondary),
    ("on-secondary", |s| s.on_secondary),
    ("secondary-container", |s| s.secondary_container),
    ("on-secondary-container", |s| s.on_secondary_container),
    ("tertiary", |s| s.tertiary),
    ("on-tertiary", |s| s.on_tertiary),
    ("tertiary-container", |s| s.tertiary_container),
    ("on-tertiary-container", |s| s.on_tertiary_container),
    ("error", |s| s.error),
    ("on-error", |s| s.on_error),
    ("error-container", |s| s.error_container),
    ("on-error-container", |s| s.on_error_container),
    ("background", |s| s.background),
    ("on-background", |s| s.on_background),
    ("surface", |s| s.surface),
    ("on-surface", |s| s.on_surface),
    ("surface-variant", |s| s.surface_variant),
    ("on-surface-variant", |s| s.on_surface_variant),
    ("outline", |s| s.outline),
    ("outline-variant", |s| s.outline_variant),
    ("shadow", |s| s.shadow),
    ("scrim", |s| s.scrim),
    ("inverse-surface", |s| s.inverse_surface),
    ("inverse-on-surface", |s| s.inverse_on_surface),
    ("inverse-primary", |s| s.inverse_primary),
    ("primary-fixed", |s| s.primary_fixed),
    ("on-primary-fixed", |s| s.on_primary_fixed),
    ("primary-fixed-dim", |s| s.primary_fixed_dim),
    ("on-primary-fixed-variant", |s| s.on_primary_fixed_variant),
    ("secondary-fixed", |s| s.secondary_fixed),
    ("on-secondary-fixed", |s| s.on_secondary_fixed),
    ("secondary-fixed-dim", |s| s.secondary_fixed_dim),
    ("on-secondary-fixed-variant", |s| s.on_secondary_fixed_variant),
    ("tertiary-fixed", |s| s.tertiary_fixed),
    ("on-tertiary-fixed", |s| s.on_tertiary_fixed),
    ("tertiary-fixed-dim", |s| s.tertiary_fixed_dim),
    ("on-tertiary-fixed-variant", |s| s.on_tertiary_fixed_variant),
    ("surface-dim", |s| s.surface_dim),
    ("surface-bright", |s| s.surface_bright),
    ("surface-container-lowest", |s| s.surface_container_lowest),
    ("surface-container-low", |s| s.surface_container_low),
    ("surface-container", |s| s.surface_container),
    ("surface-container-high", |s| s.surface_container_high),
    ("surface-container-highest", |s| s.surface_container_highest),
];

impl MaterialColorScheme {
    /// Iterates the 49 color roles as `(kebab-case field name, color)` pairs.
    pub fn role_colors(&self) -> impl Iterator<Item = (&'static str, Color)> {
        ROLE_COLORS.iter().map(|(name, get)| (*name, get(self)))
    }
}
