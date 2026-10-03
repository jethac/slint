// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore Casteljau scanline scanlines subpaths supersampled supersampling

//! Scanline coverage rasterization for shape outlines and paths.
//!
//! This module is the software renderer's single rasterization pipeline for
//! everything vector: element outlines, `Path` elements, clip outlines,
//! shadow masks and spread strokes. The pipeline is
//!
//! 1. flatten — curve events to closed polylines at [`FLATTEN_TOLERANCE`],
//!    in physical pixels;
//! 2. stroke → fill — [`stroke_to_fill`] turns open or closed polylines into
//!    closed outline contours honoring joins (miter, round, bevel) and caps
//!    (butt, round, square);
//! 3. coverage rasterization — a per-scanline active-edge sweep accumulating
//!    signed fractional area per pixel (the font-rasterizer approach, so
//!    anti-aliasing needs no supersampling buffers), honoring the `nonzero`
//!    and `evenodd` fill rules;
//! 4. span emission — one row of coverage at a time, so `RenderToBuffer`
//!    composites directly and `render_by_line` produces the same pixels from
//!    stored contours. Memory is O(edges + row width), never the path's
//!    bounding box.

use alloc::vec;
use alloc::vec::Vec;
use i_slint_core::items::{FillRule, LineCap, LineJoin};
use i_slint_core::lengths::PhysicalPx;
#[cfg(not(feature = "std"))]
#[allow(unused_imports)]
use num_traits::Float;

/// A rasterizer-space point: physical pixels.
pub type Point = euclid::Point2D<f32, PhysicalPx>;

/// A flattened contour: a polyline. Closed contours wrap their last point
/// back to the first.
pub type Contour = Vec<Point>;

/// The flattening tolerance for curves, in physical pixels. Matches the
/// tolerance `ElementOutline::flatten` uses, so rasterization and
/// hit-testing see the same geometry.
// Not `pub`: cbindgen emits exported constants into the C++ headers.
pub(crate) const FLATTEN_TOLERANCE: f32 = 0.25;

/// Flattens `events` (lyon path events) into closed contours, applying
/// `transform` to every point. Curves are subdivided adaptively until the
/// maximum distance between the curve and its chord is within `tolerance`.
/// Open subpaths are closed: every consumer in the renderer (fills, strokes,
/// clips, shadow masks) works on closed geometry.
#[cfg(feature = "path")]
pub fn flatten_events<P: Copy>(
    events: impl IntoIterator<Item = lyon_path::Event<P, P>>,
    transform: impl Fn(P) -> Point,
    tolerance: f32,
) -> Vec<Contour> {
    let mut contours = Vec::new();
    let mut current: Contour = Vec::new();
    for event in events {
        match event {
            lyon_path::Event::Begin { at } => {
                if !current.is_empty() {
                    contours.push(core::mem::take(&mut current));
                }
                current.push(transform(at));
            }
            lyon_path::Event::Line { to, .. } => {
                current.push(transform(to));
            }
            lyon_path::Event::Quadratic { ctrl, to, .. } => {
                let Some(&from) = current.last() else { continue };
                push_flattened_quadratic(
                    from,
                    transform(ctrl),
                    transform(to),
                    tolerance,
                    &mut current,
                    0,
                );
            }
            lyon_path::Event::Cubic { ctrl1, ctrl2, to, .. } => {
                let Some(&from) = current.last() else { continue };
                push_flattened_cubic(
                    from,
                    transform(ctrl1),
                    transform(ctrl2),
                    transform(to),
                    tolerance,
                    &mut current,
                    0,
                );
            }
            lyon_path::Event::End { close, .. } => {
                if close && current.len() > 1 && current[0] != *current.last().unwrap() {
                    // Repeat the first point so closed contours carry their
                    // explicit wrap: `stroke_to_fill` detects closure by
                    // first == last and otherwise drops the closing segment.
                    current.push(current[0]);
                }
                if !current.is_empty() {
                    contours.push(core::mem::take(&mut current));
                }
            }
        }
    }
    if !current.is_empty() {
        contours.push(current);
    }
    contours
}

/// Recursively subdivides the cubic `p0`-`c0`-`c1`-`p1` appending the
/// resulting polyline to `out` (without re-pushing `p0`).
#[cfg(feature = "path")]
fn push_flattened_cubic(
    p0: Point,
    c0: Point,
    c1: Point,
    p1: Point,
    tolerance: f32,
    out: &mut Contour,
    depth: usize,
) {
    const MAX_DEPTH: usize = 12;
    let flat_enough = |p0: Point, c: Point, p1: Point| {
        // The distance from the control point to the chord, scaled so that a
        // cubic is flat when both control points are close enough.
        let dx = p1.x - p0.x;
        let dy = p1.y - p0.y;
        let d = ((c.y - p0.y) * dx - (c.x - p0.x) * dy).abs();
        d * d <= tolerance * tolerance * (dx * dx + dy * dy).max(1e-12)
    };
    if depth >= MAX_DEPTH || (flat_enough(p0, c0, p1) && flat_enough(p0, c1, p1)) {
        out.push(p1);
        return;
    }
    // De Casteljau split at t = 0.5.
    let m0 = midpoint(p0, c0);
    let m1 = midpoint(c0, c1);
    let m2 = midpoint(c1, p1);
    let m3 = midpoint(m0, m1);
    let m4 = midpoint(m1, m2);
    let m = midpoint(m3, m4);
    push_flattened_cubic(p0, m0, m3, m, tolerance, out, depth + 1);
    push_flattened_cubic(m, m4, m2, p1, tolerance, out, depth + 1);
}

