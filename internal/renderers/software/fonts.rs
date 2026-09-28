// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore pixelfont vectorfont
use alloc::rc::Rc;
use alloc::vec::Vec;
use core::cell::RefCell;

use super::{Fixed, PhysicalLength, PhysicalSize};
#[cfg(feature = "owned-pre-rendered-resources")]
use i_slint_core::graphics::OwnedStorage;
use i_slint_core::graphics::{BitmapFont, FontRequest, Storage};
#[cfg(any(feature = "systemfonts", feature = "embedded-vector-fonts"))]
use i_slint_core::graphics::{Rgba8Pixel, SharedPixelBuffer};
use i_slint_core::lengths::ScaleFactor;
use i_slint_core::model::Model;
use i_slint_core::textlayout::TextLayout;
use pixelfont::PixelFont;

i_slint_core::thread_local! {
    static BITMAP_FONTS: RefCell<Vec<RegisteredFont>> = RefCell::default()
}

enum RegisteredFont {
    Static(&'static BitmapFont),
    #[cfg(feature = "owned-pre-rendered-resources")]
    Shared(alloc::rc::Weak<BitmapFont<OwnedStorage>>),
}

impl PartialEq for RegisteredFont {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Static(a), Self::Static(b)) => core::ptr::eq(*a, *b),
            #[cfg(feature = "owned-pre-rendered-resources")]
            (Self::Shared(a), Self::Shared(b)) => a.ptr_eq(b),
            #[cfg(feature = "owned-pre-rendered-resources")]
            _ => false,
        }
    }
}

impl RegisteredFont {
    /// How far the weight of the font is from `weight`, when the font is still alive,
    /// of `family`, and `italic` (any, when `None`).
    fn weight_distance(
        &self,
        family: Option<&str>,
        italic: Option<bool>,
        weight: u16,
    ) -> Option<u16> {
        fn distance<S: Storage>(
            font: &BitmapFont<S>,
            family: Option<&str>,
            italic: Option<bool>,
            weight: u16,
        ) -> Option<u16> {
            let matches = italic.is_none_or(|italic| font.italic == italic)
                && family.is_none_or(|family| &*font.family_name == family.as_bytes());
            matches.then(|| font.weight.abs_diff(weight))
        }
        match self {
            Self::Static(font) => distance(font, family, italic, weight),
            #[cfg(feature = "owned-pre-rendered-resources")]
            Self::Shared(font) => distance(&*font.upgrade()?, family, italic, weight),
        }
    }

    /// [`axis_score`] of the font, when it's still alive, of `family`, and `italic`.
    fn axis_score(
        &self,
        family: &str,
        italic: bool,
        requested_weight: u16,
        requested_variations: &[(i_slint_core::SharedString, f32)],
        opsz_is_explicit: bool,
        scale_factor: ScaleFactor,
    ) -> Option<f32> {
        fn score<S: Storage>(
            font: &BitmapFont<S>,
            family: &str,
            italic: bool,
            (weight, variations, opsz_is_explicit, scale_factor): (
                u16,
                &[(i_slint_core::SharedString, f32)],
                bool,
                ScaleFactor,
            ),
        ) -> Option<f32> {
            (font.italic == italic && &*font.family_name == family.as_bytes()).then(|| {
                axis_score(font, weight, variations, opsz_is_explicit, scale_factor)
            })
        }
        let request = (requested_weight, requested_variations, opsz_is_explicit, scale_factor);
        match self {
            Self::Static(font) => score(font, family, italic, request),
            #[cfg(feature = "owned-pre-rendered-resources")]
            Self::Shared(font) => score(&*font.upgrade()?, family, italic, request),
        }
    }

    fn at_pixel_size(&self, pixel_size: PhysicalLength) -> Option<Font> {
        Some(match self {
            Self::Static(font) => Font::Pixel(PixelFont::new(*font, pixel_size)),
            #[cfg(feature = "owned-pre-rendered-resources")]
            Self::Shared(font) => Font::SharedPixel(PixelFont::new(font.upgrade()?, pixel_size)),
        })
    }
}

