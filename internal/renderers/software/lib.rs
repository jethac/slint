// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore frontmost premult rasterizers unrotated untransform
#![doc = include_str!("README.md")]
#![doc(html_logo_url = "https://slint.dev/logo/slint-logo-square-light.svg")]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![deny(unsafe_code)]
#![cfg_attr(slint_nightly_test, feature(non_exhaustive_omitted_patterns_lint))]
#![cfg_attr(slint_nightly_test, warn(non_exhaustive_omitted_patterns))]
#![no_std]
#![warn(missing_docs)]

extern crate alloc;
#[cfg(feature = "std")]
extern crate std;

mod draw_functions;
mod fixed;
mod fonts;
mod minimal_software_window;
mod scene;
#[doc(hidden)]
pub mod shape_raster;

use self::fonts::GlyphRenderer;
pub use self::minimal_software_window::MinimalSoftwareWindow;
use self::scene::*;
use alloc::rc::{Rc, Weak};
use alloc::vec::Vec;
use core::cell::{Cell, RefCell};
use core::pin::Pin;
use euclid::Length;
use fixed::Fixed;
#[cfg(feature = "std")]
use i_slint_core::api::PlatformError;
#[cfg(feature = "std")]
use i_slint_core::graphics::Rgba8Pixel;
use i_slint_core::graphics::rendering_metrics_collector::{RefreshMode, RenderingMetricsCollector};
use i_slint_core::graphics::{BorderRadius, SharedImageBuffer, SharedPixelBuffer, shadow};
use i_slint_core::item_rendering::HasFont;
use i_slint_core::item_rendering::{
    CachedRenderingData, ItemRenderer, ItemRendererFeatures, PlainOrStyledText,
    RenderBorderRectangle, RenderImage, RenderRectangle,
};
use i_slint_core::item_tree::ItemTreeWeak;
use i_slint_core::items::{FillRule, ItemRc, TextOverflow, TextWrap};
use i_slint_core::lengths::{
    LogicalLength, LogicalPoint, LogicalPx, LogicalRect, LogicalSize, LogicalVector, PhysicalPx,
    PointLengths, RectLengths, ScaleFactor, SizeLengths,
};
use i_slint_core::partial_renderer::{DirtyRegion, PartialRenderer, PartialRenderingState};
use i_slint_core::renderer::RendererSealed;
use i_slint_core::textlayout::{AbstractFont, FontMetrics, TextParagraphLayout};
use i_slint_core::window::{WindowAdapter, WindowInner};
use i_slint_core::{Brush, Color, Coord, ImageInner, StaticTextures};
#[allow(unused)]
use num_traits::Float;
use num_traits::NumCast;

pub use draw_functions::{PremultipliedRgbaColor, Rgb565BigEndianPixel, Rgb565Pixel, TargetPixel};

type PhysicalLength = euclid::Length<i16, PhysicalPx>;
type PhysicalRect = euclid::Rect<i16, PhysicalPx>;
type PhysicalSize = euclid::Size2D<i16, PhysicalPx>;
type PhysicalPoint = euclid::Point2D<i16, PhysicalPx>;
type PhysicalBorderRadius = BorderRadius<i16, PhysicalPx>;

pub use i_slint_core::partial_renderer::RepaintBufferType;

use fonts::with_font;

/// This enum describes the rotation that should be applied to the contents rendered by the software renderer.
///
/// Argument to be passed in [`SoftwareRenderer::set_rendering_rotation`].
#[non_exhaustive]
#[derive(Default, Copy, Clone, Eq, PartialEq, Debug)]
pub enum RenderingRotation {
    /// No rotation
    #[default]
    NoRotation,
    /// Rotate 90° to the right
    Rotate90,
    /// 180° rotation (upside-down)
    Rotate180,
    /// Rotate 90° to the left
    Rotate270,
}

impl RenderingRotation {
    fn is_transpose(self) -> bool {
        matches!(self, Self::Rotate90 | Self::Rotate270)
    }
    fn mirror_width(self) -> bool {
        matches!(self, Self::Rotate270 | Self::Rotate180)
    }
    fn mirror_height(self) -> bool {
        matches!(self, Self::Rotate90 | Self::Rotate180)
    }
    /// Angle of the rotation in degrees
    pub fn angle(self) -> f32 {
        match self {
            RenderingRotation::NoRotation => 0.,
            RenderingRotation::Rotate90 => 90.,
            RenderingRotation::Rotate180 => 180.,
            RenderingRotation::Rotate270 => 270.,
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub(crate) struct RotationInfo {
    pub(crate) orientation: RenderingRotation,
    pub(crate) screen_size: PhysicalSize,
}

/// Extension trait for euclid type to transpose coordinates (swap x and y, as well as width and height)
trait Transform {
    /// Return a copy of Self whose coordinate are swapped (x swapped with y)
    #[must_use]
    fn transformed(self, info: RotationInfo) -> Self;
}

impl<T: Copy + NumCast + core::ops::Sub<Output = T>> Transform for euclid::Point2D<T, PhysicalPx> {
    fn transformed(mut self, info: RotationInfo) -> Self {
        if info.orientation.mirror_width() {
            self.x = T::from(info.screen_size.width).unwrap() - self.x - T::from(1).unwrap()
        }
        if info.orientation.mirror_height() {
            self.y = T::from(info.screen_size.height).unwrap() - self.y - T::from(1).unwrap()
        }
        if info.orientation.is_transpose() {
            core::mem::swap(&mut self.x, &mut self.y);
        }
        self
    }
}

impl<T: Copy> Transform for euclid::Size2D<T, PhysicalPx> {
    fn transformed(mut self, info: RotationInfo) -> Self {
        if info.orientation.is_transpose() {
            core::mem::swap(&mut self.width, &mut self.height);
        }
        self
    }
}

/// Applies the rendering rotation to a point in continuous (sub-pixel)
/// coordinates: cells mirror about `W − x` / `H − y`, unlike the integer
/// pixel transform which subtracts the extra 1 of pixel indexing.
fn transform_continuous(
    mut p: euclid::Point2D<f32, PhysicalPx>,
    info: RotationInfo,
) -> euclid::Point2D<f32, PhysicalPx> {
    if info.orientation.mirror_width() {
        p.x = info.screen_size.width as f32 - p.x;
    }
    if info.orientation.mirror_height() {
        p.y = info.screen_size.height as f32 - p.y;
    }
    if info.orientation.is_transpose() {
        core::mem::swap(&mut p.x, &mut p.y);
    }
    p
}

/// The screen rotation as an affine in post-rotation physical space:
/// mirror-width, mirror-height, then transpose, matching
/// [`transform_continuous`].
fn screen_space_affine(rotation: RotationInfo) -> shadow::Affine {
    let w = rotation.screen_size.width as f32;
    let h = rotation.screen_size.height as f32;
    let mirror = shadow::Affine::new(
        if rotation.orientation.mirror_width() { -1. } else { 1. },
        0.,
        0.,
        if rotation.orientation.mirror_height() { -1. } else { 1. },
        if rotation.orientation.mirror_width() { w } else { 0. },
        if rotation.orientation.mirror_height() { h } else { 0. },
    );
    if rotation.orientation.is_transpose() {
        shadow::Affine::new(0., 1., 1., 0., 0., 0.).pre_concat(&mirror)
    } else {
        mirror
    }
}

/// The inverse of [`transform_continuous`]: maps a rendered point back to
/// the frame it was drawn in before the renderer's rotation.
fn untransform_continuous(
    mut p: euclid::Point2D<f32, PhysicalPx>,
    info: RotationInfo,
) -> euclid::Point2D<f32, PhysicalPx> {
    if info.orientation.is_transpose() {
        core::mem::swap(&mut p.x, &mut p.y);
    }
    if info.orientation.mirror_height() {
        p.y = info.screen_size.height as f32 - p.y;
    }
    if info.orientation.mirror_width() {
        p.x = info.screen_size.width as f32 - p.x;
    }
    p
}

impl<T: Copy + NumCast + core::ops::Sub<Output = T>> Transform for euclid::Rect<T, PhysicalPx> {
    fn transformed(self, info: RotationInfo) -> Self {
        let one = T::from(1).unwrap();
        let mut origin = self.origin.transformed(info);
        let size = self.size.transformed(info);
        if info.orientation.mirror_width() {
            origin.y = origin.y - (size.height - one);
        }
        if info.orientation.mirror_height() {
            origin.x = origin.x - (size.width - one);
        }
        Self::new(origin, size)
    }
}

impl<T: Copy> Transform for BorderRadius<T, PhysicalPx> {
    fn transformed(self, info: RotationInfo) -> Self {
        match info.orientation {
            RenderingRotation::NoRotation => self,
            RenderingRotation::Rotate90 => {
                Self::new(self.bottom_left, self.top_left, self.top_right, self.bottom_right)
            }
            RenderingRotation::Rotate180 => {
                Self::new(self.bottom_right, self.bottom_left, self.top_left, self.top_right)
            }
            RenderingRotation::Rotate270 => {
                Self::new(self.top_right, self.bottom_right, self.bottom_left, self.top_left)
            }
        }
    }
}

/// This trait defines a bi-directional interface between Slint and your code to send lines to your screen, when using
/// the [`SoftwareRenderer::render_by_line`] function.
///
/// * Through the associated `TargetPixel` type Slint knows how to create and manipulate pixels without having to know
///   the exact device-specific binary representation and operations for blending.
/// * Through the `process_line` function Slint notifies you when a line can be rendered and provides a callback that
///   you can invoke to fill a slice of pixels for the given line.
///
/// See the [`render_by_line`](SoftwareRenderer::render_by_line) documentation for an example.
pub trait LineBufferProvider {
    /// The pixel type of the buffer
    type TargetPixel: TargetPixel;

    /// Called once per line, you will have to call the render_fn back with the buffer.
    ///
    /// The `line` is the y position of the line to be drawn.
    /// The `range` is the range within the line that is going to be rendered (eg, within the dirty region).
    /// Its start and length are multiples of the horizontal
    /// [`DirtyRegionAlignment`](SoftwareRenderer::set_dirty_region_alignment).
    /// The runs of lines it is called for begin and end on the vertical one.
    /// The `render_fn` function should be called to render the line, passing the buffer
    /// corresponding to the specified line and range.
    fn process_line(
        &mut self,
        line: usize,
        range: core::ops::Range<usize>,
        render_fn: impl FnOnce(&mut [Self::TargetPixel]),
    );
}

#[cfg(not(cbindgen))]
const PHYSICAL_REGION_MAX_SIZE: usize = DirtyRegion::MAX_COUNT;
// cbindgen can't understand associated const correctly, so hardcode the value
#[cfg(cbindgen)]
pub const PHYSICAL_REGION_MAX_SIZE: usize = 3;
const _: () = {
    assert!(PHYSICAL_REGION_MAX_SIZE == 3);
    assert!(DirtyRegion::MAX_COUNT == 3);
};

/// Represents a rectangular region on the screen, used for partial rendering.
///
/// The region may be composed of multiple sub-regions.
#[derive(Clone, Debug, Default)]
#[repr(C)]
pub struct PhysicalRegion {
    rectangles: [euclid::Box2D<i16, PhysicalPx>; PHYSICAL_REGION_MAX_SIZE],
    count: usize,
}

impl PhysicalRegion {
    fn iter_box(&self) -> impl Iterator<Item = euclid::Box2D<i16, PhysicalPx>> + '_ {
        (0..self.count).map(|x| self.rectangles[x])
    }

    fn bounding_rect(&self) -> PhysicalRect {
        if self.count == 0 {
            return Default::default();
        }
        let mut r = self.rectangles[0];
        for i in 1..self.count {
            r = r.union(&self.rectangles[i]);
        }
        r.to_rect()
    }

    /// Returns the size of the bounding box of this region.
    pub fn bounding_box_size(&self) -> i_slint_core::api::PhysicalSize {
        let bb = self.bounding_rect();
        i_slint_core::api::PhysicalSize { width: bb.width() as _, height: bb.height() as _ }
    }
    /// Returns the origin of the bounding box of this region.
    pub fn bounding_box_origin(&self) -> i_slint_core::api::PhysicalPosition {
        let bb = self.bounding_rect();
        i_slint_core::api::PhysicalPosition { x: bb.origin.x as _, y: bb.origin.y as _ }
    }

    /// Returns an iterator over the rectangles in this region.
    /// Each rectangle is represented by its position and its size.
    /// They do not overlap.
    pub fn iter(
        &self,
    ) -> impl Iterator<Item = (i_slint_core::api::PhysicalPosition, i_slint_core::api::PhysicalSize)> + '_
    {
        let mut line_ranges = Vec::<core::ops::Range<i16>>::new();
        let mut begin_line = 0;
        let mut end_line = 0;
        core::iter::from_fn(move || {
            loop {
                match line_ranges.pop() {
                    Some(r) => {
                        return Some((
                            i_slint_core::api::PhysicalPosition {
                                x: r.start as _,
                                y: begin_line as _,
                            },
                            i_slint_core::api::PhysicalSize {
                                width: r.len() as _,
                                height: (end_line - begin_line) as _,
                            },
                        ));
                    }
                    None => {
                        begin_line = end_line;
                        end_line = match region_line_ranges(self, begin_line, &mut line_ranges) {
                            Some(end_line) => end_line,
                            None => return None,
                        };
                        line_ranges.reverse();
                    }
                }
            }
        })
    }

    fn intersection(&self, clip: &PhysicalRect) -> PhysicalRegion {
        let mut res = Self::default();
        let clip = clip.to_box2d();
        let mut count = 0;
        for i in 0..self.count {
            if let Some(r) = self.rectangles[i].intersection(&clip) {
                res.rectangles[count] = r;
                count += 1;
            }
        }
        res.count = count;
        res
    }
}

/// Aligns software-renderer dirty regions to a physical pixel grid.
///
/// Some display controllers require the origin and size of address windows to be multiples of a
/// fixed number of pixels.
/// The default alignment of one pixel on each axis preserves the renderer's existing behavior.
/// Set it with [`SoftwareRenderer::set_dirty_region_alignment`].
///
/// The physical screen width must be a multiple of the horizontal alignment, and the physical
/// screen height must be a multiple of the vertical alignment.
/// This applies after [`RenderingRotation`] transforms the screen.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct DirtyRegionAlignment {
    horizontal: u16,
    vertical: u16,
}

impl Default for DirtyRegionAlignment {
    fn default() -> Self {
        Self { horizontal: 1, vertical: 1 }
    }
}

impl DirtyRegionAlignment {
    /// Creates a physical dirty-region alignment.
    ///
    /// Values of zero are treated as one.
    pub fn new(horizontal: u16, vertical: u16) -> Self {
        Self { horizontal: horizontal.max(1), vertical: vertical.max(1) }
    }

    /// Returns the horizontal alignment in physical pixels.
    pub fn horizontal(self) -> u16 {
        self.horizontal
    }

