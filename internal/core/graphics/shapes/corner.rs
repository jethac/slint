// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore androidx

//! Port of `CornerRounding.kt` and the private `RoundedCorner` class from
//! `RoundedPolygon.kt` in androidx.graphics.shapes.

use super::cubic::Cubic;
use super::utils::{
    DISTANCE_EPSILON, Point, direction_vector, interpolate_point, k_min, k_sqrt, square,
};
use alloc::vec::Vec;

/// The rounding parameters of a shape vertex, `#[repr(C)]` and visible to `.slint`
/// code as the `CornerRounding` builtin struct (generated in `crate::items` from
/// `i_slint_common::builtin_structs`, where its documentation lives). A value of 0
/// for `radius` means a sharp corner; `smoothing` in 0 to 1 extends the curve from
/// the circular arc to the adjacent edges.
pub use crate::items::CornerRounding;

impl CornerRounding {
    /// A rounding radius of zero, producing a sharp corner at a vertex.
    pub const UNROUNDED: CornerRounding = CornerRounding { radius: 0., smoothing: 0. };

    /// A corner rounding from a circular-arc `radius` and `smoothing` factor in
    /// the 0–1 range (see the struct's fields).
    ///
    /// This constructor exists because the struct is `#[non_exhaustive]` and so
    /// cannot be built with a literal outside i-slint-core.
    pub const fn new(radius: f32, smoothing: f32) -> Self {
        Self { radius, smoothing }
    }
}

/// Utility type that holds the information about each corner in a polygon. The shape
/// of the corner is produced by [`RoundedCorner::cubics`], which returns a list of
/// curves representing the corner geometry, and depends on the `rounding` constructor
/// parameter.
///
/// If rounding is absent, there is no rounding; the corner is simply a single point at
/// p1, represented by a [Cubic] of length 0 at that point.
///
/// If rounding is present, the corner is rounded either with a curve approximating a
/// circular arc of the radius specified in `rounding`, or with three curves if
/// `rounding` has a nonzero smoothing parameter. These three curves are a circular arc
/// in the middle and two symmetrical flanking curves on either side. The smoothing
/// parameter determines the curvature of the flanking curves.
///
/// This is a class because the work happens in 2 steps: first determine how much we
/// want to cut to comply with the parameters, then we are given how much we can
/// actually cut (because of space restrictions outside this corner).
///
/// * `p0` the vertex before the one being rounded
/// * `p1` the vertex of this rounded corner
/// * `p2` the vertex after the one being rounded
/// * `rounding` the optional parameters specifying how this corner should be rounded
pub(crate) struct RoundedCorner {
    p0: Point,
    p1: Point,
    p2: Point,
    d1: Point,
    d2: Point,
    corner_radius: f32,
    smoothing: f32,
    #[allow(dead_code)]
    cos_angle: f32,
    #[allow(dead_code)]
    sin_angle: f32,
    expected_round_cut: f32,
    /// The center of the circle approximated by the rounding curve (or the middle of
    /// the three curves if smoothing is requested). The center is the same as p0 if
    /// there is no rounding.
    center: Point,
}

