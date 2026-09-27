// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::color_spec::ColorSpec;
use super::color_spec_2026::ColorSpec2026;
use super::dynamic_color::DynamicColor;
use super::dynamic_scheme::DynamicScheme;

/// Named colors, otherwise known as tokens, or roles, in the Material Design
/// system.
pub struct MaterialDynamicColors {
    color_spec: ColorSpec2026,
}

impl Default for MaterialDynamicColors {
    fn default() -> Self {
        Self::new()
    }
}

impl MaterialDynamicColors {
    pub fn new() -> Self {
        Self { color_spec: ColorSpec2026 }
    }

    pub fn highest_surface(&self, scheme: &DynamicScheme) -> DynamicColor {
        self.color_spec.highest_surface(scheme)
    }

    // ////////////////////////////////////////////////////////////////
    // Main Palettes //
    // ////////////////////////////////////////////////////////////////
    pub fn primary_palette_key_color(&self) -> DynamicColor {
        self.color_spec.primary_palette_key_color()
    }

    pub fn secondary_palette_key_color(&self) -> DynamicColor {
        self.color_spec.secondary_palette_key_color()
    }

    pub fn tertiary_palette_key_color(&self) -> DynamicColor {
        self.color_spec.tertiary_palette_key_color()
    }

    pub fn neutral_palette_key_color(&self) -> DynamicColor {
        self.color_spec.neutral_palette_key_color()
    }

    pub fn neutral_variant_palette_key_color(&self) -> DynamicColor {
        self.color_spec.neutral_variant_palette_key_color()
    }

    pub fn error_palette_key_color(&self) -> DynamicColor {
        self.color_spec.error_palette_key_color()
    }

    // ////////////////////////////////////////////////////////////////
    // Surfaces [S] //
    // ////////////////////////////////////////////////////////////////
    pub fn background(&self) -> DynamicColor {
        self.color_spec.background()
    }

    pub fn on_background(&self) -> DynamicColor {
        self.color_spec.on_background()
    }

    pub fn surface(&self) -> DynamicColor {
        self.color_spec.surface()
    }

    pub fn surface_dim(&self) -> DynamicColor {
        self.color_spec.surface_dim()
    }

    pub fn surface_bright(&self) -> DynamicColor {
        self.color_spec.surface_bright()
    }

    pub fn surface_container_lowest(&self) -> DynamicColor {
        self.color_spec.surface_container_lowest()
    }

    pub fn surface_container_low(&self) -> DynamicColor {
        self.color_spec.surface_container_low()
    }

    pub fn surface_container(&self) -> DynamicColor {
        self.color_spec.surface_container()
    }

    pub fn surface_container_high(&self) -> DynamicColor {
        self.color_spec.surface_container_high()
    }

    pub fn surface_container_highest(&self) -> DynamicColor {
        self.color_spec.surface_container_highest()
    }

    pub fn on_surface(&self) -> DynamicColor {
        self.color_spec.on_surface()
    }

    pub fn surface_variant(&self) -> DynamicColor {
        self.color_spec.surface_variant()
    }

    pub fn on_surface_variant(&self) -> DynamicColor {
        self.color_spec.on_surface_variant()
    }

    pub fn inverse_surface(&self) -> DynamicColor {
        self.color_spec.inverse_surface()
    }

    pub fn inverse_on_surface(&self) -> DynamicColor {
        self.color_spec.inverse_on_surface()
    }

    pub fn outline(&self) -> DynamicColor {
        self.color_spec.outline()
    }

    pub fn outline_variant(&self) -> DynamicColor {
        self.color_spec.outline_variant()
    }

    pub fn shadow(&self) -> DynamicColor {
        self.color_spec.shadow()
    }

    pub fn scrim(&self) -> DynamicColor {
        self.color_spec.scrim()
    }

    pub fn surface_tint(&self) -> DynamicColor {
        self.color_spec.surface_tint()
    }

    // ////////////////////////////////////////////////////////////////
    // Primaries [P] //
    // ////////////////////////////////////////////////////////////////
    pub fn primary(&self) -> DynamicColor {
        self.color_spec.primary()
    }

    pub fn primary_dim(&self) -> Option<DynamicColor> {
        self.color_spec.primary_dim()
    }

    pub fn on_primary(&self) -> DynamicColor {
        self.color_spec.on_primary()
    }

