// Copyright © Klarälvdalens Datakonsult AB, a KDAB Group company , info@kdab.com, author Robin Cramer <robin.cramer@kdab.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore signum underdamped

#[cfg(test)]
use crate::animations::simulations::assert_approx_eq;

#[cfg(not(feature = "std"))]
use num_traits::Float;

/// Converts a springs configuration into the `(natural_frequency, damping_ratio)` pair
/// that the `SpringSimulation` solves the ODE with.
pub trait SpringParameters {
    /// Returns `(w_n, zeta)`
    fn to_natural_frequency_and_damping_ratio(&self) -> (f32, f32);
}

/// `duration` decides the natural frequency and bounce decides the damping
#[derive(Debug, Clone, Copy)]
pub struct SpringDurationBounceParameters {
    /// Fixes the spring's natural frequency, independent of `bounce`.
    pub duration_secs: f32,
    /// Expected range `-1.0..=1.0`, but not clamped here.
    pub bounce: f32,
}

impl SpringDurationBounceParameters {
    /// Creates new `duration`/`bounce`-style spring parameters.
    pub fn new(duration_secs: f32, bounce: f32) -> Self {
        Self { duration_secs, bounce }
    }
}

impl SpringParameters for SpringDurationBounceParameters {
    fn to_natural_frequency_and_damping_ratio(&self) -> (f32, f32) {
        debug_assert!(self.duration_secs > 0., "duration must be greater than zero");
        let w_n = 2. * core::f32::consts::PI / self.duration_secs;
        let zeta = 1. - self.bounce;
        (w_n, zeta)
    }
}

/// `mass`/`stiffness`/`damping`-style spring configuration
#[allow(dead_code)] // leaving in case a physical-spring curve is added
#[derive(Debug, Clone, Copy)]
pub struct SpringPhysicalParameters {
    /// The mass attached to the spring
    pub mass: f32,
    /// The spring's stiffness (spring constant)
    pub stiffness: f32,
    /// The spring's damping coefficient
    pub damping: f32,
}

impl SpringPhysicalParameters {
    /// Creates new `mass`/`stiffness`/`damping`-style spring parameters.
    #[allow(dead_code)] // leaving in case a physical-spring curve is added
    pub fn new(mass: f32, stiffness: f32, damping: f32) -> Self {
        Self { mass, stiffness, damping }
    }
}

impl SpringParameters for SpringPhysicalParameters {
    fn to_natural_frequency_and_damping_ratio(&self) -> (f32, f32) {
        debug_assert!(self.mass > 0., "mass must be greater than zero");
        debug_assert!(self.stiffness >= 0., "stiffness must not be negative");
        let w_n = f32::sqrt(self.stiffness / self.mass);
        let critical_damping = 2. * f32::sqrt(self.mass * self.stiffness);
        let zeta = if critical_damping > 0. { self.damping / critical_damping } else { 0. };
        (w_n, zeta)
    }
}

/// Precomputed coefficients for a spring, one variant per damping regime
/// All are relative to `target` (`x_rel = x - target`)
/// `x_rel(0) = start_value - target`
/// `vel(0) = initial_velocity`
#[derive(Debug, Clone, Copy)]
pub enum SpringRegime {
    /// `zeta < 1`: oscillates while decaying. `x_rel(t) = e^(-zeta*w_n*t) * (c1*cos(w_d*t) + c2*sin(w_d*t))`
    Underdamped { w_n: f32, zeta: f32, w_d: f32, c1: f32, c2: f32 },
    /// `zeta == 1`: fastest non-oscillating approach. `x_rel(t) = (c1 + c2*t) * e^(-w_n*t)`
    Critical { w_n: f32, c1: f32, c2: f32 },
    /// `zeta > 1`: slow, non-oscillating approach. `x_rel(t) = c1*e^(r1*t) + c2*e^(r2*t)`
    Overdamped { r1: f32, r2: f32, c1: f32, c2: f32 },
}