    /// Returns the vertical alignment in physical pixels.
    pub fn vertical(self) -> u16 {
        self.vertical
    }
}

fn expand_dirty_region_for_alignment(
    dirty_region: &DirtyRegion,
    factor: ScaleFactor,
    rotation: RenderingRotation,
    alignment: DirtyRegionAlignment,
) -> DirtyRegion {
    let (horizontal, vertical) = if rotation.is_transpose() {
        (alignment.vertical(), alignment.horizontal())
    } else {
        (alignment.horizontal(), alignment.vertical())
    };
    let factor = factor.get();
    // Inflating by the full alignment is deliberately more than the `alignment - 1` pixels that
    // snapping can add on each side: it stays a superset after the logical-to-physical rounding
    // and is cheaper than inverse-mapping the snapped physical region back to logical space.
    let horizontal = (horizontal as f32 / factor).ceil() as Coord;
    let vertical = (vertical as f32 / factor).ceil() as Coord;
    let mut expanded = DirtyRegion::default();
    for dirty_box in dirty_region.iter() {
        expanded.add_box(dirty_box.inflate(horizontal, vertical));
    }
    expanded
}

fn snap_interval_to_grid(min: i16, max: i16, granularity: u16, limit: i16) -> (i16, i16) {
    let granularity = granularity as i32;
    let min = min as i32;
    let max = max as i32;
    let limit = limit as i32;
    let snapped_min = min.div_euclid(granularity) * granularity;
    // The clamp can leave the extent unaligned when the screen size is not a multiple of the
    // granularity; that configuration is rejected by the debug_assert in to_physical_region,
    // and this is the release-mode fallback for it.
    let snapped_max = ((max + granularity - 1).div_euclid(granularity) * granularity).min(limit);
    (snapped_min as i16, snapped_max as i16)
}

fn to_physical_region(
    dirty_region: &DirtyRegion,
    factor: ScaleFactor,
    rotation: RotationInfo,
    size: PhysicalSize,
    alignment: DirtyRegionAlignment,
) -> PhysicalRegion {
    let screen_rect = PhysicalRect::from_size(size);
    let panel_size = size.transformed(rotation);
    debug_assert!(
        panel_size.width <= 0 || panel_size.width as i32 % alignment.horizontal() as i32 == 0,
        "the screen width must be a multiple of the horizontal dirty-region alignment"
    );
    debug_assert!(
        panel_size.height <= 0 || panel_size.height as i32 % alignment.vertical() as i32 == 0,
        "the screen height must be a multiple of the vertical dirty-region alignment"
    );

    let mut physical_region = PhysicalRegion::default();
    for dirty_box in dirty_region.iter() {
        let Some(rect) =
            (dirty_box.cast() * factor).to_rect().round_out().cast().intersection(&screen_rect)
        else {
            continue;
        };
        let mut aligned_box = rect.transformed(rotation).to_box2d();
        if alignment.horizontal() > 1 {
            (aligned_box.min.x, aligned_box.max.x) = snap_interval_to_grid(
                aligned_box.min.x,
                aligned_box.max.x,
                alignment.horizontal(),
                panel_size.width,
            );
        }
        if alignment.vertical() > 1 {
            (aligned_box.min.y, aligned_box.max.y) = snap_interval_to_grid(
                aligned_box.min.y,
                aligned_box.max.y,
                alignment.vertical(),
                panel_size.height,
            );
        }
        if aligned_box.is_empty() {
            continue;
        }
        debug_assert!(physical_region.count < PHYSICAL_REGION_MAX_SIZE);
        physical_region.rectangles[physical_region.count] = aligned_box;
        physical_region.count += 1;
    }
    physical_region
}

#[test]
fn region_iter() {
    let mut region = PhysicalRegion::default();
    assert_eq!(region.iter().next(), None);
    region.rectangles[0] =
        euclid::Box2D::from_origin_and_size(euclid::point2(1, 1), euclid::size2(2, 3));
    region.rectangles[1] =
        euclid::Box2D::from_origin_and_size(euclid::point2(6, 2), euclid::size2(3, 20));
    region.rectangles[2] =
        euclid::Box2D::from_origin_and_size(euclid::point2(0, 10), euclid::size2(10, 5));
    assert_eq!(region.iter().next(), None);
    region.count = 1;
    let r = |x, y, width, height| {
        (
            i_slint_core::api::PhysicalPosition { x, y },
            i_slint_core::api::PhysicalSize { width, height },
        )
    };

    let mut iter = region.iter();
    assert_eq!(iter.next(), Some(r(1, 1, 2, 3)));
    assert_eq!(iter.next(), None);
    drop(iter);

    region.count = 3;
    let mut iter = region.iter();
    assert_eq!(iter.next(), Some(r(1, 1, 2, 1))); // the two first rectangle could have been merged
    assert_eq!(iter.next(), Some(r(1, 2, 2, 2)));
    assert_eq!(iter.next(), Some(r(6, 2, 3, 2)));
    assert_eq!(iter.next(), Some(r(6, 4, 3, 6)));
    assert_eq!(iter.next(), Some(r(0, 10, 10, 5)));
    assert_eq!(iter.next(), Some(r(6, 15, 3, 7)));
    assert_eq!(iter.next(), None);
}

#[test]
fn dirty_region_alignment_snaps_minimal_region() {
    use i_slint_core::lengths::LogicalRect;

    let factor = ScaleFactor::new(1.0);
    let size = euclid::size2(64, 64);
    let rotation = RotationInfo { orientation: RenderingRotation::NoRotation, screen_size: size };
    let mut dirty_region = DirtyRegion::default();
    dirty_region.add_rect(LogicalRect::new(euclid::point2(3.0, 5.0), euclid::size2(7.0, 9.0)));

    let aligned =
        to_physical_region(&dirty_region, factor, rotation, size, DirtyRegionAlignment::new(2, 2));
    assert_eq!(aligned.count, 1);
    assert_eq!(aligned.rectangles[0].min, euclid::point2(2, 4));
    assert_eq!(aligned.rectangles[0].max, euclid::point2(10, 14));

    let unaligned = to_physical_region(&dirty_region, factor, rotation, size, Default::default());
    assert_eq!(unaligned.count, 1);
    assert_eq!(unaligned.rectangles[0].min, euclid::point2(3, 5));
    assert_eq!(unaligned.rectangles[0].max, euclid::point2(10, 14));
}

#[test]
fn dirty_region_alignment_expands_with_scale_factor() {
    use i_slint_core::lengths::LogicalRect;

    let factor = ScaleFactor::new(1.5);
    let size = euclid::size2(48, 48);
    let rotation = RotationInfo { orientation: RenderingRotation::NoRotation, screen_size: size };
    let alignment = DirtyRegionAlignment::new(2, 2);
    let mut dirty_region = DirtyRegion::default();
    dirty_region.add_rect(LogicalRect::new(euclid::point2(3.0, 5.0), euclid::size2(7.0, 9.0)));

    let aligned = to_physical_region(&dirty_region, factor, rotation, size, alignment);
    assert_eq!(aligned.count, 1);
    assert_eq!(aligned.rectangles[0].min, euclid::point2(4, 6));
    assert_eq!(aligned.rectangles[0].max, euclid::point2(16, 22));

    // The logical expansion is in logical pixels, so it has to cover the alignment divided by
    // the scale factor, rounded up.
    let expanded = expand_dirty_region_for_alignment(
        &dirty_region,
        factor,
        RenderingRotation::NoRotation,
        alignment,
    );
    let expanded_box = expanded.iter().next().unwrap();
    assert_eq!(expanded_box.min, euclid::point2(1.0, 3.0));
    assert_eq!(expanded_box.max, euclid::point2(12.0, 16.0));

    // What is drawn must cover what is reported as rendered.
    let redrawn = to_physical_region(&expanded, factor, rotation, size, Default::default());
    assert!(redrawn.rectangles[0].contains_box(&aligned.rectangles[0]));
}

#[test]
fn dirty_region_alignment_accepts_non_power_of_two_grid() {
    use i_slint_core::lengths::LogicalRect;

    let factor = ScaleFactor::new(1.0);
    let size = euclid::size2(48, 48);
    let rotation = RotationInfo { orientation: RenderingRotation::NoRotation, screen_size: size };
    let mut dirty_region = DirtyRegion::default();
    dirty_region.add_rect(LogicalRect::new(euclid::point2(4.0, 7.0), euclid::size2(5.0, 5.0)));

    let aligned =
        to_physical_region(&dirty_region, factor, rotation, size, DirtyRegionAlignment::new(3, 3));
    assert_eq!(aligned.count, 1);
    assert_eq!(aligned.rectangles[0].min, euclid::point2(3, 6));
    assert_eq!(aligned.rectangles[0].max, euclid::point2(9, 12));
}

#[test]
fn dirty_region_alignment_uses_rotated_panel_axes() {
    use i_slint_core::lengths::LogicalRect;

    let factor = ScaleFactor::new(1.0);
    let size = euclid::size2(80, 40);
    let rotation = RotationInfo { orientation: RenderingRotation::Rotate90, screen_size: size };
    let alignment = DirtyRegionAlignment::new(4, 8);
    let mut dirty_region = DirtyRegion::default();
    dirty_region.add_rect(LogicalRect::new(euclid::point2(70.0, 30.0), euclid::size2(7.0, 7.0)));

    let aligned = to_physical_region(&dirty_region, factor, rotation, size, alignment);
    assert_eq!(aligned.count, 1);
    assert_eq!(aligned.rectangles[0].min, euclid::point2(0, 64));
    assert_eq!(aligned.rectangles[0].max, euclid::point2(12, 80));

    let expanded = expand_dirty_region_for_alignment(
        &dirty_region,
        factor,
        RenderingRotation::Rotate90,
        alignment,
    );
    let expanded_box = expanded.iter().next().unwrap();
    assert_eq!(expanded_box.min, euclid::point2(62.0, 26.0));
    assert_eq!(expanded_box.max, euclid::point2(85.0, 41.0));
}

#[test]
fn physical_region_count_excludes_clipped_rectangles() {
    use i_slint_core::lengths::LogicalRect;

    let factor = ScaleFactor::new(1.0);
    let size = euclid::size2(64, 64);
    let rotation = RotationInfo { orientation: RenderingRotation::NoRotation, screen_size: size };
    let mut dirty_region = DirtyRegion::default();
    dirty_region
        .add_rect(LogicalRect::new(euclid::point2(100.0, 100.0), euclid::size2(10.0, 10.0)));
    dirty_region.add_rect(LogicalRect::new(euclid::point2(3.0, 5.0), euclid::size2(7.0, 9.0)));

    let physical = to_physical_region(&dirty_region, factor, rotation, size, Default::default());
    assert_eq!(physical.count, 1);
    assert_eq!(physical.rectangles[0].min, euclid::point2(3, 5));
    assert_eq!(physical.rectangles[0].max, euclid::point2(10, 14));
}

/// Computes what are the x ranges that intersects the region for specified y line.
///
/// This uses a mutable reference to a Vec so that the memory is re-used between calls.
///
/// Returns the y position until which this range is valid
fn region_line_ranges(
    region: &PhysicalRegion,
    line: i16,
    line_ranges: &mut Vec<core::ops::Range<i16>>,
) -> Option<i16> {
    line_ranges.clear();
    let mut next_validity = None::<i16>;
    for geom in region.iter_box() {
        if geom.is_empty() {
            continue;
        }
        if geom.y_range().contains(&line) {
            match &mut next_validity {
                Some(val) => *val = geom.max.y.min(*val),
                None => next_validity = Some(geom.max.y),
            }
            let mut tmp = Some(geom.x_range());
            line_ranges.retain_mut(|it| {
                if let Some(r) = &mut tmp {
                    if it.end < r.start {
                        true
                    } else if it.start <= r.start {
                        if it.end >= r.end {
                            tmp = None;
                            return true;
                        }
                        r.start = it.start;
                        false
                    } else if it.start <= r.end {
                        if it.end <= r.end {
                            false
                        } else {
                            it.start = r.start;
                            tmp = None;
                            true
                        }
                    } else {
                        core::mem::swap(it, r);
                        true
                    }
                } else {
                    true
                }
            });
            if let Some(r) = tmp {
                line_ranges.push(r);
            }
            continue;
        } else if geom.min.y >= line {
            match &mut next_validity {
                Some(val) => *val = geom.min.y.min(*val),
                None => next_validity = Some(geom.min.y),
            }
        }
    }
    // check that current items are properly sorted
    debug_assert!(line_ranges.array_windows().all(|[a, b]| a.end < b.start));
    next_validity
}

mod target_pixel_buffer;

#[cfg(feature = "experimental")]
pub use target_pixel_buffer::{
    DrawRectangleArgs, DrawTextureArgs, TargetPixelBuffer, TexturePixelFormat,
};

#[cfg(not(feature = "experimental"))]
use target_pixel_buffer::TexturePixelFormat;

struct TargetPixelSlice<'a, T> {
    data: &'a mut [T],
    pixel_stride: usize,
}

impl<'a, T: TargetPixel> target_pixel_buffer::TargetPixelBuffer for TargetPixelSlice<'a, T> {
    type TargetPixel = T;

    fn line_slice(&mut self, line_number: usize) -> &mut [Self::TargetPixel] {
        let offset = line_number * self.pixel_stride;
        &mut self.data[offset..offset + self.pixel_stride]
    }

    fn num_lines(&self) -> usize {
        self.data.len() / self.pixel_stride
    }
}

/// A Renderer that do the rendering in software
///
/// The renderer can remember what items needs to be redrawn from the previous iteration.
///
/// There are two kind of possible rendering
///  1. Using [`render()`](Self::render()) to render the window in a buffer
///  2. Using [`render_by_line()`](Self::render()) to render the window line by line. This
///     is only useful if the device does not have enough memory to render the whole window
///     in one single buffer
pub struct SoftwareRenderer {
    repaint_buffer_type: Cell<RepaintBufferType>,
    dirty_region_alignment: Cell<DirtyRegionAlignment>,
    /// This is the area which was dirty on the previous frame.
    /// Only used if repaint_buffer_type == RepaintBufferType::SwappedBuffers
    prev_frame_dirty: Cell<DirtyRegion>,
    partial_rendering_state: PartialRenderingState,
    maybe_window_adapter: RefCell<Option<Weak<dyn i_slint_core::window::WindowAdapter>>>,
    rotation: Cell<RenderingRotation>,
    rendering_metrics_collector: Option<Rc<RenderingMetricsCollector>>,
    /// The blurred shadow alpha masks of `draw_box_shadow`,
    /// keyed by a hash of everything that produced them.
    shadow_mask_cache: RefCell<ShadowMaskCache>,
    #[cfg(feature = "systemfonts")]
    text_layout_cache: sharedparley::TextLayoutCache,
}

/// A bounded cache for box-shadow alpha masks: FIFO eviction.
#[derive(Default)]
struct ShadowMaskCache {
    masks: alloc::collections::BTreeMap<u64, Rc<[u8]>>,
    /// Insertion order of `masks`, for eviction.
    order: alloc::collections::VecDeque<u64>,
}

impl ShadowMaskCache {
    const MAX_ENTRIES: usize = 32;

    fn get(&self, key: u64) -> Option<Rc<[u8]>> {
        self.masks.get(&key).cloned()
    }

    fn insert(&mut self, key: u64, mask: Rc<[u8]>) {
        if !self.masks.contains_key(&key) {
            self.order.push_back(key);
        }
        self.masks.insert(key, mask);
        while self.order.len() > Self::MAX_ENTRIES {
            if let Some(evicted) = self.order.pop_front() {
                self.masks.remove(&evicted);
            }
        }
    }
}

impl Default for SoftwareRenderer {
    fn default() -> Self {
        Self {
            partial_rendering_state: Default::default(),
            prev_frame_dirty: Default::default(),
            dirty_region_alignment: Default::default(),
            maybe_window_adapter: Default::default(),
            rotation: Default::default(),
            rendering_metrics_collector: RenderingMetricsCollector::new("software"),
            repaint_buffer_type: Default::default(),
            shadow_mask_cache: RefCell::new(ShadowMaskCache {
                masks: Default::default(),
                order: Default::default(),
            }),
            #[cfg(feature = "systemfonts")]
            text_layout_cache: Default::default(),
        }
    }
}

#[cfg(feature = "testing")]
impl SoftwareRenderer {
    /// Returns a reference to the text layout cache for testing purposes.
    pub fn text_layout_cache(&self) -> &sharedparley::TextLayoutCache {
        &self.text_layout_cache
    }
}

impl SoftwareRenderer {
    /// Create a new Renderer
    pub fn new() -> Self {
        Default::default()
    }

    /// Create a new SoftwareRenderer.
    ///
    /// The `repaint_buffer_type` parameter specify what kind of buffer are passed to [`Self::render`]
    pub fn new_with_repaint_buffer_type(repaint_buffer_type: RepaintBufferType) -> Self {
        let self_ = Self::default();
        self_.repaint_buffer_type.set(repaint_buffer_type);
        self_
    }

    /// Change the what kind of buffer is being passed to [`Self::render`]
    ///
    /// This may clear the internal caches
    pub fn set_repaint_buffer_type(&self, repaint_buffer_type: RepaintBufferType) {
        if self.repaint_buffer_type.replace(repaint_buffer_type) != repaint_buffer_type {
            self.partial_rendering_state.clear_cache();
            let mut cache = self.shadow_mask_cache.borrow_mut();
            cache.masks.clear();
            cache.order.clear();
        }
    }

    /// Returns the kind of buffer that must be passed to  [`Self::render`]
    pub fn repaint_buffer_type(&self) -> RepaintBufferType {
        self.repaint_buffer_type.get()
    }

    /// Aligns dirty regions to the specified physical pixel grid.
    ///
    /// Use this for display controllers that require aligned address windows. The pixels the
    /// alignment adds are repainted, so the region returned by [`Self::render`] and the range
    /// passed to [`LineBufferProvider::process_line`] can be sent to the display as they are.
    ///
    /// The screen dimensions must be multiples of their corresponding alignment after applying
    /// [`RenderingRotation`].
    pub fn set_dirty_region_alignment(&self, alignment: DirtyRegionAlignment) {
        self.dirty_region_alignment.set(alignment);
    }

    /// Returns the physical pixel alignment for dirty regions.
    pub fn dirty_region_alignment(&self) -> DirtyRegionAlignment {
        self.dirty_region_alignment.get()
    }

    /// Set how the window need to be rotated in the buffer.
    ///
    /// This is typically used to implement screen rotation in software
    ///
    /// **Note:** This only affects rendering. Input events must still be given to
    /// Slint in logical (un-rotated) coordinates.
    pub fn set_rendering_rotation(&self, rotation: RenderingRotation) {
        self.rotation.set(rotation)
    }

    /// Return the current rotation. See [`Self::set_rendering_rotation()`]
    pub fn rendering_rotation(&self) -> RenderingRotation {
        self.rotation.get()
    }

    /// Render the window to the given frame buffer.
    ///
    /// The renderer uses a cache internally and will only render the part of the window
    /// which are dirty. The `extra_draw_region` is an extra region which will also
    /// be rendered. (eg: the previous dirty region in case of double buffering)
    /// This function returns the region that was rendered.
    ///
    /// The pixel_stride is the size (in pixels) between two lines in the buffer.
    /// It is equal `width` if the screen is not rotated, and `height` if the screen is rotated by 90°.
    /// The buffer needs to be big enough to contain the window, so its size must be at least
    /// `pixel_stride * height`, or `pixel_stride * width` if the screen is rotated by 90°.
    ///
    /// Returns the physical dirty region for this frame, excluding the extra_draw_region,
    /// in the window frame of reference. It is affected by the screen rotation.
    pub fn render(&self, buffer: &mut [impl TargetPixel], pixel_stride: usize) -> PhysicalRegion {
        self.render_buffer_impl(&mut TargetPixelSlice { data: buffer, pixel_stride })
    }

    /// Render the window to the given frame buffer.
    ///
    /// The renderer uses a cache internally and will only render the part of the window
    /// which are dirty. The `extra_draw_region` is an extra region which will also
    /// be rendered. (eg: the previous dirty region in case of double buffering)
    /// This function returns the region that was rendered.
    ///
    /// The buffer's line slices need to be wide enough to if the `width` of the screen and the line count the `height`,
    /// or the `height` and `width` swapped if the screen is rotated by 90°.
    ///
    /// Returns the physical dirty region for this frame, excluding the extra_draw_region,
    /// in the window frame of reference. It is affected by the screen rotation.
    #[cfg(feature = "experimental")]
    pub fn render_into_buffer(&self, buffer: &mut impl TargetPixelBuffer) -> PhysicalRegion {
        self.render_buffer_impl(buffer)
    }

    fn render_buffer_impl(
        &self,
        buffer: &mut impl target_pixel_buffer::TargetPixelBuffer,
    ) -> PhysicalRegion {
        let pixels_per_line = buffer.line_slice(0).len();
        let num_lines = buffer.num_lines();
        let buffer_pixel_count = num_lines * pixels_per_line;

        let Some(window) = self.maybe_window_adapter.borrow().as_ref().and_then(|w| w.upgrade())
        else {
            return Default::default();
        };
        let window_inner = WindowInner::from_pub(window.window());
        let factor = ScaleFactor::new(window_inner.scale_factor());
        let rotation = self.rotation.get();
        let (size, background) = if let Some(window_item) =
            window_inner.window_item().as_ref().map(|item| item.as_pin_ref())
        {
            (
                (LogicalSize::from_lengths(window_item.width(), window_item.height()).cast()
                    * factor)
                    .cast(),
                window_item.background(),
            )
        } else if rotation.is_transpose() {
            (euclid::size2(num_lines as _, pixels_per_line as _), Brush::default())
        } else {
            (euclid::size2(pixels_per_line as _, num_lines as _), Brush::default())
        };
        if size.is_empty() {
            return Default::default();
        }
        assert!(
            if rotation.is_transpose() {
                pixels_per_line >= size.height as usize
                    && buffer_pixel_count
                        >= (size.width as usize * pixels_per_line + size.height as usize)
                            - pixels_per_line
            } else {
                pixels_per_line >= size.width as usize
                    && buffer_pixel_count
                        >= (size.height as usize * pixels_per_line + size.width as usize)
                            - pixels_per_line
            },
            "buffer of size {} with {pixels_per_line} pixels per line is too small to handle a window of size {size:?}",
            buffer_pixel_count
        );
        let buffer_renderer = SceneBuilder::new(
            size,
            factor,
            window_inner,
            RenderToBuffer {
                buffer,
                dirty_range_cache: Vec::new(),
                dirty_region: Default::default(),
                scale_factor: factor,
                clip_mask: None,
                mask_scratch: Vec::new(),
                mask_row: Vec::new(),
            },
            rotation,
            &self.shadow_mask_cache,
            #[cfg(feature = "systemfonts")]
            &self.text_layout_cache,
        );
        let mut renderer = self.partial_rendering_state.create_partial_renderer(buffer_renderer);
        let window_adapter = renderer.window_adapter.clone();

        window_inner
            .draw_contents(|components, post_render| {
                let logical_size = (size.cast() / factor).cast();

                let dirty_region = self.compute_frame_dirty_region(
                    &mut renderer,
                    components,
                    logical_size,
                    factor,
                    size,
                );

                renderer.actual_renderer.processor.dirty_region = dirty_region.clone();
                if !renderer
                    .actual_renderer
                    .processor
                    .buffer
                    .fill_background(&background, &dirty_region)
                {
                    let mut bg = TargetPixel::background();
                    // TODO: gradient background
                    TargetPixel::blend(&mut bg, background.color().into());
                    renderer.actual_renderer.processor.foreach_ranges(
                        &dirty_region.bounding_rect(),
                        |_, buffer, _, _| {
                            buffer.fill(bg);
                        },
                    );
                }

                let partial = self.repaint_buffer_type.get() != RepaintBufferType::NewBuffer;
                for (component, origin) in components {
                    if let Some(component) = ItemTreeWeak::upgrade(component) {
                        i_slint_core::item_rendering::render_component_items(
                            &component,
                            if partial { &mut renderer } else { &mut renderer.actual_renderer },
                            *origin,
                            &window_adapter,
                        );
                    }
                }

                if partial {
                    post_render(&mut renderer);
                } else {
                    post_render(&mut renderer.actual_renderer);
                }

                self.measure_frame_rendered(&mut renderer);

                dirty_region
            })
            .unwrap_or_default()
    }

    /// Computes the dirty region for this frame according to the repaint buffer type, converts
    /// it to the physical region to return to the caller, applying the configured
    /// [`DirtyRegionAlignment`], and expands the logical dirty region in place so that the
    /// partial renderer repaints every pixel the alignment added.
    ///
    /// This runs before the items are drawn, so `renderer`'s dirty region is what the partial
    /// renderer culls against.
    ///
    /// For `SwappedBuffers`, `prev_frame_dirty` intentionally keeps the unexpanded region:
    /// the next frame starts from a superset of it and re-applies the expansion.
    fn compute_frame_dirty_region<T: ItemRenderer + ItemRendererFeatures>(
        &self,
        renderer: &mut PartialRenderer<'_, T>,
        components: &[(ItemTreeWeak, LogicalPoint)],
        logical_size: LogicalSize,
        factor: ScaleFactor,
        size: PhysicalSize,
    ) -> PhysicalRegion {
        match self.repaint_buffer_type.get() {
            RepaintBufferType::NewBuffer => {
                // NewBuffer always redraws the full screen, so skip dirty region
                // tracking to avoid unbounded growth of the partial rendering cache.
                renderer.dirty_region = LogicalRect::from_size(logical_size).into();
                self.partial_rendering_state.clear_cache();
            }
            RepaintBufferType::ReusedBuffer => {
                self.partial_rendering_state.apply_dirty_region(
                    renderer,
                    components,
                    logical_size,
                    None,
                );
            }
            RepaintBufferType::SwappedBuffers => {
                let dirty_region_for_this_frame = self.partial_rendering_state.apply_dirty_region(
                    renderer,
                    components,
                    logical_size,
                    Some(self.prev_frame_dirty.take()),
                );
                self.prev_frame_dirty.set(dirty_region_for_this_frame);
            }
        }

        let alignment = self.dirty_region_alignment.get();
        let rotation = self.rotation.get();
        let physical_region = to_physical_region(
            &renderer.dirty_region,
            factor,
            RotationInfo { orientation: rotation, screen_size: size },
            size,
            alignment,
        );
        if alignment != DirtyRegionAlignment::default()
            && self.repaint_buffer_type.get() != RepaintBufferType::NewBuffer
        {
            renderer.dirty_region = expand_dirty_region_for_alignment(
                &renderer.dirty_region,
                factor,
                rotation,
                alignment,
            );
        }
        physical_region
    }

