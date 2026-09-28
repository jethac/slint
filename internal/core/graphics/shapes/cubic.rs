// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore androidx muller

//! Port of `Cubic.kt` from androidx.graphics.shapes.

use super::utils::{
    DISTANCE_EPSILON, Point, PointTransformer, convex, direction_vector, distance, interpolate,
    k_max, k_min, k_sqrt,
};

/// Holds the anchor and control point data for a single cubic Bézier curve, with anchor
/// points (anchor0) and (anchor1) at either end and control points (control0) and
/// (control1) determining the slope of the curve between the anchor points.
///
/// The points are stored in a flat `[f32; 8]` array, matching the Kotlin layout:
/// `anchor0x, anchor0y, control0x, control0y, control1x, control1y, anchor1x, anchor1y`.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
#[repr(C)]
pub struct Cubic {
    /// The four points of the cubic in a flat array: anchor0, control0, control1, anchor1.
    pub points: [f32; 8],
}

impl Cubic {
    /// A cubic from the four points: anchor0, control0, control1, anchor1.
    pub const fn new(anchor0: Point, control0: Point, control1: Point, anchor1: Point) -> Self {
        Self {
            points: [
                anchor0.x, anchor0.y, control0.x, control0.y, control1.x, control1.y, anchor1.x,
                anchor1.y,
            ],
        }
    }

    /// A cubic from the eight raw coordinates.
    #[allow(clippy::too_many_arguments)]
    pub const fn from_floats(
        anchor0_x: f32,
        anchor0_y: f32,
        control0_x: f32,
        control0_y: f32,
        control1_x: f32,
        control1_y: f32,
        anchor1_x: f32,
        anchor1_y: f32,
    ) -> Self {
        Self {
            points: [
                anchor0_x, anchor0_y, control0_x, control0_y, control1_x, control1_y, anchor1_x,
                anchor1_y,
            ],
        }
    }

    /// The first anchor point x coordinate.
    pub fn anchor0_x(&self) -> f32 {
        self.points[0]
    }
    /// The first anchor point y coordinate.
    pub fn anchor0_y(&self) -> f32 {
        self.points[1]
    }
    /// The first control point x coordinate.
    pub fn control0_x(&self) -> f32 {
        self.points[2]
    }
    /// The first control point y coordinate.
    pub fn control0_y(&self) -> f32 {
        self.points[3]
    }
    /// The second control point x coordinate.
    pub fn control1_x(&self) -> f32 {
        self.points[4]
    }
    /// The second control point y coordinate.
    pub fn control1_y(&self) -> f32 {
        self.points[5]
    }
    /// The second anchor point x coordinate.
    pub fn anchor1_x(&self) -> f32 {
        self.points[6]
    }
    /// The second anchor point y coordinate.
    pub fn anchor1_y(&self) -> f32 {
        self.points[7]
    }

    /// The first anchor point.
    pub fn anchor0(&self) -> Point {
        Point { x: self.points[0], y: self.points[1] }
    }
    /// The first control point.
    pub fn control0(&self) -> Point {
        Point { x: self.points[2], y: self.points[3] }
    }
    /// The second control point.
    pub fn control1(&self) -> Point {
        Point { x: self.points[4], y: self.points[5] }
    }
    /// The second anchor point.
    pub fn anchor1(&self) -> Point {
        Point { x: self.points[6], y: self.points[7] }
    }

    /// A point on the curve for parameter t, representing the proportional distance
    /// along the curve between its starting point at anchor0 and ending point at
    /// anchor1, where 0 is at anchor0 and 1 is at anchor1.
    pub fn point_on_curve(&self, t: f32) -> Point {
        let u = 1. - t;
        Point {
            x: self.anchor0_x() * (u * u * u)
                + self.control0_x() * (3. * t * u * u)
                + self.control1_x() * (3. * t * t * u)
                + self.anchor1_x() * (t * t * t),
            y: self.anchor0_y() * (u * u * u)
                + self.control0_y() * (3. * t * u * u)
                + self.control1_y() * (3. * t * t * u)
                + self.anchor1_y() * (t * t * t),
        }
    }

    pub(crate) fn zero_length(&self) -> bool {
        (self.anchor0_x() - self.anchor1_x()).abs() < DISTANCE_EPSILON
            && (self.anchor0_y() - self.anchor1_y()).abs() < DISTANCE_EPSILON
    }

    pub(crate) fn convex_to(&self, next: &Cubic) -> bool {
        let prev_vertex = self.anchor0();
        let curr_vertex = self.anchor1();
        let next_vertex = next.anchor1();
        convex(prev_vertex, curr_vertex, next_vertex)
    }

    fn zero_ish(value: f32) -> bool {
        value.abs() < DISTANCE_EPSILON
    }

