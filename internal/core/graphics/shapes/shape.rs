// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore androidx

//! The `Shape` value type: the flattened, `#[repr(C)]` representation of a closed
//! outline of cubic Bézier curves segmented into edges and corners. This is the
//! canonical form exchanged with the `.slint` language, the language bindings and
//! JSON serialization, plus the content-keyed morph cache used by shape animations.

use super::ShapeError;
use super::cubic::Cubic;
use super::feature::{Feature, detect_features};
use super::morph::Morph;
use super::next_shape_id;
use super::rounded_polygon::RoundedPolygon;
use super::svg::SvgPathParser;
use super::utils::{Point, PointTransformer, interpolate, k_cos, k_sin};
use crate::SharedVector;
use crate::items::FillRule;
use alloc::collections::VecDeque;
use alloc::rc::Rc;
use alloc::string::String;
use alloc::vec::Vec;
use core::cell::RefCell;

/// A point in a shape's unitless coordinate space, `#[repr(C)]` for FFI.
///
/// Unlike [`crate::graphics::Point`], this is always `f32`-based: shapes are stored
/// and morphed in their own normalized space before being fitted to an element's
/// geometry, and must not be quantized by the `slint_int_coord` configuration.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
#[repr(C)]
pub struct ShapePoint {
    /// The x coordinate.
    pub x: f32,
    /// The y coordinate.
    pub y: f32,
}

/// The kind of a [Feature] in the flattened representation.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
#[repr(C)]
pub enum ShapeFeatureKind {
    /// An edge: straight segment between corners.
    #[default]
    Edge = 0,
    /// A corner with outward (convex) indentation.
    ConvexCorner = 1,
    /// A corner with inward (concave) indentation.
    ConcaveCorner = 2,
}

/// A feature in the flattened [`Shape`] representation: a range of cubics plus its
/// kind. `cubic_start`/`cubic_len` index into [`Shape::cubics`] (in units of cubics —
/// multiply by 8 for float indices).
#[derive(Copy, Clone, Debug, Default, PartialEq)]
#[repr(C)]
pub struct ShapeFeature {
    /// Index of this feature's first cubic.
    pub cubic_start: u32,
    /// Number of cubics in this feature.
    pub cubic_len: u32,
    /// The feature kind (edge, convex corner, concave corner).
    pub kind: ShapeFeatureKind,
}

/// A rounded-polygon shape value: an immutable description of a closed outline made
/// of cubic Bézier curves, segmented into edges and corners for morph feature
/// matching. `#[repr(C)]` like `PathData`: it is the payload of `Value::Shape`, of
/// `.slint` `shape` properties, and of the C++/JSON interchange forms.
///
/// `Shape` is cheap to clone (its vectors are reference counted) and compares by
/// content. Construction functions live in the `Shapes` builtin namespace
/// (`.slint`) and in `slint::shapes` (Rust); see [`crate::graphics::shapes`].
#[derive(Clone, Default)]
#[repr(C)]
pub struct Shape {
    // The fields are read-only from outside the module: `content_hash`/`id` are
    // computed at construction and keyed to the payload, so mutating them (or
    // the payload behind them) would let the morph cache return a stale match.
    /// The cubic Bézier outline: 8 floats per cubic
    /// (`anchor0x, anchor0y, control0x, control0y, control1x, control1y, anchor1x, anchor1y`).
    cubics: SharedVector<f32>,
    /// The feature segmentation of the outline.
    features: SharedVector<ShapeFeature>,
    /// The shape's center (used for max-bounds and as the morphing anchor).
    center: ShapePoint,
    /// FNV-1a hash over the canonical content (cubic `to_bits`, feature ranges
    /// and kinds, center), computed once at construction — the morph cache's
    /// content key. `0` when built outside `Shape::new` (`Default`, FFI).
    content_hash: u64,
    /// Interned construction id from a thread-local counter — `0` means "no id"
    /// (`Default`, FFI, deserialization). Equal ids imply the same construction,
    /// so the morph cache compares them without touching the outline data.
    id: u64,
    /// The fill rule a renderer applies when filling this shape's outline
    /// (`nonzero` unless the shape was built by `Shapes.path(evenodd, …)`).
    /// It is part of the value — equality and serialization preserve it — but
    /// not of the geometry, so it is excluded from `content_hash`.
    fill_rule: FillRule,
}

