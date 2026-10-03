// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore androidx

//! Port of `Morph.kt` from androidx.graphics.shapes.

use super::cubic::Cubic;
use super::mapping::feature_mapper;
use super::measure::{AngleMeasurer, LengthMeasurer, MeasuredPolygon};
use super::rounded_polygon::RoundedPolygon;
use super::utils::{ANGLE_EPSILON, interpolate, k_max, k_min, positive_modulo};
use alloc::rc::Rc;
use alloc::vec::Vec;

/// This class is used to animate between [start] and [end] polygon objects.
///
/// Morphing between arbitrary objects can be problematic because it can be difficult
/// to determine how the points of a given shape map to the points of some other shape.
/// [Morph] simplifies the problem by only operating on [RoundedPolygon] objects, which
/// are known to have similar, contiguous structures. For one thing, the shape of a
/// polygon is contiguous from start to end (compared to an arbitrary Path object,
/// which could have one or more `moveTo` operations in the shape). Also, all edges of
/// a polygon shape are represented by [Cubic] objects, thus the start and end shapes
/// use similar operations. Two polygon shapes then only differ in the quantity and
/// placement of their curves. The morph works by determining how to map the curves of
/// the two shapes together (based on proximity and other information, such as distance
/// to polygon vertices and concavity), and splitting curves when the shapes do not
/// have the same number of curves or when the curve placement within the shapes is
/// very different.
#[derive(Clone)]
pub struct Morph {
    start: RoundedPolygon,
    end: RoundedPolygon,
    /// The structure which holds the actual shape being morphed. It contains all
    /// cubics necessary to represent the start and end shapes (the original cubics in
    /// the shapes may be cut to align the start/end shapes), matched one to one in
    /// each pair.
    morph_match: Vec<(Cubic, Cubic)>,
    /// The measured perimeter of `start`, used by [Morph::scalar_delta] to
    /// normalize displacement into morph-progress units.
    start_perimeter: f32,
    /// Whether the endpoints couldn't be measured (zero-sized or non-finite
    /// outlines, where the Kotlin `require`s throw): the morph degenerates to
    /// the target outline at every progress value.
    degenerate: bool,
}

impl Morph {
    /// Creates the structure used to animate between the start and end shapes.
    /// The technique is to match geometry (curves) between the shapes when and where
    /// possible, and to create new/placeholder curves when necessary (when one of the
    /// shapes has more curves than the other).
    /// When either polygon's outline can't be measured (a zero-sized or non-finite
    /// shape, where the Kotlin `require`s throw), the morph degenerates to the
    /// target shape: every progress value produces `end`'s outline.
    pub fn new(start: RoundedPolygon, end: RoundedPolygon) -> Self {
        let (morph_match, start_perimeter, degenerate) = match match_shapes(&start, &end) {
            Some((m, p)) => (m, p, false),
            None => {
                crate::debug_log!(
                    "Shapes: morphing a shape whose outline can't be measured; \
                     the morph stays at the target"
                );
                (end.cubics().iter().map(|c| (*c, *c)).collect(), 0., true)
            }
        };
        Self { start, end, morph_match, start_perimeter, degenerate }
    }

    /// Whether this morph degenerated to the target outline because an endpoint
    /// couldn't be measured (see [Morph::new]).
    pub fn is_degenerate(&self) -> bool {
        self.degenerate
    }

    /// The matched cubic pairs: the first of each pair holds the geometry of the
    /// start shape, the second holds the geometry for the end shape.
    pub fn morph_match(&self) -> &[(Cubic, Cubic)] {
        &self.morph_match
    }

    /// The axis-aligned bounds of the object `[left, top, right, bottom]`, unioning
    /// the start and end shapes' bounds.
    pub fn calculate_bounds(&self, approximate: bool) -> [f32; 4] {
        let b_start = self.start.calculate_bounds(approximate);
        let b_end = self.end.calculate_bounds(approximate);
        [
            k_min(b_start[0], b_end[0]),
            k_min(b_start[1], b_end[1]),
            k_max(b_start[2], b_end[2]),
            k_max(b_start[3], b_end[3]),
        ]
    }

    /// The axis-aligned "max bounds" (square) of the morph, unioning the start and
    /// end shapes' max bounds. See [RoundedPolygon::calculate_max_bounds].
    pub fn calculate_max_bounds(&self) -> [f32; 4] {
        let b_start = self.start.calculate_max_bounds();
        let b_end = self.end.calculate_max_bounds();
        [
            k_min(b_start[0], b_end[0]),
            k_min(b_start[1], b_end[1]),
            k_max(b_start[2], b_end[2]),
            k_max(b_start[3], b_end[3]),
        ]
    }