impl SpringRegime {
    pub(crate) fn new(x0: f32, v0: f32, w_n: f32, zeta: f32) -> Self {
        // Compose branches on `dampingRatio == 1f` exactly (SpringSimulation.kt
        // `updateValues`), so only a damping ratio of exactly 1 is critical.
        if zeta == 1. {
            Self::Critical { w_n, c1: x0, c2: v0 + w_n * x0 }
        } else if zeta < 1. {
            let w_d = w_n * f32::sqrt(1. - zeta * zeta);
            Self::Underdamped { w_n, zeta, w_d, c1: x0, c2: (v0 + zeta * w_n * x0) / w_d }
        } else {
            let disc = f32::sqrt(zeta * zeta - 1.);
            let r1 = w_n * (-zeta + disc);
            let r2 = w_n * (-zeta - disc);
            let c1 = (v0 - r2 * x0) / (r1 - r2);
            Self::Overdamped { r1, r2, c1, c2: x0 - c1 }
        }
    }

    /// This regime's damping ratio
    pub(crate) fn zeta(&self) -> f32 {
        match *self {
            Self::Underdamped { zeta, .. } => zeta,
            Self::Critical { .. } => 1.0,
            // Not stored in regime but doesn't matter in comparisons so return 1
            // It is going to be less than 1 in reality
            Self::Overdamped { .. } => 1.0,
        }
    }

    /// Evaluates the closed form at elapsed time `t`, returning `(x_rel, vel)`.
    pub(crate) fn evaluate(&self, t: f32) -> (f32, f32) {
        match *self {
            Self::Underdamped { w_n, zeta, w_d, c1, c2 } => {
                let decay = f32::exp(-zeta * w_n * t);
                let (s, c) = f32::sin_cos(w_d * t);
                let pos = decay * (c1 * c + c2 * s);
                let vel =
                    decay * ((-zeta * w_n * c1 + w_d * c2) * c + (-zeta * w_n * c2 - w_d * c1) * s);
                (pos, vel)
            }
            Self::Critical { w_n, c1, c2 } => {
                let decay = f32::exp(-w_n * t);
                let pos = decay * (c1 + c2 * t);
                let vel = decay * (c2 - w_n * (c1 + c2 * t));
                (pos, vel)
            }
            Self::Overdamped { r1, r2, c1, c2 } => {
                let pos = c1 * f32::exp(r1 * t) + c2 * f32::exp(r2 * t);
                let vel = c1 * r1 * f32::exp(r1 * t) + c2 * r2 * f32::exp(r2 * t);
                (pos, vel)
            }
        }
    }
}

/// Input parameters for the [`PhysicalSpringToLimit`] simulation: a
/// `spring(damping_ratio, stiffness[, mass])` fling toward a limit value.
#[derive(Debug, Clone, Copy)]
pub struct PhysicalSpringParameters {
    /// The damping ratio `ζ` of the spring.
    pub damping_ratio: f32,
    /// The spring constant `k`.
    pub stiffness: f32,
    /// The mass attached to the spring.
    pub mass: f32,
    /// The initial velocity, in the property's units per second.
    pub initial_velocity: f32,
}

impl PhysicalSpringParameters {
    /// Creates new physical-spring parameters, clamping invalid values the way
    /// the runtime does for `spring()` (ζ >= 0, stiffness > 0, mass > 0).
    pub fn new(damping_ratio: f32, stiffness: f32, mass: f32, initial_velocity: f32) -> Self {
        Self {
            damping_ratio: damping_ratio.max(0.),
            stiffness: stiffness.max(f32::EPSILON),
            mass: mass.max(f32::EPSILON),
            initial_velocity,
        }
    }
}

impl super::Parameter for PhysicalSpringParameters {
    type Output = PhysicalSpringToLimit;
    fn simulation(
        self,
        start_value: f32,
        limit_value: core::pin::Pin<alloc::boxed::Box<crate::Property<f32>>>,
    ) -> Self::Output {
        PhysicalSpringToLimit::new(start_value, limit_value, self)
    }
}

