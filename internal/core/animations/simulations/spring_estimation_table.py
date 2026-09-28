#!/usr/bin/env python3
# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

"""Regenerates the reference table in `spring_estimation.rs`'s
`estimation_matches_compose_reference_table` test.

A line-by-line transcription of `SpringEstimation.kt` from
androidx.compose.animation.core (pinned androidx commit
23327507f7fc7d5b19d65fec4b090f60c970079b). Kotlin `Double` is Python `float`;
`estimateAnimationDurationMillis` returns `Long` = the truncated product.
"""

import math
import sys

MAX_LONG_MILLIS = sys.maxsize // 1_000_000


def iterate_newtons_method(x, fn, fn_prime):
    return x - fn(x) / fn_prime(x)


def estimate_under_damped(first_root_real, first_root_imaginary, p0, v0, delta):
    r = first_root_real
    c1 = p0
    c2 = (v0 - r * c1) / first_root_imaginary
    c = math.sqrt(c1 * c1 + c2 * c2)
    return math.log(delta / c) / r


def estimate_critically_damped(first_root_real, p0, v0, delta):
    r = first_root_real
    c1 = p0
    c2 = v0 - r * c1

    t1 = math.log(abs(delta / c1)) / r
    guess = math.log(abs(delta / c2))
    t = guess
    for _ in range(6):
        t = guess - math.log(abs(t / r))
    t2 = t / r

    if not math.isfinite(t1):
        t_curr = t2
    elif not math.isfinite(t2):
        t_curr = t1
    else:
        t_curr = max(t1, t2)

    t_inflection = -(r * c1 + c2) / (r * c2)
    x_inflection = c1 * math.exp(r * t_inflection) + c2 * t_inflection * math.exp(r * t_inflection)

    if math.isnan(t_inflection) or t_inflection <= 0.0:
        signed_delta = -delta
    elif t_inflection > 0.0 and -x_inflection < delta:
        if c2 < 0 and c1 > 0:
            t_curr = 0.0
        signed_delta = -delta
    else:
        t_curr = -(2.0 / r) - (c1 / c2)
        signed_delta = delta

    t_delta = sys.float_info.max
    iterations = 0
    while t_delta > 0.001 and iterations < 100:
        iterations += 1
        t_last = t_curr
        t_curr = iterate_newtons_method(
            t_curr,
            lambda t: (c1 + c2 * t) * math.exp(r * t) + signed_delta,
            lambda t: (c2 * (r * t + 1) + c1 * r) * math.exp(r * t),
        )
        t_delta = abs(t_last - t_curr)
    return t_curr


def estimate_over_damped(first_root_real, second_root_real, p0, v0, delta):
    r1 = first_root_real
    r2 = second_root_real
    c2 = (r1 * p0 - v0) / (r1 - r2)
    c1 = p0 - c2

    t1 = math.log(abs(delta / c1)) / r1
    t2 = math.log(abs(delta / c2)) / r2

    if not math.isfinite(t1):
        t_curr = t2
    elif not math.isfinite(t2):
        t_curr = t1
    else:
        t_curr = max(t1, t2)

    t_inflection = math.log((c1 * r1) / (-c2 * r2)) / (r2 - r1)
    x_inflection = c1 * math.exp(r1 * t_inflection) + c2 * math.exp(r2 * t_inflection)

    if math.isnan(t_inflection) or t_inflection <= 0.0:
        signed_delta = -delta
    elif t_inflection > 0.0 and -x_inflection < delta:
        if c2 > 0.0 and c1 < 0.0:
            t_curr = 0.0
        signed_delta = -delta
    else:
        t_curr = math.log(-(c2 * r2 * r2) / (c1 * r1 * r1)) / (r1 - r2)
        signed_delta = delta

    if abs(c1 * r1 * math.exp(r1 * t_curr) + c2 * r2 * math.exp(r2 * t_curr)) < 0.0001:
        return t_curr

    t_delta = sys.float_info.max
    iterations = 0
    while t_delta > 0.001 and iterations < 100:
        iterations += 1
        t_last = t_curr
        t_curr = iterate_newtons_method(
            t_curr,
            lambda t: c1 * math.exp(r1 * t) + c2 * math.exp(r2 * t) + signed_delta,
            lambda t: c1 * r1 * math.exp(r1 * t) + c2 * r2 * math.exp(r2 * t),
        )
        t_delta = abs(t_last - t_curr)
    return t_curr