    /// A representation of the morph object at a given `progress` value as a list of
    /// Cubics.
    ///
    /// `progress` is a value from 0 to 1 that determines the morph's current shape,
    /// between the start and end shapes provided at construction time. A value of 0
    /// results in the start shape, a value of 1 results in the end shape, and any
    /// value in between results in a shape which is a linear interpolation between
    /// those two shapes. The range is generally [0, 1] and values outside could
    /// result in undefined shapes, but values close to (but outside) the range can be
    /// used to get an exaggerated effect (e.g., for a bounce or overshoot animation).
    pub fn as_cubics(&self, progress: f32) -> Vec<Cubic> {
        let mut result = Vec::with_capacity(self.morph_match.len());
        // The first/last mechanism here ensures that the final anchor point in the
        // shape exactly matches the first anchor point. There can be rendering
        // artifacts introduced by those points being slightly off, even by much less
        // than a pixel
        let mut first_cubic: Option<Cubic> = None;
        let mut last_cubic: Option<Cubic> = None;
        for i in 0..self.morph_match.len() {
            let mut points = [0f32; 8];
            for (p, (&a, &b)) in points
                .iter_mut()
                .zip(self.morph_match[i].0.points.iter().zip(self.morph_match[i].1.points.iter()))
            {
                *p = interpolate(a, b, progress);
            }
            let cubic = Cubic { points };
            if first_cubic.is_none() {
                first_cubic = Some(cubic);
            }
            if let Some(last) = last_cubic {
                result.push(last);
            }
            last_cubic = Some(cubic);
        }
        if let (Some(last), Some(first)) = (last_cubic, first_cubic) {
            result.push(Cubic::from_floats(
                last.anchor0_x(),
                last.anchor0_y(),
                last.control0_x(),
                last.control0_y(),
                last.control1_x(),
                last.control1_y(),
                first.anchor0_x(),
                first.anchor0_y(),
            ));
        }
        result
    }

    /// Like [Morph::as_cubics], but writes each interpolated cubic into `target`,
    /// avoiding a per-frame allocation. `target.len()` is set to the number of
    /// cubics emitted; it must have capacity for `self.morph_match().len()` cubics
    /// (it is grown if necessary).
    pub fn for_each_cubic(&self, progress: f32, target: &mut Vec<Cubic>) {
        target.clear();
        target.reserve(self.morph_match.len());
        for i in 0..self.morph_match.len() {
            let mut cubic = Cubic::default();
            cubic.interpolate_assign(&self.morph_match[i].0, &self.morph_match[i].1, progress);
            target.push(cubic);
        }
    }

    /// The scalar morph distance between the start and end shapes: the mean
    /// Euclidean displacement of the matched cubic anchors, normalized by the
    /// start shape's measured perimeter.
    ///
    /// This is the `D(a, b)` of the design note's retarget math: spring velocity
    /// carried over a mid-morph retarget is divided by this distance to become a
    /// morph-progress speed. A value of 0 means the morph does not move.
    pub fn scalar_delta(&self) -> f32 {
        if self.morph_match.is_empty() || self.start_perimeter == 0. {
            return 0.;
        }
        let mut total = 0f32;
        for (a, b) in &self.morph_match {
            for i in 0..4 {
                let dx = a.points[2 * i] - b.points[2 * i];
                let dy = a.points[2 * i + 1] - b.points[2 * i + 1];
                total += super::utils::k_sqrt(dx * dx + dy * dy);
            }
        }
        (total / (self.morph_match.len() * 4) as f32) / self.start_perimeter
    }
}