    fn measure_frame_rendered(&self, renderer: &mut dyn ItemRenderer) {
        if let Some(metrics) = &self.rendering_metrics_collector {
            let prev_frame_dirty = self.prev_frame_dirty.take();
            let m = i_slint_core::graphics::rendering_metrics_collector::RenderingMetrics {
                dirty_region: Some(prev_frame_dirty.clone()),
                ..Default::default()
            };
            self.prev_frame_dirty.set(prev_frame_dirty);
            metrics.measure_frame_rendered(renderer, m);
            if metrics.refresh_mode() == RefreshMode::FullSpeed {
                self.partial_rendering_state.force_screen_refresh();
            }
        }
    }

    /// Render the window, line by line, into the line buffer provided by the [`LineBufferProvider`].
    ///
    /// The renderer uses a cache internally and will only render the part of the window
    /// which are dirty, depending on the dirty tracking policy set in [`SoftwareRenderer::new`]
    /// This function returns the physical region that was rendered considering the rotation.
    ///
    /// The [`LineBufferProvider::process_line()`] function will be called for each line and should
    ///  provide a buffer to draw into.
    ///
    /// As an example, let's imagine we want to render into a plain buffer.
    /// (You wouldn't normally use `render_by_line` for that because the [`Self::render`] would
    /// then be more efficient)
    ///
    /// ```rust
    /// # use i_slint_renderer_software::{LineBufferProvider, SoftwareRenderer, Rgb565Pixel};
    /// # fn xxx<'a>(the_frame_buffer: &'a mut [Rgb565Pixel], display_width: usize, renderer: &SoftwareRenderer) {
    /// struct FrameBuffer<'a>{ frame_buffer: &'a mut [Rgb565Pixel], stride: usize }
    /// impl<'a> LineBufferProvider for FrameBuffer<'a> {
    ///     type TargetPixel = Rgb565Pixel;
    ///     fn process_line(
    ///         &mut self,
    ///         line: usize,
    ///         range: core::ops::Range<usize>,
    ///         render_fn: impl FnOnce(&mut [Self::TargetPixel]),
    ///     ) {
    ///         let line_begin = line * self.stride;
    ///         render_fn(&mut self.frame_buffer[line_begin..][range]);
    ///         // The line has been rendered and there could be code here to
    ///         // send the pixel to the display
    ///     }
    /// }
    /// renderer.render_by_line(FrameBuffer{ frame_buffer: the_frame_buffer, stride: display_width });
    /// # }
    /// ```
    pub fn render_by_line(&self, line_buffer: impl LineBufferProvider) -> PhysicalRegion {
        let Some(window) = self.maybe_window_adapter.borrow().as_ref().and_then(|w| w.upgrade())
        else {
            return Default::default();
        };
        let window_inner = WindowInner::from_pub(window.window());
        let component_rc = window_inner.component();
        let component = i_slint_core::item_tree::ItemTreeRc::borrow_pin(&component_rc);
        if let Some(window_item) = i_slint_core::items::ItemRef::downcast_pin::<
            i_slint_core::items::WindowItem,
        >(component.as_ref().get_item_ref(0))
        {
            let factor = ScaleFactor::new(window_inner.scale_factor());
            let size = LogicalSize::from_lengths(window_item.width(), window_item.height()).cast()
                * factor;
            render_window_frame_by_line(
                window_inner,
                window_item.background(),
                size.cast(),
                self,
                line_buffer,
            )
        } else {
            PhysicalRegion { ..Default::default() }
        }
    }
}

#[doc(hidden)]
impl RendererSealed for SoftwareRenderer {
    #[cfg(feature = "systemfonts")]
    fn text_layout_cache(&self) -> Option<&sharedparley::TextLayoutCache> {
        Some(&self.text_layout_cache)
    }

    fn text_size(
        &self,
        text_item: Pin<&dyn i_slint_core::item_rendering::RenderString>,
        item_rc: &i_slint_core::item_tree::ItemRc,
        max_width: Option<LogicalLength>,
        text_wrap: TextWrap,
    ) -> LogicalSize {
        let Some(scale_factor) = self.scale_factor() else {
            return LogicalSize::default();
        };
        let font_request = text_item.font_request(item_rc);
        // Evaluate text() before borrowing font_context: the binding can
        // re-enter text_size for other elements and would panic on a second
        // borrow_mut().
        let content = text_item.text();
        #[cfg(feature = "systemfonts")]
        let Some(slint_ctx) = self.slint_context() else {
            return Default::default();
        };
        let font = {
            #[cfg(feature = "systemfonts")]
            let mut font_ctx = slint_ctx.font_context().borrow_mut();
            fonts::match_font(
                &font_request,
                scale_factor,
                #[cfg(feature = "systemfonts")]
                &mut font_ctx,
            )
        };

        #[cfg(feature = "systemfonts")]
        if uses_parley(&font) {
            return sharedparley::text_size(
                self,
                text_item,
                item_rc,
                max_width,
                text_wrap,
                Some(&self.text_layout_cache),
            )
            .unwrap_or_default();
        }

        let max_lines = text_item.line_limit();
        let string = match &content {
            PlainOrStyledText::Plain(string) => alloc::borrow::Cow::Borrowed(string.as_str()),
            PlainOrStyledText::Styled(styled_text) => {
                i_slint_core::styled_text::get_raw_text(styled_text)
            }
        };
        let (longest_line_width, height) = with_font!(&font, |font| {
            let layout = fonts::text_layout_for_font(font, &font_request, scale_factor);
            layout.text_size(
                &string,
                max_width.map(|max_width| (max_width.cast() * scale_factor).cast()),
                text_wrap,
                max_lines,
            )
        });
        (PhysicalSize::from_lengths(longest_line_width, height).cast() / scale_factor).cast()
    }

    // Mirrors the font selection in text_size(), so both widths always come from the
    // same font as what is rendered. The bitmap path measures word breaks only, which is
    // all the caller asks for: char-wrap has no minimum and never reaches here.
    fn text_content_widths(
        &self,
        text_item: Pin<&dyn i_slint_core::item_rendering::RenderString>,
        item_rc: &i_slint_core::item_tree::ItemRc,
    ) -> Option<i_slint_core::renderer::ContentWidths> {
        let scale_factor = self.scale_factor()?;
        let font_request = text_item.font_request(item_rc);
        // Evaluate text() before borrowing font_context, as in text_size().
        let content = text_item.text();
        #[cfg(feature = "systemfonts")]
        let slint_ctx = self.slint_context()?;
        let font = {
            #[cfg(feature = "systemfonts")]
            let mut font_ctx = slint_ctx.font_context().borrow_mut();
            fonts::match_font(
                &font_request,
                scale_factor,
                #[cfg(feature = "systemfonts")]
                &mut font_ctx,
            )
        };

        #[cfg(feature = "systemfonts")]
        if uses_parley(&font) {
            return sharedparley::text_content_widths(
                self,
                text_item,
                item_rc,
                Some(&self.text_layout_cache),
            );
        }

        let max_lines = text_item.line_limit();
        let string = match &content {
            PlainOrStyledText::Plain(string) => alloc::borrow::Cow::Borrowed(string.as_str()),
            PlainOrStyledText::Styled(styled_text) => {
                i_slint_core::styled_text::get_raw_text(styled_text)
            }
        };
        let (min, max) = with_font!(&font, |font| {
            fonts::text_layout_for_font(font, &font_request, scale_factor)
                .content_widths(&string, max_lines)
        });
        Some(i_slint_core::renderer::ContentWidths {
            min: (min.cast() / scale_factor).cast(),
            max: (max.cast() / scale_factor).cast(),
        })
    }

    // Bitmap fonts fall back to text_size() with the same font.
    fn text_line_height(
        &self,
        font_request: i_slint_core::graphics::FontRequest,
    ) -> Option<LogicalLength> {
        #[cfg(feature = "systemfonts")]
        {
            let scale_factor = self.scale_factor()?;
            let slint_ctx = self.slint_context()?;
            let mut font_ctx = slint_ctx.font_context().borrow_mut();
            let font = fonts::match_font(&font_request, scale_factor, &mut font_ctx);
            if uses_parley(&font) {
                return sharedparley::text_line_height(&mut font_ctx, &font_request);
            }
        }
        #[cfg(not(feature = "systemfonts"))]
        let _ = font_request;
        None
    }

    fn char_size(
        &self,
        text_item: Pin<&dyn i_slint_core::item_rendering::HasFont>,
        item_rc: &i_slint_core::item_tree::ItemRc,
        ch: char,
    ) -> LogicalSize {
        let Some(scale_factor) = self.scale_factor() else {
            return LogicalSize::default();
        };
        let font_request = text_item.font_request(item_rc);
        #[cfg(feature = "systemfonts")]
        let Some(slint_ctx) = self.slint_context() else {
            return Default::default();
        };
        let font = {
            #[cfg(feature = "systemfonts")]
            let mut font_ctx = slint_ctx.font_context().borrow_mut();
            fonts::match_font(
                &font_request,
                scale_factor,
                #[cfg(feature = "systemfonts")]
                &mut font_ctx,
            )
        };

        #[cfg(feature = "systemfonts")]
        if uses_parley(&font) {
            let mut font_ctx = slint_ctx.font_context().borrow_mut();
            return sharedparley::char_size(&mut font_ctx, text_item, item_rc, ch)
                .unwrap_or_default();
        }

        let (longest_line_width, height) = with_font!(&font, |font| {
            let mut buf = [0u8, 0u8, 0u8, 0u8];
            let layout = fonts::text_layout_for_font(font, &font_request, scale_factor);
            layout.text_size(ch.encode_utf8(&mut buf), None, TextWrap::NoWrap, None)
        });
        (PhysicalSize::from_lengths(longest_line_width, height).cast() / scale_factor).cast()
    }

    fn font_metrics(
        &self,
        font_request: i_slint_core::graphics::FontRequest,
    ) -> i_slint_core::items::FontMetrics {
        let Some(scale_factor) = self.scale_factor() else {
            return i_slint_core::items::FontMetrics::default();
        };
        #[cfg(feature = "systemfonts")]
        let Some(slint_ctx) = self.slint_context() else {
            return Default::default();
        };
        #[cfg(feature = "systemfonts")]
        let mut font_ctx = slint_ctx.font_context().borrow_mut();
        let font = fonts::match_font(
            &font_request,
            scale_factor,
            #[cfg(feature = "systemfonts")]
            &mut font_ctx,
        );

        #[cfg(feature = "systemfonts")]
        if uses_parley(&font) {
            return sharedparley::font_metrics(&mut font_ctx, font_request);
        }

        // `Font` forwards the metrics to the concrete font itself, so this needs no dispatch.
        let ascent: LogicalLength = (font.ascent().cast() / scale_factor).cast();
        let descent: LogicalLength = (font.descent().cast() / scale_factor).cast();
        let x_height: LogicalLength = (font.x_height().cast() / scale_factor).cast();
        let cap_height: LogicalLength = (font.cap_height().cast() / scale_factor).cast();

        i_slint_core::items::FontMetrics {
            ascent: ascent.get() as _,
            descent: descent.get() as _,
            x_height: x_height.get() as _,
            cap_height: cap_height.get() as _,
        }
    }

    fn text_input_byte_offset_for_position(
        &self,
        text_input: Pin<&i_slint_core::items::TextInput>,
        item_rc: &ItemRc,
        pos: LogicalPoint,
    ) -> (usize, i_slint_core::items::TextCursorAffinity) {
        let Some(scale_factor) = self.scale_factor() else {
            return Default::default();
        };
        let font_request = text_input.font_request(item_rc);
        #[cfg(feature = "systemfonts")]
        let Some(slint_ctx) = self.slint_context() else {
            return Default::default();
        };
        let font = {
            #[cfg(feature = "systemfonts")]
            let mut font_ctx = slint_ctx.font_context().borrow_mut();
            fonts::match_font(
                &font_request,
                scale_factor,
                #[cfg(feature = "systemfonts")]
                &mut font_ctx,
            )
        };

        #[cfg(feature = "systemfonts")]
        if uses_parley(&font) {
            return sharedparley::text_input_byte_offset_for_position(
                self,
                text_input,
                item_rc,
                pos,
                Some(&self.text_layout_cache),
            );
        }

        let visual_representation = text_input.visual_representation();

        let pos = (pos.cast() * scale_factor)
            .clamp(euclid::point2(0., 0.), euclid::point2(i16::MAX, i16::MAX).cast())
            .cast();

        let byte_offset = with_font!(&font, |font| {
            let layout = fonts::text_layout_for_font(font, &font_request, scale_factor);
            let paragraph = text_input_query_paragraph(
                text_input,
                &visual_representation.text,
                layout,
                scale_factor,
            );
            paragraph.byte_offset_for_position((pos.x_length(), pos.y_length()))
        });

        (
            visual_representation.map_byte_offset_from_visual_text_to_actual_text(byte_offset),
            i_slint_core::items::TextCursorAffinity::NextCharacter,
        )
    }

    // Answers for the accessibility tree what `draw_text_input` decides for drawing; both
    // ask `uses_parley`, so they cannot describe a layout that isn't the one drawn.
    #[cfg(feature = "systemfonts")]
    fn text_input_has_parley_layout(
        &self,
        text_input: Pin<&i_slint_core::items::TextInput>,
        item_rc: &ItemRc,
    ) -> bool {
        let (Some(scale_factor), Some(slint_ctx)) = (self.scale_factor(), self.slint_context())
        else {
            return false;
        };
        let font_request = text_input.font_request(item_rc);
        let font = {
            let mut font_ctx = slint_ctx.font_context().borrow_mut();
            fonts::match_font(&font_request, scale_factor, &mut font_ctx)
        };

        uses_parley(&font)
    }

    fn text_input_cursor_rect_for_byte_offset(
        &self,
        text_input: Pin<&i_slint_core::items::TextInput>,
        item_rc: &ItemRc,
        byte_offset: usize,
        affinity: i_slint_core::items::TextCursorAffinity,
    ) -> LogicalRect {
        #[cfg(not(feature = "systemfonts"))]
        let _ = affinity;
        let Some(scale_factor) = self.scale_factor() else {
            return LogicalRect::default();
        };
        let font_request = text_input.font_request(item_rc);
        #[cfg(feature = "systemfonts")]
        let Some(slint_ctx) = self.slint_context() else {
            return Default::default();
        };
        let font = {
            #[cfg(feature = "systemfonts")]
            let mut font_ctx = slint_ctx.font_context().borrow_mut();
            fonts::match_font(
                &font_request,
                scale_factor,
                #[cfg(feature = "systemfonts")]
                &mut font_ctx,
            )
        };

        #[cfg(feature = "systemfonts")]
        if uses_parley(&font) {
            return sharedparley::text_input_cursor_rect_for_byte_offset(
                self,
                text_input,
                item_rc,
                byte_offset,
                affinity,
                Some(&self.text_layout_cache),
            );
        }

        let visual_representation = text_input.visual_representation();

        let (cursor_position, band_offset, cursor_height) = with_font!(&font, |font| {
            let layout = fonts::text_layout_for_font(font, &font_request, scale_factor);
            let paragraph = text_input_query_paragraph(
                text_input,
                &visual_representation.text,
                layout,
                scale_factor,
            );

            let cursor_position = paragraph.cursor_pos_for_byte_offset(byte_offset);
            let (band_offset, cursor_height) = paragraph.layout.cursor_band();
            (cursor_position, band_offset, cursor_height)
        });

        (PhysicalRect::new(
            PhysicalPoint::from_lengths(cursor_position.0, cursor_position.1 + band_offset),
            PhysicalSize::from_lengths(
                (text_input.text_cursor_width().cast() * scale_factor).cast(),
                cursor_height,
            ),
        )
        .cast()
            / scale_factor)
            .cast()
    }

    fn free_graphics_resources(
        &self,
        component: i_slint_core::item_tree::ItemTreeRef,
        items: &mut dyn Iterator<Item = Pin<i_slint_core::items::ItemRef<'_>>>,
    ) -> Result<(), i_slint_core::platform::PlatformError> {
        #[cfg(feature = "systemfonts")]
        self.text_layout_cache.component_destroyed(component);
        self.partial_rendering_state.free_graphics_resources(component, items);
        Ok(())
    }

    fn mark_dirty_region(&self, region: DirtyRegion) {
        self.partial_rendering_state.mark_dirty_region(region);
    }

    fn register_bitmap_font(&self, font_data: &'static i_slint_core::graphics::BitmapFont) {
        fonts::register_bitmap_font(font_data);
    }

    #[cfg(any(feature = "systemfonts", feature = "embedded-vector-fonts"))]
    fn register_font_from_memory(
        &self,
        data: &'static [u8],
    ) -> Result<(), alloc::boxed::Box<dyn core::error::Error>> {
        #[cfg(feature = "systemfonts")]
        let result = {
            let ctx = self.slint_context().ok_or("slint platform not initialized")?;
            ctx.font_context().borrow_mut().register_static_font(data);
            Ok(())
        };
        #[cfg(all(feature = "embedded-vector-fonts", not(feature = "systemfonts")))]
        let result = fonts::embeddedfonts::register(data);
        result
    }

    #[cfg(all(feature = "systemfonts", not(target_arch = "wasm32")))]
    fn register_font_from_path(
        &self,
        path: &std::path::Path,
    ) -> Result<(), std::boxed::Box<dyn std::error::Error>> {
        let ctx = self.slint_context().ok_or("slint platform not initialized")?;
        self::fonts::systemfonts::register_font_from_path(
            &mut ctx.font_context().borrow_mut().collection,
            path,
        )
    }

    fn set_window_adapter(&self, window_adapter: &Rc<dyn WindowAdapter>) {
        *self.maybe_window_adapter.borrow_mut() = Some(Rc::downgrade(window_adapter));
        #[cfg(feature = "systemfonts")]
        self.text_layout_cache.clear_all();
        self.partial_rendering_state.clear_cache();
    }

    fn window_adapter(&self) -> Option<Rc<dyn WindowAdapter>> {
        self.maybe_window_adapter
            .borrow()
            .as_ref()
            .and_then(|window_adapter| window_adapter.upgrade())
    }

    // Rendering to a second pixel format monomorphizes the pipeline twice; keep it out of MCU builds.
    #[cfg(feature = "std")]
    fn take_snapshot(&self) -> Result<SharedPixelBuffer<Rgba8Pixel>, PlatformError> {
        let Some(window_adapter) =
            self.maybe_window_adapter.borrow().as_ref().and_then(|w| w.upgrade())
        else {
            return Err(
                "SoftwareRenderer's screenshot called without a window adapter present".into()
            );
        };

        let window = window_adapter.window();
        let size = window.size();

        if size.width == 0 || size.height == 0 {
            // Nothing to render
            return Err("take_snapshot() called on window with invalid size".into());
        };

        // Render into a premultiplied buffer so that windows with a transparent
        // or semi-transparent background end up with the right alpha in the
        // snapshot. PremultipliedRgbaColor::background() is (0,0,0,0), so
        // anything the window doesn't paint stays fully transparent.
        let mut premul = SharedPixelBuffer::<PremultipliedRgbaColor>::new(size.width, size.height);

        let old_repaint_buffer_type = self.repaint_buffer_type();
        // ensure that caches are clear
        self.set_repaint_buffer_type(RepaintBufferType::NewBuffer);
        self.render(premul.make_mut_slice(), size.width as usize);
        self.set_repaint_buffer_type(old_repaint_buffer_type);

        let mut target_buffer_with_alpha =
            SharedPixelBuffer::<Rgba8Pixel>::new(premul.width(), premul.height());
        for (target_pixel, source_pixel) in
            target_buffer_with_alpha.make_mut_slice().iter_mut().zip(premul.as_slice().iter())
        {
            // Un-premultiply: straight RGBA is what the public API exposes (and
            // what PNG encoders expect). Round half up to keep `255 * a / a == 255`.
            let a = source_pixel.alpha;
            if a == 0 {
                *target_pixel = Rgba8Pixel::new(0, 0, 0, 0);
            } else {
                let unp = |c: u8| ((c as u32 * 255 + (a as u32 / 2)) / a as u32).min(255) as u8;
                *target_pixel = Rgba8Pixel::new(
                    unp(source_pixel.red),
                    unp(source_pixel.green),
                    unp(source_pixel.blue),
                    a,
                );
            }
        }
        Ok(target_buffer_with_alpha)
    }