/// Settle-driven spring simulation toward `limit_value`, driven by
/// `spring(damping_ratio, stiffness[, mass])` parameters. Solves the same ODE
/// as [`SpringRegime`]; the displacement is measured relative to the limit, so a
/// changing limit retargets the spring from the current position and velocity.
#[derive(Debug)]
pub struct PhysicalSpringToLimit {
    limit_value: core::pin::Pin<alloc::boxed::Box<crate::Property<f32>>>,
    params: PhysicalSpringParameters,
    w_n: f32,
    zeta: f32,
    /// The solved spring relative to `anchored_limit`, anchored at `anchor_tick`.
    regime: SpringRegime,
    anchor_tick: crate::animations::Instant,
    anchored_limit: f32,
    /// Estimated remaining duration in milliseconds, from `anchor_tick`.
    estimated_ms: u64,
}

impl PhysicalSpringToLimit {
    fn new(
        start_value: f32,
        limit_value: core::pin::Pin<alloc::boxed::Box<crate::Property<f32>>>,
        params: PhysicalSpringParameters,
    ) -> Self {
        let w_n = f32::sqrt(params.stiffness / params.mass);
        let anchored_limit = limit_value.as_ref().get();
        let regime = SpringRegime::new(
            start_value - anchored_limit,
            params.initial_velocity,
            w_n,
            params.damping_ratio,
        );
        Self {
            limit_value,
            params,
            w_n,
            zeta: params.damping_ratio,
            regime,
            anchor_tick: crate::animations::current_tick(),
            anchored_limit,
            estimated_ms: 0,
        }
        .re_estimated()
    }

    /// Recomputes the estimated remaining duration after (re)anchoring `regime`.
    fn re_estimated(mut self) -> Self {
        let (x0, v0) = self.regime.evaluate(0.);
        self.estimated_ms = if self.params.damping_ratio == 0. {
            u64::MAX
        } else {
            super::spring_estimation::estimate_animation_duration_ms_with_mass(
                self.params.stiffness as f64,
                // damping_coefficient = 2 * ζ * sqrt(k * m)
                2.0 * self.params.damping_ratio as f64
                    * f64::sqrt(self.params.stiffness as f64 * self.params.mass as f64),
                self.params.mass as f64,
                v0 as f64,
                x0 as f64,
                crate::animations::SPRING_DEFAULT_DISPLACEMENT_THRESHOLD as f64,
            )
        };
        self
    }
}

impl super::Simulation for PhysicalSpringToLimit {
    fn step(&mut self, current: &mut f32, new_tick: crate::animations::Instant) -> bool {
        let limit = self.limit_value.as_ref().get();
        let t = new_tick.duration_since(self.anchor_tick).as_secs_f32();
        let (rel, vel) = self.regime.evaluate(t);
        if limit != self.anchored_limit {
            // The limit moved (e.g. the content size changed): re-anchor the
            // spring onto the new limit, keeping the current position and velocity.
            self.regime =
                SpringRegime::new(self.anchored_limit + rel - limit, vel, self.w_n, self.zeta);
            self.anchor_tick = new_tick;
            self.anchored_limit = limit;
            let (x0, v0) = self.regime.evaluate(0.);
            self.estimated_ms = if self.zeta == 0. {
                u64::MAX
            } else {
                super::spring_estimation::estimate_animation_duration_ms_with_mass(
                    self.params.stiffness as f64,
                    2.0 * self.zeta as f64
                        * f64::sqrt(self.params.stiffness as f64 * self.params.mass as f64),
                    self.params.mass as f64,
                    v0 as f64,
                    x0 as f64,
                    crate::animations::SPRING_DEFAULT_DISPLACEMENT_THRESHOLD as f64,
                )
            };
            let t = 0.;
            let (rel, _vel) = self.regime.evaluate(t);
            *current = limit + rel;
            return self.estimated_ms == 0;
        }
        *current = self.anchored_limit + rel;
        t * 1000. >= self.estimated_ms as f32
    }
}

#[cfg(test)]
mod spring_regime_tests {
    use super::*;
    use crate::animations::simulations::{Simulation, test_limit_property};
    use core::time::Duration;

    const W_N: f32 = 10.;
    const X0: f32 = 5.;
    const V0: f32 = -3.;

