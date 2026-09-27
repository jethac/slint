// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT OR Apache-2.0

use alloc::rc::Rc;
use alloc::vec::Vec;

use crate::Argb;
use crate::hct::Hct;
use crate::palettes::TonalPalette;
use crate::utils::MathUtils;

use super::color_spec::SpecVersion;
use super::dynamic_color::DynamicColor;
use super::material_dynamic_colors::MaterialDynamicColors;
use super::variant::Variant;

/// The platform on which this scheme is intended to be used.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Platform {
    Phone,
    Watch,
}

/// Constructed by a set of values representing the current UI state (such as
/// whether or not its dark mode, what the theme style is, etc.), and
/// provides a set of TonalPalettes that can create colors that fit in
/// with the theme.
pub struct DynamicScheme {
    /// The source colors of the scheme in HCT format. The first element is
    /// the primary source color; a second element, when present, is the
    /// secondary source color used by the CMF variant.
    pub source_color_hct_list: Vec<Hct>,
    /// The variant of the scheme.
    pub variant: Variant,
    /// Whether or not the scheme is in dark mode.
    pub is_dark: bool,
    /// Value from -1 to 1. -1 represents minimum contrast, 0 represents
    /// standard (i.e. the design as spec'd), and 1 represents maximum contrast.
    pub contrast_level: f64,
    /// The platform on which this scheme is intended to be used.
    pub platform: Platform,
    /// The spec version of the scheme, after any variant-mandated fallback.
    pub spec_version: SpecVersion,
    /// Given a tone, produces a color. Hue and chroma of the color are
    /// specified in the design specification of the scheme. Usually colorful.
    pub primary_palette: Rc<TonalPalette>,
    /// Given a tone, produces a color. Hue and chroma of the color are
    /// specified in the design specification of the scheme. Usually less
    /// colorful than primaryPalette.
    pub secondary_palette: Rc<TonalPalette>,
    /// Given a tone, produces a color. Hue and chroma of the color are
    /// specified in the design specification of the scheme. Usually a
    /// different hue than primaryPalette.
    pub tertiary_palette: Rc<TonalPalette>,
    /// Given a tone, produces a color. Hue and chroma of the color are
    /// specified in the design specification of the scheme. Usually not
    /// colorful at all, intended for background & surface colors.
    pub neutral_palette: Rc<TonalPalette>,
    /// Given a tone, produces a color. Hue and chroma of the color are
    /// specified in the design specification of the scheme. Usually not
    /// colorful, but slightly more colorful than Neutral. Intended for
    /// backgrounds & surfaces.
    pub neutral_variant_palette: Rc<TonalPalette>,
    /// Given a tone, produces a reddish, colorful, color.
    pub error_palette: Rc<TonalPalette>,

    dynamic_colors: MaterialDynamicColors,
}

impl DynamicScheme {
    pub const DEFAULT_SPEC_VERSION: SpecVersion = SpecVersion::Spec2021;
    pub const DEFAULT_PLATFORM: Platform = Platform::Phone;

    #[allow(clippy::too_many_arguments)]
    pub fn new(
        source_color_hct_list: Vec<Hct>,
        variant: Variant,
        is_dark: bool,
        contrast_level: f64,
        platform: Platform,
        spec_version: SpecVersion,
        primary_palette: TonalPalette,
        secondary_palette: TonalPalette,
        tertiary_palette: TonalPalette,
        neutral_palette: TonalPalette,
        neutral_variant_palette: TonalPalette,
        error_palette: TonalPalette,
    ) -> Self {
        assert!(!source_color_hct_list.is_empty(), "source_color_hct_list cannot be empty");
        let spec_version = Self::maybe_fallback_spec_version(spec_version, variant);
        Self {
            source_color_hct_list,
            variant,
            is_dark,
            contrast_level,
            platform,
            spec_version,
            primary_palette: Rc::new(primary_palette),
            secondary_palette: Rc::new(secondary_palette),
            tertiary_palette: Rc::new(tertiary_palette),
            neutral_palette: Rc::new(neutral_palette),
            neutral_variant_palette: Rc::new(neutral_variant_palette),
            error_palette: Rc::new(error_palette),
            dynamic_colors: MaterialDynamicColors::new(),
        }
    }

    /// The source color of the scheme in HCT format.
    pub fn source_color_hct(&self) -> Hct {
        self.source_color_hct_list[0]
    }

    /// The source color of the scheme in ARGB format.
    pub fn source_color_argb(&self) -> Argb {
        self.source_color_hct().to_int()
    }

    pub fn get_hct(&self, dynamic_color: &DynamicColor) -> Hct {
        dynamic_color.get_hct(self)
    }

    pub fn get_argb(&self, dynamic_color: &DynamicColor) -> Argb {
        dynamic_color.get_argb(self)
    }

    /// Named colors, otherwise known as tokens, or roles, in the Material
    /// Design system.
    pub fn colors(&self) -> &MaterialDynamicColors {
        &self.dynamic_colors
    }

