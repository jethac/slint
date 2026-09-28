// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore uncacheable unrepresentable

//! From text to shaped paragraphs.
//!
//! Everything that ends up in a cache entry is produced here, and [`shape_paragraphs`] is the
//! one function it happens through: measuring and drawing must register identical cache
//! dependencies, so neither may shape through anything narrower.

use super::*;
use crate::SharedString;
use crate::model::Model;

/// Font size of inline `code` runs, as a fraction of the surrounding body
/// text. Matches the convention used by GitHub-style markdown renderers — the
/// glyphs sit a little smaller than body text, inside a translucent capsule
/// that visually marks them as code.
const INLINE_CODE_FONT_SCALE: f32 = 0.85;

std::thread_local! {
    static LAYOUT_CONTEXT: RefCell<parley::LayoutContext<Brush>> = Default::default();
}

#[derive(Debug, Default, PartialEq, Clone, Copy)]
pub(super) struct Brush {
    /// When set, this overrides the fill/stroke to use this color.
    pub(super) override_fill_color: Option<Color>,
    pub(super) stroke: Option<TextStrokeStyle>,
    pub(super) link_color: Option<Color>,
}

pub(super) struct LayoutWithoutLineBreaksBuilder {
    font_request: Option<FontRequest>,
    pub(super) text_wrap: TextWrap,
    stroke: Option<TextStrokeStyle>,
    pub(super) scale_factor: ScaleFactor,
    pub(super) pixel_size: LogicalLength,
    /// When false, overlong words are not broken up. Only used to measure the
    /// min-content width (the longest word), never to lay text out for display.
    overflow_wrap_anywhere: bool,
}

impl LayoutWithoutLineBreaksBuilder {
    pub(super) fn new(
        font_request: Option<FontRequest>,
        text_wrap: TextWrap,
        stroke: Option<TextStrokeStyle>,
        scale_factor: ScaleFactor,
    ) -> Self {
        let pixel_size = font_request
            .as_ref()
            .and_then(|font_request| font_request.pixel_size)
            .unwrap_or(DEFAULT_FONT_SIZE);

        Self {
            font_request,
            text_wrap,
            stroke,
            scale_factor,
            pixel_size,
            overflow_wrap_anywhere: true,
        }
    }