    fn supports_transformations(&self) -> bool {
        false
    }
}

/// Only the vector font path can hand off to parley, so this is only ever asked under
/// `systemfonts`; without it there is nothing to disable.
///
/// Read once: this sits on the per-item, per-frame text path, and every other `SLINT_`
/// switch is resolved once as well. Setting the variable after the first text is laid
/// out therefore has no effect.
#[cfg(feature = "systemfonts")]
fn parley_disabled() -> bool {
    static DISABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *DISABLED.get_or_init(|| std::env::var_os("SLINT_SOFTWARE_RENDERER_PARLEY_DISABLED").is_some())
}

/// Whether this font is laid out by parley rather than by the local text layout.
///
/// Every text method has to agree on this: the accessibility tree describes the layout
/// `text_input_has_parley_layout` reports, and it has to be the one that was drawn.
#[cfg(feature = "systemfonts")]
fn uses_parley(font: &fonts::Font) -> bool {
    matches!(font, fonts::Font::VectorFont(_)) && !parley_disabled()
}

/// The paragraph layout the cursor and hit-testing queries measure against.
///
/// `string` is the text input's visual representation, which the caller owns because the
/// returned layout borrows from it.
///
/// Note that this does not pass on the text input's `single_line`, matching what these
/// queries did before they shared this helper. `draw_text_input` does pass it on.
fn text_input_query_paragraph<'a, Font>(
    text_input: Pin<&i_slint_core::items::TextInput>,
    string: &'a str,
    layout: i_slint_core::textlayout::TextLayout<'a, Font>,
    scale_factor: ScaleFactor,
) -> TextParagraphLayout<'a, Font>
where
    Font: AbstractFont + i_slint_core::textlayout::TextShaper<Length = PhysicalLength>,
{
    TextParagraphLayout {
        string,
        layout,
        max_width: (text_input.width().cast() * scale_factor).cast(),
        max_height: (text_input.height().cast() * scale_factor).cast(),
        horizontal_alignment: text_input.horizontal_alignment(),
        vertical_alignment: text_input.vertical_alignment(),
        wrap: text_input.wrap(),
        overflow: TextOverflow::Clip,
        single_line: false,
        max_lines: None,
    }
}

fn render_window_frame_by_line(
    window: &WindowInner,
    background: Brush,
    size: PhysicalSize,
    renderer: &SoftwareRenderer,
    mut line_buffer: impl LineBufferProvider,
) -> PhysicalRegion {
    let mut scene = prepare_scene(window, size, renderer);

    let to_draw_tr = scene.dirty_region.bounding_rect();

    // One rasterizer per scene path, primed on first use: spans touching the
    // same path on consecutive lines reuse the built edge table, whose
    // active-edge window stays valid since lines advance monotonically.
    let mut path_rasterizers: Vec<Option<shape_raster::Rasterizer>> =
        (0..scene.vectors.paths.len()).map(|_| None).collect();

    let mut background_color = TargetPixel::background();
    // FIXME gradient
    TargetPixel::blend(&mut background_color, background.color().into());

    while scene.current_line < to_draw_tr.origin.y_length() + to_draw_tr.size.height_length() {
        for r in &scene.current_line_ranges {
            line_buffer.process_line(
                scene.current_line.get() as usize,
                r.start as usize..r.end as usize,
                |line_buffer| {
                    let offset = r.start;

                    let items = &scene.items[0..scene.current_items_index];
                    // A span that opaquely covers the whole range hides
                    // everything behind it, background included. Draw from the
                    // frontmost such span and drop the rest.
                    let first_cover = items.iter().position(|span| {
                        span.pos.x <= r.start
                            && span.pos.x + span.size.width >= r.end
                            && scene.is_guaranteed_opaque(&span.command)
                    });
                    let items = match first_cover {
                        Some(i) => &items[..=i],
                        None => {
                            line_buffer.fill(background_color);
                            items
                        }
                    };
                    for span in items.iter().rev() {
                        debug_assert!(scene.current_line >= span.pos.y_length());
                        debug_assert!(
                            scene.current_line < span.pos.y_length() + span.size.height_length(),
                        );
                        if span.pos.x >= r.end {
                            continue;
                        }
                        let begin = r.start.max(span.pos.x);
                        let end = r.end.min(span.pos.x + span.size.width);
                        if begin >= end {
                            continue;
                        }

                        let extra_left_clip = begin - span.pos.x;
                        let extra_right_clip = span.pos.x + span.size.width - end;
                        let range_buffer =
                            &mut line_buffer[(begin - offset) as usize..(end - offset) as usize];

                        // A shape clip applies the outline's coverage on top
                        // of the rectangular clip: draw into a copy of the
                        // destination, then lerp it back by the coverage.
                        let mut mask = Vec::new();
                        let mut scratch = Vec::new();
                        if span.clip != 0 {
                            let clip = &scene.vectors.clip_outlines[span.clip as usize - 1];
                            scratch.clear();
                            scratch.extend_from_slice(range_buffer);
                            mask.resize(range_buffer.len(), 0);
                            clip.rasterize_row(
                                scene.current_line.get() as i32,
                                begin as i32,
                                &mut mask,
                            );
                        }

                        match span.command {
                            SceneCommand::Rectangle { color } => {
                                TargetPixel::blend_slice(range_buffer, color);
                            }
                            SceneCommand::Texture { texture_index } => {
                                let texture = &scene.vectors.textures[texture_index as usize];
                                draw_functions::draw_texture_line(
                                    &PhysicalRect { origin: span.pos, size: span.size },
                                    scene.current_line,
                                    texture,
                                    range_buffer,
                                    extra_left_clip,
                                    extra_right_clip,
                                );
                            }
                            SceneCommand::SharedBuffer { shared_buffer_index } => {
                                let texture = scene.vectors.shared_buffers
                                    [shared_buffer_index as usize]
                                    .as_texture();
                                draw_functions::draw_texture_line(
                                    &PhysicalRect { origin: span.pos, size: span.size },
                                    scene.current_line,
                                    &texture,
                                    range_buffer,
                                    extra_left_clip,
                                    extra_right_clip,
                                );
                            }
                            SceneCommand::RoundedRectangle { rectangle_index } => {
                                let rr =
                                    &scene.vectors.rounded_rectangles[rectangle_index as usize];
                                draw_functions::draw_rounded_rectangle_line(
                                    &PhysicalRect { origin: span.pos, size: span.size },
                                    scene.current_line,
                                    rr,
                                    range_buffer,
                                    extra_left_clip,
                                    extra_right_clip,
                                );
                            }
                            SceneCommand::LinearGradient { linear_gradient_index } => {
                                let g =
                                    &scene.vectors.linear_gradients[linear_gradient_index as usize];

                                draw_functions::draw_linear_gradient(
                                    &PhysicalRect { origin: span.pos, size: span.size },
                                    scene.current_line,
                                    g,
                                    range_buffer,
                                    extra_left_clip,
                                );
                            }
                            SceneCommand::RadialGradient { radial_gradient_index } => {
                                let g =
                                    &scene.vectors.radial_gradients[radial_gradient_index as usize];
                                draw_functions::draw_radial_gradient(
                                    &PhysicalRect { origin: span.pos, size: span.size },
                                    scene.current_line,
                                    g,
                                    range_buffer,
                                    extra_left_clip,
                                    extra_right_clip,
                                );
                            }
                            SceneCommand::ConicGradient { conic_gradient_index } => {
                                let g =
                                    &scene.vectors.conic_gradients[conic_gradient_index as usize];
                                draw_functions::draw_conic_gradient(
                                    &PhysicalRect { origin: span.pos, size: span.size },
                                    scene.current_line,
                                    g,
                                    range_buffer,
                                    extra_left_clip,
                                    extra_right_clip,
                                );
                            }
                            SceneCommand::Path { path_index } => {
                                let data = &scene.vectors.paths[path_index as usize];
                                let rasterizer = path_rasterizers[path_index as usize]
                                    .get_or_insert_with(|| {
                                        let mut r = shape_raster::Rasterizer::default();
                                        r.begin(&data.contours);
                                        r
                                    });
                                let mut row = Vec::new();
                                draw_path_line(
                                    scene.current_line.get() as i32,
                                    range_buffer,
                                    begin as i32,
                                    data,
                                    rasterizer,
                                    &mut row,
                                );
                            }
                        }

                        if !mask.is_empty() {
                            for (d, (o, &m)) in range_buffer
                                .iter_mut()
                                .zip(scratch.iter().copied().zip(mask.iter()))
                            {
                                let mut px = o;
                                px.lerp_from(*d, m);
                                *d = px;
                            }
                        }
                    }
                },
            );
        }

        if scene.current_line < to_draw_tr.origin.y_length() + to_draw_tr.size.height_length() {
            scene.next_line();
        }
    }
    scene.dirty_region
}

fn prepare_scene(
    window: &WindowInner,
    size: PhysicalSize,
    software_renderer: &SoftwareRenderer,
) -> Scene {
    let factor = ScaleFactor::new(window.scale_factor());
    let prepare_scene = SceneBuilder::new(
        size,
        factor,
        window,
        PrepareScene { scale_factor: factor, ..Default::default() },
        software_renderer.rotation.get(),
        &software_renderer.shadow_mask_cache,
        #[cfg(feature = "systemfonts")]
        &software_renderer.text_layout_cache,
    );
    let mut renderer =
        software_renderer.partial_rendering_state.create_partial_renderer(prepare_scene);
    let window_adapter = renderer.window_adapter.clone();

    let mut dirty_region = PhysicalRegion::default();
    window.draw_contents(|components, post_render| {
        let logical_size = (size.cast() / factor).cast();

        dirty_region = software_renderer.compute_frame_dirty_region(
            &mut renderer,
            components,
            logical_size,
            factor,
            size,
        );

        let partial = software_renderer.repaint_buffer_type.get() != RepaintBufferType::NewBuffer;
        for (component, origin) in components {
            if let Some(component) = ItemTreeWeak::upgrade(component) {
                i_slint_core::item_rendering::render_component_items(
                    &component,
                    if partial { &mut renderer } else { &mut renderer.actual_renderer },
                    *origin,
                    &window_adapter,
                );
            }
        }

        if partial {
            post_render(&mut renderer);
        } else {
            post_render(&mut renderer.actual_renderer);
        }
    });

    software_renderer.measure_frame_rendered(&mut renderer);

    let prepare_scene = renderer.into_inner();

    /* // visualize dirty regions
    let mut prepare_scene = prepare_scene;
    for rect in dirty_region.iter() {
        prepare_scene.processor.process_rounded_rectangle(
            euclid::rect(rect.0.x as _, rect.0.y as _, rect.1.width as _, rect.1.height as _),
            RoundedRectangle {
                radius: BorderRadius::default(),
                width: Length::new(1),
                border_color: Color::from_argb_u8(128, 255, 0, 0).into(),
                inner_color: PremultipliedRgbaColor::default(),
                left_clip: Length::default(),
                right_clip: Length::default(),
                top_clip: Length::default(),
                bottom_clip: Length::default(),
            },
        )
    } // */

    Scene::new(prepare_scene.processor.items, prepare_scene.processor.vectors, dirty_region)
}

trait ProcessScene {
    fn process_scene_texture(&mut self, geometry: PhysicalRect, texture: SceneTexture<'static>);
    fn process_target_texture(
        &mut self,
        texture: &target_pixel_buffer::DrawTextureArgs,
        clip: PhysicalRect,
    );
    fn process_rectangle(&mut self, _: &target_pixel_buffer::DrawRectangleArgs, clip: PhysicalRect);

    fn process_simple_rectangle(&mut self, geometry: PhysicalRect, color: PremultipliedRgbaColor);
    fn process_rounded_rectangle(&mut self, geometry: PhysicalRect, data: RoundedRectangle);
    fn process_linear_gradient(&mut self, geometry: PhysicalRect, gradient: LinearGradientCommand);
    fn process_radial_gradient(&mut self, geometry: PhysicalRect, gradient: RadialGradientCommand);
    fn process_conic_gradient(&mut self, geometry: PhysicalRect, gradient: ConicGradientCommand);
    /// Draws one flattened path (a fill, or a stroke already expanded to
    /// fill contours), clipped to `clip_geometry`. The `data`'s contours are
    /// in absolute physical screen coordinates, rotation applied.
    fn process_path(&mut self, data: alloc::rc::Rc<PathCommandData>, clip_geometry: PhysicalRect);
    /// Sets the shape clip applying to subsequent draws: a mask AND-ed per
    /// row with everything drawn while it is set, on top of the rectangular
    /// clip.
    fn set_clip_outline(&mut self, clip: Option<alloc::rc::Rc<ClipOutlineData>>);
}

fn process_rectangle_impl(
    processor: &mut dyn ProcessScene,
    args: &target_pixel_buffer::DrawRectangleArgs,
    clip: &PhysicalRect,
    scale_factor: ScaleFactor,
) {
    let geom = args.geometry();
    let Some(clipped) = geom.intersection(&clip.cast()) else { return };
    let geom_w = geom.width();
    let geom_h = geom.height();
    let to_clipped_center = |cx: f32, cy: f32| {
        (geom.min_x() + cx - clipped.min_x(), geom.min_y() + cy - clipped.min_y())
    };

    let color = if let Brush::LinearGradient(g) = &args.background {
        let angle = g.angle() + args.rotation.angle();
        let tan = angle.to_radians().tan().abs();
        let start = if !tan.is_finite() {
            255.
        } else {
            let h = tan * geom.width();
            255. * h / (h + geom.height())
        } as u8;
        let mut angle = angle as i32 % 360;
        if angle < 0 {
            angle += 360;
        }
        let mut stops = g
            .stops()
            .copied()
            .map(|mut s| {
                s.color = alpha_color(s.color, args.alpha);
                s
            })
            .peekable();
        let mut idx = 0;
        let stop_count = g.stops().count();
        while let (Some(mut s1), Some(mut s2)) = (stops.next(), stops.peek().copied()) {
            let mut flags = 0;
            if (angle % 180) > 90 {
                flags |= 0b1;
            }
            if angle <= 90 || angle > 270 {
                core::mem::swap(&mut s1, &mut s2);
                s1.position = 1. - s1.position;
                s2.position = 1. - s2.position;
                if idx == 0 {
                    flags |= 0b100;
                }
                if idx == stop_count - 2 {
                    flags |= 0b010;
                }
            } else {
                if idx == 0 {
                    flags |= 0b010;
                }
                if idx == stop_count - 2 {
                    flags |= 0b100;
                }
            }

            idx += 1;

            let (adjust_left, adjust_right) = if (angle % 180) > 90 {
                (
                    (geom.width() * s1.position).floor() as i16,
                    (geom.width() * (1. - s2.position)).ceil() as i16,
                )
            } else {
                (
                    (geom.width() * (1. - s2.position)).ceil() as i16,
                    (geom.width() * s1.position).floor() as i16,
                )
            };

            let gr = LinearGradientCommand {
                color1: s1.color.into(),
                color2: s2.color.into(),
                start,
                flags,
                top_clip: Length::new(
                    (clipped.min_y() - geom.min_y() - (geom.height() * s1.position).floor()) as i16,
                ),
                bottom_clip: Length::new(
                    (geom.max_y() - clipped.max_y() - (geom.height() * (1. - s2.position)).ceil())
                        as i16,
                ),
                left_clip: Length::new((clipped.min_x() - geom.min_x()) as i16 - adjust_left),
                right_clip: Length::new((geom.max_x() - clipped.max_x()) as i16 - adjust_right),
            };

            let act_rect = clipped.round().cast();
            let size_y = act_rect.height_length() + gr.top_clip + gr.bottom_clip;
            let size_x = act_rect.width_length() + gr.left_clip + gr.right_clip;
            if size_x.get() == 0 || size_y.get() == 0 {
                // the position are too close to each other
                // FIXME: For the first or the last, we should draw a plain color to the end
                continue;
            }

            processor.process_linear_gradient(act_rect, gr);
        }
        Color::default()
    } else if let Brush::RadialGradient(g) = &args.background {
        let (cx, cy) = g.center_or_default_scaled(geom_w, geom_h, scale_factor.get());
        let (center_x, center_y) = to_clipped_center(cx, cy);
        let radius = g.radius_or_default_scaled(geom_w, geom_h, scale_factor.get());

        let radial_grad = RadialGradientCommand {
            stops: g
                .stops()
                .map(|s| {
                    let mut stop = *s;
                    stop.color = alpha_color(stop.color, args.alpha);
                    stop
                })
                .collect(),
            center_x,
            center_y,
            radius,
        };

        processor.process_radial_gradient(clipped.cast(), radial_grad);
        Color::default()
    } else if let Brush::ConicGradient(g) = &args.background {
        let (cx, cy) = g.center_or_default_scaled(geom_w, geom_h, scale_factor.get());
        let (center_x, center_y) = to_clipped_center(cx, cy);
        let conic_grad = ConicGradientCommand {
            stops: g
                .stops()
                .map(|s| {
                    let mut stop = *s;
                    stop.color = alpha_color(stop.color, args.alpha);
                    stop
                })
                .collect(),
            center_x,
            center_y,
        };

        processor.process_conic_gradient(clipped.cast(), conic_grad);
        Color::default()
    } else {
        alpha_color(args.background.color(), args.alpha)
    };

    let mut border_color =
        PremultipliedRgbaColor::from(alpha_color(args.border.color(), args.alpha));
    let color = PremultipliedRgbaColor::from(color);
    let mut border = PhysicalLength::new(args.border_width as _);
    if border_color.alpha == 0 {
        border = PhysicalLength::new(0);
    } else if border_color.alpha < 255 {
        // Find a color for the border which is an equivalent to blend the background and then the border.
        // In the end, the resulting of blending the background and the color is
        // (A + B) + C, where A is the buffer color, B is the background, and C is the border.
        // which expands to (A*(1-Bα) + B*Bα)*(1-Cα) + C*Cα = A*(1-(Bα+Cα-Bα*Cα)) + B*Bα*(1-Cα) + C*Cα
        // so let the new alpha be: Nα = Bα+Cα-Bα*Cα, then this is A*(1-Nα) + N*Nα
        // with N = (B*Bα*(1-Cα) + C*Cα)/Nα
        // N being the equivalent color of the border that mixes the background and the border
        // In pre-multiplied space, the formula simplifies further N' = B'*(1-Cα) + C'
        let b = border_color;
        let b_alpha_16 = b.alpha as u16;
        border_color = PremultipliedRgbaColor {
            red: ((color.red as u16 * (255 - b_alpha_16)) / 255) as u8 + b.red,
            green: ((color.green as u16 * (255 - b_alpha_16)) / 255) as u8 + b.green,
            blue: ((color.blue as u16 * (255 - b_alpha_16)) / 255) as u8 + b.blue,
            alpha: (color.alpha as u16 + b_alpha_16 - (color.alpha as u16 * b_alpha_16) / 255)
                as u8,
        }
    }

    let radius = PhysicalBorderRadius {
        top_left: args.top_left_radius as _,
        top_right: args.top_right_radius as _,
        bottom_right: args.bottom_right_radius as _,
        bottom_left: args.bottom_left_radius as _,
        _unit: Default::default(),
    };

    if !radius.is_zero() {
        // Add a small value to make sure that the clip is always positive despite floating point shenanigans
        const E: f32 = 0.00001;

        processor.process_rounded_rectangle(
            clipped.round().cast(),
            RoundedRectangle {
                radius,
                width: border,
                border_color,
                inner_color: color,
                top_clip: PhysicalLength::new((clipped.min_y() - geom.min_y() + E) as _),
                bottom_clip: PhysicalLength::new((geom.max_y() - clipped.max_y() + E) as _),
                left_clip: PhysicalLength::new((clipped.min_x() - geom.min_x() + E) as _),
                right_clip: PhysicalLength::new((geom.max_x() - clipped.max_x() + E) as _),
            },
        );
        return;
    }

    if color.alpha > 0
        && let Some(r) =
            geom.round().cast().inflate(-border.get(), -border.get()).intersection(clip)
    {
        processor.process_simple_rectangle(r, color);
    }

    if border_color.alpha > 0 {
        let mut add_border = |r: PhysicalRect| {
            if let Some(r) = r.intersection(clip) {
                processor.process_simple_rectangle(r, border_color);
            }
        };
        let b = border.get();
        let g = geom.round().cast();
        add_border(euclid::rect(g.min_x(), g.min_y(), g.width(), b));
        add_border(euclid::rect(g.min_x(), g.min_y() + g.height() - b, g.width(), b));
        add_border(euclid::rect(g.min_x(), g.min_y() + b, b, g.height() - b - b));
        add_border(euclid::rect(g.min_x() + g.width() - b, g.min_y() + b, b, g.height() - b - b));
    }
}

struct RenderToBuffer<'a, B: target_pixel_buffer::TargetPixelBuffer> {
    buffer: &'a mut B,
    dirty_range_cache: Vec<core::ops::Range<i16>>,
    dirty_region: PhysicalRegion,
    scale_factor: ScaleFactor,
    /// The active shape clip, masked per row on top of the rectangular clip.
    clip_mask: Option<alloc::rc::Rc<ClipOutlineData>>,
    /// Scratch rows for masked compositing and path coverage.
    mask_scratch: Vec<B::TargetPixel>,
    mask_row: Vec<u8>,
}

