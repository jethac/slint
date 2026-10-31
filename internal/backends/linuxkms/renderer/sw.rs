// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Delegate the rendering to the [`i_slint_renderer_software::SoftwareRenderer`]

use i_slint_core::api::PhysicalSize as PhysicalWindowSize;
use i_slint_core::platform::PlatformError;
use i_slint_core::renderer::DrawOutcome;
pub use i_slint_renderer_software::SoftwareRenderer;
use i_slint_renderer_software::{PremultipliedRgbaColor, RepaintBufferType, TargetPixel};
use std::sync::Arc;

use crate::display::RenderingRotation;

pub struct SoftwareRendererAdapter {
    renderer: SoftwareRenderer,
    display: Arc<dyn crate::display::swdisplay::SoftwareBufferDisplay>,
    presenter: Arc<dyn crate::display::Presenter>,
    size: PhysicalWindowSize,
    shadow: std::cell::RefCell<Shadow>,
}

/// A cached-memory copy of the screen for displays whose buffers are
/// write-combined. Blending (antialiased text, translucent fills) reads the
/// destination, and reads from write-combined memory are an order of magnitude
/// slower than from cached memory; so the renderer draws here, partially (the
/// shadow always holds the previous frame), and only the changed rectangles are
/// copied into the display's buffer.
#[derive(Default)]
struct Shadow {
    pixels: Vec<u32>,
    /// Rectangles (x, y, width, height) copied for the previous frame: a
    /// buffer two frames old also needs those.
    prev: Vec<(usize, usize, usize, usize)>,
}

const SOFTWARE_RENDER_SUPPORTED_DRM_FOURCC_FORMATS: &[drm::buffer::DrmFourcc] = &[
    // Preferred formats
    drm::buffer::DrmFourcc::Xrgb8888,
    drm::buffer::DrmFourcc::Argb8888,
    drm::buffer::DrmFourcc::Bgra8888,
    // drm::buffer::DrmFourcc::Rgba8888,

    // 16-bit formats
    drm::buffer::DrmFourcc::Rgb565,
    // drm::buffer::DrmFourcc::Bgr565,

    // // 4444 formats
    // drm::buffer::DrmFourcc::Argb4444,
    // drm::buffer::DrmFourcc::Abgr4444,
    // drm::buffer::DrmFourcc::Rgba4444,
    // drm::buffer::DrmFourcc::Bgra4444,

    // // Single channel formats
    // drm::buffer::DrmFourcc::Gray8,
    // drm::buffer::DrmFourcc::C8,
    // drm::buffer::DrmFourcc::R8,
    // drm::buffer::DrmFourcc::R16,

    // // Dual channel formats
    // drm::buffer::DrmFourcc::Gr88,
    // drm::buffer::DrmFourcc::Rg88,
    // drm::buffer::DrmFourcc::Gr1616,
    // drm::buffer::DrmFourcc::Rg1616,

    // // 10-bit formats
    // drm::buffer::DrmFourcc::Xrgb2101010,
    // drm::buffer::DrmFourcc::Argb2101010,
    // drm::buffer::DrmFourcc::Abgr2101010,
    // drm::buffer::DrmFourcc::Rgba1010102,
    // drm::buffer::DrmFourcc::Bgra1010102,
    // drm::buffer::DrmFourcc::Rgbx1010102,
    // drm::buffer::DrmFourcc::Bgrx1010102,
];

#[repr(transparent)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct DumbBufferPixelXrgb888(pub u32);

#[repr(transparent)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct DumbBufferPixelBgra8888(pub u32);

impl From<DumbBufferPixelXrgb888> for PremultipliedRgbaColor {
    #[inline]
    fn from(pixel: DumbBufferPixelXrgb888) -> Self {
        let v = pixel.0;
        PremultipliedRgbaColor {
            red: (v >> 16) as u8,
            green: (v >> 8) as u8,
            blue: v as u8,
            alpha: (v >> 24) as u8,
        }
    }
}

impl From<PremultipliedRgbaColor> for DumbBufferPixelXrgb888 {
    #[inline]
    fn from(pixel: PremultipliedRgbaColor) -> Self {
        Self(
            (pixel.alpha as u32) << 24
                | ((pixel.red as u32) << 16)
                | ((pixel.green as u32) << 8)
                | (pixel.blue as u32),
        )
    }
}