    pub fn primary_palette_key_color(&self) -> Argb {
        self.get_argb(&self.dynamic_colors.primary_palette_key_color())
    }
    pub fn secondary_palette_key_color(&self) -> Argb {
        self.get_argb(&self.dynamic_colors.secondary_palette_key_color())
    }
    pub fn tertiary_palette_key_color(&self) -> Argb {
        self.get_argb(&self.dynamic_colors.tertiary_palette_key_color())
    }
    pub fn neutral_palette_key_color(&self) -> Argb {
        self.get_argb(&self.dynamic_colors.neutral_palette_key_color())
    }
    pub fn neutral_variant_palette_key_color(&self) -> Argb {
        self.get_argb(&self.dynamic_colors.neutral_variant_palette_key_color())
    }
    pub fn background(&self) -> Argb {
        self.get_argb(&self.dynamic_colors.background())
    }
    pub fn on_background(&self) -> Argb {
        self.get_argb(&self.dynamic_colors.on_background())
    }
    pub fn surface(&self) -> Argb {
        self.get_argb(&self.dynamic_colors.surface())
    }
    pub fn surface_dim(&self) -> Argb {
        self.get_argb(&self.dynamic_colors.surface_dim())
    }
    pub fn surface_bright(&self) -> Argb {
        self.get_argb(&self.dynamic_colors.surface_bright())
    }
    pub fn surface_container_lowest(&self) -> Argb {
        self.get_argb(&self.dynamic_colors.surface_container_lowest())
    }
    pub fn surface_container_low(&self) -> Argb {
        self.get_argb(&self.dynamic_colors.surface_container_low())
    }
    pub fn surface_container(&self) -> Argb {
        self.get_argb(&self.dynamic_colors.surface_container())
    }
    pub fn surface_container_high(&self) -> Argb {
        self.get_argb(&self.dynamic_colors.surface_container_high())
    }
    pub fn surface_container_highest(&self) -> Argb {
        self.get_argb(&self.dynamic_colors.surface_container_highest())
    }
    pub fn on_surface(&self) -> Argb {
        self.get_argb(&self.dynamic_colors.on_surface())
    }
    pub fn surface_variant(&self) -> Argb {
        self.get_argb(&self.dynamic_colors.surface_variant())
    }
    pub fn on_surface_variant(&self) -> Argb {
        self.get_argb(&self.dynamic_colors.on_surface_variant())
    }
    pub fn inverse_surface(&self) -> Argb {
        self.get_argb(&self.dynamic_colors.inverse_surface())
    }
    pub fn inverse_on_surface(&self) -> Argb {
        self.get_argb(&self.dynamic_colors.inverse_on_surface())
    }
    pub fn outline(&self) -> Argb {
        self.get_argb(&self.dynamic_colors.outline())
    }
    pub fn outline_variant(&self) -> Argb {
        self.get_argb(&self.dynamic_colors.outline_variant())
    }
    pub fn shadow(&self) -> Argb {
        self.get_argb(&self.dynamic_colors.shadow())
    }
    pub fn scrim(&self) -> Argb {
        self.get_argb(&self.dynamic_colors.scrim())
    }
    pub fn surface_tint(&self) -> Argb {
        self.get_argb(&self.dynamic_colors.surface_tint())
    }
    pub fn primary(&self) -> Argb {
        self.get_argb(&self.dynamic_colors.primary())
    }
    pub fn on_primary(&self) -> Argb {
        self.get_argb(&self.dynamic_colors.on_primary())
    }
    pub fn primary_container(&self) -> Argb {
        self.get_argb(&self.dynamic_colors.primary_container())
    }
    pub fn on_primary_container(&self) -> Argb {
        self.get_argb(&self.dynamic_colors.on_primary_container())
    }
    pub fn inverse_primary(&self) -> Argb {
        self.get_argb(&self.dynamic_colors.inverse_primary())
    }
    pub fn secondary(&self) -> Argb {
        self.get_argb(&self.dynamic_colors.secondary())
    }
    pub fn on_secondary(&self) -> Argb {
        self.get_argb(&self.dynamic_colors.on_secondary())
    }
    pub fn secondary_container(&self) -> Argb {
        self.get_argb(&self.dynamic_colors.secondary_container())
    }
    pub fn on_secondary_container(&self) -> Argb {
        self.get_argb(&self.dynamic_colors.on_secondary_container())
    }
    pub fn tertiary(&self) -> Argb {
        self.get_argb(&self.dynamic_colors.tertiary())
    }
    pub fn on_tertiary(&self) -> Argb {
        self.get_argb(&self.dynamic_colors.on_tertiary())
    }
    pub fn tertiary_container(&self) -> Argb {
        self.get_argb(&self.dynamic_colors.tertiary_container())
    }
    pub fn on_tertiary_container(&self) -> Argb {
        self.get_argb(&self.dynamic_colors.on_tertiary_container())
    }
    pub fn error(&self) -> Argb {
        self.get_argb(&self.dynamic_colors.error())
    }
    pub fn on_error(&self) -> Argb {
        self.get_argb(&self.dynamic_colors.on_error())
    }
    pub fn error_container(&self) -> Argb {
        self.get_argb(&self.dynamic_colors.error_container())
    }
    pub fn on_error_container(&self) -> Argb {
        self.get_argb(&self.dynamic_colors.on_error_container())
    }
    pub fn primary_fixed(&self) -> Argb {
        self.get_argb(&self.dynamic_colors.primary_fixed())
    }
    pub fn primary_fixed_dim(&self) -> Argb {
        self.get_argb(&self.dynamic_colors.primary_fixed_dim())
    }
    pub fn on_primary_fixed(&self) -> Argb {
        self.get_argb(&self.dynamic_colors.on_primary_fixed())
    }
    pub fn on_primary_fixed_variant(&self) -> Argb {
        self.get_argb(&self.dynamic_colors.on_primary_fixed_variant())
    }
    pub fn secondary_fixed(&self) -> Argb {
        self.get_argb(&self.dynamic_colors.secondary_fixed())
    }
    pub fn secondary_fixed_dim(&self) -> Argb {
        self.get_argb(&self.dynamic_colors.secondary_fixed_dim())
    }
    pub fn on_secondary_fixed(&self) -> Argb {
        self.get_argb(&self.dynamic_colors.on_secondary_fixed())
    }
    pub fn on_secondary_fixed_variant(&self) -> Argb {
        self.get_argb(&self.dynamic_colors.on_secondary_fixed_variant())
    }
    pub fn tertiary_fixed(&self) -> Argb {
        self.get_argb(&self.dynamic_colors.tertiary_fixed())
    }
    pub fn tertiary_fixed_dim(&self) -> Argb {
        self.get_argb(&self.dynamic_colors.tertiary_fixed_dim())
    }
    pub fn on_tertiary_fixed(&self) -> Argb {
        self.get_argb(&self.dynamic_colors.on_tertiary_fixed())
    }
    pub fn on_tertiary_fixed_variant(&self) -> Argb {
        self.get_argb(&self.dynamic_colors.on_tertiary_fixed_variant())
    }