#[cfg(feature = "path")]
fn push_flattened_quadratic(
    p0: Point,
    c: Point,
    p1: Point,
    tolerance: f32,
    out: &mut Contour,
    depth: usize,
) {
    const MAX_DEPTH: usize = 12;
    let dx = p1.x - p0.x;
    let dy = p1.y - p0.y;
    let d = ((c.y - p0.y) * dx - (c.x - p0.x) * dy).abs();
    let flat_enough = d * d <= tolerance * tolerance * (dx * dx + dy * dy).max(1e-12);
    if depth >= MAX_DEPTH || flat_enough {
        out.push(p1);
        return;
    }
    let m0 = midpoint(p0, c);
    let m1 = midpoint(c, p1);
    let m = midpoint(m0, m1);
    push_flattened_quadratic(p0, m0, m, tolerance, out, depth + 1);
    push_flattened_quadratic(m, m1, p1, tolerance, out, depth + 1);
}

#[cfg(feature = "path")]
fn midpoint(a: Point, b: Point) -> Point {
    Point::new((a.x + b.x) * 0.5, (a.y + b.y) * 0.5)
}

/// Converts a polyline stroke to closed fill contours: the union of the
/// geometry on both sides of the path plus join and cap geometry.
///
/// `closed` selects whether each source contour wraps around (closed shapes
/// never get caps). The returned contours are always closed.
///
/// Joins: miter (clipped at `miter_limit` times the half-width), round, or
/// bevel. Caps: butt, round (a semicircle), or square (half-width extension).
pub fn stroke_to_fill(
    contours: &[Contour],
    width: f32,
    cap: LineCap,
    join: LineJoin,
    miter_limit: f32,
) -> Vec<Contour> {
    let h = width / 2.;
    if h <= 0. {
        return Vec::new();
    }
    let mut result = Vec::with_capacity(contours.len());
    for contour in contours {
        if contour.len() < 2 {
            continue;
        }
        // A closed contour wraps; an open one is capped at both ends.
        // A contour that already ends at its start point counts as closed.
        let closed = {
            let first = contour[0];
            let last = contour[contour.len() - 1];
            first.x == last.x && first.y == last.y
        };
        let n = if closed { contour.len() - 1 } else { contour.len() };
        if n < 2 {
            continue;
        }
        // Flattened paths repeat the seam vertex between curve segments;
        // a zero-length segment has no direction and would poison the joins
        // of the vertex that follows it.
        let mut deduped: alloc::vec::Vec<Point> = contour[..n].to_vec();
        deduped.dedup_by(|a, b| (a.x - b.x).abs() < 1e-6 && (a.y - b.y).abs() < 1e-6);
        if deduped.len() < 2 {
            continue;
        }
        let n = deduped.len();
        let pts = &deduped[..];

        // Segment directions.
        let seg_count = if closed { n } else { n - 1 };
        let mut dirs = Vec::with_capacity(seg_count.max(1));
        for i in 0..seg_count {
            let a = pts[i];
            let b = pts[(i + 1) % n];
            let d: euclid::Vector2D<f32, PhysicalPx> = euclid::vec2(b.x - a.x, b.y - a.y);
            let len = (d.x * d.x + d.y * d.y).sqrt();
            dirs.push(if len > 1e-6 {
                euclid::vec2(d.x / len, d.y / len)
            } else {
                euclid::vec2(1., 0.)
            });
        }
        if dirs.is_empty() {
            continue;
        }
        let normal = |d: euclid::Vector2D<f32, PhysicalPx>| -> euclid::Vector2D<f32, PhysicalPx> {
            euclid::vec2(-d.y, d.x)
        };

        // The join geometry for each interior vertex: points on the left and
        // right offset curves, in path order.
        let mut left: Contour = Vec::new();
        let mut right: Contour = Vec::new();

        let vertex_count = if closed { n } else { n - 2 };
        for i in 0..vertex_count {
            // `i` indexes the interior vertex: for open polylines that's
            // pts[i+1]; for closed ones pts[i].
            let vi = if closed { i } else { i + 1 };
            let v = pts[vi];
            let d_in = dirs[if closed { (vi + n - 1) % n } else { vi - 1 }];
            let d_out = dirs[vi % n];
            let n_in = normal(d_in) * h;
            let n_out = normal(d_out) * h;

            // Cross product of the two directions: >0 means the path turns
            // left, so the join's convex side is the left.
            let turn = d_in.x * d_out.y - d_in.y * d_out.x;
            for &is_left in [true, false].iter() {
                let (n_in, n_out) = if is_left { (n_in, n_out) } else { (-n_in, -n_out) };
                let convex = (turn > 0.) == is_left;
                let side = if is_left { &mut left } else { &mut right };
                emit_join(side, v, d_in, d_out, n_in, n_out, convex, h, join, miter_limit);
            }
        }

        // Caps for open polylines, in outward order: the end cap runs left to
        // right around `d_end`, the start cap right to left around
        // `-d_start` — the same frame, so both insert as-is.
        let (start_cap_pts, end_cap_pts) = if closed {
            (Vec::new(), Vec::new())
        } else {
            (
                cap_points(pts[0], -dirs[0], h, cap),
                cap_points(pts[n - 1], dirs[seg_count - 1], h, cap),
            )
        };

        if closed {
            // A closed stroke is two rings — the outward and inward offset
            // loops — wound oppositely so the band between them fills under
            // either fill rule. A single merged loop can't express the hole:
            // stitching the rings into one contour replaces the edges across
            // the wrap point with bridges and the ring loses that side.
            for ring in [left, {
                right.reverse();
                right
            }] {
                let mut ring = ring;
                ring.dedup_by(|a, b| (a.x - b.x).abs() < 1e-6 && (a.y - b.y).abs() < 1e-6);
                if ring.len() >= 3 {
                    result.push(ring);
                }
            }
            continue;
        }

        // Assemble: left of the start, the left offset points, left of the
        // end, the end cap, right of the end, the right offset points walked
        // back, right of the start, the start cap — one closed loop.
        let mut outline = Vec::with_capacity(left.len() + right.len() + 8);
        outline.push(pts[0] + normal(dirs[0]) * h);
        outline.extend_from_slice(&left);
        outline.push(pts[n - 1] + normal(dirs[seg_count - 1]) * h);
        outline.extend_from_slice(&end_cap_pts);
        outline.push(pts[n - 1] - normal(dirs[seg_count - 1]) * h);
        for p in right.iter().rev() {
            outline.push(*p);
        }
        outline.push(pts[0] - normal(dirs[0]) * h);
        outline.extend_from_slice(&start_cap_pts);
        // Remove consecutive duplicate points.
        outline.dedup_by(|a, b| (a.x - b.x).abs() < 1e-6 && (a.y - b.y).abs() < 1e-6);
        if outline.len() >= 3 {
            result.push(outline);
        }
    }
    result
}

