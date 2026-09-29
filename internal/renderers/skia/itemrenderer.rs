// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore rrect skpath Minkowski unspread

use std::pin::Pin;

use super::{PhysicalBorderRadius, PhysicalLength, PhysicalPoint, PhysicalRect, PhysicalSize};
use i_slint_core::graphics::ApproxEq;
use i_slint_core::graphics::ResolvedBrush;
use i_slint_core::graphics::boxshadowcache::BoxShadowCache;
use i_slint_core::graphics::euclid::num::Zero;
use i_slint_core::graphics::euclid::{self, Vector2D};
use i_slint_core::item_rendering::{
    BorderRectLayout, CachedRenderingData, ItemCache, ItemRenderer, ItemRendererFeatures,
    LayerRenderer, RenderImage, RenderText,
};
use i_slint_core::items::{ImageFit, ImageRendering, ItemRc, Layer, Opacity, RenderingResult};
use i_slint_core::lengths::{
    LogicalLength, LogicalPoint, LogicalPx, LogicalRect, LogicalSize, LogicalVector, PhysicalPx,
    RectLengths, ScaleFactor, SizeLengths, logical_size_from_api,
};
use i_slint_core::textlayout::sharedparley::{self, GlyphRenderer, fontique};
use i_slint_core::window::WindowInner;
use i_slint_core::{Brush, Color, SharedString};
use skia_safe::{Matrix, TileMode};

pub type SkiaBoxShadowCache = BoxShadowCache<skia_safe::Image>;

#[derive(Clone, Copy)]
struct RenderState {
    alpha: f32,
    transform: i_slint_core::lengths::ItemTransform,
}

pub struct SkiaItemRenderer<'a> {
    pub canvas: &'a skia_safe::Canvas,
    pub scale_factor: ScaleFactor,
    pub window: &'a i_slint_core::api::Window,
    surface: Option<&'a dyn crate::Surface>,
    state_stack: Vec<RenderState>,
    current_state: RenderState,
    image_cache: &'a ItemCache<Option<skia_safe::Image>>,
    layer_cache: &'a ItemCache<Option<(PhysicalPoint, skia_safe::Image)>>,
    path_cache: &'a ItemCache<Option<(Vector2D<f32, PhysicalPx>, skia_safe::Path)>>,
    text_layout_cache: &'a sharedparley::TextLayoutCache,
    box_shadow_cache: &'a SkiaBoxShadowCache,
}

impl<'a> SkiaItemRenderer<'a> {
    pub fn new(
        canvas: &'a skia_safe::Canvas,
        window: &'a i_slint_core::api::Window,
        surface: Option<&'a dyn crate::Surface>,
        image_cache: &'a ItemCache<Option<skia_safe::Image>>,
        layer_cache: &'a ItemCache<Option<(PhysicalPoint, skia_safe::Image)>>,
        path_cache: &'a ItemCache<Option<(Vector2D<f32, PhysicalPx>, skia_safe::Path)>>,
        text_layout_cache: &'a sharedparley::TextLayoutCache,
        box_shadow_cache: &'a SkiaBoxShadowCache,
    ) -> Self {
        Self {
            canvas,
            scale_factor: ScaleFactor::new(window.scale_factor()),
            window,
            surface,
            state_stack: Vec::new(),
            current_state: RenderState {
                alpha: 1.0,
                transform: i_slint_core::lengths::ItemTransform::identity(),
            },
            image_cache,
            layer_cache,
            path_cache,
            text_layout_cache,
            box_shadow_cache,
        }
    }

    fn default_paint(&self) -> Option<skia_safe::Paint> {
        if self.current_state.alpha.approx_eq(&1.0) {
            None
        } else {
            let mut paint = skia_safe::Paint::default();
            paint.set_alpha_f(self.current_state.alpha);
            Some(paint)
        }
    }

    /// Skia leaves anti-aliasing off by default, which keeps an upright rectangle's edges crisp.
    /// A transform that tilts the rectangle turns those edges into stair steps instead.
    fn needs_anti_alias(&self) -> bool {
        !self.canvas.local_to_device_as_3x3().preserves_axis_alignment()
    }

    fn render_drop_shadow_image(
        canvas: &skia_safe::Canvas,
        shadow_options: &i_slint_core::graphics::boxshadowcache::BoxShadowOptions,
    ) -> Option<skia_safe::Image> {
        let shape_size = shadow_options.shape_size();
        if shape_size.is_empty() {
            return None;
        }

        let canvas_size: skia_safe::Size = {
            let size = shadow_options.drop_texture_size();
            (size.width, size.height).into()
        };

        let image_info = crate::image_info(
            canvas_size.to_ceil(),
            skia_safe::ColorType::RGBA8888,
            skia_safe::AlphaType::Premul,
        );

        let outline = shadow_options.element_outline();
        let mut paint = crate::solid_paint(&shadow_options.color);
        paint.set_anti_alias(true);
        if shadow_options.blur.get() > 0. {
            paint.set_mask_filter(skia_safe::MaskFilter::blur(
                skia_safe::BlurStyle::Normal,
                shadow_options.blur_sigma(),
                None,
            ));
        }

        let mut surface = canvas.new_surface(&image_info, None)?;
        let surface_canvas = surface.canvas();
        surface_canvas.clear(skia_safe::Color::TRANSPARENT);
        match &outline {
            None => {
                let rounded_rect = to_skia_rrect(
                    &PhysicalRect::new(shadow_options.shape_origin(), shape_size),
                    &shadow_options.outer_radius(),
                );
                surface_canvas.draw_rrect(rounded_rect, &paint);
            }
            Some(outline) => {
                // The shadow silhouette is the outline grown by the spread, i.e.
                // the Minkowski sum of the outline and a disk of that radius:
                // fill(outline) ∪ stroke(outline, 2·spread). The outline sits
                // at the unspread geometry's position within the texture.
                // A negative spread erodes instead: fill ∖ stroke(2·|spread|).
                let spread = shadow_options.spread.get();
                let target = PhysicalRect::new(
                    PhysicalPoint::new(
                        shadow_options.shape_origin().x + spread.max(0.),
                        shadow_options.shape_origin().y + spread.max(0.),
                    ),
                    PhysicalSize::new(shadow_options.width.get(), shadow_options.height.get()),
                );
                let path = outline_to_skia_path(outline, target);
                let band = if spread != 0. {
                    let mut stroke_paint = skia_safe::Paint::default();
                    stroke_paint.set_style(skia_safe::PaintStyle::Stroke);
                    stroke_paint.set_stroke_width(2. * spread.abs());
                    stroke_paint.set_stroke_join(skia_safe::PaintJoin::Round);
                    stroke_paint.set_stroke_cap(skia_safe::PaintCap::Round);
                    let mut stroked = skia_safe::PathBuilder::new();
                    skia_safe::path_utils::fill_path_with_paint(
                        &path,
                        &stroke_paint,
                        &mut stroked,
                        None,
                        None,
                    )
                    .then(|| stroked.detach())
                } else {
                    None
                };
                let silhouette = if spread > 0. {
                    let mut builder = skia_safe::PathBuilder::new();
                    builder.set_fill_type(skia_safe::PathFillType::Winding);
                    builder.add_path(&path, skia_safe::path::AddPathMode::Append);
                    if let Some(band) = &band {
                        builder.add_path(band, skia_safe::path::AddPathMode::Append);
                    }
                    builder.detach()
                } else if let Some(band) = band {
                    skia_safe::op(&path, &band, skia_safe::PathOp::Difference).unwrap_or(path)
                } else {
                    path
                };
                surface_canvas.draw_path(&silhouette, &paint);
            }
        }
        Some(surface.image_snapshot())
    }

