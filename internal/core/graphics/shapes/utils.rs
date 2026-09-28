// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore androidx

//! Port of `Utils.kt` and `Point.kt` from androidx.graphics.shapes.
//!
//! `kotlin.math` functions on `Float` arguments widen to `Double`, run the JVM math
//! function and narrow the result to `Float` again. The helpers below reproduce that
//! computation chain exactly: they evaluate in `f64` and cast back to `f32`, which keeps
//! the ported algorithms bit-compatible with the Kotlin implementation (up to one-ulp
//! differences of the underlying `f64` math routines between libm implementations).
//! Plain arithmetic stays in `f32`, in the same order of operations as the Kotlin code.

#[cfg(not(feature = "std"))]
use num_traits::Float;

/// A point in a shape's own (unitless) coordinate space.
/// Port of the internal `Point` typealias (`androidx.collection.FloatFloatPair`).
#[derive(Copy, Clone, Debug, Default, PartialEq)]
#[repr(C)]
pub struct Point {
    /// The x coordinate.
    pub x: f32,
    /// The y coordinate.
    pub y: f32,
}

impl Point {
    /// A point at (0, 0).
    pub const ZERO: Point = Point { x: 0., y: 0. };

    /// The magnitude of the Point, which is the distance of this point from (0, 0).
    pub fn distance(self) -> f32 {
        k_sqrt(self.x * self.x + self.y * self.y)
    }

    /// The square of the magnitude of the Point.
    pub fn distance_squared(self) -> f32 {
        self.x * self.x + self.y * self.y
    }

    pub(crate) fn dot_product(self, other: Point) -> f32 {
        self.x * other.x + self.y * other.y
    }

    pub(crate) fn dot_product_xy(self, other_x: f32, other_y: f32) -> f32 {
        self.x * other_x + self.y * other_y
    }

    /// The Z coordinate of the cross product of two vectors, true if the second vector
    /// is going clockwise (> 0) compared with the first one.
    pub(crate) fn clockwise(self, other: Point) -> bool {
        self.x * other.y - self.y * other.x > 0.
    }

    /// Unit vector representing the direction to this point from (0, 0), or `None`
    /// on a zero-length vector (the Kotlin `require` in `Point.getDirection`, which
    /// the shape constructors surface as `ShapeError`).
    pub(crate) fn direction(self) -> Option<Point> {
        let d = self.distance();
        if d > 0. { Some(self / d) } else { None }
    }

    /// The point rotated by 90 degrees counter-clockwise.
    pub(crate) fn rotate90(self) -> Point {
        Point { x: -self.y, y: self.x }
    }

    /// Transform this point with the given transformer.
    pub fn transformed(&self, f: &dyn crate::graphics::shapes::PointTransformer) -> Point {
        f.transform(self.x, self.y)
    }
}

impl core::ops::Neg for Point {
    type Output = Point;
    fn neg(self) -> Point {
        Point { x: -self.x, y: -self.y }
    }
}

impl core::ops::Sub for Point {
    type Output = Point;
    fn sub(self, other: Point) -> Point {
        Point { x: self.x - other.x, y: self.y - other.y }
    }
}

impl core::ops::Add for Point {
    type Output = Point;
    fn add(self, other: Point) -> Point {
        Point { x: self.x + other.x, y: self.y + other.y }
    }
}

impl core::ops::Mul<f32> for Point {
    type Output = Point;
    fn mul(self, operand: f32) -> Point {
        Point { x: self.x * operand, y: self.y * operand }
    }
}

impl core::ops::Div<f32> for Point {
    type Output = Point;
    fn div(self, operand: f32) -> Point {
        Point { x: self.x / operand, y: self.y / operand }
    }
}

/// A function that can transform (rotate/scale/translate/etc.) points.
/// Port of the `PointTransformer` fun interface.
pub trait PointTransformer {
    /// Transform the point given the x and y parameters, returning the transformed point.
    fn transform(&self, x: f32, y: f32) -> Point;
}

impl<F: Fn(f32, f32) -> Point> PointTransformer for F {
    fn transform(&self, x: f32, y: f32) -> Point {
        self(x, y)
    }
}

pub(crate) fn distance(x: f32, y: f32) -> f32 {
    k_sqrt(x * x + y * y)
}

pub(crate) fn distance_squared(x: f32, y: f32) -> f32 {
    x * x + y * y
}

/// Unit vector representing the direction to the point (x, y) from (0, 0), or
/// `None` on a zero-length vector (the Kotlin `require` in `directionVector`).
pub(crate) fn direction_vector(x: f32, y: f32) -> Option<Point> {
    let d = distance(x, y);
    if d > 0. { Some(Point { x: x / d, y: y / d }) } else { None }
}

/// Unit vector at the given angle.
pub(crate) fn direction_vector_angle(angle_radians: f32) -> Point {
    Point { x: k_cos(angle_radians), y: k_sin(angle_radians) }
}

/// Kotlin `radialToCartesian(radius, angleRadians, center = Origin)`.
pub(crate) fn radial_to_cartesian(radius: f32, angle_radians: f32) -> Point {
    direction_vector_angle(angle_radians) * radius
}

