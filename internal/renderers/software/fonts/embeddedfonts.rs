// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Font matching for `embedded-vector-fonts` builds: the compiler emits the
//! vector font data a text binding uses into the binary, and this module keeps
//! the registered fonts and resolves a [`super::FontRequest`] against them —
//! including the variable font's normalized axis coordinates, so an animated
//! `font-variation-settings` binding rasterizes the exact instance.
//!
//! Unlike `systemfonts`, nothing here queries a host font collection; there is
//! no `std` requirement. Shaping uses the renderer's local layout code
//! (`TextShaper` via swash), so the complex-script shaping the parley path
//! adds (right-to-left scripts, ligatures, kerning) is not available — the
//! same limitation the pre-rendered bitmap fonts have.

use alloc::vec::Vec;
use core::cell::RefCell;
use i_slint_core::api::ToSharedString as _;

use skrifa::MetadataProvider as _;

use i_slint_core::graphics::FontRequest;
use i_slint_core::lengths::ScaleFactor;

use super::vectorfont::VectorFont;
use super::{DEFAULT_FONT_SIZE, PhysicalLength};

/// A registered embedded font face: the `&'static` data plus the swash lookup
/// info and the family name and fvar axes parsed once at registration.
struct EmbeddedFont {
    data: &'static [u8],
    font_index: u32,
    swash_key: swash::CacheKey,
    swash_offset: u32,
    family: i_slint_core::SharedString,
    /// fvar axes as (tag, min, default, max), in font order. Empty for a
    /// non-variable font.
    axes: Vec<(u32, f32, f32, f32)>,
    /// Whether the face is italic/oblique, for `font-italic` matching.
    italic: bool,
}

i_slint_core::thread_local! {
    static EMBEDDED_FONTS: RefCell<Vec<EmbeddedFont>> = RefCell::default()
}

fn localized_string(
    face: &skrifa::FontRef<'_>,
    id: skrifa::string::StringId,
) -> i_slint_core::SharedString {
    face.localized_strings(id)
        .find(|s| s.language().is_none_or(|l| l.starts_with("en")))
        .or_else(|| face.localized_strings(id).next())
        .map(|s| s.to_shared_string())
        .unwrap_or_default()
}

/// Parses `data` and registers every face it contains with the embedded font
/// set. Called by generated code through `Renderer::register_font_from_memory`
/// for each font the compiler decided must be available at runtime (the
/// `exclude_vector_fonts` compiler option keeps this from ever being emitted on
/// targets without a rasterizer).
pub fn register(data: &'static [u8]) -> Result<(), alloc::boxed::Box<dyn core::error::Error>> {
    let mut index = 0u32;
    loop {
        let (Ok(face), Some(font_ref)) = (
            skrifa::FontRef::from_index(data, index),
            swash::FontRef::from_index(data, index as usize),
        ) else {
            break;
        };
        let axes: Vec<(u32, f32, f32, f32)> = face
            .axes()
            .iter()
            .map(|axis| {
                (
                    u32::from_be_bytes(axis.tag().to_be_bytes()),
                    axis.min_value(),
                    axis.default_value(),
                    axis.max_value(),
                )
            })
            .collect();
        let subfamily = localized_string(&face, skrifa::string::StringId::SUBFAMILY_NAME);
        let subfamily = subfamily.as_str();
        let italic = {
            let l = subfamily.to_lowercase();
            l.contains("italic")
                || l.contains("oblique")
                // `slnt`/`ital` axes can produce an oblique instance.
                || axes.iter().any(|(tag, ..)| *tag == u32::from_be_bytes(*b"slnt"))
        };
        EMBEDDED_FONTS.with(|fonts| {
            fonts.borrow_mut().push(EmbeddedFont {
                data,
                font_index: index,
                swash_key: font_ref.key,
                swash_offset: font_ref.offset,
                family: localized_string(&face, skrifa::string::StringId::FAMILY_NAME),
                axes,
                italic,
            })
        });
        index += 1;
    }
    if index == 0 { Err("not a valid font".into()) } else { Ok(()) }
}

/// Normalizes the request's effective axis values (user space) to F2Dot14 in
/// the font's fvar axis order, ready for
/// [`VectorFont::new_from_blob_and_index_with_coords`]. Axes the request
/// doesn't pin resolve to the fvar default, matching CSS' "unspecified axes
/// use the font's default".
fn normalized_coords(
    font: &EmbeddedFont,
    variations: &[(i_slint_core::SharedString, f32)],
) -> Vec<i16> {
    font.axes
        .iter()
        .map(|(tag, min, default, max)| {
            let value = variations
                .iter()
                .find(|(t, _)| t.as_bytes().try_into().map(u32::from_be_bytes).ok() == Some(*tag))
                .map(|(_, v)| *v)
                .unwrap_or(*default)
                .clamp(*min, *max);
            let t = if value < *default && default > min {
                (value - *default) / (*default - *min)
            } else if value > *default && max > default {
                (value - *default) / (*max - *default)
            } else {
                0.
            };
            skrifa::instance::NormalizedCoord::from_f32(t).to_bits()
        })
        .collect()
}

