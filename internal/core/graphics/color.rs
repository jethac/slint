// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell:ignore oklab unquantize cbrt

/*!
This module contains color related types for the run-time library.
*/

use crate::properties::InterpolatedPropertyValue;
#[cfg(not(feature = "std"))]
#[allow(unused_imports)]
use num_traits::Float;

/// RgbaColor stores the red, green, blue and alpha components of a color
/// with the precision of the generic parameter T. For example if T is f32,
/// the values are normalized between 0 and 1. If T is u8, they values range
/// is 0 to 255.
/// This is merely a helper class for use with [`Color`].
#[derive(Copy, Clone, PartialEq, Debug, Default)]
pub struct RgbaColor<T> {
    /// The alpha component.
    pub alpha: T,
    /// The red channel.
    pub red: T,
    /// The green channel.
    pub green: T,
    /// The blue channel.
    pub blue: T,
}

#[cfg(feature = "32-bit-color")]
type Channel = f32;
#[cfg(not(feature = "32-bit-color"))]
type Channel = u8;

/// Color represents a color in the Slint run-time, represented using 8-bit channels for
/// red, green, blue and the alpha (opacity).
/// It can be conveniently converted using the `to_` and `from_` (a)rgb helper functions:
/// ```
/// # fn do_something_with_red_and_green(_:f32, _:f32) {}
/// # fn do_something_with_red(_:u8) {}
/// # use i_slint_core::graphics::{Color, RgbaColor};
/// # let some_color = Color::from_rgb_u8(0, 0, 0);
/// let col = some_color.to_argb_f32();
/// do_something_with_red_and_green(col.red, col.green);
///
/// let RgbaColor { red, blue, green, .. } = some_color.to_argb_u8();
/// do_something_with_red(red);
///
/// let new_col = Color::from(RgbaColor{ red: 0.5, green: 0.65, blue: 0.32, alpha: 1.});
/// ```
#[derive(Copy, Clone, PartialEq, PartialOrd, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(C)]
pub struct Color {
    red: Channel,
    green: Channel,
    blue: Channel,
    alpha: Channel,
}

// Can't use f32::round() yet because the Float trait in num_traits isn't const
// (when building with libm)
const fn round(mut value: f32) -> u8 {
    if value % 1.0 > 0.5 {
        value += 0.5;
    }

    value as _
}

const fn quantize(value: f32) -> u8 {
    round(value * 255.0)
}

const fn unquantize(value: u8) -> f32 {
    (value as f32) / 255.0
}

#[test]
fn unquantize_roundtrip() {
    for v in 0..=255 {
        assert_eq!(v, quantize(unquantize(v)));
    }
}

impl From<RgbaColor<u8>> for RgbaColor<f32> {
    #[inline]
    fn from(col: RgbaColor<u8>) -> Self {
        Self {
            red: unquantize(col.red),
            green: unquantize(col.green),
            blue: unquantize(col.blue),
            alpha: unquantize(col.alpha),
        }
    }
}

impl From<RgbaColor<f32>> for RgbaColor<u8> {
    #[inline]
    fn from(col: RgbaColor<f32>) -> Self {
        Self {
            red: quantize(col.red),
            green: quantize(col.green),
            blue: quantize(col.blue),
            alpha: quantize(col.alpha),
        }
    }
}

impl From<Color> for RgbaColor<f32> {
    #[inline]
    fn from(col: Color) -> Self {
        #[cfg(feature = "32-bit-color")]
        {
            Self { red: col.red, green: col.green, blue: col.blue, alpha: col.alpha }
        }
        #[cfg(not(feature = "32-bit-color"))]
        {
            let col: RgbaColor<u8> = col.into();
            col.into()
        }
    }
}

impl From<RgbaColor<f32>> for Color {
    #[inline]
    fn from(col: RgbaColor<f32>) -> Self {
        #[cfg(feature = "32-bit-color")]
        {
            Self { red: col.red, green: col.green, blue: col.blue, alpha: col.alpha }
        }
        #[cfg(not(feature = "32-bit-color"))]
        {
            let col: RgbaColor<u8> = col.into();
            col.into()
        }
    }
}

impl From<RgbaColor<u8>> for Color {
    #[inline]
    fn from(col: RgbaColor<u8>) -> Self {
        #[cfg(feature = "32-bit-color")]
        {
            let col: RgbaColor<f32> = col.into();
            col.into()
        }
        #[cfg(not(feature = "32-bit-color"))]
        {
            Self { red: col.red, green: col.green, blue: col.blue, alpha: col.alpha }
        }
    }
}

impl From<Color> for RgbaColor<u8> {
    #[inline]
    fn from(col: Color) -> Self {
        #[cfg(feature = "32-bit-color")]
        {
            let col: RgbaColor<f32> = col.into();
            col.into()
        }
        #[cfg(not(feature = "32-bit-color"))]
        {
            Self { red: col.red, green: col.green, blue: col.blue, alpha: col.alpha }
        }
    }
}

impl Color {
    /// Construct a color from an integer encoded as `0xAARRGGBB`
    pub const fn from_argb_encoded(encoded: u32) -> Color {
        Self::from_argb_u8(
            (encoded >> 24) as u8,
            (encoded >> 16) as u8,
            (encoded >> 8) as u8,
            encoded as u8,
        )
    }

    /// Returns `(alpha, red, green, blue)` encoded as u32
    pub fn as_argb_encoded(&self) -> u32 {
        let col: RgbaColor<u8> = (*self).into();
        ((col.red as u32) << 16)
            | ((col.green as u32) << 8)
            | (col.blue as u32)
            | ((col.alpha as u32) << 24)
    }

    /// Construct a color from the alpha, red, green and blue color channel parameters.
    pub const fn from_argb_u8(alpha: u8, red: u8, green: u8, blue: u8) -> Self {
        #[cfg(feature = "32-bit-color")]
        {
            Self {
                red: unquantize(red),
                green: unquantize(green),
                blue: unquantize(blue),
                alpha: unquantize(alpha),
            }
        }
        #[cfg(not(feature = "32-bit-color"))]
        {
            Self { red, green, blue, alpha }
        }
    }