impl PartialEq for Shape {
    /// Content equality: the morph-cache metadata (`content_hash`, `id`) is not
    /// part of the value.
    fn eq(&self, other: &Self) -> bool {
        self.cubics == other.cubics
            && self.features == other.features
            && self.center == other.center
            && self.fill_rule == other.fill_rule
    }
}

impl Shape {
    /// A shape from its flattened parts. Validates that the outline is
    /// structurally consistent: `cubics.len()` is a multiple of 8 and every
    /// feature range is in bounds.
    pub fn new(
        cubics: SharedVector<f32>,
        features: SharedVector<ShapeFeature>,
        center: ShapePoint,
    ) -> Result<Shape, ShapeError> {
        if !cubics.len().is_multiple_of(8) {
            return Err(ShapeError::new("Shape cubics length is not a multiple of 8"));
        }
        let cubic_count = cubics.len() / 8;
        for f in features.iter() {
            if (f.cubic_start + f.cubic_len) as usize > cubic_count {
                return Err(ShapeError::new("Shape feature range out of bounds"));
            }
        }
        let content_hash = hash_parts(&cubics, &features, &center);
        Ok(Shape {
            cubics,
            features,
            center,
            content_hash,
            id: next_shape_id(),
            fill_rule: FillRule::Nonzero,
        })
    }

    /// A shape from a [RoundedPolygon].
    pub fn from_polygon(polygon: &RoundedPolygon) -> Shape {
        polygon_to_shape(polygon)
    }

    /// The empty shape: a degenerate outline with no features.
    pub fn empty() -> Shape {
        Shape::default()
    }

    /// Whether this shape has an empty outline.
    pub fn is_empty(&self) -> bool {
        self.cubics.is_empty()
    }

    /// A shape from its flattened parts plus fill rule. See [`Shape::new`] and
    /// [`Shape::fill_rule`].
    pub fn new_with_fill_rule(
        cubics: SharedVector<f32>,
        features: SharedVector<ShapeFeature>,
        center: ShapePoint,
        fill_rule: FillRule,
    ) -> Result<Shape, ShapeError> {
        let mut shape = Self::new(cubics, features, center)?;
        shape.fill_rule = fill_rule;
        Ok(shape)
    }

    /// A shape from its flattened parts, without a content hash or construction
    /// id — the value an FFI/deserialization path produces when it fills the
    /// `repr(C)` payload by hand. The hash is then computed on demand and the
    /// id fast path of the morph cache is skipped.
    #[doc(hidden)]
    pub fn new_unkeyed(
        cubics: SharedVector<f32>,
        features: SharedVector<ShapeFeature>,
        center: ShapePoint,
    ) -> Shape {
        Shape { cubics, features, center, ..Shape::default() }
    }

    /// The cubic Bézier outline: 8 floats per cubic
    /// (`anchor0x, anchor0y, control0x, control0y, control1x, control1y, anchor1x, anchor1y`).
    pub fn cubics(&self) -> &SharedVector<f32> {
        &self.cubics
    }

    /// The feature segmentation of the outline.
    pub fn features(&self) -> &SharedVector<ShapeFeature> {
        &self.features
    }

    /// The shape's center (used for max-bounds and as the morphing anchor).
    pub fn center(&self) -> ShapePoint {
        self.center
    }

    /// The fill rule a renderer applies when filling this shape's outline
    /// (`nonzero` unless the shape was built by `Shapes.path(evenodd, …)`).
    pub fn fill_rule(&self) -> FillRule {
        self.fill_rule
    }