    /// The builder plus the `FontVariations` axis list pushed as the default —
    /// callers keep it in [`TextParagraph::variations`] so that renderers can
    /// reconstruct a run's user-space axis settings.
    fn ranged_builder<'a>(
        &self,
        layout_ctx: &'a mut parley::LayoutContext<Brush>,
        font_ctx: &'a mut parley::FontContext,
        text: &'a str,
    ) -> (parley::RangedBuilder<'a, Brush>, Vec<parley::style::FontVariation>) {
        // Use the requested font's natural line-height ratio for every run so fallback fonts,
        // such as the symbol font used for password characters, don't enlarge the line box.
        // `FontSizeRelative` scales the result with each styled span's font size.
        let line_height_ratio =
            self.font_request.as_ref().and_then(|fr| line_height_ratio(font_ctx, fr));

        let mut builder = layout_ctx.ranged_builder(font_ctx, text, self.scale_factor.get(), false);
        let mut default_variations = Vec::new();

        if let Some(ratio) = line_height_ratio {
            builder.push_default(parley::StyleProperty::LineHeight(
                parley::style::LineHeight::FontSizeRelative(ratio),
            ));
        }

        if let Some(ref font_request) = self.font_request {
            let mut fallback_family_iter = sharedfontique::FALLBACK_FAMILIES
                .into_iter()
                .map(parley::style::FontFamilyName::Generic);

            let font_families: &[parley::style::FontFamilyName] = if let Some(family) =
                &font_request.family
            {
                let mut iter =
                    core::iter::once(parley::style::FontFamilyName::named(family.as_str()))
                        .chain(fallback_family_iter);
                &core::array::from_fn::<
                    _,
                    { sharedfontique::FALLBACK_FAMILIES.as_slice().len() + 1 },
                    _,
                >(|_| iter.next().unwrap())
            } else {
                &core::array::from_fn::<_, { sharedfontique::FALLBACK_FAMILIES.as_slice().len() }, _>(
                    |_| fallback_family_iter.next().unwrap(),
                )
            };

            builder.push_default(parley::style::FontFamily::List(std::borrow::Cow::Borrowed(
                font_families,
            )));

            if let Some(weight) = font_request.weight {
                builder.push_default(parley::StyleProperty::FontWeight(
                    parley::style::FontWeight::new(weight as f32),
                ));
            }
            if let Some(stretch) = font_request.stretch {
                builder.push_default(parley::StyleProperty::FontWidth(
                    parley::style::FontWidth::from_percentage(stretch),
                ));
            }
            default_variations = parley_variation_list(
                font_request.shaping_variations(Some(self.pixel_size.get())).drain(..),
            );
            if !default_variations.is_empty() {
                builder.push_default(parley::StyleProperty::FontVariations(
                    parley::style::FontVariations::List(std::borrow::Cow::Owned(
                        default_variations.clone(),
                    )),
                ));
            }
            if let Some(letter_spacing) = font_request.letter_spacing {
                builder.push_default(parley::StyleProperty::LetterSpacing(letter_spacing.get()));
            }
            builder.push_default(parley::StyleProperty::FontStyle(if font_request.italic {
                parley::style::FontStyle::Italic
            } else {
                parley::style::FontStyle::Normal
            }));
        }
        builder.push_default(parley::StyleProperty::FontSize(self.pixel_size.get()));
        builder.push_default(parley::StyleProperty::WordBreak(match self.text_wrap {
            TextWrap::NoWrap => parley::style::WordBreak::KeepAll,
            TextWrap::WordWrap => parley::style::WordBreak::Normal,
            TextWrap::CharWrap => parley::style::WordBreak::BreakAll,
        }));
        builder.push_default(parley::StyleProperty::OverflowWrap(
            match (self.text_wrap, self.overflow_wrap_anywhere) {
                (TextWrap::NoWrap, _) | (_, false) => parley::style::OverflowWrap::Normal,
                (TextWrap::WordWrap | TextWrap::CharWrap, true) => {
                    parley::style::OverflowWrap::Anywhere
                }
            },
        ));
        if self.text_wrap == TextWrap::NoWrap {
            // Parley 0.9 removed the width parameter from `Layout::align()` and instead
            // uses the `max_advance` set by `break_all_lines()` as the alignment container
            // width. To allow passing `max_physical_width` to `break_all_lines` for alignment
            // purposes without triggering actual line wrapping, we must set `TextWrapMode::NoWrap`.
            builder.push_default(parley::StyleProperty::TextWrapMode(
                parley::style::TextWrapMode::NoWrap,
            ));
        }

        builder.push_default(parley::StyleProperty::Brush(Brush {
            override_fill_color: None,
            stroke: self.stroke,
            link_color: None,
        }));

        (builder, default_variations)
    }

    /// Note that the selection is deliberately absent here: it is a rendering concern, not a
    /// styling one, and baking it into the layout both makes the layout uncacheable across
    /// selection changes and makes sub-glyph selection boundaries unrepresentable. See
    /// [`SelectionSpan`].
    ///
    /// Besides the layout, this returns the `FontVariations` axis lists pushed
    /// to the shaper (the paragraph default plus one list per styled range, in
    /// push order), which renderers need to reconstruct a run's user-space
    /// axis settings.
    pub(super) fn build(
        &self,
        font_context: &mut parley::FontContext,
        text: &str,
        formatting: impl IntoIterator<Item = i_slint_common::styled_text::FormattedSpan>,
        link_color: Option<Color>,
    ) -> (parley::Layout<Brush>, ParagraphVariations) {
        use i_slint_common::styled_text::Style;

        LAYOUT_CONTEXT.with_borrow_mut(|layout_ctx| {
            let (mut builder, default_variations) =
                self.ranged_builder(layout_ctx, font_context, text);
            let mut variation_ranges: Vec<(Range<usize>, Vec<parley::style::FontVariation>)> =
                Vec::new();

            // filter empty ranges otherwise parley will panic on assert
            for span in formatting.into_iter().filter(|s| !s.range.is_empty()) {
                match span.style {
                    Style::Emphasis => {
                        builder.push(
                            parley::StyleProperty::FontStyle(parley::style::FontStyle::Italic),
                            span.range,
                        );
                    }
                    Style::Strikethrough => {
                        builder.push(parley::StyleProperty::Strikethrough(true), span.range);
                    }
                    Style::Strong => {
                        builder.push(
                            parley::StyleProperty::FontWeight(parley::style::FontWeight::BOLD),
                            span.range,
                        );
                    }
                    Style::Code => {
                        builder.push(
                            parley::StyleProperty::FontFamily(parley::style::FontFamily::Single(
                                parley::style::FontFamilyName::Generic(
                                    parley::style::GenericFamily::Monospace,
                                ),
                            )),
                            span.range.clone(),
                        );
                        // Inline `code` reads as slightly smaller text on top of a
                        // translucent capsule (drawn separately in `TextParagraph::draw`),
                        // matching the convention used by common markdown renderers.
                        builder.push(
                            parley::StyleProperty::FontSize(
                                self.pixel_size.get() * INLINE_CODE_FONT_SCALE,
                            ),
                            span.range,
                        );
                    }
                    Style::Underline => {
                        builder.push(parley::StyleProperty::Underline(true), span.range);
                    }
                    Style::Link => {
                        builder.push(parley::StyleProperty::Underline(true), span.range.clone());
                        builder.push(
                            parley::StyleProperty::Brush(Brush {
                                override_fill_color: None,
                                stroke: self.stroke,
                                link_color,
                            }),
                            span.range,
                        );
                    }
                    Style::Color(color) => {
                        builder.push(
                            parley::StyleProperty::Brush(Brush {
                                override_fill_color: Some(crate::Color::from_argb_encoded(color)),
                                stroke: self.stroke,
                                link_color: None,
                            }),
                            span.range,
                        );
                    }
                    Style::FontTag(font_tag) => {
                        if let Some(color) = font_tag.color {
                            builder.push(
                                parley::StyleProperty::Brush(Brush {
                                    override_fill_color: Some(crate::Color::from_argb_encoded(
                                        color,
                                    )),
                                    stroke: self.stroke,
                                    link_color: None,
                                }),
                                span.range.clone(),
                            );
                        }
                        if let Some(stretch) = font_tag.font_stretch {
                            builder.push(
                                parley::StyleProperty::FontWidth(
                                    parley::style::FontWidth::from_percentage(stretch),
                                ),
                                span.range.clone(),
                            );
                        }
                        if font_tag.font_variation_settings.is_some()
                            || font_tag.font_optical_sizing.is_some()
                        {
                            let variations = self.font_tag_variations(&font_tag);
                            builder.push(
                                parley::StyleProperty::FontVariations(
                                    parley::style::FontVariations::List(std::borrow::Cow::Owned(
                                        variations.clone(),
                                    )),
                                ),
                                span.range.clone(),
                            );
                            variation_ranges.push((span.range, variations));
                        }
                    }
                }
            }

            (
                builder.build(text),
                ParagraphVariations { default_list: default_variations, ranges: variation_ranges },
            )
        })
    }

    /// The `FontVariations` list a `Style::FontTag` span pushes: `opsz` derived
    /// from the span's `font-optical-sizing` (inheriting the element's
    /// setting), then the span's `font-variation-settings` entries — or, when
    /// the span doesn't set the shorthand, the element's entries, matching CSS
    /// inheritance of the individual properties.
    fn font_tag_variations(
        &self,
        font_tag: &i_slint_common::styled_text::FontTagStyle,
    ) -> Vec<parley::style::FontVariation> {
        let optical_auto = font_tag.font_optical_sizing.unwrap_or_else(|| {
            self.font_request.as_ref().and_then(|f| f.optical_sizing).unwrap_or(true)
        });
        let mut entries: Vec<(SharedString, f32)> = Vec::new();
        if optical_auto {
            entries.push((SharedString::from("opsz"), self.pixel_size.get()));
        }
        match &font_tag.font_variation_settings {
            Some(settings) => {
                for (tag, value) in settings {
                    let tag = SharedString::from(tag.as_str());
                    if let Some(existing) = entries.iter_mut().find(|(t, _)| *t == tag) {
                        existing.1 = *value;
                    } else {
                        entries.push((tag, *value));
                    }
                }
            }
            None => {
                if let Some(request) = self.font_request.as_ref() {
                    crate::graphics::merge_variation_entries(
                        &mut entries,
                        request.variations.iter(),
                    );
                }
            }
        }
        parley_variation_list(entries.into_iter())
    }
}