    /// Construct a color from the red, green and blue color channel parameters. The alpha
    /// channel will have the value 255.
    pub const fn from_rgb_u8(red: u8, green: u8, blue: u8) -> Self {
        Self::from_argb_u8(255, red, green, blue)
    }

    /// Construct a color from the alpha, red, green and blue color channel parameters.
    pub fn from_argb_f32(alpha: f32, red: f32, green: f32, blue: f32) -> Self {
        RgbaColor { alpha, red, green, blue }.into()
    }

    /// Construct a color from the red, green and blue color channel parameters. The alpha
    /// channel will have the value 255.
    pub fn from_rgb_f32(red: f32, green: f32, blue: f32) -> Self {
        Self::from_argb_f32(1.0, red, green, blue)
    }

    /// Converts this color to an RgbaColor struct for easy destructuring.
    pub fn to_argb_u8(&self) -> RgbaColor<u8> {
        RgbaColor::from(*self)
    }

    /// Converts this color to an RgbaColor struct for easy destructuring.
    pub fn to_argb_f32(&self) -> RgbaColor<f32> {
        RgbaColor::from(*self)
    }

    /// Converts this color to the HSV color space.
    pub fn to_hsva(&self) -> HsvaColor {
        let rgba: RgbaColor<f32> = (*self).into();
        rgba.into()
    }

    /// Construct a color from the hue, saturation, and value HSV color space parameters.
    ///
    /// Hue is between 0 and 360, the others parameters between 0 and 1.
    pub fn from_hsva(hue: f32, saturation: f32, value: f32, alpha: f32) -> Self {
        let hsva = HsvaColor { hue, saturation, value, alpha };
        <RgbaColor<f32>>::from(hsva).into()
    }

    /// The color's four animation channels, `(alpha, lightness, a, b)` in Oklab —
    /// the same decomposition androidx.compose.animation.core animates `Color` on.
    pub(crate) fn oklab_channels(&self) -> [f32; 4] {
        let oklab = OklabColor::from(RgbaColor::<f32>::from(*self));
        [oklab.alpha, oklab.l, oklab.a, oklab.b]
    }

    /// Rebuild a color from `(alpha, lightness, a, b)` Oklab channels — the inverse
    /// of [`Self::oklab_channels`]. Channels are clamped exactly like
    /// androidx.compose.animation's `Color.VectorConverter` on the way back:
    /// alpha to `0..=1`, lightness to `0..=1`, `a`/`b` to `-0.5..=0.5`, then the
    /// result to sRGB gamut.
    pub(crate) fn from_oklab_channels(channels: &[f32]) -> Self {
        RgbaColor::from(OklabColor {
            alpha: channels[0].clamp(0.0, 1.0),
            l: channels[1].clamp(0.0, 1.0),
            a: channels[2].clamp(-0.5, 0.5),
            b: channels[3].clamp(-0.5, 0.5),
        })
        .into()
    }

    /// Converts this color to the Oklch color space.
    ///
    /// Oklch is a perceptually uniform color space with:
    /// - Lightness (L): 0 to 1
    /// - Chroma (C): typically 0 to ~0.4
    /// - Hue (h): 0 to 360 degrees
    pub fn to_oklch(&self) -> OklchColor {
        let rgba: RgbaColor<f32> = (*self).into();
        rgba.into()
    }

    /// Construct a color from the Oklch color space parameters.
    ///
    /// - `lightness`: 0 to 1 (black to white)
    /// - `chroma`: typically 0 to ~0.4 (grayscale to vivid)
    /// - `hue`: 0 to 360 degrees
    /// - `alpha`: 0 to 1
    pub fn from_oklch(lightness: f32, chroma: f32, hue: f32, alpha: f32) -> Self {
        let oklch = OklchColor { lightness, chroma, hue, alpha };
        <RgbaColor<f32>>::from(oklch).into()
    }

    /// Returns the red channel of the color as u8 in the range 0..255.
    #[inline(always)]
    pub fn red(self) -> u8 {
        RgbaColor::<u8>::from(self).red
    }

    /// Returns the green channel of the color as u8 in the range 0..255.
    #[inline(always)]
    pub fn green(self) -> u8 {
        RgbaColor::<u8>::from(self).green
    }

    /// Returns the blue channel of the color as u8 in the range 0..255.
    #[inline(always)]
    pub fn blue(self) -> u8 {
        RgbaColor::<u8>::from(self).blue
    }

    /// Returns the alpha channel of the color as u8 in the range 0..255.
    #[inline(always)]
    pub fn alpha(self) -> u8 {
        RgbaColor::<u8>::from(self).alpha
    }

    /// Returns a new version of this color that has the brightness increased
    /// by the specified factor. This is done by converting the color to the HSV
    /// color space and multiplying the brightness (value) with (1 + factor).
    /// The result is converted back to RGB and the alpha channel is unchanged.
    /// So for example `brighter(0.2)` will increase the brightness by 20%, and
    /// calling `brighter(-0.5)` will return a color that's 50% darker.
    #[must_use]
    pub fn brighter(&self, factor: f32) -> Self {
        let rgba: RgbaColor<f32> = (*self).into();
        let mut hsva: HsvaColor = rgba.into();
        hsva.value *= 1. + factor;
        let rgba: RgbaColor<f32> = hsva.into();
        rgba.into()
    }

    /// Returns a new version of this color that has the brightness decreased
    /// by the specified factor. This is done by converting the color to the HSV
    /// color space and dividing the brightness (value) by (1 + factor). The
    /// result is converted back to RGB and the alpha channel is unchanged.
    /// So for example `darker(0.3)` will decrease the brightness by 30%.
    #[must_use]
    pub fn darker(&self, factor: f32) -> Self {
        let rgba: RgbaColor<f32> = (*self).into();
        let mut hsva: HsvaColor = rgba.into();
        hsva.value /= 1. + factor;
        let rgba: RgbaColor<f32> = hsva.into();
        rgba.into()
    }