    #[test]
    fn regime_matches_initial_conditions() {
        let regime = SpringRegime::new(X0, V0, W_N, 0.3);
        let (pos, vel) = regime.evaluate(0.);
        assert_approx_eq!(pos, X0);
        assert_approx_eq!(vel, V0);

        let regime = SpringRegime::new(X0, V0, W_N, 1.);
        let (pos, vel) = regime.evaluate(0.);
        assert_approx_eq!(pos, X0);
        assert_approx_eq!(vel, V0);

        let regime = SpringRegime::new(X0, V0, W_N, 1.8);
        let (pos, vel) = regime.evaluate(0.);
        assert_approx_eq!(pos, X0);
        assert_approx_eq!(vel, V0);
    }

    #[test]
    fn regime_decays_to_rest_over_time() {
        let regime = SpringRegime::new(X0, V0, W_N, 0.3);
        let (pos, vel) = regime.evaluate(10.);
        assert_approx_eq!(pos, 0.);
        assert_approx_eq!(vel, 0.);

        let regime = SpringRegime::new(X0, V0, W_N, 1.);
        let (pos, vel) = regime.evaluate(10.);
        assert_approx_eq!(pos, 0.);
        assert_approx_eq!(vel, 0.);

        let regime = SpringRegime::new(X0, V0, W_N, 1.8);
        let (pos, vel) = regime.evaluate(10.);
        assert_approx_eq!(pos, 0.);
        assert_approx_eq!(vel, 0.);
    }

    #[test]
    fn undamped_oscillates_without_decay() {
        // zeta == 0: pure oscillation, amplitude must be preserved over a full period
        let regime = SpringRegime::new(X0, 0., W_N, 0.);
        let period = 2. * core::f32::consts::PI / W_N;
        let (pos, vel) = regime.evaluate(period);
        assert_approx_eq!(pos, X0);
        assert_approx_eq!(vel, 0.);

        // Quarter period: position crosses zero, velocity is at its (negative) extreme
        let (pos, vel) = regime.evaluate(period / 4.);
        assert_approx_eq!(pos, 0.);
        assert_approx_eq!(vel, -X0 * W_N);
    }

    /// Reference closed-form solution of the spring ODE in `f64`, matching
    /// androidx `SpringSimulation` (`SpringSimulation.kt`) at the pinned commit.
    /// `x0`/`v0` are measured relative to the target.
    fn reference(t: f64, x0: f64, v0: f64, zeta: f64, w_n: f64) -> (f64, f64) {
        if zeta < 1.0 {
            let w_d = w_n * f64::sqrt(1.0 - zeta * zeta);
            let e = f64::exp(-zeta * w_n * t);
            let c2 = (v0 + zeta * w_n * x0) / w_d;
            let pos = e * (x0 * f64::cos(w_d * t) + c2 * f64::sin(w_d * t));
            let vel =
                -zeta * w_n * pos + e * w_d * (-x0 * f64::sin(w_d * t) + c2 * f64::cos(w_d * t));
            (pos, vel)
        } else {
            // Compose uses the "real mode" expression for zeta > 1 and a
            // critically damped form at zeta == 1.
            if (zeta - 1.0).abs() < 1e-9 {
                let e = f64::exp(-w_n * t);
                let c2 = v0 + w_n * x0;
                let pos = e * (x0 + c2 * t);
                let vel = e * (c2 - w_n * (x0 + c2 * t));
                (pos, vel)
            } else {
                let disc = f64::sqrt(zeta * zeta - 1.0);
                let r1 = w_n * (-zeta + disc);
                let r2 = w_n * (-zeta - disc);
                let c1 = (v0 - r2 * x0) / (r1 - r2);
                let c2 = x0 - c1;
                let pos = c1 * f64::exp(r1 * t) + c2 * f64::exp(r2 * t);
                let vel = c1 * r1 * f64::exp(r1 * t) + c2 * r2 * f64::exp(r2 * t);
                (pos, vel)
            }
        }
    }

