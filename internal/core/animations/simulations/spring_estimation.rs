// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Port of `SpringEstimation.kt` from androidx.compose.animation.core (pinned androidx
//! commit 23327507f7fc7d5b19d65fec4b090f60c970079b) — estimates the time after which a
//! spring last exceeds a displacement `delta` from its resting position.
//!
//! The math runs in `f64` exactly like the Kotlin implementation.

#[cfg(not(feature = "std"))]
use num_traits::Float;

/// Returns the estimated time that the spring will last be at `delta`,
/// in milliseconds.
///
/// Port of `estimateAnimationDurationMillis(springConstant, dampingCoefficient,
/// mass, initialVelocity, initialDisplacement, delta)` — the overload taking
/// `(spring_constant, damping_coefficient, mass)`. Unlike the `(stiffness,
/// dampingRatio)` overload this one has no `dampingRatio == 0` early return —
/// a zero damping coefficient takes the under-damped branch and yields an
/// infinite estimate, exactly like the Kotlin source.
pub fn estimate_animation_duration_ms_with_mass(
    spring_constant: f64,
    damping_coefficient: f64,
    mass: f64,
    initial_velocity: f64,
    initial_displacement: f64,
    delta: f64,
) -> u64 {
    let critical_damping = 2.0 * f64::sqrt(spring_constant * mass);
    let damping_ratio = damping_coefficient / critical_damping;

    // Compute the roots of the polynomial [a]x^2+[b]x+[c]=0 which may be complex.
    let partial_root = damping_coefficient * damping_coefficient - 4.0 * mass * spring_constant;
    let divisor = 1.0 / (2.0 * mass);
    let partial_root_real = if partial_root < 0.0 { 0.0 } else { f64::sqrt(partial_root) };
    let partial_root_imaginary =
        if partial_root < 0.0 { f64::sqrt(f64::abs(partial_root)) } else { 0.0 };

    let first_root_real = (-damping_coefficient + partial_root_real) * divisor;
    let first_root_imaginary = partial_root_imaginary * divisor;
    let second_root_real = (-damping_coefficient - partial_root_real) * divisor;

    estimate_duration_internal(
        first_root_real,
        first_root_imaginary,
        second_root_real,
        damping_ratio,
        initial_velocity,
        initial_displacement,
        delta,
    )
}

/// In the under-damped case we simply calculate the envelope of the function. The
/// general solution is of the form `x(t) = c_1*e^(r*t)*cos(...) +
/// c_2*e^(r*t)*sin(...)` which simplifies to `x(t) = c*e^(r*t)*cos(...)` where
/// `c*e^(r*t)` is the envelope of `x(t)`
fn estimate_under_damped(
    first_root_real: f64,
    first_root_imaginary: f64,
    p0: f64,
    v0: f64,
    delta: f64,
) -> f64 {
    let r = first_root_real;
    let c1 = p0;
    let c2 = (v0 - r * c1) / first_root_imaginary;
    let c = f64::sqrt(c1 * c1 + c2 * c2);

    f64::ln(delta / c) / r
}

