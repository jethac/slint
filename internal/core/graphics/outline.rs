// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Element outlines: turning the `shape`/`shape-fit` properties into concrete
//! geometry. Every consumer of an element's shape — background, border, clip,
//! shadows, hit-testing, accessibility bounds — goes through the types defined
//! here, so a `shape` behaves identically in each place, and `border-radius`
//! is a special case of the same machinery.

use crate::graphics::Shape;
use crate::items::{FillRule, ShapeFit};
use crate::lengths::{LogicalBorderRadius, LogicalPoint, LogicalPx, LogicalRect, LogicalSize};
use alloc::vec::Vec;
#[cfg(not(feature = "std"))]
use num_traits::Float;

/// A point of a flattened outline, in item-local logical coordinates.
///
/// Outline coordinates stay `f32` even when `Coord` is integer-quantized
/// (`slint_int_coord` builds): rounding the outline to logical pixels would
/// lose the sub-pixel detail the rasterizers need.
pub type OutlinePoint = euclid::Point2D<f32, LogicalPx>;

/// An item's shape boundary, as returned by `ItemVTable::boundary_shape`.
///
/// An empty [`Shape`] means the item is bounded by its rectangular geometry,
/// rounded by `radius`.
#[repr(C)]
#[derive(Clone, Default)]
pub struct ItemBoundaryShape {
    /// The item's `shape`.
    pub shape: Shape,
    /// How to fit the shape into the item's bounds.
    pub fit: ShapeFit,
    /// The corner radius of the rectangle the item is bounded by when `shape`
    /// is empty.
    pub radius: LogicalBorderRadius,
}

impl From<ElementOutline> for ItemBoundaryShape {
    fn from(outline: ElementOutline) -> Self {
        match outline {
            ElementOutline::Rectangle(radius) => Self { radius, ..Default::default() },
            ElementOutline::Shape { shape, fit } => Self { shape, fit, ..Default::default() },
        }
    }
}

/// An element's outline: either a (possibly rounded) rectangle, or a `shape`
/// fitted into the element's bounds.
#[derive(Clone, Debug)]
pub enum ElementOutline {
    /// A plain or rounded rectangle.
    Rectangle(LogicalBorderRadius),
    /// An arbitrary shape.
    Shape {
        /// The shape, in its own coordinate space.
        shape: Shape,
        /// How the shape's bounding box maps onto the element's bounds.
        fit: ShapeFit,
    },
}

/// The affine map from a shape's own coordinates into an element's coordinate
/// space, produced by [`ElementOutline::shape_transform`].
#[derive(Copy, Clone, Debug, Default)]
pub struct ShapeFitTransform {
    /// Horizontal scale.
    pub sx: f32,
    /// Vertical scale.
    pub sy: f32,
    /// Horizontal translation applied after scaling.
    pub tx: f32,
    /// Vertical translation applied after scaling.
    pub ty: f32,
}

impl ShapeFitTransform {
    /// Maps a point in shape coordinates into the element's coordinates.
    #[inline]
    pub fn transform(&self, x: f32, y: f32) -> OutlinePoint {
        OutlinePoint::new(x * self.sx + self.tx, y * self.sy + self.ty)
    }

    /// Whether the transform scales uniformly in both directions.
    pub fn is_uniform(&self) -> bool {
        (self.sx - self.sy).abs() <= 1e-6 * self.sx.abs().max(self.sy.abs())
    }

    /// The maximum of the two scale factors, used to convert a stroke width
    /// measured in element space into a width in shape space (or the other way
    /// around). Non-uniform `fill` fits can't express this exactly; the larger
    /// axis keeps the width at least as wide as requested.
    pub fn max_scale(&self) -> f32 {
        self.sx.abs().max(self.sy.abs())
    }
}

/// The transform fitting `shape`'s bounding box into `target`, or `None` for
/// degenerate shapes or target sizes.
pub fn shape_fit_transform<U>(
    shape: &Shape,
    target: euclid::Rect<f32, U>,
    fit: ShapeFit,
) -> Option<ShapeFitTransform> {
    let [x0, y0, x1, y1] = shape.bounds(false)?;
    let bw = x1 - x0;
    let bh = y1 - y0;
    if !(bw > 0.) || !(bh > 0.) {
        return None;
    }
    let w = target.width() as f32;
    let h = target.height() as f32;
    if !(w > 0.) || !(h > 0.) {
        return None;
    }
    let tx0 = target.origin.x as f32;
    let ty0 = target.origin.y as f32;
    let (sx, sy) = match fit {
        ShapeFit::Fill => (w / bw, h / bh),
        ShapeFit::Contain => {
            let s = (w / bw).min(h / bh);
            (s, s)
        }
        ShapeFit::Cover => {
            let s = (w / bw).max(h / bh);
            (s, s)
        }
    };
    let dw = w - bw * sx;
    let dh = h - bh * sy;
    Some(ShapeFitTransform { sx, sy, tx: tx0 + dw / 2. - x0 * sx, ty: ty0 + dh / 2. - y0 * sy })
}