    /// The interned construction id, `0` for unkeyed values. See [`Shape::id`].
    #[doc(hidden)]
    pub fn id(&self) -> u64 {
        self.id
    }

    /// Drops the construction id and stored content hash, so the morph cache's
    /// interned-id fast path can't fire and the hash is recomputed from the
    /// payload. Called on values entering Rust across a boundary where the
    /// fields could have been mutated behind the key (the C++ `repr(C)`
    /// payload, `Default`, hand-built POD).
    #[doc(hidden)]
    pub fn unkey(&mut self) {
        self.id = 0;
        self.content_hash = 0;
    }

    /// The [RoundedPolygon] for this shape. Returns an error if the payload is not
    /// a valid polygon (e.g. empty).
    pub fn polygon(&self) -> Result<RoundedPolygon, ShapeError> {
        data_to_polygon(self)
    }

    /// An FNV-1a hash over the full content (cubic bits, feature kinds, center).
    /// This is the morph cache's key: equal content hashes imply a shared morph.
    /// For values built through `Shape::new` this is the field computed at
    /// construction; for `Default`/FFI-built values it is computed on demand.
    pub fn content_hash(&self) -> u64 {
        if self.content_hash != 0 { self.content_hash } else { content_hash(self) }
    }

    /// Whether every coordinate in the outline is finite. Shapes containing NaN
    /// or infinite coordinates are never cached: their morphs are computed fresh
    /// each time (and degenerate to the target when unmeasurable).
    pub(crate) fn is_finite(&self) -> bool {
        self.cubics.iter().all(|v| v.is_finite())
            && self.center.x.is_finite()
            && self.center.y.is_finite()
    }

    /// The axis-aligned bounds `[left, top, right, bottom]`; empty for an invalid
    /// shape. See [RoundedPolygon::calculate_bounds].
    pub fn bounds(&self, approximate: bool) -> Option<[f32; 4]> {
        self.polygon().ok().map(|p| p.calculate_bounds(approximate))
    }

    /// The axis-aligned max bounds (square) or `None` for an invalid shape.
    /// See [RoundedPolygon::calculate_max_bounds].
    pub fn max_bounds(&self) -> Option<[f32; 4]> {
        self.polygon().ok().map(|p| p.calculate_max_bounds())
    }

    /// This shape transformed by `f` (see [RoundedPolygon::transformed]).
    ///
    /// A transform that produces an invalid outline (e.g. non-finite coordinates)
    /// logs a warning and returns the empty shape.
    pub fn transformed(&self, f: impl PointTransformer) -> Shape {
        match self.polygon().and_then(|p| p.transformed(&f)) {
            Ok(p) => {
                let mut shape = Shape::from_polygon(&p);
                shape.fill_rule = self.fill_rule;
                shape
            }
            Err(e) => {
                crate::debug_log!("Shapes: {e}");
                Shape::empty()
            }
        }
    }

    /// A shape parsed from an SVG path data string (the `path()` builtin).
    /// See [SvgPathParser::parse_features].
    ///
    /// A shape is a single closed outline: `svg_path` must not describe more
    /// than one outline (i.e. contain a second `m`/`M` section), and percentages
    /// are not supported — shapes have no reference box to resolve them against.
    pub fn from_svg_path(svg_path: &str, fill_rule: FillRule) -> Result<Shape, ShapeError> {
        if svg_path.contains('%') {
            return Err(ShapeError::new("percentages are not supported in shape paths"));
        }
        if super::svg::has_multiple_outlines(svg_path) {
            return Err(ShapeError::new("a shape path must describe a single outline"));
        }
        let features = SvgPathParser::parse_features(svg_path)?;
        let polygon = RoundedPolygon::from_features(features, None)?;
        let mut shape = Shape::from_polygon(&polygon);
        shape.fill_rule = fill_rule;
        Ok(shape)
    }

