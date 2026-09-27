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
    let contrast_level = contrast_level as f64;
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