impl ElementOutline {
    /// The outline of an element from its `shape`/`shape-fit` and corner
    /// radius: a shape is used when set, the rounded rectangle otherwise.
    pub fn new(shape: Shape, fit: ShapeFit, radius: LogicalBorderRadius) -> Self {
        if shape.is_empty() { Self::Rectangle(radius) } else { Self::Shape { shape, fit } }
    }

    /// The shape and its fit, or `None` for a rectangular outline.
    pub fn shape(&self) -> Option<(&Shape, ShapeFit)> {
        match self {
            Self::Rectangle(..) => None,
            Self::Shape { shape, fit } => Some((shape, *fit)),
        }
    }

    /// The corner radius for rectangular outlines. `0` when this is a shape.
    pub fn radius(&self) -> LogicalBorderRadius {
        match self {
            Self::Rectangle(radius) => *radius,
            Self::Shape { .. } => Default::default(),
        }
    }

    /// The fill rule a renderer uses to fill this outline: the shape's own
    /// rule, or `nonzero` for rectangles.
    pub fn fill_rule(&self) -> FillRule {
        match self {
            Self::Rectangle(..) => FillRule::Nonzero,
            Self::Shape { shape, .. } => shape.fill_rule(),
        }
    }

    /// Whether the outline is a plain rectangle without rounded corners —
    /// the case every renderer's fast path already handles.
    pub fn is_plain_rect(&self) -> bool {
        matches!(self, Self::Rectangle(radius) if radius.is_zero())
    }

    /// Flattens the outline fitted into `target` into closed polyline contours
    /// (in `target`'s coordinate space). `tolerance` is the maximum distance
    /// in logical pixels between the outline and the approximation.
    ///
    /// Returns an empty `Vec` when the shape or `target` is degenerate.
    pub fn flatten<U>(
        &self,
        target: euclid::Rect<f32, U>,
        tolerance: f32,
    ) -> Vec<Vec<OutlinePoint>> {
        match self {
            Self::Rectangle(radius) => {
                flatten_rounded_rectangle(target, *radius, tolerance).into_iter().collect()
            }
            Self::Shape { shape, fit } => {
                let Some(transform) = shape_fit_transform(shape, target, *fit) else {
                    return Vec::new();
                };
                let cubics = shape.cubics();
                if cubics.is_empty() {
                    return Vec::new();
                }
                let mut contour = Vec::with_capacity(cubics.len() / 8 * 4);
                let mut p0 = transform.transform(cubics[0], cubics[1]);
                contour.push(p0);
                for c in cubics.chunks_exact(8) {
                    let c0 = transform.transform(c[2], c[3]);
                    let c1 = transform.transform(c[4], c[5]);
                    let p1 = transform.transform(c[6], c[7]);
                    push_flattened_cubic(p0, c0, c1, p1, tolerance, &mut contour, 0);
                    p0 = p1;
                }
                alloc::vec![contour]
            }
        }
    }

    /// Whether `point` (in `target`'s coordinate space) lies inside this
    /// outline fitted into `target`.
    pub fn hit_test<U>(&self, target: euclid::Rect<f32, U>, point: OutlinePoint) -> bool {
        match self {
            Self::Rectangle(radius) => {
                if !target.contains(euclid::point2(point.x, point.y)) {
                    return false;
                }
                // The corners are rounded: a point in the rounded corner's
                // bounding square is inside only if it is within the corner
                // circle.
                let radius = *radius;
                if radius.is_zero() {
                    return true;
                }
                point_in_rounded_rectangle(target, radius, point)
            }
            Self::Shape { .. } => point_in_contours(
                &self.flatten(target, HIT_TEST_TOLERANCE),
                self.fill_rule(),
                point,
            ),
        }
    }