    /// Returns a new version of this color with the opacity decreased by `factor`.
    ///
    /// The transparency is obtained by multiplying the alpha channel by `(1 - factor)`.
    ///
    /// # Examples
    /// Decreasing the opacity of a red color by half:
    /// ```
    /// # use i_slint_core::graphics::Color;
    /// let red = Color::from_argb_f32(1.0, 1.0, 0.0, 0.0);
    /// assert_eq!(red.transparentize(0.5), Color::from_argb_f32(0.5, 1.0, 0.0, 0.0));
    /// ```
    ///
    /// Decreasing the opacity of a blue color by 20%:
    /// ```
    /// # use i_slint_core::graphics::Color;
    /// let blue = Color::from_argb_f32(1.0, 0.0, 0.0, 1.0);
    /// assert_eq!(blue.transparentize(0.2), Color::from_argb_f32(0.8, 0.0, 0.0, 1.0));
    /// ```
    ///
    /// Negative values increase the opacity
    ///
    /// ```
    /// # use i_slint_core::graphics::Color;
    /// let blue = Color::from_argb_f32(0.5, 0.0, 0.0, 1.0);
    /// assert_eq!(blue.transparentize(-0.1), Color::from_argb_f32(0.55, 0.0, 0.0, 1.0));
    /// ```
    #[must_use]
    pub fn transparentize(&self, factor: f32) -> Self {
        let mut col: RgbaColor<f32> = (*self).into();
        col.alpha = (col.alpha * (1.0 - factor)).clamp(0.0, 1.0);
        col.into()
    }

    /// Returns a new color that is a mix of this color and `other`. The specified factor is
    /// clamped to be between `0.0` and `1.0` and then applied to this color, while `1.0 - factor`
    /// is applied to `other`.
    ///
    /// # Examples
    /// Mix red with black half-and-half:
    /// ```
    /// # use i_slint_core::graphics::Color;
    /// let red = Color::from_rgb_f32(1.0, 0.0, 0.0);
    /// let black = Color::from_rgb_f32(0.0, 0.0, 0.0);
    /// assert_eq!(red.mix(&black, 0.5), Color::from_rgb_f32(0.5, 0.0, 0.0));
    /// ```
    ///
    /// Mix Purple with OrangeRed,  with `75%` purple and `25%` orange red ratio:
    /// ```
    /// # use i_slint_core::graphics::{Color, RgbaColor};
    /// let purple = Color::from_rgb_u8(128, 0, 128);
    /// let orange_red = Color::from_rgb_u8(255, 69, 0);
    /// assert_eq!(purple.mix(&orange_red, 0.75), Color::from_rgb_f32(0.6264706, 0.06764706, 0.37647063));
    /// ```
    #[must_use]
    pub fn mix(&self, other: &Self, factor: f32) -> Self {
        // * NOTE: The opacity (`alpha` as a "percentage") of each color involved
        // *       must be taken into account when mixing them. Because of this,
        // *       we cannot just interpolate between them.
        // * NOTE: Considering the spec (textual):
        // *       <https://github.com/sass/sass/blob/47d30713765b975c86fa32ec359ed16e83ad1ecc/spec/built-in-modules/color.md#mix>

        fn lerp(v1: f32, v2: f32, f: f32) -> f32 {
            (v1 * f + v2 * (1.0 - f)).clamp(0.0, 1.0)
        }

        let original_factor = factor.clamp(0.0, 1.0);

        let col = RgbaColor::<f32>::from(*self);
        let other = RgbaColor::<f32>::from(*other);

        let normal_weight = 2.0 * original_factor - 1.0;
        let alpha_distance = col.alpha - other.alpha;
        let weight_by_distance = normal_weight * alpha_distance;

        // As to not divide by 0.0
        let combined_weight = if weight_by_distance == -1.0 {
            normal_weight
        } else {
            (normal_weight + alpha_distance) / (1.0 + weight_by_distance)
        };

        let channels_factor = (combined_weight + 1.0) / 2.0;

        let red = lerp(col.red, other.red, channels_factor);
        let green = lerp(col.green, other.green, channels_factor);
        let blue = lerp(col.blue, other.blue, channels_factor);

        let alpha = lerp(col.alpha, other.alpha, original_factor);

        RgbaColor { red, green, blue, alpha }.into()
    }

    /// Returns a new version of this color with the opacity set to `alpha`.
    #[must_use]
    pub fn with_alpha(&self, alpha: f32) -> Self {
        let mut rgba: RgbaColor<f32> = (*self).into();
        rgba.alpha = alpha.clamp(0.0, 1.0);
        rgba.into()
    }
}

impl InterpolatedPropertyValue for Color {
    /// Interpolates in Oklab space (`alpha`, `l`, `a`, `b`) — the perceptually uniform
    /// space androidx.compose.animation.core uses for color animations.
    fn interpolate(&self, target_value: &Self, t: f32) -> Self {
        let from = OklabColor::from(RgbaColor::<f32>::from(*self));
        let to = OklabColor::from(RgbaColor::<f32>::from(*target_value));
        // `Color.VectorConverter`'s `convertFromVector` clamps the interpolated
        // channels (`fastCoerceIn`) — needed for overshooting easings.
        Self::from_oklab_channels(&[
            from.alpha + t * (to.alpha - from.alpha),
            from.l + t * (to.l - from.l),
            from.a + t * (to.a - from.a),
            from.b + t * (to.b - from.b),
        ])
    }

    /// Color channels are `(alpha, l, a, b)` in Oklab — the same four channels
    /// androidx.compose.animation.core animates `Color` on.
    fn channel_count(&self, _target_value: &Self) -> usize {
        4
    }

    fn write_channels(&self, _target_value: &Self, out: &mut [f32]) {
        let oklab = OklabColor::from(RgbaColor::<f32>::from(*self));
        out[..4].copy_from_slice(&[oklab.alpha, oklab.l, oklab.a, oklab.b]);
    }

    fn from_channels(&self, _target_value: &Self, channels: &[f32]) -> Self {
        Self::from_oklab_channels(channels)
    }
}

impl core::fmt::Display for Color {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "argb({}, {}, {}, {})", self.alpha(), self.red(), self.green(), self.blue())
    }
}

