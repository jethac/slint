// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Port of kotlin/utils/MathUtils.kt (material-color-utilities @ 5b3618b).

use crate::math;

/// Utility methods for mathematical operations.
pub struct MathUtils;

impl MathUtils {
    /// The linear interpolation function.
    ///
    /// Returns `start` if `amount` = 0 and `stop` if `amount` = 1.
    pub fn lerp(start: f64, stop: f64, amount: f64) -> f64 {
        (1.0 - amount) * start + amount * stop
    }

    /// Sanitizes a degree measure as an integer.
    ///
    /// Returns a degree measure between 0 (inclusive) and 360 (exclusive).
    pub fn sanitize_degrees_int(degrees: i64) -> i64 {
        let mut degrees = degrees % 360;
        if degrees < 0 {
            degrees += 360;
        }
        degrees
    }

    /// Sanitizes a degree measure as a floating-point number.
    ///
    /// Returns a degree measure between 0.0 (inclusive) and 360.0 (exclusive).
    pub fn sanitize_degrees_double(degrees: f64) -> f64 {
        let mut degrees = degrees % 360.0;
        if degrees < 0.0 {
            degrees += 360.0;
        }
        degrees
    }

    /// Clamps an integer between two floating-point numbers.
    ///
    /// Returns `input` when `min <= input <= max`, and either `min` or `max`
    /// otherwise.
    pub fn clamp_double(min: f64, max: f64, input: f64) -> f64 {
        if input < min {
            min
        } else if input > max {
            max
        } else {
            input
        }
    }

    /// Sign of direction change needed to travel from one angle to another.
    ///
    /// For angles that are 180 degrees apart from each other, both directions
    /// have the same travel distance, so either direction is shortest. The
    /// value 1.0 is returned in this case.
    ///
    /// Returns -1 if decreasing `from` leads to the shortest travel distance,
    /// 1 if increasing `from` leads to the shortest travel distance.
    pub fn rotation_direction(from: f64, to: f64) -> f64 {
        let increasing_difference = Self::sanitize_degrees_double(to - from);
        if increasing_difference <= 180.0 { 1.0 } else { -1.0 }
    }

    /// Distance of two points on a circle, represented using degrees.
    pub fn difference_degrees(a: f64, b: f64) -> f64 {
        180.0 - math::abs(math::abs(a - b) - 180.0)
    }

    /// Multiplies a 1x3 row vector with a 3x3 matrix.
    pub fn matrix_multiply(row: [f64; 3], matrix: &[[f64; 3]; 3]) -> [f64; 3] {
        let a = row[0] * matrix[0][0] + row[1] * matrix[0][1] + row[2] * matrix[0][2];
        let b = row[0] * matrix[1][0] + row[1] * matrix[1][1] + row[2] * matrix[1][2];
        let c = row[0] * matrix[2][0] + row[1] * matrix[2][1] + row[2] * matrix[2][2];
        [a, b, c]
    }
}