impl RoundedCorner {
    pub fn new(p0: Point, p1: Point, p2: Point, rounding: Option<CornerRounding>) -> Self {
        let v01 = p0 - p1;
        let v21 = p2 - p1;
        let d01 = v01.distance();
        let d21 = v21.distance();
        let (d1, d2, corner_radius, smoothing, cos_angle, sin_angle, expected_round_cut);
        if d01 > 0. && d21 > 0. {
            d1 = v01 / d01;
            d2 = v21 / d21;
            corner_radius = rounding.map(|r| r.radius).unwrap_or(0.);
            smoothing = rounding.map(|r| r.smoothing).unwrap_or(0.);

            // cosine of angle at p1 is dot product of unit vectors to the other two
            // vertices
            cos_angle = d1.dot_product(d2);

            // identity: sin^2 + cos^2 = 1
            // sin_angle gives us the intersection
            sin_angle = k_sqrt(1. - square(cos_angle));
            // How much we need to cut, as measured on a side, to get the required
            // radius calculating where the rounding circle hits the edge. This uses the
            // identity of tan(A/2) = sinA/(1 + cosA), where tan(A/2) = radius/cut
            expected_round_cut =
                if sin_angle > 1e-3 { corner_radius * (cos_angle + 1.) / sin_angle } else { 0. };
        } else {
            // One (or both) of the sides is empty, not much we can do.
            d1 = Point::ZERO;
            d2 = Point::ZERO;
            corner_radius = 0.;
            smoothing = 0.;
            cos_angle = 0.;
            sin_angle = 0.;
            expected_round_cut = 0.;
        }
        Self {
            p0,
            p1,
            p2,
            d1,
            d2,
            corner_radius,
            smoothing,
            cos_angle,
            sin_angle,
            expected_round_cut,
            center: Point::ZERO,
        }
    }

    /// The expected round cut for this corner.
    pub fn expected_round_cut(&self) -> f32 {
        self.expected_round_cut
    }

    /// smoothing changes the actual cut: 0 is same as expected_round_cut, 1 doubles it.
    pub fn expected_cut(&self) -> f32 {
        (1. + self.smoothing) * self.expected_round_cut
    }

    /// The list of [Cubic] curves for this corner. `allowed_cut0` is the amount we are
    /// able to cut on the side from the previous corner to this one, `allowed_cut1` on
    /// the side from this corner to the next one.
    pub fn cubics(&mut self, allowed_cut0: f32, allowed_cut1: f32) -> Vec<Cubic> {
        // We use the minimum of both cuts to determine the radius, but if there is more
        // space in one side we can use it for smoothing.
        let allowed_cut = k_min(allowed_cut0, allowed_cut1);
        // Nothing to do, just use lines, or a point
        if self.expected_round_cut < DISTANCE_EPSILON
            || allowed_cut < DISTANCE_EPSILON
            || self.corner_radius < DISTANCE_EPSILON
        {
            self.center = self.p1;
            return alloc::vec![Cubic::straight_line(self.p1.x, self.p1.y, self.p1.x, self.p1.y)];
        }
        // How much of the cut is required for the rounding part.
        let actual_round_cut = k_min(allowed_cut, self.expected_round_cut);
        // We have two smoothing values, one for each side of the vertex
        // Space is used for rounding values first. If there is space left over, then we
        // apply smoothing, if it was requested
        let actual_smoothing0 = self.actual_smoothing_value(allowed_cut0);
        let actual_smoothing1 = self.actual_smoothing_value(allowed_cut1);
        // Scale the radius if needed
        let actual_r = self.corner_radius * actual_round_cut / self.expected_round_cut;
        // Distance from the corner (p1) to the center
        let center_distance = k_sqrt(square(actual_r) + square(actual_round_cut));
        // Center of the arc we will use for rounding
        self.center = self.p1 + ((self.d1 + self.d2) / 2.).direction() * center_distance;
        let circle_intersection0 = self.p1 + self.d1 * actual_round_cut;
        let circle_intersection2 = self.p1 + self.d2 * actual_round_cut;
        let flanking0 = Self::compute_flanking_curve(
            actual_round_cut,
            actual_smoothing0,
            self.p1,
            self.p0,
            circle_intersection0,
            circle_intersection2,
            self.center,
            actual_r,
        );
        let flanking2 = Self::compute_flanking_curve(
            actual_round_cut,
            actual_smoothing1,
            self.p1,
            self.p2,
            circle_intersection2,
            circle_intersection0,
            self.center,
            actual_r,
        )
        .reverse();
        alloc::vec![
            flanking0,
            Cubic::circular_arc(
                self.center.x,
                self.center.y,
                flanking0.anchor1_x(),
                flanking0.anchor1_y(),
                flanking2.anchor0_x(),
                flanking2.anchor0_y(),
            ),
            flanking2,
        ]
    }