/// These epsilon values are used internally to determine when two points are the same,
/// within some reasonable roundoff error. The distance epsilon is smaller, with the
/// intention that the roundoff should not be larger than a pixel on any reasonable
/// sized display.
pub(crate) const DISTANCE_EPSILON: f32 = 1e-4;
pub(crate) const ANGLE_EPSILON: f32 = 1e-6;

/// This epsilon is based on the observation that people tend to see e.g. collinearity
/// much more relaxed than what is mathematically correct. This effect is heightened on
/// smaller displays. Use this epsilon for operations that allow higher tolerances.
pub(crate) const RELAXED_DISTANCE_EPSILON: f32 = 5e-3;

pub(crate) const FLOAT_PI: f32 = core::f32::consts::PI;
pub(crate) const TWO_PI: f32 = 2. * FLOAT_PI;

pub(crate) fn square(x: f32) -> f32 {
    x * x
}

/// Linearly interpolate between `start` and `stop` with `fraction` fraction between them.
pub(crate) fn interpolate(start: f32, stop: f32, fraction: f32) -> f32 {
    (1. - fraction) * start + fraction * stop
}

/// Linearly interpolate between two points.
pub(crate) fn interpolate_point(start: Point, stop: Point, fraction: f32) -> Point {
    Point { x: interpolate(start.x, stop.x, fraction), y: interpolate(start.y, stop.y, fraction) }
}

/// Similar to `num % mod`, but ensures the result is always positive.
pub(crate) fn positive_modulo(num: f32, modulus: f32) -> f32 {
    (num % modulus + modulus) % modulus
}

/// Returns whether C is on the line defined by the two points AB.
pub(crate) fn collinear_ish(
    a_x: f32,
    a_y: f32,
    b_x: f32,
    b_y: f32,
    c_x: f32,
    c_y: f32,
    tolerance: f32,
) -> bool {
    // The dot product of a perpendicular angle is 0. By rotating one of the vectors,
    // we save the calculations to convert the dot product to degrees afterwards.
    let ab = Point { x: b_x - a_x, y: b_y - a_y }.rotate90();
    let ac = Point { x: c_x - a_x, y: c_y - a_y };
    let dot_product = ab.dot_product(ac).abs();
    let relative_tolerance = tolerance * ab.distance() * ac.distance();

    dot_product < tolerance || dot_product < relative_tolerance
}

/// Approximates whether corner at this vertex is concave or convex, based on the
/// relationship of the prev->curr/curr->next vectors.
pub(crate) fn convex(previous: Point, current: Point, next: Point) -> bool {
    // TODO: b/369320447 - This is a fast, but not reliable calculation.
    (current - previous).clockwise(next - current)
}

/// Does a ternary search in `v0..v1` to find the parameter that minimizes the given
/// function. Stops when the search space size is reduced below the given tolerance.
#[allow(dead_code)]
pub(crate) fn find_minimum(v0: f32, v1: f32, tolerance: f32, f: &dyn Fn(f32) -> f32) -> f32 {
    let mut a = v0;
    let mut b = v1;
    while b - a > tolerance {
        let c1 = (2. * a + b) / 3.;
        let c2 = (2. * b + a) / 3.;
        if f(c1) < f(c2) {
            b = c2;
        } else {
            a = c1;
        }
    }
    (a + b) / 2.
}

// The `kmath` helpers mirror `kotlin.math` Float overloads: they evaluate in f64 and
// narrow back to f32, like `Math.sin(x.toDouble()).toFloat()` does on the JVM.

/// `kotlin.math.sqrt(Float)` — correctly rounded in f64, narrowed to f32.
pub(crate) fn k_sqrt(x: f32) -> f32 {
    (x as f64).sqrt() as f32
}

/// `kotlin.math.sin(Float)`.
pub(crate) fn k_sin(x: f32) -> f32 {
    (x as f64).sin() as f32
}

/// `kotlin.math.cos(Float)`.
pub(crate) fn k_cos(x: f32) -> f32 {
    (x as f64).cos() as f32
}

/// `kotlin.math.tan(Float)`.
pub(crate) fn k_tan(x: f32) -> f32 {
    (x as f64).tan() as f32
}

/// `kotlin.math.atan2(Float, Float)`.
pub(crate) fn k_atan2(y: f32, x: f32) -> f32 {
    (y as f64).atan2(x as f64) as f32
}

/// `kotlin.math.min(Float, Float)` — NaN propagates like `Math.min`.
pub(crate) fn k_min(a: f32, b: f32) -> f32 {
    if a.is_nan() || b.is_nan() { f32::NAN } else { a.min(b) }
}

/// `kotlin.math.max(Float, Float)` — NaN propagates like `Math.max`.
pub(crate) fn k_max(a: f32, b: f32) -> f32 {
    if a.is_nan() || b.is_nan() { f32::NAN } else { a.max(b) }
}

/// `kotlin.math.ceil(Float)` — returns a Float, widened through Double.
#[allow(dead_code)]
pub(crate) fn k_ceil(x: f32) -> f32 {
    (x as f64).ceil() as f32
}