/// `match`, called at Morph construction time, creates the structure used to animate
/// between the start and end shapes.
///
/// Curves on both shapes are matched by running the [Measurer](super::measure::Measurer)
/// to determine where the points are in each shape (proportionally, along the
/// outline), and then running [feature_mapper] which decides how to map (match) all
/// of the curves with each other.
fn match_shapes(p1: &RoundedPolygon, p2: &RoundedPolygon) -> Option<(Vec<(Cubic, Cubic)>, f32)> {
    // Measure polygons, returns lists of measured cubics for each polygon, which
    // we then use to match start/end curves
    let measured_polygon1 = MeasuredPolygon::measure_polygon(
        Rc::new(AngleMeasurer::new(p1.center_x(), p1.center_y()))
            as Rc<dyn super::measure::Measurer>,
        p1,
    )?;
    let measured_polygon2 = MeasuredPolygon::measure_polygon(
        Rc::new(AngleMeasurer::new(p2.center_x(), p2.center_y()))
            as Rc<dyn super::measure::Measurer>,
        p2,
    )?;

    // features1 and 2 will contain the list of corners (just the inner circular
    // curve) along with the progress at the middle of those corners. These
    // measurement values are then used to compare and match between the two polygons
    let features1 = &measured_polygon1.features;
    let features2 = &measured_polygon2.features;

    // Map features: doubleMapper is the result of mapping the features in each
    // shape to the closest feature in the other shape.
    // Given a progress in one of the shapes it can be used to find the corresponding
    // progress in the other shape (in both directions)
    let double_mapper = feature_mapper(features1, features2)?;

    // cut point on poly2 is the mapping of the 0 point on poly1
    let polygon2_cut_point = double_mapper.map(0.)?;

    // Cut and rotate.
    // Polygons start at progress 0, and the featureMapper has decided that we want
    // to match progress 0 in the first polygon to `polygon2CutPoint` on the second
    // polygon. So we need to cut the second polygon there and "rotate it", so as
    // we walk through both polygons we can find the matching.
    // The resulting bs1/2 are MeasuredPolygons, whose MeasuredCubics start from
    // outlineProgress=0 and increasing until outlineProgress=1
    // The measured perimeter of p1, used by Morph::scalar_delta for retarget
    // velocity normalization: a physical length, so it is measured by
    // arc length rather than by the angle measurer above.
    let start_perimeter: f32 = MeasuredPolygon::measure_polygon(
        Rc::new(LengthMeasurer::default()) as Rc<dyn super::measure::Measurer>,
        p1,
    )
    .map(|mp| (0..mp.size()).filter_map(|i| mp.get(i)).map(|c| c.measured_size).sum())
    .unwrap_or(0.);

    let bs1 = &measured_polygon1;
    let bs2 = measured_polygon2.cut_and_shift(polygon2_cut_point)?;

    // Match
    // Now we can compare the two lists of measured cubics and create a list of
    // pairs of cubics [ret], which are the start/end curves that represent the
    // Morph object and the start and end shapes, and which can be interpolated to
    // animate the between those shapes.
    let mut ret: Vec<(Cubic, Cubic)> = Vec::new();
    // i1/i2 are the indices of the current cubic on the start (1) and end (2) shapes
    let mut i1 = 0usize;
    let mut i2 = 0usize;
    // b1, b2 are the current measured cubic for each polygon
    let mut b1 = bs1.get(i1);
    i1 += 1;
    let mut b2 = bs2.get(i2);
    i2 += 1;
    // Iterate until all curves are accounted for and matched
    while let (Some(cur_b1), Some(cur_b2)) = (b1.clone(), b2.clone()) {
        // Progresses are in shape1's perspective
        // b1a, b2a are ending progress values of current measured cubics in [0,1]
        // range
        let b1a = if i1 == bs1.size() { 1. } else { cur_b1.end_outline_progress };
        let b2a = if i2 == bs2.size() {
            1.
        } else {
            double_mapper
                .map_back(positive_modulo(cur_b2.end_outline_progress + polygon2_cut_point, 1.))?
        };
        let minb = k_min(b1a, b2a);

        // min b is the progress at which the curve that ends first ends.
        // If both curves ends roughly there, no cutting is needed, we have a match.
        // If one curve extends beyond, we need to cut it.
        let (seg1, new_b1) = if b1a > minb + ANGLE_EPSILON {
            let (s, rest) = cur_b1.cut_at_progress(bs1.measurer().as_ref(), minb)?;
            (s, Some(rest))
        } else {
            let nb = bs1.get(i1);
            i1 += 1;
            (cur_b1.clone(), nb)
        };
        let (seg2, new_b2) = if b2a > minb + ANGLE_EPSILON {
            let cut = positive_modulo(double_mapper.map(minb)? - polygon2_cut_point, 1.);
            let (s, rest) = cur_b2.cut_at_progress(bs2.measurer().as_ref(), cut)?;
            (s, Some(rest))
        } else {
            let nb = bs2.get(i2);
            i2 += 1;
            (cur_b2.clone(), nb)
        };
        ret.push((seg1.cubic, seg2.cubic));
        b1 = new_b1;
        b2 = new_b2;
    }
    if b1.is_some() || b2.is_some() {
        crate::debug_log!("Shapes: expected both polygons' cubics to be fully matched");
        return None;
    }
    Some((ret, start_perimeter))
}