    fn render_inset_shadow_image(
        canvas: &skia_safe::Canvas,
        shadow_options: &i_slint_core::graphics::boxshadowcache::BoxShadowOptions,
    ) -> Option<skia_safe::Image> {
        let width = shadow_options.width.get();
        let height = shadow_options.height.get();
        if width < 1. || height < 1. {
            return None;
        }
        let blur = shadow_options.blur.get();
        let spread = shadow_options.spread.get();
        let radius = shadow_options.radius;
        let offset_x = shadow_options.offset_x_inset;
        let offset_y = shadow_options.offset_y_inset;

        // Image is sized to the rectangle's geometry; the geometry rrect serves as the clip so the
        // outer blurred edge stays hidden.
        let canvas_size = skia_safe::ISize::new(width.ceil() as i32, height.ceil() as i32);
        let image_info = crate::image_info(
            canvas_size,
            skia_safe::ColorType::RGBA8888,
            skia_safe::AlphaType::Premul,
        );

        let outline = shadow_options.element_outline();
        let geometry_rect =
            PhysicalRect::new(PhysicalPoint::zero(), PhysicalSize::new(width, height));

        // Inner "hole" rrect: geometry inset by spread on each side, translated by offset.
        let inner_rect = skia_safe::Rect::new(
            spread + offset_x,
            spread + offset_y,
            width - spread + offset_x,
            height - spread + offset_y,
        );

        // Outer rect inflated well beyond the geometry so its blurred edge falls outside the clip.
        let inflate = blur + spread.abs() + offset_x.abs() + offset_y.abs() + 16.;
        let outer_rect =
            skia_safe::Rect::new(-inflate, -inflate, width + inflate, height + inflate);

        let mut path_builder = skia_safe::PathBuilder::new();
        path_builder.set_fill_type(skia_safe::PathFillType::EvenOdd);
        path_builder.add_rect(outer_rect, None, None);
        match &outline {
            None => {
                let inner_rrect = to_skia_rrect(
                    &PhysicalRect::new(
                        PhysicalPoint::new(inner_rect.left, inner_rect.top),
                        PhysicalSize::new(inner_rect.width(), inner_rect.height()),
                    ),
                    &shadow_options.inner_radius(),
                );
                path_builder.add_rrect(inner_rrect, None, None);
            }
            Some(outline) => {
                // The hole is the outline eroded by a positive spread —
                // fill(outline) minus the band the stroke of width 2·spread
                // covers — and dilated by a negative one — fill ∪ band —
                // both translated by the inset offset.
                let hole_path = outline_to_skia_path(
                    outline,
                    PhysicalRect::new(PhysicalPoint::new(offset_x, offset_y), geometry_rect.size),
                );
                let hole = if spread != 0. {
                    let mut stroke_paint = skia_safe::Paint::default();
                    stroke_paint.set_style(skia_safe::PaintStyle::Stroke);
                    stroke_paint.set_stroke_width(2. * spread.abs());
                    stroke_paint.set_stroke_join(skia_safe::PaintJoin::Round);
                    stroke_paint.set_stroke_cap(skia_safe::PaintCap::Round);
                    let mut band = skia_safe::PathBuilder::new();
                    if skia_safe::path_utils::fill_path_with_paint(
                        &hole_path,
                        &stroke_paint,
                        &mut band,
                        None,
                        None,
                    ) {
                        skia_safe::op(
                            &hole_path,
                            &band.detach(),
                            if spread > 0. {
                                skia_safe::PathOp::Difference
                            } else {
                                skia_safe::PathOp::Union
                            },
                        )
                        .unwrap_or_else(|| hole_path.clone())
                    } else {
                        hole_path.clone()
                    }
                } else {
                    hole_path
                };
                path_builder.add_path(&hole, skia_safe::path::AddPathMode::Append);
            }
        }
        let path = path_builder.detach();

        let mut paint = crate::solid_paint(&shadow_options.color);
        paint.set_anti_alias(true);
        if blur > 0. {
            paint.set_mask_filter(skia_safe::MaskFilter::blur(
                skia_safe::BlurStyle::Normal,
                shadow_options.blur_sigma(),
                None,
            ));
        }

        let mut surface = canvas.new_surface(&image_info, None)?;
        let surface_canvas = surface.canvas();
        surface_canvas.clear(skia_safe::Color::TRANSPARENT);
        match &outline {
            None => {
                let geometry_rrect = to_skia_rrect(&geometry_rect, &radius);
                surface_canvas.clip_rrect(geometry_rrect, None, true);
            }
            Some(outline) => {
                let clip = outline_to_skia_path(outline, geometry_rect);
                surface_canvas.clip_path(&clip, None, true);
            }
        }
        surface_canvas.draw_path(&path, &paint);
        Some(surface.image_snapshot())
    }

    fn brush_to_paint(
        &self,
        brush: Brush,
        width: PhysicalLength,
        height: PhysicalLength,
    ) -> Option<skia_safe::Paint> {
        let (mut paint, shader) = Self::brush_to_shader(
            self.default_paint().unwrap_or_default(),
            brush,
            width,
            height,
            self.scale_factor,
        )?;
        paint.set_shader(Some(shader));

        Some(paint)
    }