impl From<DumbBufferPixelBgra8888> for PremultipliedRgbaColor {
    #[inline]
    fn from(pixel: DumbBufferPixelBgra8888) -> Self {
        let v = pixel.0;
        PremultipliedRgbaColor {
            red: (v >> 8) as u8,
            green: (v >> 16) as u8,
            blue: (v >> 24) as u8,
            alpha: v as u8,
        }
    }
}

impl From<PremultipliedRgbaColor> for DumbBufferPixelBgra8888 {
    #[inline]
    fn from(pixel: PremultipliedRgbaColor) -> Self {
        Self(
            pixel.alpha as u32
                | ((pixel.red as u32) << 8)
                | ((pixel.green as u32) << 16)
                | ((pixel.blue as u32) << 24),
        )
    }
}

impl TargetPixel for DumbBufferPixelXrgb888 {
    fn blend(&mut self, color: PremultipliedRgbaColor) {
        let mut x = PremultipliedRgbaColor::from(*self);
        x.blend(color);
        *self = x.into();
    }

    fn lerp_from(&mut self, src: Self, mask: u8) {
        let mut x = PremultipliedRgbaColor::from(*self);
        x.lerp_from(PremultipliedRgbaColor::from(src), mask);
        *self = x.into();
    }

    fn from_rgb(r: u8, g: u8, b: u8) -> Self {
        Self(0xff000000 | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32))
    }

    fn background() -> Self {
        Self(0)
    }
}

impl TargetPixel for DumbBufferPixelBgra8888 {
    fn blend(&mut self, color: PremultipliedRgbaColor) {
        let mut x = PremultipliedRgbaColor::from(*self);
        x.blend(color);
        *self = x.into();
    }

    fn lerp_from(&mut self, src: Self, mask: u8) {
        let mut x = PremultipliedRgbaColor::from(*self);
        x.lerp_from(PremultipliedRgbaColor::from(src), mask);
        *self = x.into();
    }

    fn from_rgb(r: u8, g: u8, b: u8) -> Self {
        Self(0x000000ff | ((r as u32) << 8) | ((g as u32) << 16) | ((b as u32) << 24))
    }
    fn background() -> Self {
        Self(0)
    }
}

impl SoftwareRendererAdapter {
    #[allow(clippy::new_ret_no_self)]
    pub fn new(
        device_opener: &crate::DeviceOpener,
        _requested_graphics_api: Option<&i_slint_core::graphics::RequestedGraphicsAPI>,
    ) -> Result<Box<dyn crate::fullscreenwindowadapter::FullscreenRenderer>, PlatformError> {
        let display = crate::display::swdisplay::new(
            device_opener,
            SOFTWARE_RENDER_SUPPORTED_DRM_FOURCC_FORMATS,
        )?;

        let (width, height) = display.size();
        let size = i_slint_core::api::PhysicalSize::new(width, height);

        let renderer = Box::new(Self {
            renderer: SoftwareRenderer::new(),
            display: display.clone(),
            presenter: display.as_presenter(),
            size,
            shadow: Default::default(),
        });

        eprintln!("Using Software renderer");

        Ok(renderer)
    }
}

impl SoftwareRendererAdapter {
    /// Render into the cached shadow buffer, then copy what changed into the
    /// display's buffer `pixels` (32 bits per pixel, `stride` pixels per row)
    /// of the given age.
    fn render_shadowed<P: TargetPixel + bytemuck::Pod>(
        &self,
        pixels: &mut [u8],
        age: u8,
        stride: usize,
    ) {
        let width = self.size.width as usize;
        let height = self.size.height as usize;
        let mut shadow = self.shadow.borrow_mut();
        let fresh = shadow.pixels.len() != width * height;
        if fresh {
            shadow.pixels = vec![0; width * height];
        }
        self.renderer.set_repaint_buffer_type(if fresh {
            RepaintBufferType::NewBuffer
        } else {
            RepaintBufferType::ReusedBuffer
        });
        let region = {
            let buffer: &mut [P] = bytemuck::cast_slice_mut(&mut shadow.pixels);
            self.renderer.render(buffer, width)
        };
        let rects: Vec<Rect> = region
            .iter()
            .map(|(p, s)| {
                (p.x.max(0) as usize, p.y.max(0) as usize, s.width as usize, s.height as usize)
            })
            .collect();
        let dst: &mut [u32] = bytemuck::cast_slice_mut(pixels);
        copy_damage(dst, stride, &shadow.pixels, width, height, &rects, &shadow.prev, age, fresh);
        shadow.prev = rects;
    }
}

