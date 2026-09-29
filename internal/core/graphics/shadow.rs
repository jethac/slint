// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Elevation shadows: the Android ambient + spot shadow model.
//!
//! This module is a Rust port of the two tessellated shadow meshes Skia builds
//! for `SkShadowUtils::DrawShadow` — `SkAmbientShadowTessellator` and
//! `SkSpotShadowTessellator` from `src/utils/SkShadowTessellator.cpp` — together
//! with the metrics from `src/core/SkDrawShadowInfo.{h,cpp}` and the polygon
//! helpers from `src/utils/SkPolyUtils.cpp`, all at Skia `61e7ca4e`. Each
//! function carries the name of the function it ports.
//!
//! The result of each tessellator is a [`ShadowMesh`]: a triangle mesh whose
//! vertex alpha ramps from 1 (umbra) to 0 (penumbra edge). Renderers draw it
//! tinted by the shadow color, mapping the interpolated alpha through
//! [`gauss_falloff`] (the `gauss_a_to_rgba` quartic) — identical in spirit to
//! Skia's `drawVertices` + Gaussian color filter.
//!
//! Only the non-perspective, point-light paths are ported; directional light
//! and perspective CTMs do not occur for window-space element shadows.
//!
//! The light is the per-window Android "material" light: centered on the
//! display's top edge, `lightZ = 500·(min(W,H)/450 + 2)/3`, radius 800
//! (`ThreadedRenderer.setLightCenter` + `dimens.xml` at the pinned compose
//! revision). `elevation` is the caster's z in logical px.

use crate::Color;
use crate::graphics::ElementOutline;
use crate::graphics::shapes::Cubic;
use crate::lengths::LogicalPx;
use alloc::vec::Vec;
#[allow(unused_imports)]
use num_traits::Float;

type Pt = euclid::Point2D<f32, euclid::UnknownUnit>;
type Vec2 = euclid::Vector2D<f32, euclid::UnknownUnit>;

fn pt(x: f32, y: f32) -> Pt {
    euclid::point2(x, y)
}
fn vec2(x: f32, y: f32) -> Vec2 {
    euclid::vec2(x, y)
}
fn mid(a: Pt, b: Pt) -> Pt {
    pt((a.x + b.x) * 0.5, (a.y + b.y) * 0.5)
}
fn cross(a: Vec2, b: Vec2) -> f32 {
    a.x * b.y - a.y * b.x
}
fn dot(a: Vec2, b: Vec2) -> f32 {
    a.x * b.y + a.y * b.x
}
fn dist_sq(a: Pt, b: Pt) -> f32 {
    (b - a).square_length()
}
fn normalize(v: Vec2) -> Option<Vec2> {
    let len = v.length();
    (len.is_finite() && len > 0.).then(|| v / len)
}
fn set_length(v: Vec2, len: f32) -> Option<Vec2> {
    normalize(v).map(|n| n * len)
}

/// `SK_ScalarNearlyZero`: `SK_Scalar1 / (1 << 12)`.
const SK_SCALAR_NEARLY_ZERO: f32 = 1.0 / 4096.0;
/// `kCrossTolerance` in SkPolyUtils: `SK_ScalarNearlyZero²`.
const CROSS_TOLERANCE: f32 = SK_SCALAR_NEARLY_ZERO * SK_SCALAR_NEARLY_ZERO;
/// `kClose`/`duplicate_pt`: points closer than 1/16 px merge.
const DUPLICATE_PT_SQ: f32 = (1.0 / 16.0) * (1.0 / 16.0);
/// The 1/16 px grid `sanitize_point` quantizes path vertices to.
const SANITIZE_GRID: f32 = 1.0 / 16.0;
/// Tessellation tolerance of the curve flattening fed to the tessellators
/// (`kQuadTolerance`/`kCubicTolerance`).
pub const TESSELLATION_TOLERANCE: f32 = 0.2;

fn sanitize_point(p: Pt) -> Pt {
    pt((16. * p.x).round() * SANITIZE_GRID, (16. * p.y).round() * SANITIZE_GRID)
}

fn duplicate_pt(a: Pt, b: Pt) -> bool {
    dist_sq(a, b) < DUPLICATE_PT_SQ
}

/// `SkPointPriv::EqualsWithinTolerance` (per-coordinate, default NearlyZero).
fn equals_within_tolerance(a: Pt, b: Pt, tolerance: f32) -> bool {
    (a.x - b.x).abs() <= tolerance && (a.y - b.y).abs() <= tolerance
}

/// `SkPointPriv::DistanceToLineSegmentBetweenSqd`.
fn distance_to_segment_sq(p: Pt, a: Pt, b: Pt) -> f32 {
    let ab = b - a;
    let len_sq = ab.square_length();
    if len_sq > 0. {
        let t = (dot(p - a, ab) / len_sq).clamp(0., 1.);
        dist_sq(p, a + ab * t)
    } else {
        dist_sq(p, a)
    }
}

fn perp_dot(p0: Pt, p1: Pt, p2: Pt) -> f32 {
    cross(p1 - p0, p2 - p1)
}

/// A 2D affine transform: `x' = m11·x + m21·y + tx`, `y' = m12·x + m22·y + ty`.
/// Mirrors the `SkMatrix` subset the non-perspective shadow path needs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Affine {
    /// Row-major matrix element `m[0][0]`.
    pub m11: f32,
    /// Row-major matrix element `m[0][1]`.
    pub m12: f32,
    /// Row-major matrix element `m[1][0]`.
    pub m21: f32,
    /// Row-major matrix element `m[1][1]`.
    pub m22: f32,
    /// Translation x.
    pub tx: f32,
    /// Translation y.
    pub ty: f32,
}

impl Affine {
    /// The identity transform.
    pub const IDENTITY: Self = Self::new(1., 0., 0., 1., 0., 0.);

    /// All six matrix elements.
    pub const fn new(m11: f32, m12: f32, m21: f32, m22: f32, tx: f32, ty: f32) -> Self {
        Self { m11, m12, m21, m22, tx, ty }
    }

    /// `setScaleTranslate(sx, sy, tx, ty)`.
    pub fn scale_translate(sx: f32, sy: f32, tx: f32, ty: f32) -> Self {
        Self::new(sx, 0., 0., sy, tx, ty)
    }

    /// Apply the transform to `p`.
    pub fn map_point(&self, p: Pt) -> Pt {
        pt(self.m11 * p.x + self.m21 * p.y + self.tx, self.m12 * p.x + self.m22 * p.y + self.ty)
    }

    /// `self ∘ other`: applies `other` first, matching Skia's `preConcat`.
    pub fn pre_concat(&self, other: &Affine) -> Affine {
        Affine {
            m11: self.m11 * other.m11 + self.m21 * other.m12,
            m12: self.m12 * other.m11 + self.m22 * other.m12,
            m21: self.m11 * other.m21 + self.m21 * other.m22,
            m22: self.m12 * other.m21 + self.m22 * other.m22,
            tx: self.m11 * other.tx + self.m21 * other.ty + self.tx,
            ty: self.m12 * other.tx + self.m22 * other.ty + self.ty,
        }
    }

    /// The same transform with its translation removed (Skia's `noTrans`
    /// trick for canonical-space tessellation).
    pub fn without_translation(&self) -> Affine {
        Affine { tx: 0., ty: 0., ..*self }
    }

    /// The inverse transform, or `None` when singular.
    pub fn inverse(&self) -> Option<Affine> {
        let det = self.m11 * self.m22 - self.m12 * self.m21;
        if !(det.is_finite() && det != 0.) {
            return None;
        }
        let inv_det = 1. / det;
        let m11 = self.m22 * inv_det;
        let m12 = -self.m12 * inv_det;
        let m21 = -self.m21 * inv_det;
        let m22 = self.m11 * inv_det;
        Some(Affine {
            m11,
            m12,
            m21,
            m22,
            tx: -(m11 * self.tx + m21 * self.ty),
            ty: -(m12 * self.tx + m22 * self.ty),
        })
    }

    /// `getMinScale`: the smaller of the two axis scale factors.
    pub fn min_scale(&self) -> f32 {
        let sx = (self.m11 * self.m11 + self.m12 * self.m12).sqrt();
        let sy = (self.m21 * self.m21 + self.m22 * self.m22).sqrt();
        sx.min(sy)
    }

    /// Build the affine part of an item transform (`euclid::Transform2D`
    /// row-major `m11 m12 m21 m22 m31 m32` where `m31/m32` is the translation).
    pub fn from_transform<U, V>(t: &euclid::Transform2D<f32, U, V>) -> Self {
        Self::new(t.m11, t.m12, t.m21, t.m22, t.m31, t.m32)
    }
}

/// A tessellated shadow mesh: per-vertex coverage alpha (1 umbra → 0 penumbra)
/// and triangle indices into the vertex list.
#[derive(Clone, Debug, Default)]
pub struct ShadowMesh {
    /// Vertex positions in canonical draw space; add `draw offset` when drawing.
    pub positions: Vec<Pt>,
    /// Per-vertex shadow alpha in `0..=1` before the Gaussian falloff.
    pub alphas: Vec<f32>,
    /// Triangle indices into `positions`/`alphas`.
    pub indices: Vec<u16>,
}

impl ShadowMesh {
    fn is_empty(&self) -> bool {
        self.indices.is_empty()
    }
}

// ---------------------------------------------------------------------------
// SkDrawShadowMetrics (src/core/SkDrawShadowInfo.h)
// ---------------------------------------------------------------------------

const AMBIENT_HEIGHT_FACTOR: f32 = 1.0 / 128.0;
const AMBIENT_GEOM_FACTOR: f32 = 64.0;
/// `kMaxAmbientRadius = 300·(1/128)·64 = 150`.
const MAX_AMBIENT_RADIUS: f32 = 300. * AMBIENT_HEIGHT_FACTOR * AMBIENT_GEOM_FACTOR;

fn divide_and_pin(numerator: f32, denom: f32, min: f32, max: f32) -> f32 {
    (numerator / denom).clamp(min, max)
}

/// `SkDrawShadowMetrics::AmbientBlurRadius` — the ambient outset `min(z/2, 150)`.
pub fn ambient_blur_radius(height: f32) -> f32 {
    (height * AMBIENT_HEIGHT_FACTOR * AMBIENT_GEOM_FACTOR).min(MAX_AMBIENT_RADIUS)
}

/// `SkDrawShadowMetrics::AmbientRecipAlpha` — `1 + z/128`; the umbra inset
/// factor of the ambient ring.
pub fn ambient_recip_alpha(height: f32) -> f32 {
    1. + (height * AMBIENT_HEIGHT_FACTOR).max(0.)
}

/// `SkDrawShadowMetrics::SpotBlurRadius`.
pub fn spot_blur_radius(occluder_z: f32, light_z: f32, light_radius: f32) -> f32 {
    light_radius * divide_and_pin(occluder_z, light_z - occluder_z, 0., 0.95)
}

/// `SkDrawShadowMetrics::GetSpotParams` — `(blur radius, scale, translate)`.
pub fn spot_params(
    occluder_z: f32,
    light_x: f32,
    light_y: f32,
    light_z: f32,
    light_radius: f32,
) -> (f32, f32, Vec2) {
    let z_ratio = divide_and_pin(occluder_z, light_z - occluder_z, 0., 0.95);
    let blur_radius = light_radius * z_ratio;
    let scale = divide_and_pin(light_z, light_z - occluder_z, 1., 1.95);
    let translate = vec2(-z_ratio * light_x, -z_ratio * light_y);
    (blur_radius, scale, translate)
}

// ---------------------------------------------------------------------------
// SkPolyUtils (src/utils/SkPolyUtils.cpp)
// ---------------------------------------------------------------------------

fn compute_side(p0: Pt, v: Vec2, p: Pt) -> i32 {
    let perp = cross(v, p - p0);
    if perp.abs() > CROSS_TOLERANCE { if perp > 0. { 1 } else { -1 } } else { 0 }
}

/// `SkGetPolygonWinding`: 1 for ccw, −1 for cw, 0 for zero signed area.
fn polygon_winding(polygon: &[Pt]) -> i32 {
    if polygon.len() < 3 {
        return 0;
    }
    let mut quad_area = 0.;
    let mut v0 = polygon[1] - polygon[0];
    for curr in 2..polygon.len() {
        let v1 = polygon[curr] - polygon[0];
        quad_area += cross(v0, v1);
        v0 = v1;
    }
    if quad_area.abs() <= CROSS_TOLERANCE {
        return 0;
    }
    if quad_area > 0. { 1 } else { -1 }
}

fn compute_offset_vector(p0: Pt, p1: Pt, offset: f32, side: i32) -> Option<Vec2> {
    let perp = vec2(p0.y - p1.y, p1.x - p0.x);
    set_length(perp, offset * side as f32)
}

#[derive(Clone, Copy)]
struct OffsetSegment {
    p0: Pt,
    v: Vec2,
}

fn outside_interval(numerator: f32, denom: f32, denom_positive: bool) -> bool {
    (denom_positive && (numerator < 0. || numerator > denom))
        || (!denom_positive && (numerator > 0. || numerator < denom))
}

fn zero_length(v: Vec2, v_dot_v: f32) -> bool {
    !(v.x.is_finite() && v.y.is_finite() && v_dot_v != 0.)
}

/// `compute_intersection`: intersection `p` between offset segments at
/// parametric `s`/`t`; degenerate (zero-length) segments use their origin.
fn compute_intersection(s0: OffsetSegment, s1: OffsetSegment) -> Option<(Pt, f32, f32)> {
    let v0 = s0.v;
    let v1 = s1.v;
    let w = s1.p0 - s0.p0;
    let mut denom = cross(v0, v1);
    let denom_positive = denom > 0.;
    let (mut s_numerator, mut t_numerator);
    if denom.abs() <= CROSS_TOLERANCE {
        // Segments are parallel; bail unless they are also collinear.
        if cross(w, v0).abs() > CROSS_TOLERANCE || cross(w, v1).abs() > CROSS_TOLERANCE {
            return None;
        }
        let v0_dot_v0 = dot(v0, v0);
        if zero_length(v0, v0_dot_v0) {
            let v1_dot_v1 = dot(v1, v1);
            if zero_length(v1, v1_dot_v1) {
                if normalize(w).is_none() {
                    return Some((s0.p0, 0., 0.));
                }
                return None;
            }
            t_numerator = dot(v1, -w);
            denom = v1_dot_v1;
            if outside_interval(t_numerator, denom, true) {
                return None;
            }
            s_numerator = 0.;
        } else {
            s_numerator = dot(v0, w);
            denom = v0_dot_v0;
            t_numerator = 0.;
            if outside_interval(s_numerator, denom, true) {
                let v1_dot_v1 = dot(v1, v1);
                if zero_length(v1, v1_dot_v1) {
                    return None;
                }
                let old_s_numerator = s_numerator;
                s_numerator = dot(v0, w + v1);
                t_numerator = denom;
                if outside_interval(s_numerator, denom, true) {
                    if s_numerator * old_s_numerator > 0. {
                        return None;
                    }
                    s_numerator = 0.;
                    t_numerator = dot(v1, -w);
                    denom = v1_dot_v1;
                }
            }
        }
    } else {
        s_numerator = cross(w, v1);
        if outside_interval(s_numerator, denom, denom_positive) {
            return None;
        }
        t_numerator = cross(w, v0);
        if outside_interval(t_numerator, denom, denom_positive) {
            return None;
        }
    }
    let local_s = s_numerator / denom;
    let local_t = t_numerator / denom;
    Some((s0.p0 + v0 * local_s, local_s, local_t))
}