    fn brush_to_shader(
        mut paint: skia_safe::Paint,
        brush: Brush,
        width: PhysicalLength,
        height: PhysicalLength,
        scale_factor: ScaleFactor,
    ) -> Option<(skia_safe::Paint, skia_safe::Shader)> {
        let resolved = i_slint_core::graphics::resolve_brush(
            &brush,
            euclid::Size2D::from_lengths(width, height),
            scale_factor,
        )?;

        fn gradient<'g>(
            colors: &'g [skia_safe::Color4f],
            pos: &'g [f32],
            in_premul: skia_safe::gradient::interpolation::InPremul,
        ) -> skia_safe::gradient::Gradient<'g> {
            skia_safe::gradient::Gradient::new(
                crate::gradient_colors(colors, pos),
                skia_safe::gradient::Interpolation {
                    in_premul,
                    color_space: skia_safe::gradient::interpolation::ColorSpace::SRGB,
                    ..Default::default()
                },
            )
        }

        match resolved {
            ResolvedBrush::SolidColor(color) => Some(crate::color_shader(&color)),

            ResolvedBrush::LinearGradient(g) => {
                let (colors, pos) = to_skia_stops(&g.stops);

                paint.set_dither(true);

                skia_safe::gradient::shaders::linear_gradient(
                    (
                        skia_safe::Point::new(g.start.x, g.start.y),
                        skia_safe::Point::new(g.end.x, g.end.y),
                    ),
                    &gradient(&colors, &pos, skia_safe::gradient::interpolation::InPremul::Yes),
                    None,
                )
            }
            ResolvedBrush::RadialGradient(g) => {
                let (colors, pos) = to_skia_stops(&g.stops);

                paint.set_dither(true);

                let mut local_matrix = skia_safe::Matrix::scale((g.radius.get(), g.radius.get()));
                local_matrix.post_translate((g.center.x, g.center.y));
                skia_safe::gradient::shaders::radial_gradient(
                    (skia_safe::Point::new(0., 0.), 1.),
                    &gradient(&colors, &pos, skia_safe::gradient::interpolation::InPremul::Yes),
                    &local_matrix,
                )
            }
            ResolvedBrush::ConicGradient(g) => {
                let (colors, pos) = to_skia_stops(&g.stops);

                paint.set_dither(true);

                // Skia's sweep gradient uses 0 degrees at 3 o'clock (east)
                // We want 0 degrees at 12 o'clock (north), so we need to rotate by -90 degrees
                let center = skia_safe::Point::new(g.center.x, g.center.y);
                skia_safe::gradient::shaders::sweep_gradient(
                    center,
                    (0.0, 360.0),
                    &gradient(&colors, &pos, skia_safe::gradient::interpolation::InPremul::No),
                    &skia_safe::Matrix::rotate_deg_pivot(-90.0, center),
                )
            }
        }
        .map(|shader| (paint, shader))
    }

    fn colorize_image(
        &mut self,
        image: skia_safe::Image,
        colorize_brush: Brush,
    ) -> Option<skia_safe::Image> {
        let image_info = crate::image_info(
            image.dimensions(),
            skia_safe::ColorType::RGBA8888,
            skia_safe::AlphaType::Premul,
        );

        Self::brush_to_shader(
            skia_safe::Paint::default(), // Don't use the renderer's default paint because alpha is applied later
            colorize_brush,
            PhysicalLength::new(image.width() as f32),
            PhysicalLength::new(image.height() as f32),
            self.scale_factor,
        )
        .map(|(mut paint, colorize_shader)| {
            let mut surface = self.canvas.new_surface(&image_info, None)?;
            let canvas = surface.canvas();
            canvas.clear(skia_safe::Color::TRANSPARENT);

            paint.set_image_filter(skia_safe::image_filters::blend(
                skia_safe::BlendMode::SrcIn,
                skia_safe::image_filters::image(image, None, None, None),
                skia_safe::image_filters::shader(colorize_shader, None),
                None,
            ));
            canvas.draw_paint(&paint);
            Some(surface.image_snapshot())
        })?
    }

    fn draw_image_impl(
        &mut self,
        item_rc: &ItemRc,
        item: Pin<&dyn RenderImage>,
        dest_rect: PhysicalRect,
    ) {
        let tiling = item.tiling();

        // TODO: avoid doing creating an SkImage multiple times when the same source is used in multiple image elements
        let skia_image = self.image_cache.get_or_update_cache_entry(item_rc, || {
            let image = item.source();
            super::cached_image::as_skia_image(
                image,
                &|| item.target_size(),
                if tiling != Default::default() { ImageFit::Preserve } else { item.image_fit() },
                self.scale_factor,
                self.canvas,
                self.surface,
            )
            .and_then(|skia_image| {
                let brush = item.colorize();
                if !brush.is_transparent() {
                    self.colorize_image(skia_image, brush)
                } else {
                    Some(skia_image)
                }
            })
        });

        let Some(skia_image) = skia_image else { return };
        let source = item.source();
        let source_size = source.size();
        if source_size.is_empty() {
            // Not sure how this can happen, but we've seen with #6280
            // that somehow we end up with a `skia_safe::Image` but a zero
            // source size.
            return;
        }
        let fits = if let i_slint_core::ImageInner::NineSlice(nine) =
            <&i_slint_core::ImageInner>::from(&source)
        {
            i_slint_core::graphics::fit9slice(
                source_size.cast(),
                nine.1,
                dest_rect.size,
                self.scale_factor,
                item.alignment(),
                tiling,
            )
            .collect::<Vec<_>>()
        } else {
            vec![i_slint_core::graphics::fit(
                item.image_fit(),
                dest_rect.size,
                item.source_clip().unwrap_or_else(|| euclid::Rect::from_size(source_size.cast())),
                self.scale_factor,
                item.alignment(),
                tiling,
            )]
        };

        let _saved_canvas = self.pixel_align_origin_auto_restore();
        for fit in fits {
            self.canvas.save();

            let dst = to_skia_rect(&PhysicalRect::new(fit.offset, fit.size));
            self.canvas.clip_rect(dst, None, None);
            let src = skia_safe::IRect::from_xywh(
                skia_image.width() * fit.clip_rect.origin.x / source_size.width as i32,
                skia_image.height() * fit.clip_rect.origin.y / source_size.height as i32,
                skia_image.width() * fit.clip_rect.size.width / source_size.width as i32,
                skia_image.height() * fit.clip_rect.size.height / source_size.height as i32,
            );

            let filter_mode: skia_safe::sampling_options::SamplingOptions =
                match item.rendering() {
                    ImageRendering::Pixelated => skia_safe::sampling_options::FilterMode::Nearest,
                    ImageRendering::Smooth | _ => skia_safe::sampling_options::FilterMode::Linear,
                }
                .into();

            if let Some(tiled_offset) = fit.tiled {
                let matrix = Matrix::translate(((fit.offset.x as i32), (fit.offset.y as i32)))
                    * Matrix::scale((
                        fit.source_to_target_x * source_size.width as f32
                            / skia_image.width() as f32,
                        fit.source_to_target_y * source_size.height as f32
                            / skia_image.height() as f32,
                    ))
                    * Matrix::translate((-(tiled_offset.x as i32), -(tiled_offset.y as i32)));
                if let Some(shader) = skia_image
                    .make_subset(
                        self.canvas
                            .recording_context()
                            .as_mut()
                            .map(|c| c.as_recorder() as &mut dyn skia_safe::Recorder),
                        src,
                        skia_safe::image::RequiredProperties::default(),
                    )
                    .and_then(|i| {
                        i.to_shader((TileMode::Repeat, TileMode::Repeat), filter_mode, &matrix)
                    })
                {
                    let mut paint = self.default_paint().unwrap_or_default();
                    paint.set_shader(shader);
                    self.canvas.draw_paint(&paint);
                }
            } else {
                let transform =
                    skia_safe::Matrix::rect_2_rect(skia_safe::Rect::from(src), dst, None)
                        .unwrap_or_default();
                self.canvas.concat(&transform);
                self.canvas.draw_image_with_sampling_options(
                    skia_image.clone(),
                    skia_safe::Point::default(),
                    filter_mode,
                    self.default_paint().as_ref(),
                );
            }

            self.canvas.restore();
        }
    }

    fn render_and_blend_layer(&mut self, item_rc: &ItemRc) -> RenderingResult {
        if let Some((layer_offset, layer_image)) =
            i_slint_core::item_rendering::render_layer(self, item_rc)
        {
            self.canvas.translate(skia_safe::Vector::from((layer_offset.x, layer_offset.y)));
            let _saved_canvas = self.pixel_align_origin_auto_restore();
            self.canvas.draw_image_with_sampling_options(
                layer_image,
                skia_safe::Point::default(),
                skia_safe::sampling_options::FilterMode::Linear,
                self.default_paint().as_ref(),
            );
        }
        RenderingResult::ContinueRenderingWithoutChildren
    }

    // Snap the alignment anchor; the caller restores the canvas when this returns true.
    fn save_canvas_and_pixel_align_origin(&self, anchor: PhysicalPoint) -> bool {
        let local_to_device = self.canvas.local_to_device_as_3x3();
        if !local_to_device.is_translate() {
            return false;
        }
        let Some(device_to_local) = local_to_device.invert() else {
            return false;
        };
        let mut target_point = local_to_device.map_point(to_skia_point(anchor));

        target_point.x = target_point.x.round();
        target_point.y = target_point.y.round();

        self.canvas.save();

        self.canvas.translate(device_to_local.map_point(target_point) - to_skia_point(anchor));

        true
    }

    fn pixel_align_origin_auto_restore(&self) -> Option<skia_safe::canvas::AutoRestoredCanvas<'_>> {
        let local_to_device = self.canvas.local_to_device_as_3x3();
        if !local_to_device.is_translate() || local_to_device.is_identity() {
            return None;
        }
        let device_to_local = local_to_device.invert()?;
        let mut target_point = local_to_device.map_point(skia_safe::Point::default());

        target_point.x = target_point.x.round();
        target_point.y = target_point.y.round();

        let restore_point = skia_safe::AutoCanvasRestore::guard(self.canvas, true);

        self.canvas.translate(device_to_local.map_point(target_point));

        Some(restore_point)
    }
}