    pub fn primary_container(&self) -> DynamicColor {
        self.color_spec.primary_container()
    }

    pub fn on_primary_container(&self) -> DynamicColor {
        self.color_spec.on_primary_container()
    }

    pub fn inverse_primary(&self) -> DynamicColor {
        self.color_spec.inverse_primary()
    }

    // ////////////////////////////////////////////////////////////////
    // Secondaries [Q] //
    // ////////////////////////////////////////////////////////////////
    pub fn secondary(&self) -> DynamicColor {
        self.color_spec.secondary()
    }

    pub fn secondary_dim(&self) -> Option<DynamicColor> {
        self.color_spec.secondary_dim()
    }

    pub fn on_secondary(&self) -> DynamicColor {
        self.color_spec.on_secondary()
    }

    pub fn secondary_container(&self) -> DynamicColor {
        self.color_spec.secondary_container()
    }

    pub fn on_secondary_container(&self) -> DynamicColor {
        self.color_spec.on_secondary_container()
    }

    // ////////////////////////////////////////////////////////////////
    // Tertiaries [T] //
    // ////////////////////////////////////////////////////////////////
    pub fn tertiary(&self) -> DynamicColor {
        self.color_spec.tertiary()
    }

    pub fn tertiary_dim(&self) -> Option<DynamicColor> {
        self.color_spec.tertiary_dim()
    }

    pub fn on_tertiary(&self) -> DynamicColor {
        self.color_spec.on_tertiary()
    }

    pub fn tertiary_container(&self) -> DynamicColor {
        self.color_spec.tertiary_container()
    }

    pub fn on_tertiary_container(&self) -> DynamicColor {
        self.color_spec.on_tertiary_container()
    }

    // ////////////////////////////////////////////////////////////////
    // Errors [E] //
    // ////////////////////////////////////////////////////////////////
    pub fn error(&self) -> DynamicColor {
        self.color_spec.error()
    }

    pub fn error_dim(&self) -> Option<DynamicColor> {
        self.color_spec.error_dim()
    }

    pub fn on_error(&self) -> DynamicColor {
        self.color_spec.on_error()
    }

    pub fn error_container(&self) -> DynamicColor {
        self.color_spec.error_container()
    }

    pub fn on_error_container(&self) -> DynamicColor {
        self.color_spec.on_error_container()
    }

    // ////////////////////////////////////////////////////////////////
    // Primary Fixed Colors [PF] //
    // ////////////////////////////////////////////////////////////////
    pub fn primary_fixed(&self) -> DynamicColor {
        self.color_spec.primary_fixed()
    }

    pub fn primary_fixed_dim(&self) -> DynamicColor {
        self.color_spec.primary_fixed_dim()
    }

    pub fn on_primary_fixed(&self) -> DynamicColor {
        self.color_spec.on_primary_fixed()
    }

    pub fn on_primary_fixed_variant(&self) -> DynamicColor {
        self.color_spec.on_primary_fixed_variant()
    }

    // ////////////////////////////////////////////////////////////////
    // Secondary Fixed Colors [QF] //
    // ////////////////////////////////////////////////////////////////
    pub fn secondary_fixed(&self) -> DynamicColor {
        self.color_spec.secondary_fixed()
    }

    pub fn secondary_fixed_dim(&self) -> DynamicColor {
        self.color_spec.secondary_fixed_dim()
    }

    pub fn on_secondary_fixed(&self) -> DynamicColor {
        self.color_spec.on_secondary_fixed()
    }

    pub fn on_secondary_fixed_variant(&self) -> DynamicColor {
        self.color_spec.on_secondary_fixed_variant()
    }

    // ////////////////////////////////////////////////////////////////
    // Tertiary Fixed Colors [TF] //
    // ////////////////////////////////////////////////////////////////
    pub fn tertiary_fixed(&self) -> DynamicColor {
        self.color_spec.tertiary_fixed()
    }

    pub fn tertiary_fixed_dim(&self) -> DynamicColor {
        self.color_spec.tertiary_fixed_dim()
    }

    pub fn on_tertiary_fixed(&self) -> DynamicColor {
        self.color_spec.on_tertiary_fixed()
    }

    pub fn on_tertiary_fixed_variant(&self) -> DynamicColor {
        self.color_spec.on_tertiary_fixed_variant()
    }
}