    /// Like [`Shape::from_svg_path`], but for the `.slint` runtime where `d` is
    /// not a compile-time literal: invalid input logs a warning and produces
    /// the empty shape instead of an error.
    #[doc(hidden)]
    pub fn from_svg_path_lossy(svg_path: &str, fill_rule: FillRule) -> Shape {
        match Self::from_svg_path(svg_path, fill_rule) {
            Ok(shape) => shape,
            Err(e) => {
                crate::debug_log!("Shapes.path: {e}");
                Shape::empty()
            }
        }
    }

    /// This shape rotated by `angle` degrees around its center.
    pub fn rotated(&self, angle: f32) -> Shape {
        let cx = self.center.x;
        let cy = self.center.y;
        self.transformed(move |x, y| {
            let dx = x - cx;
            let dy = y - cy;
            let rad = angle * (core::f32::consts::PI / 180.);
            let c = k_cos(rad);
            let s = k_sin(rad);
            Point { x: cx + dx * c - dy * s, y: cy + dx * s + dy * c }
        })
    }

    /// This shape scaled by (`scale_x`, `scale_y`) around its center.
    pub fn scaled(&self, scale_x: f32, scale_y: f32) -> Shape {
        let cx = self.center.x;
        let cy = self.center.y;
        self.transformed(move |x, y| Point {
            x: cx + (x - cx) * scale_x,
            y: cy + (y - cy) * scale_y,
        })
    }

    /// This shape translated by (`dx`, `dy`).
    pub fn translated(&self, dx: f32, dy: f32) -> Shape {
        self.transformed(move |x, y| Point { x: x + dx, y: y + dy })
    }

    /// This shape scaled to fit the (0,0)-(1,1) square ([RoundedPolygon::normalized]).
    /// A zero-sized shape can't be normalized: logs a warning and returns the
    /// empty shape.
    pub fn normalized(&self) -> Shape {
        match self.polygon().and_then(|p| p.normalized()) {
            Ok(p) => {
                let mut shape = Shape::from_polygon(&p);
                shape.fill_rule = self.fill_rule;
                shape
            }
            Err(e) => {
                crate::debug_log!("Shapes: {e}");
                Shape::empty()
            }
        }
    }

    /// A new shape morphing between `self` and `other` at `progress`, which may
    /// leave [0, 1] for animation overshoot.
    ///
    /// The morph match is content-keyed and cached: computing a morph at a new
    /// progress between the same two shape contents does not redo the
    /// measuring/feature-mapping work (see [`MorphCache`]).
    ///
    /// The resulting shape's center is the linear interpolation of the endpoints'
    /// centers; its features are re-detected on the morphed outline.
    pub fn morph(&self, other: &Shape, progress: f32) -> Shape {
        super::MORPH_CACHE.with(|c| self.morph_with(other, progress, &c.borrow()))
    }

    /// Like [`Shape::morph`] but using an explicit cache.
    pub fn morph_with(&self, other: &Shape, progress: f32, cache: &MorphCache) -> Shape {
        // Non-finite endpoints can't be measured (Kotlin throws): the morph
        // degenerates to the finite endpoint.
        if !self.is_finite() || !other.is_finite() {
            return if other.is_finite() { other.clone() } else { self.clone() };
        }
        let morph = cache.morph(self, other);
        // An unmeasurable (e.g. zero-sized) endpoint degenerates to the target.
        if morph.is_degenerate() {
            return other.clone();
        }
        morph_to_shape(&morph, progress, Some((self, other)))
    }

    /// The outline as an SVG path data string (a `d` attribute value).
    pub fn to_svg_path(&self) -> String {
        let mut out = String::from("M ");
        let mut first = true;
        for p in self.cubics.as_slice().as_chunks::<8>().0 {
            if first {
                first = false;
            } else {
                out.push(' ');
            }
            // Kotlin Float.toString() is approximated by Rust's shortest round-trip
            // f32 formatting — both are round-trippable decimal representations.
            for v in [p[0], p[1]] {
                out.push_str(&format_f32(v));
                out.push(' ');
            }
            out.push_str("C ");
            for v in [p[2], p[3], p[4], p[5], p[6], p[7]] {
                out.push_str(&format_f32(v));
                out.push(' ');
            }
        }
        out.push('Z');
        out
    }
}