/// HsvaColor stores the hue, saturation, value and alpha components of a color
/// in the HSV color space as `f32 ` fields.
/// This is merely a helper struct for use with [`Color`].
#[derive(Copy, Clone, PartialOrd, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct HsvaColor {
    /// The hue component in degrees between 0 and 360.
    pub hue: f32,
    /// The saturation component, between 0 and 1.
    pub saturation: f32,
    /// The value component, between 0 and 1.
    pub value: f32,
    /// The alpha component, between 0 and 1.
    pub alpha: f32,
}

impl PartialEq for HsvaColor {
    fn eq(&self, other: &Self) -> bool {
        (self.hue - other.hue).abs() < 0.00001
            && (self.saturation - other.saturation).abs() < 0.00001
            && (self.value - other.value).abs() < 0.00001
            && (self.alpha - other.alpha).abs() < 0.00001
    }
}

impl From<RgbaColor<f32>> for HsvaColor {
    fn from(col: RgbaColor<f32>) -> Self {
        // RGB to HSL conversion from https://en.wikipedia.org/wiki/HSL_and_HSV#Color_conversion_formulae

        let red = col.red;
        let green = col.green;
        let blue = col.blue;

        let min = red.min(green).min(blue);
        let max = red.max(green).max(blue);
        let chroma = max - min;

        #[allow(clippy::float_cmp)] // `max` is either `red`, `green` or `blue`
        let hue = num_traits::Euclid::rem_euclid(
            &(60.
                * if chroma == 0.0 {
                    0.0
                } else if max == red {
                    ((green - blue) / chroma) % 6.0
                } else if max == green {
                    2. + (blue - red) / chroma
                } else {
                    4. + (red - green) / chroma
                }),
            &360.0,
        );
        let saturation = if max == 0. { 0. } else { chroma / max };

        Self { hue, saturation, value: max, alpha: col.alpha }
    }
}

impl From<HsvaColor> for RgbaColor<f32> {
    fn from(col: HsvaColor) -> Self {
        // RGB to HSL conversion from https://en.wikipedia.org/wiki/HSL_and_HSV#Color_conversion_formulae

        let chroma = col.saturation * col.value;

        let hue = num_traits::Euclid::rem_euclid(&col.hue, &360.0);

        let x = chroma * (1. - ((hue / 60.) % 2. - 1.).abs());

        let (red, green, blue) = match (hue / 60.0) as usize {
            0 => (chroma, x, 0.),
            1 => (x, chroma, 0.),
            2 => (0., chroma, x),
            3 => (0., x, chroma),
            4 => (x, 0., chroma),
            5 => (chroma, 0., x),
            _ => (0., 0., 0.),
        };

        let m = col.value - chroma;

        Self { red: red + m, green: green + m, blue: blue + m, alpha: col.alpha }
    }
}

impl From<HsvaColor> for Color {
    fn from(value: HsvaColor) -> Self {
        RgbaColor::from(value).into()
    }
}

impl From<Color> for HsvaColor {
    fn from(value: Color) -> Self {
        value.to_hsva()
    }
}

/// OklchColor stores the lightness, chroma, hue and alpha components of a color
/// in the Oklch color space as `f32` fields.
/// Oklch is a perceptually uniform color space, useful for color manipulation.
/// This is merely a helper struct for use with [`Color`].
///
/// Reference: <https://bottosson.github.io/posts/oklab/>
#[derive(Copy, Clone, PartialOrd, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct OklchColor {
    /// The lightness component, between 0 (black) and 1 (white).
    pub lightness: f32,
    /// The chroma component (color intensity), typically between 0 and about 0.4.
    pub chroma: f32,
    /// The hue component in degrees between 0 and 360.
    pub hue: f32,
    /// The alpha component, between 0 and 1.
    pub alpha: f32,
}

impl PartialEq for OklchColor {
    fn eq(&self, other: &Self) -> bool {
        (self.lightness - other.lightness).abs() < 0.00001
            && (self.chroma - other.chroma).abs() < 0.00001
            && (self.hue - other.hue).abs() < 0.00001
            && (self.alpha - other.alpha).abs() < 0.00001
    }
}

/// Helper struct for Oklab color space (intermediate representation).
#[derive(Copy, Clone, Debug)]
struct OklabColor {
    l: f32,
    a: f32,
    b: f32,
    alpha: f32,
}

impl From<OklchColor> for OklabColor {
    fn from(oklch: OklchColor) -> Self {
        let hue_rad = oklch.hue * core::f32::consts::PI / 180.0;
        Self {
            l: oklch.lightness,
            a: oklch.chroma * hue_rad.cos(),
            b: oklch.chroma * hue_rad.sin(),
            alpha: oklch.alpha,
        }
    }
}

impl From<OklabColor> for OklchColor {
    fn from(oklab: OklabColor) -> Self {
        let chroma = (oklab.a * oklab.a + oklab.b * oklab.b).sqrt();
        let hue = if chroma < 0.00001 {
            0.0
        } else {
            let hue_rad = oklab.b.atan2(oklab.a);
            num_traits::Euclid::rem_euclid(&(hue_rad * 180.0 / core::f32::consts::PI), &360.0)
        };
        Self { lightness: oklab.l, chroma, hue, alpha: oklab.alpha }
    }
}

/// androidx.compose.ui.graphics colorspace math, ported at the androidx pin
/// (commit 23327507f7fc7d5b19d65fec4b090f60c970079b). Compose animates `Color`
/// in Oklab via `Color.convert(ColorSpaces.Oklab)`: sRGB is first adapted to
/// the D50 profile connection space (`Rgb.adapt`, Bradford), then converted by
/// `Oklab.fromXyz`, whose `M1` already carries the D50→D65 adaptation. All
/// matrices are 3x3 column-major and every multiply happens in the same order
/// — and the same f32 width — as `mul3x3Float3`/`mul3x3`/`inverse3x3` in
/// `ColorSpace.kt`.
mod compose_oklab {
    const fn mul3x3f3(m: &[f32; 9], v: [f32; 3]) -> [f32; 3] {
        [
            m[0] * v[0] + m[3] * v[1] + m[6] * v[2],
            m[1] * v[0] + m[4] * v[1] + m[7] * v[2],
            m[2] * v[0] + m[5] * v[1] + m[8] * v[2],
        ]
    }