impl ItemRenderer for SkiaItemRenderer<'_> {
    fn global_alpha_transparent(&self) -> bool {
        self.current_state.alpha == 0.0
    }

    fn draw_rectangle(
        &mut self,
        rect: Pin<&dyn i_slint_core::item_rendering::RenderRectangle>,
        _self_rc: &i_slint_core::items::ItemRc,
        size: LogicalSize,
        _cache: &CachedRenderingData,
    ) {
        let geometry = PhysicalRect::from(size * self.scale_factor);
        if geometry.is_empty() {
            return;
        }

        let mut paint = match self.brush_to_paint(
            rect.background(),
            geometry.width_length(),
            geometry.height_length(),
        ) {
            Some(paint) => paint,
            None => return,
        };
        paint.set_anti_alias(self.needs_anti_alias());
        self.canvas.draw_rect(to_skia_rect(&geometry), &paint);
    }

    fn draw_border_rectangle(
        &mut self,
        rect: Pin<&dyn i_slint_core::item_rendering::RenderBorderRectangle>,
        _self_rc: &i_slint_core::items::ItemRc,
        size: LogicalSize,
        _: &CachedRenderingData,
    ) {
        let Some(layout) = BorderRectLayout::new(rect, size, self.scale_factor) else {
            return;
        };
        let brush_width = layout.brush_size.width_length();
        let brush_height = layout.brush_size.height_length();
        let outline = rect.outline();

        if let Some(mut fill_paint) =
            self.brush_to_paint(rect.background(), brush_width, brush_height)
        {
            match &outline {
                i_slint_core::graphics::ElementOutline::Rectangle(..) => {
                    let background_rect =
                        to_skia_rrect(&layout.background_rect, &layout.background_radius);
                    fill_paint.set_style(skia_safe::PaintStyle::Fill);
                    if !background_rect.is_rect() || self.needs_anti_alias() {
                        fill_paint.set_anti_alias(true);
                    }
                    self.canvas.draw_rrect(background_rect, &fill_paint);
                }
                outline => {
                    let path = outline_to_skia_path(outline, layout.background_rect);
                    fill_paint.set_style(skia_safe::PaintStyle::Fill);
                    fill_paint.set_anti_alias(true);
                    self.canvas.draw_path(&path, &fill_paint);
                }
            }
        }

        if layout.border_width.get() > 0.0
            && let Some(mut border_paint) =
                self.brush_to_paint(layout.border_color, brush_width, brush_height)
        {
            match &outline {
                i_slint_core::graphics::ElementOutline::Rectangle(..) => {
                    let border_rect = to_skia_rrect(&layout.border_rect, &layout.border_radius);
                    border_paint.set_style(skia_safe::PaintStyle::Stroke);
                    border_paint.set_stroke_width(layout.border_width.get());
                    if !border_rect.is_rect() || self.needs_anti_alias() {
                        border_paint.set_anti_alias(true);
                    }
                    self.canvas.draw_rrect(border_rect, &border_paint);
                }
                outline => {
                    let path = outline_to_skia_path(outline, layout.border_rect);
                    border_paint.set_style(skia_safe::PaintStyle::Stroke);
                    border_paint.set_stroke_width(layout.border_width.get());
                    border_paint.set_anti_alias(true);
                    self.canvas.draw_path(&path, &border_paint);
                }
            }
        }
    }

    fn draw_window_background(
        &mut self,
        rect: Pin<&dyn i_slint_core::item_rendering::RenderRectangle>,
        _self_rc: &ItemRc,
        _size: LogicalSize,
        _cache: &CachedRenderingData,
    ) {
        // Register a dependency for the partial renderer's dirty tracker. The actual rendering
        // is done earlier in SkiaRenderer, which clears (solid color) or draws (gradient) the
        // background before the item tree is rendered.
        let _ = rect.background();
    }

    fn draw_image(
        &mut self,
        image: Pin<&dyn RenderImage>,
        self_rc: &ItemRc,
        size: LogicalSize,
        _cache: &CachedRenderingData,
    ) {
        let geometry = PhysicalRect::from(size * self.scale_factor);
        if geometry.is_empty() {
            return;
        }
        self.draw_image_impl(self_rc, image, geometry);
    }

    fn draw_text(
        &mut self,
        text: Pin<&dyn RenderText>,
        self_rc: &i_slint_core::items::ItemRc,
        size: LogicalSize,
        _cache: &CachedRenderingData,
    ) {
        let (horizontal, vertical) = text.alignment();
        let anchor = i_slint_core::item_rendering::text_alignment_anchor(
            size * self.scale_factor,
            horizontal,
            vertical,
        );
        let restore = self.save_canvas_and_pixel_align_origin(anchor);
        sharedparley::draw_text(self, text, Some(self_rc), size, Some(self.text_layout_cache));
        if restore {
            self.canvas.restore();
        }
    }

    fn draw_text_input(
        &mut self,
        text_input: Pin<&i_slint_core::items::TextInput>,
        self_rc: &i_slint_core::items::ItemRc,
        size: LogicalSize,
    ) {
        let anchor = i_slint_core::item_rendering::text_alignment_anchor(
            size * self.scale_factor,
            text_input.horizontal_alignment(),
            text_input.vertical_alignment(),
        );
        let restore = self.save_canvas_and_pixel_align_origin(anchor);
        sharedparley::draw_text_input(self, text_input, self_rc, size, self.text_layout_cache);
        if restore {
            self.canvas.restore();
        }
    }

    fn draw_path(
        &mut self,
        path: Pin<&i_slint_core::items::Path>,
        item_rc: &i_slint_core::items::ItemRc,
        size: LogicalSize,
    ) {
        let geometry = PhysicalRect::from(size * self.scale_factor);

        let (physical_offset, skpath): (crate::euclid::Vector2D<f32, PhysicalPx>, _) =
            match self.path_cache.get_or_update_cache_entry(item_rc, || {
                let (logical_offset, path_events): (crate::euclid::Vector2D<f32, LogicalPx>, _) =
                    path.fitted_path_events(item_rc)?;

                let mut builder = skia_safe::PathBuilder::new();
                builder.set_fill_type(match path.effective_fill_rule() {
                    i_slint_core::items::FillRule::Evenodd => skia_safe::PathFillType::EvenOdd,
                    _ => skia_safe::PathFillType::Winding,
                });

                for x in path_events.iter() {
                    match x {
                        lyon_path::Event::Begin { at } => {
                            builder.move_to(to_skia_point(
                                LogicalPoint::from_untyped(at) * self.scale_factor,
                            ));
                        }
                        lyon_path::Event::Line { from: _, to } => {
                            builder.line_to(to_skia_point(
                                LogicalPoint::from_untyped(to) * self.scale_factor,
                            ));
                        }
                        lyon_path::Event::Quadratic { from: _, ctrl, to } => {
                            builder.quad_to(
                                to_skia_point(LogicalPoint::from_untyped(ctrl) * self.scale_factor),
                                to_skia_point(LogicalPoint::from_untyped(to) * self.scale_factor),
                            );
                        }

                        lyon_path::Event::Cubic { from: _, ctrl1, ctrl2, to } => {
                            builder.cubic_to(
                                to_skia_point(
                                    LogicalPoint::from_untyped(ctrl1) * self.scale_factor,
                                ),
                                to_skia_point(
                                    LogicalPoint::from_untyped(ctrl2) * self.scale_factor,
                                ),
                                to_skia_point(LogicalPoint::from_untyped(to) * self.scale_factor),
                            );
                        }
                        lyon_path::Event::End { last: _, first: _, close } => {
                            if close {
                                builder.close();
                            }
                        }
                    }
                }

                (logical_offset * self.scale_factor, builder.detach()).into()
            }) {
                Some(offset_and_path) => offset_and_path,
                None => return,
            };

        self.canvas.translate((physical_offset.x, physical_offset.y));

        let anti_alias = path.anti_alias();

        // For Path elements with conic gradients, we need to handle the viewbox transformation
        let viewbox_width = path.viewbox_width();
        let viewbox_height = path.viewbox_height();

        let paint = if viewbox_width > 0.0 && viewbox_height > 0.0 {
            // If there's a viewbox, we need to create the gradient in viewbox space
            // and then transform it to the actual size
            let scale_x = geometry.width() / viewbox_width;
            let scale_y = geometry.height() / viewbox_height;

            let paint = self.default_paint().unwrap_or_default();
            if let Some((mut paint, shader)) = Self::brush_to_shader(
                paint,
                path.fill(),
                PhysicalLength::new(viewbox_width),
                PhysicalLength::new(viewbox_height),
                ScaleFactor::new(1.0),
            ) {
                // Apply the viewbox transformation to the shader
                let transform = skia_safe::Matrix::scale((scale_x, scale_y));
                paint.set_shader(shader.with_local_matrix(&transform));
                Some(paint)
            } else {
                None
            }
        } else {
            self.brush_to_paint(path.fill(), geometry.width_length(), geometry.height_length())
        };

        if let Some(mut fill_paint) = paint {
            fill_paint.set_anti_alias(anti_alias);
            self.canvas.draw_path(&skpath, &fill_paint);
        }
        if let Some(mut border_paint) =
            self.brush_to_paint(path.stroke(), geometry.width_length(), geometry.height_length())
        {
            border_paint.set_anti_alias(anti_alias);
            border_paint.set_stroke_width((path.stroke_width() * self.scale_factor).get());
            border_paint.set_stroke_cap(match path.stroke_line_cap() {
                i_slint_core::items::LineCap::Round => skia_safe::PaintCap::Round,
                i_slint_core::items::LineCap::Square => skia_safe::PaintCap::Square,
                i_slint_core::items::LineCap::Butt | _ => skia_safe::PaintCap::Butt,
            });
            border_paint.set_stroke_join(match path.stroke_line_join() {
                i_slint_core::items::LineJoin::Round => skia_safe::PaintJoin::Round,
                i_slint_core::items::LineJoin::Bevel => skia_safe::PaintJoin::Bevel,
                i_slint_core::items::LineJoin::Miter | _ => skia_safe::PaintJoin::Miter,
            });
            border_paint.set_stroke_miter(path.stroke_miter_limit());
            border_paint.set_stroke(true);
            self.canvas.draw_path(&skpath, &border_paint);
        }
    }

    fn draw_box_shadow(
        &mut self,
        box_shadow: Pin<&i_slint_core::items::BoxShadow>,
        self_rc: &i_slint_core::items::ItemRc,
        _size: LogicalSize,
    ) {
        let offset = LogicalPoint::from_lengths(box_shadow.offset_x(), box_shadow.offset_y())
            * self.scale_factor;
        let inset = box_shadow.inset();
        let spread = box_shadow.spread() * self.scale_factor;

        // Drop shadow with no offset / blur / spread is invisible.
        if !inset
            && offset.x == 0.
            && offset.y == 0.
            && box_shadow.blur() == LogicalLength::zero()
            && spread == PhysicalLength::zero()
        {
            return;
        }

        let cached_shadow_image = self.box_shadow_cache.get_box_shadow(
            self_rc,
            self.image_cache,
            box_shadow,
            self.scale_factor,
            |shadow_options| {
                if shadow_options.inset {
                    Self::render_inset_shadow_image(self.canvas, shadow_options)
                } else {
                    Self::render_drop_shadow_image(self.canvas, shadow_options)
                }
            },
        );

        let cached_shadow_image = match cached_shadow_image {
            Some(img) => img,
            None => return,
        };

        if inset {
            // Inset image is sized exactly to the geometry; blit at origin.
            self.canvas.draw_image(
                cached_shadow_image,
                skia_safe::Point::new(0., 0.),
                self.default_paint().as_ref(),
            );
        } else {
            let blur = box_shadow.blur() * self.scale_factor;
            let pad = blur.get() + spread.get().max(0.);
            self.canvas.draw_image(
                cached_shadow_image,
                to_skia_point(offset - PhysicalPoint::new(pad, pad).to_vector()),
                self.default_paint().as_ref(),
            );
        }
    }

    fn combine_clip(
        &mut self,
        rect: LogicalRect,
        outline: &i_slint_core::graphics::ElementOutline,
    ) -> bool {
        match outline {
            i_slint_core::graphics::ElementOutline::Rectangle(radius) => {
                let rounded_rect =
                    to_skia_rrect(&(rect * self.scale_factor), &(*radius * self.scale_factor));
                self.canvas.clip_rrect(rounded_rect, None, true);
            }
            outline => {
                let path = outline_to_skia_path(outline, rect * self.scale_factor);
                self.canvas.clip_path(&path, None, true);
            }
        }
        self.canvas.local_clip_bounds().is_some()
    }

    fn get_current_clip(&self) -> LogicalRect {
        from_skia_rect(&self.canvas.local_clip_bounds().unwrap_or_default()) / self.scale_factor
    }

    fn translate(&mut self, distance: LogicalVector) {
        self.current_state.transform = self.current_state.transform.pre_translate(distance.cast());
        let distance = distance * self.scale_factor;
        self.canvas.translate(skia_safe::Vector::from((distance.x, distance.y)));
    }

    fn current_transform(&self) -> i_slint_core::lengths::ItemTransform {
        self.current_state.transform
    }

    /// The elevation shadow of an element via Skia's own
    /// `SkShadowUtils::DrawShadow` — the Android ambient + spot model the
    /// ported `i_slint_core::graphics::shadow` rasterizer mirrors for the
    /// other renderers. The path is fitted into the item's bounds in physical
    /// pixels, matching the physical space the canvas transform operates in;
    /// the light comes back from `elevation_light` in device space, which is
    /// exactly what `DrawShadow` expects for `light_pos`.
    #[allow(clippy::unnecessary_cast)] // Coord
    fn draw_elevation_shadow(
        &mut self,
        shadow_item: Pin<&i_slint_core::items::ElevationShadow>,
        _self_rc: &ItemRc,
        size: LogicalSize,
    ) {
        use i_slint_core::graphics::shadow;

        let geom = LogicalRect::from(size);
        let scale_factor = self.scale_factor.get();
        let z = shadow_item.elevation().get() as f32 * scale_factor;
        if z < shadow::MIN_HEIGHT {
            return;
        }
        let caster_alpha = shadow_item.caster_alpha();
        let ambient_color =
            shadow::effective_ambient_color(shadow_item.ambient_shadow_color(), caster_alpha);
        let spot_color =
            shadow::effective_spot_color(shadow_item.spot_shadow_color(), caster_alpha);
        if ambient_color.alpha() == 0 && spot_color.alpha() == 0 {
            return;
        }
        let outline = shadow_item.element_outline();

        let adapter = WindowInner::from_pub(self.window).window_adapter();
        let (light, light_radius) = shadow::elevation_light(
            adapter.display_geometry(),
            adapter.size(),
            self.window.scale_factor(),
        );

        let path = outline_to_skia_path(&outline, geom * self.scale_factor);
        if path.is_empty() {
            return;
        }

        let flags = (caster_alpha < 1.)
            .then_some(skia_safe::utils::shadow_utils::ShadowFlags::TRANSPARENT_OCCLUDER);
        self.canvas.draw_shadow(
            &path,
            skia_safe::Point3::new(0., 0., z),
            skia_safe::Point3::new(light[0], light[1], light[2]),
            light_radius,
            to_skia_color(&ambient_color),
            to_skia_color(&spot_color),
            flags,
        );
    }

    fn rotate(&mut self, angle_in_degrees: f32) {
        self.current_state.transform =
            self.current_state.transform.pre_rotate(euclid::Angle::degrees(angle_in_degrees));
        self.canvas.rotate(angle_in_degrees, None);
    }

    fn scale(&mut self, x_factor: f32, y_factor: f32) {
        self.current_state.transform = self.current_state.transform.pre_scale(x_factor, y_factor);
        self.canvas.scale((x_factor, y_factor));
    }

    fn apply_opacity(&mut self, opacity: f32) {
        self.current_state.alpha *= opacity;
    }

    fn save_state(&mut self) {
        self.canvas.save();
        self.state_stack.push(self.current_state);
    }

    fn restore_state(&mut self) {
        self.current_state = self.state_stack.pop().unwrap();
        self.canvas.restore();
    }

    fn scale_factor(&self) -> ScaleFactor {
        self.scale_factor
    }

    fn draw_cached_pixmap(
        &mut self,
        item_rc: &i_slint_core::items::ItemRc,
        update_fn: &dyn Fn(&mut dyn FnMut(u32, u32, &[u8])),
    ) {
        let skia_image = self.image_cache.get_or_update_cache_entry(item_rc, || {
            let mut cached_image = None;
            update_fn(&mut |width: u32, height: u32, data: &[u8]| {
                let image_info = crate::image_info(
                    skia_safe::ISize::new(width as i32, height as i32),
                    skia_safe::ColorType::RGBA8888,
                    skia_safe::AlphaType::Premul,
                );
                cached_image = skia_safe::images::raster_from_data(
                    &image_info,
                    skia_safe::Data::new_copy(data),
                    width as usize * 4,
                );
            });
            cached_image
        });
        let skia_image = match skia_image {
            Some(img) => img,
            None => return,
        };
        let _saved_canvas = self.pixel_align_origin_auto_restore();
        self.canvas.draw_image(skia_image, skia_safe::Point::default(), None);
    }

    fn draw_string(&mut self, string: &str, color: i_slint_core::Color) {
        sharedparley::draw_text(
            self,
            std::pin::pin!((SharedString::from(string), Brush::from(color))),
            None,
            logical_size_from_api(self.window.size().to_logical(self.scale_factor().get())),
            None,
        );
    }

    fn draw_image_direct(&mut self, image: i_slint_core::graphics::Image) {
        let skia_image = super::cached_image::as_skia_image(
            image.clone(),
            &|| LogicalSize::from_untyped(image.size().cast()),
            ImageFit::Fill,
            self.scale_factor,
            self.canvas,
            self.surface,
        );

        let skia_image = match skia_image {
            Some(img) => img,
            None => return,
        };

        self.canvas.draw_image(
            skia_image,
            skia_safe::Point::default(),
            self.default_paint().as_ref(),
        );
    }

    fn window(&self) -> &i_slint_core::window::WindowInner {
        i_slint_core::window::WindowInner::from_pub(self.window)
    }

    fn as_any(&mut self) -> Option<&mut dyn core::any::Any> {
        None
    }

    fn visit_opacity(
        &mut self,
        opacity_item: Pin<&Opacity>,
        item_rc: &ItemRc,
        _size: LogicalSize,
    ) -> RenderingResult {
        let opacity = opacity_item.opacity();
        if Opacity::need_layer(item_rc, opacity) {
            self.canvas.save_layer_alpha(None, (opacity * 255.) as u32);
            self.state_stack.push(self.current_state);
            self.current_state.alpha = 1.0;

            let window_adapter = WindowInner::from_pub(self.window).window_adapter();

            i_slint_core::item_rendering::render_item_children(
                self,
                item_rc.item_tree(),
                item_rc.index() as isize,
                &window_adapter,
            );

            self.current_state = self.state_stack.pop().unwrap();
            self.canvas.restore();
            RenderingResult::ContinueRenderingWithoutChildren
        } else {
            self.apply_opacity(opacity);
            RenderingResult::ContinueRenderingChildren
        }
    }

    fn visit_layer(
        &mut self,
        layer_item: Pin<&Layer>,
        self_rc: &ItemRc,
        _size: LogicalSize,
    ) -> RenderingResult {
        if layer_item.cache_rendering_hint() {
            self.render_and_blend_layer(self_rc)
        } else {
            self.image_cache.release(self_rc);
            RenderingResult::ContinueRenderingChildren
        }
    }
}