impl core::fmt::Debug for Shape {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Shape")
            .field("cubics", &(self.cubics.len() / 8))
            .field("features", &self.features.len())
            .field("center", &self.center)
            .finish()
    }
}

fn format_f32(v: f32) -> String {
    if v % 1. == 0. && v.is_finite() {
        // Kotlin prints integral floats without a fraction ("1", not "1.0").
        let i = v as i64;
        alloc::format!("{i}")
    } else {
        alloc::format!("{v}")
    }
}

/// Morphing support for shape properties: `animate shape` interpolates through the
/// content-keyed [`MorphCache`], so the feature match between the two endpoints is
/// computed once per endpoint pair and per-frame interpolation is a cubic lerp.
impl crate::properties::InterpolatedPropertyValue for Shape {
    fn interpolate(&self, target_value: &Self, t: f32) -> Self {
        self.morph(target_value, t)
    }

    /// Mean per-anchor displacement of the matched morph, normalized by `self`'s
    /// measured perimeter — the scalar morph-progress distance used to carry
    /// velocity across a mid-morph retarget (design note R6).
    fn scalar_delta(&self, target_value: &Self) -> f32 {
        super::MORPH_CACHE.with(|c| c.borrow().morph(self, target_value).scalar_delta())
    }

    /// A physical `spring(damping_ratio, stiffness)` animates a shape on a single
    /// channel: morph progress measured in normalized anchor displacement, i.e.
    /// the mean per-anchor displacement over the morph's match pairs divided by
    /// the start shape's measured perimeter. The start sits at channel 0 and the
    /// target at that displacement, so the spring's velocity is the morph's
    /// per-anchor speed in start-perimeter fractions per second — the velocity
    /// a mid-morph retarget carries into the new pair (design note R6).
    fn channel_count(&self, _target_value: &Self) -> usize {
        1
    }

    fn write_start_channels(&self, _target_value: &Self, out: &mut [f32]) {
        debug_assert_eq!(out.len(), 1);
        out[0] = 0.;
    }

    fn write_target_channels(&self, start_value: &Self, out: &mut [f32]) {
        debug_assert_eq!(out.len(), 1);
        out[0] = start_value.scalar_delta(self);
    }

    fn rebuild_from_channels(&self, target_value: &Self, channels: &[f32]) -> Self {
        debug_assert_eq!(channels.len(), 1);
        let distance = self.scalar_delta(target_value);
        let t = if distance <= f32::EPSILON { 1. } else { channels[0] / distance };
        self.morph(target_value, t)
    }
}

/// A content-keyed cache of [Morph] instances between pairs of shapes.
///
/// The cache is keyed by the *content* of the endpoint shapes — two `Shape`
/// values with identical content share the same morph, and a recreated-but-equal
/// endpoint reuses it. Lookup order: the interned construction ids
/// ([`Shape::id`]) are a zero-cost fast path when the same two `Shape` values
/// persist; otherwise the content hashes narrow to a bucket and a full content
/// comparison guards against hash collisions.
///
/// The cache is a bounded LRU (64 entries): each hit moves the entry to the
/// back, and a miss evicts the oldest entry once the cap is reached. Shapes
/// with non-finite coordinates are never cached.
///
/// The default instance is the thread-local [`super::MORPH_CACHE`]; it is not
/// thread-safe and does not need to be: properties are only evaluated on the UI
/// thread.
#[derive(Default)]
pub struct MorphCache {
    /// Most-recently-used last.
    morphs: RefCell<VecDeque<MorphCacheEntry>>,
    /// Number of lookups served from the cache (for benchmarks and tests).
    hits: core::cell::Cell<usize>,
}