/// Emits the offset-join points for one side of a vertex: `n_in`/`n_out` are
/// the inward and outward offset vectors (signed half-width normal), `d_in`
/// `d_out` the segment directions around `v`.
#[allow(clippy::too_many_arguments)]
fn emit_join(
    out: &mut Contour,
    v: Point,
    d_in: euclid::Vector2D<f32, PhysicalPx>,
    d_out: euclid::Vector2D<f32, PhysicalPx>,
    n_in: euclid::Vector2D<f32, PhysicalPx>,
    n_out: euclid::Vector2D<f32, PhysicalPx>,
    convex: bool,
    half_width: f32,
    join: LineJoin,
    miter_limit: f32,
) {
    if !convex {
        // Inner join: the intersection of the two offset lines is enough —
        // a join primitive there would poke inside the outline.
        let a0 = v + n_in;
        let a1 = v - d_in + n_in;
        let b0 = v + n_out;
        let b1 = v + d_out + n_out;
        let p = line_intersection(a0.to_vector(), a1.to_vector(), b0.to_vector(), b1.to_vector())
            .unwrap_or_else(|| (v + n_out).to_vector());
        out.push(p.to_point());
        return;
    }
    match join {
        LineJoin::Round => {
            push_join_arc(out, v, n_in, n_out, half_width);
        }
        LineJoin::Bevel => {
            out.push(v + n_in);
            out.push(v + n_out);
        }
        _ => {
            // Miter: the point where the two offset lines would meet,
            // clipped at miter_limit × half-width.
            let cos_half = ((1. + d_in.x * d_out.x + d_in.y * d_out.y) / 2.).max(0.).sqrt();
            let miter_len = if cos_half < 1e-4 { f32::MAX } else { half_width / cos_half };
            if miter_len > miter_limit.max(1.) * half_width {
                out.push(v + n_in);
                out.push(v + n_out);
            } else {
                let dm = (n_in + n_out).normalize();
                out.push(v + dm * miter_len);
            }
        }
    }
}

fn line_intersection(
    a0: euclid::Vector2D<f32, PhysicalPx>,
    a1: euclid::Vector2D<f32, PhysicalPx>,
    b0: euclid::Vector2D<f32, PhysicalPx>,
    b1: euclid::Vector2D<f32, PhysicalPx>,
) -> Option<euclid::Vector2D<f32, PhysicalPx>> {
    let da = a1 - a0;
    let db = b1 - b0;
    let denom = da.x * db.y - da.y * db.x;
    if denom.abs() < 1e-6 {
        return None;
    }
    let t = ((b0.x - a0.x) * db.y - (b0.y - a0.y) * db.x) / denom;
    Some(a0 + da * t)
}

/// Appends an arc around `center` from `start` to `end` (offset vectors of
/// length `radius`) sweeping the short way.
fn push_join_arc(
    out: &mut Contour,
    center: Point,
    start: euclid::Vector2D<f32, PhysicalPx>,
    end: euclid::Vector2D<f32, PhysicalPx>,
    radius: f32,
) {
    let a0 = start.y.atan2(start.x);
    let mut a1 = end.y.atan2(end.x);
    while a1 - a0 > core::f32::consts::PI {
        a1 -= 2. * core::f32::consts::PI;
    }
    while a0 - a1 > core::f32::consts::PI {
        a1 += 2. * core::f32::consts::PI;
    }
    if a1 < a0 {
        a1 += 2. * core::f32::consts::PI;
    }
    let sweep = a1 - a0;
    let steps = ((sweep * radius) / FLATTEN_TOLERANCE).ceil() as usize;
    let steps = steps.clamp(2, 64);
    out.push(center + start);
    for i in 1..steps {
        let a = a0 + sweep * (i as f32 / steps as f32);
        out.push(center + euclid::vec2::<f32, PhysicalPx>(a.cos() * radius, a.sin() * radius));
    }
    out.push(center + end);
}