impl<'a> LayerRenderer<'a> for SkiaItemRenderer<'a> {
    type LayerTarget = skia_safe::Surface;
    type Image = skia_safe::Image;

    fn layer_cache(&self) -> &'a ItemCache<Option<(PhysicalPoint, Self::Image)>> {
        self.layer_cache
    }

    fn create_layer_target(
        &mut self,
        _item_rc: &ItemRc,
        physical_size: euclid::Size2D<f32, PhysicalPx>,
    ) -> Option<Self::LayerTarget> {
        let image_info = crate::image_info(
            to_skia_size(&physical_size).to_ceil(),
            skia_safe::ColorType::RGBA8888,
            skia_safe::AlphaType::Premul,
        );
        self.canvas.new_surface(&image_info, None)
    }

    fn render_into_layer(
        &mut self,
        mut surface: Self::LayerTarget,
        item_rc: &ItemRc,
        bounding_rect: LogicalRect,
    ) -> Self::Image {
        let canvas = surface.canvas();
        canvas.clear(skia_safe::Color::TRANSPARENT);

        let mut sub_renderer = SkiaItemRenderer::new(
            canvas,
            self.window,
            self.surface,
            self.image_cache,
            self.layer_cache,
            self.path_cache,
            self.text_layout_cache,
            self.box_shadow_cache,
        );
        sub_renderer.translate(-bounding_rect.origin.to_vector());

        i_slint_core::item_rendering::render_item_children(
            &mut sub_renderer,
            item_rc.item_tree(),
            item_rc.index() as isize,
            &WindowInner::from_pub(self.window).window_adapter(),
        );

        surface.image_snapshot()
    }
}

