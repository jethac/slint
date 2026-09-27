// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::color_spec::{ColorSpec, SpecVersion};
use super::color_spec_2021::ColorSpec2021;
use super::color_spec_2025::ColorSpec2025;
use super::color_spec_2026::ColorSpec2026;

/// A utility class to get the correct color spec for a given spec version.
pub struct ColorSpecs;

static SPEC_2021: ColorSpec2021 = ColorSpec2021;
static SPEC_2025: ColorSpec2025 = ColorSpec2025;
static SPEC_2026: ColorSpec2026 = ColorSpec2026;

impl ColorSpecs {
    pub fn default_spec() -> &'static dyn ColorSpec {
        Self::get(SpecVersion::Spec2021)
    }

    pub fn get(spec_version: SpecVersion) -> &'static dyn ColorSpec {
        match spec_version {
            SpecVersion::Spec2025 => &SPEC_2025,
            SpecVersion::Spec2026 => &SPEC_2026,
            _ => &SPEC_2021,
        }
    }

    /// `is_extended_fidelity` exists for parity with the Java API surface.
    /// It does not change dispatch.
    pub fn get_with_fidelity(
        spec_version: SpecVersion,
        _is_extended_fidelity: bool,
    ) -> &'static dyn ColorSpec {
        Self::get(spec_version)
    }
}