impl<B: target_pixel_buffer::TargetPixelBuffer> RenderToBuffer<'_, B> {
    fn foreach_ranges(
        &mut self,
        geometry: &PhysicalRect,
        mut f: impl FnMut(i16, &mut [B::TargetPixel], i16, i16),
    ) {
        let has_clip_mask = self.clip_mask.is_some();
        let mut line = geometry.min_y();
        while let Some(mut next) =
            region_line_ranges(&self.dirty_region, line, &mut self.dirty_range_cache)
        {
            next = next.min(geometry.max_y());
            for r in &self.dirty_range_cache {
                if geometry.origin.x >= r.end {
                    continue;
                }
                let begin = r.start.max(geometry.origin.x);
                let end = r.end.min(geometry.origin.x + geometry.size.width);
                if begin >= end {
                    continue;
                }
                let extra_left_clip = begin - geometry.origin.x;
                let extra_right_clip = geometry.origin.x + geometry.size.width - end;

                let region = PhysicalRect {
                    origin: PhysicalPoint::new(begin, line),
                    size: PhysicalSize::new(end - begin, next - line),
                };

                for l in region.y_range() {
                    let dst = &mut self.buffer.line_slice(l as usize)
                        [region.min_x() as usize..region.max_x() as usize];
                    if !has_clip_mask {
                        f(l, dst, extra_left_clip, extra_right_clip);
                        continue;
                    }
                    // Masked span: draw into a copy of the destination row,
                    // then lerp the destination toward the result by the
                    // clip coverage. Bounded to the span's width.
                    let clip = self.clip_mask.clone().unwrap();
                    self.mask_row.clear();
                    self.mask_row.resize(dst.len(), 0);
                    clip.rasterize_row(l as i32, region.min_x() as i32, &mut self.mask_row);
                    self.mask_scratch.clear();
                    self.mask_scratch.extend_from_slice(dst);
                    f(l, &mut self.mask_scratch, extra_left_clip, extra_right_clip);
                    for (d, (s, &m)) in dst
                        .iter_mut()
                        .zip(self.mask_scratch.iter().copied().zip(self.mask_row.iter()))
                    {
                        d.lerp_from(s, m);
                    }
                }
            }
            if next == geometry.max_y() {
                break;
            }
            line = next;
        }
    }

    fn process_texture_impl(&mut self, geometry: PhysicalRect, texture: SceneTexture<'_>) {
        self.foreach_ranges(&geometry, |line, buffer, extra_left_clip, extra_right_clip| {
            draw_functions::draw_texture_line(
                &geometry,
                PhysicalLength::new(line),
                &texture,
                buffer,
                extra_left_clip,
                extra_right_clip,
            );
        });
    }
}

/// Rasterizes `data`'s coverage for line `l` (columns start at `x_start`)
/// into `row` and blends the brush's per-pixel color scaled by coverage.
fn draw_path_line<P: TargetPixel>(
    l: i32,
    dst: &mut [P],
    x_start: i32,
    data: &PathCommandData,
    rasterizer: &mut shape_raster::Rasterizer,
    row: &mut Vec<u8>,
) {
    row.clear();
    row.resize(dst.len(), 0);
    rasterizer.rasterize_row(l, x_start, row, data.fill_rule);
    for (i, (pix, &cov)) in dst.iter_mut().zip(row.iter()).enumerate() {
        if cov == 0 {
            continue;
        }
        let color = eval_path_brush(&data.brush, x_start + i as i32, l, data);
        pix.blend(scale_premult(color, cov));
    }
}

/// The premultiplied `color` scaled down by `coverage` (0..=255).
fn scale_premult(color: PremultipliedRgbaColor, coverage: u8) -> PremultipliedRgbaColor {
    if coverage == 255 {
        return color;
    }
    let m = coverage as u16;
    PremultipliedRgbaColor {
        red: (color.red as u16 * m / 255) as u8,
        green: (color.green as u16 * m / 255) as u8,
        blue: (color.blue as u16 * m / 255) as u8,
        alpha: (color.alpha as u16 * m / 255) as u8,
    }
}

/// Evaluates a path's brush at the physical pixel center `(x + 0.5, y + 0.5)`,
/// mapping the pixel back through `data.rotation` so gradient parameters stay
/// in the frame the shape was drawn in.
fn eval_path_brush(
    brush: &PathBrush,
    x: i32,
    y: i32,
    data: &PathCommandData,
) -> PremultipliedRgbaColor {
    let p = untransform_continuous(
        euclid::point2::<f32, PhysicalPx>(x as f32 + 0.5, y as f32 + 0.5),
        data.rotation,
    );
    let bounds = &data.brush_bounds;
    match brush {
        PathBrush::Solid(color) => *color,
        PathBrush::LinearGradient { stops, angle_deg } => {
            let t = linear_gradient_t(*angle_deg, bounds, p.x, p.y);
            eval_stops(stops, t)
        }
        PathBrush::RadialGradient { stops, center_x, center_y, radius } => {
            if *radius <= 0. {
                return eval_stops(stops, 1.);
            }
            let dx = p.x - center_x;
            let dy = p.y - center_y;
            eval_stops(stops, (dx * dx + dy * dy).sqrt() / radius)
        }
        PathBrush::ConicGradient { stops, center_x, center_y } => {
            let dx = p.x - center_x;
            let dy = p.y - center_y;
            // Angle clockwise from north, matching draw_conic_gradient.
            let a = dy.atan2(dx) + core::f32::consts::FRAC_PI_2;
            let tau = 2. * core::f32::consts::PI;
            let t = (a % tau + tau) % tau / tau;
            eval_stops(stops, t)
        }
    }
}

/// The normalized gradient coordinate of point `(x, y)` for a linear
/// gradient of `angle_deg` degrees over `bounds`. The projection direction
/// is the same convention `process_rectangle_impl` uses: angle 0 sweeps
/// top to bottom.
fn linear_gradient_t(
    angle_deg: f32,
    bounds: &euclid::Rect<f32, PhysicalPx>,
    x: f32,
    y: f32,
) -> f32 {
    let a = angle_deg.to_radians();
    let (dx, dy) = (a.sin(), a.cos());
    let corners = [
        (bounds.min_x(), bounds.min_y()),
        (bounds.max_x(), bounds.min_y()),
        (bounds.min_x(), bounds.max_y()),
        (bounds.max_x(), bounds.max_y()),
    ];
    let mut t_min = f32::MAX;
    let mut t_max = f32::MIN;
    for (cx, cy) in corners {
        let t = cx * dx + cy * dy;
        t_min = t_min.min(t);
        t_max = t_max.max(t);
    }
    if t_max - t_min < 1e-6 {
        return 1.;
    }
    ((x * dx + y * dy) - t_min) / (t_max - t_min)
}

/// Converts a [`Brush`] into a path brush, baking `alpha` into the color or
/// every stop. `bounds` is the path's physical bounding box: gradient centers
/// and radii are resolved against it. Returns `None` for a transparent brush.
fn path_brush(
    brush: &i_slint_core::Brush,
    alpha: f32,
    bounds: euclid::Rect<f32, PhysicalPx>,
) -> Option<PathBrush> {
    let alpha_u8 = (alpha * 255.) as u8;
    match brush {
        i_slint_core::Brush::SolidColor(color) => {
            let color = alpha_color(*color, alpha_u8);
            if color.alpha() == 0 { None } else { Some(PathBrush::Solid(color.into())) }
        }
        i_slint_core::Brush::LinearGradient(g) => Some(PathBrush::LinearGradient {
            stops: alloc::rc::Rc::new(
                g.stops()
                    .map(|s| {
                        let mut s = *s;
                        s.color = alpha_color(s.color, alpha_u8);
                        s
                    })
                    .collect(),
            ),
            angle_deg: g.angle(),
        }),
        i_slint_core::Brush::RadialGradient(g) => {
            let (w, h) = (bounds.width(), bounds.height());
            let (cx, cy) = g.center_or_default_scaled(w, h, 1.);
            Some(PathBrush::RadialGradient {
                stops: alloc::rc::Rc::new(
                    g.stops()
                        .map(|s| {
                            let mut s = *s;
                            s.color = alpha_color(s.color, alpha_u8);
                            s
                        })
                        .collect(),
                ),
                center_x: bounds.min_x() + cx,
                center_y: bounds.min_y() + cy,
                radius: g.radius_or_default_scaled(w, h, 1.),
            })
        }
        i_slint_core::Brush::ConicGradient(g) => {
            let (w, h) = (bounds.width(), bounds.height());
            let (cx, cy) = g.center_or_default_scaled(w, h, 1.);
            Some(PathBrush::ConicGradient {
                stops: alloc::rc::Rc::new(
                    g.stops()
                        .map(|s| {
                            let mut s = *s;
                            s.color = alpha_color(s.color, alpha_u8);
                            s
                        })
                        .collect(),
                ),
                center_x: bounds.min_x() + cx,
                center_y: bounds.min_y() + cy,
            })
        }
        _ => None,
    }
}

/// The axis-aligned bounding box of `contours`, or `None` when empty.
fn contours_bounds(contours: &[shape_raster::Contour]) -> Option<euclid::Rect<f32, PhysicalPx>> {
    let mut min = shape_raster::Point::new(f32::MAX, f32::MAX);
    let mut max = shape_raster::Point::new(f32::MIN, f32::MIN);
    for c in contours {
        for p in c {
            min = min.min(*p);
            max = max.max(*p);
        }
    }
    (min.x <= max.x && min.y <= max.y)
        .then(|| euclid::Rect::new(min, euclid::size2(max.x - min.x, max.y - min.y)))
}

/// Interpolates the sorted `stops` at normalized position `t`.
fn eval_stops(stops: &[i_slint_core::graphics::GradientStop], t: f32) -> PremultipliedRgbaColor {
    let Some(first) = stops.first() else { return Default::default() };
    if t <= first.position {
        return first.color.into();
    }
    for [s1, s2] in stops.array_windows() {
        if t <= s2.position {
            let f = if s2.position > s1.position {
                ((t - s1.position) / (s2.position - s1.position)).clamp(0., 1.)
            } else {
                1.
            };
            let (c1, c2): (PremultipliedRgbaColor, PremultipliedRgbaColor) =
                (s1.color.into(), s2.color.into());
            let w = (f * 256.) as i32;
            let lerp = |a: u8, b: u8| {
                (a as i32 + ((b as i32 - a as i32) * w + 128) / 256).clamp(0, 255) as u8
            };
            return PremultipliedRgbaColor {
                red: lerp(c1.red, c2.red),
                green: lerp(c1.green, c2.green),
                blue: lerp(c1.blue, c2.blue),
                alpha: lerp(c1.alpha, c2.alpha),
            };
        }
    }
    stops.last().map(|s| s.color.into()).unwrap_or_default()
}

impl<B: target_pixel_buffer::TargetPixelBuffer> ProcessScene for RenderToBuffer<'_, B> {
    fn process_scene_texture(&mut self, geometry: PhysicalRect, texture: SceneTexture<'static>) {
        self.process_texture_impl(geometry, texture);
    }

    fn process_target_texture(
        &mut self,
        texture: &target_pixel_buffer::DrawTextureArgs,
        clip: PhysicalRect,
    ) {
        if self.buffer.draw_texture(texture, &self.dirty_region.intersection(&clip)) {
            return;
        }

        let Some((texture, geometry)) = SceneTexture::from_target_texture(texture, &clip) else {
            return;
        };

        self.process_texture_impl(geometry, texture);
    }

    fn process_rectangle(
        &mut self,
        args: &target_pixel_buffer::DrawRectangleArgs,
        clip: PhysicalRect,
    ) {
        if self.buffer.draw_rectangle(args, &self.dirty_region.intersection(&clip)) {
            return;
        }

        let scale_factor = self.scale_factor;
        process_rectangle_impl(self, args, &clip, scale_factor);
    }

    fn process_rounded_rectangle(&mut self, geometry: PhysicalRect, rr: RoundedRectangle) {
        self.foreach_ranges(&geometry, |line, buffer, extra_left_clip, extra_right_clip| {
            draw_functions::draw_rounded_rectangle_line(
                &geometry,
                PhysicalLength::new(line),
                &rr,
                buffer,
                extra_left_clip,
                extra_right_clip,
            );
        });
    }

    fn process_simple_rectangle(&mut self, geometry: PhysicalRect, color: PremultipliedRgbaColor) {
        self.foreach_ranges(&geometry, |_line, buffer, _extra_left_clip, _extra_right_clip| {
            <B::TargetPixel>::blend_slice(buffer, color)
        });
    }

    fn process_linear_gradient(&mut self, geometry: PhysicalRect, g: LinearGradientCommand) {
        self.foreach_ranges(&geometry, |line, buffer, extra_left_clip, _extra_right_clip| {
            draw_functions::draw_linear_gradient(
                &geometry,
                PhysicalLength::new(line),
                &g,
                buffer,
                extra_left_clip,
            );
        });
    }
    fn process_radial_gradient(&mut self, geometry: PhysicalRect, g: RadialGradientCommand) {
        self.foreach_ranges(&geometry, |line, buffer, extra_left_clip, extra_right_clip| {
            draw_functions::draw_radial_gradient(
                &geometry,
                PhysicalLength::new(line),
                &g,
                buffer,
                extra_left_clip,
                extra_right_clip,
            );
        });
    }
    fn process_conic_gradient(&mut self, geometry: PhysicalRect, g: ConicGradientCommand) {
        self.foreach_ranges(&geometry, |line, buffer, extra_left_clip, extra_right_clip| {
            draw_functions::draw_conic_gradient(
                &geometry,
                PhysicalLength::new(line),
                &g,
                buffer,
                extra_left_clip,
                extra_right_clip,
            );
        });
    }

    fn process_path(&mut self, data: alloc::rc::Rc<PathCommandData>, clip_geometry: PhysicalRect) {
        let Some(geometry) = data.bounds.round_out().cast::<i16>().intersection(&clip_geometry)
        else {
            return;
        };
        let mut rasterizer = shape_raster::Rasterizer::default();
        rasterizer.begin(&data.contours);
        let mut row = Vec::new();
        self.foreach_ranges(&geometry, |line, buffer, extra_left_clip, _extra_right_clip| {
            draw_path_line(
                line as i32,
                buffer,
                geometry.min_x() as i32 + extra_left_clip as i32,
                &data,
                &mut rasterizer,
                &mut row,
            );
        });
    }

    fn set_clip_outline(&mut self, clip: Option<alloc::rc::Rc<ClipOutlineData>>) {
        self.clip_mask = clip;
    }
}

#[derive(Default)]
struct PrepareScene {
    items: Vec<SceneItem>,
    vectors: SceneVectors,
    scale_factor: ScaleFactor,
    /// The active shape clip and its index (1-based) in `clip_outlines`.
    current_clip: Option<alloc::rc::Rc<ClipOutlineData>>,
    current_clip_index: u16,
}

impl ProcessScene for PrepareScene {
    fn process_scene_texture(&mut self, geometry: PhysicalRect, texture: SceneTexture<'static>) {
        let texture_index = self.vectors.textures.len() as u16;
        self.vectors.textures.push(texture);
        self.items.push(SceneItem {
            pos: geometry.origin,
            size: geometry.size,
            z: self.items.len() as u16,
            clip: self.current_clip_index,
            command: SceneCommand::Texture { texture_index },
        });
    }

    fn process_target_texture(
        &mut self,
        texture: &target_pixel_buffer::DrawTextureArgs,
        clip: PhysicalRect,
    ) {
        let Some((extra, geometry)) = SceneTextureExtra::from_target_texture(texture, &clip) else {
            return;
        };
        match &texture.data {
            target_pixel_buffer::TextureDataContainer::Static(texture_data) => {
                let texture_index = self.vectors.textures.len() as u16;
                let pixel_stride =
                    (texture_data.byte_stride / texture_data.pixel_format.bpp()) as u16;
                self.vectors.textures.push(SceneTexture {
                    data: texture_data.data,
                    format: texture_data.pixel_format,
                    pixel_stride,
                    extra,
                });
                self.items.push(SceneItem {
                    pos: geometry.origin,
                    size: geometry.size,
                    z: self.items.len() as u16,
                    clip: self.current_clip_index,
                    command: SceneCommand::Texture { texture_index },
                });
            }
            target_pixel_buffer::TextureDataContainer::Shared { buffer, source_rect } => {
                let shared_buffer_index = self.vectors.shared_buffers.len() as u16;
                self.vectors.shared_buffers.push(SharedBufferCommand {
                    buffer: buffer.clone(),
                    source_rect: *source_rect,
                    extra,
                });
                self.items.push(SceneItem {
                    pos: geometry.origin,
                    size: geometry.size,
                    z: self.items.len() as u16,
                    clip: self.current_clip_index,
                    command: SceneCommand::SharedBuffer { shared_buffer_index },
                });
            }
        }
    }

    fn process_rectangle(
        &mut self,
        args: &target_pixel_buffer::DrawRectangleArgs,
        clip: PhysicalRect,
    ) {
        let scale_factor = self.scale_factor;
        process_rectangle_impl(self, args, &clip, scale_factor);
    }

    fn process_simple_rectangle(&mut self, geometry: PhysicalRect, color: PremultipliedRgbaColor) {
        let size = geometry.size;
        if !size.is_empty() {
            let z = self.items.len() as u16;
            let pos = geometry.origin;
            self.items.push(SceneItem {
                pos,
                size,
                z,
                clip: self.current_clip_index,
                command: SceneCommand::Rectangle { color },
            });
        }
    }

    fn process_rounded_rectangle(&mut self, geometry: PhysicalRect, data: RoundedRectangle) {
        let size = geometry.size;
        if !size.is_empty() {
            let rectangle_index = self.vectors.rounded_rectangles.len() as u16;
            self.vectors.rounded_rectangles.push(data);
            self.items.push(SceneItem {
                pos: geometry.origin,
                size,
                z: self.items.len() as u16,
                clip: self.current_clip_index,
                command: SceneCommand::RoundedRectangle { rectangle_index },
            });
        }
    }