impl GlyphRenderer for SkiaItemRenderer<'_> {
    type PlatformBrush = skia_safe::Paint;

    fn platform_text_fill_brush(
        &mut self,
        brush: i_slint_core::Brush,
        size: LogicalSize,
    ) -> Option<Self::PlatformBrush> {
        self.brush_to_paint(
            brush,
            size.width_length() * self.scale_factor,
            size.height_length() * self.scale_factor,
        )
    }

    fn platform_brush_for_color(
        &mut self,
        color: &i_slint_core::Color,
    ) -> Option<Self::PlatformBrush> {
        if color.alpha() == 0 {
            None
        } else {
            let mut paint = self.default_paint().unwrap_or_default();
            paint.set_shader(crate::color_shader(color));
            Some(paint)
        }
    }

    fn platform_text_stroke_brush(
        &mut self,
        brush: i_slint_core::Brush,
        physical_stroke_width: f32,
        size: LogicalSize,
    ) -> Option<Self::PlatformBrush> {
        match self.brush_to_paint(
            brush.clone(),
            size.width_length() * self.scale_factor,
            size.height_length() * self.scale_factor,
        ) {
            Some(mut stroke_paint) => {
                stroke_paint.set_style(skia_safe::PaintStyle::Stroke);
                stroke_paint.set_stroke_width(physical_stroke_width);
                // Set stroke cap/join/miter to match FemtoVG
                stroke_paint.set_stroke_cap(skia_safe::PaintCap::Butt);
                stroke_paint.set_stroke_join(skia_safe::PaintJoin::Miter);
                stroke_paint.set_stroke_miter(10.0);
                Some(stroke_paint)
            }
            None => None,
        }
    }

    fn snap_selection_x(&self, x: f32) -> f32 {
        let transform = self.canvas.local_to_device_as_3x3();
        if !transform.is_translate() {
            return x;
        }
        let origin = transform.map_point(skia_safe::Point::default()).x;
        (origin + x).round() - origin
    }

    fn draw_glyph_run(
        &mut self,
        font: &sharedparley::parley::FontData,
        font_size: PhysicalLength,
        _normalized_coords: &[i16],
        synthesis: &fontique::Synthesis,
        variations: &[sharedparley::parley::style::FontVariation],
        brush: Self::PlatformBrush,
        y_offset: sharedparley::PhysicalLength,
        glyphs_it: &mut dyn Iterator<Item = sharedparley::parley::layout::Glyph>,
    ) {
        let Some(type_face) = crate::font_cache::FONT_CACHE.with_borrow_mut(|font_cache| {
            font_cache.font_with_variations(font, synthesis, variations)
        }) else {
            // The typeface can't take the run's variation arguments (or failed
            // to load): rasterize glyph outlines at the exact coordinates the
            // shaper used rather than silently dropping the requested axes.
            let variation_settings = sharedparley::merged_variation_settings(synthesis, variations);
            let glyph_paths: Vec<_> = glyphs_it
                .map(|g| {
                    (
                        g.id,
                        g.x,
                        g.y + y_offset.get(),
                        crate::font_cache::FONT_CACHE.with_borrow_mut(|font_cache| {
                            font_cache.glyph_path(font, g.id, font_size.get(), &variation_settings)
                        }),
                    )
                })
                .collect();
            for (_glyph_id, x, y, path) in glyph_paths {
                if let Some(path) = path {
                    self.canvas.draw_path(&path.make_offset((x, y)), &brush);
                }
            }
            return;
        };
        let mut font = skia_safe::Font::from_typeface(type_face, font_size.get());
        font.set_subpixel(true);
        // The typeface itself is cached (keyed on blob + variation settings, not synthesis), so
        // faux styling has to be applied to this per-draw-call `Font` instead: skewing or
        // emboldening the cached typeface would leak into every other run drawn with it.
        if synthesis.embolden() {
            font.set_embolden(true);
        }
        if let Some(skew_degrees) = synthesis.skew() {
            // Skia skews text left/right relative to the y-axis; a negative skew leans glyphs
            // to the right, matching the forward lean of real italic/oblique faces.
            font.set_skew_x(-skew_degrees.to_radians().tan());
        }

        let (glyph_ids, glyph_positions): (Vec<_>, Vec<_>) = glyphs_it
            .into_iter()
            .map(|g| (g.id as skia_safe::GlyphId, skia_safe::Point::new(g.x, g.y + y_offset.get())))
            .unzip();

        self.canvas.draw_glyphs_at(
            &glyph_ids,
            skia_safe::canvas::GlyphPositions::Points(&glyph_positions),
            skia_safe::Point::default(),
            &font,
            &brush,
        );
    }

    fn fill_rectangle(
        &mut self,
        physical_rect: sharedparley::PhysicalRect,
        paint: Self::PlatformBrush,
        radius: sharedparley::PhysicalLength,
        border: Option<sharedparley::RectangleBorder<Self::PlatformBrush>>,
    ) {
        let rect = skia_safe::Rect::from_xywh(
            physical_rect.min_x(),
            physical_rect.min_y(),
            physical_rect.width(),
            physical_rect.height(),
        );

        if radius.get() <= 0.0 && border.is_none() {
            self.canvas.draw_rect(rect, &paint);
            return;
        }

        let rrect = skia_safe::RRect::new_rect_xy(rect, radius.get(), radius.get());
        self.canvas.draw_rrect(rrect, &paint);

        if let Some(sharedparley::RectangleBorder { brush: mut stroke_paint, width }) = border
            && width.get() > 0.0
        {
            stroke_paint.set_style(skia_safe::PaintStyle::Stroke);
            stroke_paint.set_stroke_width(width.get());
            stroke_paint.set_anti_alias(true);
            self.canvas.draw_rrect(rrect, &stroke_paint);
        }
    }
}

pub fn from_skia_rect(rect: &skia_safe::Rect) -> PhysicalRect {
    let top_left = euclid::Point2D::new(rect.left, rect.top);
    let bottom_right = euclid::Point2D::new(rect.right, rect.bottom);
    euclid::Box2D::new(top_left, bottom_right).to_rect()
}

pub fn to_skia_rect(rect: &PhysicalRect) -> skia_safe::Rect {
    skia_safe::Rect::from_xywh(rect.origin.x, rect.origin.y, rect.size.width, rect.size.height)
}

