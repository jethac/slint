// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cspell:ignore COLR CBDT sbix xcbf

use core::num::NonZeroU16;

use alloc::vec::Vec;
use skrifa::MetadataProvider;

use crate::PhysicalLength;
use crate::fixed::Fixed;
#[cfg(feature = "systemfonts")]
use i_slint_common::sharedfontique::fontique;
use i_slint_core::graphics::{Rgba8Pixel, SharedPixelBuffer};
use i_slint_core::lengths::PhysicalPx;
use i_slint_core::textlayout::{Glyph, TextShaper};

use super::{RenderableVectorGlyph, VectorGlyphPixels};

// A length in font design space.
struct FontUnit;
type FontLength = euclid::Length<i32, FontUnit>;
type FontScaleFactor = euclid::Scale<f32, FontUnit, PhysicalPx>;

/// Number of horizontal sub-pixel positions a glyph can be placed at. The
/// shaper produces sub-pixel accurate pen positions, but glyph bitmaps live on
/// the integer pixel grid; rendering each glyph at the nearest 1/N pixel bin
/// (instead of snapping the pen to a whole pixel) keeps inter-glyph spacing
/// even. 4 bins (quarter-pixel) is enough to remove the visible unevenness at
/// UI text sizes while keeping the glyph cache small.
pub(crate) const SUBPIXEL_BIN_COUNT: i32 = 4;

/// Steps per physical pixel that a glyph stroke width is quantized to.
/// Quantizing keeps an animated `stroke-width` from filling the glyph cache with
/// one entry per frame.
const STROKE_WIDTH_STEPS_PER_PIXEL: f32 = 8.;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct StrokeWidth(NonZeroU16);

impl StrokeWidth {
    /// Rounds `physical_width` to the nearest step. Zero, negative, and `NaN` widths become
    /// the smallest step.
    pub fn from_physical(physical_width: f32) -> Self {
        let steps = num_traits::Float::round(physical_width * STROKE_WIDTH_STEPS_PER_PIXEL) as u16;
        Self(NonZeroU16::new(steps).unwrap_or(NonZeroU16::MIN))
    }