/// In the critically-damped case we apply Newton-Raphson's iterative numerical
/// method of solving the equation `x(t) = c_1*e^(r*t) + c_2*t*e^(r*t)`
fn estimate_critically_damped(first_root_real: f64, p0: f64, v0: f64, delta: f64) -> f64 {
    let r = first_root_real;
    let c1 = p0;
    let c2 = v0 - r * c1;

    // For our initial guess, determine the max t of c_1*e^(r*t) = delta and
    // c_2*t*e^(r*t) = delta
    let t1 = f64::ln(f64::abs(delta / c1)) / r;
    let t2 = {
        // Application of Lambert's W function to solve te^t
        let guess = f64::ln(f64::abs(delta / c2));
        let mut t = guess;
        for _ in 0..6 {
            t = guess - f64::ln(f64::abs(t / r));
        }
        t
    } / r;
    let mut t_curr = if !t1.is_finite() {
        t2
    } else if !t2.is_finite() {
        t1
    } else {
        f64::max(t1, t2)
    };

    // Calculate the inflection time. This is important if the inflection is in t > 0
    let t_inflection = -(r * c1 + c2) / (r * c2);
    let x_inflection =
        c1 * f64::exp(r * t_inflection) + c2 * t_inflection * f64::exp(r * t_inflection);

    // For inflection that does not exist in real time, we always solve for x(t)=delta.
    // Note the system is manipulated such that p0 is always positive.
    let signed_delta = if t_inflection.is_nan() || t_inflection <= 0.0 {
        -delta
    } else if t_inflection > 0.0 && -x_inflection < delta {
        // In this scenario the first crossing with the threshold is to be found. Note
        // that the inflection does not exceed delta. As such, we search from the left.
        if c2 < 0.0 && c1 > 0.0 {
            t_curr = 0.0;
        }
        -delta
    } else {
        // In this scenario there are three total crossings of the threshold, once from
        // above, and then once when the inflection exceeds the threshold and then one
        // last one when x(t) finally decays to zero. The point of determining
        // concavity is to find the final crossing.
        //
        // By finding a point between when concavity changes, and when the inflection
        // point is, Newton's method will always converge onto the rightmost point (in
        // this case), the one that we are interested in.
        t_curr = -(2.0 / r) - (c1 / c2);
        delta
    };

    let mut t_delta = f64::MAX;
    let mut iterations = 0;
    while t_delta > 0.001 && iterations < 100 {
        iterations += 1;
        let t_last = t_curr;
        t_curr = iterate_newtons_method(
            t_curr,
            &|t| (c1 + c2 * t) * f64::exp(r * t) + signed_delta,
            &|t| (c2 * (r * t + 1.0) + c1 * r) * f64::exp(r * t),
        );
        t_delta = f64::abs(t_last - t_curr);
    }

    t_curr
}

/// In the over-damped case we apply Newton-Raphson's iterative numerical method of
/// solving the equation `x(t) = c_1*e^(r_1*t) + c_2*e^(r_2*t)`
fn estimate_over_damped(
    first_root_real: f64,
    second_root_real: f64,
    p0: f64,
    v0: f64,
    delta: f64,
) -> f64 {
    let r1 = first_root_real;
    let r2 = second_root_real;
    let c2 = (r1 * p0 - v0) / (r1 - r2);
    let c1 = p0 - c2;

    // For our initial guess, determine the max t of c_1*e^(r_1*t) = delta and
    // c_2*e^(r_2*t) = delta
    let t1 = f64::ln(f64::abs(delta / c1)) / r1;
    let t2 = f64::ln(f64::abs(delta / c2)) / r2;

    let mut t_curr = if !t1.is_finite() {
        t2
    } else if !t2.is_finite() {
        t1
    } else {
        f64::max(t1, t2)
    };

    // Calculate the inflection time. This is important if the inflection is in t > 0
    let t_inflection = f64::ln((c1 * r1) / (-c2 * r2)) / (r2 - r1);
    let x_inflection = |t: f64| c1 * f64::exp(r1 * t) + c2 * f64::exp(r2 * t);

    // For inflection that does not exist in real time, we always solve for x(t)=delta.
    // Note the system is manipulated such that p0 is always positive.
    let signed_delta = if t_inflection.is_nan() || t_inflection <= 0.0 {
        -delta
    } else if t_inflection > 0.0 && -x_inflection(t_inflection) < delta {
        // In this scenario the first crossing with the threshold is to be found. Note
        // that the inflection does not exceed delta. As such, we search from the left.
        if c2 > 0.0 && c1 < 0.0 {
            t_curr = 0.0;
        }
        -delta
    } else {
        // In this scenario there are three total crossings of the threshold, once from
        // above, and then once when the inflection exceeds the threshold and then one
        // last one when x(t) finally decays to zero. The point of determining
        // concavity is to find the final crossing.
        //
        // By finding a point between when concavity changes, and when the inflection
        // point is, Newton's method will always converge onto the rightmost point (in
        // this case), the one that we are interested in.
        t_curr = f64::ln(-(c2 * r2 * r2) / (c1 * r1 * r1)) / (r1 - r2);
        delta
    };

    // For a good initial guess, simply return
    if f64::abs(c1 * r1 * f64::exp(r1 * t_curr) + c2 * r2 * f64::exp(r2 * t_curr)) < 0.0001 {
        return t_curr;
    }
    let mut t_delta = f64::MAX;

    // Cap iterations for safety - Experimentally this method takes <= 5 iterations
    let mut iterations = 0;
    while t_delta > 0.001 && iterations < 100 {
        iterations += 1;
        let t_last = t_curr;
        t_curr = iterate_newtons_method(
            t_curr,
            &|t| c1 * f64::exp(r1 * t) + c2 * f64::exp(r2 * t) + signed_delta,
            &|t| c1 * r1 * f64::exp(r1 * t) + c2 * r2 * f64::exp(r2 * t),
        );
        t_delta = f64::abs(t_last - t_curr);
    }

    t_curr
}