/// A rectangle (x, y, width, height) in pixels.
type Rect = (usize, usize, usize, usize);

/// Bring a display buffer of the given `age` up to date with the shadow `src`
/// (`width` x `height`, tightly packed), given the rectangles that changed in
/// this frame (`rects`) and in the previous one (`prev`).
///
/// A buffer of age 1 holds the previous frame, so this frame's rectangles are
/// enough. A buffer of age 2 holds the frame before that: it also misses what
/// changed in the previous frame, which went only into the other buffer. That
/// includes the place an item has just left, so without `prev` an item that
/// moves or disappears leaves a copy of itself behind in every other frame.
/// Any other age, or a shadow that was just created, takes a full copy.
#[allow(clippy::too_many_arguments)]
fn copy_damage(
    dst: &mut [u32],
    stride: usize,
    src: &[u32],
    width: usize,
    height: usize,
    rects: &[Rect],
    prev: &[Rect],
    age: u8,
    fresh: bool,
) {
    let mut copy = |&(x, y, w, h): &Rect| {
        let x1 = (x + w).min(width);
        if x >= x1 {
            return;
        }
        for row in y..(y + h).min(height) {
            dst[row * stride + x..row * stride + x1]
                .copy_from_slice(&src[row * width + x..row * width + x1]);
        }
    };
    match age {
        1 if !fresh => rects.iter().for_each(copy),
        2 if !fresh => rects.iter().chain(prev.iter()).for_each(copy),
        _ => copy(&(0, 0, width, height)),
    }
}

#[cfg(test)]
mod tests {
    use super::{Rect, copy_damage};

    const W: usize = 40;
    const H: usize = 20;
    const STRIDE: usize = 48;

    /// The shadow for a frame: background 0, and a 10 x 8 "pill" at `x`
    /// (which may be partly or wholly left of the screen), if shown.
    fn frame(x: Option<i32>) -> Vec<u32> {
        let mut v = vec![0; W * H];
        if let Some(x) = x {
            for row in 6..14 {
                for col in x.max(0)..(x + 10).clamp(0, W as i32) {
                    v[row * W + col as usize] = 7;
                }
            }
        }
        v
    }

    /// What changed between two frames, as a renderer reports it: the union of
    /// the old and new places, clipped to the screen.
    fn damage(a: Option<i32>, b: Option<i32>) -> Vec<Rect> {
        let span = |x: Option<i32>| x.map(|x| (x.max(0), (x + 10).clamp(0, W as i32)));
        let spans: Vec<(i32, i32)> =
            [span(a), span(b)].into_iter().flatten().filter(|s| s.0 < s.1).collect();
        if spans.is_empty() {
            return vec![];
        }
        let x0 = spans.iter().map(|s| s.0).min().unwrap();
        let x1 = spans.iter().map(|s| s.1).max().unwrap();
        vec![(x0 as usize, 6, (x1 - x0) as usize, 8)]
    }

    /// Two display buffers in rotation, each frame copied from the shadow into
    /// the one two frames old, must show exactly the shadow. The sequence is
    /// the volume charm: off the left edge, sliding in, then out and hidden,
    /// with a few frames where only something elsewhere changes.
    #[test]
    fn two_buffers_follow_an_item_that_slides_off_screen_and_hides() {
        let seq: Vec<Option<i32>> = [
            None,
            None,
            Some(-9),
            Some(-5),
            Some(-1),
            Some(3),
            Some(5),
            Some(5),
            Some(1),
            Some(-4),
            Some(-8),
            None,
            None,
            None,
        ]
        .into();
        let mut buffers = [vec![0xdead_u32; STRIDE * H], vec![0xbeef_u32; STRIDE * H]];
        let mut ages = [0u8, 0u8];
        let mut prev: Vec<Rect> = vec![];
        let mut last = None;
        for (i, &x) in seq.iter().enumerate() {
            let src = frame(x);
            let rects = damage(last, x);
            let b = i % 2;
            copy_damage(&mut buffers[b], STRIDE, &src, W, H, &rects, &prev, ages[b], i == 0);
            for row in 0..H {
                assert_eq!(
                    &buffers[b][row * STRIDE..row * STRIDE + W],
                    &src[row * W..(row + 1) * W],
                    "frame {i} (pill at {x:?}), row {row}"
                );
            }
            // As DumbBufferDisplay::present: the presented buffer is age 1, the
            // other one a frame older (a never drawn buffer stays at 0).
            ages[b] = 1;
            if ages[1 - b] != 0 {
                ages[1 - b] += 1;
            }
            prev = rects;
            last = x;
        }
    }
}