/// `SkIsConvexPolygon`.
fn is_convex_polygon(polygon: &[Pt]) -> bool {
    let n = polygon.len();
    if n < 3 {
        return false;
    }
    let mut last_perp_dot = 0.;
    let mut x_sign_changes = 0;
    let mut y_sign_changes = 0;
    let mut curr_index = 0;
    let mut next_index = 1;
    let mut v0 = polygon[curr_index] - polygon[n - 1];
    let mut last_vx = v0.x;
    let mut last_vy = v0.y;
    let mut v1 = polygon[next_index] - polygon[curr_index];
    for i in 0..n {
        if !(polygon[i].x.is_finite() && polygon[i].y.is_finite()) {
            return false;
        }
        let perp = cross(v0, v1);
        if last_perp_dot * perp < 0. {
            return false;
        }
        if perp != 0. {
            last_perp_dot = perp;
        }
        if last_vx * v1.x < 0. {
            x_sign_changes += 1;
        }
        if last_vy * v1.y < 0. {
            y_sign_changes += 1;
        }
        if x_sign_changes > 2 || y_sign_changes > 2 {
            return false;
        }
        curr_index = next_index;
        next_index = (curr_index + 1) % n;
        if v1.x != 0. {
            last_vx = v1.x;
        }
        if v1.y != 0. {
            last_vy = v1.y;
        }
        v0 = v1;
        v1 = polygon[next_index] - polygon[curr_index];
    }
    true
}

/// An edge of a polygon being offset, as a node of a doubly-linked ring
/// (`OffsetEdge`; indices instead of pointers).
#[derive(Clone, Copy)]
struct OffsetEdge {
    prev: usize,
    next: usize,
    offset: OffsetSegment,
    intersection: Pt,
    t_value: f32,
    index: u16,
    end: u16,
}

impl OffsetEdge {
    fn init(&mut self, start: u16, end: u16) {
        self.intersection = self.offset.p0;
        self.t_value = f32::MIN;
        self.index = start;
        self.end = end;
    }

    /// `checkIntersection`: endpoint match shortcut, then `compute_intersection`.
    fn check_intersection(&self, that: &OffsetEdge) -> Option<(Pt, f32, f32)> {
        if self.end == that.index {
            let p1 = self.offset.p0 + self.offset.v;
            if equals_within_tolerance(p1, that.offset.p0, SK_SCALAR_NEARLY_ZERO) {
                return Some((p1, 1., 0.));
            }
        }
        compute_intersection(self.offset, that.offset)
    }

    /// `computeCrossingDistance`: signed squared distance from this segment to
    /// the line intersection with `that`; negative when the crossing lies
    /// inside this segment.
    fn compute_crossing_distance(&self, that: &OffsetEdge) -> f32 {
        let v0 = self.offset.v;
        let v1 = that.offset.v;
        let denom = cross(v0, v1);
        if denom.abs() <= CROSS_TOLERANCE {
            return f32::MAX;
        }
        let w = that.offset.p0 - self.offset.p0;
        let mut local_s = cross(w, v1) / denom;
        if local_s < 0. {
            local_s = -local_s;
        } else {
            local_s -= 1.;
        }
        local_s * local_s.abs() * dot(v0, v0)
    }
}

struct OffsetRing {
    edges: Vec<OffsetEdge>,
    head: Option<usize>,
    count: usize,
}

impl OffsetRing {
    fn remove_node(&mut self, node: usize) {
        let prev = self.edges[node].prev;
        let next = self.edges[node].next;
        self.edges[prev].next = next;
        self.edges[next].prev = prev;
        if self.head == Some(node) {
            self.head = (next != node).then_some(next);
        }
        self.count -= 1;
    }
}

/// `SkInsetConvexPolygon` — the umbra ring for convex casters.
fn inset_convex_polygon(polygon: &[Pt], inset: f32) -> Option<Vec<Pt>> {
    let n = polygon.len();
    if n < 3 || n > u16::MAX as usize {
        return None;
    }
    if inset < -SK_SCALAR_NEARLY_ZERO || !inset.is_finite() {
        return None;
    }
    if inset <= SK_SCALAR_NEARLY_ZERO {
        return Some(polygon.to_vec());
    }
    let winding = polygon_winding(polygon);
    if winding == 0 {
        return None;
    }

    let mut ring = OffsetRing {
        edges: (0..n)
            .map(|_| OffsetEdge {
                prev: 0,
                next: 0,
                offset: OffsetSegment { p0: pt(0., 0.), v: vec2(0., 0.) },
                intersection: pt(0., 0.),
                t_value: f32::MIN,
                index: 0,
                end: 0,
            })
            .collect(),
        head: Some(0),
        count: n,
    };
    {
        let mut prev = n - 1;
        for curr in 0..n {
            let next = (curr + 1) % n;
            if !(polygon[curr].x.is_finite() && polygon[curr].y.is_finite()) {
                return None;
            }
            if compute_side(polygon[prev], polygon[curr] - polygon[prev], polygon[next]) * winding
                < 0
            {
                return None;
            }
            let v = polygon[next] - polygon[curr];
            let perp = set_length(vec2(-v.y, v.x), inset * winding as f32)?;
            ring.edges[curr].prev = prev;
            ring.edges[curr].next = next;
            ring.edges[curr].offset = OffsetSegment { p0: polygon[curr] + perp, v };
            ring.edges[curr].init(0, 0);
            prev = curr;
        }
    }

    let mut curr_edge = ring.head.unwrap();
    let mut prev_edge = ring.edges[curr_edge].prev;
    let mut iterations = 0usize;
    let max_iterations = n * n;
    while ring.head.is_some() && prev_edge != curr_edge {
        iterations += 1;
        if iterations > max_iterations {
            return None;
        }
        if let Some((intersection, s, t)) =
            compute_intersection(ring.edges[prev_edge].offset, ring.edges[curr_edge].offset)
        {
            if s < ring.edges[prev_edge].t_value {
                ring.remove_node(prev_edge);
                prev_edge = ring.edges[prev_edge].prev;
            } else if ring.edges[curr_edge].t_value > f32::MIN
                && equals_within_tolerance(intersection, ring.edges[curr_edge].intersection, 1.0e-6)
            {
                break;
            } else {
                ring.edges[curr_edge].intersection = intersection;
                ring.edges[curr_edge].t_value = t;
                prev_edge = curr_edge;
                curr_edge = ring.edges[curr_edge].next;
            }
        } else {
            let side = winding
                * compute_side(
                    ring.edges[curr_edge].offset.p0,
                    ring.edges[curr_edge].offset.v,
                    ring.edges[prev_edge].offset.p0,
                );
            if side < 0
                && side
                    == winding
                        * compute_side(
                            ring.edges[curr_edge].offset.p0,
                            ring.edges[curr_edge].offset.v,
                            ring.edges[prev_edge].offset.p0 + ring.edges[prev_edge].offset.v,
                        )
            {
                ring.remove_node(prev_edge);
                prev_edge = ring.edges[prev_edge].prev;
            } else {
                ring.remove_node(curr_edge);
                curr_edge = ring.edges[curr_edge].next;
            }
        }
    }

    let head = ring.head?;
    if ring.count == 0 {
        return None;
    }
    const CLEANUP_TOLERANCE: f32 = 0.01;
    let mut inset_polygon = Vec::with_capacity(ring.count);
    inset_polygon.push(ring.edges[head].intersection);
    let mut curr = ring.edges[head].next;
    while curr != head {
        if !equals_within_tolerance(
            ring.edges[curr].intersection,
            *inset_polygon.last().unwrap(),
            CLEANUP_TOLERANCE,
        ) {
            inset_polygon.push(ring.edges[curr].intersection);
        }
        curr = ring.edges[curr].next;
    }
    if inset_polygon.len() >= 2
        && equals_within_tolerance(
            inset_polygon[0],
            *inset_polygon.last().unwrap(),
            CLEANUP_TOLERANCE,
        )
    {
        inset_polygon.pop();
    }
    is_convex_polygon(&inset_polygon).then_some(inset_polygon)
}

/// `SkComputeRadialSteps` — rotation increment + step count for the round join
/// at a reflex/penumbra vertex (~4px arc segments, capped at u16).
fn compute_radial_steps(v1: Vec2, v2: Vec2, offset: f32) -> Option<(f32, f32, usize)> {
    const RECIP_PIXELS_PER_ARC_SEGMENT: f32 = 0.25;
    let r_cos = dot(v1, v2);
    if !r_cos.is_finite() {
        return None;
    }
    let r_sin = cross(v1, v2);
    if !r_sin.is_finite() {
        return None;
    }
    let theta = r_sin.atan2(r_cos);
    let float_steps = (offset * theta * RECIP_PIXELS_PER_ARC_SEGMENT).abs();
    if float_steps >= u16::MAX as f32 {
        return None;
    }
    let steps = (float_steps + 0.5).floor() as usize;
    let d_theta = if steps > 0 { theta / steps as f32 } else { 0. };
    let rot_sin = d_theta.sin();
    let rot_cos = d_theta.cos();
    if steps > 0 && (rot_sin == 0. || rot_cos == 1.) {
        return None;
    }
    Some((rot_sin, rot_cos, steps))
}

// --- sweep-line helpers for `is_simple_polygon` / `offset_simple_polygon` ---

fn left(p0: Pt, p1: Pt) -> bool {
    p0.x < p1.x || (p0.x <= p1.x && p0.y > p1.y)
}

#[derive(Clone, Copy, PartialEq)]
struct SweepVertex {
    position: Pt,
    index: u16,
    prev_index: u16,
    next_index: u16,
    flags: u16,
}

const PREV_LEFT: u16 = 0x1;
const NEXT_LEFT: u16 = 0x2;

/// `ActiveEdge`: a polygon edge in the sweep-line status structure. Stored in
/// an arena; `above`/`below`/`child` are arena indices (`usize::MAX` = none).
const NIL: usize = usize::MAX;

#[derive(Clone)]
struct ActiveEdge {
    segment: OffsetSegment,
    index0: u16,
    index1: u16,
    child: [usize; 2],
    above: usize,
    below: usize,
    red: bool,
}

impl ActiveEdge {
    fn new(p0: Pt, v: Vec2, index0: u16, index1: u16) -> Self {
        Self {
            segment: OffsetSegment { p0, v },
            index0,
            index1,
            child: [NIL, NIL],
            above: NIL,
            below: NIL,
            red: true,
        }
    }
}

/// `ActiveEdgeList`: the red-black edge list used by the sweep line. Arena
/// allocation (`Vec`) replaces the C++ placement-new pool.
struct ActiveEdgeList {
    arena: Vec<ActiveEdge>,
    /// Sentinel head node at index 0; the tree root is `head.child[1]`.
    head_child: [usize; 2],
}

impl ActiveEdgeList {
    fn new(max_edges: usize) -> Self {
        Self { arena: Vec::with_capacity(max_edges + 1), head_child: [NIL, NIL] }
    }

    fn allocate(&mut self, p0: Pt, v: Vec2, index0: u16, index1: u16) -> Option<usize> {
        if self.arena.len() >= self.arena.capacity() && self.arena.capacity() != 0 {
            // capacity is the C++ maxEdges bound
            return None;
        }
        self.arena.push(ActiveEdge::new(p0, v, index0, index1));
        Some(self.arena.len() - 1)
    }

    fn is_red(&self, node: usize) -> bool {
        node != NIL && self.arena[node].red
    }

    fn child(&self, node: usize, dir: usize) -> usize {
        if node == NIL { NIL } else { self.arena[node].child[dir] }
    }
    fn set_child(&mut self, node: usize, dir: usize, c: usize) {
        if node == NIL {
            self.head_child[dir] = c;
        } else {
            self.arena[node].child[dir] = c;
        }
    }

    fn single_rotation(&mut self, node: usize, dir: usize) -> usize {
        let tmp = self.arena[node].child[1 - dir];
        self.arena[node].child[1 - dir] = self.arena[tmp].child[dir];
        self.arena[tmp].child[dir] = node;
        self.arena[node].red = true;
        self.arena[tmp].red = false;
        tmp
    }

    fn double_rotation(&mut self, node: usize, dir: usize) -> usize {
        let inner = self.arena[node].child[1 - dir];
        self.arena[node].child[1 - dir] = self.single_rotation(inner, 1 - dir);
        self.single_rotation(node, dir)
    }

    /// `ActiveEdge::intersect` (segment form): cheap straddle test between the
    /// stored edge and the (p0, v) segment; neighbors never count as crossing.
    fn edge_intersect(&self, edge: usize, q0: Pt, w: Vec2, index0: u16, index1: u16) -> bool {
        let e = &self.arena[edge];
        if e.index0 == index0 || e.index1 == index0 || e.index0 == index1 || e.index1 == index1 {
            return false;
        }
        let p0 = e.segment.p0;
        let v = e.segment.v;
        let p1 = p0 + v;
        let q1 = q0 + w;
        if p0.x < q0.x {
            if q1.x < p1.x {
                compute_side(p0, v, q0) * compute_side(p0, v, q1) < 0
            } else {
                compute_side(p0, v, q0) * compute_side(q0, w, p1) > 0
            }
        } else if p1.x < q1.x {
            compute_side(q0, w, p0) * compute_side(q0, w, p1) < 0
        } else {
            compute_side(q0, w, p0) * compute_side(p0, v, q1) > 0
        }
    }