    const fn mul3x3(lhs: &[f32; 9], rhs: &[f32; 9]) -> [f32; 9] {
        [
            lhs[0] * rhs[0] + lhs[3] * rhs[1] + lhs[6] * rhs[2],
            lhs[1] * rhs[0] + lhs[4] * rhs[1] + lhs[7] * rhs[2],
            lhs[2] * rhs[0] + lhs[5] * rhs[1] + lhs[8] * rhs[2],
            lhs[0] * rhs[3] + lhs[3] * rhs[4] + lhs[6] * rhs[5],
            lhs[1] * rhs[3] + lhs[4] * rhs[4] + lhs[7] * rhs[5],
            lhs[2] * rhs[3] + lhs[5] * rhs[4] + lhs[8] * rhs[5],
            lhs[0] * rhs[6] + lhs[3] * rhs[7] + lhs[6] * rhs[8],
            lhs[1] * rhs[6] + lhs[4] * rhs[7] + lhs[7] * rhs[8],
            lhs[2] * rhs[6] + lhs[5] * rhs[7] + lhs[8] * rhs[8],
        ]
    }

    const fn inverse3x3(m: &[f32; 9]) -> [f32; 9] {
        let (a, b, c, d, e, f, g, h, i) = (m[0], m[3], m[6], m[1], m[4], m[7], m[2], m[5], m[8]);
        let xa = e * i - f * h;
        let xb = f * g - d * i;
        let xc = d * h - e * g;
        let det = a * xa + b * xb + c * xc;
        [
            xa / det,
            xb / det,
            xc / det,
            (c * h - b * i) / det,
            (a * i - c * g) / det,
            (b * g - a * h) / det,
            (b * f - c * e) / det,
            (c * d - a * f) / det,
            (a * e - b * d) / det,
        ]
    }

    const fn mul3x3_diag(lhs: [f32; 3], rhs: &[f32; 9]) -> [f32; 9] {
        [
            lhs[0] * rhs[0],
            lhs[1] * rhs[1],
            lhs[2] * rhs[2],
            lhs[0] * rhs[3],
            lhs[1] * rhs[4],
            lhs[2] * rhs[5],
            lhs[0] * rhs[6],
            lhs[1] * rhs[7],
            lhs[2] * rhs[8],
        ]
    }

    /// `ColorSpace.kt`'s `chromaticAdaptation(Adaptation.Bradford, src, dst)`.
    const fn chromatic_adaptation(src_xyz: [f32; 3], dst_xyz: [f32; 3]) -> [f32; 9] {
        const BRADFORD: [f32; 9] = [
            0.8951, -0.7502, 0.0389, //
            0.2664, 1.7135, -0.0685, //
            -0.1614, 0.0367, 1.0296,
        ];
        let src_lms = mul3x3f3(&BRADFORD, src_xyz);
        let dst_lms = mul3x3f3(&BRADFORD, dst_xyz);
        let lms = [dst_lms[0] / src_lms[0], dst_lms[1] / src_lms[1], dst_lms[2] / src_lms[2]];
        mul3x3(&inverse3x3(&BRADFORD), &mul3x3_diag(lms, &BRADFORD))
    }

    /// `WhitePoint.toXyz()`: xyY (Y = 1) to XYZ.
    const fn white_point_to_xyz(x: f32, y: f32) -> [f32; 3] {
        [x / y, 1.0, (1.0 - x - y) / y]
    }

    /// `Illuminant.D50` and `Illuminant.D65` as XYZ (from xy, like `toXyz()`).
    const D50_XYZ: [f32; 3] = white_point_to_xyz(0.34567, 0.35850);
    const D65_XYZ: [f32; 3] = white_point_to_xyz(0.31271, 0.32902);

    /// `Rgb.kt`'s `computeXYZMatrix` for the sRGB primaries in `ColorSpaces.kt`
    /// (`SrgbPrimaries`) under `Illuminant.D65`.
    const fn srgb_to_xyz_d65() -> [f32; 9] {
        const PRIMARIES: [f32; 6] = [0.640, 0.330, 0.300, 0.600, 0.150, 0.060];
        let rx = PRIMARIES[0];
        let ry = PRIMARIES[1];
        let gx = PRIMARIES[2];
        let gy = PRIMARIES[3];
        let bx = PRIMARIES[4];
        let by = PRIMARIES[5];
        let wx = 0.31271f32;
        let wy = 0.32902f32;

        let one_rx_ry = (1.0 - rx) / ry;
        let one_gx_gy = (1.0 - gx) / gy;
        let one_bx_by = (1.0 - bx) / by;
        let one_wx_wy = (1.0 - wx) / wy;

        let rx_ry = rx / ry;
        let gx_gy = gx / gy;
        let bx_by = bx / by;
        let wx_wy = wx / wy;

        let by_numerator =
            (one_wx_wy - one_rx_ry) * (gx_gy - rx_ry) - (wx_wy - rx_ry) * (one_gx_gy - one_rx_ry);
        let by_denominator =
            (one_bx_by - one_rx_ry) * (gx_gy - rx_ry) - (bx_by - rx_ry) * (one_gx_gy - one_rx_ry);
        let by_lum = by_numerator / by_denominator;
        let gy_lum = (wx_wy - rx_ry - by_lum * (bx_by - rx_ry)) / (gx_gy - rx_ry);
        let ry_lum = 1.0 - gy_lum - by_lum;

        let r_ry = ry_lum / ry;
        let g_gy = gy_lum / gy;
        let b_by = by_lum / by;

        [
            r_ry * rx,
            ry_lum,
            r_ry * (1.0 - rx - ry),
            g_gy * gx,
            gy_lum,
            g_gy * (1.0 - gx - gy),
            b_by * bx,
            by_lum,
            b_by * (1.0 - bx - by),
        ]
    }

    /// `Srgb.adapt(Illuminant.D50).transform`: linear sRGB to XYZ D50.
    const SRGB_TO_XYZ_D50: [f32; 9] =
        mul3x3(&chromatic_adaptation(D65_XYZ, D50_XYZ), &srgb_to_xyz_d65());
    /// `Srgb.adapt(Illuminant.D50).inverseTransform`.
    const SRGB_FROM_XYZ_D50: [f32; 9] = inverse3x3(&SRGB_TO_XYZ_D50);