/// Converts `(tag, value)` user-space pairs into parley `FontVariation`s.
/// Tags that aren't exactly four printable ASCII bytes are dropped; property
/// values are validated at compile time, so this only filters axis lists built
/// through an API binding.
fn parley_variation_list(
    entries: impl Iterator<Item = (SharedString, f32)>,
) -> Vec<parley::style::FontVariation> {
    entries
        .filter_map(|(tag, value)| {
            parley::setting::Tag::parse(tag.as_str())
                .map(|tag| parley::style::FontVariation::new(tag, value))
        })
        .collect()
}

/// The `FontVariations` axis lists pushed for one paragraph, so renderers can
/// reconstruct each run's user-space axis settings: parley shapes with
/// `synthesis.variation_settings()` followed by the list that applies to the
/// run's text range, and e.g. Skia needs the same merged list to pick the
/// typeface instance (`normalized_coords` is post-avar and not invertible).
pub(super) struct ParagraphVariations {
    /// The list pushed as the paragraph default style.
    pub default_list: Vec<parley::style::FontVariation>,
    /// Lists pushed for styled ranges, in push order — the last entry whose
    /// range covers the run wins, matching parley's range-style resolution.
    pub ranges: Vec<(Range<usize>, Vec<parley::style::FontVariation>)>,
}

