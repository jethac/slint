// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Port of kotlin/palettes/CorePalettes.kt (material-color-utilities @ 5b3618b).

use crate::palettes::TonalPalette;

/// Comprises foundational palettes to build a color scheme.
///
/// Generated from a source color, these palettes will then be part of a
/// `DynamicScheme` together with appearance preferences.
pub struct CorePalettes {
    /// Primary palette.
    pub primary: TonalPalette,
    /// Secondary palette.
    pub secondary: TonalPalette,
    /// Tertiary palette.
    pub tertiary: TonalPalette,
    /// Neutral palette.
    pub neutral: TonalPalette,
    /// Neutral variant palette.
    pub neutral_variant: TonalPalette,
}