    fn process_linear_gradient(&mut self, geometry: PhysicalRect, gradient: LinearGradientCommand) {
        let size = geometry.size;
        if !size.is_empty() {
            let gradient_index = self.vectors.linear_gradients.len() as u16;
            self.vectors.linear_gradients.push(gradient);
            self.items.push(SceneItem {
                pos: geometry.origin,
                size,
                z: self.items.len() as u16,
                clip: self.current_clip_index,
                command: SceneCommand::LinearGradient { linear_gradient_index: gradient_index },
            });
        }
    }
    fn process_radial_gradient(&mut self, geometry: PhysicalRect, gradient: RadialGradientCommand) {
        let size = geometry.size;
        if !size.is_empty() {
            let radial_gradient_index = self.vectors.radial_gradients.len() as u16;
            self.vectors.radial_gradients.push(gradient);
            self.items.push(SceneItem {
                pos: geometry.origin,
                size,
                z: self.items.len() as u16,
                clip: self.current_clip_index,
                command: SceneCommand::RadialGradient { radial_gradient_index },
            });
        }
    }
    fn process_conic_gradient(&mut self, geometry: PhysicalRect, gradient: ConicGradientCommand) {
        let size = geometry.size;
        if !size.is_empty() {
            let conic_gradient_index = self.vectors.conic_gradients.len() as u16;
            self.vectors.conic_gradients.push(gradient);
            self.items.push(SceneItem {
                pos: geometry.origin,
                size,
                z: self.items.len() as u16,
                clip: self.current_clip_index,
                command: SceneCommand::ConicGradient { conic_gradient_index },
            });
        }
    }

    fn process_path(&mut self, data: alloc::rc::Rc<PathCommandData>, clip_geometry: PhysicalRect) {
        let Some(geometry) = data.bounds.round_out().cast::<i16>().intersection(&clip_geometry)
        else {
            return;
        };
        if geometry.size.is_empty() {
            return;
        }
        let path_index = self.vectors.paths.len() as u16;
        self.vectors.paths.push(data);
        self.items.push(SceneItem {
            pos: geometry.origin,
            size: geometry.size,
            z: self.items.len() as u16,
            clip: self.current_clip_index,
            command: SceneCommand::Path { path_index },
        });
    }

    fn set_clip_outline(&mut self, clip: Option<alloc::rc::Rc<ClipOutlineData>>) {
        if let Some(c) = &clip {
            // Reuse the stored outline when the same clip is re-set.
            self.current_clip_index = self
                .vectors
                .clip_outlines
                .iter()
                .position(|o| alloc::rc::Rc::ptr_eq(o, c))
                .map(|i| i as u16 + 1)
                .unwrap_or_else(|| {
                    self.vectors.clip_outlines.push(c.clone());
                    self.vectors.clip_outlines.len() as u16
                });
        } else {
            self.current_clip_index = 0;
        }
        self.current_clip = clip;
    }
}

struct SceneBuilder<'a, T> {
    processor: T,
    state_stack: Vec<RenderState>,
    current_state: RenderState,
    scale_factor: ScaleFactor,
    window: &'a WindowInner,
    rotation: RotationInfo,
    shadow_mask_cache: &'a RefCell<ShadowMaskCache>,
    #[cfg(feature = "systemfonts")]
    text_layout_cache: &'a sharedparley::TextLayoutCache,
}

impl<'a, T: ProcessScene> SceneBuilder<'a, T> {
    fn new(
        screen_size: PhysicalSize,
        scale_factor: ScaleFactor,
        window: &'a WindowInner,
        processor: T,
        orientation: RenderingRotation,
        shadow_mask_cache: &'a RefCell<ShadowMaskCache>,
        #[cfg(feature = "systemfonts")] text_layout_cache: &'a sharedparley::TextLayoutCache,
    ) -> Self {
        Self {
            processor,
            state_stack: Vec::new(),
            current_state: RenderState {
                alpha: 1.,
                offset: LogicalPoint::default(),
                clip: LogicalRect::new(
                    LogicalPoint::default(),
                    (screen_size.cast() / scale_factor).cast(),
                ),
                clip_outlines: Vec::new(),
            },
            scale_factor,
            window,
            rotation: RotationInfo { orientation, screen_size },
            shadow_mask_cache,
            #[cfg(feature = "systemfonts")]
            text_layout_cache,
        }
    }

    fn should_draw(&self, rect: &LogicalRect) -> bool {
        !rect.size.is_empty()
            && self.current_state.alpha > 0.01
            && self.current_state.clip.intersects(rect)
    }

    /// Forwards the state's shape clip to the processor, converted to
    /// absolute physical screen coordinates (offset, scale and rotation
    /// applied).
    fn sync_clip_outline(&mut self) {
        let clip = if self.current_state.clip_outlines.is_empty() {
            None
        } else {
            let offset = self.current_state.offset.cast::<f32>() * self.scale_factor;
            let rotation = self.rotation;
            let layers = self
                .current_state
                .clip_outlines
                .iter()
                .map(|outline| {
                    let contours: Vec<shape_raster::Contour> = outline
                        .contours
                        .iter()
                        .map(|c| {
                            c.iter()
                                .map(|p| {
                                    let pt =
                                        p.cast::<f32>() * self.scale_factor + offset.to_vector();
                                    transform_continuous(pt, rotation)
                                })
                                .collect()
                        })
                        .collect();
                    let mut y_start = i32::MAX;
                    let mut y_end = i32::MIN;
                    for c in &contours {
                        for p in c {
                            y_start = y_start.min(p.y.floor() as i32);
                            y_end = y_end.max(p.y.ceil() as i32);
                        }
                    }
                    let mut rasterizer = shape_raster::Rasterizer::default();
                    rasterizer.begin(&contours);
                    scene::ClipOutlineLayer {
                        fill_rule: outline.fill_rule,
                        y_start,
                        y_end,
                        rasterizer: core::cell::RefCell::new(rasterizer),
                    }
                })
                .collect();
            Some(alloc::rc::Rc::new(ClipOutlineData::new(layers)))
        };
        self.processor.set_clip_outline(clip);
    }

    fn draw_image_impl(
        &mut self,
        image_inner: &ImageInner,
        i_slint_core::graphics::FitResult {
            clip_rect: source_rect,
            source_to_target_x,
            source_to_target_y,
            size: fit_size,
            offset: image_fit_offset,
            tiled,
        }: i_slint_core::graphics::FitResult,
        colorize: Color,
    ) {
        let global_alpha_u16 = (self.current_state.alpha * 255.) as u16;
        let offset =
            self.current_state.offset.cast() * self.scale_factor + image_fit_offset.to_vector();

        let physical_clip =
            (self.current_state.clip.translate(self.current_state.offset.to_vector()).cast()
                * self.scale_factor)
                .round()
                .cast()
                .transformed(self.rotation);

        match image_inner {
            ImageInner::None => (),
            ImageInner::StaticTextures(StaticTextures {
                data,
                textures,
                size,
                original_size,
                ..
            }) => {
                let adjust_x = size.width as f32 / original_size.width as f32;
                let adjust_y = size.height as f32 / original_size.height as f32;
                let source_to_target_x = source_to_target_x / adjust_x;
                let source_to_target_y = source_to_target_y / adjust_y;
                let source_rect =
                    source_rect.cast::<f32>().scale(adjust_x, adjust_y).round().to_box2d().cast();

                for t in textures.as_slice() {
                    let t_rect = t.rect.to_box2d();
                    // That's the source rect in the whole image coordinate
                    let Some(src_rect) = t_rect.intersection(&source_rect) else { continue };

                    let target_rect = if tiled.is_some() {
                        euclid::Rect::new(offset, fit_size).round().cast::<i32>()
                    } else {
                        // The slice maps onto the fit rect `offset ..= offset + fit_size`;
                        // this texture only covers `src_rect` of the slice's `source_rect`, so
                        // inset each edge by the uncovered source amount. Edges reaching the
                        // slice boundary keep the exact fit rect, so abutting slices share a
                        // seamless edge (scaling each from its source extent drifted apart).
                        let inset = |a: i32, b: i32, s2t: f32| (a - b) as f32 * s2t;
                        euclid::Box2D::<f32, PhysicalPx>::new(
                            euclid::point2(
                                offset.x
                                    + inset(src_rect.min.x, source_rect.min.x, source_to_target_x),
                                offset.y
                                    + inset(src_rect.min.y, source_rect.min.y, source_to_target_y),
                            ),
                            euclid::point2(
                                offset.x + fit_size.width
                                    - inset(source_rect.max.x, src_rect.max.x, source_to_target_x),
                                offset.y + fit_size.height
                                    - inset(source_rect.max.y, src_rect.max.y, source_to_target_y),
                            ),
                        )
                        .round()
                        .to_rect()
                        .cast::<i32>()
                    };
                    let target_rect = target_rect.transformed(self.rotation).round();

                    let Some(clipped_target) = physical_clip.intersection(&target_rect) else {
                        continue;
                    };

                    let pixel_stride = t.rect.width() as usize;
                    let core::ops::Range { start, end } = compute_range_in_buffer(
                        &PhysicalRect::from_untyped(
                            &src_rect.to_rect().translate(-t.rect.origin.to_vector()).cast(),
                        ),
                        pixel_stride,
                    );
                    let bpp = t.format.bpp();

                    let color = if colorize.alpha() > 0 { colorize } else { t.color };
                    let alpha = if colorize.alpha() > 0 || t.format == TexturePixelFormat::AlphaMap
                    {
                        color.alpha() as u16 * global_alpha_u16 / 255
                    } else {
                        global_alpha_u16
                    } as u8;

                    let tiling = tiled.map(|tile_o| {
                        let src_o = src_rect.min - source_rect.min;
                        let gap = (src_o) + (source_rect.max - src_rect.max);
                        target_pixel_buffer::TilingInfo {
                            offset_x: ((src_o.x as f32 - tile_o.x as f32) * source_to_target_x)
                                .round() as _,
                            offset_y: ((src_o.y as f32 - tile_o.y as f32) * source_to_target_y)
                                .round() as _,
                            scale_x: 1. / source_to_target_x,
                            scale_y: 1. / source_to_target_y,
                            gap_x: (gap.x as f32 * source_to_target_x).round() as _,
                            gap_y: (gap.y as f32 * source_to_target_y).round() as _,
                        }
                    });

                    let t = target_pixel_buffer::DrawTextureArgs {
                        data: target_pixel_buffer::TextureDataContainer::Static(
                            target_pixel_buffer::TextureData::new(
                                &data.as_slice()[t.index..][start * bpp..end * bpp],
                                t.format,
                                pixel_stride * bpp,
                                src_rect.size().cast(),
                            ),
                        ),
                        colorize: (color.alpha() > 0).then_some(color),
                        alpha,
                        dst_x: target_rect.origin.x as _,
                        dst_y: target_rect.origin.y as _,
                        dst_width: target_rect.size.width as _,
                        dst_height: target_rect.size.height as _,
                        rotation: self.rotation.orientation,
                        tiling,
                    };

                    self.processor.process_target_texture(&t, clipped_target.cast());
                }
            }

            ImageInner::NineSlice(..) => unreachable!(),
            _ => {
                let target_rect =
                    euclid::Rect::new(offset, fit_size).round().cast().transformed(self.rotation);
                let Some(clipped_target) = physical_clip.intersection(&target_rect) else {
                    return;
                };

                let orig = image_inner.size().cast::<f32>();
                let svg_target_size = if tiled.is_some() {
                    euclid::size2(orig.width * source_to_target_x, orig.height * source_to_target_y)
                        .round()
                        .cast()
                } else {
                    target_rect.size.cast()
                };
                if let Some(buffer) = image_inner.render_to_buffer(Some(svg_target_size)) {
                    let buf_size = buffer.size().cast::<f32>();

                    let alpha = if colorize.alpha() > 0 {
                        colorize.alpha() as u16 * global_alpha_u16 / 255
                    } else {
                        global_alpha_u16
                    } as u8;

                    let tiling = tiled.map(|tile_o| target_pixel_buffer::TilingInfo {
                        offset_x: (tile_o.x as f32 * -source_to_target_x).round() as _,
                        offset_y: (tile_o.y as f32 * -source_to_target_y).round() as _,
                        scale_x: 1. / source_to_target_x,
                        scale_y: 1. / source_to_target_y,
                        gap_x: 0,
                        gap_y: 0,
                    });

                    let t = target_pixel_buffer::DrawTextureArgs {
                        data: target_pixel_buffer::TextureDataContainer::Shared {
                            buffer: SharedBufferData::SharedImage(buffer),
                            source_rect: PhysicalRect::from_untyped(
                                &source_rect
                                    .cast::<f32>()
                                    .scale(
                                        buf_size.width / orig.width,
                                        buf_size.height / orig.height,
                                    )
                                    .round()
                                    .cast(),
                            ),
                        },
                        colorize: (colorize.alpha() > 0).then_some(colorize),
                        alpha,
                        dst_x: target_rect.origin.x as _,
                        dst_y: target_rect.origin.y as _,
                        dst_width: target_rect.size.width as _,
                        dst_height: target_rect.size.height as _,
                        rotation: self.rotation.orientation,
                        tiling,
                    };

                    self.processor.process_target_texture(&t, clipped_target.cast());
                } else {
                    unimplemented!("The image cannot be rendered")
                }
            }
        };
    }

    fn draw_text_paragraph<Font>(
        &mut self,
        paragraph: &TextParagraphLayout<'_, Font>,
        physical_clip: euclid::Rect<f32, PhysicalPx>,
        offset: euclid::Vector2D<f32, PhysicalPx>,
        color: Color,
        selection: Option<SelectionInfo>,
    ) where
        Font: AbstractFont
            + i_slint_core::textlayout::TextShaper<Length = PhysicalLength>
            + GlyphRenderer,
    {
        let slint_context = self.window.context();
        paragraph
            .layout_lines::<()>(
                |glyphs, line_x, line_y, _, sel| {
                    let baseline_y =
                        line_y + paragraph.layout.half_leading() + paragraph.layout.font.ascent();
                    if let (Some(sel), Some(selection)) = (sel, &selection) {
                        let (band_offset, band_height) = paragraph.layout.cursor_band();
                        let geometry = euclid::rect(
                            line_x.get() + sel.start.get(),
                            (line_y + band_offset).get(),
                            (sel.end - sel.start).get(),
                            band_height.get(),
                        );
                        if let Some(clipped_src) = geometry.intersection(&physical_clip.cast()) {
                            let geometry =
                                clipped_src.translate(offset.cast()).transformed(self.rotation);
                            let args = target_pixel_buffer::DrawRectangleArgs::from_rect(
                                geometry.cast(),
                                selection.selection_background.into(),
                            );
                            self.processor.process_rectangle(&args, geometry);
                        }
                    }
                    let scale_delta = paragraph.layout.font.scale_delta();
                    for positioned_glyph in glyphs {
                        let Some(glyph) = paragraph
                            .layout
                            .font
                            .render_glyph(positioned_glyph.glyph_id, slint_context)
                        else {
                            continue;
                        };

                        let gl_x = PhysicalLength::new((-glyph.x).truncate() as i16);
                        let gl_y = PhysicalLength::new(glyph.y.truncate() as i16);
                        let target_rect = PhysicalRect::new(
                            PhysicalPoint::from_lengths(
                                line_x + positioned_glyph.x - gl_x,
                                baseline_y - gl_y - glyph.height,
                            ),
                            glyph.size(),
                        )
                        .cast();

                        let color = match &selection {
                            Some(s) if s.selection.contains(&positioned_glyph.text_byte_offset) => {
                                s.selection_color
                            }
                            _ => color,
                        };

                        let Some(clipped_target) = physical_clip.intersection(&target_rect) else {
                            continue;
                        };

                        let data = match &glyph.alpha_map {
                            fonts::GlyphAlphaMap::Static(data) => {
                                if glyph.sdf {
                                    let geometry = clipped_target.translate(offset).round();
                                    let origin =
                                        (geometry.origin - offset.round()).round().cast::<i16>();
                                    let off_x = origin.x - target_rect.origin.x as i16;
                                    let off_y = origin.y - target_rect.origin.y as i16;
                                    let pixel_stride = glyph.pixel_stride;
                                    let mut geometry = geometry.cast();
                                    if geometry.size.width > glyph.width.get() - off_x {
                                        geometry.size.width = glyph.width.get() - off_x
                                    }
                                    if geometry.size.height > glyph.height.get() - off_y {
                                        geometry.size.height = glyph.height.get() - off_y
                                    }
                                    let source_size = geometry.size;
                                    if source_size.is_empty() {
                                        continue;
                                    }

                                    let delta32 = Fixed::<i32, 8>::from_fixed(scale_delta);
                                    let normalize = |x: Fixed<i32, 8>| {
                                        if x < Fixed::from_integer(0) {
                                            x + Fixed::from_integer(1)
                                        } else {
                                            x
                                        }
                                    };
                                    let fract_x = normalize(
                                        (-glyph.x) - Fixed::from_integer(gl_x.get() as _),
                                    );
                                    let off_x = delta32 * off_x as i32 + fract_x;
                                    let fract_y =
                                        normalize(glyph.y - Fixed::from_integer(gl_y.get() as _));
                                    let off_y = delta32 * off_y as i32 + fract_y;
                                    let texture = SceneTexture {
                                        data,
                                        pixel_stride,
                                        format: TexturePixelFormat::SignedDistanceField,
                                        extra: SceneTextureExtra {
                                            colorize: color,
                                            // color already is mixed with global alpha
                                            alpha: color.alpha(),
                                            rotation: self.rotation.orientation,
                                            dx: scale_delta,
                                            dy: scale_delta,
                                            off_x: Fixed::try_from_fixed(off_x).unwrap(),
                                            off_y: Fixed::try_from_fixed(off_y).unwrap(),
                                        },
                                    };
                                    self.processor.process_scene_texture(
                                        geometry.transformed(self.rotation),
                                        texture,
                                    );
                                    continue;
                                };

                                target_pixel_buffer::TextureDataContainer::Static(
                                    target_pixel_buffer::TextureData::new(
                                        data,
                                        TexturePixelFormat::AlphaMap,
                                        glyph.pixel_stride as usize,
                                        euclid::size2(glyph.width.get(), glyph.height.get()).cast(),
                                    ),
                                )
                            }
                            fonts::GlyphAlphaMap::Shared(data) => {
                                let source_rect = euclid::rect(0, 0, glyph.width.0, glyph.height.0);
                                target_pixel_buffer::TextureDataContainer::Shared {
                                    buffer: SharedBufferData::AlphaMap {
                                        data: data.clone(),
                                        width: glyph.pixel_stride,
                                    },
                                    source_rect,
                                }
                            }
                        };
                        let clipped_target =
                            clipped_target.translate(offset).round().transformed(self.rotation);
                        let target_rect =
                            target_rect.translate(offset).round().transformed(self.rotation);
                        let t = target_pixel_buffer::DrawTextureArgs {
                            data,
                            colorize: Some(color),
                            // color already is mixed with global alpha
                            alpha: color.alpha(),
                            dst_x: target_rect.origin.x as _,
                            dst_y: target_rect.origin.y as _,
                            dst_width: target_rect.size.width as _,
                            dst_height: target_rect.size.height as _,
                            rotation: self.rotation.orientation,
                            tiling: None,
                        };

                        self.processor.process_target_texture(&t, clipped_target.cast());
                    }
                    core::ops::ControlFlow::Continue(())
                },
                selection.as_ref().map(|s| s.selection.clone()),
            )
            .ok();
    }

    /// Returns the color, mixed with the current_state's alpha
    fn alpha_color(&self, color: Color) -> Color {
        if self.current_state.alpha < 1.0 {
            Color::from_argb_u8(
                (color.alpha() as f32 * self.current_state.alpha) as u8,
                color.red(),
                color.green(),
                color.blue(),
            )
        } else {
            color
        }
    }
}

fn alpha_color(color: Color, alpha: u8) -> Color {
    if alpha < 255 {
        Color::from_argb_u8(
            ((color.alpha() as u32 * alpha as u32) / 255) as u8,
            color.red(),
            color.green(),
            color.blue(),
        )
    } else {
        color
    }
}

struct SelectionInfo {
    selection_color: Color,
    selection_background: Color,
    selection: core::ops::Range<usize>,
}

#[derive(Clone)]
struct RenderState {
    alpha: f32,
    offset: LogicalPoint,
    clip: LogicalRect,
    /// The stack of nested shape clips active on top of `clip`, outermost
    /// first: contours in this state's logical coordinate space, translated
    /// along `offset` like `clip`. Their coverages intersect.
    clip_outlines: Vec<alloc::rc::Rc<LogicalClipOutline>>,
}