/// The points making up a cap at `tip`, spanning the offset line `tip ±
/// normal(outward)` on the `outward` side of the tip — from the left offset
/// to the right offset in the outward frame. A butt cap emits nothing: the
/// closing edge between the offset ends is the cap line.
fn cap_points(
    tip: Point,
    outward: euclid::Vector2D<f32, PhysicalPx>,
    half_width: f32,
    cap: LineCap,
) -> Vec<Point> {
    let n = euclid::vec2(-outward.y, outward.x) * half_width;
    match cap {
        LineCap::Round => {
            let mut pts = Vec::with_capacity(9);
            // Semicircle from +n through `outward` to −n.
            let a0 = n.y.atan2(n.x);
            for i in 1..=8 {
                let a = a0 - core::f32::consts::PI * (i as f32 / 8.);
                pts.push(
                    tip + euclid::vec2::<f32, PhysicalPx>(
                        a.cos() * half_width,
                        a.sin() * half_width,
                    ),
                );
            }
            pts
        }
        LineCap::Square => {
            vec![tip + n + outward * half_width, tip - n + outward * half_width]
        }
        _ => Vec::new(),
    }
}

/// Scanline coverage rasterizer state: reusable row buffers, one pair per
/// scanline. The same `Rasterizer` must be reused across the rows of one
/// shape; each `rasterize_row` is independent though, so no ordering is
/// required.
#[derive(Default)]
pub struct Rasterizer {
    /// Signed vertical coverage accumulated per column.
    cover: Vec<f32>,
    /// Signed area-to-the-right accumulation per column.
    area: Vec<f32>,
    /// Edges of the current shape, sorted by their first covered scanline.
    edges: Vec<Edge>,
    /// Index into `edges` of the first edge that could still cover future
    /// scanlines. Used as an incremental active-edge window.
    window_start: usize,
    /// The last scanline rasterized; a lower `y` on the next call rewinds
    /// `window_start` since the active-edge window is only valid
    /// monotonically.
    last_y: i32,
}

/// One rasterization edge: a line segment with a winding sign.
struct Edge {
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
    /// First scanline covered.
    y_start: i32,
    /// One past the last scanline covered.
    y_end: i32,
}

impl Rasterizer {
    /// Prepares the rasterizer for `contours`: builds the edge list sorted by
    /// covered scanline. Call [`rasterize_row`] for each scanline afterwards.
    pub fn begin(&mut self, contours: &[Contour]) {
        self.edges.clear();
        self.window_start = 0;
        self.last_y = i32::MIN;
        for contour in contours {
            if contour.len() < 2 {
                continue;
            }
            for i in 0..contour.len() {
                let a = contour[i];
                let b = contour[(i + 1) % contour.len()];
                if a.y == b.y {
                    continue;
                }
                let y_min = a.y.min(b.y);
                let y_max = a.y.max(b.y);
                self.edges.push(Edge {
                    x0: a.x,
                    y0: a.y,
                    x1: b.x,
                    y1: b.y,
                    y_start: y_min.floor() as i32,
                    y_end: y_max.ceil() as i32,
                });
            }
        }
        self.edges.sort_unstable_by(|a, b| a.y_start.cmp(&b.y_start).then(a.y_end.cmp(&b.y_end)));
    }

    /// Rasterizes scanline `y` (the pixel row's top edge) into `alpha` in
    /// coverage units 0..=255. `alpha[x]` holds the coverage of the pixel
    /// column `x_start + x`. Columns outside the stored x-range are ignored.
    pub fn rasterize_row(&mut self, y: i32, x_start: i32, alpha: &mut [u8], fill_rule: FillRule) {
        let w = alpha.len();
        if w == 0 {
            return;
        }
        self.cover.resize(w, 0.);
        self.area.resize(w, 0.);
        self.cover.fill(0.);
        self.area.fill(0.);

        // A row below the previous one invalidates the active-edge window.
        if y < self.last_y {
            self.window_start = 0;
        }
        self.last_y = y;

        // Advance the active-edge window past edges that end at or above this
        // row; they can't be covered again since edges are sorted by y_start.
        while self.window_start < self.edges.len() && self.edges[self.window_start].y_end <= y {
            self.window_start += 1;
        }

        let row_top = y as f32;
        let row_bottom = row_top + 1.;
        let (cover, area) = (&mut self.cover, &mut self.area);
        // Signed coverage accumulated by fragments to the left of `x_start`:
        // it seeds the running sum so the first drawn column starts at the
        // true winding level instead of zero.
        let mut left_cover = 0f32;
        for edge in &self.edges[self.window_start..] {
            if edge.y_start > y {
                break;
            }
            if edge.y_end <= y {
                continue;
            }
            accumulate_edge(cover, area, &mut left_cover, edge, row_top, row_bottom, x_start, w);
        }

        // Horizontal sweep: running signed coverage plus per-cell remainder.
        let mut running = left_cover;
        for (x, alpha) in alpha.iter_mut().enumerate().take(w) {
            running += self.cover[x];
            let coverage = running - self.area[x];
            let cov = coverage.abs();
            let a = match fill_rule {
                FillRule::Evenodd => {
                    // Fold the coverage into [0, 1]: the triangle wave of the
                    // winding number.
                    let k = cov % 2.;
                    if k > 1. { 2. - k } else { k }
                }
                _ => cov.min(1.),
            };
            *alpha = (a.clamp(0., 1.) * 255.).round() as u8;
        }
    }
}