    fn to_physical(self) -> f32 {
        self.0.get() as f32 / STROKE_WIDTH_STEPS_PER_PIXEL
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct GlyphCacheKey {
    /// Font blob id.
    font_blob_id: u64,
    /// Font index within the blob.
    font_index: u32,
    /// Rendered pixel size.
    pixel_size: PhysicalLength,
    /// Glyph id.
    glyph_id: core::num::NonZeroU16,
    /// Hash of the normalized variation coordinates.
    coords_hash: u64,
    /// Horizontal sub-pixel bin.
    subpixel_bin: u8,
    /// Faux-italic skew angle in degrees, bit-cast for `Eq`/`Hash`; `None` when the font has
    /// (or doesn't need) a real italic/oblique face.
    skew_bits: Option<u32>,
    /// Outline stroke width; `None` for a filled glyph.
    stroke_width: Option<StrokeWidth>,
    /// Whether a color glyph was asked for, where the font has one, and for a font with color
    /// layers the text's color, which layers drawn in the text's color use (`[0; 3]` otherwise).
    color: Option<[u8; 3]>,
}

#[cfg(feature = "systemfonts")]
mod glyph_cache {
    use super::{GlyphCacheKey, RenderableVectorGlyph};

    struct RenderableGlyphWeightScale;

    impl clru::WeightScale<GlyphCacheKey, RenderableVectorGlyph> for RenderableGlyphWeightScale {
        fn weight(&self, _: &GlyphCacheKey, value: &RenderableVectorGlyph) -> usize {
            value.byte_size()
        }
    }

    pub struct GlyphCache(
        clru::CLruCache<
            GlyphCacheKey,
            RenderableVectorGlyph,
            std::collections::hash_map::RandomState,
            RenderableGlyphWeightScale,
        >,
    );

    impl GlyphCache {
        pub fn new(capacity_bytes: usize) -> Self {
            Self(clru::CLruCache::with_config(
                clru::CLruCacheConfig::new(core::num::NonZeroUsize::new(capacity_bytes).unwrap())
                    .with_scale(RenderableGlyphWeightScale),
            ))
        }

        pub fn get(&mut self, key: &GlyphCacheKey) -> Option<RenderableVectorGlyph> {
            self.0.get(key).cloned()
        }

        pub fn insert(&mut self, key: GlyphCacheKey, value: RenderableVectorGlyph) {
            self.0.put_with_weight(key, value).ok();
        }
    }
}

/// Glyph cache for `embedded-vector-fonts` builds without `std` (no `clru`, no
/// `RandomState`): a `BTreeMap` keyed by glyph identity plus a monotonically
/// increasing access stamp, bounded by total alpha-map bytes. On overflow the
/// oldest entries are evicted by stamp order; eviction is O(n) but happens only
/// when the cache is full, so the common lookup stays O(log n).
#[cfg(all(feature = "embedded-vector-fonts", not(feature = "systemfonts")))]
mod glyph_cache {
    use super::{GlyphCacheKey, RenderableVectorGlyph};
    use alloc::collections::BTreeMap;

    /// Ordering needs `pixel_size` as raw bits. `PhysicalLength` is `PartialOrd`
    /// only, so order on its bit pattern.
    type Key = (u64, u32, u32, u16, u64, u8, u32, u16, Option<[u8; 3]>);

    fn key(k: &GlyphCacheKey) -> Key {
        (
            k.font_blob_id,
            k.font_index,
            (k.pixel_size.get() as f32).to_bits(),
            k.glyph_id.get(),
            k.coords_hash,
            k.subpixel_bin,
            k.skew_bits.unwrap_or(u32::MAX),
            k.stroke_width.map_or(0, |width| width.0.get()),
            k.color,
        )
    }

    pub struct GlyphCache {
        map: BTreeMap<Key, (u64, RenderableVectorGlyph)>,
        stamp: u64,
        total_bytes: usize,
        capacity_bytes: usize,
    }

    impl GlyphCache {
        pub fn new(capacity_bytes: usize) -> Self {
            Self { map: BTreeMap::new(), stamp: 0, total_bytes: 0, capacity_bytes }
        }

        pub fn get(&mut self, k: &GlyphCacheKey) -> Option<RenderableVectorGlyph> {
            self.stamp += 1;
            self.map.get_mut(&key(k)).map(|(stamp, glyph)| {
                *stamp = self.stamp;
                glyph.clone()
            })
        }

        pub fn insert(&mut self, k: GlyphCacheKey, glyph: RenderableVectorGlyph) {
            let bytes = glyph.byte_size();
            if bytes > self.capacity_bytes {
                return;
            }
            if let Some((_, old)) = self.map.insert(key(&k), (0, glyph)) {
                self.total_bytes -= old.byte_size();
            }
            self.stamp += 1;
            if let Some((stamp, _)) = self.map.get_mut(&key(&k)) {
                *stamp = self.stamp;
            }
            self.total_bytes += bytes;
            while self.total_bytes > self.capacity_bytes {
                let evict_count = self.map.len().saturating_add(3) / 4;
                let mut by_stamp: alloc::vec::Vec<(u64, Key)> =
                    self.map.iter().map(|(k, (s, _))| (*s, *k)).collect();
                by_stamp.sort_unstable_by_key(|(s, _)| *s);
                for (_, k) in by_stamp.into_iter().take(evict_count.max(1)) {
                    if let Some((_, glyph)) = self.map.remove(&k) {
                        self.total_bytes -= glyph.byte_size();
                    }
                }
                if evict_count == 0 {
                    break;
                }
            }
        }
    }
}

use glyph_cache::GlyphCache;

/// Alpha-map cache capacity in bytes. The default 1 MiB covers full-screen
/// text on typical displays; MCUs with little RAM can shrink it by setting
/// `SLINT_VECTOR_FONT_CACHE_BYTES` in the environment or in
/// `.cargo/config.toml`'s `[env]` table, which Cargo exports to dependency
/// compilations as well.
const GLYPH_CACHE_CAPACITY_BYTES: usize = {
    const fn parse_usize(s: &str) -> usize {
        let bytes = s.as_bytes();
        let mut value = 0usize;
        let mut i = 0;
        while i < bytes.len() {
            assert!(bytes[i].is_ascii_digit(), "SLINT_VECTOR_FONT_CACHE_BYTES must be a number");
            value = value * 10 + (bytes[i] - b'0') as usize;
            i += 1;
        }
        value
    }
    match option_env!("SLINT_VECTOR_FONT_CACHE_BYTES") {
        Some(s) => parse_usize(s),
        None => 1024 * 1024,
    }
};

i_slint_core::thread_local!(static GLYPH_CACHE: core::cell::RefCell<GlyphCache>  =
    core::cell::RefCell::new(GlyphCache::new(GLYPH_CACHE_CAPACITY_BYTES))
);

/// The font bytes a `VectorFont` rasterizes from: a fontique-managed blob when
/// fonts come from `fontique`'s collection (`systemfonts`), or the `&'static`
/// data the compiler embedded (`embedded-vector-fonts`, no `std` required).
pub enum FontData {
    #[cfg(feature = "systemfonts")]
    /// A blob shared with the fontique collection.
    Shared(fontique::Blob<u8>),
    /// Compiler-embedded `&'static` font data.
    Embedded(&'static [u8]),
}

impl FontData {
    fn data(&self) -> &[u8] {
        match self {
            #[cfg(feature = "systemfonts")]
            Self::Shared(blob) => blob.data(),
            Self::Embedded(data) => data,
        }
    }

    /// Stable id used in the glyph cache key. fontique blobs carry an
    /// incrementing id; embedded data is `&'static`, so its address is unique
    /// for the program's lifetime.
    fn id(&self) -> u64 {
        match self {
            #[cfg(feature = "systemfonts")]
            Self::Shared(blob) => blob.id(),
            Self::Embedded(data) => data.as_ptr() as usize as u64,
        }
    }
}

#[cfg(feature = "systemfonts")]
impl From<fontique::Blob<u8>> for FontData {
    fn from(blob: fontique::Blob<u8>) -> Self {
        Self::Shared(blob)
    }
}

impl From<&'static [u8]> for FontData {
    fn from(data: &'static [u8]) -> Self {
        Self::Embedded(data)
    }
}

pub struct VectorFont {
    font_index: u32,
    font_blob: FontData,
    swash_key: swash::CacheKey,
    swash_offset: u32,
    ascender: PhysicalLength,
    descender: PhysicalLength,
    height: PhysicalLength,
    pixel_size: PhysicalLength,
    x_height: PhysicalLength,
    cap_height: PhysicalLength,
    /// Normalized variation coordinates (F2Dot14, fvar axis order) for variable font rendering.
    normalized_coords: Vec<i16>,
    /// Hash of normalized_coords for use in the glyph cache key.
    coords_hash: u64,
    /// Faux-italic/faux-bold hints from fontique, applied at render time via
    /// [`with_synthesis`](Self::with_synthesis). Left at the default (no-op) for instances used
    /// only for shaping and metrics, where synthesis is irrelevant.
    #[cfg(feature = "systemfonts")]
    synthesis: fontique::Synthesis,
}

fn hash_coords(coords: &[i16]) -> u64 {
    // FNV-1a; no_std-compatible (std's `DefaultHasher` isn't available without std).
    let mut hash = 0xcbf29ce484222325u64;
    for coord in coords {
        for byte in coord.to_le_bytes() {
            hash ^= byte as u64;
            hash = hash.wrapping_mul(0x100000001b3);
        }
    }
    hash
}

impl VectorFont {
    fn swash_font_ref(&self) -> swash::FontRef<'_> {
        swash::FontRef {
            data: self.font_blob.data(),
            offset: self.swash_offset,
            key: self.swash_key,
        }
    }

    #[cfg(feature = "systemfonts")]
    pub fn new(
        font: fontique::QueryFont,
        swash_key: swash::CacheKey,
        swash_offset: u32,
        pixel_size: PhysicalLength,
    ) -> Self {
        Self::new_from_blob_and_index(font.blob, font.index, swash_key, swash_offset, pixel_size)
    }

    #[cfg(feature = "systemfonts")]
    pub fn new_from_blob_and_index(
        font_blob: fontique::Blob<u8>,
        font_index: u32,
        swash_key: swash::CacheKey,
        swash_offset: u32,
        pixel_size: PhysicalLength,
    ) -> Self {
        Self::new_from_blob_and_index_with_coords(
            font_blob.into(),
            font_index,
            swash_key,
            swash_offset,
            pixel_size,
            &[],
        )
    }

    pub fn new_from_blob_and_index_with_coords(
        font_blob: FontData,
        font_index: u32,
        swash_key: swash::CacheKey,
        swash_offset: u32,
        pixel_size: PhysicalLength,
        normalized_coords: &[i16],
    ) -> Self {
        let face = skrifa::FontRef::from_index(font_blob.data(), font_index).unwrap();

        let skrifa_coords: Vec<skrifa::instance::NormalizedCoord> = normalized_coords
            .iter()
            .map(|&c| skrifa::instance::NormalizedCoord::from_bits(c))
            .collect();
        let location = skrifa::instance::LocationRef::new(&skrifa_coords);

        let metrics = face.metrics(skrifa::instance::Size::unscaled(), location);

        let ascender = FontLength::new(metrics.ascent as _);
        let descender = FontLength::new(metrics.descent as _);
        let height = FontLength::new((metrics.ascent - metrics.descent) as _);
        let x_height = FontLength::new(metrics.x_height.unwrap_or_default() as _);
        let cap_height = FontLength::new(metrics.cap_height.unwrap_or_default() as _);
        let units_per_em = metrics.units_per_em;
        let scale = FontScaleFactor::new(pixel_size.get() as f32 / units_per_em as f32);
        let coords_hash = hash_coords(normalized_coords);
        Self {
            font_index,
            font_blob,
            swash_key,
            swash_offset,
            ascender: (ascender.cast() * scale).cast(),
            descender: (descender.cast() * scale).cast(),
            height: (height.cast() * scale).cast(),
            pixel_size,
            x_height: (x_height.cast() * scale).cast(),
            cap_height: (cap_height.cast() * scale).cast(),
            normalized_coords: normalized_coords.to_vec(),
            coords_hash,
            #[cfg(feature = "systemfonts")]
            synthesis: fontique::Synthesis::default(),
        }
    }

    /// Attaches fontique's synthesis suggestions (currently only faux-italic skew is applied,
    /// see [`render_vector_glyph`](Self::render_vector_glyph)) to use when rasterizing glyphs.
    /// Only meaningful for a font instance used to render (as opposed to shape) text, since
    /// synthesis changes the glyph outline, not its advance width.
    #[cfg(feature = "systemfonts")]
    pub fn with_synthesis(mut self, synthesis: fontique::Synthesis) -> Self {
        self.synthesis = synthesis;
        self
    }

    #[cfg(feature = "systemfonts")]
    fn skew_degrees(&self) -> Option<f32> {
        self.synthesis.skew()
    }

    #[cfg(all(feature = "embedded-vector-fonts", not(feature = "systemfonts")))]
    fn skew_degrees(&self) -> Option<f32> {
        None
    }

    /// Renders a glyph. With `color` (the text's red, green and blue), a glyph the font draws in
    /// color (an emoji's layers or bitmap) comes back as [`VectorGlyphPixels::Color`], its layers
    /// that use the text's color drawn in `color`; otherwise, and for a stroke, it's the
    /// outline's coverage.
    pub fn render_vector_glyph(
        &self,
        glyph_id: core::num::NonZeroU16,
        subpixel_bin: u8,
        stroke_width: Option<StrokeWidth>,
        color: Option<[u8; 3]>,
        slint_context: &i_slint_core::SlintContext,
    ) -> Option<RenderableVectorGlyph> {
        GLYPH_CACHE.with(|cache| {
            let mut cache = cache.borrow_mut();

            let skew_degrees = self.skew_degrees();
            // A stroke is drawn from the outline, so color makes no difference to it. Only color
            // layers (a `COLR` table) can use the text's color: other glyphs keep one cache entry
            // whatever the text's color is.
            let color = color.filter(|_| stroke_width.is_none()).map(|rgb| {
                let has_layers =
                    self.swash_font_ref().table(swash::tag_from_bytes(b"COLR")).is_some();
                if has_layers { rgb } else { [0; 3] }
            });

            let cache_key = GlyphCacheKey {
                font_blob_id: self.font_blob.id(),
                font_index: self.font_index,
                pixel_size: self.pixel_size,
                glyph_id,
                coords_hash: self.coords_hash,
                subpixel_bin,
                skew_bits: skew_degrees.map(f32::to_bits),
                stroke_width,
                color,
            };

            if let Some(entry) = cache.get(&cache_key) {
                return Some(entry.clone());
            }

            let subpixel_offset_x = subpixel_bin as f32 / SUBPIXEL_BIN_COUNT as f32;

            let glyph = {
                let font_ref = self.swash_font_ref();
                let mut ctx = slint_context.swash_scale_context().borrow_mut();
                let mut scaler = ctx
                    .builder(font_ref)
                    .size(self.pixel_size.get() as f32)
                    .normalized_coords(&self.normalized_coords)
                    .build();
                // Faux italic, for fonts fontique picked as the closest match to an `italic`
                // request but that carry neither a true italic face nor an `ital`/`slnt`
                // variation axis (common for CJK fonts, see issue #10178). This transform runs
                // in the outline's own font-design space (Y-up: ascenders have larger Y), not
                // device pixels, so the sign that leans glyphs forward here is the opposite of
                // the device-space renderers -- verified by rendering both ways and comparing
                // which one actually leans right, not derived from a convention doc alone.
                let transform = skew_degrees.map(|degrees| {
                    swash::zeno::Transform::skew(
                        swash::zeno::Angle::from_degrees(degrees),
                        swash::zeno::Angle::ZERO,
                    )
                });
                // Color first: the layers of a `COLR` font (version 0), then the bitmaps of a
                // `CBDT` or `sbix` font, scaled to the pixel size. A font without either, and a
                // stroke, gives the outline.
                use swash::scale::{Source, StrikeWith};
                let sources: &[Source] = if color.is_some() {
                    &[
                        Source::ColorOutline(0),
                        Source::ColorBitmap(StrikeWith::BestFit),
                        Source::Outline,
                    ]
                } else {
                    &[Source::Outline]
                };
                let mut render = swash::scale::Render::new(sources);
                render
                    .format(swash::zeno::Format::Alpha)
                    .offset(swash::zeno::Vector::new(subpixel_offset_x, 0.0))
                    .transform(transform);
                if let Some([r, g, b]) = color {
                    // Opaque: the text's alpha applies when the glyph is drawn.
                    render.default_color([r, g, b, 255]);
                }
                if let Some(stroke_width) = stroke_width {
                    let mut stroke = swash::zeno::Stroke::new(stroke_width.to_physical());
                    // Matches the Skia and FemtoVG text stroke.
                    stroke
                        .join(swash::zeno::Join::Miter)
                        .miter_limit(10.)
                        .cap(swash::zeno::Cap::Butt);
                    render.style(stroke);
                }
                let image = render.render(&mut scaler, glyph_id.get())?;

                let placement = image.placement;
                let pixels = match image.content {
                    swash::scale::image::Content::Color => {
                        let mut buffer = SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(
                            &image.data,
                            placement.width,
                            placement.height,
                        );
                        // Layers come composited over transparent, so premultiplied; bitmaps
                        // are decoded from PNG with straight alpha.
                        if matches!(image.source, Source::ColorBitmap(_)) {
                            for pixel in buffer.make_mut_slice() {
                                let alpha = pixel.a as u16;
                                pixel.r = ((pixel.r as u16 * alpha + 127) / 255) as u8;
                                pixel.g = ((pixel.g as u16 * alpha + 127) / 255) as u8;
                                pixel.b = ((pixel.b as u16 * alpha + 127) / 255) as u8;
                            }
                        }
                        VectorGlyphPixels::Color(buffer)
                    }
                    _ => VectorGlyphPixels::AlphaMap(image.data.into()),
                };

                Some(RenderableVectorGlyph {
                    x: Fixed::from_integer(placement.left),
                    y: Fixed::from_integer(placement.top - placement.height as i32),
                    width: PhysicalLength::new(placement.width.try_into().unwrap()),
                    height: PhysicalLength::new(placement.height.try_into().unwrap()),
                    pixels,
                    pixel_stride: placement.width.try_into().unwrap(),
                    glyph_origin_x: placement.left as f32,
                })
            };

            if let Some(glyph) = glyph {
                cache.insert(cache_key, glyph.clone());
                Some(glyph)
            } else {
                None
            }
        })
    }
}

impl TextShaper for VectorFont {
    type LengthPrimitive = i16;
    type Length = PhysicalLength;
    fn shape_text<GlyphStorage: core::iter::Extend<Glyph<PhysicalLength>>>(
        &self,
        text: &str,
        glyphs: &mut GlyphStorage,
    ) {
        let font_ref = self.swash_font_ref();
        let charmap = font_ref.charmap();
        let gm = font_ref.glyph_metrics(&self.normalized_coords);
        let metrics = font_ref.metrics(&[]);
        let scale = self.pixel_size.get() as f32 / metrics.units_per_em as f32;

        glyphs.extend(text.char_indices().map(|(byte_offset, char)| {
            let glyph_id = NonZeroU16::try_from(charmap.map(char)).ok();
            let x_advance = glyph_id.map_or_else(
                || self.pixel_size.get(),
                |id| (gm.advance_width(id.get()) * scale) as _,
            );

            Glyph {
                glyph_id,
                advance: PhysicalLength::new(x_advance),
                text_byte_offset: byte_offset,
                ..Default::default()
            }
        }));
    }