#[derive(derive_more::From, Clone)]
pub enum GlyphAlphaMap {
    /// A glyph of a font that generated code keeps in a static
    Static(&'static [u8]),
    /// A glyph of a font built at run time, which it keeps alive while the glyph is drawn
    #[cfg(feature = "owned-pre-rendered-resources")]
    #[from(skip)]
    Shared { font: Rc<BitmapFont<OwnedStorage>>, glyph_set: u16, glyph: u16 },
    /// A glyph rendered at run time, from a vector font
    Rendered(Rc<[u8]>),
}

impl GlyphAlphaMap {
    pub fn as_slice(&self) -> &[u8] {
        match self {
            Self::Static(data) => data,
            #[cfg(feature = "owned-pre-rendered-resources")]
            Self::Shared { font, glyph_set, glyph } => {
                &font.glyphs[*glyph_set as usize].glyph_data[*glyph as usize].data
            }
            Self::Rendered(data) => data,
        }
    }
}

#[derive(Clone)]
pub struct RenderableGlyph {
    pub x: Fixed<i32, 8>,
    pub y: Fixed<i32, 8>,
    pub width: PhysicalLength,
    pub height: PhysicalLength,
    pub alpha_map: GlyphAlphaMap,
    pub pixel_stride: u16,
    pub sdf: bool,
}

impl RenderableGlyph {
    pub fn size(&self) -> PhysicalSize {
        PhysicalSize::from_lengths(self.width, self.height)
    }
}

/// The pixels of a glyph rendered from a vector font.
#[cfg(any(feature = "systemfonts", feature = "embedded-vector-fonts"))]
#[derive(Clone)]
pub enum VectorGlyphPixels {
    /// One coverage value per pixel, drawn in the text's color.
    AlphaMap(Rc<[u8]>),
    /// Premultiplied RGBA pixels of a color glyph, such as an emoji, drawn as they are.
    Color(SharedPixelBuffer<Rgba8Pixel>),
}

// Subset of `RenderableGlyph`, specifically for VectorFonts.
#[cfg(any(feature = "systemfonts", feature = "embedded-vector-fonts"))]
#[derive(Clone)]
pub struct RenderableVectorGlyph {
    pub x: Fixed<i32, 8>,
    pub y: Fixed<i32, 8>,
    pub width: PhysicalLength,
    pub height: PhysicalLength,
    pub pixels: VectorGlyphPixels,
    pub pixel_stride: u16,
    /// Only the parley glyph-run path (`systemfonts`) positions glyphs at
    /// sub-pixel offsets; the embedded path renders the offset into the bitmap.
    #[cfg_attr(
        all(feature = "embedded-vector-fonts", not(feature = "systemfonts")),
        allow(dead_code)
    )]
    pub glyph_origin_x: f32,
}

#[cfg(any(feature = "systemfonts", feature = "embedded-vector-fonts"))]
impl RenderableVectorGlyph {
    #[cfg_attr(
        all(feature = "embedded-vector-fonts", not(feature = "systemfonts")),
        allow(dead_code)
    )]
    pub fn size(&self) -> PhysicalSize {
        PhysicalSize::from_lengths(self.width, self.height)
    }

    /// The number of bytes the glyph's pixels take, for the glyph cache.
    pub fn byte_size(&self) -> usize {
        match &self.pixels {
            VectorGlyphPixels::AlphaMap(alpha_map) => alpha_map.len(),
            VectorGlyphPixels::Color(buffer) => buffer.as_bytes().len(),
        }
    }
}

pub trait GlyphRenderer {
    fn render_glyph(
        &self,
        glyph_id: core::num::NonZeroU16,
        slint_context: &i_slint_core::SlintContext,
    ) -> Option<RenderableGlyph>;
    /// The amount of pixel in the original image that correspond to one pixel in the rendered image
    fn scale_delta(&self) -> Fixed<u16, 8>;
}

pub(super) use i_slint_core::textlayout::DEFAULT_FONT_SIZE;

mod pixelfont;
#[cfg(any(feature = "systemfonts", feature = "embedded-vector-fonts"))]
pub mod vectorfont;

#[cfg(feature = "systemfonts")]
pub mod systemfonts;