    /// The true bounds of this curve: the axis-aligned bounding box as
    /// `[left, top, right, bottom]`.
    ///
    /// When `approximate` is true, this uses the bounding box of all anchor and control
    /// points instead of the exact curve bounds.
    pub fn calculate_bounds(&self, approximate: bool) -> [f32; 4] {
        // A curve might be of zero-length, with both anchors co-located.
        // Just return the point itself.
        if self.zero_length() {
            return [self.anchor0_x(), self.anchor0_y(), self.anchor0_x(), self.anchor0_y()];
        }

        let mut min_x = k_min(self.anchor0_x(), self.anchor1_x());
        let mut min_y = k_min(self.anchor0_y(), self.anchor1_y());
        let mut max_x = k_max(self.anchor0_x(), self.anchor1_x());
        let mut max_y = k_max(self.anchor0_y(), self.anchor1_y());

        if approximate {
            // Approximate bounds use the bounding box of all anchors and controls
            return [
                k_min(min_x, k_min(self.control0_x(), self.control1_x())),
                k_min(min_y, k_min(self.control0_y(), self.control1_y())),
                k_max(max_x, k_max(self.control0_x(), self.control1_x())),
                k_max(max_y, k_max(self.control0_y(), self.control1_y())),
            ];
        }

        // Find the derivative, which is a quadratic Bezier. Then we can solve for t
        // using the quadratic formula
        let xa =
            -self.anchor0_x() + 3. * self.control0_x() - 3. * self.control1_x() + self.anchor1_x();
        let xb = 2. * self.anchor0_x() - 4. * self.control0_x() + 2. * self.control1_x();
        let xc = -self.anchor0_x() + self.control0_x();

        if Self::zero_ish(xa) {
            // Try Muller's method instead; it can find a single root when a is 0
            if xb != 0. {
                let t = 2. * xc / (-2. * xb);
                if (0. ..=1.).contains(&t) {
                    let x = self.point_on_curve(t).x;
                    if x < min_x {
                        min_x = x;
                    }
                    if x > max_x {
                        max_x = x;
                    }
                }
            }
        } else {
            let xs = xb * xb - 4. * xa * xc;
            if xs >= 0. {
                let t1 = (-xb + k_sqrt(xs)) / (2. * xa);
                if (0. ..=1.).contains(&t1) {
                    let x = self.point_on_curve(t1).x;
                    if x < min_x {
                        min_x = x;
                    }
                    if x > max_x {
                        max_x = x;
                    }
                }

                let t2 = (-xb - k_sqrt(xs)) / (2. * xa);
                if (0. ..=1.).contains(&t2) {
                    let x = self.point_on_curve(t2).x;
                    if x < min_x {
                        min_x = x;
                    }
                    if x > max_x {
                        max_x = x;
                    }
                }
            }
        }

        // Repeat the above for the y coordinate
        let ya =
            -self.anchor0_y() + 3. * self.control0_y() - 3. * self.control1_y() + self.anchor1_y();
        let yb = 2. * self.anchor0_y() - 4. * self.control0_y() + 2. * self.control1_y();
        let yc = -self.anchor0_y() + self.control0_y();

        if Self::zero_ish(ya) {
            if yb != 0. {
                let t = 2. * yc / (-2. * yb);
                if (0. ..=1.).contains(&t) {
                    let y = self.point_on_curve(t).y;
                    if y < min_y {
                        min_y = y;
                    }
                    if y > max_y {
                        max_y = y;
                    }
                }
            }
        } else {
            let ys = yb * yb - 4. * ya * yc;
            if ys >= 0. {
                let t1 = (-yb + k_sqrt(ys)) / (2. * ya);
                if (0. ..=1.).contains(&t1) {
                    let y = self.point_on_curve(t1).y;
                    if y < min_y {
                        min_y = y;
                    }
                    if y > max_y {
                        max_y = y;
                    }
                }

                let t2 = (-yb - k_sqrt(ys)) / (2. * ya);
                if (0. ..=1.).contains(&t2) {
                    let y = self.point_on_curve(t2).y;
                    if y < min_y {
                        min_y = y;
                    }
                    if y > max_y {
                        max_y = y;
                    }
                }
            }
        }
        [min_x, min_y, max_x, max_y]
    }

    /// Two cubics, created by splitting this curve at the given distance of `t` between
    /// the original starting and ending anchor points.
    pub fn split(&self, t: f32) -> (Cubic, Cubic) {
        let u = 1. - t;
        let point_on_curve = self.point_on_curve(t);
        (
            Cubic::from_floats(
                self.anchor0_x(),
                self.anchor0_y(),
                self.anchor0_x() * u + self.control0_x() * t,
                self.anchor0_y() * u + self.control0_y() * t,
                self.anchor0_x() * (u * u)
                    + self.control0_x() * (2. * u * t)
                    + self.control1_x() * (t * t),
                self.anchor0_y() * (u * u)
                    + self.control0_y() * (2. * u * t)
                    + self.control1_y() * (t * t),
                point_on_curve.x,
                point_on_curve.y,
            ),
            Cubic::from_floats(
                point_on_curve.x,
                point_on_curve.y,
                self.control0_x() * (u * u)
                    + self.control1_x() * (2. * u * t)
                    + self.anchor1_x() * (t * t),
                self.control0_y() * (u * u)
                    + self.control1_y() * (2. * u * t)
                    + self.anchor1_y() * (t * t),
                self.control1_x() * u + self.anchor1_x() * t,
                self.control1_y() * u + self.anchor1_y() * t,
                self.anchor1_x(),
                self.anchor1_y(),
            ),
        )
    }