    fn glyph_for_char(&self, ch: char) -> Option<Glyph<PhysicalLength>> {
        let font_ref = self.swash_font_ref();
        let charmap = font_ref.charmap();
        let gm = font_ref.glyph_metrics(&self.normalized_coords);
        let metrics = font_ref.metrics(&[]);
        let scale = self.pixel_size.get() as f32 / metrics.units_per_em as f32;

        NonZeroU16::try_from(charmap.map(ch)).ok().map(|glyph_id| Glyph {
            glyph_id: Some(glyph_id),
            advance: PhysicalLength::new((gm.advance_width(glyph_id.get()) * scale) as _),
            ..Default::default()
        })
    }
}

impl i_slint_core::textlayout::FontMetrics<PhysicalLength> for VectorFont {
    fn ascent(&self) -> PhysicalLength {
        self.ascender
    }

    fn height(&self) -> PhysicalLength {
        self.height
    }

    fn descent(&self) -> PhysicalLength {
        self.descender
    }

    fn x_height(&self) -> PhysicalLength {
        self.x_height
    }

    fn cap_height(&self) -> PhysicalLength {
        self.cap_height
    }
}

impl super::GlyphRenderer for VectorFont {
    fn render_glyph(
        &self,
        glyph_id: core::num::NonZeroU16,
        slint_context: &i_slint_core::SlintContext,
    ) -> Option<super::RenderableGlyph> {
        // A coverage map, like the bitmap fonts' glyphs this path also draws.
        self.render_vector_glyph(glyph_id, 0, None, None, slint_context).and_then(|glyph| {
            let VectorGlyphPixels::AlphaMap(alpha_map) = glyph.pixels else { return None };
            Some(super::RenderableGlyph {
                x: glyph.x,
                y: glyph.y,
                width: glyph.width,
                height: glyph.height,
                alpha_map: alpha_map.into(),
                pixel_stride: glyph.pixel_stride,
                sdf: false,
            })
        })
    }

    fn scale_delta(&self) -> super::Fixed<u16, 8> {
        super::Fixed::from_integer(1)
    }
}