// Applies Newton-Raphson's method to solve for the estimated time the spring mass
// system will last be at [delta].
fn estimate_duration_internal(
    first_root_real: f64,
    first_root_imaginary: f64,
    second_root_real: f64,
    damping_ratio: f64,
    initial_velocity: f64,
    initial_position: f64,
    delta: f64,
) -> u64 {
    if initial_position == 0.0 && initial_velocity == 0.0 {
        return 0;
    }

    let v0 = if initial_position < 0.0 { -initial_velocity } else { initial_velocity };
    let p0 = f64::abs(initial_position);

    (if damping_ratio > 1.0 {
        estimate_over_damped(first_root_real, second_root_real, p0, v0, delta)
    } else if damping_ratio < 1.0 {
        estimate_under_damped(first_root_real, first_root_imaginary, p0, v0, delta)
    } else {
        estimate_critically_damped(first_root_real, p0, v0, delta)
    } * 1000.0) as u64
}

fn iterate_newtons_method(x: f64, fn_: &dyn Fn(f64) -> f64, fn_prime: &dyn Fn(f64) -> f64) -> f64 {
    x - fn_(x) / fn_prime(x)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Reference estimates for `estimate_animation_duration_ms_with_mass`,
    /// generated by a transcription of `SpringEstimation.kt` (pinned androidx
    /// commit 23327507f7fc7d5b19d65fec4b090f60c970079b) — the Kotlin computes
    /// in `f64`, so identical inputs yield identical integers. The token pairs
    /// are the 12 `MaterialMotion` springs (`MotionScheme.kt`, mirrored in
    /// `material_motion_tokens.slint`), each estimated for three
    /// `(displacement, velocity)` starts at `delta` = 0.01
    /// (`SPRING_DEFAULT_DISPLACEMENT_THRESHOLD`), mass 1.
    #[test]
    fn estimation_matches_compose_reference_table() {
        // (damping_ratio, stiffness, [est for (x0,v0) ∈ {(-100,0), (42,-350), (5,1000)}])
        let table: [(f64, f64, [u64; 3]); 12] = [
            (0.9, 700., [421, 371, 385]),  // standard · default spatial
            (1.0, 1600., [293, 264, 259]), // standard · default effects
            (0.9, 1400., [298, 265, 263]), // standard · fast spatial
            (1.0, 3800., [190, 173, 161]), // standard · fast effects
            (0.9, 300., [644, 556, 613]),  // standard · slow spatial
            (1.0, 800., [415, 370, 377]),  // standard · slow effects
            (0.8, 380., [623, 545, 585]),  // expressive · default spatial
            (1.0, 1600., [293, 264, 259]), // expressive · default effects
            (0.6, 800., [555, 495, 499]),  // expressive · fast spatial
            (1.0, 3800., [190, 173, 161]), // expressive · fast effects
            (0.8, 200., [859, 742, 833]),  // expressive · slow spatial
            (1.0, 800., [415, 370, 377]),  // expressive · slow effects
        ];
        let cases = [(-100.0, 0.0), (42.0, -350.0), (5.0, 1000.0)];
        for (zeta, k, expected) in table {
            for ((x0, v0), expected) in cases.iter().zip(expected) {
                let damping_coefficient = 2.0 * zeta * f64::sqrt(k);
                let actual = estimate_animation_duration_ms_with_mass(
                    k,
                    damping_coefficient,
                    1.0,
                    *v0,
                    *x0,
                    0.01,
                );
                assert_eq!(
                    actual, expected,
                    "ζ={zeta} k={k} x0={x0} v0={v0}: {actual} vs {expected}"
                );
            }
        }
    }
}