    /// A new cubic with the control/anchor points of this curve in reverse order.
    pub fn reverse(&self) -> Cubic {
        Cubic::from_floats(
            self.anchor1_x(),
            self.anchor1_y(),
            self.control1_x(),
            self.control1_y(),
            self.control0_x(),
            self.control0_y(),
            self.anchor0_x(),
            self.anchor0_y(),
        )
    }

    /// Transform the points of this cubic with the given transformer, writing the result
    /// into `self` in place.
    pub fn transform_assign(&mut self, f: &dyn PointTransformer) {
        for ix in [0, 2, 4, 6] {
            let result = f.transform(self.points[ix], self.points[ix + 1]);
            self.points[ix] = result.x;
            self.points[ix + 1] = result.y;
        }
    }

    /// The cubic transformed by the given transformer.
    pub fn transformed(&self, f: &dyn PointTransformer) -> Cubic {
        let mut new_cubic = *self;
        new_cubic.transform_assign(f);
        new_cubic
    }

    /// Set the points of this cubic to `c1` interpolated towards `c2` by `progress`.
    /// This is the `MutableCubic.interpolate` pattern, used to avoid per-frame
    /// allocation during morphing.
    pub fn interpolate_assign(&mut self, c1: &Cubic, c2: &Cubic, progress: f32) {
        for it in 0..8 {
            self.points[it] = interpolate(c1.points[it], c2.points[it], progress);
        }
    }

    /// A bézier curve that is a straight line between the given anchor points. The
    /// control points lie 1/3 of the distance from their respective anchor points.
    pub fn straight_line(x0: f32, y0: f32, x1: f32, y1: f32) -> Cubic {
        Cubic::from_floats(
            x0,
            y0,
            interpolate(x0, x1, 1. / 3.),
            interpolate(y0, y1, 1. / 3.),
            interpolate(x0, x1, 2. / 3.),
            interpolate(y0, y1, 2. / 3.),
            x1,
            y1,
        )
    }

    /// A bézier curve that approximates a circular arc, with p0 and p1 as the starting
    /// and ending anchor points. The curve generated is the smallest of the two possible
    /// arcs around the entire 360-degree circle. Arcs of greater than 180 degrees should
    /// use more than one arc together. Note that p0 and p1 should be equidistant from the
    /// center.
    ///
    /// Returns `None` when p0 or p1 coincides with the center (the Kotlin
    /// `require`s in `directionVector`, surfaced by the constructors as `ShapeError`).
    pub fn circular_arc(
        center_x: f32,
        center_y: f32,
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
    ) -> Option<Cubic> {
        let p0d = direction_vector(x0 - center_x, y0 - center_y)?;
        let p1d = direction_vector(x1 - center_x, y1 - center_y)?;
        let rotated_p0 = p0d.rotate90();
        let rotated_p1 = p1d.rotate90();
        let clockwise = rotated_p0.dot_product_xy(x1 - center_x, y1 - center_y) >= 0.;
        let cosa = p0d.dot_product(p1d);
        if cosa > 0.999 {
            // p0 ~= p1
            return Some(Self::straight_line(x0, y0, x1, y1));
        }
        let k = distance(x0 - center_x, y0 - center_y) * 4. / 3.
            * (k_sqrt(2. * (1. - cosa)) - k_sqrt(1. - cosa * cosa))
            / (1. - cosa)
            * if clockwise { 1. } else { -1. };
        Some(Cubic::from_floats(
            x0,
            y0,
            x0 + rotated_p0.x * k,
            y0 + rotated_p0.y * k,
            x1 - rotated_p1.x * k,
            y1 - rotated_p1.y * k,
            x1,
            y1,
        ))
    }

    /// An empty cubic defined at (x0, y0).
    pub(crate) fn empty(x0: f32, y0: f32) -> Cubic {
        Cubic::from_floats(x0, y0, x0, y0, x0, y0, x0, y0)
    }
}

impl core::ops::Add for Cubic {
    type Output = Cubic;
    fn add(self, o: Cubic) -> Cubic {
        let mut points = [0.; 8];
        for (p, (&a, &b)) in points.iter_mut().zip(self.points.iter().zip(o.points.iter())) {
            *p = a + b;
        }
        Cubic { points }
    }
}

impl core::ops::Mul<f32> for Cubic {
    type Output = Cubic;
    fn mul(self, x: f32) -> Cubic {
        let mut points = self.points;
        for p in points.iter_mut() {
            *p *= x;
        }
        Cubic { points }
    }
}

impl core::ops::Div<f32> for Cubic {
    type Output = Cubic;
    fn div(self, x: f32) -> Cubic {
        self * (1. / x)
    }
}

impl core::fmt::Display for Cubic {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "anchor0: ({}, {}) control0: ({}, {}), control1: ({}, {}), anchor1: ({}, {})",
            self.anchor0_x(),
            self.anchor0_y(),
            self.control0_x(),
            self.control0_y(),
            self.control1_x(),
            self.control1_y(),
            self.anchor1_x(),
            self.anchor1_y()
        )
    }
}