#[cfg(all(feature = "embedded-vector-fonts", not(feature = "systemfonts")))]
pub mod embeddedfonts;

#[derive(derive_more::From)]
pub enum Font {
    Pixel(PixelFont),
    #[cfg(feature = "owned-pre-rendered-resources")]
    SharedPixel(PixelFont<OwnedStorage>),
    #[cfg(any(feature = "systemfonts", feature = "embedded-vector-fonts"))]
    Vector(vectorfont::VectorFont),
}

/// Runs `$body` with `$bound` bound to the concrete font held by `$font`.
///
/// The bitmap and vector paths through the text code are the same code; they need two
/// match arms only because `PixelFont` and `VectorFont` are distinct types. The body is
/// monomorphized per variant, the way a generic function would be, so that it can be
/// written once.
///
/// Keep only font-dependent work in the body: it is emitted once per variant.
///
/// Callers that hand off to parley for vector fonts must do so before calling this, as
/// `sharedparley::` needs the font context rather than a laid-out font.
macro_rules! with_font {
    ($font:expr, |$bound:ident| $body:block) => {
        match $font {
            $crate::fonts::Font::Pixel($bound) => $body,
            #[cfg(feature = "owned-pre-rendered-resources")]
            $crate::fonts::Font::SharedPixel($bound) => $body,
            #[cfg(any(feature = "systemfonts", feature = "embedded-vector-fonts"))]
            $crate::fonts::Font::Vector($bound) => $body,
        }
    };
}
pub(crate) use with_font;

impl i_slint_core::textlayout::FontMetrics<PhysicalLength> for Font {
    fn ascent(&self) -> PhysicalLength {
        with_font!(self, |font| { font.ascent() })
    }

    fn height(&self) -> PhysicalLength {
        with_font!(self, |font| { font.height() })
    }

    fn descent(&self) -> PhysicalLength {
        with_font!(self, |font| { font.descent() })
    }

    fn x_height(&self) -> PhysicalLength {
        with_font!(self, |font| { font.x_height() })
    }

    fn cap_height(&self) -> PhysicalLength {
        with_font!(self, |font| { font.cap_height() })
    }
}

/// Scores an embedded bitmap font against the requested axis settings:
/// sum of |requested − rasterized| over the requested axes plus
/// |rasterized − fvar default| over axes the request didn't pin. `wght` is
/// carried by the font's `weight` field. Axes the font doesn't declare are
/// inert and score zero — no bitmap or vector font could do better.
fn axis_score<S: Storage>(
    font: &BitmapFont<S>,
    requested_weight: u16,
    requested_variations: &[(i_slint_core::SharedString, f32)],
    opsz_is_explicit: bool,
    scale_factor: ScaleFactor,
) -> f32 {
    const WGHT: u32 = u32::from_be_bytes(*b"wght");
    const OPSZ: u32 = u32::from_be_bytes(*b"opsz");

    let mut score = (font.weight as f32 - requested_weight as f32).abs();
    for (tag, requested) in requested_variations {
        let Ok(tag_bytes) = <[u8; 4]>::try_from(tag.as_bytes()) else { continue };
        let tag = u32::from_be_bytes(tag_bytes);
        if tag == WGHT {
            continue;
        }
        if tag == OPSZ && font.auto_opsz {
            if !opsz_is_explicit {
                continue;
            }
            // A pinned opsz only matches glyph sets rasterized near it.
            score += font
                .glyphs
                .iter()
                .map(|glyphs| (glyphs.pixel_size as f32 / scale_factor.get() - requested).abs())
                .fold(f32::MAX, f32::min);
            continue;
        }
        if let Some(axis) = font.variations.iter().find(|axis| axis.tag == tag) {
            score += (axis.value - requested).abs();
        }
    }
    for axis in font.variations.iter() {
        if !requested_variations.iter().any(|(tag, _)| {
            tag.as_bytes().try_into().map(u32::from_be_bytes).ok() == Some(axis.tag)
        }) {
            score += (axis.value - axis.default_value).abs();
        }
    }
    score
}