fn make_vector_font(
    font: &EmbeddedFont,
    request: &FontRequest,
    scale_factor: ScaleFactor,
) -> VectorFont {
    let requested_pixel_size: PhysicalLength =
        (request.pixel_size.unwrap_or(DEFAULT_FONT_SIZE).cast() * scale_factor).cast();
    let variations = request.effective_variations(request.pixel_size.map(|s| s.get() as f32));
    VectorFont::new_from_blob_and_index_with_coords(
        font.data.into(),
        font.font_index,
        font.swash_key,
        font.swash_offset,
        requested_pixel_size,
        &normalized_coords(font, &variations),
    )
}

/// Scores a registered embedded font against the request. `None` excludes the
/// face: a requested family must match and an italic request needs an italic
/// face. Otherwise the score is the sum over requested axes of how far each
/// request sits outside the font's fvar range — 0 when every axis is covered.
fn score(font: &EmbeddedFont, request: &FontRequest) -> Option<f32> {
    if let Some(family) = request.family.as_ref()
        && !family.eq_ignore_ascii_case(&font.family)
    {
        return None;
    }
    if request.italic && !font.italic {
        return None;
    }
    let variations = request.effective_variations(request.pixel_size.map(|s| s.get() as f32));
    Some(
        variations
            .iter()
            .map(|(tag, value)| {
                let Some(t) = <[u8; 4]>::try_from(tag.as_bytes()).ok() else { return 0. };
                let tag = u32::from_be_bytes(t);
                match font.axes.iter().find(|(axis_tag, ..)| *axis_tag == tag) {
                    Some((_, min, _, max)) => (*value - (*value).clamp(*min, *max)).abs(),
                    // The axis isn't in the font — a hefty penalty so a font
                    // that covers the request wins, without making a covered
                    // axis beat a family match.
                    None => 1000.,
                }
            })
            .sum(),
    )
}

/// Finds the best registered embedded font for the request, or `None` when
/// nothing matches the requested family.
pub fn match_font(request: &FontRequest, scale_factor: ScaleFactor) -> Option<VectorFont> {
    EMBEDDED_FONTS.with(|fonts| {
        let fonts = fonts.borrow();
        fonts
            .iter()
            .filter_map(|font| score(font, request).map(|score| (font, score)))
            .min_by(|(_, a), (_, b)| a.total_cmp(b))
            .map(|(font, _)| make_vector_font(font, request, scale_factor))
    })
}

/// The font to use when the request names no (or an unknown) family: the first
/// registered one — the compiler emits the default font's data first.
pub fn fallback_font(request: &FontRequest, scale_factor: ScaleFactor) -> Option<VectorFont> {
    EMBEDDED_FONTS.with(|fonts| {
        fonts.borrow().first().map(|font| make_vector_font(font, request, scale_factor))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use i_slint_core::SharedString;
    use i_slint_core::items::FontVariation;
    use i_slint_core::model::{ModelRc, VecModel};
    use i_slint_core::textlayout::TextShaper as _;

    // The test fonts used by the screenshot tests: Noto Sans covers wght
    // 100–900 and wdth 62.5–100; Roboto Flex adds opsz, GRAD, ROND, slnt.
    static NOTO_SANS: &[u8] =
        include_bytes!("../../../../tests/screenshots/fonts/NotoSans-Regular.ttf");
    static ROBOTO_FLEX: &[u8] =
        include_bytes!("../../../../tests/screenshots/fonts/RobotoFlex.ttf");

    fn request(family: &str, variations: &[FontVariation]) -> FontRequest {
        FontRequest {
            family: Some(SharedString::from(family)),
            variations: ModelRc::new(VecModel::from(variations.to_vec())),
            ..Default::default()
        }
    }

    fn axis(tag: &str, value: f32) -> FontVariation {
        FontVariation { tag: SharedString::from(tag), value }
    }

    /// One test function: `EMBEDDED_FONTS` is a single-threaded global, so the
    /// assertions share it rather than racing parallel registrations.
    #[test]
    fn registers_and_matches_end_to_end() {
        assert!(register(&[]).is_err());
        assert!(register(&[0u8; 128]).is_err());
        register(NOTO_SANS).unwrap();
        register(ROBOTO_FLEX).unwrap();

        assert!(
            match_font(&request("Noto Sans", &[]), ScaleFactor::new(1.)).is_some(),
            "a registered family must match"
        );
        assert!(
            match_font(&request("No Such Family", &[]), ScaleFactor::new(1.)).is_none(),
            "an unknown family must not match"
        );
        assert!(
            fallback_font(&request("No Such Family", &[]), ScaleFactor::new(1.)).is_some(),
            "the fallback resolves even for an unknown family"
        );
        // GRAD and ROND are declared only by Roboto Flex.
        assert!(
            match_font(
                &request("Roboto Flex", &[axis("GRAD", 50.), axis("ROND", 70.)]),
                ScaleFactor::new(1.),
            )
            .is_some(),
            "declared axes on the named family must match"
        );

        // The request's axis values reach the rasterizer as normalized
        // coordinates: a narrower `wdth` produces a narrower advance.
        let narrow =
            match_font(&request("Noto Sans", &[axis("wdth", 62.5)]), ScaleFactor::new(1.)).unwrap();
        let wide =
            match_font(&request("Noto Sans", &[axis("wdth", 100.)]), ScaleFactor::new(1.)).unwrap();
        let narrow_advance = narrow.glyph_for_char('A').unwrap().advance;
        let wide_advance = wide.glyph_for_char('A').unwrap().advance;
        assert!(
            narrow_advance < wide_advance,
            "wdth 62.5 must shape narrower than wdth 100: {narrow_advance:?} vs {wide_advance:?}"
        );
    }
}