    /// `Oklab.kt`'s `M1`: raw Oklab M1 times Bradford D50→D65.
    const OKLAB_M1: [f32; 9] = mul3x3(
        &[
            0.8189330101,
            0.0329845436,
            0.0482003018, //
            0.3618667424,
            0.9293118715,
            0.2643662691, //
            -0.1288597137,
            0.0361456387,
            0.6338517070,
        ],
        &chromatic_adaptation(D50_XYZ, D65_XYZ),
    );
    const OKLAB_INVERSE_M1: [f32; 9] = inverse3x3(&OKLAB_M1);
    /// `Oklab.kt`'s `M2` and `InverseM2`.
    const OKLAB_M2: [f32; 9] = [
        0.2104542553,
        1.9779984951,
        0.0259040371, //
        0.7936177850,
        -2.4285922050,
        0.7827717662, //
        -0.0040720468,
        0.4505937099,
        -0.8086757660,
    ];
    const OKLAB_INVERSE_M2: [f32; 9] = inverse3x3(&OKLAB_M2);

    /// Compose's eotf/oetf functions run on `Double` (`Rgb.eotfFunc`).
    fn srgb_eotf(c: f32) -> f32 {
        let c = c as f64;
        (if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }) as f32
    }

    fn srgb_oetf(c: f32) -> f32 {
        let c = c as f64;
        (if c <= 0.0031308 { c * 12.92 } else { 1.055 * c.powf(1.0 / 2.4) - 0.055 }) as f32
    }

    /// `Connector.transform` sRGB→Oklab, returning `(l, a, b)`.
    pub(super) fn srgb_to_oklab(r: f32, g: f32, b: f32) -> [f32; 3] {
        let v = mul3x3f3(&SRGB_TO_XYZ_D50, [srgb_eotf(r), srgb_eotf(g), srgb_eotf(b)]);
        let v = mul3x3f3(&OKLAB_M1, v);
        let v = [v[0].cbrt(), v[1].cbrt(), v[2].cbrt()];
        mul3x3f3(&OKLAB_M2, v)
    }

    /// `Connector.transform` Oklab→sRGB for `(l, a, b)`. The channel clamps
    /// live inside `Oklab.toXyz`/`toXy` in Compose.
    pub(super) fn oklab_to_srgb(l: f32, a: f32, b: f32) -> [f32; 3] {
        let l = l.clamp(0.0, 1.0);
        let a = a.clamp(-0.5, 0.5);
        let b = b.clamp(-0.5, 0.5);
        let v = mul3x3f3(&OKLAB_INVERSE_M2, [l, a, b]);
        let v = [v[0] * v[0] * v[0], v[1] * v[1] * v[1], v[2] * v[2] * v[2]];
        let v = mul3x3f3(&OKLAB_INVERSE_M1, v);
        let v = mul3x3f3(&SRGB_FROM_XYZ_D50, v);
        [srgb_oetf(v[0]), srgb_oetf(v[1]), srgb_oetf(v[2])]
    }
}

impl From<RgbaColor<f32>> for OklabColor {
    fn from(col: RgbaColor<f32>) -> Self {
        // Compose's `Color.convert(ColorSpaces.Oklab)`: sRGB adapted to D50 XYZ
        // (Bradford), then `Oklab.fromXyz`.
        let [l, a, b] = compose_oklab::srgb_to_oklab(col.red, col.green, col.blue);
        Self { l, a, b, alpha: col.alpha }
    }
}

impl From<OklabColor> for RgbaColor<f32> {
    fn from(oklab: OklabColor) -> Self {
        // `Oklab.toXyz` (clamping `l`/`a`/`b` internally) then XYZ D50 back into
        // adapted sRGB; the result is clamped to the sRGB gamut.
        let [r, g, b] = compose_oklab::oklab_to_srgb(oklab.l, oklab.a, oklab.b);
        Self {
            red: r.clamp(0.0, 1.0),
            green: g.clamp(0.0, 1.0),
            blue: b.clamp(0.0, 1.0),
            alpha: oklab.alpha,
        }
    }
}

impl From<OklchColor> for RgbaColor<f32> {
    fn from(oklch: OklchColor) -> Self {
        let oklab = OklabColor::from(oklch);
        RgbaColor::from(oklab)
    }
}

impl From<RgbaColor<f32>> for OklchColor {
    fn from(col: RgbaColor<f32>) -> Self {
        let oklab = OklabColor::from(col);
        OklchColor::from(oklab)
    }
}

impl From<OklchColor> for Color {
    fn from(value: OklchColor) -> Self {
        RgbaColor::from(value).into()
    }
}

impl From<Color> for OklchColor {
    fn from(value: Color) -> Self {
        value.to_oklch()
    }
}