    /// `ActiveEdgeList::insert`.
    fn insert(&mut self, p0: Pt, p1: Pt, index0: u16, index1: u16) -> bool {
        let v = p1 - p0;
        if !(v.x.is_finite() && v.y.is_finite()) {
            return false;
        }
        let root = self.head_child[1];
        if root == NIL {
            let r = match self.allocate(p0, v, index0, index1) {
                Some(r) => r,
                None => return false,
            };
            self.arena[r].red = false;
            self.head_child[1] = r;
            return true;
        }

        let mut top = NIL; // fTreeHead sentinel: child[1] access via set_child/get
        let mut grandparent = NIL;
        let mut parent = NIL;
        let mut curr = root;
        let mut dir = 0usize;
        let mut last = 0usize;
        let mut pred = NIL;
        let mut next = NIL;

        loop {
            if curr == NIL {
                if (pred != NIL && self.edge_intersect(pred, p0, v, index0, index1))
                    || (next != NIL && self.edge_intersect(next, p0, v, index0, index1))
                {
                    return false;
                }
                let new_node = match self.allocate(p0, v, index0, index1) {
                    Some(c) => c,
                    None => return false,
                };
                self.arena[parent].child[dir] = new_node;
                let curr = new_node;
                self.arena[curr].above = pred;
                self.arena[curr].below = next;
                if pred != NIL {
                    if self.arena[pred].segment.p0 == self.arena[curr].segment.p0
                        && self.arena[pred].segment.v == self.arena[curr].segment.v
                    {
                        return false;
                    }
                    self.arena[pred].below = curr;
                }
                if next != NIL {
                    if self.arena[next].segment.p0 == self.arena[curr].segment.p0
                        && self.arena[next].segment.v == self.arena[curr].segment.v
                    {
                        return false;
                    }
                    self.arena[next].above = curr;
                }
                if self.is_red(parent) {
                    let dir2 = (self.child(top, 1) == grandparent) as usize;
                    let new_sub = if curr == self.arena[parent].child[last] {
                        self.single_rotation(grandparent, 1 - last)
                    } else {
                        self.double_rotation(grandparent, 1 - last)
                    };
                    self.set_child(top, dir2, new_sub);
                }
                break;
            } else if self.is_red(self.arena[curr].child[0])
                && self.is_red(self.arena[curr].child[1])
            {
                self.arena[curr].red = true;
                let c0 = self.arena[curr].child[0];
                let c1 = self.arena[curr].child[1];
                self.arena[c0].red = false;
                self.arena[c1].red = false;
                if self.is_red(parent) {
                    let dir2 = (self.child(top, 1) == grandparent) as usize;
                    let new_sub = if curr == self.arena[parent].child[last] {
                        self.single_rotation(grandparent, 1 - last)
                    } else {
                        self.double_rotation(grandparent, 1 - last)
                    };
                    self.set_child(top, dir2, new_sub);
                }
            }

            last = dir;
            let side = if self.arena[curr].index0 == index0 {
                compute_side(self.arena[curr].segment.p0, self.arena[curr].segment.v, p1)
            } else {
                compute_side(self.arena[curr].segment.p0, self.arena[curr].segment.v, p0)
            };
            if side == 0 {
                return false;
            }
            dir = (side < 0) as usize;

            if dir == 0 {
                next = curr;
            } else {
                pred = curr;
            }

            if grandparent != NIL {
                top = grandparent;
            }
            grandparent = parent;
            parent = curr;
            curr = self.arena[curr].child[dir];
        }

        if self.head_child[1] != NIL {
            self.arena[self.head_child[1]].red = false;
        }
        true
    }

    /// `ActiveEdgeList::replace`.
    fn replace(&mut self, p0: Pt, p1: Pt, p2: Pt, index0: u16, index1: u16, index2: u16) -> bool {
        if self.head_child[1] == NIL {
            return false;
        }
        let v = p2 - p1;
        // walk down from a virtual head whose right child is the root
        let mut curr = NIL; // sentinel: child[1] = root
        let mut found = NIL;
        let mut dir = 1usize;
        loop {
            let next = self.child(curr, dir);
            if next == NIL {
                break;
            }
            curr = next;
            let c = &self.arena[curr];
            if c.index0 == index0 && c.index1 == index1 {
                found = curr;
                break;
            }
            let side = if c.index1 == index1 {
                compute_side(c.segment.p0, c.segment.v, p0)
            } else {
                compute_side(c.segment.p0, c.segment.v, p1)
            };
            if side == 0 {
                return false;
            }
            dir = (side < 0) as usize;
        }
        if found == NIL {
            return false;
        }
        let pred = self.arena[found].above;
        let next = self.arena[found].below;
        if pred != NIL {
            let f = self.arena[found].clone();
            if self.edge_intersect(pred, f.segment.p0, f.segment.v, f.index0, f.index1)
                || self.edge_intersect(pred, p1, v, index1, index2)
            {
                return false;
            }
        }
        if next != NIL {
            let f = self.arena[found].clone();
            if self.edge_intersect(next, f.segment.p0, f.segment.v, f.index0, f.index1)
                || self.edge_intersect(next, p1, v, index1, index2)
            {
                return false;
            }
        }
        self.arena[found].segment = OffsetSegment { p0: p1, v };
        self.arena[found].index0 = index1;
        self.arena[found].index1 = index2;
        true
    }

    /// `ActiveEdgeList::remove`.
    fn remove(&mut self, p0: Pt, p1: Pt, index0: u16, index1: u16) -> bool {
        if self.head_child[1] == NIL {
            return false;
        }
        let mut curr = NIL; // sentinel head
        let mut parent = NIL;
        let mut grandparent: usize;
        let mut found = NIL;
        let mut dir = 1usize;

        while self.child(curr, dir) != NIL {
            let last = dir;
            grandparent = parent;
            parent = curr;
            curr = self.child(curr, dir);
            if self.arena[curr].index0 == index0 && self.arena[curr].index1 == index1 {
                found = curr;
                dir = 0;
            } else {
                let c = &self.arena[curr];
                let side = if c.index1 == index1 {
                    compute_side(c.segment.p0, c.segment.v, p0)
                } else {
                    compute_side(c.segment.p0, c.segment.v, p1)
                };
                if side == 0 {
                    return false;
                }
                dir = (side < 0) as usize;
            }

            // push the red node down
            if !self.is_red(curr) && !self.is_red(self.arena[curr].child[dir]) {
                if self.is_red(self.arena[curr].child[1 - dir]) {
                    let new_sub = self.single_rotation(curr, dir);
                    if parent == NIL {
                        self.head_child[last] = new_sub;
                    } else {
                        self.arena[parent].child[last] = new_sub;
                    }
                    parent = new_sub;
                } else {
                    let s = self.child(parent, 1 - last);
                    if s != NIL {
                        if !self.is_red(self.arena[s].child[1 - last])
                            && !self.is_red(self.arena[s].child[last])
                        {
                            self.arena[parent].red = false;
                            self.arena[s].red = true;
                            self.arena[curr].red = true;
                        } else {
                            let dir2 = (self.child(grandparent, 1) == parent) as usize;
                            if self.is_red(self.arena[s].child[last]) {
                                let new_sub = self.double_rotation(parent, last);
                                self.set_child(grandparent, dir2, new_sub);
                            } else {
                                let s_last = self.arena[s].child[1 - last];
                                if self.is_red(s_last) {
                                    let new_sub = self.single_rotation(parent, last);
                                    self.set_child(grandparent, dir2, new_sub);
                                }
                            }
                            let gp_child = self.child(grandparent, dir2);
                            if gp_child != NIL {
                                self.arena[curr].red = true;
                                self.arena[gp_child].red = true;
                                let c0 = self.arena[gp_child].child[0];
                                let c1 = self.arena[gp_child].child[1];
                                self.arena[c0].red = false;
                                self.arena[c1].red = false;
                            }
                        }
                    }
                }
            }
        }

        if found != NIL {
            let pred = self.arena[found].above;
            let next = self.arena[found].below;
            let f = self.arena[found].clone();
            if (pred != NIL
                && self.edge_intersect(pred, f.segment.p0, f.segment.v, f.index0, f.index1))
                || (next != NIL
                    && self.edge_intersect(next, f.segment.p0, f.segment.v, f.index0, f.index1))
            {
                return false;
            }
            if found != curr {
                self.arena[found].segment = self.arena[curr].segment;
                self.arena[found].index0 = self.arena[curr].index0;
                self.arena[found].index1 = self.arena[curr].index1;
                self.arena[found].above = self.arena[curr].above;
                // found->fBelow stays (keeps the original successor link)
            } else if next != NIL {
                self.arena[next].above = pred;
            }
            let pred = self.arena[found].above;
            if pred != NIL {
                self.arena[pred].below = self.arena[curr].below;
            }
            let dir2 = (self.child(parent, 1) == curr) as usize;
            let repl = self.arena[curr].child[(self.arena[curr].child[0] == NIL) as usize];
            self.set_child(parent, dir2, repl);
            if self.head_child[1] != NIL {
                self.arena[self.head_child[1]].red = false;
            }
        }
        if self.head_child[1] != NIL {
            self.arena[self.head_child[1]].red = false;
        }
        true
    }
}

/// `SkIsSimplePolygon`: sweep-line self-intersection test. Convex polygons are
/// trivially simple; polygons over 2048 vertices are rejected.
fn is_simple_polygon(polygon: &[Pt]) -> bool {
    let n = polygon.len();
    if n < 3 {
        return false;
    }
    if is_convex_polygon(polygon) {
        return true;
    }
    if n > 2048 {
        return false;
    }

    let mut vertices: Vec<SweepVertex> = Vec::with_capacity(n);
    for i in 0..n {
        if !(polygon[i].x.is_finite() && polygon[i].y.is_finite()) {
            return false;
        }
        let prev = (i + n - 1) % n;
        let next = (i + 1) % n;
        // The two edges adjacent to this vertex are the same: not simple.
        if polygon[prev] == polygon[next] {
            return false;
        }
        let mut flags = 0;
        if left(polygon[prev], polygon[i]) {
            flags |= PREV_LEFT;
        }
        if left(polygon[next], polygon[i]) {
            flags |= NEXT_LEFT;
        }
        vertices.push(SweepVertex {
            position: polygon[i],
            index: i as u16,
            prev_index: prev as u16,
            next_index: next as u16,
            flags,
        });
    }
    // `SkTDPQueue<Vertex, Vertex::Left>` — process vertices in `left` order.
    vertices.sort_by(|a, b| {
        // `left` ascending: x then −y
        a.position
            .x
            .partial_cmp(&b.position.x)
            .unwrap()
            .then(b.position.y.partial_cmp(&a.position.y).unwrap())
            .then(a.index.cmp(&b.index))
    });

    let mut sweep = ActiveEdgeList::new(n);
    for v in &vertices {
        let prev = polygon[v.prev_index as usize];
        let next = polygon[v.next_index as usize];
        let ok = match v.flags {
            0 => {
                sweep.insert(v.position, prev, v.index, v.prev_index)
                    && sweep.insert(v.position, next, v.index, v.next_index)
            }
            f if f == PREV_LEFT | NEXT_LEFT => {
                sweep.remove(prev, v.position, v.prev_index, v.index)
                    && sweep.remove(next, v.position, v.next_index, v.index)
            }
            f if f & PREV_LEFT != 0 => {
                sweep.replace(prev, v.position, next, v.prev_index, v.index, v.next_index)
            }
            _ => sweep.replace(next, v.position, prev, v.next_index, v.index, v.prev_index),
        };
        if !ok {
            return false;
        }
    }
    true
}

fn is_reflex_vertex(
    polygon: &[Pt],
    winding: i32,
    offset: f32,
    prev: usize,
    curr: usize,
    next: usize,
) -> bool {
    let side = compute_side(polygon[prev], polygon[curr] - polygon[prev], polygon[next]);
    (side as f32 * winding as f32 * offset) < 0.
}