/// The outline's vector path fitted into `target` (physical pixels), for the
/// [`i_slint_core::graphics::ElementOutline::Shape`] variant; rounded
/// rectangles keep their [`to_skia_rrect`] fast path instead.
pub fn outline_to_skia_path(
    outline: &i_slint_core::graphics::ElementOutline,
    target: PhysicalRect,
) -> skia_safe::Path {
    let mut builder = skia_safe::PathBuilder::new();
    outline.for_each_path(target, &mut |el| match el {
        i_slint_core::graphics::OutlinePathEl::MoveTo(p) => {
            builder.move_to(skia_safe::Point::new(p.x, p.y));
        }
        i_slint_core::graphics::OutlinePathEl::LineTo(p) => {
            builder.line_to(skia_safe::Point::new(p.x, p.y));
        }
        i_slint_core::graphics::OutlinePathEl::CurveTo(c0, c1, p) => {
            builder.cubic_to(
                skia_safe::Point::new(c0.x, c0.y),
                skia_safe::Point::new(c1.x, c1.y),
                skia_safe::Point::new(p.x, p.y),
            );
        }
        i_slint_core::graphics::OutlinePathEl::Close => {
            builder.close();
        }
    });
    builder.set_fill_type(match outline.fill_rule() {
        i_slint_core::items::FillRule::Evenodd => skia_safe::PathFillType::EvenOdd,
        _ => skia_safe::PathFillType::Winding,
    });
    builder.detach()
}

pub fn to_skia_rrect(rect: &PhysicalRect, radius: &PhysicalBorderRadius) -> skia_safe::RRect {
    if let Some(radius) = radius.as_uniform() {
        skia_safe::RRect::new_rect_xy(to_skia_rect(rect), radius, radius)
    } else {
        skia_safe::RRect::new_rect_radii(
            to_skia_rect(rect),
            &[
                skia_safe::Point::new(radius.top_left, radius.top_left),
                skia_safe::Point::new(radius.top_right, radius.top_right),
                skia_safe::Point::new(radius.bottom_right, radius.bottom_right),
                skia_safe::Point::new(radius.bottom_left, radius.bottom_left),
            ],
        )
    }
}

impl ItemRendererFeatures for SkiaItemRenderer<'_> {
    const SUPPORTS_TRANSFORMATIONS: bool = true;
}

pub fn to_skia_point(point: PhysicalPoint) -> skia_safe::Point {
    skia_safe::Point::new(point.x, point.y)
}

pub fn to_skia_size(size: &PhysicalSize) -> skia_safe::Size {
    skia_safe::Size::new(size.width, size.height)
}

/// The result is gamma encoded sRGB, like the `slint::Color` it comes from.
fn to_skia_stops(
    stops: &[i_slint_core::graphics::GradientStop],
) -> (Vec<skia_safe::Color4f>, Vec<f32>) {
    stops.iter().map(|s| (to_skia_color4f(&s.color), s.position)).unzip()
}

pub fn to_skia_color(col: &Color) -> skia_safe::Color {
    skia_safe::Color::from_argb(col.alpha(), col.red(), col.green(), col.blue())
}

/// Gamma encoded sRGB as well, see [`to_skia_color`].
pub fn to_skia_color4f(col: &Color) -> skia_safe::Color4f {
    to_skia_color(col).into()
}

#[cfg(test)]
mod shadow_parity_tests {
    //! The ported `i_slint_core::graphics::shadow` rasterizer vs. the native
    //! `SkShadowUtils::DrawShadow` oracle the Skia renderer draws with. Both
    //! render the Android ambient + spot model; this draws each into a raster
    //! surface and compares the pixels (the caster is composited on top of
    //! both, as in a real frame, so umbra dropped under an opaque caster
    //! doesn't count as a difference).
    use super::*;
    use i_slint_core::graphics::ElementOutline;
    use i_slint_core::graphics::shadow::{self, Affine};
    use i_slint_core::items::ShapeFit;
    use i_slint_core::lengths::LogicalBorderRadius;

    const W: i32 = 480;
    const H: i32 = 360;

    fn snapshot(surface: &mut skia_safe::Surface) -> Vec<u8> {
        surface
            .image_snapshot()
            .peek_pixels()
            .and_then(|p| p.bytes().map(<[u8]>::to_vec))
            .unwrap_or_default()
    }

    fn clear(surface: &mut skia_safe::Surface) {
        surface.canvas().clear(skia_safe::Color::WHITE);
    }

    /// The ported rasterizer's draw: tint each mask layer and composite it
    /// snapped to whole device pixels.
    fn draw_ported(
        surface: &mut skia_safe::Surface,
        outline: &ElementOutline,
        geom: LogicalRect,
        ctm: &Affine,
        z: f32,
        light: [f32; 3],
        light_radius: f32,
        caster_alpha: f32,
        ambient: i_slint_core::graphics::Color,
        spot: i_slint_core::graphics::Color,
    ) {
        let masks = shadow::elevation_shadow_masks(
            outline,
            geom,
            ctm,
            z,
            light,
            light_radius,
            caster_alpha < 1.,
        );
        let canvas = surface.canvas();
        for (layer, color) in [(masks.ambient, ambient), (masks.spot, spot)].into_iter() {
            let Some(layer) = layer else { continue };
            if color.alpha() == 0 {
                continue;
            }
            let data: Vec<u8> = layer
                .mask
                .iter()
                .flat_map(|&a| {
                    let alpha = (u16::from(a) * u16::from(color.alpha()) + 127) / 255;
                    [
                        (u16::from(color.red()) * alpha + 127) / 255,
                        (u16::from(color.green()) * alpha + 127) / 255,
                        (u16::from(color.blue()) * alpha + 127) / 255,
                        alpha,
                    ]
                    .map(|v| v as u8)
                })
                .collect();
            let image_info = crate::image_info(
                skia_safe::ISize::new(layer.size.width as i32, layer.size.height as i32),
                skia_safe::ColorType::RGBA8888,
                skia_safe::AlphaType::Premul,
            );
            let Some(image) = skia_safe::images::raster_from_data(
                &image_info,
                skia_safe::Data::new_copy(&data),
                layer.size.width as usize * 4,
            ) else {
                continue;
            };
            canvas.save();
            canvas.reset_matrix();
            canvas.draw_image(
                image,
                skia_safe::Point::new(layer.rect.origin.x.round(), layer.rect.origin.y.round()),
                None,
            );
            canvas.restore();
        }
    }

    /// The native draw, matching `draw_elevation_shadow`: the path in
    /// physical local space, `canvas_ctm` mapping physical local to device
    /// space, z/light/radius in device pixels.
    fn draw_native(
        surface: &mut skia_safe::Surface,
        outline: &ElementOutline,
        geom_phys: PhysicalRect,
        canvas_ctm: &skia_safe::Matrix,
        z: f32,
        light: [f32; 3],
        light_radius: f32,
        caster_alpha: f32,
        ambient: i_slint_core::graphics::Color,
        spot: i_slint_core::graphics::Color,
    ) {
        let canvas = surface.canvas();
        canvas.save();
        canvas.set_matrix(&(*canvas_ctm).into());
        let path = outline_to_skia_path(outline, geom_phys);
        let flags = (caster_alpha < 1.)
            .then_some(skia_safe::utils::shadow_utils::ShadowFlags::TRANSPARENT_OCCLUDER);
        canvas.draw_shadow(
            &path,
            skia_safe::Point3::new(0., 0., z),
            skia_safe::Point3::new(light[0], light[1], light[2]),
            light_radius,
            to_skia_color(&ambient),
            to_skia_color(&spot),
            flags,
        );
        canvas.restore();
    }

    /// Composite the caster silhouette over the shadow, as the real frame
    /// does — the umbra under an opaque caster must not count as a
    /// difference.
    fn draw_caster(
        surface: &mut skia_safe::Surface,
        outline: &ElementOutline,
        geom_phys: PhysicalRect,
        canvas_ctm: &skia_safe::Matrix,
        alpha: f32,
    ) {
        let canvas = surface.canvas();
        canvas.save();
        canvas.set_matrix(&(*canvas_ctm).into());
        let path = outline_to_skia_path(outline, geom_phys);
        let mut paint = skia_safe::Paint::default();
        paint.set_anti_alias(true);
        paint.set_color(skia_safe::Color::from_argb((alpha * 255.) as u8, 255, 255, 255));
        canvas.draw_path(&path, &paint);
        canvas.restore();
    }

    fn to_skia_matrix(a: &Affine) -> skia_safe::Matrix {
        // SkMatrix order: scaleX, skewX, transX, skewY, scaleY, transY —
        // the affine's m21/m12 swap places.
        skia_safe::Matrix::new_all(a.m11, a.m21, a.tx, a.m12, a.m22, a.ty, 0., 0., 1.)
    }