impl ParagraphVariations {
    /// The list that applies to `range` — the last pushed covering range's,
    /// or the default list.
    pub(super) fn for_range(&self, range: &Range<usize>) -> &[parley::style::FontVariation] {
        self.ranges
            .iter()
            .rev()
            .find(|(r, _)| r.start <= range.start && r.end >= range.end)
            .map_or(&self.default_list[..], |(_, list)| list)
    }
}

/// Merges a font's synthesis settings with a pushed `FontVariations` list into
/// the run's user-space axis list, later entries overriding earlier ones with
/// the same tag. `(axis tag as big-endian u32, value)` pairs — the exact list
/// the shaper consumed for the run.
pub fn merged_variation_settings(
    synthesis: &fontique::Synthesis,
    pushed: &[parley::style::FontVariation],
) -> Vec<(u32, f32)> {
    let mut merged: Vec<(u32, f32)> = synthesis
        .variation_settings()
        .iter()
        .map(|(tag, value)| (u32::from_be_bytes(tag.to_be_bytes()), *value))
        .collect();
    for variation in pushed {
        let tag = u32::from_be_bytes(variation.tag.to_bytes());
        if let Some(existing) = merged.iter_mut().find(|(t, _)| *t == tag) {
            existing.1 = variation.value;
        } else {
            merged.push((tag, variation.value));
        }
    }
    merged
}