/// A clip outline in logical coordinates; converted to physical screen
/// coordinates when the processor is told about it.
struct LogicalClipOutline {
    /// The closed polylines making up the clip.
    contours: Vec<Vec<euclid::Point2D<f32, LogicalPx>>>,
    /// How the contours combine into coverage.
    fill_rule: i_slint_core::items::FillRule,
}

impl<T: ProcessScene> i_slint_core::item_rendering::ItemRenderer for SceneBuilder<'_, T> {
    fn global_alpha_transparent(&self) -> bool {
        self.current_state.alpha == 0.0
    }

    fn draw_rectangle(
        &mut self,
        rect: Pin<&dyn RenderRectangle>,
        _: &ItemRc,
        size: LogicalSize,
        _cache: &CachedRenderingData,
    ) {
        let geom = LogicalRect::from(size);
        if self.should_draw(&geom) {
            let geom = (geom.translate(self.current_state.offset.to_vector()).cast()
                * self.scale_factor)
                .transformed(self.rotation);

            let clipped =
                (self.current_state.clip.translate(self.current_state.offset.to_vector()).cast()
                    * self.scale_factor)
                    .round()
                    .cast()
                    .transformed(self.rotation);

            let mut args =
                target_pixel_buffer::DrawRectangleArgs::from_rect(geom, rect.background());
            args.alpha = (self.current_state.alpha * 255.) as u8;
            args.rotation = self.rotation.orientation;
            self.processor.process_rectangle(&args, clipped);
        }
    }

    fn draw_border_rectangle(
        &mut self,
        rect: Pin<&dyn RenderBorderRectangle>,
        _: &ItemRc,
        size: LogicalSize,
        _: &CachedRenderingData,
    ) {
        let geom = LogicalRect::from(size);
        if self.should_draw(&geom) {
            let geom = (geom.translate(self.current_state.offset.to_vector()).cast()
                * self.scale_factor)
                .transformed(self.rotation);

            let clipped =
                (self.current_state.clip.translate(self.current_state.offset.to_vector()).cast()
                    * self.scale_factor)
                    .round()
                    .cast()
                    .transformed(self.rotation);

            if rect.outline().shape().is_some() {
                let logical_geom = LogicalRect::from(size)
                    .translate(self.current_state.offset.to_vector())
                    .cast::<f32>();
                let scale_factor = self.scale_factor;
                let rotation = self.rotation;
                let unrotated: Vec<shape_raster::Contour> = rect
                    .outline()
                    .flatten(logical_geom, shape_raster::FLATTEN_TOLERANCE / scale_factor.get())
                    .into_iter()
                    .map(|c| c.into_iter().map(|p| p * scale_factor).collect())
                    .collect();
                let Some(brush_bounds) = contours_bounds(&unrotated) else { return };
                let contours: Vec<shape_raster::Contour> = unrotated
                    .into_iter()
                    .map(|c| c.into_iter().map(|p| transform_continuous(p, rotation)).collect())
                    .collect();
                let Some(bounds) = contours_bounds(&contours) else { return };
                let Some(clipped_geom) = bounds.round_out().cast().intersection(&clipped) else {
                    return;
                };
                let alpha = self.current_state.alpha;
                let contours = alloc::rc::Rc::new(contours);
                if let Some(brush) = path_brush(&rect.background(), alpha, brush_bounds) {
                    self.processor.process_path(
                        alloc::rc::Rc::new(PathCommandData {
                            contours: contours.clone(),
                            fill_rule: rect.outline().fill_rule(),
                            brush,
                            bounds,
                            brush_bounds,
                            rotation,
                        }),
                        clipped_geom,
                    );
                }
                let border_color: PremultipliedRgbaColor =
                    self.alpha_color(rect.border_color().color()).into();
                let border = rect.border_width().cast() * self.scale_factor;
                if border.get() > 0.01 && border_color.alpha > 0 {
                    let stroke_contours = shape_raster::stroke_to_fill(
                        &contours,
                        border.get(),
                        i_slint_core::items::LineCap::Butt,
                        i_slint_core::items::LineJoin::Miter,
                        4.,
                    );
                    if let Some(stroke_bounds) = contours_bounds(&stroke_contours)
                        && let Some(stroke_clip) =
                            stroke_bounds.round_out().cast().intersection(&clipped)
                    {
                        self.processor.process_path(
                            alloc::rc::Rc::new(PathCommandData {
                                contours: alloc::rc::Rc::new(stroke_contours),
                                fill_rule: FillRule::Nonzero,
                                brush: PathBrush::Solid(border_color),
                                bounds: stroke_bounds,
                                brush_bounds,
                                rotation,
                            }),
                            stroke_clip,
                        );
                    }
                }
                return;
            }

            let radius = (rect.border_radius().cast() * self.scale_factor)
                .transformed(self.rotation)
                .min(BorderRadius::from_length(geom.width_length() / 2.))
                .min(BorderRadius::from_length(geom.height_length() / 2.));

            let border = rect.border_width().cast() * self.scale_factor;
            let border_color =
                if border.get() > 0.01 { rect.border_color() } else { Default::default() };

            let args = target_pixel_buffer::DrawRectangleArgs {
                x: geom.origin.x,
                y: geom.origin.y,
                width: geom.size.width,
                height: geom.size.height,
                top_left_radius: radius.top_left,
                top_right_radius: radius.top_right,
                bottom_right_radius: radius.bottom_right,
                bottom_left_radius: radius.bottom_left,
                border_width: border.get(),
                background: rect.background(),
                border: border_color,
                alpha: (self.current_state.alpha * 255.) as u8,
                rotation: self.rotation.orientation,
            };

            self.processor.process_rectangle(&args, clipped);
        }
    }

    fn draw_window_background(
        &mut self,
        rect: Pin<&dyn RenderRectangle>,
        _self_rc: &ItemRc,
        _size: LogicalSize,
        _cache: &CachedRenderingData,
    ) {
        // register a dependency for the partial renderer's dirty tracker. The actual rendering is done earlier in the software renderer.
        let _ = rect.background();
    }

    fn draw_image(
        &mut self,
        image: Pin<&dyn RenderImage>,
        _: &ItemRc,
        size: LogicalSize,
        _: &CachedRenderingData,
    ) {
        let geom = LogicalRect::from(size);
        if self.should_draw(&geom) {
            let source = image.source();

            let image_inner: &ImageInner = (&source).into();
            if let ImageInner::NineSlice(nine) = image_inner {
                let colorize = image.colorize().color();
                let source_size = source.size();
                for fit in i_slint_core::graphics::fit9slice(
                    source_size,
                    nine.1,
                    size.cast() * self.scale_factor,
                    self.scale_factor,
                    image.alignment(),
                    image.tiling(),
                ) {
                    self.draw_image_impl(&nine.0, fit, colorize);
                }
                return;
            }

            let source_clip = image.source_clip().map_or_else(
                || euclid::Rect::new(Default::default(), source.size().cast()),
                |clip| {
                    clip.intersection(&euclid::Rect::from_size(source.size().cast()))
                        .unwrap_or_default()
                },
            );

            let phys_size = geom.size_length().cast() * self.scale_factor;
            let fit = i_slint_core::graphics::fit(
                image.image_fit(),
                phys_size,
                source_clip,
                self.scale_factor,
                image.alignment(),
                image.tiling(),
            );
            self.draw_image_impl(image_inner, fit, image.colorize().color());
        }
    }

    fn draw_text(
        &mut self,
        text: Pin<&dyn i_slint_core::item_rendering::RenderText>,
        self_rc: &ItemRc,
        size: LogicalSize,
        _cache: &CachedRenderingData,
    ) {
        let font_request = text.font_request(self_rc);

        #[cfg(feature = "systemfonts")]
        let mut font_ctx = self.window.context().font_context().borrow_mut();
        let font = fonts::match_font(
            &font_request,
            self.scale_factor,
            #[cfg(feature = "systemfonts")]
            &mut font_ctx,
        );

        #[cfg(feature = "systemfonts")]
        if uses_parley(&font) {
            drop(font_ctx);
            sharedparley::draw_text(self, text, Some(self_rc), size, Some(self.text_layout_cache));
            return;
        }

        let content = text.text();
        let string = match &content {
            PlainOrStyledText::Plain(string) => alloc::borrow::Cow::Borrowed(string.as_str()),
            PlainOrStyledText::Styled(styled_text) => {
                i_slint_core::styled_text::get_raw_text(styled_text)
            }
        };

        if string.trim().is_empty() {
            return;
        }

        let geom = LogicalRect::from(size);
        if !self.should_draw(&geom) {
            return;
        }

        let color = self.alpha_color(text.color().color());
        let max_size = (geom.size.cast() * self.scale_factor).cast();

        // Clip glyphs not only against the global clip but also against the Text's geometry to avoid drawing outside
        // of its boundaries (that breaks partial rendering and the cast to usize for the item relative coordinate below).
        // FIXME: we should allow drawing outside of the Text element's boundaries.
        let physical_clip = if let Some(logical_clip) = self.current_state.clip.intersection(&geom)
        {
            logical_clip.cast() * self.scale_factor
        } else {
            return; // This should have been caught earlier already
        };
        let offset = self.current_state.offset.to_vector().cast() * self.scale_factor;

        let (horizontal_alignment, vertical_alignment) = text.alignment();
        let max_lines = text.line_limit();

        with_font!(&font, |font| {
            let layout = fonts::text_layout_for_font(font, &font_request, self.scale_factor);
            let paragraph = TextParagraphLayout {
                string: &string,
                layout,
                max_width: max_size.width_length(),
                max_height: max_size.height_length(),
                horizontal_alignment,
                vertical_alignment,
                wrap: text.wrap(),
                overflow: text.overflow(),
                single_line: false,
                max_lines,
            };

            self.draw_text_paragraph(&paragraph, physical_clip, offset, color, None);
        });
    }

    fn draw_text_input(
        &mut self,
        text_input: Pin<&i_slint_core::items::TextInput>,
        self_rc: &ItemRc,
        size: LogicalSize,
    ) {
        let font_request = text_input.font_request(self_rc);
        #[cfg(feature = "systemfonts")]
        let mut font_ctx = self.window.context().font_context().borrow_mut();
        let font = fonts::match_font(
            &font_request,
            self.scale_factor,
            #[cfg(feature = "systemfonts")]
            &mut font_ctx,
        );

        #[cfg(feature = "systemfonts")]
        if uses_parley(&font) {
            drop(font_ctx);
            sharedparley::draw_text_input(self, text_input, self_rc, size, self.text_layout_cache);
            return;
        }

        let geom = LogicalRect::from(size);
        if !self.should_draw(&geom) {
            return;
        }

        let max_size = (geom.size.cast() * self.scale_factor).cast();

        // Clip glyphs not only against the global clip but also against the Text's geometry to avoid drawing outside
        // of its boundaries (that breaks partial rendering and the cast to usize for the item relative coordinate below).
        // FIXME: we should allow drawing outside of the Text element's boundaries.
        let physical_clip = if let Some(logical_clip) = self.current_state.clip.intersection(&geom)
        {
            logical_clip.cast() * self.scale_factor
        } else {
            return; // This should have been caught earlier already
        };
        let offset = self.current_state.offset.to_vector().cast() * self.scale_factor;

        let text_visual_representation = text_input.visual_representation();
        let color = self.alpha_color(text_visual_representation.text_color.color());

        let selection =
            (!text_visual_representation.selection_range.is_empty()).then_some(SelectionInfo {
                selection_background: self.alpha_color(text_input.selection_background_color()),
                selection_color: self.alpha_color(text_input.selection_foreground_color()),
                selection: text_visual_representation.selection_range.clone(),
            });

        let cursor_pos_and_height = with_font!(&font, |font| {
            let paragraph = TextParagraphLayout {
                string: &text_visual_representation.text,
                layout: fonts::text_layout_for_font(font, &font_request, self.scale_factor),
                max_width: max_size.width_length(),
                max_height: max_size.height_length(),
                horizontal_alignment: text_input.horizontal_alignment(),
                vertical_alignment: text_input.vertical_alignment(),
                wrap: text_input.wrap(),
                overflow: TextOverflow::Clip,
                single_line: text_input.single_line(),
                max_lines: None,
            };

            self.draw_text_paragraph(&paragraph, physical_clip, offset, color, selection);

            text_visual_representation.cursor_position.map(|cursor_offset| {
                let (band_offset, band_height) = paragraph.layout.cursor_band();
                let (cursor_x, cursor_y) = paragraph.cursor_pos_for_byte_offset(cursor_offset);
                ((cursor_x, cursor_y + band_offset), band_height)
            })
        });

        // Nothing below depends on the concrete font, so keep it out of the monomorphized body.
        if let Some(((cursor_x, cursor_y), cursor_height)) = cursor_pos_and_height {
            let cursor_rect = PhysicalRect::new(
                PhysicalPoint::from_lengths(cursor_x, cursor_y),
                PhysicalSize::from_lengths(
                    (text_input.text_cursor_width().cast() * self.scale_factor).cast(),
                    cursor_height,
                ),
            );

            if let Some(clipped_src) = cursor_rect.intersection(&physical_clip.cast()) {
                let geometry = clipped_src.translate(offset.cast()).transformed(self.rotation);
                let args = target_pixel_buffer::DrawRectangleArgs::from_rect(
                    geometry.cast(),
                    self.alpha_color(text_visual_representation.cursor_color).into(),
                );
                self.processor.process_rectangle(&args, geometry);
            }
        }
    }

    #[cfg(all(feature = "std", not(feature = "path")))]
    fn draw_path(
        &mut self,
        _path: Pin<&i_slint_core::items::Path>,
        _self_rc: &ItemRc,
        _size: LogicalSize,
    ) {
        // Path rendering is disabled without the path feature
    }

    #[cfg(feature = "path")]
    #[allow(clippy::unnecessary_cast)] // Coord
    fn draw_path(
        &mut self,
        path: Pin<&i_slint_core::items::Path>,
        self_rc: &ItemRc,
        size: LogicalSize,
    ) {
        let geom = LogicalRect::from(size);
        if !self.should_draw(&geom) {
            return;
        }

        // Get the fitted path events from the Path item
        let Some((offset, path_iterator)) = path.fitted_path_events(self_rc) else {
            return;
        };

        let state_offset = self.current_state.offset;
        let scale_factor = self.scale_factor;
        let rotation = self.rotation;
        let unrotated = shape_raster::flatten_events(
            path_iterator.iter(),
            |p| {
                euclid::point2::<f32, LogicalPx>(
                    p.x as f32 + offset.x as f32 + state_offset.x as f32,
                    p.y as f32 + offset.y as f32 + state_offset.y as f32,
                ) * scale_factor
            },
            shape_raster::FLATTEN_TOLERANCE / scale_factor.get(),
        );
        let Some(brush_bounds) = contours_bounds(&unrotated) else { return };
        let contours: Vec<shape_raster::Contour> = unrotated
            .into_iter()
            .map(|c| c.into_iter().map(|p| transform_continuous(p, rotation)).collect())
            .collect();
        let Some(bounds) = contours_bounds(&contours) else { return };

        let physical_clip =
            (self.current_state.clip.translate(self.current_state.offset.to_vector()).cast()
                * self.scale_factor)
                .round()
                .cast::<i16>()
                .transformed(self.rotation);

        let Some(clipped_geom) = bounds.round_out().cast().intersection(&physical_clip) else {
            return;
        };

        let alpha = self.current_state.alpha;
        let contours = alloc::rc::Rc::new(contours);

        // Fill
        if let Some(brush) = path_brush(&path.fill(), alpha, brush_bounds) {
            self.processor.process_path(
                alloc::rc::Rc::new(PathCommandData {
                    contours: contours.clone(),
                    fill_rule: path.effective_fill_rule(),
                    brush,
                    bounds,
                    brush_bounds,
                    rotation,
                }),
                clipped_geom,
            );
        }

        // Stroke: outline the path, then fill the outline like a fill.
        let stroke_width = path.stroke_width().get() as f32 * scale_factor.get();
        let stroke_color: PremultipliedRgbaColor = self.alpha_color(path.stroke().color()).into();
        if stroke_width > 0.01 && stroke_color.alpha > 0 {
            let stroke_contours = shape_raster::stroke_to_fill(
                &contours,
                stroke_width,
                path.stroke_line_cap(),
                path.stroke_line_join(),
                path.stroke_miter_limit(),
            );
            if let Some(stroke_bounds) = contours_bounds(&stroke_contours)
                && let Some(stroke_clip) =
                    stroke_bounds.round_out().cast().intersection(&physical_clip)
            {
                self.processor.process_path(
                    alloc::rc::Rc::new(PathCommandData {
                        contours: alloc::rc::Rc::new(stroke_contours),
                        fill_rule: FillRule::Nonzero,
                        brush: PathBrush::Solid(stroke_color),
                        bounds: stroke_bounds,
                        brush_bounds,
                        rotation,
                    }),
                    stroke_clip,
                );
            }
        }
    }

    #[allow(clippy::unnecessary_cast)] // Coord
    fn draw_box_shadow(
        &mut self,
        box_shadow: Pin<&i_slint_core::items::BoxShadow>,
        _self_rc: &ItemRc,
        size: LogicalSize,
    ) {
        let geom = LogicalRect::from(size);
        if !self.should_draw(&geom) {
            return;
        }
        let color = self.alpha_color(box_shadow.color());
        if color.alpha() == 0 {
            return;
        }

        let outline = box_shadow.element_outline();
        let logical_geom = geom.translate(self.current_state.offset.to_vector()).cast::<f32>();
        let scale_factor = self.scale_factor;
        let rotation = self.rotation;
        let mut contours: Vec<shape_raster::Contour> = outline
            .flatten(logical_geom, shape_raster::FLATTEN_TOLERANCE / scale_factor.get())
            .into_iter()
            .map(|c| {
                c.into_iter().map(|p| transform_continuous(p * scale_factor, rotation)).collect()
            })
            .collect();
        if contours.is_empty() {
            return;
        }

        let spread = box_shadow.spread().get() as f32 * scale_factor.get();
        let sigma = box_shadow.blur().get() as f32 * scale_factor.get() / 2.;
        let margin = (3. * sigma).ceil() as i32 + 1;

        // The shadow offset is applied in physical space so its direction
        // rotates with the element.
        let shifted = |contours: &mut Vec<shape_raster::Contour>| {
            let zero = transform_continuous(euclid::point2::<f32, PhysicalPx>(0., 0.), rotation);
            let off = transform_continuous(
                euclid::point2::<f32, PhysicalPx>(
                    box_shadow.offset_x().get() as f32 * scale_factor.get(),
                    box_shadow.offset_y().get() as f32 * scale_factor.get(),
                ),
                rotation,
            ) - zero.to_vector();
            for c in contours.iter_mut() {
                for p in c.iter_mut() {
                    p.x += off.x;
                    p.y += off.y;
                }
            }
        };
        let padded_bounds = |contours: &[shape_raster::Contour], pad: f32| {
            let b = contours_bounds(contours).unwrap();
            (
                euclid::point2((b.min_x() - pad).floor() as i32, (b.min_y() - pad).floor() as i32),
                euclid::size2(
                    (b.max_x() + pad).ceil() as i32 - (b.min_x() - pad).floor() as i32,
                    (b.max_y() + pad).ceil() as i32 - (b.min_y() - pad).floor() as i32,
                ),
            )
        };

        let inset = box_shadow.inset();
        let hole_contours = if inset {
            // The inner "hole" is the outline translated by the inset offset:
            // the shadow ring is what the hole doesn't cover.
            let mut h = contours.clone();
            shifted(&mut h);
            h
        } else {
            Vec::new()
        };
        if !inset {
            shifted(&mut contours);
        }
        let off_extent = if inset {
            (box_shadow.offset_x().get().abs() as f32 + box_shadow.offset_y().get().abs() as f32)
                * scale_factor.get()
        } else {
            0.
        };
        let (mask_origin, mask_size) =
            padded_bounds(&contours, spread.abs() + margin as f32 + off_extent);

        // Everything that feeds the blurred mask: the transformed contours
        // (which carry outline content, geometry, offset and rotation), the
        // spread and sigma, and the mask's own placement.
        let mask_key = {
            let mut h = 14695981039346656037u64;
            let mut mix = |bits: u64| {
                h ^= bits;
                h = h.wrapping_mul(0x100000001b3);
            };
            for p in contours.iter().flatten() {
                mix(p.x.to_bits() as u64);
                mix(p.y.to_bits() as u64);
            }
            for p in hole_contours.iter().flatten() {
                mix(p.x.to_bits() as u64);
                mix(p.y.to_bits() as u64);
            }
            mix(spread.to_bits() as u64);
            mix(sigma.to_bits() as u64);
            mix(inset as u64);
            mix(outline.fill_rule() as u64);
            mix(mask_origin.x as u64);
            mix(mask_origin.y as u64);
            mix(mask_size.width as u64);
            mix(mask_size.height as u64);
            h
        };
        let blurred_rc = self.shadow_mask_cache.borrow().get(mask_key);
        let blurred = match blurred_rc {
            Some(mask) => mask,
            None => {
                let coverage = shape_raster::rasterize_spread_mask(
                    &contours,
                    if inset { 0. } else { spread },
                    mask_origin,
                    mask_size,
                    outline.fill_rule(),
                );
                let mut blurred = alloc::vec![0; coverage.len()];
                let (w, h) = (mask_size.width.max(0) as usize, mask_size.height.max(0) as usize);
                if inset {
                    // Ring coverage = 1 - hole, where the hole is the outline
                    // moved by the inset offset and eroded by the spread (a
                    // positive spread shrinks the hole and so thickens the
                    // shadow band). Blurring the ring and clipping to the
                    // element coverage keeps the shadow on the inside edge.
                    let hole = shape_raster::rasterize_spread_mask(
                        &hole_contours,
                        -spread,
                        mask_origin,
                        mask_size,
                        outline.fill_rule(),
                    );
                    let mut ring = alloc::vec![0u8; hole.len()];
                    for (r, h) in ring.iter_mut().zip(hole.iter()) {
                        *r = 255 - *h;
                    }
                    shape_raster::gaussian_blur(&ring, &mut blurred, w, h, sigma);
                    for (b, c) in blurred.iter_mut().zip(coverage.iter()) {
                        *b = (*b as u16 * *c as u16 / 255) as u8;
                    }
                } else {
                    shape_raster::gaussian_blur(&coverage, &mut blurred, w, h, sigma);
                }
                let blurred: Rc<[u8]> = Rc::from(blurred.as_slice());
                self.shadow_mask_cache.borrow_mut().insert(mask_key, blurred.clone());
                blurred
            }
        };

        let width = mask_size.width.max(0) as u32;
        let height = mask_size.height.max(0) as u32;
        if width == 0 || height == 0 {
            return;
        }
        // The mask was rasterized in post-rotation screen space: draw it
        // with no further rotation.
        let args = target_pixel_buffer::DrawTextureArgs {
            data: target_pixel_buffer::TextureDataContainer::Shared {
                buffer: SharedBufferData::AlphaMap { data: blurred, width: width as _ },
                source_rect: euclid::rect(0, 0, width as i16, height as i16),
            },
            colorize: Some(color),
            alpha: color.alpha(),
            dst_x: mask_origin.x as _,
            dst_y: mask_origin.y as _,
            dst_width: width as _,
            dst_height: height as _,
            rotation: RenderingRotation::NoRotation,
            tiling: None,
        };
        let clipped =
            (self.current_state.clip.translate(self.current_state.offset.to_vector()).cast()
                * self.scale_factor)
                .round()
                .cast::<i16>()
                .transformed(self.rotation);
        self.processor.process_target_texture(&args, clipped.cast());
    }

    /// The elevation shadow of an element: the Android ambient + spot model
    /// (i_slint_core::graphics::shadow). Both layers are tessellated into
    /// A8 masks bounded by the shadow's extent — never a full-window mask —
    /// and drawn through the shared texture path, so this works identically
    /// in whole-scene and line-by-line mode.
    fn draw_elevation_shadow(
        &mut self,
        shadow_item: Pin<&i_slint_core::items::ElevationShadow>,
        _self_rc: &ItemRc,
        size: LogicalSize,
    ) {
        let geom = LogicalRect::from(size);
        if !self.should_draw(&geom) {
            return;
        }
        let scale_factor = self.scale_factor.get();
        let z = shadow_item.elevation().get() * scale_factor;
        if z < shadow::MIN_HEIGHT {
            return;
        }
        let caster_alpha = shadow_item.caster_alpha();
        let ambient_color = self.alpha_color(shadow::effective_ambient_color(
            shadow_item.ambient_shadow_color(),
            caster_alpha,
        ));
        let spot_color = self.alpha_color(shadow::effective_spot_color(
            shadow_item.spot_shadow_color(),
            caster_alpha,
        ));
        if ambient_color.alpha() == 0 && spot_color.alpha() == 0 {
            return;
        }
        let outline = shadow_item.element_outline();

        // Map item space to post-rotation physical screen space: offset in
        // logical coordinates, then the scale factor, then the screen
        // rotation (a physical-space affine built from the rotation flags,
        // matching `transform_continuous`).
        let offset = self.current_state.offset.cast::<f32>();
        let st = shadow::Affine::scale_translate(
            scale_factor,
            scale_factor,
            offset.x * scale_factor,
            offset.y * scale_factor,
        );
        let screen = screen_space_affine(self.rotation);
        let ctm = screen.pre_concat(&st);

        let adapter = self.window.window_adapter();
        let (light, light_radius) =
            shadow::elevation_light(adapter.display_geometry(), adapter.size());
        let lp = screen.map_point(euclid::point2(light[0], light[1]));
        let light = [lp.x, lp.y, light[2]];

        let masks = shadow::elevation_shadow_masks(
            &outline,
            geom.cast::<f32>(),
            &ctm,
            z,
            light,
            light_radius,
            caster_alpha < 1.,
        );

        let clipped =
            (self.current_state.clip.translate(self.current_state.offset.to_vector()).cast()
                * self.scale_factor)
                .round()
                .cast::<i16>()
                .transformed(self.rotation);
        for (layer, color) in [(masks.ambient, ambient_color), (masks.spot, spot_color)].into_iter()
        {
            let Some(layer) = layer else { continue };
            if color.alpha() == 0 {
                continue;
            }
            let (width, height) = (layer.size.width, layer.size.height);
            // The mask was rasterized in post-rotation screen space: draw it
            // with no further rotation.
            let args = target_pixel_buffer::DrawTextureArgs {
                data: target_pixel_buffer::TextureDataContainer::Shared {
                    buffer: SharedBufferData::AlphaMap { data: layer.mask, width: width as u16 },
                    source_rect: euclid::rect(0, 0, width as i16, height as i16),
                },
                colorize: Some(color),
                alpha: color.alpha(),
                dst_x: layer.rect.origin.x.round() as isize,
                dst_y: layer.rect.origin.y.round() as isize,
                dst_width: width as usize,
                dst_height: height as usize,
                rotation: RenderingRotation::NoRotation,
                tiling: None,
            };
            self.processor.process_target_texture(&args, clipped.cast());
        }
    }

    fn combine_clip(
        &mut self,
        other: LogicalRect,
        outline: &i_slint_core::graphics::ElementOutline,
    ) -> bool {
        // A rectangular clip leaves the stacked shape clips in place: both
        // constraints apply (the rectangle lands on `clip` below). A shape
        // clip pushes onto the stack so nested clips intersect.
        if !outline.is_plain_rect() {
            // Keep the flattened outline in this state's logical space; it
            // is moved along with `clip` by `translate` and converted to
            // physical coordinates when the processor is told about it.
            self.current_state.clip_outlines.push(alloc::rc::Rc::new(LogicalClipOutline {
                contours: outline.flatten(other.to_f32(), shape_raster::FLATTEN_TOLERANCE),
                fill_rule: outline.fill_rule(),
            }));
            self.sync_clip_outline();
        }
        match self.current_state.clip.intersection(&other) {
            Some(r) => {
                self.current_state.clip = r;
                true
            }
            None => {
                self.current_state.clip = LogicalRect::default();
                false
            }
        }
    }

    fn get_current_clip(&self) -> LogicalRect {
        self.current_state.clip
    }

    #[allow(clippy::unnecessary_cast)] // Coord
    fn translate(&mut self, distance: LogicalVector) {
        self.current_state.offset += distance;
        self.current_state.clip = self.current_state.clip.translate(-distance);
        // The clip outline keeps its absolute position: shift it back the
        // same way `clip` is shifted.
        if !self.current_state.clip_outlines.is_empty() {
            self.current_state.clip_outlines = self
                .current_state
                .clip_outlines
                .iter()
                .map(|outline| {
                    let contours = outline
                        .contours
                        .iter()
                        .map(|c| {
                            c.iter()
                                .map(|p| {
                                    euclid::point2(p.x - distance.x as f32, p.y - distance.y as f32)
                                })
                                .collect()
                        })
                        .collect();
                    alloc::rc::Rc::new(LogicalClipOutline {
                        contours,
                        fill_rule: outline.fill_rule,
                    })
                })
                .collect();
            self.sync_clip_outline();
        }
    }

    fn current_transform(&self) -> i_slint_core::lengths::ItemTransform {
        let v = self.current_state.offset.to_vector().cast::<f32>();
        i_slint_core::lengths::ItemTransform::translation(v.x, v.y)
    }

    fn rotate(&mut self, _angle_in_degrees: f32) {
        // TODO (#6068)
    }

    fn scale(&mut self, _x_factor: f32, _y_factor: f32) {
        // TODO
    }

    fn apply_opacity(&mut self, opacity: f32) {
        self.current_state.alpha *= opacity;
    }

    fn save_state(&mut self) {
        self.state_stack.push(self.current_state.clone());
    }

    fn restore_state(&mut self) {
        self.current_state = self.state_stack.pop().unwrap();
        self.sync_clip_outline();
    }

    fn scale_factor(&self) -> ScaleFactor {
        self.scale_factor
    }

    fn draw_cached_pixmap(
        &mut self,
        _item: &ItemRc,
        update_fn: &dyn Fn(&mut dyn FnMut(u32, u32, &[u8])),
    ) {
        // FIXME: actually cache the pixmap
        update_fn(&mut |width, height, data| {
            let img = SharedImageBuffer::RGBA8Premultiplied(SharedPixelBuffer::clone_from_slice(
                data, width, height,
            ));

            let physical_clip = (self.current_state.clip.cast() * self.scale_factor).cast();
            let source_rect = euclid::rect(0, 0, width as _, height as _);

            if let Some(clipped_src) = source_rect.intersection(&physical_clip) {
                let offset = self.current_state.offset.cast() * self.scale_factor;
                let geometry = clipped_src.translate(offset.to_vector().cast()).round_in();

                let t = target_pixel_buffer::DrawTextureArgs {
                    data: target_pixel_buffer::TextureDataContainer::Shared {
                        buffer: SharedBufferData::SharedImage(img),
                        source_rect,
                    },
                    colorize: None,
                    alpha: (self.current_state.alpha * 255.) as u8,
                    dst_x: offset.x as _,
                    dst_y: offset.y as _,
                    dst_width: width as _,
                    dst_height: height as _,
                    rotation: self.rotation.orientation,
                    tiling: None,
                };
                self.processor
                    .process_target_texture(&t, geometry.cast().transformed(self.rotation));
            }
        });
    }

    fn draw_string(&mut self, string: &str, color: Color) {
        let font_request = Default::default();
        #[cfg(feature = "systemfonts")]
        let mut font_ctx = self.window.context().font_context().borrow_mut();
        let font = fonts::match_font(
            &font_request,
            self.scale_factor,
            #[cfg(feature = "systemfonts")]
            &mut font_ctx,
        );

        #[cfg(feature = "systemfonts")]
        if uses_parley(&font) {
            drop(font_ctx);
            sharedparley::draw_text(
                self,
                std::pin::pin!((i_slint_core::SharedString::from(string), Brush::from(color))),
                None,
                self.current_state.clip.size.cast(),
                None,
            );
            return;
        }

        let clip = self.current_state.clip.cast() * self.scale_factor;

        with_font!(&font, |font| {
            let layout = fonts::text_layout_for_font(font, &font_request, self.scale_factor);

            let paragraph = TextParagraphLayout {
                string,
                layout,
                max_width: clip.width_length().cast(),
                max_height: clip.height_length().cast(),
                horizontal_alignment: Default::default(),
                vertical_alignment: Default::default(),
                wrap: Default::default(),
                overflow: Default::default(),
                single_line: false,
                max_lines: None,
            };

            self.draw_text_paragraph(&paragraph, clip, Default::default(), color, None);
        });
    }

    fn draw_image_direct(&mut self, image: i_slint_core::graphics::Image) {
        let image_inner: &ImageInner = (&image).into();
        let source_size = image.size();
        if source_size.is_empty() {
            return;
        }
        let target_size = euclid::Size2D::<f32, i_slint_core::lengths::LogicalPx>::from_untyped(
            source_size.cast(),
        ) * self.scale_factor;
        let fit = i_slint_core::graphics::fit(
            i_slint_core::items::ImageFit::Fill,
            target_size,
            i_slint_core::graphics::IntRect::from_size(source_size.cast()),
            self.scale_factor,
            Default::default(),
            Default::default(),
        );
        self.draw_image_impl(image_inner, fit, i_slint_core::Color::default());
    }

    fn window(&self) -> &i_slint_core::window::WindowInner {
        self.window
    }

    fn as_any(&mut self) -> Option<&mut dyn core::any::Any> {
        None
    }
}