struct MorphCacheEntry {
    /// `(a.id, b.id)` — the interned construction ids, compared first.
    ids: (u64, u64),
    /// `(a.content_hash(), b.content_hash())` — content-keyed bucket.
    hashes: (u64, u64),
    /// The endpoint payloads, kept for the full-content verify on a hash hit.
    a: Shape,
    b: Shape,
    morph: Rc<Morph>,
}

/// Maximum number of cached morphs (bounded LRU).
const MORPH_CACHE_CAP: usize = 64;

impl MorphCache {
    /// A new, empty cache.
    pub const fn new() -> Self {
        Self { morphs: RefCell::new(VecDeque::new()), hits: core::cell::Cell::new(0) }
    }

    /// The [Morph] between the two given shapes, computing and caching it on first
    /// use and reusing it while both endpoints' content is unchanged.
    ///
    /// Falls back to morphing degenerate empty polygons when an endpoint is invalid.
    /// Non-finite endpoint shapes are computed fresh each call and never cached.
    pub fn morph(&self, a: &Shape, b: &Shape) -> Rc<Morph> {
        let compute = || {
            Rc::new(Morph::new(
                a.polygon().unwrap_or_else(|_| empty_polygon()),
                b.polygon().unwrap_or_else(|_| empty_polygon()),
            ))
        };
        if !a.is_finite() || !b.is_finite() {
            return compute();
        }
        let ids = (a.id, b.id);
        let hashes = (a.content_hash(), b.content_hash());
        let mut morphs = self.morphs.borrow_mut();
        if let Some(entry) = morphs
            .iter()
            .position(|e| {
                // The id fast path requires *both* ids: an id-0 endpoint can be
                // built by hand (FFI POD, `Default`), so `e.ids == ids` alone
                // would alias any other morph to another id-0 shape.
                (ids.0 != 0 && ids.1 != 0 && e.ids == ids)
                    || (e.hashes == hashes && e.a == *a && e.b == *b)
            })
            .and_then(|index| morphs.remove(index))
        {
            let morph = Rc::clone(&entry.morph);
            morphs.push_back(entry);
            self.hits.set(self.hits.get() + 1);
            return morph;
        }
        let morph = compute();
        if morphs.len() >= MORPH_CACHE_CAP {
            morphs.pop_front();
        }
        morphs.push_back(MorphCacheEntry {
            ids,
            hashes,
            a: a.clone(),
            b: b.clone(),
            morph: Rc::clone(&morph),
        });
        morph
    }

    /// Drop all cached morphs.
    pub fn clear(&self) {
        self.morphs.borrow_mut().clear();
    }

    /// The number of cached morphs (testing).
    #[doc(hidden)]
    pub fn len(&self) -> usize {
        self.morphs.borrow().len()
    }

    /// Whether the cache is empty (testing).
    #[doc(hidden)]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The number of lookups that were served from the cache so far (testing).
    #[doc(hidden)]
    pub fn hits(&self) -> usize {
        self.hits.get()
    }
}

/// A degenerate single-point polygon used when an invalid [`Shape`] must be morphed
/// anyway (mirrors `RoundedPolygon`'s handling of empty input).
fn empty_polygon() -> RoundedPolygon {
    RoundedPolygon::degenerate(Point::ZERO)
}

/// Builds a [Shape] from a morph's interpolated cubics at `progress`.
/// When `endpoints` are given, the result's center is the linear interpolation of
/// the endpoints' centers; otherwise the centroid of the outline is used.
/// Features are re-detected on the interpolated outline (needed for chained morphs).
fn morph_to_shape(morph: &Morph, progress: f32, endpoints: Option<(&Shape, &Shape)>) -> Shape {
    let cubics = morph.as_cubics(progress);
    if cubics.is_empty() {
        return Shape::empty();
    }
    let center = endpoints.map(|(a, b)| Point {
        x: interpolate(a.center.x, b.center.x, progress),
        y: interpolate(a.center.y, b.center.y, progress),
    });
    let features = detect_features(&cubics);
    match RoundedPolygon::from_features(features, center) {
        Ok(p) => {
            let mut shape = Shape::from_polygon(&p);
            // A morph animates the outline, not the fill rule: the result keeps
            // the `from` shape's rule.
            if let Some((a, _)) = endpoints {
                shape.fill_rule = a.fill_rule;
            }
            shape
        }
        Err(_) => Shape::empty(),
    }
}