/// A font's synthesis settings followed by the request's pushed axis list —
/// the same axis settings the shaper applies to the default run style, so the
/// location metrics compute from (`axes().location()` applies last-wins per
/// axis and clamps to the fvar ranges itself).
pub(super) fn location_settings(
    synthesis: &fontique::Synthesis,
    font_request: &FontRequest,
) -> Vec<skrifa::setting::VariationSetting> {
    let mut settings: Vec<skrifa::setting::VariationSetting> = synthesis
        .variation_settings()
        .iter()
        .map(skrifa::setting::VariationSetting::from)
        .collect();
    settings.extend(
        font_request
            .shaping_variations(Some(font_request.pixel_size.unwrap_or(DEFAULT_FONT_SIZE).get()))
            .iter()
            .map(|(tag, value)| skrifa::setting::VariationSetting::from((tag.as_str(), *value))),
    );
    settings
}

/// The line-height ratio, relative to the font size, that every shaped line gets.
/// An absolute `FontRequest::line_height` is expressed as a ratio over the
/// requested font size: parley scales it back up with each span's physical size.
pub(super) fn line_height_ratio(
    font_ctx: &mut parley::FontContext,
    font_request: &FontRequest,
) -> Option<f32> {
    if let Some(line_height) = font_request.line_height {
        let pixel_size = font_request.pixel_size.unwrap_or(DEFAULT_FONT_SIZE);
        return (pixel_size.get() != 0.0).then(|| line_height.get() / pixel_size.get());
    }
    let font = font_request.query_fontique(&mut font_ctx.collection, &mut font_ctx.source_cache)?;
    let face = skrifa::FontRef::from_index(font.blob.data(), font.index).ok()?;
    let location = face.axes().location(location_settings(&font.synthesis, font_request));
    let metrics = face.metrics(skrifa::instance::Size::unscaled(), &location);
    let units_per_em = metrics.units_per_em as f32;
    (units_per_em > 0.0)
        .then(|| (metrics.ascent - metrics.descent + metrics.leading) / units_per_em)
        .map(|natural_ratio| {
            font_request.line_height_for_natural_height(natural_ratio).unwrap_or(natural_ratio)
        })
}

/// Splits plain text into paragraph byte ranges at `'\n'`. The `'\n'` and any preceding `'\r'`
/// are excluded from the range: parley treats a lone CR as a mandatory line break, so a CRLF
/// left in the paragraph would render an extra empty line.
pub(super) fn paragraph_ranges(text: &str) -> impl Iterator<Item = Range<usize>> + '_ {
    let mut start = 0;
    text.split('\n').map(move |paragraph| {
        let end = start + paragraph.len();
        let range = if paragraph.ends_with('\r') { start..end - 1 } else { start..end };
        start = end + 1;
        range
    })
}

pub(super) fn create_text_paragraphs(
    layout_builder: &LayoutWithoutLineBreaksBuilder,
    font_context: &mut parley::FontContext,
    text: PlainOrStyledText,
    link_color: Color,
) -> Vec<TextParagraph> {
    let paragraph_from_text =
        |font_context: &mut parley::FontContext,
         text: &str,
         range: std::ops::Range<usize>,
         formatting: Vec<i_slint_common::styled_text::FormattedSpan>,
         links: Vec<(std::ops::Range<usize>, std::string::String)>| {
            let code_ranges: alloc::vec::Vec<Range<usize>> = formatting
                .iter()
                .filter(|s| matches!(s.style, i_slint_common::styled_text::Style::Code))
                .map(|s| s.range.clone())
                .collect();

            let (layout, variations) =
                layout_builder.build(font_context, text, formatting, Some(link_color));

            TextParagraph {
                range,
                y: PhysicalLength::default(),
                layout,
                links,
                code_ranges,
                variations,
            }
        };

    let mut paragraphs = Vec::with_capacity(1);

    match text {
        PlainOrStyledText::Plain(ref text) => {
            for range in paragraph_ranges(text) {
                paragraphs.push(paragraph_from_text(
                    font_context,
                    &text[range.clone()],
                    range,
                    Default::default(),
                    Default::default(),
                ));
            }
        }
        PlainOrStyledText::Styled(rich_text) => {
            for paragraph in rich_text.paragraphs {
                paragraphs.push(paragraph_from_text(
                    font_context,
                    &paragraph.text,
                    0..0,
                    paragraph.formatting,
                    paragraph.links,
                ));
            }
        }
    };

    paragraphs
}