#[test]
fn test_rgb_to_hsv() {
    // White
    assert_eq!(
        HsvaColor::from(RgbaColor::<f32> { red: 1., green: 1., blue: 1., alpha: 0.5 }),
        HsvaColor { hue: 0., saturation: 0., value: 1., alpha: 0.5 }
    );
    assert_eq!(
        RgbaColor::<f32>::from(HsvaColor { hue: 0., saturation: 0., value: 1., alpha: 0.3 }),
        RgbaColor::<f32> { red: 1., green: 1., blue: 1., alpha: 0.3 }
    );

    // #8a0c77ff ensure the hue ends up positive
    assert_eq!(
        HsvaColor::from(Color::from_argb_u8(0xff, 0x8a, 0xc, 0x77,).to_argb_f32()),
        HsvaColor { hue: 309.0476, saturation: 0.9130435, value: 0.5411765, alpha: 1.0 }
    );

    let received = RgbaColor::<f32>::from(HsvaColor {
        hue: 309.0476,
        saturation: 0.9130435,
        value: 0.5411765,
        alpha: 1.0,
    });
    let expected = Color::from_argb_u8(0xff, 0x8a, 0xc, 0x77).to_argb_f32();

    assert!(
        (received.alpha - expected.alpha).abs() < 0.00001
            && (received.red - expected.red).abs() < 0.00001
            && (received.green - expected.green).abs() < 0.00001
            && (received.blue - expected.blue).abs() < 0.00001
    );

    // Bright greenish, verified via colorizer.org
    assert_eq!(
        HsvaColor::from(RgbaColor::<f32> { red: 0., green: 0.9, blue: 0., alpha: 1.0 }),
        HsvaColor { hue: 120., saturation: 1., value: 0.9, alpha: 1.0 }
    );
    assert_eq!(
        RgbaColor::<f32>::from(HsvaColor { hue: 120., saturation: 1., value: 0.9, alpha: 1.0 }),
        RgbaColor::<f32> { red: 0., green: 0.9, blue: 0., alpha: 1.0 }
    );

    // Hue should wrap around 360deg i.e. 480 == 120 && -240 == 240
    assert_eq!(
        RgbaColor::<f32> { red: 0., green: 0.9, blue: 0., alpha: 1.0 },
        RgbaColor::<f32>::from(HsvaColor { hue: 480., saturation: 1., value: 0.9, alpha: 1.0 }),
    );
    assert_eq!(
        RgbaColor::<f32> { red: 0., green: 0.9, blue: 0., alpha: 1.0 },
        RgbaColor::<f32>::from(HsvaColor { hue: -240., saturation: 1., value: 0.9, alpha: 1.0 }),
    );
}

#[test]
fn test_brighter_darker() {
    let blue = Color::from_rgb_u8(0, 0, 128);
    assert_eq!(blue.brighter(0.5), Color::from_rgb_f32(0.0, 0.0, 0.75294125));
    assert_eq!(blue.darker(0.5), Color::from_rgb_f32(0.0, 0.0, 0.33464053));
}

#[test]
fn test_transparent_transition() {
    // Interpolation happens in Oklab: fading from transparent black to a gray
    // crosses darker grays, like `lerp`ing Compose's `Color` channels does —
    // only the alpha tracks `t` 1:1.
    let color = Color::from_argb_f32(0.0, 0.0, 0.0, 0.0);
    let target = Color::from_rgb_f32(0.8, 0.8, 0.8);
    for (t, expected) in [(0.25, 0.0961), (0.5, 0.3045), (0.75, 0.5418)] {
        let interpolated = RgbaColor::<f32>::from(color.interpolate(&target, t));
        assert!(
            (interpolated.alpha - t as f32).abs() < 0.01
                && (interpolated.red - expected).abs() < 0.01
                && (interpolated.green - expected).abs() < 0.01
                && (interpolated.blue - expected).abs() < 0.01,
            "t={t}: {interpolated:?}"
        );
    }
}

#[test]
fn test_oklch_roundtrip() {
    // Test that Oklch round trips correctly through RGB
    // Use colors with low chroma that are definitely within sRGB gamut
    let test_colors = [
        OklchColor { lightness: 0.5, chroma: 0.08, hue: 30.0, alpha: 1.0 },
        OklchColor { lightness: 0.6, chroma: 0.1, hue: 120.0, alpha: 0.8 },
        OklchColor { lightness: 0.4, chroma: 0.08, hue: 240.0, alpha: 1.0 },
        OklchColor { lightness: 0.8, chroma: 0.05, hue: 0.0, alpha: 1.0 },
        // Grayscale (chroma = 0)
        OklchColor { lightness: 0.5, chroma: 0.0, hue: 0.0, alpha: 1.0 },
    ];

    for oklch in test_colors {
        let rgba = RgbaColor::<f32>::from(oklch);
        let roundtrip = OklchColor::from(rgba);
        // Allow some tolerance due to floating point operations and gamut mapping
        assert!(
            (oklch.lightness - roundtrip.lightness).abs() < 0.01,
            "Lightness mismatch: {:?} vs {:?}",
            oklch,
            roundtrip
        );
        // Skip chroma/hue comparison for grayscale since hue is undefined
        if oklch.chroma > 0.001 {
            assert!(
                (oklch.chroma - roundtrip.chroma).abs() < 0.02,
                "Chroma mismatch: {:?} vs {:?}",
                oklch,
                roundtrip
            );
            // Hue can wrap around, so we need to handle that
            let hue_diff = (oklch.hue - roundtrip.hue).abs();
            let hue_diff = hue_diff.min(360.0 - hue_diff);
            assert!(hue_diff < 2.0, "Hue mismatch: {:?} vs {:?}", oklch, roundtrip);
        }
    }
}

#[test]
fn test_oklch_known_values() {
    // Test conversion of a known Oklch value to RGB
    // These values are approximate and verified against online converters
    let red_oklch = OklchColor { lightness: 0.63, chroma: 0.26, hue: 29.0, alpha: 1.0 };
    let red_rgba = RgbaColor::<f32>::from(red_oklch);
    // Red should have high red component and low green/blue
    assert!(red_rgba.red > 0.8, "Red component should be high: {}", red_rgba.red);
    assert!(red_rgba.green < 0.3, "Green component should be low: {}", red_rgba.green);
    assert!(red_rgba.blue < 0.3, "Blue component should be low: {}", red_rgba.blue);
}

#[test]
fn test_rgb_to_oklch() {
    // White: should have high lightness, zero chroma
    let white = OklchColor::from(RgbaColor::<f32> { red: 1., green: 1., blue: 1., alpha: 0.5 });
    assert!((white.lightness - 1.0).abs() < 0.01, "White lightness should be ~1.0");
    assert!(white.chroma < 0.001, "White chroma should be ~0");
    assert!((white.alpha - 0.5).abs() < 0.001, "Alpha should be preserved");

    // Black: should have zero lightness, zero chroma
    let black = OklchColor::from(RgbaColor::<f32> { red: 0., green: 0., blue: 0., alpha: 1.0 });
    assert!(black.lightness < 0.01, "Black lightness should be ~0");
    assert!(black.chroma < 0.001, "Black chroma should be ~0");

    // Pure red: should have hue around 29 degrees (red in Oklch)
    let red = OklchColor::from(RgbaColor::<f32> { red: 1., green: 0., blue: 0., alpha: 1.0 });
    assert!(red.lightness > 0.5 && red.lightness < 0.7, "Red lightness should be ~0.63");
    assert!(red.chroma > 0.2, "Red should have significant chroma");
    assert!(red.hue > 20.0 && red.hue < 40.0, "Red hue should be around 29 degrees");

    // Pure blue: should have hue around 264 degrees
    let blue = OklchColor::from(RgbaColor::<f32> { red: 0., green: 0., blue: 1., alpha: 1.0 });
    assert!(blue.lightness > 0.4 && blue.lightness < 0.5, "Blue lightness should be ~0.45");
    assert!(blue.chroma > 0.2, "Blue should have significant chroma");
    assert!(blue.hue > 250.0 && blue.hue < 280.0, "Blue hue should be around 264 degrees");
}