    /// Visit the outline's vector path fitted into `target`, as a sequence of
    /// [`OutlinePathEl`] ops. Cubic Bézier segments are emitted verbatim for a
    /// shape outline; a rounded rectangle is expressed as line segments and
    /// quarter-arc cubics. `f` is invoked in path order; contours are closed
    /// with [`OutlinePathEl::Close`]. The fill rule for rasterization is
    /// [`Self::fill_rule`].
    pub fn for_each_path<U>(&self, target: euclid::Rect<f32, U>, f: &mut dyn FnMut(OutlinePathEl)) {
        match self {
            Self::Rectangle(radius) => emit_rounded_rectangle_path(target, *radius, f),
            Self::Shape { shape, fit } => {
                let Some(transform) = shape_fit_transform(shape, target, *fit) else {
                    return;
                };
                let map = |x: f32, y: f32| {
                    OutlinePoint::new(
                        transform.sx * x + transform.tx,
                        transform.sy * y + transform.ty,
                    )
                };
                let mut first = true;
                for cubic in shape.cubics().chunks_exact(8) {
                    if first {
                        f(OutlinePathEl::MoveTo(map(cubic[0], cubic[1])));
                        first = false;
                    }
                    f(OutlinePathEl::CurveTo(
                        map(cubic[2], cubic[3]),
                        map(cubic[4], cubic[5]),
                        map(cubic[6], cubic[7]),
                    ));
                }
                if !first {
                    f(OutlinePathEl::Close);
                }
            }
        }
    }

    /// The bounds of the outline fitted into `target` (for accessibility
    /// bounds reporting).
    pub fn bounds(&self, target: LogicalRect) -> LogicalRect {
        match self {
            Self::Rectangle(..) => target,
            Self::Shape { shape, fit } => {
                let Some(transform) = shape_fit_transform(
                    shape,
                    euclid::rect::<f32, LogicalPx>(
                        target.min_x() as f32,
                        target.min_y() as f32,
                        target.width() as f32,
                        target.height() as f32,
                    ),
                    *fit,
                ) else {
                    return target;
                };
                let [x0, y0, x1, y1] = shape.bounds(false).unwrap_or_default();
                let p0 = transform.transform(x0, y0);
                let p1 = transform.transform(x1, y1);
                LogicalRect::new(
                    LogicalPoint::new(
                        p0.x.min(p1.x) as crate::Coord,
                        p0.y.min(p1.y) as crate::Coord,
                    ),
                    LogicalSize::new(
                        (p0.x - p1.x).abs() as crate::Coord,
                        (p0.y - p1.y).abs() as crate::Coord,
                    ),
                )
            }
        }
    }

    /// The outline as a `lyon` path fitted into `target`, for the renderers
    /// that consume paths.
    #[cfg(feature = "path")]
    pub fn to_lyon_path<U>(&self, target: euclid::Rect<f32, U>) -> Option<lyon_path::Path> {
        let mut builder = lyon_path::Path::builder();
        match self {
            Self::Rectangle(radius) => {
                let rect = lyon_path::geom::euclid::Box2D::new(
                    euclid::point2(target.min_x(), target.min_y()),
                    euclid::point2(target.max_x(), target.max_y()),
                );
                if radius.is_zero() {
                    builder.add_rectangle(&rect, lyon_path::Winding::Positive);
                } else {
                    builder.add_rounded_rectangle(
                        &rect,
                        &lyon_path::builder::BorderRadii {
                            top_left: radius.top_left,
                            top_right: radius.top_right,
                            bottom_left: radius.bottom_left,
                            bottom_right: radius.bottom_right,
                        },
                        lyon_path::Winding::Positive,
                    );
                }
            }
            Self::Shape { shape, fit } => {
                let transform = shape_fit_transform(shape, target, *fit)?;
                let cubics = shape.cubics();
                if cubics.is_empty() {
                    return None;
                }
                let p = transform.transform(cubics[0], cubics[1]);
                builder.begin(euclid::point2(p.x, p.y));
                for c in cubics.chunks_exact(8) {
                    let c0 = transform.transform(c[2], c[3]);
                    let c1 = transform.transform(c[4], c[5]);
                    let p1 = transform.transform(c[6], c[7]);
                    builder.cubic_bezier_to(
                        euclid::point2(c0.x, c0.y),
                        euclid::point2(c1.x, c1.y),
                        euclid::point2(p1.x, p1.y),
                    );
                }
                builder.end(true);
            }
        }
        Some(builder.build())
    }
}

/// One op of an outline's vector path, in the target coordinate space.
/// (destination points of `MoveTo`/`LineTo`/`CurveTo`)
#[derive(Clone, Copy, Debug)]
pub enum OutlinePathEl {
    /// Start a new contour at this point.
    MoveTo(OutlinePoint),
    /// Line segment to this point.
    LineTo(OutlinePoint),
    /// Cubic Bézier segment: control point 1, control point 2, end point.
    CurveTo(OutlinePoint, OutlinePoint, OutlinePoint),
    /// Close the current contour back to its `MoveTo` point.
    Close,
}