/// The coverage contribution of `edge` accumulated into `cover`/`area` for
/// the scanline band `[row_top, row_bottom)`.
///
/// The contribution of an edge to a pixel column is a signed quantity:
/// `cover` accumulates the vertical extent of the edge inside the column and
/// `area` the same extent weighted by how far inside the column the edge is.
/// After the sweep, `coverage(x) = total cover − area(x)` is the fraction of the
/// pixel inside the outline.
fn accumulate_edge(
    cover: &mut [f32],
    area: &mut [f32],
    left_cover: &mut f32,
    edge: &Edge,
    row_top: f32,
    row_bottom: f32,
    x_start: i32,
    width: usize,
) {
    let y_min = edge.y0.min(edge.y1);
    let y_max = edge.y0.max(edge.y1);
    let ya = y_min.max(row_top);
    let yb = y_max.min(row_bottom);
    if ya >= yb {
        return;
    }
    let dy = edge.y1 - edge.y0;
    let s = dy.signum();
    let inv_dy = 1. / dy;
    // x at the clipped y values.
    let xa = edge.x0 + (ya - edge.y0) * inv_dy * (edge.x1 - edge.x0);
    let xb = edge.x0 + (yb - edge.y0) * inv_dy * (edge.x1 - edge.x0);

    if (xa - xb).abs() < 1e-9 {
        // Vertical edge: full y-extent at one x.
        let cov = s * (yb - ya);
        let c = xa.floor() as i32 - x_start;
        if c >= 0 && (c as usize) < width {
            let mid = xa - (xa.floor());
            cover[c as usize] += cov;
            area[c as usize] += cov * mid;
        } else if c < 0 {
            *left_cover += cov;
        }
        return;
    }

    let (lo, hi) = (xa.min(xb), xa.max(xb));
    let x_first = lo.floor() as i32;
    let x_last = hi.floor() as i32;
    let inv_dx = 1. / (xb - xa);
    let mut c = x_first;
    while c <= x_last {
        let l = lo.max(c as f32);
        let r = hi.min(c as f32 + 1.);
        if l < r {
            // y at the fragment's x-extent inside the column.
            let y_l = ya + (l - xa) * inv_dx * (yb - ya);
            let y_r = ya + (r - xa) * inv_dx * (yb - ya);
            let dy_cell = s * (y_r - y_l).abs();
            let mid = (l + r) / 2. - c as f32;
            let idx = c - x_start;
            if idx >= 0 && (idx as usize) < width {
                cover[idx as usize] += dy_cell;
                area[idx as usize] += dy_cell * mid;
            } else if idx < 0 {
                *left_cover += dy_cell;
            }
        }
        c += 1;
    }
}

/// Rasterizes `contours` together with a disk-kernel stroke of width
/// `2·|spread|` around them — one scanline pass over the same rows, two
/// coverage rows combined per pixel:
/// - `spread > 0` (dilate): coverage where fill ∪ ring, combined as
///   `a = 1 - (1 - a_fill)·(1 - a_ring)`;
/// - `spread < 0` (erode): coverage where fill minus ring,
///   `a = a_fill·(1 - a_ring)`.
///   `spread` is in physical pixels; `origin`/`size` bound the mask.
pub fn rasterize_spread_mask(
    contours: &[Contour],
    spread: f32,
    origin: euclid::Point2D<i32, PhysicalPx>,
    size: euclid::Size2D<i32, PhysicalPx>,
    fill_rule: FillRule,
) -> Vec<u8> {
    let mut fill = Rasterizer::default();
    fill.begin(contours);
    let ring_contours = if spread.abs() < 0.01 {
        Vec::new()
    } else {
        stroke_to_fill(contours, 2. * spread.abs(), LineCap::Round, LineJoin::Round, 4.)
    };
    let mut ring = Rasterizer::default();
    ring.begin(&ring_contours);
    let w = size.width.max(0) as usize;
    let h = size.height.max(0) as usize;
    let mut mask = vec![0u8; w * h];
    let (mut fill_row, mut ring_row) = (Vec::new(), Vec::new());
    for y in 0..h {
        let py = origin.y + y as i32;
        fill_row.clear();
        fill_row.resize(w, 0);
        fill.rasterize_row(py, origin.x, &mut fill_row, fill_rule);
        if ring_contours.is_empty() {
            mask[y * w..][..w].copy_from_slice(&fill_row);
            continue;
        }
        ring_row.clear();
        ring_row.resize(w, 0);
        ring.rasterize_row(py, origin.x, &mut ring_row, FillRule::Nonzero);
        for x in 0..w {
            let (a, r) = (fill_row[x] as u16, ring_row[x] as u16);
            mask[y * w + x] = if spread > 0. {
                (255 - (255 - a) * (255 - r) / 255) as u8
            } else {
                (a * (255 - r) / 255) as u8
            };
        }
    }
    mask
}