/// `SkOffsetSimplePolygon` — offset a simple polygon by `offset` (positive
/// insets, negative outsets), removing self-intersections. Returns the offset
/// polygon and the index of the source vertex each output vertex came from.
fn offset_simple_polygon(
    polygon: &[Pt],
    bounds_half_extent: f32,
    offset: f32,
) -> Option<(Vec<Pt>, Vec<i32>)> {
    let n = polygon.len();
    if n < 3 || n >= u16::MAX as usize {
        return None;
    }
    if !offset.is_finite() {
        return None;
    }
    // can't inset more than half the bounds of the polygon
    if offset > bounds_half_extent {
        return None;
    }
    if offset.abs() <= SK_SCALAR_NEARLY_ZERO {
        return Some((polygon.to_vec(), (0..n as i32).collect()));
    }
    let winding = polygon_winding(polygon);
    if winding == 0 {
        return None;
    }

    // build normals and count edges (reflex vertices add round-join edges)
    let mut normals: Vec<Vec2> = Vec::with_capacity(n);
    let mut num_edges = 0usize;
    {
        let mut prev_index = n - 1;
        for curr_index in 0..n {
            if !(polygon[curr_index].x.is_finite() && polygon[curr_index].y.is_finite()) {
                return None;
            }
            let next_index = (curr_index + 1) % n;
            let normal =
                compute_offset_vector(polygon[curr_index], polygon[next_index], offset, winding)?;
            normals.push(normal);
            if curr_index > 0
                && is_reflex_vertex(polygon, winding, offset, prev_index, curr_index, next_index)
            {
                let (_, _, num_steps) =
                    compute_radial_steps(normals[prev_index], normals[curr_index], offset)?;
                num_edges += num_steps.max(1);
            }
            num_edges += 1;
            prev_index = curr_index;
        }
    }
    if is_reflex_vertex(polygon, winding, offset, n - 1, 0, 1) {
        let (_, _, num_steps) = compute_radial_steps(normals[n - 1], normals[0], offset)?;
        num_edges += num_steps.max(1);
    }
    if num_edges > i32::MAX as usize {
        return None;
    }

    // build the offset edge ring
    let mut ring =
        OffsetRing { edges: Vec::with_capacity(num_edges), head: None, count: num_edges };
    let mut prev_edge: Option<usize> = None;
    {
        let mut prev_index = n - 1;
        for curr_index in 0..n {
            let next_index = (curr_index + 1) % n;
            if is_reflex_vertex(polygon, winding, offset, prev_index, curr_index, next_index) {
                let (rot_sin, rot_cos, num_steps) =
                    compute_radial_steps(normals[prev_index], normals[curr_index], offset)?;
                let mut prev_normal = normals[prev_index];
                let steps = num_steps.max(1);
                for i in 0..steps {
                    let (e0, e1) = if i + 1 == steps {
                        (
                            polygon[curr_index] + prev_normal,
                            polygon[curr_index] + normals[curr_index],
                        )
                    } else {
                        let curr_normal = vec2(
                            prev_normal.x * rot_cos - prev_normal.y * rot_sin,
                            prev_normal.y * rot_cos + prev_normal.x * rot_sin,
                        );
                        let pair =
                            (polygon[curr_index] + prev_normal, polygon[curr_index] + curr_normal);
                        prev_normal = curr_normal;
                        pair
                    };
                    ring.edges.push({
                        let mut e = OffsetEdge {
                            prev: usize::MAX,
                            next: usize::MAX,
                            offset: OffsetSegment { p0: e0, v: e1 - e0 },
                            intersection: e0,
                            t_value: f32::MIN,
                            index: curr_index as u16,
                            end: curr_index as u16,
                        };
                        e.init(curr_index as u16, curr_index as u16);
                        e
                    });
                    let idx = ring.edges.len() - 1;
                    ring.edges[idx].prev = prev_edge.unwrap_or(usize::MAX);
                    if let Some(pe) = prev_edge {
                        ring.edges[pe].next = idx;
                    }
                    prev_edge = Some(idx);
                }
            }
            // the offset edge for polygon[curr] → polygon[next]
            ring.edges.push({
                let mut e = OffsetEdge {
                    prev: usize::MAX,
                    next: usize::MAX,
                    offset: OffsetSegment {
                        p0: polygon[curr_index] + normals[curr_index],
                        v: polygon[next_index] - polygon[curr_index],
                    },
                    intersection: pt(0., 0.),
                    t_value: f32::MIN,
                    index: 0,
                    end: 0,
                };
                e.init(curr_index as u16, next_index as u16);
                e.intersection = e.offset.p0;
                e
            });
            let idx = ring.edges.len() - 1;
            ring.edges[idx].prev = prev_edge.unwrap_or(usize::MAX);
            if let Some(pe) = prev_edge {
                ring.edges[pe].next = idx;
            }
            prev_edge = Some(idx);
            prev_index = curr_index;
        }
    }
    // close the ring
    let last = ring.edges.len() - 1;
    ring.edges[last].next = 0;
    ring.edges[0].prev = last;
    ring.head = Some(0);

    // clip edges
    let mut curr_edge = 0usize;
    let mut iterations = 0usize;
    let max_iterations = num_edges.saturating_mul(num_edges);
    while ring.head.is_some() && prev_edge != Some(curr_edge) && ring.count > 0 {
        iterations += 1;
        if iterations > max_iterations {
            return None;
        }
        let pe = prev_edge.unwrap();
        if let Some((intersection, s, t)) =
            ring.edges[pe].check_intersection(&ring.edges[curr_edge])
        {
            if s < ring.edges[pe].t_value {
                ring.remove_node(pe);
                prev_edge = Some(ring.edges[pe].prev);
            } else if ring.edges[curr_edge].t_value > f32::MIN
                && equals_within_tolerance(intersection, ring.edges[curr_edge].intersection, 1.0e-6)
            {
                break;
            } else {
                ring.edges[curr_edge].intersection = intersection;
                ring.edges[curr_edge].t_value = t;
                ring.edges[curr_edge].index = ring.edges[pe].end;
                prev_edge = Some(curr_edge);
                curr_edge = ring.edges[curr_edge].next;
            }
        } else {
            let prev_prev = ring.edges[pe].prev;
            let curr_next = ring.edges[curr_edge].next;
            let dist0 = ring.edges[curr_edge].compute_crossing_distance(&ring.edges[prev_prev]);
            let dist1 = ring.edges[pe].compute_crossing_distance(&ring.edges[curr_next]);
            if dist0 < 0. && dist1 < 0. {
                let p1 = ring.edges[prev_prev].offset.p0 + ring.edges[prev_prev].offset.v;
                let prev_same_contour =
                    equals_within_tolerance(p1, ring.edges[pe].offset.p0, SK_SCALAR_NEARLY_ZERO);
                let p1 = ring.edges[curr_edge].offset.p0 + ring.edges[curr_edge].offset.v;
                let curr_same_contour = equals_within_tolerance(
                    p1,
                    ring.edges[curr_next].offset.p0,
                    SK_SCALAR_NEARLY_ZERO,
                );
                if curr_same_contour && !prev_same_contour {
                    ring.remove_node(curr_edge);
                    curr_edge = curr_next;
                    continue;
                } else if prev_same_contour && !curr_same_contour {
                    ring.remove_node(pe);
                    prev_edge = Some(prev_prev);
                    continue;
                }
            }
            if dist0 < dist1 {
                ring.remove_node(pe);
                prev_edge = Some(prev_prev);
            } else {
                ring.remove_node(curr_edge);
                curr_edge = curr_next;
            }
        }
    }

    let head = ring.head?;
    if ring.count == 0 || ring.count >= u16::MAX as usize {
        return None;
    }
    const CLEANUP_TOLERANCE: f32 = 0.01;
    let mut out_polygon = Vec::with_capacity(ring.count);
    let mut out_indices = Vec::with_capacity(ring.count);
    out_polygon.push(ring.edges[head].intersection);
    out_indices.push(ring.edges[head].index as i32);
    let mut curr = ring.edges[head].next;
    while curr != head {
        if !equals_within_tolerance(
            ring.edges[curr].intersection,
            *out_polygon.last().unwrap(),
            CLEANUP_TOLERANCE,
        ) {
            out_polygon.push(ring.edges[curr].intersection);
            out_indices.push(ring.edges[curr].index as i32);
        }
        curr = ring.edges[curr].next;
    }
    if out_polygon.len() >= 2
        && equals_within_tolerance(out_polygon[0], *out_polygon.last().unwrap(), CLEANUP_TOLERANCE)
    {
        out_polygon.pop();
        out_indices.pop();
    }
    let offset_winding = polygon_winding(&out_polygon);
    if winding * offset_winding > 0 && is_simple_polygon(&out_polygon) {
        Some((out_polygon, out_indices))
    } else {
        None
    }
}

// --- ear-clipping triangulation (SkTriangulateSimplePolygon) ---

fn compute_triangle_bounds(p0: Pt, p1: Pt, p2: Pt) -> (f32, f32, f32, f32) {
    (
        p0.x.min(p1.x).min(p2.x),
        p0.y.min(p1.y).min(p2.y),
        p0.x.max(p1.x).max(p2.x),
        p0.y.max(p1.y).max(p2.y),
    )
}

fn point_in_triangle(p0: Pt, p1: Pt, p2: Pt, p: Pt) -> bool {
    let v0 = p1 - p0;
    let v1 = p2 - p1;
    let n = cross(v0, v1);
    if n * cross(v0, p - p0) < SK_SCALAR_NEARLY_ZERO {
        return false;
    }
    if n * cross(v1, p - p1) < SK_SCALAR_NEARLY_ZERO {
        return false;
    }
    let v2 = p0 - p2;
    if n * cross(v2, p - p2) < SK_SCALAR_NEARLY_ZERO {
        return false;
    }
    true
}

/// `SkTriangulateSimplePolygon`: ear-clipping triangulation of a simple
/// polygon; `index_map` maps each input vertex to its index in the caller's
/// mesh vertex array.
fn triangulate_simple_polygon(polygon: &[Pt], index_map: &[u16], out: &mut Vec<u16>) -> bool {
    let n = polygon.len();
    if n < 3 || n >= u16::MAX as usize {
        return false;
    }
    let mut min_x = f32::MAX;
    let mut min_y = f32::MAX;
    let mut max_x = f32::MIN;
    let mut max_y = f32::MIN;
    for p in polygon {
        if !(p.x.is_finite() && p.y.is_finite()) {
            return false;
        }
        min_x = min_x.min(p.x);
        min_y = min_y.min(p.y);
        max_x = max_x.max(p.x);
        max_y = max_y.max(p.y);
    }
    let width = max_x - min_x;
    let height = max_y - min_y;
    if !(width.is_finite() && height.is_finite()) {
        return false;
    }
    let winding = polygon_winding(polygon);
    if winding == 0 {
        return false;
    }

    // classify vertices; reflex (incl. near-collinear) vertices go into the hash grid
    const CONVEX: u8 = 0;
    const REFLEX: u8 = 1;
    let mut vertex_types = alloc::vec![CONVEX; n];
    let mut v0 = polygon[0] - polygon[n - 1];
    for curr in 0..n {
        let next = (curr + 1) % n;
        let v1 = polygon[next] - polygon[curr];
        if (winding as f32) * cross(v0, v1) <= CROSS_TOLERANCE {
            vertex_types[curr] = REFLEX;
        }
        v0 = v1;
    }

    // ReflexHash: uniform grid over the bounds sized to the vertex count.
    let h_count_f = ((n as f32 * width / height.max(f32::MIN_POSITIVE)).sqrt()).max(0.);
    if !h_count_f.is_finite() {
        return false;
    }
    let h_count = ((h_count_f + 0.5).floor() as usize).clamp(1, n);
    let v_count = n / h_count;
    if v_count == 0 {
        return false;
    }
    let grid_x = (h_count as f32 - 0.001) / width;
    let grid_y = (v_count as f32 - 0.001) / height;
    if !(grid_x.is_finite() && grid_y.is_finite()) {
        return false;
    }
    let grid_cell = |p: Pt| -> usize {
        let h = ((p.x - min_x) * grid_x) as isize;
        let v = ((p.y - min_y) * grid_y) as isize;
        let i = (v.max(0) as usize) * h_count + h.max(0) as usize;
        i.min(h_count * v_count - 1)
    };
    let mut grid: Vec<Vec<usize>> = alloc::vec![Vec::new(); h_count * v_count];
    // `convex_list` keeps the C++ linked-list order: convex verts adjacent to a
    // reflex vertex go to the head, others to the tail.
    let mut convex_list: alloc::collections::VecDeque<usize> = Default::default();
    for curr in 0..n {
        if vertex_types[curr] == CONVEX {
            let prev_index = (curr + n - 1) % n;
            let next = (curr + 1) % n;
            if vertex_types[prev_index] == REFLEX || vertex_types[next] == REFLEX {
                convex_list.push_front(curr);
            } else {
                convex_list.push_back(curr);
            }
        } else {
            grid[grid_cell(polygon[curr])].push(curr);
        }
    }

    let mut prev_of: Vec<usize> = (0..n).map(|i| (i + n - 1) % n).collect();
    let mut next_of: Vec<usize> = (0..n).map(|i| (i + 1) % n).collect();

    let check_triangle =
        |grid: &[Vec<usize>], p0: Pt, p1: Pt, p2: Pt, ignore0: usize, ignore1: usize| -> bool {
            let (l, t, r, b) = compute_triangle_bounds(p0, p1, p2);
            let h0 = ((l - min_x) * grid_x) as isize;
            let h1 = ((r - min_x) * grid_x) as isize;
            let v0 = ((t - min_y) * grid_y) as isize;
            let v1 = ((b - min_y) * grid_y) as isize;
            for v in v0..=v1 {
                for h in h0..=h1 {
                    if h < 0 || v < 0 {
                        continue;
                    }
                    let i = v as usize * h_count + h as usize;
                    if i >= grid.len() {
                        continue;
                    }
                    for &rv in &grid[i] {
                        if rv != ignore0
                            && rv != ignore1
                            && point_in_triangle(p0, p1, p2, polygon[rv])
                        {
                            return true;
                        }
                    }
                }
            }
            false
        };

    out.reserve(out.len() + 3 * (n - 2));
    let mut vertex_count = n;
    while vertex_count > 3 {
        let mut ear = None;
        for (pos, &cv) in convex_list.iter().enumerate() {
            let p0 = prev_of[cv];
            let p2 = next_of[cv];
            if !check_triangle(&grid, polygon[p0], polygon[cv], polygon[p2], p0, p2) {
                ear = Some((pos, cv));
                break;
            }
        }
        let Some((pos, ear_vertex)) = ear else {
            return false;
        };
        convex_list.remove(pos);
        let p0 = prev_of[ear_vertex];
        let p2 = next_of[ear_vertex];
        out.push(index_map[p0]);
        out.push(index_map[ear_vertex]);
        out.push(index_map[p2]);
        vertex_count -= 1;

        // reclassify neighbors
        next_of[p0] = next_of[ear_vertex];
        prev_of[p2] = prev_of[ear_vertex];
        for &u in &[p0, p2] {
            if vertex_types[u] == REFLEX {
                let vv0 = polygon[u] - polygon[prev_of[u]];
                let vv1 = polygon[next_of[u]] - polygon[u];
                if (winding as f32) * cross(vv0, vv1) > CROSS_TOLERANCE {
                    vertex_types[u] = CONVEX;
                    // remove from grid, add to convex list tail
                    let cell = grid_cell(polygon[u]);
                    grid[cell].retain(|&x| x != u);
                    convex_list.push_back(u);
                }
            }
        }
    }
    for &v in &convex_list {
        out.push(index_map[v]);
    }
    true
}

// ---------------------------------------------------------------------------
// SkBaseShadowTessellator (src/utils/SkShadowTessellator.cpp)
// ---------------------------------------------------------------------------

/// Elevations below this (physical px) cast no shadow — Skia's `kMinHeight`.
pub const MIN_HEIGHT: f32 = 0.1;

/// Shared state and algorithms of the ambient and spot tessellators.
struct BaseTessellator {
    mesh: ShadowMesh,
    path_polygon: Vec<Pt>,
    clip_polygon: Vec<Pt>,
    clip_vectors: Vec<Vec2>,
    path_bounds: (f32, f32, f32, f32),
    centroid: Pt,
    area: f32,
    last_area: f32,
    last_cross: f32,
    first_vertex_index: usize,
    first_outset: Vec2,
    first_point: Pt,

    transparent: bool,
    is_convex: bool,
    valid_umbra: bool,
    direction: f32,
    prev_umbra_index: i32,
    curr_umbra_index: usize,
    curr_clip_index: usize,
    prev_umbra_outside: bool,
    first_umbra_outside: bool,
    prev_outset: Vec2,
    prev_point: Pt,
}

impl BaseTessellator {
    fn new(bounds: (f32, f32, f32, f32), transparent: bool) -> Self {
        Self {
            mesh: ShadowMesh::default(),
            path_polygon: Vec::new(),
            clip_polygon: Vec::new(),
            clip_vectors: Vec::new(),
            path_bounds: bounds,
            centroid: pt(0., 0.),
            area: 0.,
            last_area: 0.,
            last_cross: 0.,
            first_vertex_index: usize::MAX,
            first_outset: vec2(0., 0.),
            first_point: pt(0., 0.),

            transparent,
            is_convex: true,
            valid_umbra: true,
            direction: 1.,
            prev_umbra_index: -1,
            curr_umbra_index: 0,
            curr_clip_index: 0,
            prev_umbra_outside: false,
            first_umbra_outside: false,
            prev_outset: vec2(0., 0.),
            prev_point: pt(0., 0.),
        }
    }

