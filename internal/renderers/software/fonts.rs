// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore pixelfont vectorfont
use alloc::rc::Rc;
use alloc::vec::Vec;
use core::cell::RefCell;

use super::{Fixed, PhysicalLength, PhysicalSize};
use i_slint_core::graphics::{BitmapFont, FontRequest};
use i_slint_core::lengths::ScaleFactor;
use i_slint_core::model::Model;
use i_slint_core::textlayout::TextLayout;

i_slint_core::thread_local! {
    static BITMAP_FONTS: RefCell<Vec<&'static BitmapFont>> = RefCell::default()
}

#[derive(derive_more::From, Clone)]
pub enum GlyphAlphaMap {
    Static(&'static [u8]),
    Shared(Rc<[u8]>),
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

// Subset of `RenderableGlyph`, specifically for VectorFonts.
#[cfg(feature = "systemfonts")]
#[derive(Clone)]
pub struct RenderableVectorGlyph {
    pub x: Fixed<i32, 8>,
    pub y: Fixed<i32, 8>,
    pub width: PhysicalLength,
    pub height: PhysicalLength,
    pub alpha_map: Rc<[u8]>,
    pub pixel_stride: u16,
    pub glyph_origin_x: f32,
}

#[cfg(feature = "systemfonts")]
impl RenderableVectorGlyph {
    pub fn size(&self) -> PhysicalSize {
        PhysicalSize::from_lengths(self.width, self.height)
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
#[cfg(feature = "systemfonts")]
pub mod vectorfont;

#[cfg(feature = "systemfonts")]
pub mod systemfonts;

#[derive(derive_more::From)]
pub enum Font {
    PixelFont(pixelfont::PixelFont),
    #[cfg(feature = "systemfonts")]
    VectorFont(vectorfont::VectorFont),
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
            $crate::fonts::Font::PixelFont($bound) => $body,
            #[cfg(feature = "systemfonts")]
            $crate::fonts::Font::VectorFont($bound) => $body,
        }
    };
}
pub(crate) use with_font;

/// Returns the size of the pre-rendered font in pixels.
pub fn pixel_size(glyphs: &i_slint_core::graphics::BitmapGlyphs) -> PhysicalLength {
    PhysicalLength::new(glyphs.pixel_size)
}

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
fn axis_score(
    font: &'static BitmapFont,
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
    font_context: &mut i_slint_core::textlayout::sharedparley::parley::FontContext,
) -> Font {
    let requested_weight = request
        .weight
        .and_then(|weight| weight.try_into().ok())
        .unwrap_or(/* CSS normal */ 400);

    let requested_variations =
        request.effective_variations(request.pixel_size.map(|s| s.get() as f32));
    // `opsz` entries in `font-variation-settings` pin the axis explicitly;
    // otherwise it comes from `font-optical-sizing` and an `auto_opsz` bitmap
    // font satisfies it per glyph set.
    let opsz_is_explicit = request.variations.iter().any(|entry| entry.tag.as_str() == "opsz");

    let bitmap_font = BITMAP_FONTS.with(|fonts| {
        let fonts = fonts.borrow();

        request.family.as_ref().and_then(|requested_family| {
            fonts
                .iter()
                .filter(|bitmap_font| {
                    core::str::from_utf8(bitmap_font.family_name.as_slice()).unwrap()
                        == requested_family.as_str()
                        && bitmap_font.italic == request.italic
                })
                .map(|bitmap_font| {
                    (
                        *bitmap_font,
                        axis_score(
                            bitmap_font,
                            requested_weight,
                            &requested_variations,
                            opsz_is_explicit,
                            scale_factor,
                        ),
                    )
                })
                .min_by(|(_, a), (_, b)| a.total_cmp(b))
        })
    });

    let font = match bitmap_font {
        Some((bitmap_font, score)) if score == 0. => bitmap_font,
        Some((bitmap_font, _)) => {
            // The best bitmap font doesn't cover the requested axes: prefer a
            // vector font that can rasterize the exact instance.
            #[cfg(feature = "systemfonts")]
            if let Some(vectorfont) = systemfonts::match_font(
                request,
                scale_factor,
                &mut font_context.collection,
                &mut font_context.source_cache,
            ) {
                return vectorfont.into();
            }
            bitmap_font
        }
        None => {
            #[cfg(feature = "systemfonts")]
            if let Some(vectorfont) = systemfonts::match_font(
                request,
                scale_factor,
                &mut font_context.collection,
                &mut font_context.source_cache,
            ) {
                return vectorfont.into();
            }
            if let Some(fallback_bitmap_font) = BITMAP_FONTS.with(|fonts| {
                let fonts = fonts.borrow();
                fonts
                    .iter()
                    .cloned()
                    .filter(|bitmap_font| bitmap_font.italic == request.italic)
                    .min_by_key(|bitmap_font| bitmap_font.weight.abs_diff(requested_weight))
                    .or_else(|| fonts.first().cloned())
            }) {
                fallback_bitmap_font
            } else {
                #[cfg(feature = "systemfonts")]
                return systemfonts::fallbackfont(
                    request,
                    scale_factor,
                    &mut font_context.collection,
                    &mut font_context.source_cache,
                )
                .into();
                #[cfg(not(feature = "systemfonts"))]
                panic!(
                    "No font fallback found. The software renderer requires enabling the `EmbedForSoftwareRenderer` option when compiling slint files."
                )
            }
        }
    };

    let requested_pixel_size: PhysicalLength =
        (request.pixel_size.unwrap_or(DEFAULT_FONT_SIZE).cast() * scale_factor).cast();

    let nearest_pixel_size = font
        .glyphs
        .partition_point(|glyphs| pixel_size(glyphs) <= requested_pixel_size)
        .saturating_sub(1);
    let matching_glyphs = &font.glyphs[nearest_pixel_size];

    let pixel_size = if font.sdf { requested_pixel_size } else { pixel_size(matching_glyphs) };

    pixelfont::PixelFont { bitmap_font: font, glyphs: matching_glyphs, pixel_size }.into()
}

pub fn text_layout_for_font<'a, Font>(
    font: &'a Font,
    font_request: &FontRequest,
    scale_factor: ScaleFactor,
) -> TextLayout<'a, Font>
where
    Font: i_slint_core::textlayout::AbstractFont
        + i_slint_core::textlayout::TextShaper<Length = PhysicalLength>,
{
    let letter_spacing =
        font_request.letter_spacing.map(|spacing| (spacing.cast() * scale_factor).cast());
    let line_height = font_request.line_height_for_natural_height(font.height().get() as f32).map(
        |line_height| PhysicalLength::new(num_traits::Float::round(line_height).max(0.) as i16),
    );

    TextLayout { font, letter_spacing, line_height }
}

pub fn register_bitmap_font(font_data: &'static BitmapFont) {
    BITMAP_FONTS.with(|fonts| fonts.borrow_mut().push(font_data))
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::boxed::Box;
    use i_slint_core::lengths::LogicalLength;
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
    fn line_height_factor_scales_natural_height() {
        let font_request = FontRequest {
            pixel_size: Some(LogicalLength::new(20.)),
            line_height_factor: Some(1.5),
            ..Default::default()
        };

        let layout = text_layout_for_font(&TestFont, &font_request, ScaleFactor::new(1.));

        assert_eq!(TestFont.height(), PhysicalLength::new(24));
        assert_eq!(layout.line_height, Some(PhysicalLength::new(36)));
    }

    #[test]
    fn line_height_factor_zero_collapses_lines() {
        let font_request = FontRequest {
            pixel_size: Some(LogicalLength::new(20.)),
            line_height_factor: Some(0.),
            ..Default::default()
        };

        let layout = text_layout_for_font(&TestFont, &font_request, ScaleFactor::new(1.));

        assert_eq!(layout.line_height, Some(PhysicalLength::new(0)));
    }

    #[test]
    fn line_height_absolute_takes_precedence() {
        let font_request = FontRequest {
            pixel_size: Some(LogicalLength::new(20.)),
            line_height_factor: Some(2.),
            line_height: Some(LogicalLength::new(10.)),
            ..Default::default()
        };

        let layout = text_layout_for_font(&TestFont, &font_request, ScaleFactor::new(1.));

        assert_eq!(layout.line_height, Some(PhysicalLength::new(10)));
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
        assert_eq!(
            axis_score(default, 400, &none, false, ScaleFactor::new(1.)),
            0.
        );
        assert_eq!(
            axis_score(narrow, 400, &none, false, ScaleFactor::new(1.)),
            25.
        );
    }

    #[test]
    fn axis_score_weight_distance_only() {
        // wght lives in `weight`, never in `variations`.
        let font = bitmap(400, &[(WDTH, 100., 100.)], false);
        let w = variations(&[("wght", 700.)]);
        assert_eq!(
            axis_score(font, 700, &w, false, ScaleFactor::new(1.)),
            300.
        );
    }

    #[test]
    fn axis_score_unknown_requested_axes_are_inert() {
        // An axis the font doesn't declare can't be satisfied by anyone.
        let font = bitmap(400, &[(WDTH, 100., 100.)], false);
        let g = variations(&[("GRAD", 50.)]);
        assert_eq!(
            axis_score(font, 400, &g, false, ScaleFactor::new(1.)),
            0.
        );
        let _ = GRAD;
    }
}