fn polygon_to_shape(polygon: &RoundedPolygon) -> Shape {
    let mut cubics_vec: Vec<f32> = Vec::new();
    let mut features_vec: Vec<ShapeFeature> = Vec::new();
    let mut cubic_index = 0u32;
    for feature in polygon.features() {
        let kind = match feature {
            Feature::Edge(..) => ShapeFeatureKind::Edge,
            Feature::Corner { convex: true, .. } => ShapeFeatureKind::ConvexCorner,
            Feature::Corner { convex: false, .. } => ShapeFeatureKind::ConcaveCorner,
        };
        for cubic in feature.cubics() {
            cubics_vec.extend_from_slice(&cubic.points);
        }
        features_vec.push(ShapeFeature {
            cubic_start: cubic_index,
            cubic_len: feature.cubics().len() as u32,
            kind,
        });
        cubic_index += feature.cubics().len() as u32;
    }
    Shape::new(
        SharedVector::from(cubics_vec.as_slice()),
        SharedVector::from(features_vec.as_slice()),
        ShapePoint { x: polygon.center().x, y: polygon.center().y },
    )
    .unwrap_or_else(|_| Shape::default())
}

/// Reconstruct a [RoundedPolygon] from a flattened [Shape].
pub(crate) fn data_to_polygon(data: &Shape) -> Result<RoundedPolygon, ShapeError> {
    let cubics: Vec<Cubic> =
        data.cubics.as_slice().as_chunks::<8>().0.iter().map(|p| Cubic { points: *p }).collect();
    let mut features: Vec<Feature> = Vec::with_capacity(data.features.len());
    for f in data.features.iter() {
        let range = f.cubic_start as usize..(f.cubic_start as usize + f.cubic_len as usize);
        if range.end > cubics.len() {
            return Err(ShapeError::new("Shape feature range out of bounds"));
        }
        let feature_cubics: Vec<Cubic> = cubics[range].to_vec();
        match f.kind {
            ShapeFeatureKind::Edge => features.push(Feature::Edge(feature_cubics)),
            ShapeFeatureKind::ConvexCorner => {
                features.push(Feature::Corner { cubics: feature_cubics, convex: true })
            }
            ShapeFeatureKind::ConcaveCorner => {
                features.push(Feature::Corner { cubics: feature_cubics, convex: false })
            }
        }
    }
    if features.is_empty() {
        // Degenerate payload (empty shape).
        return Err(ShapeError::new("Shape has no features"));
    }
    RoundedPolygon::from_features(features, Some(Point { x: data.center.x, y: data.center.y }))
}

/// FNV-1a over the payload (f32 bit patterns and feature kinds).
fn content_hash(data: &Shape) -> u64 {
    hash_parts(&data.cubics, &data.features, &data.center)
}

/// `content_hash` over the unassembled parts (used inside `Shape::new`, where the
/// parts are computed before the `Shape` exists).
fn hash_parts(
    cubics: &SharedVector<f32>,
    features: &SharedVector<ShapeFeature>,
    center: &ShapePoint,
) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    let mut mix = |b: u64| {
        h ^= b;
        h = h.wrapping_mul(0x100000001b3);
    };
    for v in cubics.iter() {
        mix(v.to_bits() as u64);
    }
    for f in features.iter() {
        mix(f.cubic_start as u64);
        mix(f.cubic_len as u64);
        mix(f.kind as u64);
    }
    mix(center.x.to_bits() as u64);
    mix(center.y.to_bits() as u64);
    h
}