/// The exact separable Gaussian blur: `src` convolved horizontally then
/// vertically with the normalized kernel `w[i] = exp(-i^2 / 2σ²)` over
/// `i ∈ [-⌈3σ⌉, ⌈3σ⌉]`, in single precision. `src` and the output are
/// `w × h` rows of 8-bit coverage.
pub fn gaussian_blur(src: &[u8], dst: &mut [u8], w: usize, h: usize, sigma: f32) {
    if w == 0 || h == 0 {
        return;
    }
    if sigma < 0.5 {
        dst.copy_from_slice(src);
        return;
    }
    let radius = (3. * sigma).ceil() as usize;
    let mut kernel: Vec<f32> =
        (0..=radius).map(|i| (-((i * i) as f32) / (2. * sigma * sigma)).exp()).collect();
    let sum: f32 = kernel[0] + 2. * kernel[1..].iter().sum::<f32>();
    for v in kernel.iter_mut() {
        *v /= sum;
    }
    let mut tmp = vec![0f32; src.len()];
    for y in 0..h {
        for x in 0..w {
            let mut acc = kernel[0] * src[y * w + x] as f32;
            for i in 1..=radius {
                let l = src[y * w + x.saturating_sub(i)] as f32;
                let r = src[y * w + (x + i).min(w - 1)] as f32;
                acc += kernel[i] * (l + r);
            }
            tmp[y * w + x] = acc;
        }
    }
    for y in 0..h {
        for x in 0..w {
            let mut acc = kernel[0] * tmp[y * w + x];
            for i in 1..=radius {
                let t = tmp[y.saturating_sub(i) * w + x];
                let b = tmp[(y + i).min(h - 1) * w + x];
                acc += kernel[i] * (t + b);
            }
            dst[y * w + x] = acc.round().clamp(0., 255.) as u8;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pt(x: f32, y: f32) -> Point {
        Point::new(x, y)
    }

    /// Reference coverage: 16×16 supersampled point-in-polygon.
    fn oracle(contours: &[Contour], rule: FillRule, px: f32, py: f32) -> f32 {
        let mut inside = 0;
        for sy in 0..16 {
            for sx in 0..16 {
                let x = px + (sx as f32 + 0.5) / 16.;
                let y = py + (sy as f32 + 0.5) / 16.;
                let mut winding = 0i32;
                for c in contours {
                    for i in 0..c.len() {
                        let a = c[i];
                        let b = c[(i + 1) % c.len()];
                        let cross = (b.x - a.x) * (y - a.y) - (x - a.x) * (b.y - a.y);
                        if a.y <= y {
                            if b.y > y && cross > 0. {
                                winding += 1;
                            }
                        } else if b.y <= y && cross < 0. {
                            winding -= 1;
                        }
                    }
                }
                let hit = match rule {
                    FillRule::Evenodd => winding % 2 != 0,
                    _ => winding != 0,
                };
                if hit {
                    inside += 1;
                }
            }
        }
        inside as f32 / 256.
    }

    /// Rasterizes `contours` over `w`×`h` pixels at `x0`/`y0` and returns the
    /// alpha grid.
    fn rasterize(
        contours: &[Contour],
        rule: FillRule,
        x0: i32,
        y0: i32,
        w: usize,
        h: usize,
    ) -> Vec<Vec<u8>> {
        let mut rasterizer = Rasterizer::default();
        rasterizer.begin(contours);
        (0..h)
            .map(|r| {
                let mut row = vec![0u8; w];
                rasterizer.rasterize_row(y0 + r as i32, x0, &mut row, rule);
                row
            })
            .collect()
    }

    /// Asserts the rasterizer's alpha matches the supersampled oracle within
    /// `tolerance` (in coverage units).
    fn check(contours: &[Contour], rule: FillRule, x0: i32, y0: i32, w: usize, h: usize) {
        let grid = rasterize(contours, rule, x0, y0, w, h);
        for (y, row) in grid.iter().enumerate() {
            for (x, &alpha) in row.iter().enumerate() {
                let expected =
                    oracle(contours, rule, (x0 + x as i32) as f32, (y0 + y as i32) as f32);
                let got = alpha as f32 / 255.;
                assert!(
                    (got - expected).abs() <= 0.03,
                    "pixel ({x},{y}): expected {expected}, got {got}"
                );
            }
        }
    }

    #[test]
    fn integer_rect_is_exact() {
        let rect = vec![vec![pt(2., 2.), pt(10., 2.), pt(10., 8.), pt(2., 8.)]];
        let grid = rasterize(&rect, FillRule::Nonzero, 0, 0, 14, 12);
        for (y, row) in grid.iter().enumerate() {
            for (x, &alpha) in row.iter().enumerate() {
                let inside = (2..10).contains(&x) && (2..8).contains(&y);
                assert_eq!(alpha, if inside { 255 } else { 0 }, "at {x},{y}");
            }
        }
    }

    #[test]
    fn half_pixel_rect() {
        let rect = vec![vec![pt(2.5, 2.), pt(10., 2.), pt(10., 8.), pt(2.5, 8.)]];
        let grid = rasterize(&rect, FillRule::Nonzero, 0, 0, 14, 12);
        // Column 2 is half covered on a vertical boundary: alpha ≈ 0.5.
        assert!((grid[3][2] as f32 / 255. - 0.5).abs() < 0.02);
        assert_eq!(grid[3][3], 255);
        assert_eq!(grid[3][9], 255);
        assert_eq!(grid[3][10], 0);
    }

    #[test]
    fn triangle() {
        let tri = vec![vec![pt(2., 8.), pt(10., 8.), pt(6., 2.)]];
        check(&tri, FillRule::Nonzero, 0, 0, 14, 12);
    }

    #[test]
    fn hole_nonzero() {
        // Outer CCW + inner CW → hole.
        let outer = vec![pt(1., 1.), pt(11., 1.), pt(11., 9.), pt(1., 9.)];
        let inner = vec![pt(4., 4.), pt(4., 7.), pt(8., 7.), pt(8., 4.)];
        check(&[outer, inner], FillRule::Nonzero, 0, 0, 13, 11);
    }

    #[test]
    fn hole_evenodd() {
        // Same winding direction — nonzero keeps it filled, even-odd cuts it.
        let outer = vec![pt(1., 1.), pt(11., 1.), pt(11., 9.), pt(1., 9.)];
        let inner = vec![pt(4., 4.), pt(8., 4.), pt(8., 7.), pt(4., 7.)];
        let contours = [outer, inner];
        let grid = rasterize(&contours, FillRule::Evenodd, 0, 0, 13, 11);
        assert_eq!(grid[5][6], 0);
        assert_eq!(grid[2][6], 255);
        let grid_nz = rasterize(&contours, FillRule::Nonzero, 0, 0, 13, 11);
        assert_eq!(grid_nz[5][6], 255);
    }

    #[test]
    fn stroke_horizontal_line() {
        // A 4-px-wide horizontal segment from (2,5) to (10,5): butt caps.
        let stroked = stroke_to_fill(
            &[vec![pt(2., 5.), pt(10., 5.)]],
            4.,
            LineCap::Butt,
            LineJoin::Miter,
            4.,
        );
        let grid = rasterize(&stroked, FillRule::Nonzero, 0, 0, 14, 10);
        // Rows 3..7 covered between columns 2..10.
        assert_eq!(grid[4][5], 255);
        assert_eq!(grid[4][1], 0);
        assert_eq!(grid[4][10], 0);
        assert_eq!(grid[2][5], 0);
        assert_eq!(grid[7][5], 0);
    }

    #[test]
    fn stroke_rect_outline() {
        // Stroke a closed rect as an outline: the stroke ring stays filled,
        // the middle is empty.
        let rect = vec![pt(2., 2.), pt(10., 2.), pt(10., 8.), pt(2., 8.)];
        let stroked = stroke_to_fill(&[rect], 2., LineCap::Butt, LineJoin::Miter, 4.);
        let grid = rasterize(&stroked, FillRule::Nonzero, 0, 0, 14, 12);
        assert_eq!(grid[2][6], 255, "on the stroke ring");
        assert_eq!(grid[5][6], 0, "inside the stroked rect");
    }

    #[test]
    fn stroke_closed_contour_covers_closing_edge() {
        // A rect whose last point repeats its first is closed: the closing
        // edge joins rather than caps, so coverage reaches both sides of it.
        let closed = vec![pt(2., 2.), pt(10., 2.), pt(10., 8.), pt(2., 8.), pt(2., 2.)];
        let stroked = stroke_to_fill(&[closed], 2., LineCap::Butt, LineJoin::Miter, 4.);
        let grid = rasterize(&stroked, FillRule::Nonzero, 0, 0, 14, 12);
        // The closing edge is the left side (x = 2): the band [1, 3] covers
        // the pixels on both sides of it.
        assert_eq!(grid[5][1], 255, "left of the closing edge");
        assert_eq!(grid[5][2], 255, "right of the closing edge");
        assert_eq!(grid[5][3], 0, "past the band, on the hole boundary");
        assert_eq!(grid[5][6], 0, "inside the stroked rect");
        // The same rect without the wrap is open: no closing edge at all.
        let open = vec![pt(2., 2.), pt(10., 2.), pt(10., 8.), pt(2., 8.)];
        let stroked = stroke_to_fill(&[open], 2., LineCap::Butt, LineJoin::Miter, 4.);
        let grid = rasterize(&stroked, FillRule::Nonzero, 0, 0, 14, 12);
        assert_eq!(grid[5][1], 0, "left of the missing closing edge");
        assert_eq!(grid[5][2], 0, "right of the missing closing edge");
        assert_eq!(grid[5][6], 0, "inside the stroked rect");
    }

    #[test]
    fn clipped_region_seam() {
        // A rect extending left of the rasterized window: the first drawn
        // column must start at full coverage, not at zero.
        let rect = vec![vec![pt(0., 2.), pt(10., 2.), pt(10., 8.), pt(0., 8.)]];
        let grid = rasterize(&rect, FillRule::Nonzero, 5, 0, 5, 12);
        for (y, row) in grid.iter().enumerate().take(8).skip(2) {
            for (x, &alpha) in row.iter().enumerate().take(5) {
                assert_eq!(alpha, 255, "at {x},{y}");
            }
        }
        assert_eq!(grid[1][0], 0);
        assert_eq!(grid[9][0], 0);
        // Slanted variant: a diamond reaching past the window's left edge.
        let diamond = vec![vec![pt(-3., 5.), pt(5., 1.), pt(13., 5.), pt(5., 9.)]];
        check(&diamond, FillRule::Nonzero, 2, 0, 8, 12);
    }

    #[test]
    fn rotated_stroke_matches_mirrored() {
        let rect = vec![pt(6., 6.), pt(26., 6.), pt(26., 54.), pt(6., 54.)];
        let stroke =
            stroke_to_fill(core::slice::from_ref(&rect), 2., LineCap::Butt, LineJoin::Miter, 4.);
        let base = rasterize(&stroke, FillRule::Nonzero, 0, 0, 64, 64);
        let rot: Vec<Contour> = vec![rect.iter().map(|p| pt(64. - p.x, 64. - p.y)).collect()];
        let stroke_r = stroke_to_fill(&rot, 2., LineCap::Butt, LineJoin::Miter, 4.);
        let got = rasterize(&stroke_r, FillRule::Nonzero, 0, 0, 64, 64);
        for y in 0..64 {
            for x in 0..64 {
                assert_eq!(got[y][x], base[63 - y][63 - x], "at {x},{y}");
            }
        }
    }

    #[test]
    fn stroke_round_cap() {
        let stroked = stroke_to_fill(
            &[vec![pt(4., 6.), pt(10., 6.)]],
            4.,
            LineCap::Round,
            LineJoin::Miter,
            4.,
        );
        let grid = rasterize(&stroked, FillRule::Nonzero, 0, 0, 16, 12);
        // Round cap extends coverage beyond x=2.. into the semicircle around
        // (4,6): pixel (2,6) center (2.5,6.5) is inside the cap.
        assert!(grid[6][2] > 0);
        // But far above the cap there's nothing.
        assert_eq!(grid[1][2], 0);
    }

    #[test]
    fn gaussian_blur_impulse_is_the_kernel() {
        // A single-impulse image convolved with the 2-D Gaussian yields the
        // separable kernel products around the impulse.
        let (w, h) = (15, 15);
        let mut src = vec![0u8; w * h];
        src[7 * w + 7] = 255;
        let mut dst = vec![0u8; w * h];
        let sigma = 1.5;
        gaussian_blur(&src, &mut dst, w, h, sigma);
        // Rebuild the normalized 1-D kernel exactly like gaussian_blur does.
        let radius = (3. * sigma).ceil() as usize;
        let mut kernel: Vec<f32> =
            (0..=radius).map(|i| (-((i * i) as f32) / (2. * sigma * sigma)).exp()).collect();
        let sum: f32 = kernel[0] + 2. * kernel[1..].iter().sum::<f32>();
        for v in kernel.iter_mut() {
            *v /= sum;
        }
        for dy in -(radius as isize)..=radius as isize {
            for dx in -(radius as isize)..=radius as isize {
                let expect =
                    (kernel[dx.unsigned_abs()] * kernel[dy.unsigned_abs()] * 255.).round() as u8;
                let got = dst[(7 + dy) as usize * w + (7 + dx) as usize];
                assert!(
                    got.abs_diff(expect) <= 1,
                    "dx={dx} dy={dy}: got {got}, expected ≈{expect}"
                );
            }
        }
        // The impulse's energy only spreads: per-pixel u8 quantization loses
        // a few points of total coverage, but it stays close to conserved.
        assert!(dst[7 * w + 7] < 255);
        let total: u64 = dst.iter().map(|v| *v as u64).sum();
        assert!(total.abs_diff(255) <= 16, "energy lost: {total}");
    }

    #[test]
    fn gaussian_blur_small_sigma_is_identity() {
        let src = [0u8, 128, 255, 30];
        let mut dst = [0u8; 4];
        gaussian_blur(&src, &mut dst, 4, 1, 0.3);
        assert_eq!(dst, src);
    }

    /// A closed contour whose vertices repeat at the curve seams (what
    /// `ElementOutline::flatten` emits) must stroke to a uniform band: the
    /// zero-length segments carry no direction and must not poison the joins
    /// of the vertex that follows them.
    #[test]
    fn stroke_dedupes_seam_vertices() {
        // A rounded-rectangle contour in the style `ElementOutline::flatten`
        // produces: each edge's end point repeats as the next curve's start.
        let contour = vec![
            pt(40., 6.),
            pt(40., 6.),
            pt(38., 8.),
            pt(36., 10.),
            pt(36., 10.),
            pt(8., 10.),
            pt(8., 10.),
            pt(4., 8.),
            pt(2., 6.),
            pt(2., 6.),
            pt(2., 4.),
            pt(2., 4.),
            pt(4., 4.),
            pt(6., 2.),
            pt(6., 2.),
            pt(34., 2.),
            pt(34., 2.),
            pt(38., 4.),
            pt(40., 6.),
            pt(40., 6.),
        ];
        let stroked = stroke_to_fill(&[contour], 2., LineCap::Butt, LineJoin::Miter, 4.);
        let grid = rasterize(&stroked, FillRule::Nonzero, 0, 0, 42, 14);
        // The band along the bottom edge (the path at y10, stroke ±1 covers
        // rows 9 and 10 between the two arcs) is fully covered — the same
        // holds for the top edge at rows 1 and 2.
        for x in 10..=32 {
            for y in [9usize, 10usize] {
                assert_eq!(grid[y][x], 255, "bottom edge at {x},{y}");
            }
            for y in [1usize, 2usize] {
                assert_eq!(grid[y][x], 255, "top edge at {x},{y}");
            }
        }
    }
}