impl<T: ProcessScene> i_slint_core::item_rendering::ItemRendererFeatures for SceneBuilder<'_, T> {
    const SUPPORTS_TRANSFORMATIONS: bool = false;
}

#[cfg(feature = "systemfonts")]
use i_slint_core::textlayout::sharedparley::{self, fontique};

#[cfg(feature = "systemfonts")]
impl<T: ProcessScene> sharedparley::GlyphRenderer for SceneBuilder<'_, T> {
    type PlatformBrush = Color;

    fn platform_brush_for_color(&mut self, color: &Color) -> Option<Self::PlatformBrush> {
        Some(*color)
    }

    fn platform_text_fill_brush(
        &mut self,
        brush: Brush,
        _size: LogicalSize,
    ) -> Option<Self::PlatformBrush> {
        Some(brush.color())
    }

    fn platform_text_stroke_brush(
        &mut self,
        brush: Brush,
        _physical_stroke_width: f32,
        _size: LogicalSize,
    ) -> Option<Self::PlatformBrush> {
        Some(brush.color())
    }

    fn fill_rectangle(
        &mut self,
        mut physical_rect: sharedparley::PhysicalRect,
        color: Color,
        radius: sharedparley::PhysicalLength,
        border: Option<sharedparley::RectangleBorder<Color>>,
    ) {
        let has_visible_border =
            border.as_ref().is_some_and(|b| b.width.get() > 0.0 && b.brush.alpha() > 0);
        if color.alpha() == 0 && !has_visible_border {
            return;
        }

        let global_offset =
            (self.current_state.offset.to_vector().cast() * self.scale_factor).cast();

        physical_rect.origin += global_offset;
        // Round the edges instead of truncating them, so that they quantize the way `draw_glyph_run`
        // quantizes the glyph origin and the clip it cuts glyphs with. Truncating here puts a
        // selection highlight edge a whole pixel away from the glyph clip meant to line up with it,
        // as soon as the item's own offset lands on a fractional device pixel.
        let geometry: PhysicalRect = physical_rect.round().cast().transformed(self.rotation);
        // These fills reach the processor directly rather than through `draw_rectangle`, so they
        // have to bring the clip along themselves. Without it a text decoration, a selection
        // highlight or a cursor taller than the item it belongs to paints right over its
        // surroundings, while the glyphs beside it are clipped.
        let clip =
            (self.current_state.clip.translate(self.current_state.offset.to_vector()).cast()
                * self.scale_factor)
                .round()
                .cast()
                .transformed(self.rotation);
        let mut args = target_pixel_buffer::DrawRectangleArgs::from_rect(
            geometry.cast(),
            Brush::SolidColor(color),
        );

        if radius.get() > 0.0 {
            let r = radius.get().min(args.width / 2.0).min(args.height / 2.0);
            args.top_left_radius = r;
            args.top_right_radius = r;
            args.bottom_right_radius = r;
            args.bottom_left_radius = r;
        }

        if let Some(sharedparley::RectangleBorder { brush: border_color, width: border_width }) =
            border
            && border_width.get() > 0.0
            && border_color.alpha() > 0
        {
            args.border_width = border_width.get();
            args.border = Brush::SolidColor(border_color);
        }

        self.processor.process_rectangle(&args, clip);
    }

    fn draw_glyph_run(
        &mut self,
        font: &sharedparley::parley::FontData,
        font_size: sharedparley::PhysicalLength,
        normalized_coords: &[i16],
        synthesis: &fontique::Synthesis,
        _variations: &[sharedparley::parley::style::FontVariation],
        color: Self::PlatformBrush,
        y_offset: sharedparley::PhysicalLength,
        glyphs_it: &mut dyn Iterator<Item = sharedparley::parley::layout::Glyph>,
    ) {
        let slint_context = self.window.context();
        let (swash_key, swash_offset) =
            fonts::systemfonts::get_swash_font_info(&font.data, font.index);
        let font = fonts::vectorfont::VectorFont::new_from_blob_and_index_with_coords(
            font.data.clone().into(),
            font.index,
            swash_key,
            swash_offset,
            font_size.cast(),
            normalized_coords,
        )
        .with_synthesis(*synthesis);

        let global_offset: euclid::Vector2D<f32, PhysicalPx> =
            self.current_state.offset.to_vector().cast() * self.scale_factor;

        const SUBPIXEL_BINS: i32 = fonts::vectorfont::SUBPIXEL_BIN_COUNT;

        let color = self.alpha_color(color);
        let physical_clip: euclid::Rect<i32, PhysicalPx> =
            (self.current_state.clip.translate(self.current_state.offset.to_vector()).cast()
                * self.scale_factor)
                .round()
                .cast()
                .transformed(self.rotation);

        for positioned_glyph in glyphs_it {
            let Some(id) = std::num::NonZero::new(positioned_glyph.id as u16) else {
                continue;
            };

            // Absolute, sub-pixel-accurate device position of the pen for this glyph.
            let abs_x = global_offset.x + positioned_glyph.x;
            let abs_y = global_offset.y + positioned_glyph.y + y_offset.get();

            // Quantize the horizontal position to SUBPIXEL_BINS positions per pixel.
            // The integer part is the blit origin; the fractional part is rendered
            // into the glyph coverage (see `render_vector_glyph`) instead of being
            // discarded. This keeps inter-glyph spacing even: snapping the pen to a
            // whole pixel (truncate or round) redistributes the sub-pixel advances
            // unevenly between neighboring glyph pairs.
            let quantized_x = (abs_x * SUBPIXEL_BINS as f32).round() as i32;
            let dst_int_x = quantized_x.div_euclid(SUBPIXEL_BINS);
            let subpixel_bin = quantized_x.rem_euclid(SUBPIXEL_BINS) as u8;

            let Some(glyph) = font.render_vector_glyph(id, subpixel_bin, slint_context) else {
                continue;
            };

            let gl_y = PhysicalLength::new(glyph.y.truncate() as i16);
            // i32 so a glyph past the i16 range survives until it is clipped below.
            let target_rect: euclid::Rect<i32, PhysicalPx> = euclid::Rect::<f32, PhysicalPx>::new(
                euclid::Point2D::new(
                    dst_int_x as f32 + glyph.glyph_origin_x,
                    abs_y.round() + (-gl_y - glyph.height).get() as f32,
                ),
                glyph.size().cast(),
            )
            .cast()
            .transformed(self.rotation);

            let Some(clipped_target) = physical_clip.intersection(&target_rect) else {
                continue;
            };

            let data = {
                let source_rect = euclid::rect(0, 0, glyph.width.0, glyph.height.0);
                target_pixel_buffer::TextureDataContainer::Shared {
                    buffer: SharedBufferData::AlphaMap {
                        data: glyph.alpha_map,
                        width: glyph.pixel_stride,
                    },
                    source_rect,
                }
            };

            let t = target_pixel_buffer::DrawTextureArgs {
                data,
                colorize: Some(color),
                // color already is mixed with global alpha
                alpha: color.alpha(),
                dst_x: target_rect.origin.x as _,
                dst_y: target_rect.origin.y as _,
                dst_width: target_rect.size.width as _,
                dst_height: target_rect.size.height as _,
                rotation: self.rotation.orientation,
                tiling: None,
            };

            self.processor.process_target_texture(&t, clipped_target.cast());
        }
    }
}