/// The builder the shaped paragraphs of `text` must be produced with. Measuring and drawing share
/// cache entries, so they have to agree on every input baked into the shaping -- which is why this
/// lives in one place rather than at each call site.
pub(super) fn shaping_builder(
    text: Pin<&dyn crate::item_rendering::RenderString>,
    item_rc: Option<&crate::item_tree::ItemRc>,
    text_wrap: TextWrap,
    scale_factor: ScaleFactor,
) -> LayoutWithoutLineBreaksBuilder {
    let (stroke_brush, _, stroke_style) = text.stroke();
    LayoutWithoutLineBreaksBuilder::new(
        item_rc.map(|irc| text.font_request(irc)),
        text_wrap,
        (!stroke_brush.is_transparent()).then_some(stroke_style),
        scale_factor,
    )
}

/// The builder for measuring content widths, without an item to derive one from.
///
/// `WordWrap` gives `WordBreak::Normal`, so the min-content width becomes the longest word.
/// Content widths are intrinsic to the text, so they don't depend on the item's actual wrap mode.
/// `overflow_wrap_anywhere` is off because parley may otherwise break anywhere to keep overlong
/// words from overflowing, which would make the min-content width a single character instead of
/// the longest word.
pub(super) fn content_widths_builder(
    font_request: FontRequest,
    scale_factor: ScaleFactor,
) -> LayoutWithoutLineBreaksBuilder {
    let mut builder = LayoutWithoutLineBreaksBuilder::new(
        Some(font_request),
        TextWrap::WordWrap,
        None,
        scale_factor,
    );
    builder.overflow_wrap_anywhere = false;
    builder
}

/// A builder for tests, which have no item to derive one from. Everything else obtains its
/// builder through [`shaping_builder`] or [`content_widths_builder`], so that it cannot disagree
/// with what the item's cache entry was shaped with.
#[cfg(test)]
pub(super) fn plain_builder_for_tests() -> LayoutWithoutLineBreaksBuilder {
    LayoutWithoutLineBreaksBuilder::new(None, TextWrap::NoWrap, None, ScaleFactor::new(1.0))
}

#[cfg(test)]
pub(super) fn wrap_builder_for_tests() -> LayoutWithoutLineBreaksBuilder {
    LayoutWithoutLineBreaksBuilder::new(None, TextWrap::WordWrap, None, ScaleFactor::new(1.0))
}

/// Shapes `text` the way both the drawing and the measuring paths need it, so that they can share
/// one cache entry. `text_wrap` is passed separately because `text_size` measures the unwrapped
/// width of items that are otherwise wrapped.
pub(super) fn shape_paragraphs(
    text: Pin<&dyn crate::item_rendering::RenderString>,
    item_rc: Option<&crate::item_tree::ItemRc>,
    text_wrap: TextWrap,
    scale_factor: ScaleFactor,
    font_context: &mut parley::FontContext,
) -> Vec<TextParagraph> {
    let builder = shaping_builder(text, item_rc, text_wrap, scale_factor);
    create_text_paragraphs(&builder, font_context, text.text(), text.link_color())
}

pub(super) struct TextParagraph {
    pub(super) range: Range<usize>,
    pub(super) y: PhysicalLength,
    pub(super) layout: parley::Layout<Brush>,
    pub(super) links: std::vec::Vec<(Range<usize>, std::string::String)>,
    /// Byte ranges within the paragraph's text that carry `Style::Code`. Drawn with a
    /// translucent rounded background by `draw` for visual parity with common markdown
    /// renderers.
    pub(super) code_ranges: std::vec::Vec<Range<usize>>,
    /// The axis lists pushed while shaping: renderers combine
    /// [`ParagraphVariations::for_range`] with the run's synthesis settings to
    /// recover the user-space axes the run was shaped at.
    pub(super) variations: ParagraphVariations,
}