    fn compute_normal(p0: Pt, p1: Pt, dir: f32) -> Option<Vec2> {
        let normal = vec2(p0.y - p1.y, p1.x - p0.x) * dir;
        normalize(normal)
    }

    fn accumulate_centroid(&mut self, curr: Pt, next: Pt) -> bool {
        if duplicate_pt(curr, next) {
            return false;
        }
        let v0 = curr - self.path_polygon[0];
        let v1 = next - self.path_polygon[0];
        let quad_area = cross(v0, v1);
        self.centroid.x += (v0.x + v1.x) * quad_area;
        self.centroid.y += (v0.y + v1.y) * quad_area;
        self.area += quad_area;
        if quad_area * self.last_area < 0. {
            self.is_convex = false;
        }
        if quad_area != 0. {
            self.last_area = quad_area;
        }
        true
    }

    fn check_convexity(&mut self, p0: Pt, p1: Pt, p2: Pt) -> bool {
        let c = perp_dot(p0, p1, p2);
        if c.abs() <= SK_SCALAR_NEARLY_ZERO {
            return false;
        }
        if self.last_cross * c < 0. {
            self.is_convex = false;
        }
        if c != 0. {
            self.last_cross = c;
        }
        true
    }

    fn finish_path_polygon(&mut self) {
        if self.path_polygon.len() > 1 {
            let last = *self.path_polygon.last().unwrap();
            if !self.accumulate_centroid(last, self.path_polygon[0]) {
                self.path_polygon.pop();
            }
        }
        if self.path_polygon.len() > 2 {
            self.centroid = self.centroid * (1. / (3. * self.area));
            self.centroid += self.path_polygon[0].to_vector();
            let n = self.path_polygon.len();
            if !self.check_convexity(
                self.path_polygon[n - 2],
                self.path_polygon[n - 1],
                self.path_polygon[0],
            ) {
                self.path_polygon[0] = self.path_polygon[n - 1];
                self.path_polygon.pop();
            }
        }
        self.direction = if self.area > 0. { -1. } else { 1. };
    }

    fn handle_line(&mut self, p: Pt) {
        let p = sanitize_point(p);
        if !self.path_polygon.is_empty() {
            let last = *self.path_polygon.last().unwrap();
            if !self.accumulate_centroid(last, p) {
                return;
            }
        }
        if self.path_polygon.len() > 1 {
            let n = self.path_polygon.len();
            if !self.check_convexity(self.path_polygon[n - 2], self.path_polygon[n - 1], p) {
                self.path_polygon.pop();
                let last = *self.path_polygon.last().unwrap();
                if duplicate_pt(last, p) {
                    self.path_polygon.pop();
                }
            }
        }
        self.path_polygon.push(p);
    }

    fn append_triangle(&mut self, i0: u16, i1: u16, i2: u16) {
        self.mesh.indices.extend_from_slice(&[i0, i1, i2]);
    }

    fn append_quad(&mut self, i0: u16, i1: u16, i2: u16, i3: u16) {
        self.mesh.indices.extend_from_slice(&[i0, i1, i2, i2, i1, i3]);
    }

    fn push_vertex(&mut self, p: Pt, alpha: f32) -> u16 {
        self.mesh.positions.push(p);
        self.mesh.alphas.push(alpha);
        (self.mesh.positions.len() - 1) as u16
    }

    /// `computeClipVectorsAndTestCentroid` — clip polygon edges plus whether
    /// the centroid is hidden behind the clip polygon (→ transparent fan).
    fn compute_clip_vectors_and_test_centroid(&mut self) {
        self.curr_clip_index = self.clip_polygon.len() - 1;
        let v0 = self.clip_polygon[1] - self.clip_polygon[0];
        self.clip_vectors.push(v0);
        let mut hidden_centroid = true;
        let v1 = self.centroid - self.clip_polygon[0];
        let init_cross = cross(v0, v1);
        for p in 1..self.clip_polygon.len() {
            let v0 = self.clip_polygon[(p + 1) % self.clip_polygon.len()] - self.clip_polygon[p];
            self.clip_vectors.push(v0);
            let v1 = self.centroid - self.clip_polygon[p];
            if init_cross * cross(v0, v1) <= 0. {
                hidden_centroid = false;
            }
        }
        self.transparent = self.transparent || !hidden_centroid;
    }

    /// `clipUmbraPoint` — where the umbra→centroid segment exits the clip
    /// polygon. Returns `Some(clip_point)` when the umbra point is outside.
    fn clip_umbra_point(&mut self, umbra_point: Pt, centroid: Pt) -> Option<Pt> {
        let mut segment_vector = centroid - umbra_point;
        let start = self.curr_clip_index;
        loop {
            let clip_p = self.clip_polygon[self.curr_clip_index];
            let clip_v = self.clip_vectors[self.curr_clip_index];
            let dp = umbra_point - clip_p;
            let denom = cross(clip_v, segment_vector);
            let t_num = cross(dp, segment_vector);
            if denom.abs() <= SK_SCALAR_NEARLY_ZERO {
                if t_num.abs() <= SK_SCALAR_NEARLY_ZERO {
                    return None;
                }
            } else if t_num >= 0. && t_num <= denom {
                let s_num = cross(dp, clip_v);
                if s_num >= 0. && s_num <= denom {
                    segment_vector = segment_vector * (s_num / denom);
                    return Some(umbra_point + segment_vector);
                }
            }
            self.curr_clip_index = (self.curr_clip_index + 1) % self.clip_polygon.len();
            if self.curr_clip_index == start {
                break;
            }
        }
        None
    }

    /// `getClosestUmbraIndex` — walk the umbra ring in the direction of
    /// decreasing distance from `p`.
    fn closest_umbra_index(&mut self, p: Pt, umbra_polygon: &[Pt]) -> usize {
        let n = umbra_polygon.len();
        let mut min_dist = dist_sq(p, umbra_polygon[self.curr_umbra_index]);
        let mut index = self.curr_umbra_index;
        let mut dir = 1usize;
        let next = (index + dir) % n;
        let d = dist_sq(p, umbra_polygon[next]);
        if d < min_dist {
            index = next;
            min_dist = d;
        } else {
            dir = n - 1;
        }
        let mut next = (index + dir) % n;
        let mut d = dist_sq(p, umbra_polygon[next]);
        while d < min_dist {
            index = next;
            min_dist = d;
            next = (index + dir) % n;
            d = dist_sq(p, umbra_polygon[next]);
        }
        self.curr_umbra_index = index;
        index
    }

    /// `addInnerPoint` — push the umbra point for path vertex `path_point`,
    /// merging near-duplicates. Returns `true` when merged (duplicate).
    fn add_inner_point(
        &mut self,
        path_point: Pt,
        umbra_alpha: f32,
        umbra_polygon: &[Pt],
        curr_umbra_index: &mut usize,
    ) -> bool {
        let umbra_point = if !self.valid_umbra {
            let v = (self.centroid - path_point) * 0.95;
            path_point + v
        } else {
            umbra_polygon[self.closest_umbra_index(path_point, umbra_polygon)]
        };
        self.prev_point = path_point;

        let first = self.first_vertex_index;
        if self.prev_umbra_index == -1
            || !duplicate_pt(umbra_point, self.mesh.positions[self.prev_umbra_index as usize])
        {
            if self.prev_umbra_index >= 0 && duplicate_pt(umbra_point, self.mesh.positions[first]) {
                *curr_umbra_index = first;
            } else {
                *curr_umbra_index = self.push_vertex(umbra_point, umbra_alpha) as usize;
            }
            false
        } else {
            *curr_umbra_index = self.prev_umbra_index as usize;
            true
        }
    }

    /// `addArc` — penumbra round join between `prev_outset` and `next_normal`.
    fn add_arc(&mut self, next_normal: Vec2, offset: f32, finish_arc: bool) -> bool {
        // C++ recovers with numSteps = 0 when the steps computation fails
        let (rot_sin, rot_cos, num_steps) =
            compute_radial_steps(self.prev_outset, next_normal, offset).unwrap_or((0., 1., 0));
        let mut prev_normal = self.prev_outset;
        for _ in 0..num_steps.saturating_sub(1) {
            let curr_normal = vec2(
                prev_normal.x * rot_cos - prev_normal.y * rot_sin,
                prev_normal.y * rot_cos + prev_normal.x * rot_sin,
            );
            let idx = self.push_vertex(self.prev_point + curr_normal, 0.);
            self.append_triangle(self.prev_umbra_index as u16, idx, idx - 1);
            prev_normal = curr_normal;
        }
        if finish_arc && num_steps > 0 {
            let idx = self.push_vertex(self.prev_point + next_normal, 0.);
            self.append_triangle(self.prev_umbra_index as u16, idx, idx - 1);
        }
        self.prev_outset = next_normal;
        num_steps > 0
    }

    /// `addEdge`.
    fn add_edge(
        &mut self,
        next_point: Pt,
        next_normal: Vec2,
        umbra_alpha: f32,
        umbra_polygon: &[Pt],
        last_edge: bool,
        do_clip: bool,
    ) {
        let mut curr_umbra_index = 0usize;
        let duplicate;
        if last_edge {
            duplicate = false;
            curr_umbra_index = self.first_vertex_index;
            self.prev_point = next_point;
        } else {
            duplicate =
                self.add_inner_point(next_point, umbra_alpha, umbra_polygon, &mut curr_umbra_index);
        }
        let prev_penumbra_index = if duplicate || curr_umbra_index == self.first_vertex_index {
            self.mesh.positions.len() - 1
        } else {
            self.mesh.positions.len() - 2
        };
        if !duplicate {
            if self.transparent {
                self.append_triangle(0, self.prev_umbra_index as u16, curr_umbra_index as u16);
            } else if do_clip {
                let is_outside = if last_edge {
                    self.first_umbra_outside
                } else {
                    match self
                        .clip_umbra_point(self.mesh.positions[curr_umbra_index], self.centroid)
                    {
                        Some(clip_point) => {
                            self.push_vertex(clip_point, umbra_alpha);
                            true
                        }
                        None => false,
                    }
                };
                if is_outside {
                    // note: for !last_edge the clip vertex was pushed above and is
                    // curr_umbra_index + 1
                    let clip_idx = if last_edge {
                        // C++ pushes the clip point only for non-last edges; for the
                        // last edge it reuses fFirstUmbraOutside without a new vertex,
                        // and the +1 index lands on the first clip vertex of the ring.
                        self.first_vertex_index + 1
                    } else {
                        curr_umbra_index + 1
                    };
                    self.append_triangle(
                        self.prev_umbra_index as u16,
                        curr_umbra_index as u16,
                        clip_idx as u16,
                    );
                    if self.prev_umbra_outside {
                        self.append_triangle(
                            self.prev_umbra_index as u16,
                            clip_idx as u16,
                            self.prev_umbra_index as u16 + 1,
                        );
                    }
                } else if self.prev_umbra_outside {
                    self.append_triangle(
                        self.prev_umbra_index as u16,
                        curr_umbra_index as u16,
                        self.prev_umbra_index as u16 + 1,
                    );
                }
                self.prev_umbra_outside = is_outside;
            }
        }

        let new_idx = self.push_vertex(next_point + next_normal, 0.);
        if !duplicate {
            self.append_triangle(
                self.prev_umbra_index as u16,
                prev_penumbra_index as u16,
                curr_umbra_index as u16,
            );
        }
        self.append_triangle(prev_penumbra_index as u16, new_idx, curr_umbra_index as u16);
        self.prev_umbra_index = curr_umbra_index as i32;
        self.prev_outset = next_normal;
    }

    /// `computeConvexShadow` — inset umbra + outset penumbra ring for a convex
    /// path polygon. `do_clip` (spot shadows) clips the umbra to the clip
    /// polygon so spot pixels land only outside the caster.
    fn compute_convex_shadow(&mut self, inset: f32, outset: f32, do_clip: bool) -> bool {
        if do_clip {
            self.compute_clip_vectors_and_test_centroid();
        }

        let mut umbra_alpha = 1.0f32;
        let mut min_dist_sq =
            distance_to_segment_sq(self.centroid, self.path_polygon[0], self.path_polygon[1]);
        for i in 1..self.path_polygon.len() {
            let j = if i == self.path_polygon.len() - 1 { 0 } else { i + 1 };
            let d =
                distance_to_segment_sq(self.centroid, self.path_polygon[i], self.path_polygon[j]);
            if d < min_dist_sq {
                min_dist_sq = d;
            }
        }

        let mut inset = inset;
        let mut inset_polygon: Vec<Pt> = Vec::new();
        if inset > SK_SCALAR_NEARLY_ZERO {
            const TOLERANCE: f32 = 1.0e-2;
            if min_dist_sq < (inset + TOLERANCE) * (inset + TOLERANCE) {
                // the umbra would collapse: back off the inset and fade the
                // umbra ring to preserve total darkness
                let new_inset = min_dist_sq.sqrt() - TOLERANCE;
                let ratio = (128. * (new_inset / inset + 1.)).clamp(0., 256.);
                umbra_alpha = 1. - ratio / 256.;
                inset = new_inset;
            }
            match inset_convex_polygon(&self.path_polygon, inset) {
                Some(p) => inset_polygon = p,
                None => self.valid_umbra = false,
            }
        }
        // `umbraPolygon` is the inset ring whenever an inset was requested;
        // when `valid_umbra` is false it stays empty and `add_inner_point`
        // takes the centroid fallback instead.
        let umbra_polygon =
            if inset > SK_SCALAR_NEARLY_ZERO { inset_polygon } else { self.path_polygon.clone() };

        if self.transparent {
            self.push_vertex(self.centroid, umbra_alpha);
        }
        self.curr_umbra_index = 0;

        let n = self.path_polygon.len();
        let Some(mut first_outset) =
            Self::compute_normal(self.path_polygon[n - 1], self.path_polygon[0], self.direction)
        else {
            return false;
        };
        first_outset = first_outset * outset;
        self.first_outset = first_outset;
        self.first_point = self.path_polygon[n - 1];
        self.first_vertex_index = self.mesh.positions.len();
        self.prev_outset = first_outset;
        self.prev_point = self.first_point;
        self.prev_umbra_index = -1;

        let mut prev_umbra_index = 0usize;
        self.add_inner_point(self.first_point, umbra_alpha, &umbra_polygon, &mut prev_umbra_index);
        self.prev_umbra_index = prev_umbra_index as i32;

        if !self.transparent && do_clip {
            let is_outside = self
                .clip_umbra_point(self.mesh.positions[self.first_vertex_index], self.centroid)
                .map(|clip_point| {
                    self.push_vertex(clip_point, umbra_alpha);
                    true
                })
                .unwrap_or(false);
            self.prev_umbra_outside = is_outside;
            self.first_umbra_outside = is_outside;
        }

        self.push_vertex(self.first_point + self.first_outset, 0.);
        self.add_edge(
            self.path_polygon[0],
            self.first_outset,
            umbra_alpha,
            &umbra_polygon,
            false,
            do_clip,
        );

        for i in 1..n {
            let Some(mut normal) =
                Self::compute_normal(self.prev_point, self.path_polygon[i], self.direction)
            else {
                return false;
            };
            normal = normal * outset;
            self.add_arc(normal, outset, true);
            self.add_edge(
                self.path_polygon[i],
                normal,
                umbra_alpha,
                &umbra_polygon,
                i == n - 1,
                do_clip,
            );
        }
        debug_assert!(!self.mesh.indices.is_empty());

        // final fan
        debug_assert!(self.mesh.positions.len() >= 3);
        if self.add_arc(self.first_outset, outset, false) {
            let last = self.mesh.positions.len() - 1;
            if self.first_umbra_outside {
                self.append_triangle(
                    self.first_vertex_index as u16,
                    last as u16,
                    self.first_vertex_index as u16 + 2,
                );
            } else {
                self.append_triangle(
                    self.first_vertex_index as u16,
                    last as u16,
                    self.first_vertex_index as u16 + 1,
                );
            }
        } else {
            let last_pos = *self.mesh.positions.last().unwrap();
            if self.first_umbra_outside {
                self.mesh.positions[self.first_vertex_index + 2] = last_pos;
            } else {
                self.mesh.positions[self.first_vertex_index + 1] = last_pos;
            }
        }
        true
    }

