// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Port of kotlin/hct/Hct.kt (material-color-utilities @ 5b3618b).

use crate::Argb;
use crate::hct::Cam16;
use crate::hct::HctSolver;
use crate::hct::ViewingConditions;
use crate::math;
use crate::utils::ColorUtils;

/// HCT, hue, chroma, and tone. A color system that provides a perceptually
/// accurate color measurement system that can also accurately render what
/// colors will appear as in different lighting environments.
///
/// HCT is built using CAM16 hue and chroma, and L* from L*a*b*.
///
/// Using L* creates a link between the color system, contrast, and thus
/// accessibility. Contrast ratio depends on relative luminance, or Y in the
/// XYZ color space. L*, or perceptual luminance can be calculated from Y.
///
/// Unlike Y, L* is linear to human perception, allowing trivial creation of
/// accurate color tones.
///
/// Unlike contrast ratio, measuring contrast in L* is linear, and simple to
/// calculate. A difference of 40 in HCT tone guarantees a contrast ratio >=
/// 3.0, and a difference of 50 guarantees a contrast ratio >= 4.5.
#[derive(Clone, Copy, Debug)]
pub struct Hct {
    hue: f64,
    chroma: f64,
    tone: f64,
    argb: Argb,
}

impl Hct {
    /// The hue of this color, in degrees.
    pub fn hue(&self) -> f64 {
        self.hue
    }

    /// The chroma of this color.
    pub fn chroma(&self) -> f64 {
        self.chroma
    }

    /// The tone of this color: perceptual luminance, 0-100.
    pub fn tone(&self) -> f64 {
        self.tone
    }

    /// ARGB representation of this color.
    pub fn to_int(&self) -> Argb {
        self.argb
    }

    /// Set the hue of this color. Chroma may decrease because chroma has a
    /// different maximum for any given hue and tone.
    ///
    /// `new_hue` is 0 <= new_hue < 360; invalid values are corrected.
    pub fn set_hue(&mut self, new_hue: f64) {
        self.set_internal_state(HctSolver::solve_to_int(new_hue, self.chroma, self.tone));
    }

    /// Set the chroma of this color. Chroma may decrease because chroma has a
    /// different maximum for any given hue and tone.
    ///
    /// `new_chroma` is 0 <= new_chroma < ?
    pub fn set_chroma(&mut self, new_chroma: f64) {
        self.set_internal_state(HctSolver::solve_to_int(self.hue, new_chroma, self.tone));
    }

    /// Set the tone of this color. Chroma may decrease because chroma has a
    /// different maximum for any given hue and tone.
    ///
    /// `new_tone` is 0 <= new_tone <= 100; invalid values are corrected.
    pub fn set_tone(&mut self, new_tone: f64) {
        self.set_internal_state(HctSolver::solve_to_int(self.hue, self.chroma, new_tone));
    }

    /// Translate a color into different ViewingConditions.
    ///
    /// Colors change appearance. They look different with lights on versus
    /// off, the same color, as in hex code, on white looks different when on
    /// black. This is called color relativity, most famously explicated by
    /// Josef Albers in Interaction of Color.
    ///
    /// In color science, color appearance models can account for and calculate
    /// the appearance of a color in different settings. HCT is based on
    /// CAM16, a color appearance model, and uses it to make these
    /// calculations.
    ///
    /// See `ViewingConditions::make` for parameters affecting color
    /// appearance.
    pub fn in_viewing_conditions(&self, vc: &ViewingConditions) -> Hct {
        // 1. Use CAM16 to find XYZ coordinates of color in specified VC.
        let cam16 = Cam16::from_int(self.to_int());
        let viewed_in_vc = cam16.xyz_in_viewing_conditions(vc);

        // 2. Create CAM16 of those XYZ coordinates in default VC.
        let recast_in_vc = Cam16::from_xyz_in_viewing_conditions(
            viewed_in_vc[0],
            viewed_in_vc[1],
            viewed_in_vc[2],
            &ViewingConditions::default(),
        );

        // 3. Create HCT from:
        // - CAM16 using default VC with XYZ coordinates in specified VC.
        // - L* converted from Y in XYZ coordinates in specified VC.
        Hct::from(recast_in_vc.hue, recast_in_vc.chroma, ColorUtils::lstar_from_y(viewed_in_vc[1]))
    }

    fn set_internal_state(&mut self, argb: Argb) {
        self.argb = argb;
        let cam = Cam16::from_int(argb);
        self.hue = cam.hue;
        self.chroma = cam.chroma;
        self.tone = ColorUtils::lstar_from_argb(argb);
    }

    /// Create an HCT color from hue, chroma, and tone.
    ///
    /// `hue` is 0 <= hue < 360; invalid values are corrected.
    /// `chroma` is 0 <= chroma < ?; Informally, colorfulness. The color
    ///   returned may be lower than the requested chroma. Chroma has a
    ///   different maximum for any given hue and tone.
    /// `tone` is 0 <= tone <= 100; invalid values are corrected.
    ///
    /// Returns HCT representation of a color in default viewing conditions.
    pub fn from(hue: f64, chroma: f64, tone: f64) -> Hct {
        let argb = HctSolver::solve_to_int(hue, chroma, tone);
        Hct::from_int(argb)
    }

    /// Create an HCT color from a color.
    ///
    /// `argb` is the ARGB representation of a color.
    ///
    /// Returns HCT representation of a color in default viewing conditions.
    pub fn from_int(argb: Argb) -> Hct {
        let mut hct = Hct { hue: 0.0, chroma: 0.0, tone: 0.0, argb: 0 };
        hct.set_internal_state(argb);
        hct
    }

    /// Whether the hue is in the blue region: 250 <= hue < 270.
    pub fn is_blue(hue: f64) -> bool {
        (250.0..270.0).contains(&hue)
    }

    /// Whether the hue is in the yellow region: 105 <= hue < 125.
    pub fn is_yellow(hue: f64) -> bool {
        (105.0..125.0).contains(&hue)
    }

    /// Whether the hue is in the cyan region: 170 <= hue < 207.
    pub fn is_cyan(hue: f64) -> bool {
        (170.0..207.0).contains(&hue)
    }
}

impl core::fmt::Display for Hct {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "HCT({}, {}, {})",
            math::round_to_int(self.hue),
            math::round_to_int(self.chroma),
            math::round_to_int(self.tone)
        )
    }
}