    /// If `allowed_cut` (the amount we are able to cut) is greater than the expected cut
    /// (without smoothing applied yet), then there is room to apply smoothing and we
    /// calculate the actual smoothing value here.
    fn actual_smoothing_value(&self, allowed_cut: f32) -> f32 {
        if allowed_cut > self.expected_cut() {
            self.smoothing
        } else if allowed_cut > self.expected_round_cut {
            self.smoothing * (allowed_cut - self.expected_round_cut)
                / (self.expected_cut() - self.expected_round_cut)
        } else {
            0.
        }
    }

    /// Compute a Bézier to connect the linear segment defined by corner and side_start
    /// with the circular segment defined by circle_center, circle_segment_intersection,
    /// other_circle_segment_intersection and actual_r. The Bézier will start at the
    /// linear segment and end on the circular segment.
    ///
    /// * `actual_round_cut` How much we are cutting of the corner to add the circular
    ///   segment (this is before smoothing, that will cut some more).
    /// * `actual_smoothing` How much we want to smooth (this is the smooth parameter,
    ///   adjusted down if there is not enough room).
    /// * `corner` The point at which the linear side ends
    /// * `side_start` The point at which the linear side starts
    /// * `circle_segment_intersection` The point at which the linear side and the
    ///   circle intersect.
    /// * `other_circle_segment_intersection` The point at which the opposing linear
    ///   side and the circle intersect.
    /// * `circle_center` The center of the circle.
    /// * `actual_r` The radius of the circle.
    #[allow(clippy::too_many_arguments)]
    fn compute_flanking_curve(
        actual_round_cut: f32,
        actual_smoothing: f32,
        corner: Point,
        side_start: Point,
        circle_segment_intersection: Point,
        other_circle_segment_intersection: Point,
        circle_center: Point,
        actual_r: f32,
    ) -> Cubic {
        // side_start is the anchor, 'anchor' is actual control point
        let side_direction = (side_start - corner).direction();
        let curve_start = corner + side_direction * actual_round_cut * (1. + actual_smoothing);
        // We use an approximation to cut a part of the circle section proportional to
        // 1 - smooth. When smooth = 0, we take the full section, when smooth = 1, we
        // take nothing.
        // TODO: revisit this, it can be problematic as it approaches 180 degrees
        let p = interpolate_point(
            circle_segment_intersection,
            (circle_segment_intersection + other_circle_segment_intersection) / 2.,
            actual_smoothing,
        );
        // The flanking curve ends on the circle
        let curve_end = circle_center
            + direction_vector(p.x - circle_center.x, p.y - circle_center.y) * actual_r;
        // The anchor on the circle segment side is in the intersection between the
        // tangent to the circle in the circle/flanking curve boundary and the linear
        // segment.
        let circle_tangent = (curve_end - circle_center).rotate90();
        let anchor_end =
            Self::line_intersection(side_start, side_direction, curve_end, circle_tangent)
                .unwrap_or(circle_segment_intersection);
        // From what remains, we pick a point for the start anchor.
        // 2/3 seems to come from design tools?
        let anchor_start = (curve_start + anchor_end * 2.) / 3.;
        Cubic::new(curve_start, anchor_start, anchor_end, curve_end)
    }

    /// The intersection point of the two lines d0->d1 and p0->p1, or None if the lines
    /// do not intersect.
    fn line_intersection(p0: Point, d0: Point, p1: Point, d1: Point) -> Option<Point> {
        let rotated_d1 = d1.rotate90();
        let den = d0.dot_product(rotated_d1);
        if den.abs() < DISTANCE_EPSILON {
            return None;
        }
        let num = (p1 - p0).dot_product(rotated_d1);
        // Also check the relative value. This is equivalent to
        // abs(den/num) < DistanceEpsilon, but avoid doing a division
        if den.abs() < DISTANCE_EPSILON * num.abs() {
            return None;
        }
        let k = num / den;
        Some(p0 + d0 * k)
    }
}