    /// `computeConcaveShadow` — umbra/penumbra rings from
    /// `offset_simple_polygon`, stitched pairwise.
    fn compute_concave_shadow(&mut self, inset: f32, outset: f32) -> bool {
        if !is_simple_polygon(&self.path_polygon) {
            return false;
        }
        let half = (self.path_bounds.2 - self.path_bounds.0)
            .abs()
            .min((self.path_bounds.3 - self.path_bounds.1).abs())
            / 2.;
        let inset = inset.min(half);

        let Some((umbra_polygon, mut umbra_indices)) =
            offset_simple_polygon(&self.path_polygon, half, inset)
        else {
            return false;
        };
        let Some((penumbra_polygon, mut penumbra_indices)) =
            offset_simple_polygon(&self.path_polygon, half, -outset)
        else {
            return false;
        };
        if umbra_polygon.is_empty() || penumbra_polygon.is_empty() {
            return false;
        }
        self.stitch_concave_rings(
            &umbra_polygon,
            &mut umbra_indices,
            &penumbra_polygon,
            &mut penumbra_indices,
        );
        true
    }

    /// `stitchConcaveRings` — walk both rings from their lowest source index,
    /// emitting quads where both advance and triangles to fill arcs.
    fn stitch_concave_rings(
        &mut self,
        umbra_polygon: &[Pt],
        umbra_indices: &mut [i32],
        penumbra_polygon: &[Pt],
        penumbra_indices: &mut [i32],
    ) {
        let mut index_map = alloc::vec![0u16; umbra_polygon.len()];

        // align both index sequences at a shared source-vertex index
        let mut min_index = 0;
        let mut min = penumbra_indices[0];
        for i in 1..penumbra_indices.len() {
            if penumbra_indices[i] < min {
                min = penumbra_indices[i];
                min_index = i;
            }
        }
        let mut curr_penumbra = min_index;
        min_index = 0;
        min = umbra_indices[0];
        for i in 1..umbra_indices.len() {
            if umbra_indices[i] < min {
                min = umbra_indices[i];
                min_index = i;
            }
        }
        let mut curr_umbra = min_index;

        let n = self.path_polygon.len() as i32;
        let mut max_penumbra_index = n - 1;
        let mut max_umbra_index = n - 1;
        while penumbra_indices[curr_penumbra] != umbra_indices[curr_umbra] {
            if penumbra_indices[curr_penumbra] < umbra_indices[curr_umbra] {
                penumbra_indices[curr_penumbra] += n;
                max_penumbra_index = penumbra_indices[curr_penumbra];
                curr_penumbra = (curr_penumbra + 1) % penumbra_polygon.len();
            } else {
                umbra_indices[curr_umbra] += n;
                max_umbra_index = umbra_indices[curr_umbra];
                curr_umbra = (curr_umbra + 1) % umbra_polygon.len();
            }
        }

        self.push_vertex(penumbra_polygon[curr_penumbra], 0.);
        let mut prev_penumbra_index = 0u16;
        self.push_vertex(umbra_polygon[curr_umbra], 1.);
        self.prev_umbra_index = 1;
        index_map[curr_umbra] = 1;

        let mut next_penumbra = (curr_penumbra + 1) % penumbra_polygon.len();
        let mut next_umbra = (curr_umbra + 1) % umbra_polygon.len();
        while penumbra_indices[next_penumbra] <= max_penumbra_index
            || umbra_indices[next_umbra] <= max_umbra_index
        {
            if umbra_indices[next_umbra] == penumbra_indices[next_penumbra] {
                let curr_penumbra_index = self.push_vertex(penumbra_polygon[next_penumbra], 0.);
                let curr_umbra_index = self.push_vertex(umbra_polygon[next_umbra], 1.);
                index_map[next_umbra] = curr_umbra_index;
                self.append_quad(
                    prev_penumbra_index,
                    curr_penumbra_index,
                    self.prev_umbra_index as u16,
                    curr_umbra_index,
                );
                prev_penumbra_index = curr_penumbra_index;
                penumbra_indices[curr_penumbra] += n;
                curr_penumbra = next_penumbra;
                next_penumbra = (curr_penumbra + 1) % penumbra_polygon.len();
                self.prev_umbra_index = curr_umbra_index as i32;
                umbra_indices[curr_umbra] += n;
                curr_umbra = next_umbra;
                next_umbra = (curr_umbra + 1) % umbra_polygon.len();
            }
            while penumbra_indices[next_penumbra] < umbra_indices[next_umbra]
                && penumbra_indices[next_penumbra] <= max_penumbra_index
            {
                let curr_penumbra_index = self.push_vertex(penumbra_polygon[next_penumbra], 0.);
                self.append_triangle(
                    prev_penumbra_index,
                    curr_penumbra_index,
                    self.prev_umbra_index as u16,
                );
                prev_penumbra_index = curr_penumbra_index;
                penumbra_indices[curr_penumbra] += n;
                curr_penumbra = next_penumbra;
                next_penumbra = (curr_penumbra + 1) % penumbra_polygon.len();
            }
            while umbra_indices[next_umbra] < penumbra_indices[next_penumbra]
                && umbra_indices[next_umbra] <= max_umbra_index
            {
                let curr_umbra_index = self.push_vertex(umbra_polygon[next_umbra], 1.);
                index_map[next_umbra] = curr_umbra_index;
                self.append_triangle(
                    self.prev_umbra_index as u16,
                    prev_penumbra_index,
                    curr_umbra_index,
                );
                self.prev_umbra_index = curr_umbra_index as i32;
                umbra_indices[curr_umbra] += n;
                curr_umbra = next_umbra;
                next_umbra = (curr_umbra + 1) % umbra_polygon.len();
            }
        }
        let curr_penumbra_index = self.push_vertex(penumbra_polygon[next_penumbra], 0.);
        let curr_umbra_index = self.push_vertex(umbra_polygon[next_umbra], 1.);
        index_map[next_umbra] = curr_umbra_index;
        self.append_quad(
            prev_penumbra_index,
            curr_penumbra_index,
            self.prev_umbra_index as u16,
            curr_umbra_index,
        );

        if self.transparent {
            triangulate_simple_polygon(umbra_polygon, &index_map, &mut self.mesh.indices);
        }
    }
}

/// `kA..kD`: cubic Bézier coefficients at `t = 5/16` (and reversed for
/// `t = 11/16`), used for interior samples of the spot clip polygon.
const CLIP_A: f32 = 0.32495117187;
const CLIP_B: f32 = 0.44311523437;
const CLIP_C: f32 = 0.20141601562;
const CLIP_D: f32 = 0.03051757812;

/// Flatten `cubic` (already transformed to draw space) through `handle_line`
/// at [`TESSELLATION_TOLERANCE`].
fn flatten_cubic_to(cubic: &Cubic, t: &mut BaseTessellator, first: bool) {
    let [ax, ay, c0x, c0y, c1x, c1y, bx, by] = cubic.points;
    let p0 = pt(ax, ay);
    let c0 = pt(c0x, c0y);
    let c1 = pt(c1x, c1y);
    let p1 = pt(bx, by);
    if first {
        t.handle_line(p0);
    }
    flatten_cubic_rec(p0, c0, c1, p1, TESSELLATION_TOLERANCE, t, 0);
}

fn flatten_cubic_rec(
    p0: Pt,
    c0: Pt,
    c1: Pt,
    p1: Pt,
    tol: f32,
    t: &mut BaseTessellator,
    depth: u32,
) {
    // flatness: both control points within tol of the baseline
    let base = p1 - p0;
    let flat = |c: Pt| {
        let cross_d = cross(base, c - p0).abs();
        let len2 = base.square_length().max(1e-12);
        cross_d / len2.sqrt()
    };
    if depth < 16 && (flat(c0) > tol || flat(c1) > tol) {
        // de Casteljau at t = 1/2
        let m0 = mid(p0, c0);
        let m1 = mid(c0, c1);
        let m2 = mid(c1, p1);
        let q0 = mid(m0, m1);
        let q1 = mid(m1, m2);
        let r = mid(q0, q1);
        flatten_cubic_rec(p0, m0, q0, r, tol, t, depth + 1);
        flatten_cubic_rec(r, q1, m2, p1, tol, t, depth + 1);
    } else {
        t.handle_line(p1);
    }
}

fn bounds_of(poly: &[Pt]) -> (f32, f32, f32, f32) {
    let mut b = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for p in poly {
        b.0 = b.0.min(p.x);
        b.1 = b.1.min(p.y);
        b.2 = b.2.max(p.x);
        b.3 = b.3.max(p.y);
    }
    b
}

/// `SkShadowTessellator::MakeAmbient` (non-perspective): tessellate the caster
/// path polygon under `ctm` (typically the item transform without translation)
/// into umbra/penumbra rings.
fn ambient_mesh_tessellated(
    cubics: &[Cubic],
    ctm: &Affine,
    z: f32,
    transparent: bool,
) -> Option<ShadowMesh> {
    let outset = ambient_blur_radius(z);
    let inset = outset * ambient_recip_alpha(z) - outset;

    let mut t = BaseTessellator::new(bounds_of_cubics(cubics), transparent);
    for (i, c) in cubics.iter().enumerate() {
        let tc = transform_cubic(c, ctm);
        flatten_cubic_to(&tc, &mut t, i == 0);
    }
    t.finish_path_polygon();
    if t.path_polygon.len() < 3 || !t.area.is_finite() {
        // Skia returns an empty SkVertices in these degenerate cases.
        return Some(ShadowMesh::default());
    }
    if t.is_convex {
        if !t.compute_convex_shadow(inset, outset, false) {
            return None;
        }
    } else if !t.compute_concave_shadow(inset, outset) {
        return None;
    }
    Some(t.mesh)
}

fn bounds_of_cubics(cubics: &[Cubic]) -> (f32, f32, f32, f32) {
    let mut b = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for c in cubics {
        for i in 0..4 {
            let x = c.points[2 * i];
            let y = c.points[2 * i + 1];
            b.0 = b.0.min(x);
            b.1 = b.1.min(y);
            b.2 = b.2.max(x);
            b.3 = b.3.max(y);
        }
    }
    b
}

fn transform_cubic(c: &Cubic, m: &Affine) -> Cubic {
    let mut out = c.points;
    for i in 0..4 {
        let p = m.map_point(pt(c.points[2 * i], c.points[2 * i + 1]));
        out[2 * i] = p.x;
        out[2 * i + 1] = p.y;
    }
    Cubic { points: out }
}

/// The occluder classification of `SpotVerticesFactory::OccluderType` —
/// decides the tessellation path and whether the canonical-light cache trick
/// is legal.
#[derive(Clone, Copy, PartialEq, Debug)]
enum SpotOccluder {
    /// Umbra can't be dropped: caster not opaque or its center is visible.
    Transparent,
    /// Entire umbra occluded — safe to drop it.
    OpaqueNoUmbra,
    /// Part of the umbra may show: clip it to the caster silhouette.
    OpaquePartialUmbra,
}