    /// The 12 `MaterialMotion` spring specs — `(damping_ratio, stiffness)` for
    /// `{default, fast, slow} × {spatial, effects}` in both the `standard` and
    /// `expressive` schemes, from `MotionScheme.kt`/`MotionTokens` at the
    /// pinned androidx commit (mirrored in
    /// `ui-libraries/material/src/ui/styling/generated/material_motion_tokens.slint`).
    const M3_TOKEN_SPECS: [(f32, f32); 12] = [
        // standard scheme
        (0.9, 700.),  // default spatial
        (1.0, 1600.), // default effects
        (0.9, 1400.), // fast spatial
        (1.0, 3800.), // fast effects
        (0.9, 300.),  // slow spatial
        (1.0, 800.),  // slow effects
        // expressive scheme
        (0.8, 380.),  // default spatial
        (1.0, 1600.), // default effects
        (0.6, 800.),  // fast spatial
        (1.0, 3800.), // fast effects
        (0.8, 200.),  // slow spatial
        (1.0, 800.),  // slow effects
    ];

    /// Every regime matches the f64 reference trajectory, position and
    /// velocity, for all 12 `MaterialMotion` spring tokens plus damping edge
    /// cases, sampled at integer-millisecond times like Compose evaluates its
    /// `SpringSimulation` at frame boundaries.
    ///
    /// Tolerance: `SpringRegime` computes in f32 while the reference is f64.
    /// The f32 `sin`/`cos`/`exp` round-off relative to f64 stays under ~1e-6
    /// per operation; over w_n·t up to ~60 the absolute error stays within
    /// 1e-3 of the amplitude, so position asserts `1e-3` absolute plus `1e-3`
    /// relative to the reference, and velocity — which scales by w_n — gets a
    /// `1e-2` absolute term.
    #[test]
    fn regime_matches_compose_reference() {
        let specs = M3_TOKEN_SPECS
            .iter()
            .copied()
            // undamped, barely damped, and heavily overdamped edges
            .chain([(0.0, 380.), (0.01, 700.), (5.0, 300.), (1.0, 1200.)]);
        for (zeta, stiffness) in specs {
            for (mass, x0, v0) in
                [(1.0f32, -100.0f32, 0.0f32), (1.0, 42.0, -350.0), (0.5, 5.0, 1000.0)]
            {
                let w_n = f32::sqrt(stiffness / mass);
                let regime = SpringRegime::new(x0, v0, w_n, zeta);
                for i in 0..=1000 {
                    let t = i as f32 * 0.001; // integer milliseconds, 0..1 s
                    let (pos, vel) = regime.evaluate(t);
                    let (ref_pos, ref_vel) =
                        reference(t as f64, x0 as f64, v0 as f64, zeta as f64, w_n as f64);
                    assert!(
                        (pos as f64 - ref_pos).abs() < 1e-3 + 1e-3 * ref_pos.abs(),
                        "pos mismatch at t={t} ζ={zeta} k={stiffness} m={mass}: {pos} vs {ref_pos}"
                    );
                    assert!(
                        (vel as f64 - ref_vel).abs() < 1e-2 + 1e-3 * ref_vel.abs(),
                        "vel mismatch at t={t} ζ={zeta} k={stiffness} m={mass}: {vel} vs {ref_vel}"
                    );
                }
            }
        }
    }

