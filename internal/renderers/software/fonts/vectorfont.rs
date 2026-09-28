// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use core::num::NonZeroU16;

use alloc::rc::Rc;
use alloc::vec::Vec;
use skrifa::MetadataProvider;

use crate::PhysicalLength;
use crate::fixed::Fixed;
#[cfg(feature = "systemfonts")]
use i_slint_common::sharedfontique::fontique;
use i_slint_core::lengths::PhysicalPx;
use i_slint_core::textlayout::{Glyph, TextShaper};

use super::RenderableVectorGlyph;

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

/// Cache key includes blob id, font index, pixel size, glyph id, a hash of normalized
/// variation coordinates (so different variable font instances produce distinct cache
/// entries), the horizontal sub-pixel bin, and the faux-italic synthesis applied at render
/// time. Without `skew_bits`, an upright and a synthetically-italicized glyph from the same
/// font, size, and id would collide on the same cache entry and one of the two runs would
/// silently render with the other's bitmap.
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
}

#[cfg(feature = "systemfonts")]
mod glyph_cache {
    use super::{GlyphCacheKey, RenderableVectorGlyph};

    struct RenderableGlyphWeightScale;

    impl clru::WeightScale<GlyphCacheKey, RenderableVectorGlyph> for RenderableGlyphWeightScale {
        fn weight(&self, _: &GlyphCacheKey, value: &RenderableVectorGlyph) -> usize {
            value.alpha_map.len()
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
    type Key = (u64, u32, u32, u16, u64, u8, u32);

    fn key(k: &GlyphCacheKey) -> Key {
        (
            k.font_blob_id,
            k.font_index,
            (k.pixel_size.get() as f32).to_bits(),
            k.glyph_id.get(),
            k.coords_hash,
            k.subpixel_bin,
            k.skew_bits.unwrap_or(u32::MAX),
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
            let bytes = glyph.alpha_map.len();
            if bytes > self.capacity_bytes {
                return;
            }
            if let Some((_, old)) = self.map.insert(key(&k), (0, glyph)) {
                self.total_bytes -= old.alpha_map.len();
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
                        self.total_bytes -= glyph.alpha_map.len();
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

i_slint_core::thread_local!(static GLYPH_CACHE: core::cell::RefCell<GlyphCache>  =
    core::cell::RefCell::new(GlyphCache::new(1024 * 1024))
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

    pub fn render_vector_glyph(
        &self,
        glyph_id: core::num::NonZeroU16,
        subpixel_bin: u8,
        slint_context: &i_slint_core::SlintContext,
    ) -> Option<RenderableVectorGlyph> {
        GLYPH_CACHE.with(|cache| {
            let mut cache = cache.borrow_mut();

            let skew_degrees = self.skew_degrees();

            let cache_key = GlyphCacheKey {
                font_blob_id: self.font_blob.id(),
                font_index: self.font_index,
                pixel_size: self.pixel_size,
                glyph_id,
                coords_hash: self.coords_hash,
                subpixel_bin,
                skew_bits: skew_degrees.map(f32::to_bits),
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
                let image = swash::scale::Render::new(&[swash::scale::Source::Outline])
                    .format(swash::zeno::Format::Alpha)
                    .offset(swash::zeno::Vector::new(subpixel_offset_x, 0.0))
                    .transform(transform)
                    .render(&mut scaler, glyph_id.get())?;

                let placement = image.placement;
                let alpha_map: Rc<[u8]> = image.data.into();

                Some(RenderableVectorGlyph {
                    x: Fixed::from_integer(placement.left),
                    y: Fixed::from_integer(placement.top - placement.height as i32),
                    width: PhysicalLength::new(placement.width.try_into().unwrap()),
                    height: PhysicalLength::new(placement.height.try_into().unwrap()),
                    alpha_map,
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
        self.render_vector_glyph(glyph_id, 0, slint_context).map(|glyph| super::RenderableGlyph {
            x: glyph.x,
            y: glyph.y,
            width: glyph.width,
            height: glyph.height,
            alpha_map: glyph.alpha_map.into(),
            pixel_stride: glyph.pixel_stride,
            sdf: false,
        })
    }

    fn scale_delta(&self) -> super::Fixed<u16, 8> {
        super::Fixed::from_integer(1)
    }
}