/// `SkShadowTessellator::MakeSpot` (non-perspective, point light). `cubics`
/// are the caster outline in item space, `ctm` the item→window transform.
/// Returns the mesh plus the offset to draw it at (the canonical-light trick:
/// the mesh is tessellated under a light centered over the path and the real
/// light enters only through the returned offset).
fn spot_mesh_tessellated(
    cubics: &[Cubic],
    ctm: &Affine,
    z: f32,
    light_pos: [f32; 3],
    light_radius: f32,
    caster_transparent: bool,
) -> Option<(ShadowMesh, Vec2)> {
    if !(light_pos[2] >= SK_SCALAR_NEARLY_ZERO
        && light_radius.is_finite()
        && light_radius >= SK_SCALAR_NEARLY_ZERO)
    {
        return None;
    }
    let local_bounds = bounds_of_cubics(cubics);
    let local_center =
        pt((local_bounds.0 + local_bounds.2) / 2., (local_bounds.1 + local_bounds.3) / 2.);
    let dev_center = ctm.map_point(local_center);

    // `GetSpotParams` on the light relative to the caster's device center.
    let rel_x = light_pos[0] - dev_center.x;
    let rel_y = light_pos[1] - dev_center.y;
    let (radius_blur, scale, rel_offset) = spot_params(z, rel_x, rel_y, light_pos[2], light_radius);
    let _ = scale;

    let no_trans = ctm.without_translation();

    // `path.isConvex()`: classify the occluder before choosing the
    // tessellation transform. Convexity is affine-invariant, so the flattened
    // no-trans polygon answers it for every candidate transform.
    let dev_bounds = {
        let mut b = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        for corner in [
            pt(local_bounds.0, local_bounds.1),
            pt(local_bounds.2, local_bounds.1),
            pt(local_bounds.2, local_bounds.3),
            pt(local_bounds.0, local_bounds.3),
        ] {
            let p = ctm.map_point(corner);
            b.0 = b.0.min(p.x);
            b.1 = b.1.min(p.y);
            b.2 = b.2.max(p.x);
            b.3 = b.3.max(p.y);
        }
        b
    };
    let mut convex_check = BaseTessellator::new(local_bounds, false);
    for (i, c) in cubics.iter().enumerate() {
        flatten_cubic_to(&transform_cubic(c, &no_trans), &mut convex_check, i == 0);
    }
    convex_check.finish_path_polygon();
    let occluder = if caster_transparent
        || rel_offset.x.abs() > 0.5 * (dev_bounds.2 - dev_bounds.0)
        || rel_offset.y.abs() > 0.5 * (dev_bounds.3 - dev_bounds.1)
    {
        SpotOccluder::Transparent
    } else if rel_offset.length() * scale + scale < radius_blur {
        SpotOccluder::OpaqueNoUmbra
    } else if is_convex_polygon(&convex_check.path_polygon) {
        SpotOccluder::OpaquePartialUmbra
    } else {
        SpotOccluder::Transparent
    };

    let transparent = occluder == SpotOccluder::Transparent;

    // shadow transform + clip transform + draw offset, per occluder class
    let (shadow_ctm, clip_ctm, draw_offset) = match occluder {
        SpotOccluder::OpaquePartialUmbra => {
            // real light, full ctm: mesh lands directly in window space
            let z_ratio = divide_and_pin(z, light_pos[2] - z, 0., 0.95);
            let st = Affine::scale_translate(
                scale,
                scale,
                -z_ratio * light_pos[0],
                -z_ratio * light_pos[1],
            );
            (st.pre_concat(ctm), *ctm, vec2(0., 0.))
        }
        _ => {
            // canonical light centered over the (translation-stripped) path
            let cl = no_trans.map_point(local_center);
            let z_ratio = divide_and_pin(z, light_pos[2] - z, 0., 0.95);
            let st = Affine::scale_translate(scale, scale, -z_ratio * cl.x, -z_ratio * cl.y);
            let offset = rel_offset + vec2(ctm.tx, ctm.ty);
            (st.pre_concat(&no_trans), no_trans, offset)
        }
    };

    let outset = spot_blur_radius(z, light_pos[2], light_radius);
    let inset = outset;

    let mut t = BaseTessellator::new(local_bounds, transparent);
    for (i, c) in cubics.iter().enumerate() {
        // clip polygon: untransformed outline with cubic interior samples at
        // t = 5/16 and t = 11/16 — the curve start is covered by the previous
        // cubic's end (the path is closed)
        let clip_pts: Vec<Pt> =
            (0..4).map(|k| clip_ctm.map_point(pt(c.points[2 * k], c.points[2 * k + 1]))).collect();
        let curve_point = |w: [f32; 4]| {
            pt(
                w[0] * clip_pts[0].x
                    + w[1] * clip_pts[1].x
                    + w[2] * clip_pts[2].x
                    + w[3] * clip_pts[3].x,
                w[0] * clip_pts[0].y
                    + w[1] * clip_pts[1].y
                    + w[2] * clip_pts[2].y
                    + w[3] * clip_pts[3].y,
            )
        };
        let cp0 = curve_point([CLIP_A, CLIP_B, CLIP_C, CLIP_D]);
        let cp1 = curve_point([CLIP_D, CLIP_C, CLIP_B, CLIP_A]);
        for cp in [cp0, cp1, clip_pts[3]] {
            if t.clip_polygon.is_empty() || !duplicate_pt(cp, *t.clip_polygon.last().unwrap()) {
                t.clip_polygon.push(cp);
            }
        }

        let tc = transform_cubic(c, &shadow_ctm);
        flatten_cubic_to(&tc, &mut t, i == 0);
    }
    t.finish_path_polygon();
    if t.clip_polygon.len() < 3 || t.path_polygon.len() < 3 || !t.area.is_finite() {
        return Some((ShadowMesh::default(), draw_offset));
    }
    if t.is_convex {
        if !t.compute_convex_shadow(inset, outset, true) {
            return None;
        }
    } else if !t.compute_concave_shadow(inset, outset) {
        return None;
    }
    Some((t.mesh, draw_offset))
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// The quartic approximation of the ambient/spot alpha falloff
/// (`gauss_a_to_rgba` in `SkRasterPipeline_opts.h`): evaluates
/// `exp(-(1-a)²·4) - 0.018` as a Horner-form quartic in `a`.
pub fn gauss_falloff(a: f32) -> f32 {
    const C4: f32 = -2.26661229133605957031;
    const C3: f32 = 2.89795351028442382812;
    const C2: f32 = 0.21345567703247070312;
    const C1: f32 = 0.15489584207534790039;
    const C0: f32 = 0.00030726194381713867;
    a * (a * (a * (a * C4 + C3) + C2) + C1) + C0
}

/// A 256-entry LUT of [`gauss_falloff`], indexed by alpha `0..=255`.
static GAUSS_LUT: &[f32; 256] = &{
    let mut lut = [0f32; 256];
    let mut i = 0;
    while i < 256 {
        let a = i as f32 / 255.;
        // const-eval of the Horner quartic
        const C4: f32 = -2.26661229133605957031;
        const C3: f32 = 2.89795351028442382812;
        const C2: f32 = 0.21345567703247070312;
        const C1: f32 = 0.15489584207534790039;
        const C0: f32 = 0.00030726194381713867;
        lut[i] = a * (a * (a * (a * C4 + C3) + C2) + C1) + C0;
        i += 1;
    }
    lut
};

/// `gauss_falloff` on a `0..=1` alpha via the LUT.
pub fn gauss_falloff_lut(a: f32) -> f32 {
    let i = (a.clamp(0., 1.) * 255.).round() as usize;
    GAUSS_LUT[i.min(255)]
}

/// The Android per-window point light for elevation shadows, in logical px
/// window coordinates, plus the light radius.
///
/// `display` is the display's logical size and the window's top-left position
/// on it; when the platform can't report either (Wayland window position, MCU),
/// pass the window's own size and the origin — the light then sits centered on
/// the window's top edge, exactly as for a full-screen window.
pub fn window_light(
    display: euclid::Size2D<f32, LogicalPx>,
    window_pos: euclid::Point2D<f32, LogicalPx>,
) -> ([f32; 3], f32) {
    let w = display.width.max(1.);
    let h = display.height.max(1.);
    let z_ratio = w.min(h) / 450.;
    let light_z = 500. * (z_ratio + 2.) / 3.;
    ([w / 2. - window_pos.x, -window_pos.y, light_z], 800.)
}

/// Parameters of one elevation-shadow draw, resolved for a caster.
pub struct ElevationShadowParams {
    /// `elevation`: the caster's z height in logical px.
    pub z: f32,
    /// Light position in logical px window space.
    pub light_pos: [f32; 3],
    /// Light radius in logical px.
    pub light_radius: f32,
    /// Effective ambient color (alpha ×= 0.039·caster alpha applied by caller or here).
    pub ambient_color: Color,
    /// Effective spot color (alpha ×= 0.19).
    pub spot_color: Color,
    /// Whether the caster is translucent (`caster_alpha < 1`): keeps the
    /// centroid fan so the umbra shows through.
    pub transparent: bool,
}

/// Effective ambient shadow color: `ambient_color` with its alpha scaled by
/// `0.039 · caster_alpha` (alphas from `dimens.xml:712-713` at the pin).
pub fn effective_ambient_color(color: Color, caster_alpha: f32) -> Color {
    let a = (color.alpha() as f32 / 255.) * 0.039 * caster_alpha;
    color.with_alpha(a)
}

/// Effective spot shadow color: alpha scaled by `0.19 · caster_alpha`.
pub fn effective_spot_color(color: Color, caster_alpha: f32) -> Color {
    let a = (color.alpha() as f32 / 255.) * 0.19 * caster_alpha;
    color.with_alpha(a)
}

/// The caster outline as cubics in item space, via [`ElementOutline::for_each_path`].
fn outline_cubics(outline: &ElementOutline, target: euclid::Rect<f32, LogicalPx>) -> Vec<Cubic> {
    use crate::graphics::OutlinePathEl;
    let mut cubics = Vec::new();
    let mut first = Pt::default();
    let mut cur = Pt::default();
    let mut pen_down = false;
    outline.for_each_path(target, &mut |el| match el {
        OutlinePathEl::MoveTo(p) => {
            first = pt(p.x, p.y);
            cur = first;
            pen_down = true;
        }
        OutlinePathEl::LineTo(p) => {
            if pen_down {
                let p = pt(p.x, p.y);
                cubics.push(Cubic { points: [cur.x, cur.y, cur.x, cur.y, p.x, p.y, p.x, p.y] });
                cur = p;
            }
        }
        OutlinePathEl::CurveTo(c0, c1, p) => {
            if pen_down {
                cubics.push(Cubic { points: [cur.x, cur.y, c0.x, c0.y, c1.x, c1.y, p.x, p.y] });
                cur = pt(p.x, p.y);
            }
        }
        OutlinePathEl::Close => {
            if pen_down && cur != first {
                cubics.push(Cubic {
                    points: [cur.x, cur.y, cur.x, cur.y, first.x, first.y, first.x, first.y],
                });
            }
            pen_down = false;
        }
    });
    cubics
}

/// The ambient shadow mesh of an outline fitted into `rect` (item space),
/// under the item→window `ctm`. Returns the mesh and the offset to draw it at.
pub fn ambient_mesh(
    outline: &ElementOutline,
    rect: euclid::Rect<f32, LogicalPx>,
    ctm: &Affine,
    z: f32,
    transparent: bool,
) -> Option<(ShadowMesh, Vec2)> {
    let cubics = outline_cubics(outline, rect);
    if cubics.is_empty() || z < MIN_HEIGHT {
        return None;
    }
    let mesh = ambient_mesh_tessellated(&cubics, &ctm.without_translation(), z, transparent)?;
    Some((mesh, vec2(ctm.tx, ctm.ty)))
}

/// The spot shadow mesh of an outline fitted into `rect` (item space), under
/// the item→window `ctm` and the window-space `light`. Returns the mesh and
/// the offset to draw it at — `Some((mesh, offset))` with an empty mesh for
/// degenerate outlines.
pub fn spot_mesh(
    outline: &ElementOutline,
    rect: euclid::Rect<f32, LogicalPx>,
    ctm: &Affine,
    z: f32,
    light_pos: [f32; 3],
    light_radius: f32,
    caster_transparent: bool,
) -> Option<(ShadowMesh, Vec2)> {
    let cubics = outline_cubics(outline, rect);
    if cubics.is_empty() || z < MIN_HEIGHT {
        return None;
    }
    spot_mesh_tessellated(&cubics, ctm, z, light_pos, light_radius, caster_transparent)
}

/// `GetLocalBounds` (non-perspective): the item-space rect covering both
/// shadow meshes, outset by 1 for floating-point error — the caster's damage
/// region beyond its geometry.
pub fn shadow_local_bounds(
    outline_bounds: euclid::Rect<f32, LogicalPx>,
    ctm: &Affine,
    z: f32,
    light_pos: [f32; 3],
    light_radius: f32,
) -> euclid::Rect<f32, LogicalPx> {
    let mut ambient = outline_bounds;
    let occluder_z = z;
    let dev_to_src = 1. / ctm.min_scale().max(1e-6);
    let ambient_blur = ambient_blur_radius(occluder_z) * dev_to_src;

    // The spot bounds are computed in item space with the light in item
    // space (`fill_shadow_rec` inverse-maps the device light).
    let inv = ctm.inverse().unwrap_or(Affine::IDENTITY);
    let local_light = inv.map_point(pt(light_pos[0], light_pos[1]));
    let (spot_blur, spot_scale, spot_offset) =
        spot_params(occluder_z, local_light.x, local_light.y, light_pos[2], light_radius);
    let spot_blur = spot_blur * dev_to_src;

    let mut spot = euclid::rect(
        ambient.origin.x * spot_scale + spot_offset.x,
        ambient.origin.y * spot_scale + spot_offset.y,
        ambient.size.width * spot_scale,
        ambient.size.height * spot_scale,
    );
    ambient =
        ambient.outer_rect(euclid::SideOffsets2D::<f32, LogicalPx>::new_all_same(ambient_blur));
    spot = spot.outer_rect(euclid::SideOffsets2D::<f32, LogicalPx>::new_all_same(spot_blur));
    let mut bounds = ambient.union(&spot);
    bounds = bounds.outer_rect(euclid::SideOffsets2D::<f32, LogicalPx>::new_all_same(1.));
    bounds
}

/// Rasterize a shadow mesh into an A8 coverage mask for `bounds` (in the
/// mesh's coordinate space, i.e. before `offset` is applied by the caller).
///
/// Each pixel accumulates triangle coverage `C` and coverage-weighted
/// interpolated vertex alpha `A`; the stored alpha is `C · gauss(A/C)`, which
/// is Skia's pipeline (vertex-alpha interpolation → Gaussian color filter →
/// coverage) in one accumulation pass. Coverage is computed by 4×4
/// supersampling, matching the ~0.25px outline tolerance.
pub fn rasterize_shadow_mesh(mesh: &ShadowMesh, bounds: euclid::Rect<f32, LogicalPx>) -> Vec<u8> {
    let w = bounds.width().ceil() as usize;
    let h = bounds.height().ceil() as usize;
    if w == 0 || h == 0 || mesh.is_empty() {
        return Vec::new();
    }
    let mut cov = alloc::vec![0f32; w * h];
    let mut acc = alloc::vec![0f32; w * h];
    let ox = bounds.origin.x;
    let oy = bounds.origin.y;

    const SS: usize = 4; // 4×4 supersampling
    for tri in mesh.indices.chunks_exact(3) {
        let p0 = mesh.positions[tri[0] as usize];
        let p1 = mesh.positions[tri[1] as usize];
        let p2 = mesh.positions[tri[2] as usize];
        let a0 = mesh.alphas[tri[0] as usize];
        let a1 = mesh.alphas[tri[1] as usize];
        let a2 = mesh.alphas[tri[2] as usize];

        let min_x = ((p0.x.min(p1.x).min(p2.x) - ox).floor() as isize).max(0) as usize;
        let max_x =
            ((p0.x.max(p1.x).max(p2.x) - ox).ceil() as isize).min(w as isize).max(0) as usize;
        let min_y = ((p0.y.min(p1.y).min(p2.y) - oy).floor() as isize).max(0) as usize;
        let max_y =
            ((p0.y.max(p1.y).max(p2.y) - oy).ceil() as isize).min(h as isize).max(0) as usize;
        if max_x <= min_x || max_y <= min_y {
            continue;
        }
        let area = cross(p1 - p0, p2 - p0);
        if area.abs() < 1e-12 {
            continue;
        }
        for py in min_y..max_y {
            for px in min_x..max_x {
                let mut c = 0f32;
                let mut al = 0f32;
                for sy in 0..SS {
                    for sx in 0..SS {
                        let x = ox + px as f32 + (sx as f32 + 0.5) / SS as f32;
                        let y = oy + py as f32 + (sy as f32 + 0.5) / SS as f32;
                        let q = pt(x, y);
                        let w0 = cross(p1 - q, p2 - q) / area;
                        let w1 = cross(p2 - q, p0 - q) / area;
                        let w2 = cross(p0 - q, p1 - q) / area;
                        if w0 >= -1e-6 && w1 >= -1e-6 && w2 >= -1e-6 {
                            c += 1.;
                            al += w0 * a0 + w1 * a1 + w2 * a2;
                        }
                    }
                }
                if c > 0. {
                    let idx = py * w + px;
                    cov[idx] += c / (SS * SS) as f32;
                    acc[idx] += al / (SS * SS) as f32;
                }
            }
        }
    }

    // resolve: alpha = C · gauss(A / C)
    let mut out = alloc::vec![0u8; w * h];
    for i in 0..w * h {
        let c = cov[i];
        if c > 1e-6 {
            let a = acc[i] / c;
            out[i] = (c * gauss_falloff_lut(a).clamp(0., 1.) * 255.).round().min(255.) as u8;
        }
    }
    out
}

/// Where a mesh's [`rasterize_shadow_mesh`] bounds come from: the mesh's own
/// bounding box.
pub fn mesh_bounds(mesh: &ShadowMesh) -> euclid::Rect<f32, LogicalPx> {
    let b = bounds_of(&mesh.positions);
    if b.2 <= b.0 || b.3 <= b.1 {
        return euclid::Rect::zero();
    }
    euclid::rect(b.0.floor(), b.1.floor(), (b.2 - b.0).ceil(), (b.3 - b.1).ceil())
}

// ---------------------------------------------------------------------------
// Rasterized layers for mask-based renderers
// ---------------------------------------------------------------------------

/// One rasterized elevation-shadow layer (ambient or spot): an A8 coverage
/// mask plus the rectangle it draws at, both in `ctm`-target pixels.
#[derive(Clone)]
pub struct ShadowLayerMask {
    /// A8 coverage, row-major with stride == width.
    pub mask: alloc::rc::Rc<[u8]>,
    /// The mask's pixel size (stride == width).
    pub size: euclid::Size2D<u32, euclid::UnknownUnit>,
    /// The rect the mask covers, in `ctm`-target space — the rasterized mesh
    /// bounds translated by the mesh's draw offset.
    pub rect: euclid::Rect<f32, euclid::UnknownUnit>,
}

/// Both layers of an elevation shadow.
#[derive(Clone, Default)]
pub struct ElevationShadowMasks {
    /// The ambient layer, `None` for degenerate outlines or z < 0.1.
    pub ambient: Option<ShadowLayerMask>,
    /// The spot layer, `None` for degenerate outlines or z < 0.1.
    pub spot: Option<ShadowLayerMask>,
}

fn rasterized_layer(mesh_and_offset: Option<(ShadowMesh, Vec2)>) -> Option<ShadowLayerMask> {
    let (mesh, offset) = mesh_and_offset?;
    if mesh.is_empty() {
        return None;
    }
    let bounds = mesh_bounds(&mesh);
    let size = euclid::size2(bounds.width().ceil() as u32, bounds.height().ceil() as u32);
    if size.width == 0 || size.height == 0 {
        return None;
    }
    let mask = rasterize_shadow_mesh(&mesh, bounds);
    Some(ShadowLayerMask { mask: mask.into(), size, rect: bounds.cast_unit().translate(offset) })
}

/// Tessellate and rasterize both layers of an elevation shadow.
///
/// `ctm` maps the outline's item space to the rasterization space (physical
/// pixels for the software renderer, logical pixels for GPU renderers that
/// upscale on draw); `light_pos`/`light_radius` are in `ctm`-target space
/// (from [`window_light`], scaled by the same factor as `ctm`).
pub fn elevation_shadow_masks(
    outline: &ElementOutline,
    rect: euclid::Rect<f32, LogicalPx>,
    ctm: &Affine,
    z: f32,
    light_pos: [f32; 3],
    light_radius: f32,
    caster_transparent: bool,
) -> ElevationShadowMasks {
    ElevationShadowMasks {
        ambient: rasterized_layer(ambient_mesh(outline, rect, ctm, z, caster_transparent)),
        spot: rasterized_layer(spot_mesh(
            outline,
            rect,
            ctm,
            z,
            light_pos,
            light_radius,
            caster_transparent,
        )),
    }
}

/// The elevation light for a window, in physical pixels: `display_geometry`
/// is `(display size, window position)` from
/// `WindowAdapter::display_geometry`; when the backend can't report it,
/// `window_size` and the origin substitute (the light then sits centered on
/// the window's top edge, as for a full-screen window).
///
/// Returns the light position and radius.
pub fn elevation_light(
    display_geometry: Option<(crate::api::PhysicalSize, crate::api::PhysicalPosition)>,
    window_size: crate::api::PhysicalSize,
) -> ([f32; 3], f32) {
    let (dw, dh, wx, wy) = display_geometry
        .map(|(d, p)| (d.width as f32, d.height as f32, p.x as f32, p.y as f32))
        .unwrap_or((window_size.width as f32, window_size.height as f32, 0., 0.));
    window_light(euclid::size2(dw, dh), euclid::point2(wx, wy))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lengths::LogicalBorderRadius;

    fn rect_outline() -> ElementOutline {
        ElementOutline::Rectangle(Default::default())
    }

    #[test]
    fn window_light_places_light_centered_above_display() {
        // Display 1600×1200, window at (100, 50):
        // zRatio = min(W,H)/450 = 2.6667, lightZ = 500·(zRatio+2)/3 ≈ 777.8.
        let (light, radius) = window_light(euclid::size2(1600., 1200.), euclid::point2(100., 50.));
        assert_eq!(radius, 800.);
        assert!((light[0] - 700.).abs() < 1e-4, "{}", light[0]);
        assert!((light[1] - (-50.)).abs() < 1e-4, "{}", light[1]);
        let expected_z = 500. * (1200. / 450. + 2.) / 3.;
        assert!((light[2] - expected_z).abs() < 1e-4, "{}", light[2]);
    }

    #[test]
    fn elevation_light_falls_back_to_window_size() {
        let size = crate::api::PhysicalSize::new(800, 600);
        let (light, radius) = elevation_light(None, size);
        assert_eq!(radius, 800.);
        assert!((light[0] - 400.).abs() < 1e-4);
        assert!(light[1].abs() < 1e-4);
    }

    #[test]
    fn ambient_metrics_match_skia_shadow_metrics() {
        // AmbientBlurRadius(z) = min(z/128·64, 150); AmbientRecipAlpha(z) = 1 + z/128.
        assert!((ambient_blur_radius(128.) - 64.).abs() < 1e-4);
        assert!((ambient_blur_radius(1.) - 0.5).abs() < 1e-4);
        assert!((ambient_blur_radius(400.) - 150.).abs() < 1e-4);
        assert!((ambient_recip_alpha(0.) - 1.).abs() < 1e-6);
        assert!((ambient_recip_alpha(128.) - 2.).abs() < 1e-4);
    }

    #[test]
    fn spot_metrics_match_skia_shadow_metrics() {
        // zRatio = pin(z / (lightZ − z), 0, 0.95); blur = radius·zRatio;
        // scale = pin(lightZ / (lightZ − z), 1, 1.95); offset = −zRatio·(lx,ly).
        let (blur, scale, offset) = spot_params(100., 200., -50., 800., 800.);
        let z_ratio = 100. / 700.;
        assert!((blur - 800. * z_ratio).abs() < 1e-3, "{}", blur);
        assert!((scale - 800. / 700.).abs() < 1e-4, "{}", scale);
        assert!((offset.x + z_ratio * 200.).abs() < 1e-3);
        assert!((offset.y - z_ratio * 50.).abs() < 1e-3);
        // Occluder near the light: zRatio pinned at 0.95, scale pinned at 1.95.
        let (blur2, scale2, _) = spot_params(900., 0., 0., 1000., 800.);
        assert!((blur2 - 760.).abs() < 1e-3, "{}", blur2);
        assert!((scale2 - 1.95).abs() < 1e-4, "{}", scale2);
        // Occluder at or above the light: zRatio pins to 0 (no spot shadow).
        let (blur3, scale3, off3) = spot_params(2000., 0., 0., 1000., 800.);
        assert_eq!(blur3, 0.);
        assert_eq!(scale3, 1.);
        assert!(off3.length() == 0.);
    }

    #[test]
    fn gauss_falloff_is_a_unit_quartic() {
        // exp(-4) − 0.018 ≈ 0 at a=0 and ~1 at a=1, monotonic between.
        assert!(gauss_falloff(0.) < 0.01, "{}", gauss_falloff(0.));
        assert!((gauss_falloff(1.) - 1.).abs() < 1e-3, "{}", gauss_falloff(1.));
        let mut prev = 0.;
        for i in 1..=16 {
            let v = gauss_falloff(i as f32 / 16.);
            assert!(v >= prev, "falloff not monotonic at {}", i);
            prev = v;
        }
        // The LUT mirrors the function within quantization error
        // (±1/255 of `a`, amplified by the quartic's slope ≈ 2).
        for i in 0..=255 {
            let a = i as f32 / 255.;
            assert!((gauss_falloff_lut(a) - gauss_falloff(a)).abs() < 0.01, "LUT diverges at {a}");
        }
    }

    #[test]
    fn ambient_mesh_of_rect_has_valid_indices_and_alphas() {
        let outline = rect_outline();
        let rect = euclid::rect(0., 0., 100., 80.);
        let ctm = Affine::scale_translate(1., 1., 30., 40.);
        let (mesh, offset) = ambient_mesh(&outline, rect, &ctm, 24., false).unwrap();
        assert!(!mesh.indices.is_empty());
        assert_eq!(mesh.indices.len() % 3, 0);
        assert_eq!(offset, vec2(30., 40.));
        // Mesh is in canonical (untranslated) space: bounds are the rect
        // outset by the ambient blur plus outline error.
        let b = mesh_bounds(&mesh);
        let outset = ambient_blur_radius(24.) + 4.;
        assert!(b.origin.x > -outset && b.origin.y > -outset);
        assert!(b.max_x() < 100. + outset && b.max_y() < 80. + outset);
        for &a in &mesh.alphas {
            assert!((-0.05..=1.05).contains(&a), "alpha {a} out of range");
        }
        assert!(mesh.indices.iter().all(|&i| (i as usize) < mesh.positions.len()));
    }

    #[test]
    fn spot_mesh_offsets_away_from_light() {
        // Light at top center (y<0), slightly left: the spot shadow lands
        // down-right of the caster.
        let outline = rect_outline();
        let rect = euclid::rect(0., 0., 100., 80.);
        let ctm = Affine::IDENTITY;
        let (mesh, offset) =
            spot_mesh(&outline, rect, &ctm, 24., [-200., -400., 800.], 800., false).unwrap();
        assert!(!mesh.indices.is_empty());
        assert!(offset.x > 0., "{}", offset.x);
        assert!(offset.y > 0., "{}", offset.y);
    }

    #[test]
    fn masks_cover_caster_outset_and_opaque_umbra() {
        let outline = rect_outline();
        let rect = euclid::rect(0., 0., 90., 90.);
        let ctm = Affine::scale_translate(1., 1., 30., 30.);
        let masks =
            elevation_shadow_masks(&outline, rect, &ctm, 24., [200., -100., 800.], 800., false);
        let ambient = masks.ambient.expect("ambient layer");
        let spot = masks.spot.expect("spot layer");
        // Ambient mask bounds the caster outset by the ambient blur.
        assert!(ambient.rect.min_x() < 30. - 5.);
        assert!(ambient.rect.min_y() < 30. - 5.);
        assert!(ambient.rect.max_x() > 30. + 90. + 5.);
        assert!(ambient.rect.max_y() > 30. + 90. + 5.);
        // Deep umbra: the center of a filled caster is opaque.
        let max = ambient.mask.iter().copied().max().unwrap();
        assert!(max > 200, "max alpha {max}");
        // The spot mask's center is displaced away from the light
        // (−zRatio·light_xy = (−6.2, +3.1) at z=24, light=[200,−100]).
        let dx = spot.rect.center().x - ambient.rect.center().x;
        let dy = spot.rect.center().y - ambient.rect.center().y;
        assert!(dy > 1. && dy < 6., "dy {dy}");
        assert!(dx < -3. && dx > -10., "dx {dx}");
    }

    #[test]
    fn below_min_height_yields_no_meshes() {
        let outline = rect_outline();
        let rect = euclid::rect(0., 0., 90., 90.);
        let masks = elevation_shadow_masks(
            &outline,
            rect,
            &Affine::IDENTITY,
            0.05,
            [0., 0., 800.],
            800.,
            false,
        );
        assert!(masks.ambient.is_none());
        assert!(masks.spot.is_none());
    }

    #[test]
    fn local_bounds_contain_caster() {
        let caster = euclid::rect(10., 10., 90., 90.);
        let bounds = shadow_local_bounds(caster, &Affine::IDENTITY, 24., [0., 0., 500.], 800.);
        assert!(bounds.contains(euclid::point2(10., 10.)));
        assert!(bounds.contains(euclid::point2(99., 99.)));
        // And it outsets meaningfully below for the spot layer.
        assert!(bounds.max_y() > 99. + 5.);
    }

    #[test]
    fn effective_colors_scale_alpha() {
        let c = Color::from_argb_u8(255, 10, 20, 30);
        let amb = effective_ambient_color(c, 1.);
        assert!((amb.alpha() as f32 - 255. * 0.039).abs() < 0.51, "{}", amb.alpha());
        let spot = effective_spot_color(c, 0.5);
        assert!((spot.alpha() as f32 - 255. * 0.19 * 0.5).abs() < 0.51, "{}", spot.alpha());
    }
}