/// The flattening tolerance for hit-testing, matching the rasterizer's
/// tolerance so the clickable area is exactly the rendered area.
const HIT_TEST_TOLERANCE: f32 = 0.25;

/// Splits `curve` (p0, c0, c1, p1) into line segments appended to `out`
/// (the start point is not pushed again), recursing until the flatness error
/// is within `tolerance` pixels.
fn push_flattened_cubic(
    p0: OutlinePoint,
    c0: OutlinePoint,
    c1: OutlinePoint,
    p1: OutlinePoint,
    tolerance: f32,
    out: &mut Vec<OutlinePoint>,
    depth: u32,
) {
    // Flatness test: both control points must be within `tolerance` of the
    // chord. With the distance-to-line squared test a degenerate chord
    // (p0 == p1) falls back to the distance to the chord's endpoints.
    let chord = p1 - p0;
    let chord_len_sq = chord.square_length();
    let flat_enough = |p: OutlinePoint| {
        if chord_len_sq < f32::EPSILON {
            (p - p0).square_length() <= tolerance * tolerance
        } else {
            let cross = (p.x - p0.x) * chord.y - (p.y - p0.y) * chord.x;
            cross * cross <= tolerance * tolerance * chord_len_sq
        }
    };
    if depth < 20 && !(flat_enough(c0) && flat_enough(c1)) {
        // de Casteljau split at t = 0.5
        let mid = |a: OutlinePoint, b: OutlinePoint| {
            OutlinePoint::new((a.x + b.x) / 2., (a.y + b.y) / 2.)
        };
        let q0 = mid(p0, c0);
        let q1 = mid(c0, c1);
        let q2 = mid(c1, p1);
        let r0 = mid(q0, q1);
        let r1 = mid(q1, q2);
        let m = mid(r0, r1);
        push_flattened_cubic(p0, q0, r0, m, tolerance, out, depth + 1);
        push_flattened_cubic(m, r1, q2, p1, tolerance, out, depth + 1);
    } else {
        out.push(p1);
    }
}

/// Flattens a rounded rectangle into a closed polyline, or `None` for a
/// degenerate `rect`.
fn flatten_rounded_rectangle<U>(
    rect: euclid::Rect<f32, U>,
    radius: LogicalBorderRadius,
    tolerance: f32,
) -> Option<Vec<OutlinePoint>> {
    let (x, y, w, h) = (
        rect.origin.x as f32,
        rect.origin.y as f32,
        rect.size.width as f32,
        rect.size.height as f32,
    );
    if w <= 0. || h <= 0. {
        return None;
    }
    // Clamp the radii like the renderers do: no corner radius may exceed half
    // the corresponding side.
    let max = (w / 2.).min(h / 2.);
    let tl = (radius.top_left).clamp(0., max);
    let tr = (radius.top_right).clamp(0., max);
    let br = (radius.bottom_right).clamp(0., max);
    let bl = (radius.bottom_left).clamp(0., max);
    let mut contour = Vec::new();
    // (corner center, start angle, corner radius) in clockwise order.
    for (cx, cy, a0, r) in [
        (x + w - tr, y + tr, -0.5 * core::f32::consts::PI, tr),
        (x + w - br, y + h - br, 0.0, br),
        (x + bl, y + h - bl, 0.5 * core::f32::consts::PI, bl),
        (x + tl, y + tl, core::f32::consts::PI, tl),
    ] {
        if r <= 0. {
            // Sharp corner: emit the corner point itself.
            contour.push(OutlinePoint::new(cx, cy));
            continue;
        }
        // Number of segments for the quarter arc: sagitta of a chord of
        // angle θ in a circle of radius r is r(1 - cos(θ/2)) <= tolerance.
        let angle = 0.5 * core::f32::consts::PI;
        let max_step = 2. * (1. - (tolerance / r).min(1.)).clamp(-1., 1.).acos();
        let segments = (angle / max_step.max(f32::EPSILON)).ceil().max(1.) as usize;
        for i in 0..=segments {
            let a = a0 + angle * (i as f32 / segments as f32);
            contour.push(OutlinePoint::new(cx + r * a.cos(), cy + r * a.sin()));
        }
    }
    Some(contour)
}