impl crate::fullscreenwindowadapter::FullscreenRenderer for SoftwareRendererAdapter {
    fn as_core_renderer(&self) -> &dyn i_slint_core::renderer::Renderer {
        &self.renderer
    }

    fn render_and_present(
        &self,
        rotation: RenderingRotation,
        _draw_mouse_cursor_callback: &dyn Fn(&mut dyn i_slint_core::item_rendering::ItemRenderer),
    ) -> Result<DrawOutcome, PlatformError> {
        self.display.map_back_buffer(&mut |pixels, age, format| {
            let shadowed = self.display.is_write_combined_memory()
                && rotation == RenderingRotation::NoRotation
                && matches!(
                    format,
                    drm::buffer::DrmFourcc::Xrgb8888
                        | drm::buffer::DrmFourcc::Argb8888
                        | drm::buffer::DrmFourcc::Bgra8888
                );
            // The shadow path sets its own buffer type; changing the type
            // clears the renderer's caches, so it is set once per path.
            if !shadowed {
                self.renderer.set_repaint_buffer_type(match age {
                    1 => RepaintBufferType::ReusedBuffer,
                    2 => RepaintBufferType::SwappedBuffers,
                    _ => RepaintBufferType::NewBuffer,
                });
            }

            self.renderer.set_rendering_rotation(match rotation {
                RenderingRotation::NoRotation => {
                    i_slint_renderer_software::RenderingRotation::NoRotation
                }
                RenderingRotation::Rotate90 => {
                    i_slint_renderer_software::RenderingRotation::Rotate90
                }
                RenderingRotation::Rotate180 => {
                    i_slint_renderer_software::RenderingRotation::Rotate180
                }
                RenderingRotation::Rotate270 => {
                    i_slint_renderer_software::RenderingRotation::Rotate270
                }
            });

            // The buffer's rows may be padded to the driver's pitch; the
            // stride in pixels comes from the mapping, not from the width.
            let stride = |bytes_per_pixel: usize| {
                let height = (self.size.height as usize).max(1);
                (pixels.len() / height / bytes_per_pixel).max(self.size.width as usize)
            };
            match format {
                drm::buffer::DrmFourcc::Xrgb8888 | drm::buffer::DrmFourcc::Argb8888 if shadowed => {
                    self.render_shadowed::<DumbBufferPixelXrgb888>(pixels, age, stride(4));
                }
                drm::buffer::DrmFourcc::Bgra8888 if shadowed => {
                    self.render_shadowed::<DumbBufferPixelBgra8888>(pixels, age, stride(4));
                }
                drm::buffer::DrmFourcc::Xrgb8888 | drm::buffer::DrmFourcc::Argb8888 => {
                    let stride = stride(std::mem::size_of::<DumbBufferPixelXrgb888>());
                    let buffer: &mut [DumbBufferPixelXrgb888] = bytemuck::cast_slice_mut(pixels);
                    self.renderer.render(buffer, stride);
                }

                drm::buffer::DrmFourcc::Bgra8888 => {
                    let stride = stride(std::mem::size_of::<DumbBufferPixelBgra8888>());
                    let buffer: &mut [DumbBufferPixelBgra8888] = bytemuck::cast_slice_mut(pixels);
                    self.renderer.render(buffer, stride);
                }
                drm::buffer::DrmFourcc::Rgb565 => {
                    let stride =
                        stride(std::mem::size_of::<i_slint_renderer_software::Rgb565Pixel>());
                    let buffer: &mut [i_slint_renderer_software::Rgb565Pixel] =
                        bytemuck::cast_slice_mut(pixels);
                    self.renderer.render(buffer, stride);
                }
                _ => {
                    return Err(format!(
                        "Unsupported frame buffer format {format} used with software renderer"
                    )
                    .into());
                }
            }

            Ok(())
        })?;
        self.presenter.present()?;
        Ok(DrawOutcome::Success)
    }

    fn size(&self) -> i_slint_core::api::PhysicalSize {
        self.size
    }
}