    /// Re-anchoring (retargeting) a spring keeps position and velocity
    /// continuous and settles within the `SpringEstimation`-ported estimate —
    /// for every one of the 12 `MaterialMotion` tokens, retargeted both from
    /// rest and mid-flight.
    #[test]
    fn retarget_preserves_velocity_and_settles() {
        let delta = crate::animations::SPRING_DEFAULT_DISPLACEMENT_THRESHOLD;
        for &(zeta, k) in &M3_TOKEN_SPECS {
            let mass = 1.0f32;
            let w_n = f32::sqrt(k / mass);
            for from_rest in [true, false] {
                // Spring at rest at limit 100 (x0=0, v0=0) or 400 ms into a
                // −100 → 0 approach.
                let mut regime = SpringRegime::new(-100.0, 0.0, w_n, zeta);
                let (rel_pos, rel_vel) = if from_rest { (0.0, 0.0) } else { regime.evaluate(0.4) };
                // The limit moves 100 → 200: re-anchor like `PhysicalSpringToLimit`.
                regime = SpringRegime::new(rel_pos - 100.0, rel_vel, w_n, zeta);
                let (pos, vel) = regime.evaluate(0.0);
                assert!(
                    (pos - (rel_pos - 100.0)).abs() < 1e-4
                        && (vel - rel_vel).abs() < 1e-2 + 1e-3 * rel_vel.abs(),
                    "retarget lost continuity: ζ={zeta} k={k} from_rest={from_rest}"
                );

                let estimated =
                    super::super::spring_estimation::estimate_animation_duration_ms_with_mass(
                        k as f64,
                        2.0 * zeta as f64 * f64::sqrt(k as f64 * mass as f64),
                        mass as f64,
                        vel as f64,
                        pos as f64,
                        delta as f64,
                    );
                if pos == 0.0 && vel == 0.0 {
                    // At rest the estimate is 0 and the spring is done.
                    assert_eq!(estimated, 0, "ζ={zeta} k={k} from_rest={from_rest}");
                    continue;
                }
                // `SpringEstimation` bounds the last |x| = δ crossing only —
                // velocity is unconstrained (a stiff spring still moves at
                // |x| = δ). At the integer-ms floor, |x| can exceed δ by one
                // ms of drift: `delta + |v|·1ms`, covered by delta·2.
                let (pos_at, vel_at) = regime.evaluate(estimated as f32 / 1000.);
                assert!(
                    pos_at.abs() < delta * 2.0 && vel_at.is_finite(),
                    "not at rest after {estimated} ms: ζ={zeta} k={k} from_rest={from_rest} \
                     pos={pos_at} vel={vel_at}"
                );
                // The estimate must not be grossly padded: one millisecond
                // earlier the spring is still approaching — outside half the
                // settle threshold (the estimate is the last |x| = δ crossing,
                // tight within roughly a millisecond of decay).
                let (pos_before, _) = regime.evaluate((estimated - 1) as f32 / 1000.);
                assert!(
                    pos_before.abs() > delta * 0.5,
                    "estimate padded: ζ={zeta} k={k} from_rest={from_rest} \
                     pos(et−1ms)={pos_before}"
                );
            }
        }
    }

    /// `PhysicalSpringToLimit` itself: follows the reference trajectory from
    /// `start_value` toward `limit_value` and finishes once the estimate is
    /// reached.
    #[test]
    fn simulation_steps_to_estimate() {
        let mut sim = PhysicalSpringToLimit::new(
            0.0,
            test_limit_property(200.0),
            PhysicalSpringParameters::new(1.0, 380.0, 1.0, 0.0),
        );
        let start = sim.anchor_tick;
        let mut current = 0.0f32;

        // Mid-flight: matches the f64 reference.
        let mut finished = sim.step(&mut current, start + Duration::from_millis(250));
        assert!(!finished);
        let (ref_pos, _) = reference(0.25, -200.0, 0.0, 1.0, f64::sqrt(380.0));
        assert!((current - (200.0 + ref_pos as f32)).abs() < 0.05, "{current}");
        assert!(current > 0.0 && current < 200.0);

        // Retarget mid-flight: the limit moves to 300.
        sim.limit_value.as_ref().set(300.0);
        finished = sim.step(&mut current, start + Duration::from_millis(400));
        assert!(!finished);
        assert!(current > 100.0 && current < 300.0);

        // After the estimate the spring is at rest at the limit.
        finished = sim.step(
            &mut current,
            start + Duration::from_millis(400) + Duration::from_millis(sim.estimated_ms),
        );
        assert!(finished);
        assert!((current - 300.0).abs() < 1.0, "{current}");
    }

    /// An undamped spring (ζ = 0) never finishes.
    #[test]
    fn undamped_never_finishes() {
        let mut sim = PhysicalSpringToLimit::new(
            0.0,
            test_limit_property(100.0),
            PhysicalSpringParameters::new(0.0, 380.0, 1.0, 0.0),
        );
        let start = sim.anchor_tick;
        let mut current = 0.0f32;
        let finished = sim.step(&mut current, start + Duration::from_secs(60));
        assert!(!finished);
        // Still oscillating around the limit.
        assert!((current - 100.0).abs() < 100.5);
    }
}