    /// Per-pixel comparison over premultiplied RGBA buffers.
    fn compare(name: &str, ported: &[u8], native: &[u8]) {
        assert_eq!(ported.len(), native.len());
        let n = ported.len() / 4;
        let mut sum = 0u64;
        let mut max = 0u8;
        let mut big = 0usize;
        let mut ported_alpha = 0usize;
        let mut native_alpha = 0usize;
        for i in 0..n {
            let d = (0..4)
                .map(|c| (ported[i * 4 + c] as i32 - native[i * 4 + c] as i32).unsigned_abs() as u8)
                .max()
                .unwrap_or(0);
            sum += d as u64;
            max = max.max(d);
            if d > 64 {
                big += 1;
            }
            // The Android model's alphas are low (ambient ≈ 10/255, spot ≈
            // 48/255), so any darkening past quantization noise counts.
            if 255 - ported[i * 4].min(ported[i * 4 + 1]).min(ported[i * 4 + 2]) > 8 {
                ported_alpha += 1;
            }
            if 255 - native[i * 4].min(native[i * 4 + 1]).min(native[i * 4 + 2]) > 8 {
                native_alpha += 1;
            }
        }
        let mean = sum as f64 / n as f64;
        eprintln!(
            "{name}: mean={mean:.2} max={max} big(>64)={big} ported_shadow={ported_alpha} native_shadow={native_alpha}"
        );
        // Sanity: each pipeline produced a visible shadow at all.
        assert!(ported_alpha > 50, "{name}: ported shadow empty ({ported_alpha})");
        assert!(native_alpha > 50, "{name}: native shadow empty ({native_alpha})");
        // The two rasterizers quantize coverage differently (4×4 supersample
        // vs. tessellated vertices): individual pixels along the penumbra
        // ramp disagree, but the average must be tight and large misses rare.
        assert!(mean < 12., "{name}: mean diff {mean}");
        assert!((big as f64) < n as f64 * 0.02, "{name}: {big} pixels differ by >64");
    }

    struct Case {
        name: &'static str,
        outline: ElementOutline,
        /// Item size in logical px.
        w: f32,
        h: f32,
        /// Logical transform (translate/rotate/scale).
        xf: Affine,
        sf: f32,
        /// Elevation in logical px.
        elevation: f32,
        caster_alpha: f32,
    }

    fn run(case: &Case) {
        let Case { name, ref outline, w, h, xf, sf, elevation, caster_alpha } = *case;

        // Ported ctm: logical→device — elementwise × sf.
        let ctm =
            Affine::new(xf.m11 * sf, xf.m12 * sf, xf.m21 * sf, xf.m22 * sf, xf.tx * sf, xf.ty * sf);
        // Canvas ctm for the native path: physical-local→device — linear part
        // of the logical transform, translations scaled.
        let canvas_ctm = Affine::new(xf.m11, xf.m12, xf.m21, xf.m22, xf.tx * sf, xf.ty * sf);
        let canvas_matrix = to_skia_matrix(&canvas_ctm);

        let z = elevation * sf;
        let (light, light_radius) = shadow::elevation_light(
            None,
            i_slint_core::api::PhysicalSize::new((W as f32 * sf) as u32, (H as f32 * sf) as u32),
            sf,
        );
        let ambient = shadow::effective_ambient_color(Color::from_rgb_u8(0, 0, 0), caster_alpha);
        let spot = shadow::effective_spot_color(Color::from_rgb_u8(0, 0, 0), caster_alpha);
        let geom = euclid::rect(0., 0., w, h);
        let geom_phys =
            PhysicalRect::new(PhysicalPoint::new(0., 0.), PhysicalSize::new(w * sf, h * sf));

        let mut s_ported = skia_safe::surfaces::raster_n32_premul((W, H)).unwrap();
        clear(&mut s_ported);
        draw_ported(
            &mut s_ported,
            &outline,
            geom,
            &ctm,
            z,
            light,
            light_radius,
            caster_alpha,
            ambient,
            spot,
        );
        draw_caster(&mut s_ported, &outline, geom_phys, &canvas_matrix, caster_alpha);
        let p = snapshot(&mut s_ported);

        let mut s_native = skia_safe::surfaces::raster_n32_premul((W, H)).unwrap();
        clear(&mut s_native);
        draw_native(
            &mut s_native,
            &outline,
            geom_phys,
            &canvas_matrix,
            z,
            light,
            light_radius,
            caster_alpha,
            ambient,
            spot,
        );
        draw_caster(&mut s_native, &outline, geom_phys, &canvas_matrix, caster_alpha);
        let n = snapshot(&mut s_native);

        compare(name, &p, &n);
    }

    fn rect_outline() -> ElementOutline {
        ElementOutline::Rectangle(LogicalBorderRadius::default())
    }

    fn rounded_outline(r: f32) -> ElementOutline {
        ElementOutline::Rectangle(LogicalBorderRadius::new_uniform(r))
    }

    fn shape_outline(s: i_slint_core::graphics::Shape) -> ElementOutline {
        ElementOutline::Shape { shape: s, fit: ShapeFit::Fill }
    }

    #[test]
    fn elevation_shadow_port_matches_native_skia() {
        use i_slint_core::graphics::shapes;

        let star =
            shape_outline(shapes::star_shape(4, 0.45, Default::default(), Default::default()));
        let circle = shape_outline(shapes::circle_shape(64));

        let cases = [
            // Convex casters, several elevations.
            Case {
                name: "rect z4 d1",
                outline: rect_outline(),
                w: 96.,
                h: 64.,
                xf: Affine::new(1., 0., 0., 1., 80., 90.),
                sf: 1.,
                elevation: 4.,
                caster_alpha: 1.,
            },
            Case {
                name: "rect z16 d1",
                outline: rect_outline(),
                w: 96.,
                h: 64.,
                xf: Affine::new(1., 0., 0., 1., 80., 90.),
                sf: 1.,
                elevation: 16.,
                caster_alpha: 1.,
            },
            Case {
                name: "rrect z24 d1",
                outline: rounded_outline(14.),
                w: 120.,
                h: 80.,
                xf: Affine::new(1., 0., 0., 1., 200., 140.),
                sf: 1.,
                elevation: 24.,
                caster_alpha: 1.,
            },
            // Concave caster.
            Case {
                name: "star z8 d1",
                outline: star.clone(),
                w: 110.,
                h: 110.,
                xf: Affine::new(1., 0., 0., 1., 300., 60.),
                sf: 1.,
                elevation: 8.,
                caster_alpha: 1.,
            },
            Case {
                name: "star z8 d1 translucent",
                outline: star.clone(),
                w: 110.,
                h: 110.,
                xf: Affine::new(1., 0., 0., 1., 300., 200.),
                sf: 1.,
                elevation: 8.,
                caster_alpha: 0.6,
            },
            // Convex shape + translucent.
            Case {
                name: "circle z12 d1 translucent",
                outline: circle.clone(),
                w: 90.,
                h: 90.,
                xf: Affine::new(1., 0., 0., 1., 140., 220.),
                sf: 1.,
                elevation: 12.,
                caster_alpha: 0.6,
            },
            // Transform: rotate + non-uniform scale.
            Case {
                name: "rect z8 rotated",
                outline: rect_outline(),
                w: 96.,
                h: 64.,
                xf: Affine::new(0.985, 0.174, -0.191, 1.078, 60., 60.),
                sf: 1.,
                elevation: 8.,
                caster_alpha: 1.,
            },
            Case {
                name: "star z12 rotated",
                outline: star.clone(),
                w: 100.,
                h: 100.,
                xf: Affine::new(0.899, 0.416, -0.416, 0.899, 250., 180.),
                sf: 1.,
                elevation: 12.,
                caster_alpha: 1.,
            },
            // Density 2.
            Case {
                name: "rect z8 d2",
                outline: rect_outline(),
                w: 96.,
                h: 64.,
                xf: Affine::new(1., 0., 0., 1., 40., 40.),
                sf: 2.,
                elevation: 8.,
                caster_alpha: 1.,
            },
            Case {
                name: "star z16 d2 translucent",
                outline: star.clone(),
                w: 90.,
                h: 90.,
                xf: Affine::new(1., 0., 0., 1., 90., 150.),
                sf: 2.,
                elevation: 16.,
                caster_alpha: 0.6,
            },
            Case {
                name: "circle z6 d2",
                outline: circle.clone(),
                w: 70.,
                h: 70.,
                xf: Affine::new(1., 0., 0., 1., 160., 40.),
                sf: 2.,
                elevation: 6.,
                caster_alpha: 1.,
            },
            // Fractional origin.
            Case {
                name: "rect z8 frac",
                outline: rect_outline(),
                w: 96.,
                h: 64.,
                xf: Affine::new(1., 0., 0., 1., 63.7, 88.4),
                sf: 1.,
                elevation: 8.,
                caster_alpha: 1.,
            },
        ];

        for case in &cases {
            run(case);
        }
    }
}