pub fn match_font(
    request: &FontRequest,
    scale_factor: ScaleFactor,
    #[cfg(feature = "systemfonts")]
    font_context: &mut i_slint_core::textlayout::sharedparley::FontContext,
) -> Font {
    let requested_weight = request
        .weight
        .and_then(|weight| weight.try_into().ok())
        .unwrap_or(/* CSS normal */ 400);
    let requested_variations = request.effective_variations(
        #[allow(clippy::unnecessary_cast)] // Coord is f32, but i16 with `slint_int_coord`
        request.pixel_size.map(|s| s.get() as f32),
    );
    // `opsz` entries in `font-variation-settings` pin the axis explicitly;
    // otherwise it comes from `font-optical-sizing` and an `auto_opsz` bitmap
    // font satisfies it per glyph set.
    let opsz_is_explicit = request.variations.iter().any(|entry| entry.tag.as_str() == "opsz");
    let requested_pixel_size: PhysicalLength =
        (request.pixel_size.unwrap_or(DEFAULT_FONT_SIZE).cast() * scale_factor).cast();
    let nearest_weight = |family: Option<&str>, italic: Option<bool>| {
        BITMAP_FONTS.with(|fonts| {
            fonts
                .borrow()
                .iter()
                .filter_map(|font| {
                    Some((font.weight_distance(family, italic, requested_weight)?, font))
                })
                .min_by_key(|(distance, _)| *distance)
                .and_then(|(_, font)| font.at_pixel_size(requested_pixel_size))
        })
    };

    let best_bitmap_font = request.family.as_ref().and_then(|family| {
        BITMAP_FONTS.with(|fonts| {
            fonts
                .borrow()
                .iter()
                .filter_map(|font| {
                    let score = font.axis_score(
                        family.as_str(),
                        request.italic,
                        requested_weight,
                        &requested_variations,
                        opsz_is_explicit,
                        scale_factor,
                    )?;
                    Some((score, font.at_pixel_size(requested_pixel_size)?))
                })
                .min_by(|(a, _), (b, _)| a.total_cmp(b))
        })
    });
    match best_bitmap_font {
        Some((score, font)) if score == 0. => return font,
        // The best bitmap font doesn't cover the requested axes: prefer a
        // vector font that can rasterize the exact instance.
        best_bitmap_font => {
            #[cfg(feature = "systemfonts")]
            if let Some(vectorfont) = systemfonts::match_font(request, scale_factor, font_context) {
                return vectorfont.into();
            }
            #[cfg(all(feature = "embedded-vector-fonts", not(feature = "systemfonts")))]
            if let Some(vectorfont) = embeddedfonts::match_font(request, scale_factor) {
                return vectorfont.into();
            }
            if let Some((_, font)) = best_bitmap_font {
                return font;
            }
        }
    }
    if let Some(font) =
        nearest_weight(None, Some(request.italic)).or_else(|| nearest_weight(None, None))
    {
        return font;
    }
    #[cfg(feature = "systemfonts")]
    return systemfonts::fallbackfont(request, scale_factor, font_context).into();
    #[cfg(all(feature = "embedded-vector-fonts", not(feature = "systemfonts")))]
    if let Some(vectorfont) = embeddedfonts::fallback_font(request, scale_factor) {
        return vectorfont.into();
    }
    #[cfg(not(feature = "systemfonts"))]
    panic!(
        "No font fallback found. The software renderer requires enabling the `EmbedForSoftwareRenderer` option when compiling slint files."
    )
}

pub fn text_layout_for_font<'a, Font>(
    font: &'a Font,
    font_request: &FontRequest,
    line_height_factor: Option<f32>,
    scale_factor: ScaleFactor,
) -> TextLayout<'a, Font>
where
    Font: i_slint_core::textlayout::AbstractFont
        + i_slint_core::textlayout::TextShaper<Length = PhysicalLength>,
{
    let letter_spacing =
        font_request.letter_spacing.map(|spacing| (spacing.cast() * scale_factor).cast());
    let line_height = font_request
        .line_height
        .map(|line_height| line_height.get() as f32 * scale_factor.get())
        .or_else(|| line_height_factor.map(|factor| font.height().get() as f32 * factor))
        .map(|line_height| {
            PhysicalLength::new(num_traits::Float::round(line_height).max(0.) as i16)
        });

    TextLayout { font, letter_spacing, line_height }
}

