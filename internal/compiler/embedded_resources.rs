// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#[cfg(feature = "renderer-software")]
pub use resvg::tiny_skia::IntRect as Rect;

#[derive(Debug, Clone, Copy, Default)]
pub struct Size {
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Copy, Debug, strum::Display)]
pub enum PixelFormat {
    // 24 bit RGB
    Rgb,
    // 32 bit RGBA
    Rgba,
    // 32 bit RGBA, but the RGB values are pre-multiplied by the alpha
    RgbaPremultiplied,
    // 8bit alpha map with a given color
    AlphaMap([u8; 3]),
}

#[cfg(feature = "renderer-software")]
#[derive(Debug, Clone)]
pub struct Texture {
    pub total_size: Size,
    pub original_size: Size,
    pub rect: Rect,
    pub data: Vec<u8>,
    pub format: PixelFormat,
}

#[cfg(feature = "renderer-software")]
impl Texture {
    pub fn new_empty() -> Self {
        Self {
            total_size: Size::default(),
            original_size: Size::default(),
            rect: Rect::from_xywh(0, 0, 1, 1).unwrap(),
            data: vec![0, 0, 0, 0],
            format: PixelFormat::Rgba,
        }
    }
}

#[cfg(feature = "renderer-software")]
#[derive(Debug, Clone, Default)]
pub struct BitmapGlyph {
    pub x: i16,
    pub y: i16,
    pub width: i16,
    pub height: i16,
    pub x_advance: i16,
    /// 8bit alpha map or SDF if `BitMapGlyphs`'s `sdf` is `true`.
    pub data: Vec<u8>,
}

#[cfg(feature = "renderer-software")]
#[derive(Debug, Clone)]
pub struct BitmapGlyphs {
    pub pixel_size: i16,
    pub glyph_data: Vec<BitmapGlyph>,
}

#[cfg(feature = "renderer-software")]
#[derive(Debug, Clone)]
pub struct CharacterMapEntry {
    pub code_point: char,
    pub glyph_index: u16,
}

#[cfg(feature = "renderer-software")]
#[derive(Debug, Clone)]
/// One resolved axis setting of a pre-rendered [`BitmapFont`], in user-space units.
/// `default_value` is the axis' fvar default and lets the runtime score a bitmap font
/// against requests that don't mention the axis.
pub struct BitmapFontVariation {
    /// The axis tag as a big-endian `u32` (`u32::from_be_bytes(*b"wght")`).
    pub tag: u32,
    /// The user-space value the glyphs were rasterized at.
    pub value: f32,
    /// The axis' default value in the source font.
    pub default_value: f32,
}

#[cfg(feature = "renderer-software")]
#[derive(Debug, Clone)]
pub struct BitmapFont {
    pub family_name: String,
    /// map of available glyphs, sorted by char
    pub character_map: Vec<CharacterMapEntry>,
    pub units_per_em: f32,
    pub ascent: f32,
    pub descent: f32,
    pub x_height: f32,
    pub cap_height: f32,
    pub glyphs: Vec<BitmapGlyphs>,
    pub weight: u16,
    pub italic: bool,
    /// true when the font is represented as a signed distance field
    pub sdf: bool,
    /// The resolved axis tuple the glyphs were rasterized at: every fvar axis of the
    /// source font except `wght` (carried by `weight`) and `opsz` when `auto_opsz` is
    /// set. Empty for static fonts.
    pub variations: Vec<BitmapFontVariation>,
    /// true when the source font has an `opsz` axis and each glyph set was rasterized
    /// with `opsz` equal to its used (logical) size. The `opsz` axis is then absent
    /// from `variations` because its value differs per glyph set.
    pub auto_opsz: bool,
}

#[derive(Debug, Clone)]
pub enum EmbeddedResourcesKind {
    /// Only List the resource, do not actually embed it
    ListOnly,
    /// Just put the file content as a resource
    FileData,
    /// Encoded payload from a data URI (bytes, extension)
    DataUriPayload(Vec<u8>, String),
    /// The data has been processed in a texture
    #[cfg(feature = "renderer-software")]
    TextureData(Texture),
    /// A set of pre-rendered glyphs of a TrueType font
    #[cfg(feature = "renderer-software")]
    BitmapFontData(BitmapFont),
    /// The image of a Slint SC `@image-url()`, decoded at compile time. The
    /// Slint SC generator embeds its pixels in the generated code.
    #[cfg(feature = "slint-sc")]
    StaticPixels(image::RgbaImage),
}

#[derive(Debug, Clone)]
pub struct EmbeddedResources {
    /// Path on disk of the resource, or `None` for in-memory payloads such as data URIs.
    pub path: Option<smol_str::SmolStr>,

    pub kind: EmbeddedResourcesKind,
}

/// Index of an [`EmbeddedResources`] entry in [`crate::object_tree::Document::embedded_file_resources`].
#[derive(
    Debug,
    Clone,
    Copy,
    Hash,
    PartialEq,
    Eq,
    derive_more::Into,
    derive_more::From,
    derive_more::Display,
)]
pub struct EmbeddedResourcesIdx(pub usize);