    pub fn from(other: &DynamicScheme, is_dark: bool) -> DynamicScheme {
        Self::from_with_contrast(other, is_dark, other.contrast_level)
    }

    pub fn from_with_contrast(
        other: &DynamicScheme,
        is_dark: bool,
        contrast_level: f64,
    ) -> DynamicScheme {
        DynamicScheme {
            source_color_hct_list: other.source_color_hct_list.clone(),
            variant: other.variant,
            is_dark,
            contrast_level,
            platform: other.platform,
            spec_version: other.spec_version,
            primary_palette: Rc::clone(&other.primary_palette),
            secondary_palette: Rc::clone(&other.secondary_palette),
            tertiary_palette: Rc::clone(&other.tertiary_palette),
            neutral_palette: Rc::clone(&other.neutral_palette),
            neutral_variant_palette: Rc::clone(&other.neutral_variant_palette),
            error_palette: Rc::clone(&other.error_palette),
            dynamic_colors: MaterialDynamicColors::new(),
        }
    }

    /// Given a set of hues and hue rotations, returns which hue falls under
    /// which rotation.
    ///
    /// `hue_breakpoints` defines the hue intervals, `hues` the hue to use
    /// within each interval. If the source color's hue doesn't fall under any
    /// interval, the source color's hue is returned.
    pub fn get_piecewise_value(
        source_color_hct: Hct,
        hue_breakpoints: &[f64],
        hues: &[f64],
    ) -> f64 {
        let size = hue_breakpoints.len().saturating_sub(1).min(hues.len());
        let source_hue = source_color_hct.hue();
        for i in 0..size {
            if source_hue >= hue_breakpoints[i] && source_hue < hue_breakpoints[i + 1] {
                return MathUtils::sanitize_degrees_double(hues[i]);
            }
        }
        // No condition matched, return the source value.
        source_hue
    }

    /// Given a source color and a list of hue breakpoints and rotations,
    /// returns the hue rotated by the amount specified in the interval the
    /// source hue falls into.
    pub fn get_rotated_hue(
        source_color_hct: Hct,
        hue_breakpoints: &[f64],
        rotations: &[f64],
    ) -> f64 {
        let mut rotation = Self::get_piecewise_value(source_color_hct, hue_breakpoints, rotations);
        if hue_breakpoints.len().saturating_sub(1).min(rotations.len()) == 0 {
            // No condition matched, return the source hue.
            rotation = 0.0;
        }
        MathUtils::sanitize_degrees_double(source_color_hct.hue() + rotation)
    }

    fn maybe_fallback_spec_version(spec_version: SpecVersion, variant: Variant) -> SpecVersion {
        if variant == Variant::Cmf {
            return spec_version;
        }
        if variant == Variant::Expressive
            || variant == Variant::Vibrant
            || variant == Variant::TonalSpot
            || variant == Variant::Neutral
        {
            return if spec_version == SpecVersion::Spec2026 {
                SpecVersion::Spec2025
            } else {
                spec_version
            };
        }
        SpecVersion::Spec2021
    }
}