pub fn register_bitmap_font(font_data: &'static BitmapFont) {
    register(RegisteredFont::Static(font_data));
}

#[cfg(feature = "owned-pre-rendered-resources")]
pub fn register_shared_bitmap_font(font_data: &Rc<BitmapFont<OwnedStorage>>) {
    register(RegisteredFont::Shared(Rc::downgrade(font_data)));
}

fn register(font: RegisteredFont) {
    BITMAP_FONTS.with(|fonts| {
        let mut fonts = fonts.borrow_mut();
        #[cfg(feature = "owned-pre-rendered-resources")]
        fonts.retain(
            |font| !matches!(font, RegisteredFont::Shared(font) if font.strong_count() == 0),
        );
        if !fonts.contains(&font) {
            fonts.push(font);
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::boxed::Box;
    use i_slint_core::lengths::LogicalLength;
    use i_slint_core::slice::Slice;
    use i_slint_core::textlayout::{FontMetrics, Glyph, TextShaper};

    struct TestFont;

    impl FontMetrics<PhysicalLength> for TestFont {
        fn ascent(&self) -> PhysicalLength {
            PhysicalLength::new(18)
        }

        fn descent(&self) -> PhysicalLength {
            PhysicalLength::new(-6)
        }

        fn x_height(&self) -> PhysicalLength {
            PhysicalLength::new(10)
        }

        fn cap_height(&self) -> PhysicalLength {
            PhysicalLength::new(14)
        }
    }

    impl TextShaper for TestFont {
        type LengthPrimitive = i16;
        type Length = PhysicalLength;

        fn shape_text<GlyphStorage: core::iter::Extend<Glyph<Self::Length>>>(
            &self,
            _text: &str,
            _glyphs: &mut GlyphStorage,
        ) {
        }

        fn glyph_for_char(&self, _ch: char) -> Option<Glyph<Self::Length>> {
            None
        }
    }

    #[test]
    fn registering_a_font_again_keeps_one_entry() {
        static FONT: BitmapFont = BitmapFont {
            family_name: Slice::from_slice(b"Test"),
            character_map: Slice::from_slice(&[]),
            units_per_em: 1.,
            ascent: 0.,
            descent: 0.,
            x_height: 0.,
            cap_height: 0.,
            glyphs: Slice::from_slice(&[]),
            weight: 400,
            italic: false,
            sdf: false,
            variations: Slice::from_slice(&[]),
            auto_opsz: false,
        };
        register_bitmap_font(&FONT);
        register_bitmap_font(&FONT);
        assert_eq!(BITMAP_FONTS.with(|fonts| fonts.borrow().len()), 1);
    }

    #[test]
    #[cfg(feature = "owned-pre-rendered-resources")]
    fn a_shared_font_goes_away_with_its_owner() {
        let font = Rc::new(BitmapFont::<OwnedStorage> {
            family_name: b"Shared".to_vec(),
            character_map: Vec::new(),
            units_per_em: 1.,
            ascent: 0.,
            descent: 0.,
            x_height: 0.,
            cap_height: 0.,
            glyphs: alloc::vec![i_slint_core::graphics::BitmapGlyphs::<OwnedStorage> {
                pixel_size: 12,
                glyph_data: Vec::new(),
            }],
            weight: 400,
            italic: false,
            sdf: false,
        });
        let registered = || {
            BITMAP_FONTS.with(|fonts| {
                fonts
                    .borrow()
                    .iter()
                    .filter(|font| font.weight_distance(Some("Shared"), None, 400).is_some())
                    .count()
            })
        };
        register_shared_bitmap_font(&font);
        register_shared_bitmap_font(&font);
        assert_eq!(registered(), 1);
        drop(font);
        assert_eq!(registered(), 0);
    }

    #[test]
    fn line_height_factor_scales_natural_height() {
        let font_request =
            FontRequest { pixel_size: Some(LogicalLength::new(20.)), ..Default::default() };

        let layout =
            text_layout_for_font(&TestFont, &font_request, Some(1.5), ScaleFactor::new(1.));

        assert_eq!(TestFont.height(), PhysicalLength::new(24));
        assert_eq!(layout.line_height, Some(PhysicalLength::new(36)));
    }

    #[test]
    fn line_height_factor_zero_collapses_lines() {
        let font_request =
            FontRequest { pixel_size: Some(LogicalLength::new(20.)), ..Default::default() };

        let layout = text_layout_for_font(&TestFont, &font_request, Some(0.), ScaleFactor::new(1.));

        assert_eq!(layout.line_height, Some(PhysicalLength::new(0)));
    }

    #[test]
    fn line_height_absolute_takes_precedence() {
        let font_request = FontRequest {
            pixel_size: Some(LogicalLength::new(20.)),
            line_height: Some(LogicalLength::new(10.)),
            ..Default::default()
        };

        let layout = text_layout_for_font(&TestFont, &font_request, Some(2.), ScaleFactor::new(1.));
        assert_eq!(layout.line_height, Some(PhysicalLength::new(10)));

        let layout = text_layout_for_font(&TestFont, &font_request, Some(2.), ScaleFactor::new(2.));
        assert_eq!(layout.line_height, Some(PhysicalLength::new(20)));
    }

    fn bitmap(weight: u16, variations: &[(u32, f32, f32)], auto_opsz: bool) -> &'static BitmapFont {
        let variations: &'static [i_slint_core::graphics::BitmapFontVariation] =
            Box::leak(
                variations
                    .iter()
                    .map(|&(tag, value, default_value)| {
                        i_slint_core::graphics::BitmapFontVariation { tag, value, default_value }
                    })
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
            );
        Box::leak(Box::new(BitmapFont {
            family_name: b"test".as_slice().into(),
            character_map: i_slint_core::slice::Slice::default(),
            units_per_em: 1000.,
            ascent: 0.8,
            descent: -0.2,
            x_height: 0.5,
            cap_height: 0.7,
            glyphs: i_slint_core::slice::Slice::default(),
            weight,
            italic: false,
            sdf: false,
            variations: variations.into(),
            auto_opsz,
        }))
    }

    const WDTH: u32 = u32::from_be_bytes(*b"wdth");
    const GRAD: u32 = u32::from_be_bytes(*b"GRAD");

    fn variations(entries: &[(&str, f32)]) -> Vec<(i_slint_core::SharedString, f32)> {
        entries.iter().map(|(t, v)| (i_slint_core::SharedString::from(*t), *v)).collect()
    }

    #[test]
    fn axis_score_exact_match_is_zero() {
        let font = bitmap(700, &[(WDTH, 75., 100.)], false);
        assert_eq!(
            axis_score(font, 700, &variations(&[("wdth", 75.)]), false, ScaleFactor::new(1.)),
            0.
        );
    }

    #[test]
    fn axis_score_counts_unpinned_axes_off_default() {
        // Asking only for the weight penalizes a bitmap rasterized away from
        // the wdth default: score 0 beats it, so the vector path wins.
        let default = bitmap(400, &[(WDTH, 100., 100.)], false);
        let narrow = bitmap(400, &[(WDTH, 75., 100.)], false);
        let none = variations(&[]);
        assert_eq!(axis_score(default, 400, &none, false, ScaleFactor::new(1.)), 0.);
        assert_eq!(axis_score(narrow, 400, &none, false, ScaleFactor::new(1.)), 25.);
    }

    #[test]
    fn axis_score_weight_distance_only() {
        // wght lives in `weight`, never in `variations`.
        let font = bitmap(400, &[(WDTH, 100., 100.)], false);
        let w = variations(&[("wght", 700.)]);
        assert_eq!(axis_score(font, 700, &w, false, ScaleFactor::new(1.)), 300.);
    }

    #[test]
    fn axis_score_unknown_requested_axes_are_inert() {
        // An axis the font doesn't declare can't be satisfied by anyone.
        let font = bitmap(400, &[(WDTH, 100., 100.)], false);
        let g = variations(&[("GRAD", 50.)]);
        assert_eq!(axis_score(font, 400, &g, false, ScaleFactor::new(1.)), 0.);
        let _ = GRAD;
    }
}
