// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT OR Apache-2.0

use alloc::vec::Vec;

use crate::dynamiccolor::SpecVersion;
use crate::dynamiccolor::Variant;
use crate::dynamiccolor::{DynamicScheme, Platform};
use crate::hct::Hct;
use crate::palettes::TonalPalette;

/// A Dynamic Color theme with 2 source colors.
pub struct SchemeCmf;

impl SchemeCmf {
    pub fn new(
        source_color_hct_list: Vec<Hct>,
        is_dark: bool,
        contrast_level: f64,
        spec_version: SpecVersion,
        platform: Platform,
    ) -> DynamicScheme {
        assert!(
            spec_version == SpecVersion::Spec2026,
            "SchemeCmf can only be used with spec version 2026."
        );
        DynamicScheme::new(
            source_color_hct_list.clone(),
            Variant::Cmf,
            is_dark,
            contrast_level,
            platform,
            spec_version,
            TonalPalette::from_hue_and_chroma(
                source_color_hct_list[0].hue(),
                source_color_hct_list[0].chroma(),
            ),
            TonalPalette::from_hue_and_chroma(
                source_color_hct_list[0].hue(),
                source_color_hct_list[0].chroma() * 0.5,
            ),
            tertiary_palette(&source_color_hct_list),
            TonalPalette::from_hue_and_chroma(
                source_color_hct_list[0].hue(),
                source_color_hct_list[0].chroma() * 0.2,
            ),
            TonalPalette::from_hue_and_chroma(
                source_color_hct_list[0].hue(),
                source_color_hct_list[0].chroma() * 0.2,
            ),
            TonalPalette::from_hue_and_chroma(
                get_error_hue(
                    source_color_hct_list[0].hue(),
                    tertiary_palette(&source_color_hct_list).hue,
                ),
                source_color_hct_list[0].chroma().max(50.0),
            ),
        )
    }

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

fn get_error_hue(primary_hue: f64, tertiary_hue: f64) -> f64 {
    if primary_hue <= 8.0 {
        if tertiary_hue <= 24.0 {
            28.0
        } else if tertiary_hue <= 32.0 {
            16.0
        } else {
            20.0
        }
    } else if primary_hue <= 16.0 {
        if tertiary_hue <= 24.0 {
            32.0
        } else if tertiary_hue <= 32.0 {
            20.0
        } else {
            24.0
        }
    } else if primary_hue <= 20.0 {
        if tertiary_hue <= 28.0 {
            32.0
        } else if tertiary_hue <= 32.0 {
            24.0
        } else {
            28.0
        }
    } else if primary_hue <= 28.0 {
        if tertiary_hue <= 24.0 { 32.0 } else { 16.0 }
    } else if primary_hue <= 32.0 {
        if tertiary_hue <= 20.0 {
            24.0
        } else if tertiary_hue <= 28.0 {
            16.0
        } else {
            20.0
        }
    } else if primary_hue <= 40.0 {
        if tertiary_hue > 20.0 && tertiary_hue <= 28.0 { 16.0 } else { 24.0 }
    } else if primary_hue <= 152.0 {
        if tertiary_hue > 24.0 && tertiary_hue <= 36.0 { 20.0 } else { 32.0 }
    } else if primary_hue <= 272.0 {
        if tertiary_hue > 20.0 && tertiary_hue <= 28.0 { 16.0 } else { 24.0 }
    } else {
        if tertiary_hue > 12.0 && tertiary_hue <= 28.0 { 32.0 } else { 16.0 }
    }
}

fn tertiary_palette(source_color_hct_list: &[Hct]) -> TonalPalette {
    let source_color_hct = source_color_hct_list[0];
    let secondary_source_color_hct =
        source_color_hct_list.get(1).copied().unwrap_or(source_color_hct);
    if source_color_hct.to_int() == secondary_source_color_hct.to_int() {
        TonalPalette::from_hue_and_chroma(source_color_hct.hue(), source_color_hct.chroma() * 0.75)
    } else {
        TonalPalette::from_hue_and_chroma(
            secondary_source_color_hct.hue(),
            secondary_source_color_hct.chroma(),
        )
    }
}