/// Point-in-rounded-rectangle test for hit-testing: `point` is known to be
/// inside `rect`.
fn point_in_rounded_rectangle<U>(
    rect: euclid::Rect<f32, U>,
    radius: LogicalBorderRadius,
    point: OutlinePoint,
) -> bool {
    let (x, y, w, h) = (
        rect.origin.x as f32,
        rect.origin.y as f32,
        rect.size.width as f32,
        rect.size.height as f32,
    );
    let max = (w / 2.).min(h / 2.);
    let corners = [
        // (corner center x, corner center y, radius, in_corner predicate)
        (x + w, y, radius.top_right),
        (x + w, y + h, radius.bottom_right),
        (x, y + h, radius.bottom_left),
        (x, y, radius.top_left),
    ];
    for (cx, cy, r) in corners {
        let r = r.clamp(0., max);
        if r <= 0. {
            continue;
        }
        // The corner occupies the r-by-r square inward from (cx, cy)'s corner.
        let in_x = (point.x - cx).abs() <= r;
        let in_y = (point.y - cy).abs() <= r;
        if in_x && in_y {
            return (point.x - cx).powi(2) + (point.y - cy).powi(2) <= r * r;
        }
    }
    true
}

/// Point-in-polygon over flattened `contours`, honoring `fill_rule`
/// (nonzero winding vs. even-odd parity).
fn point_in_contours(
    contours: &[Vec<OutlinePoint>],
    fill_rule: FillRule,
    point: OutlinePoint,
) -> bool {
    let mut winding = 0i32;
    let mut inside = false;
    for contour in contours {
        if contour.len() < 3 {
            continue;
        }
        let mut prev = *contour.last().unwrap();
        for cur in contour {
            // Standard winding/parity test along a horizontal ray to +x.
            if (prev.y <= point.y) != (cur.y <= point.y) {
                let x_intersect = prev.x + (point.y - prev.y) / (cur.y - prev.y) * (cur.x - prev.x);
                if x_intersect > point.x {
                    inside = !inside;
                    winding += if cur.y > prev.y { 1 } else { -1 };
                }
            }
            prev = *cur;
        }
    }
    match fill_rule {
        FillRule::Nonzero => winding != 0,
        FillRule::Evenodd => inside,
    }
}
/// Emits `target`'s rounded-rectangle outline as line segments and quarter-arc
/// cubics in clockwise order (in a y-down coordinate space), the same winding
/// as [`flatten_rounded_rectangle`].
fn emit_rounded_rectangle_path<U>(
    target: euclid::Rect<f32, U>,
    radius: LogicalBorderRadius,
    f: &mut dyn FnMut(OutlinePathEl),
) {
    // https://pomax.github.io/bezierinfo/#circles_cubic: a quarter circle is
    // one cubic with handle length κ·r, κ = (4/3)·tan(π/8).
    const KAPPA: f32 = 0.552_284_75;
    let x = target.min_x();
    let y = target.min_y();
    let w = target.width();
    let h = target.height();
    if w <= 0. || h <= 0. {
        return;
    }
    let clamp = |r: f32| r.max(0.).min(w.min(h) / 2.);
    let (tl, tr, br, bl) = (
        clamp(radius.top_left),
        clamp(radius.top_right),
        clamp(radius.bottom_right),
        clamp(radius.bottom_left),
    );
    // Clockwise: top edge left-to-right, then the right, bottom and left edges.
    f(OutlinePathEl::MoveTo(OutlinePoint::new(x + tl, y)));
    f(OutlinePathEl::LineTo(OutlinePoint::new(x + w - tr, y)));
    if tr > 0. {
        f(OutlinePathEl::CurveTo(
            OutlinePoint::new(x + w - tr + tr * KAPPA, y),
            OutlinePoint::new(x + w, y + tr - tr * KAPPA),
            OutlinePoint::new(x + w, y + tr),
        ));
    }
    f(OutlinePathEl::LineTo(OutlinePoint::new(x + w, y + h - br)));
    if br > 0. {
        f(OutlinePathEl::CurveTo(
            OutlinePoint::new(x + w, y + h - br + br * KAPPA),
            OutlinePoint::new(x + w - br + br * KAPPA, y + h),
            OutlinePoint::new(x + w - br, y + h),
        ));
    }
    f(OutlinePathEl::LineTo(OutlinePoint::new(x + bl, y + h)));
    if bl > 0. {
        f(OutlinePathEl::CurveTo(
            OutlinePoint::new(x + bl - bl * KAPPA, y + h),
            OutlinePoint::new(x, y + h - bl + bl * KAPPA),
            OutlinePoint::new(x, y + h - bl),
        ));
    }
    f(OutlinePathEl::LineTo(OutlinePoint::new(x, y + tl)));
    if tl > 0. {
        f(OutlinePathEl::CurveTo(
            OutlinePoint::new(x, y + tl - tl * KAPPA),
            OutlinePoint::new(x + tl - tl * KAPPA, y),
            OutlinePoint::new(x + tl, y),
        ));
    }
    f(OutlinePathEl::Close);
}
