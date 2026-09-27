// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT OR Apache-2.0

use alloc::vec::Vec;

use crate::dynamiccolor::ColorSpecs;
use crate::dynamiccolor::SpecVersion;
use crate::dynamiccolor::Variant;
use crate::dynamiccolor::{DynamicScheme, Platform};
use crate::hct::Hct;

/// A playful theme - the source color's hue does not appear in the theme.
pub struct SchemeFruitSalad;

impl SchemeFruitSalad {
    /// Creates a new scheme for a list of source colors.
    ///
    /// `spec_version` defaults to `DynamicScheme::DEFAULT_SPEC_VERSION` and
    /// `platform` to `DynamicScheme::DEFAULT_PLATFORM` when constructed via
    /// language bindings.
    pub fn new(
        source_color_hct_list: Vec<Hct>,
        is_dark: bool,
        contrast_level: f64,
        spec_version: SpecVersion,
        platform: Platform,
    ) -> DynamicScheme {
        let spec = ColorSpecs::get(spec_version);
        let source_color_hct = source_color_hct_list[0];
        DynamicScheme::new(
            source_color_hct_list,
            Variant::FruitSalad,
            is_dark,
            contrast_level,
            platform,
            spec_version,
            spec.get_primary_palette(
                Variant::FruitSalad,
                source_color_hct,
                is_dark,
                platform,
                contrast_level,
            ),
            spec.get_secondary_palette(
                Variant::FruitSalad,
                source_color_hct,
                is_dark,
                platform,
                contrast_level,
            ),
            spec.get_tertiary_palette(
                Variant::FruitSalad,
                source_color_hct,
                is_dark,
                platform,
                contrast_level,
            ),
            spec.get_neutral_palette(
                Variant::FruitSalad,
                source_color_hct,
                is_dark,
                platform,
                contrast_level,
            ),
            spec.get_neutral_variant_palette(
                Variant::FruitSalad,
                source_color_hct,
                is_dark,
                platform,
                contrast_level,
            ),
            spec.get_error_palette(
                Variant::FruitSalad,
                source_color_hct,
                is_dark,
                platform,
                contrast_level,
            ),
        )
    }

    /// Creates a new scheme for a single source color.
    pub fn from_hct(
        source_color_hct: Hct,
        is_dark: bool,
        contrast_level: f64,
        spec_version: SpecVersion,
        platform: Platform,
    ) -> DynamicScheme {
        Self::new(alloc::vec![source_color_hct], is_dark, contrast_level, spec_version, platform)
    }
}