#[test]
fn test_oklab_compose_reference() {
    // Reference `(l, a, b)` values computed with numpy in f32 by porting the
    // pinned androidx path verbatim: `Rgb.eotfFunc` → `Srgb.adapt(D50)` →
    // `Oklab.fromXyz` (commit 23327507f7fc7d5b19d65fec4b090f60c970079b). The
    // same constant matrices and operation order are used here, so the (l, a,
    // b) values are expected to agree to a few ulps; the sRGB round trip gets a
    // wider 1e-4 bound since `toXyz` clamps out-of-gamut excursions on the way.
    let cases = [
        ([1.0, 0.0, 0.0], [0.627951, 0.224828, 0.125792]),
        ([0.0, 1.0, 0.0], [0.866445, -0.233920, 0.179420]),
        ([0.0, 0.0, 1.0], [0.451988, -0.032430, -0.311619]),
        ([1.0, 1.0, 1.0], [0.999997, -0.000033, -0.000098]),
        ([0.4, 0.2, 0.9], [0.505848, 0.071432, -0.234832]),
        ([0.5, 0.5, 0.5], [0.598179, -0.000020, -0.000058]),
        ([0.0, 1.0, 1.0], [0.905398, -0.149464, -0.039490]),
        ([1.0, 1.0, 0.0], [0.967985, -0.071414, 0.198484]),
    ];
    for (rgb, lab) in cases {
        let oklab = OklabColor::from(RgbaColor::<f32> {
            red: rgb[0],
            green: rgb[1],
            blue: rgb[2],
            alpha: 1.0,
        });
        for (i, expected) in [lab[0], lab[1], lab[2]].iter().enumerate() {
            let actual = [oklab.l, oklab.a, oklab.b][i];
            assert!(
                (actual - expected).abs() < 1e-5,
                "{rgb:?} channel {i}: {actual} vs {expected}"
            );
        }
        // ...and back to sRGB within a ulp or two.
        let back = RgbaColor::<f32>::from(oklab);
        for (i, expected) in rgb.iter().enumerate() {
            let actual = [back.red, back.green, back.blue][i];
            assert!((actual - expected).abs() < 1e-4, "{rgb:?} sRGB {i}: {actual} vs {expected}");
        }
    }
}

#[test]
fn test_interpolate_clamps_overshoot() {
    // Overshooting easings (t > 1) must clamp the way `Color.VectorConverter`'s
    // `convertFromVector` does: alpha 0..=1, l 0..=1, a/b ±0.5, then the result
    // into the sRGB gamut.
    let black = Color::from_rgb_f32(0.0, 0.0, 0.0);
    let red = Color::from_rgb_f32(1.0, 0.0, 0.0);
    let over = RgbaColor::<f32>::from(black.interpolate(&red, 3.0));
    for c in [over.red, over.green, over.blue, over.alpha] {
        assert!((0.0..=1.0).contains(&c), "out of gamut: {over:?}");
    }
    assert_eq!(over.alpha, 1.0);
    // The result equals converting the clamped channels.
    let from = OklabColor::from(RgbaColor::<f32>::from(black));
    let to = OklabColor::from(RgbaColor::<f32>::from(red));
    let expected = Color::from_oklab_channels(&[
        from.alpha + 3.0 * (to.alpha - from.alpha),
        from.l + 3.0 * (to.l - from.l),
        from.a + 3.0 * (to.a - from.a),
        from.b + 3.0 * (to.b - from.b),
    ]);
    assert_eq!(over, RgbaColor::<f32>::from(expected));
}

#[cfg(feature = "ffi")]
pub(crate) mod ffi {
    #![allow(unsafe_code)]
    use super::*;

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_color_brighter(col: &Color, factor: f32, out: *mut Color) {
        unsafe { core::ptr::write(out, col.brighter(factor)) }
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_color_darker(col: &Color, factor: f32, out: *mut Color) {
        unsafe { core::ptr::write(out, col.darker(factor)) }
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_color_transparentize(col: &Color, factor: f32, out: *mut Color) {
        unsafe { core::ptr::write(out, col.transparentize(factor)) }
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_color_mix(
        col1: &Color,
        col2: &Color,
        factor: f32,
        out: *mut Color,
    ) {
        unsafe { core::ptr::write(out, col1.mix(col2, factor)) }
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_color_with_alpha(col: &Color, alpha: f32, out: *mut Color) {
        unsafe { core::ptr::write(out, col.with_alpha(alpha)) }
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn slint_color_to_hsva(
        col: &Color,
        h: &mut f32,
        s: &mut f32,
        v: &mut f32,
        a: &mut f32,
    ) {
        let hsv = col.to_hsva();
        *h = hsv.hue;
        *s = hsv.saturation;
        *v = hsv.value;
        *a = hsv.alpha;
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn slint_color_from_hsva(h: f32, s: f32, v: f32, a: f32) -> Color {
        Color::from_hsva(h, s, v, a)
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn slint_color_from_oklch(l: f32, c: f32, h: f32, a: f32) -> Color {
        Color::from_oklch(l, c, h, a)
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn slint_color_to_oklch(
        col: &Color,
        l: &mut f32,
        c: &mut f32,
        h: &mut f32,
        a: &mut f32,
    ) {
        let oklch = col.to_oklch();
        *l = oklch.lightness;
        *c = oklch.chroma;
        *h = oklch.hue;
        *a = oklch.alpha;
    }
}