def estimate_duration_internal(
    first_root_real, first_root_imaginary, second_root_real,
    damping_ratio, initial_velocity, initial_position, delta,
):
    if initial_position == 0.0 and initial_velocity == 0.0:
        return 0
    v0 = -initial_velocity if initial_position < 0 else initial_velocity
    p0 = abs(initial_position)
    if damping_ratio > 1.0:
        estimate = estimate_over_damped(first_root_real, second_root_real, p0, v0, delta)
    elif damping_ratio < 1.0:
        estimate = estimate_under_damped(
            first_root_real, first_root_imaginary, p0, v0, delta
        )
    else:
        estimate = estimate_critically_damped(first_root_real, p0, v0, delta)
    return int(estimate * 1000.0)


def estimate_animation_duration_ms_with_mass(
    spring_constant, damping_coefficient, mass, initial_velocity, initial_displacement, delta
):
    """`SpringEstimation.estimateAnimationDurationMillis` (mass overload)."""
    critical_damping = 2.0 * math.sqrt(spring_constant * mass)
    damping_ratio = damping_coefficient / critical_damping
    partial_root = damping_coefficient * damping_coefficient - 4.0 * mass * spring_constant
    divisor = 1.0 / (2.0 * mass)
    partial_root_real = 0.0 if partial_root < 0.0 else math.sqrt(partial_root)
    partial_root_imaginary = math.sqrt(abs(partial_root)) if partial_root < 0.0 else 0.0
    first_root_real = (-damping_coefficient + partial_root_real) * divisor
    first_root_imaginary = partial_root_imaginary * divisor
    second_root_real = (-damping_coefficient - partial_root_real) * divisor
    return estimate_duration_internal(
        first_root_real,
        first_root_imaginary,
        second_root_real,
        damping_ratio,
        initial_velocity,
        initial_displacement,
        delta,
    )


# The 12 MaterialMotion springs (MotionScheme.kt / material_motion_tokens.slint)
M3_TOKEN_SPECS = [
    ("standard · default spatial", 0.9, 700.0),
    ("standard · default effects", 1.0, 1600.0),
    ("standard · fast spatial", 0.9, 1400.0),
    ("standard · fast effects", 1.0, 3800.0),
    ("standard · slow spatial", 0.9, 300.0),
    ("standard · slow effects", 1.0, 800.0),
    ("expressive · default spatial", 0.8, 380.0),
    ("expressive · default effects", 1.0, 1600.0),
    ("expressive · fast spatial", 0.6, 800.0),
    ("expressive · fast effects", 1.0, 3800.0),
    ("expressive · slow spatial", 0.8, 200.0),
    ("expressive · slow effects", 1.0, 800.0),
]

CASES = [(-100.0, 0.0), (42.0, -350.0), (5.0, 1000.0)]
DELTA = 0.01  # SPRING_DEFAULT_DISPLACEMENT_THRESHOLD
MASS = 1.0


def main():
    print("let table: [(f64, f64, [u64; 3]); 12] = [")
    for name, zeta, k in M3_TOKEN_SPECS:
        damping_coefficient = 2.0 * zeta * math.sqrt(k * MASS)
        row = [
            estimate_animation_duration_ms_with_mass(k, damping_coefficient, MASS, v0, x0, DELTA)
            for x0, v0 in CASES
        ]
        print(f"    ({zeta}, {k:g}., {row}),  // {name}")
    print("];")


if __name__ == "__main__":
    main()
