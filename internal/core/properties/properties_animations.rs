// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore segs

use super::*;
use crate::{
    animations::simulations::{
        Parameter, Simulation,
        spring::{SpringDurationBounceParameters, SpringParameters, SpringRegime},
        spring_estimation,
    },
    items::{AnimationDirection, PropertyAnimation},
    lengths::LogicalLength,
};
use alloc::vec::Vec;
use euclid::Length;
#[cfg(not(feature = "std"))]
use num_traits::Float;

enum AnimationState {
    /// The animation will start after the delay is finished
    Delaying,
    /// Actual animation
    Animating {
        current_iteration: u64,
    },
    Done {
        iteration_count: u64,
    },
}

pub(super) struct PropertyPhysicsAnimationData<S> {
    simulation: S,
    state: AnimationState,
    /// Tick at which the simulation was installed: simulations remember their own
    /// start time, so the global duration scale is applied by feeding the simulation
    /// a tick stretched relative to this anchor.
    installed_at: crate::animations::Instant,
}

impl<S> PropertyPhysicsAnimationData<S>
where
    S: Simulation,
{
    pub fn new(simulation: S) -> PropertyPhysicsAnimationData<S> {
        PropertyPhysicsAnimationData {
            simulation,
            state: AnimationState::Delaying,
            installed_at: crate::animations::current_tick(),
        }
    }

    /// Single iteration of the animation
    pub fn update_value(&mut self, target: &mut crate::Coord) -> bool {
        match self.state {
            AnimationState::Delaying => {
                // Decide on next state:
                self.state = AnimationState::Animating { current_iteration: 0 };
                self.update_value(target)
            }
            AnimationState::Animating { current_iteration: _ } => {
                // TODO: Pass in Coord directly?
                let mut value: f32 = *target as f32;
                let tick = crate::animations::current_tick();
                let scale = crate::animations::duration_scale();
                let tick = if scale <= 0.0 {
                    crate::animations::Instant(u64::MAX)
                } else {
                    crate::animations::Instant(self.installed_at.0.saturating_add(
                        (tick.0.saturating_sub(self.installed_at.0) as f64 / scale as f64) as u64,
                    ))
                };
                let finished = self.simulation.step(&mut value, tick);
                *target = value as crate::Coord;
                if finished {
                    self.state = AnimationState::Done { iteration_count: 0 };
                    true
                } else {
                    false
                }
            }
            AnimationState::Done { iteration_count: _ } => true,
        }
    }
}

pub(super) struct PropertyValueAnimationData<T> {
    from_value: T,
    to_value: Option<T>,
    details: PropertyAnimation,
    start_time: crate::animations::Instant,
    state: AnimationState,
    /// Applied to every interpolated value before it is stored. Lets a
    /// type-erased property (the interpreter's `Property<Value>`) reproduce
    /// the interpolation of the erased type, e.g. rounding for `int`.
    map: Option<fn(T) -> T>,
    spring: Option<PropertySpring>,
    /// Whether the final iteration's spring has already been re-damped
    spring_settle_clamped: bool,
    /// Set while an infinite animation (`iteration_count < 0`) is parked at its
    /// end value because the duration scale is 0 — Compose's
    /// `InfiniteTransition` `skipToEnd()` + suspend, which stops requesting
    /// frames instead of spinning at the end value. The `duration_scale` read
    /// inside `compute_interpolated_value` keeps the binding registered as a
    /// dependent of the scale property, so a later scale change wakes it.
    suspended_at_zero: bool,
}

/// How a spring animates the value: the normalized legacy `spring(bounce)` or a
/// per-channel physical `spring(damping_ratio, stiffness[, mass])`.
enum PropertySpring {
    /// `easing: spring(bounce)`: a normalized spring over the whole value, driven
    /// by the animation's `duration`.
    DurationBounce(SpringRegime),
    /// `easing: spring(damping_ratio, stiffness[, mass])`: an independent spring per
    /// animation channel, running until it settles — the `VectorizedSpringSpec`/
    /// `SpringSimulation` model of androidx.compose.animation.core.
    Physical(PhysicalSpring),
}

struct PhysicalSpring {
    /// One regime per channel: `evaluate(t)` returns `(channel - to_channel, velocity)`.
    regimes: Vec<SpringRegime>,
    /// `to_value` decomposed into the same channels.
    to_channels: Vec<f32>,
    /// Estimated time in milliseconds after which every channel has settled: the
    /// maximum over the channels of [`spring_estimation::estimate_animation_duration_ms_with_mass`].
    /// Effectively infinite (`u64::MAX`) when a channel never settles
    /// (`damping_ratio == 0`).
    duration_ms: u64,
}

impl<T: InterpolatedPropertyValue + Clone> PropertyValueAnimationData<T> {
    pub fn new(from_value: T, to_value: Option<T>, details: PropertyAnimation) -> Self {
        Self::new_with_velocity(from_value, to_value, details, Vec::new())
    }

    /// Used to carry velocity over across a retarget: `carried_velocity` holds one
    /// velocity per channel of the outgoing animation (empty when there was none).
    /// Per channel, [`PropertyAnimation::initial_velocity`] applies where no velocity
    /// is carried over.
    pub fn new_with_velocity(
        from_value: T,
        to_value: Option<T>,
        details: PropertyAnimation,
        carried_velocity: Vec<f32>,
    ) -> Self {
        let start_time = crate::animations::current_tick();
        let spring = Self::compute_spring(&details, &from_value, &to_value, &carried_velocity);
        Self {
            from_value,
            to_value,
            details,
            start_time,
            state: AnimationState::Delaying,
            map: None,
            spring,
            spring_settle_clamped: false,
            suspended_at_zero: false,
        }
    }

    /// A spring with duration <= 0 (and no mass/stiffness/damping override) can't be simulated
    fn compute_spring(
        details: &PropertyAnimation,
        from_value: &T,
        to_value: &Option<T>,
        carried_velocity: &[f32],
    ) -> Option<PropertySpring> {
        match details.easing {
            crate::animations::EasingCurve::Spring(bounce) => {
                let (w_n, zeta) = if details.duration > 0 {
                    Some(
                        SpringDurationBounceParameters::new(
                            details.duration as f32 / 1000.0,
                            bounce,
                        )
                        .to_natural_frequency_and_damping_ratio(),
                    )
                } else {
                    None
                }?;

                // -1 so that the spring knows to go to 0; re-express the carried-over velocity
                // (in property units/sec) in the spring's -1..=0-relative units.
                let delta = to_value.as_ref().map_or(0.0, |tv| from_value.scalar_delta(tv));
                let v0 = if delta != 0.0 {
                    carried_velocity.first().copied().unwrap_or(0.0) / delta
                } else {
                    0.0
                };
                Some(PropertySpring::DurationBounce(SpringRegime::new(-1.0, v0, w_n, zeta)))
            }
            crate::animations::EasingCurve::PhysicalSpring { damping_ratio, stiffness, mass } => {
                let to_value = to_value.as_ref()?;
                let channel_count = from_value.channel_count(to_value);
                if channel_count == 0 {
                    return None;
                }
                // Invalid parameters can't be simulated: the compiler diagnoses
                // literals, so clamp what only a runtime value can produce — the
                // same minimums as `PhysicalSpringParameters::new` (the Flickable
                // fling path).
                let params = crate::animations::simulations::spring::PhysicalSpringParameters::new(
                    damping_ratio,
                    stiffness,
                    mass,
                    0.,
                );
                let mass = f64::from(params.mass);
                let stiffness = f64::from(params.stiffness);
                let damping_ratio = params.damping_ratio;
                // `SpringSimulation` fixes the mass at 1, so `w_n = sqrt(k / m)`
                // covers the optional mass parameter.
                let w_n = f32::sqrt(stiffness as f32 / mass as f32);
                let damping_coefficient =
                    2.0 * f64::from(damping_ratio) * f64::sqrt(stiffness * mass);
                // An explicit override (the interpreter sets it for integer-typed
                // properties, where `Value::Number` erases the declared type)
                // wins over the animated type's default.
                let threshold = if details.visibility_threshold > 0.0 {
                    f64::from(details.visibility_threshold)
                } else {
                    f64::from(from_value.visibility_threshold(to_value))
                };

                let mut from_channels = alloc::vec![0.0; channel_count];
                from_value.write_start_channels(to_value, &mut from_channels);
                let mut to_channels = alloc::vec![0.0; channel_count];
                to_value.write_target_channels(from_value, &mut to_channels);
                // A velocity carried over from an animation with a different channel
                // count keeps the channels that line up; the rest start at 0.
                let carried = if carried_velocity.is_empty() {
                    None
                } else {
                    let mut v = alloc::vec![0.0f32; channel_count];
                    let n = carried_velocity.len().min(channel_count);
                    v[..n].copy_from_slice(&carried_velocity[..n]);
                    Some(v)
                };
                let carried = carried.as_deref();

                // `estimate_animation_duration_ms_with_mass` takes the
                // under-damped branch for `damping_ratio == 0` and yields an
                // estimate of 0 or worse — but a spring without damping never
                // settles, which Compose's `(stiffness, dampingRatio)` overload
                // reports as its maximum duration.
                let mut duration_ms = if damping_ratio == 0.0 { u64::MAX } else { 0 };
                let regimes = (0..channel_count)
                    .map(|i| {
                        let displacement = from_channels[i] - to_channels[i];
                        let v0 = carried.map(|c| c[i]).unwrap_or(details.initial_velocity);
                        duration_ms = duration_ms.max(
                            spring_estimation::estimate_animation_duration_ms_with_mass(
                                stiffness,
                                damping_coefficient,
                                mass,
                                f64::from(v0) / threshold,
                                f64::from(displacement) / threshold,
                                1.0,
                            ),
                        );
                        SpringRegime::new(displacement, v0, w_n, damping_ratio)
                    })
                    .collect();
                Some(PropertySpring::Physical(PhysicalSpring { regimes, to_channels, duration_ms }))
            }
            _ => None,
        }
    }

    pub fn with_map(mut self, map: fn(T) -> T) -> Self {
        self.map = Some(map);
        self
    }

    fn apply_map(&self, value: T) -> T {
        match self.map {
            Some(map) => map(value),
            None => value,
        }
    }

    /// The current velocity (one value per channel, in channel units per scaled
    /// second) of a live spring animation.
    ///
    /// Also answers for a binding that was installed but not evaluated yet
    /// (`Delaying`), so a `set_animated_value` retarget made before the next
    /// frame still carries the outgoing spring's velocity: its regime was
    /// computed at install and can be sampled analytically. During the `delay`
    /// window the spring hasn't started — like Compose's
    /// `SuspendAnimation.getVelocityVectorFromNanos`, which clamps `playTimeNanos`
    /// at 0, the regime's initial velocity is reported.
    fn current_channel_velocities(&self) -> Option<Vec<f32>> {
        if matches!(self.state, AnimationState::Done { .. }) {
            return None;
        }
        let spring = self.spring.as_ref()?;
        let elapsed_secs = if matches!(self.state, AnimationState::Delaying) {
            let delay_secs = self.details.delay.max(0) as f32 / 1000.0;
            (self.scaled_elapsed_secs(crate::animations::current_tick()) - delay_secs).max(0.0)
        } else {
            self.scaled_elapsed_secs(crate::animations::current_tick())
        };
        let to_value = self.to_value.as_ref().expect("The animation should have a to_value");
        match spring {
            PropertySpring::DurationBounce(spring) => {
                let (_, rel_vel) = spring.evaluate(elapsed_secs);
                Some(alloc::vec![rel_vel * self.from_value.scalar_delta(to_value)])
            }
            PropertySpring::Physical(physical) => Some(
                physical.regimes.iter().map(|regime| regime.evaluate(elapsed_secs).1).collect(),
            ),
        }
    }

    /// Elapsed time since `start_time`, with the global duration scale applied: at
    /// scale `0` an animation's progress and a spring's velocity are read as if
    /// infinitely much time had passed.
    fn scaled_elapsed_secs(&self, new_tick: crate::animations::Instant) -> f32 {
        let scale = crate::animations::duration_scale();
        if scale <= 0.0 {
            return f32::MAX / 1000.0;
        }
        (new_tick.duration_since(self.start_time).as_millis() as f64 / f64::from(scale)) as f32
            / 1000.0
    }

    /// Single iteration of the animation
    pub fn compute_interpolated_value(&mut self) -> (T, bool) {
        // If animation is disabled, immediately return the target value
        let to_value = self.to_value.clone().expect("The animation should have a to_value");
        if !self.details.enabled {
            return (self.apply_map(to_value), true);
        }

        let new_tick = crate::animations::current_tick();
        // The duration scale multiplies durations; scale `0` makes every animation
        // finish immediately, which `u64::MAX` progress expresses for all the
        // comparisons below.
        let scale = crate::animations::duration_scale();
        let mut time_progress = if scale <= 0.0 {
            u64::MAX
        } else {
            (new_tick.duration_since(self.start_time).as_millis() as f64 / f64::from(scale)) as u64
        };
        let reversed = |iteration: u64| -> bool {
            #[allow(clippy::manual_is_multiple_of)] // keep symmetry
            match self.details.direction {
                AnimationDirection::Normal => false,
                AnimationDirection::Reverse => true,
                AnimationDirection::Alternate => iteration % 2 == 1,
                AnimationDirection::AlternateReverse => iteration % 2 == 0,
            }
        };

        // Compose `InfiniteTransition.run` (pin …:182-194): at duration scale 0
        // every animation is `skipToEnd()`ed and the frame loop then suspends
        // until the scale rises above 0. The binding stays installed (the caller
        // sees `suspended_at_zero` and requests no frames) — the scale property
        // read above keeps it subscribed, so it wakes on the next change. On
        // wake-up Compose also `reset()`s the animations, so restart from 0.
        if self.details.iteration_count < 0. {
            if scale <= 0.0 {
                self.suspended_at_zero = true;
                return (self.apply_map(to_value), false);
            }
            if core::mem::take(&mut self.suspended_at_zero) {
                self.start_time = new_tick;
                self.state = AnimationState::Animating { current_iteration: 0 };
                time_progress = 0;
            }
        }

        match self.state {
            AnimationState::Delaying => {
                // At duration scale 0 the delay has already elapsed: like Compose
                // (`SuspendAnimation.kt`, `playTimeNanos = durationNanos` when
                // `scaleFactor == 0`), the animation proceeds straight to its end.
                if self.details.delay <= 0 || scale <= 0.0 {
                    self.state = AnimationState::Animating { current_iteration: 0 };
                    return self.compute_interpolated_value();
                }

                let delay = self.details.delay as u64;

                if time_progress < delay {
                    if reversed(0) {
                        (self.apply_map(to_value), false)
                    } else {
                        (self.apply_map(self.from_value.clone()), false)
                    }
                } else {
                    self.start_time = crate::animations::Instant(
                        new_tick.0.saturating_sub(time_progress - delay),
                    );

                    // Decide on next state:
                    self.state = AnimationState::Animating { current_iteration: 0 };
                    self.compute_interpolated_value()
                }
            }
            AnimationState::Animating { current_iteration } => {
                // A physical spring runs in (scaled) real time and ends only once its
                // channels are estimated to have settled: `duration`, `direction` and
                // `iteration_count` don't apply to it.
                if let Some(PropertySpring::Physical(physical)) = self.spring.as_ref() {
                    if self.details.iteration_count == 0. || time_progress >= physical.duration_ms {
                        // Springs ignore `direction`, so the end state is always the
                        // target value rather than `reversed()`-mapped back to `from`.
                        self.state = AnimationState::Done { iteration_count: 0 };
                        return (self.apply_map(to_value), true);
                    }
                    let elapsed_secs = time_progress as f32 / 1000.0;
                    let channels: Vec<f32> = physical
                        .regimes
                        .iter()
                        .zip(physical.to_channels.iter())
                        .map(|(regime, to)| to + regime.evaluate(elapsed_secs).0)
                        .collect();
                    let val = self.from_value.rebuild_from_channels(&to_value, &channels);
                    return (self.apply_map(val), false);
                }
                // A spring runs in real time and ends only once it settles.
                if matches!(self.details.easing, crate::animations::EasingCurve::Spring(_)) {
                    if self.details.iteration_count == 0. {
                        self.state = AnimationState::Done { iteration_count: 0 };
                        return self.compute_interpolated_value();
                    }
                    return if let Some(PropertySpring::DurationBounce(spring)) =
                        self.spring.as_ref()
                    {
                        if scale <= 0.0 {
                            // Duration scale 0 on a finite animation: report the
                            // settled end-of-iteration value directly. (Infinite
                            // ones are suspended before this match.)
                            self.state = AnimationState::Done {
                                iteration_count: (self.details.iteration_count.ceil() as u64)
                                    .saturating_sub(1),
                            };
                            return self.compute_interpolated_value();
                        }
                        let next_iteration = current_iteration + 1;
                        let has_more_iterations = self.details.iteration_count < 0.
                            || (next_iteration as f64) < self.details.iteration_count as f64;
                        let duration_ms = self.details.duration as u64;

                        if has_more_iterations && time_progress >= duration_ms {
                            // Bounce into the next iteration at `duration` rather than waiting to
                            // settle. Velocity always carries over; position only carries over on
                            // a direction flip (e.g. `alternate`), to stay continuous instead of
                            // snapping. Otherwise it resets to the start, like a repeating easing
                            // curve.
                            let duration_secs = duration_ms as f32 / 1000.0;
                            let (rel_pos, rel_vel) = spring.evaluate(duration_secs);
                            let crate::animations::EasingCurve::Spring(bounce) =
                                self.details.easing
                            else {
                                unreachable!()
                            };
                            let (w_n, zeta) =
                                SpringDurationBounceParameters::new(duration_secs, bounce)
                                    .to_natural_frequency_and_damping_ratio();
                            let x0 = if reversed(current_iteration) != reversed(next_iteration) {
                                -(1.0 + rel_pos)
                            } else {
                                -1.0
                            };
                            self.spring = Some(PropertySpring::DurationBounce(SpringRegime::new(
                                x0, rel_vel, w_n, zeta,
                            )));
                            self.start_time += core::time::Duration::from_millis(duration_ms);
                            self.state =
                                AnimationState::Animating { current_iteration: next_iteration };
                            self.compute_interpolated_value()
                        } else {
                            let elapsed_secs = time_progress as f32 / 1000.0;
                            let (t, settled) =
                                crate::animations::spring_settle_progress(spring, elapsed_secs);
                            if !settled
                                && !self.spring_settle_clamped
                                && time_progress >= duration_ms
                            {
                                // Hasn't settled by the end of `duration`: re-damp the tail (see
                                // `spring_settle_within`) so it's guaranteed to settle within a
                                // further 9 multiples of `duration`, carrying over position/velocity.
                                self.spring_settle_clamped = true;
                                let duration_secs = duration_ms as f32 / 1000.0;
                                let w_n = 2.0 * core::f32::consts::PI / duration_secs;
                                let settled_regime = crate::animations::spring_settle_within(
                                    spring,
                                    duration_secs,
                                    w_n,
                                );
                                self.spring = Some(PropertySpring::DurationBounce(settled_regime));
                                self.start_time += core::time::Duration::from_millis(duration_ms);
                                return self.compute_interpolated_value();
                            }
                            if settled {
                                self.state = if has_more_iterations {
                                    self.start_time = new_tick;
                                    AnimationState::Animating { current_iteration: next_iteration }
                                } else {
                                    AnimationState::Done { iteration_count: current_iteration }
                                };
                                self.compute_interpolated_value()
                            } else {
                                let progress = if reversed(current_iteration) { 1. - t } else { t };
                                let val = self.from_value.interpolate(&to_value, progress);
                                (self.apply_map(val), false)
                            }
                        }
                    } else {
                        self.state = AnimationState::Done { iteration_count: 0 };
                        self.compute_interpolated_value()
                    };
                }
                let mut current_iteration = current_iteration;

                if self.details.duration <= 0 || self.details.iteration_count == 0. {
                    self.state = AnimationState::Done { iteration_count: 0 };
                    return self.compute_interpolated_value();
                }

                let duration = self.details.duration as u64;
                if time_progress >= duration {
                    if scale <= 0.0 {
                        // Finite animations snap to and finish at the end of
                        // their last iteration, like Compose's
                        // `playTimeNanos = durationNanos` at scale 0 in
                        // `SuspendAnimation.kt`. (Infinite ones suspend before
                        // this match.)
                        self.state = AnimationState::Done {
                            iteration_count: (self.details.iteration_count.ceil() as u64)
                                .saturating_sub(1),
                        };
                        return self.compute_interpolated_value();
                    } else {
                        // wrap around
                        current_iteration =
                            current_iteration.saturating_add(time_progress / duration);
                        time_progress %= duration;
                        self.start_time =
                            crate::animations::Instant(new_tick.0.saturating_sub(time_progress));
                    }
                }

                if (self.details.iteration_count < 0.)
                    || ((current_iteration as f64 * duration as f64) + time_progress as f64)
                        < (self.details.iteration_count as f64 * duration as f64)
                {
                    self.state = AnimationState::Animating { current_iteration };

                    let progress = {
                        let progress =
                            (time_progress as f32 / self.details.duration as f32).clamp(0., 1.);
                        if reversed(current_iteration) { 1. - progress } else { progress }
                    };
                    let t = crate::animations::easing_curve(&self.details.easing, progress);
                    let val = self.from_value.interpolate(&to_value, t);

                    (self.apply_map(val), false)
                } else {
                    self.state =
                        AnimationState::Done { iteration_count: current_iteration.max(1) - 1 };
                    self.compute_interpolated_value()
                }
            }
            AnimationState::Done { iteration_count } => {
                if reversed(iteration_count) {
                    (self.apply_map(self.from_value.clone()), true)
                } else {
                    (self.apply_map(to_value), true)
                }
            }
        }
    }
}

#[derive(Clone, Copy, Eq, PartialEq, Debug)]
pub(super) enum AnimatedBindingState {
    Animating,
    NotAnimating,
    ShouldStart,
}

#[pin_project::pin_project]
pub(super) struct AnimatedBindingCallable<T, A> {
    #[pin]
    pub(super) original_binding: PropertyHandle,
    pub(super) state: Cell<AnimatedBindingState>,
    pub(super) animation_data: RefCell<PropertyValueAnimationData<T>>,
    pub(super) compute_animation_details: A,
    /// Tick captured by `mark_dirty`
    pub(super) dirty_time: Cell<crate::animations::Instant>,
    /// Velocity of the interrupted animation, one value per channel; refreshed by
    /// `mark_dirty` whenever the animated value is retargeted mid-flight.
    pub(crate) carried_velocity: RefCell<Vec<f32>>,
}

pub(super) type AnimationDetail = (PropertyAnimation, Option<crate::animations::Instant>);

impl<T: InterpolatedPropertyValue + Clone, A: Fn() -> AnimationDetail> BindingCallable<T>
    for AnimatedBindingCallable<T, A>
{
    fn evaluate(self: Pin<&Self>, value: &mut T) -> BindingResult {
        let original_binding = self.project_ref().original_binding;
        original_binding.register_as_dependency_to_current_binding(
            #[cfg(slint_debug_property)]
            "<AnimatedBindingCallable>",
        );
        match self.state.get() {
            AnimatedBindingState::Animating => {
                let mut animation_data = self.animation_data.borrow_mut();
                let (val, finished) = animation_data.compute_interpolated_value();
                let suspended = animation_data.suspended_at_zero;
                *value = val;
                if finished {
                    self.state.set(AnimatedBindingState::NotAnimating)
                } else if !suspended {
                    crate::animations::CURRENT_ANIMATION_DRIVER
                        .with(|driver| driver.set_has_active_animations());
                }
            }
            AnimatedBindingState::NotAnimating => {
                // Safety: `value` is a valid mutable reference
                unsafe { self.original_binding.update(value as *mut T) };
            }
            AnimatedBindingState::ShouldStart => {
                let mut animation_data = self.animation_data.borrow_mut();

                // Since `mark_dirty` fires when dependencies of `original_binding` changes
                // if the change doesn't actually affect the computed value, it shouldn't restart
                // the animation
                let previous_to_value = animation_data.to_value.clone();
                let mut new_to_value = T::default();
                // Safety: `new_to_value` is a valid mutable reference matching the
                // original binding's value type
                unsafe { self.original_binding.update(&mut new_to_value as *mut T) };
                animation_data.to_value = Some(new_to_value);

                if animation_data.to_value != previous_to_value {
                    animation_data.state = AnimationState::Delaying;
                    // Anchor timing to when the change was first signalled
                    animation_data.start_time = self.dirty_time.get();
                    // animation_data.details.iteration_count = 1.;
                    animation_data.from_value = value.clone();
                    let (details, start_time) = (self.compute_animation_details)();
                    if let Some(start_time) = start_time {
                        animation_data.start_time = start_time;
                    }
                    animation_data.details = details;
                    animation_data.spring = PropertyValueAnimationData::<T>::compute_spring(
                        &animation_data.details,
                        &animation_data.from_value,
                        &animation_data.to_value,
                        &self.carried_velocity.take(),
                    );
                    animation_data.spring_settle_clamped = false;
                }

                self.state.set(AnimatedBindingState::Animating);
                let (val, finished) = animation_data.compute_interpolated_value();
                let suspended = animation_data.suspended_at_zero;
                *value = val;
                if finished {
                    self.state.set(AnimatedBindingState::NotAnimating)
                } else if !suspended {
                    crate::animations::CURRENT_ANIMATION_DRIVER
                        .with(|driver| driver.set_has_active_animations());
                }
            }
        };
        BindingResult::KeepBinding
    }
    fn mark_dirty(self: Pin<&Self>) {
        if self.state.get() == AnimatedBindingState::ShouldStart {
            return;
        }
        let original_dirty = self.original_binding.access(|b| b.unwrap().dirty.get());
        // A suspended infinite animation is woken by a `duration_scale` change —
        // the dependency that dirtied this binding then — which never touches
        // the original binding.
        if original_dirty || self.animation_data.borrow().suspended_at_zero {
            *self.carried_velocity.borrow_mut() =
                self.animation_data.borrow().current_channel_velocities().unwrap_or_default();
            self.state.set(AnimatedBindingState::ShouldStart);
            self.dirty_time.set(crate::animations::current_tick());
        }
    }

    fn velocity(self: Pin<&Self>) -> Option<Vec<f32>> {
        self.animation_data.borrow().current_channel_velocities()
    }

    fn declared_animation(self: Pin<&Self>) -> Option<PropertyAnimation> {
        Some((self.compute_animation_details)().0)
    }
}

/// InterpolatedPropertyValue is a trait used to enable properties to be used with
/// animations that interpolate values. The basic requirement is the ability to apply
/// a progress that's typically between 0 and 1 to a range.
pub trait InterpolatedPropertyValue: PartialEq + Default + 'static {
    /// Returns the interpolated value between self and target_value according to the
    /// progress parameter t that's usually between 0 and 1. With certain animation
    /// easing curves it may over- or undershoot though.
    #[must_use]
    fn interpolate(&self, target_value: &Self, t: f32) -> Self;

    /// Returns `target_value - self` as a scalar.
    /// Types with no natural single-scalar notion of velocity keep the default `0.0`
    fn scalar_delta(&self, _target_value: &Self) -> f32 {
        0.0
    }

    /// The number of independent channels a physical `spring(damping_ratio, stiffness)`
    /// animates between `self` and `target_value`: one for scalars, four (Oklab +
    /// alpha) for colors, and the gradient's channels for brushes. The layout is a
    /// property of the `(self, target_value)` pair, not of either value alone, so all
    /// channel methods take the pair's other end. `0` for values a spring can't
    /// decompose, which makes them snap.
    fn channel_count(&self, target_value: &Self) -> usize {
        let _ = target_value;
        1
    }

    /// Write `self`'s channel values into `out` (of length
    /// [`channel_count`](Self::channel_count)), in the `(self, target_value)` pair's
    /// layout.
    fn write_channels(&self, target_value: &Self, out: &mut [f32]) {
        debug_assert_eq!(out.len(), self.channel_count(target_value));
        out[0] = Self::default().scalar_delta(self);
    }

    /// [`write_channels`](Self::write_channels) for the endpoint that starts a
    /// spring. Directional channel layouts (e.g. morph progress, which runs
    /// 0 at the start to a displacement at the target) override this and
    /// [`write_target_channels`](Self::write_target_channels) together.
    fn write_start_channels(&self, target_value: &Self, out: &mut [f32]) {
        self.write_channels(target_value, out);
    }

    /// [`write_channels`](Self::write_channels) for the endpoint a spring
    /// animates towards.
    fn write_target_channels(&self, start_value: &Self, out: &mut [f32]) {
        self.write_channels(start_value, out);
    }

    /// Rebuild a value from channels in the `(self, target_value)` pair's layout:
    /// `self` contributes what isn't animated (e.g. a gradient's variant) while
    /// `target_value` resolves pair-dependent details (e.g. the longer side's stop
    /// count).
    fn rebuild_from_channels(&self, target_value: &Self, channels: &[f32]) -> Self {
        debug_assert_eq!(channels.len(), self.channel_count(target_value));
        let _ = (self, target_value);
        let mut value = Self::default();
        value.set_single_channel(channels[0]);
        value
    }

    /// For scalar single-channel types: rebuild `self` so that its one channel equals
    /// `channel`. Kept internal; multi-channel types override [`rebuild_from_channels`]
    /// instead.
    #[doc(hidden)]
    fn set_single_channel(&mut self, channel: f32) {
        let _ = channel;
    }

    /// The per-channel displacement at which a spring is considered settled — the
    /// `visibilityThreshold` of androidx.compose.animation.core:
    /// [`SPRING_DEFAULT_DISPLACEMENT_THRESHOLD`](crate::animations::SPRING_DEFAULT_DISPLACEMENT_THRESHOLD)
    /// by default and `1.0` for integer-typed properties.
    fn visibility_threshold(&self, _target_value: &Self) -> f32 {
        crate::animations::SPRING_DEFAULT_DISPLACEMENT_THRESHOLD
    }
}

impl InterpolatedPropertyValue for f32 {
    fn interpolate(&self, target_value: &Self, t: f32) -> Self {
        self + t * (target_value - self)
    }

    fn scalar_delta(&self, target_value: &Self) -> f32 {
        target_value - self
    }

    fn write_channels(&self, _target_value: &Self, out: &mut [f32]) {
        out[0] = *self;
    }

    fn set_single_channel(&mut self, channel: f32) {
        *self = channel;
    }
}

impl InterpolatedPropertyValue for i32 {
    fn interpolate(&self, target_value: &Self, t: f32) -> Self {
        self + (t * (target_value - self) as f32).round() as i32
    }

    fn scalar_delta(&self, target_value: &Self) -> f32 {
        (target_value - self) as f32
    }

    fn write_channels(&self, _target_value: &Self, out: &mut [f32]) {
        out[0] = *self as f32;
    }

    fn set_single_channel(&mut self, channel: f32) {
        *self = channel.round() as i32;
    }

    fn visibility_threshold(&self, _target_value: &Self) -> f32 {
        1.0
    }
}

impl InterpolatedPropertyValue for i64 {
    fn interpolate(&self, target_value: &Self, t: f32) -> Self {
        self + (t * (target_value - self) as f32).round() as Self
    }

    fn scalar_delta(&self, target_value: &Self) -> f32 {
        (target_value - self) as f32
    }

    fn write_channels(&self, _target_value: &Self, out: &mut [f32]) {
        out[0] = *self as f32;
    }

    fn set_single_channel(&mut self, channel: f32) {
        *self = channel.round() as i64;
    }

    fn visibility_threshold(&self, _target_value: &Self) -> f32 {
        1.0
    }
}

impl InterpolatedPropertyValue for u8 {
    fn interpolate(&self, target_value: &Self, t: f32) -> Self {
        ((*self as f32) + (t * ((*target_value as f32) - (*self as f32)))).round().clamp(0., 255.)
            as u8
    }

    fn scalar_delta(&self, target_value: &Self) -> f32 {
        (*target_value as f32) - (*self as f32)
    }

    fn write_channels(&self, _target_value: &Self, out: &mut [f32]) {
        out[0] = *self as f32;
    }

    fn set_single_channel(&mut self, channel: f32) {
        *self = channel.round().clamp(0., 255.) as u8;
    }

    fn visibility_threshold(&self, _target_value: &Self) -> f32 {
        1.0
    }
}

/// Two axis lists interpolate like CSS `font-variation-settings`: entry-wise
/// when both lists pair up (same length, same tag at each position). Lists
/// that don't pair up animate discretely, switching to the target halfway
/// through the progress like a CSS discrete animation.
impl InterpolatedPropertyValue for crate::model::ModelRc<crate::items::FontVariation> {
    fn interpolate(&self, target_value: &Self, t: f32) -> Self {
        use crate::model::Model as _;
        if self.row_count() != target_value.row_count() {
            return if t < 0.5 { self.clone() } else { target_value.clone() };
        }
        let mut rows = alloc::vec::Vec::with_capacity(self.row_count());
        for (from, to) in self.iter().zip(target_value.iter()) {
            if from.tag != to.tag {
                return if t < 0.5 { self.clone() } else { target_value.clone() };
            }
            rows.push(crate::items::FontVariation {
                tag: to.tag.clone(),
                value: from.value.interpolate(&to.value, t),
            });
        }
        crate::model::ModelRc::new(crate::model::VecModel::from(rows))
    }

    fn scalar_delta(&self, target_value: &Self) -> f32 {
        use crate::model::Model as _;
        let mut delta = (self.row_count() as f32 - target_value.row_count() as f32).abs();
        for (from, to) in self.iter().zip(target_value.iter()) {
            if from.tag == to.tag {
                delta += (to.value - from.value).abs();
            } else {
                delta += to.value.abs();
            }
        }
        delta
    }
}

/// Two axis lists interpolate like CSS `font-variation-settings`: entry-wise
/// when both lists pair up (same length, same tag at each position). Lists
/// that don't pair up animate discretely, switching to the target halfway
/// through the progress like a CSS discrete animation.
impl InterpolatedPropertyValue for crate::model::ModelRc<crate::items::FontVariation> {
    fn interpolate(&self, target_value: &Self, t: f32) -> Self {
        use crate::model::Model as _;
        if self.row_count() != target_value.row_count() {
            return if t < 0.5 { self.clone() } else { target_value.clone() };
        }
        let mut rows = alloc::vec::Vec::with_capacity(self.row_count());
        for (from, to) in self.iter().zip(target_value.iter()) {
            if from.tag != to.tag {
                return if t < 0.5 { self.clone() } else { target_value.clone() };
            }
            rows.push(crate::items::FontVariation {
                tag: to.tag.clone(),
                value: from.value.interpolate(&to.value, t),
            });
        }
        crate::model::ModelRc::new(crate::model::VecModel::from(rows))
    }

    fn scalar_delta(&self, target_value: &Self) -> f32 {
        use crate::model::Model as _;
        let mut delta = (self.row_count() as f32 - target_value.row_count() as f32).abs();
        for (from, to) in self.iter().zip(target_value.iter()) {
            if from.tag == to.tag {
                delta += (to.value - from.value).abs();
            } else {
                delta += to.value.abs();
            }
        }
        delta
    }
}

impl InterpolatedPropertyValue for LogicalLength {
    fn interpolate(&self, target_value: &Self, t: f32) -> Self {
        LogicalLength::new(self.get().interpolate(&target_value.get(), t))
    }

    fn scalar_delta(&self, target_value: &Self) -> f32 {
        (target_value.get() - self.get()) as f32
    }

    fn write_channels(&self, _target_value: &Self, out: &mut [f32]) {
        out[0] = self.get() as f32;
    }

    fn set_single_channel(&mut self, channel: f32) {
        *self = LogicalLength::new(channel as crate::Coord);
    }
}

/// Binding installed by `Property::set_animated_value` (and its C FFI
/// equivalent), a named type so a retarget can report the current velocity.
pub(crate) struct AnimatedValueBinding<T> {
    pub(super) animation_data: RefCell<PropertyValueAnimationData<T>>,
}

impl<T: InterpolatedPropertyValue + Clone + 'static> BindingCallable<T>
    for AnimatedValueBinding<T>
{
    fn evaluate(self: Pin<&Self>, value: &mut T) -> BindingResult {
        let mut animation_data = self.animation_data.borrow_mut();
        let (val, finished) = animation_data.compute_interpolated_value();
        let suspended = animation_data.suspended_at_zero;
        *value = val;
        if finished {
            BindingResult::RemoveBinding
        } else {
            if !suspended {
                crate::animations::CURRENT_ANIMATION_DRIVER
                    .with(|driver| driver.set_has_active_animations());
            }
            BindingResult::KeepBinding
        }
    }

    fn velocity(self: Pin<&Self>) -> Option<Vec<f32>> {
        self.animation_data.borrow().current_channel_velocities()
    }

    fn declared_animation(self: Pin<&Self>) -> Option<PropertyAnimation> {
        Some(self.animation_data.borrow().details.clone())
    }
}

impl<T: Clone + InterpolatedPropertyValue + 'static> Property<T> {
    /// Evaluate the property and remove the (animation) binding of this property.
    ///
    /// Note that a binding can intercept this via intercept_set_binding and still remain on the property.
    /// (e.g. two-way-bindings will not be removed with this call!)
    pub fn remove_binding(self: Pin<&Self>) {
        // FIXME: This is a bit of a hack, set_animated_value will call set_binding on the internal handle,
        // which will call intercept_set_binding, which will check if the binding should be removed or not.
        // In the case of two-way bindings, we want to keep the binding, but reset the value to the current one,
        // so that any animation binding is removed, but the two-way-binding is kept.
        self.set_animated_value(self.get(), PropertyAnimation::default());
    }

    /// Change the value of this property, by animating (interpolating) from the current property's value
    /// to the specified parameter value. The animation is done according to the parameters described by
    /// the PropertyAnimation object.
    ///
    /// If other properties have binding depending of this property, these properties will
    /// be marked as dirty.
    pub fn set_animated_value(self: Pin<&Self>, value: T, animation_data: PropertyAnimation) {
        self.set_animated_value_impl(value, animation_data, None)
    }

    /// Like [`Self::set_animated_value`], but passes every interpolated value through `map`
    /// before storing it, so a type-erased property can reproduce the interpolation
    /// of the erased type (e.g. rounding for `int` properties).
    pub fn set_animated_value_with_map(
        self: Pin<&Self>,
        value: T,
        animation_data: PropertyAnimation,
        map: fn(T) -> T,
    ) {
        self.set_animated_value_impl(value, animation_data, Some(map))
    }

    fn set_animated_value_impl(
        self: Pin<&Self>,
        value: T,
        animation_data: PropertyAnimation,
        map: Option<fn(T) -> T>,
    ) {
        // Carry over the outgoing binding's velocity
        let carried_velocity = self.handle.current_velocity().unwrap_or_default();
        let mut d = properties_animations::PropertyValueAnimationData::new_with_velocity(
            self.get(),
            Some(value),
            animation_data,
            carried_velocity,
        );
        if let Some(map) = map {
            d = d.with_map(map);
        }
        let binding =
            properties_animations::AnimatedValueBinding { animation_data: RefCell::new(d) };
        // Safety: the BindingCallable will cast its argument to T
        unsafe {
            self.handle.set_binding(
                binding,
                #[cfg(slint_debug_property)]
                self.debug_name.borrow().as_str(),
            );
        }
        self.handle.mark_dirty(
            #[cfg(slint_debug_property)]
            self.debug_name.borrow().as_str(),
        );
    }

    /// Set a binding to this property, providing a callback for the animation and an optional
    /// start_time (relevant for state transitions).
    pub fn set_animated_binding(
        &self,
        binding: impl Binding<T> + 'static,
        compute_animation_details: impl Fn() -> (PropertyAnimation, Option<crate::animations::Instant>)
        + 'static,
    ) {
        self.set_animated_binding_impl(binding, compute_animation_details, None)
    }

    /// Like [`Self::set_animated_binding`], but passes every interpolated value through `map`
    /// before storing it, so a type-erased property can reproduce the interpolation
    /// of the erased type (e.g. rounding for `int` properties).
    pub fn set_animated_binding_with_map(
        &self,
        binding: impl Binding<T> + 'static,
        compute_animation_details: impl Fn() -> (PropertyAnimation, Option<crate::animations::Instant>)
        + 'static,
        map: fn(T) -> T,
    ) {
        self.set_animated_binding_impl(binding, compute_animation_details, Some(map))
    }

    fn set_animated_binding_impl(
        &self,
        binding: impl Binding<T> + 'static,
        compute_animation_details: impl Fn() -> (PropertyAnimation, Option<crate::animations::Instant>)
        + 'static,
        map: Option<fn(T) -> T>,
    ) {
        let mut animation_data = properties_animations::PropertyValueAnimationData::new(
            T::default(),
            None,
            PropertyAnimation::default(),
        );
        if let Some(map) = map {
            animation_data = animation_data.with_map(map);
        }
        let binding_callable = properties_animations::AnimatedBindingCallable::<T, _> {
            original_binding: PropertyHandle {
                handle: Cell::new(
                    (alloc_binding_holder(move |val: &mut T| {
                        *val = binding.evaluate(val);
                        BindingResult::KeepBinding
                    }) as *mut ())
                        .map_addr(|a| a | 0b10),
                ),
            },
            state: Cell::new(properties_animations::AnimatedBindingState::NotAnimating),
            animation_data: RefCell::new(animation_data),
            compute_animation_details,
            dirty_time: Cell::new(crate::animations::current_tick()),
            carried_velocity: RefCell::new(Vec::new()),
        };

        // Safety: the `AnimatedBindingCallable`'s type match the property type
        unsafe {
            self.handle.set_binding(
                binding_callable,
                #[cfg(slint_debug_property)]
                self.debug_name.borrow().as_str(),
            )
        };
        self.handle.mark_dirty(
            #[cfg(slint_debug_property)]
            self.debug_name.borrow().as_str(),
        );
    }
}

impl<Unit, S: Simulation> BindingCallable<Length<crate::Coord, Unit>>
    for RefCell<PropertyPhysicsAnimationData<S>>
{
    fn evaluate(self: Pin<&Self>, value: &mut Length<crate::Coord, Unit>) -> BindingResult {
        let finished = self.borrow_mut().update_value(&mut value.0);
        if finished {
            BindingResult::RemoveBinding
        } else {
            crate::animations::CURRENT_ANIMATION_DRIVER
                .with(|driver| driver.set_has_active_animations());
            BindingResult::KeepBinding
        }
    }

    // This binding should not be removed if the value is updated externally.
    fn intercept_set(self: Pin<&Self>, _value: &Length<crate::Coord, Unit>) -> bool {
        true
    }
}

impl<Unit> Property<Length<crate::Coord, Unit>> {
    /// Change the value by using a physics animation
    pub fn set_physic_animation_value<S: Simulation + 'static, AD: Parameter<Output = S>>(
        &self,
        limit_value: Pin<Box<Property<f32>>>,
        simulation_data: AD,
    ) {
        // Safety: the BindingCallable will cast its argument to T
        unsafe {
            self.handle.set_binding::<Length<crate::Coord, Unit>, core::cell::RefCell<PropertyPhysicsAnimationData<S>>>(RefCell::new(PropertyPhysicsAnimationData::new(
                    simulation_data.simulation(self.get_internal().0 as f32, limit_value),
                )),
                #[cfg(slint_debug_property)]
                self.debug_name.borrow().as_str()
            );
        }
        self.handle.mark_dirty(
            #[cfg(slint_debug_property)]
            self.debug_name.borrow().as_str(),
        );
    }
}

#[cfg(test)]
mod animation_tests {
    use super::*;
    use pin_weak::rc::PinWeak;
    use std::rc::Rc;

    #[derive(Default)]
    struct Component {
        width: Property<i32>,
        width_times_two: Property<i32>,
        feed_property: Property<i32>, // used by binding to feed values into width
    }

    impl Component {
        fn new_test_component() -> Pin<Rc<Self>> {
            let compo = Rc::pin(Component::default());
            let w = PinWeak::downgrade(compo.clone());
            compo.width_times_two.set_binding(move || {
                let compo = w.upgrade().unwrap();
                get_prop_value(&compo.width) * 2
            });

            compo
        }
    }

    const DURATION: std::time::Duration = std::time::Duration::from_millis(10000);
    const DELAY: std::time::Duration = std::time::Duration::from_millis(800);

    // Helper just for testing
    fn get_prop_value<T: Clone>(prop: &Property<T>) -> T {
        unsafe { Pin::new_unchecked(prop).get() }
    }

    // Helper just for testing: the property lives in a pinned `Rc<Component>`.
    fn set_animated_value<T: Clone + InterpolatedPropertyValue + 'static>(
        prop: &Property<T>,
        value: T,
        animation_data: PropertyAnimation,
    ) {
        unsafe { Pin::new_unchecked(prop) }.set_animated_value(value, animation_data);
    }

    #[test]
    fn properties_test_animation_negative_delay_triggered_by_set() {
        let compo = Component::new_test_component();

        let animation_details = PropertyAnimation {
            delay: -25,
            duration: DURATION.as_millis() as _,
            iteration_count: 1.,
            ..PropertyAnimation::default()
        };

        compo.width.set(100);
        assert_eq!(get_prop_value(&compo.width), 100);
        assert_eq!(get_prop_value(&compo.width_times_two), 200);

        let start_time = crate::animations::current_tick();

        set_animated_value(&compo.width, 200, animation_details);
        assert_eq!(get_prop_value(&compo.width), 100);
        assert_eq!(get_prop_value(&compo.width_times_two), 200);

        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DURATION / 2));
        assert_eq!(get_prop_value(&compo.width), 150);
        assert_eq!(get_prop_value(&compo.width_times_two), 300);

        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DURATION));
        assert_eq!(get_prop_value(&compo.width), 200);
        assert_eq!(get_prop_value(&compo.width_times_two), 400);

        // Overshoot: Always to_value.
        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DURATION + DURATION / 2));
        assert_eq!(get_prop_value(&compo.width), 200);
        assert_eq!(get_prop_value(&compo.width_times_two), 400);

        // the binding should be removed
        compo.width.handle.access(|binding| assert!(binding.is_none()));
    }

    #[test]
    fn properties_test_animation_triggered_by_set() {
        let compo = Component::new_test_component();

        let animation_details = PropertyAnimation {
            duration: DURATION.as_millis() as _,
            iteration_count: 1.,
            ..PropertyAnimation::default()
        };

        compo.width.set(100);
        assert_eq!(get_prop_value(&compo.width), 100);
        assert_eq!(get_prop_value(&compo.width_times_two), 200);

        let start_time = crate::animations::current_tick();

        set_animated_value(&compo.width, 200, animation_details);
        assert_eq!(get_prop_value(&compo.width), 100);
        assert_eq!(get_prop_value(&compo.width_times_two), 200);

        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DURATION / 2));
        assert_eq!(get_prop_value(&compo.width), 150);
        assert_eq!(get_prop_value(&compo.width_times_two), 300);

        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DURATION));
        assert_eq!(get_prop_value(&compo.width), 200);
        assert_eq!(get_prop_value(&compo.width_times_two), 400);

        // Overshoot: Always to_value.
        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DURATION + DURATION / 2));
        assert_eq!(get_prop_value(&compo.width), 200);
        assert_eq!(get_prop_value(&compo.width_times_two), 400);

        // the binding should be removed
        compo.width.handle.access(|binding| assert!(binding.is_none()));
    }

    #[test]
    fn properties_test_delayed_animation_triggered_by_set() {
        let compo = Component::new_test_component();

        let animation_details = PropertyAnimation {
            delay: DELAY.as_millis() as _,
            iteration_count: 1.,
            duration: DURATION.as_millis() as _,
            ..PropertyAnimation::default()
        };

        compo.width.set(100);
        assert_eq!(get_prop_value(&compo.width), 100);
        assert_eq!(get_prop_value(&compo.width_times_two), 200);

        let start_time = crate::animations::current_tick();

        set_animated_value(&compo.width, 200, animation_details);
        assert_eq!(get_prop_value(&compo.width), 100);
        assert_eq!(get_prop_value(&compo.width_times_two), 200);

        // In delay:
        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DELAY / 2));
        assert_eq!(get_prop_value(&compo.width), 100);
        assert_eq!(get_prop_value(&compo.width_times_two), 200);

        // In animation:
        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DELAY));
        assert_eq!(get_prop_value(&compo.width), 100);
        assert_eq!(get_prop_value(&compo.width_times_two), 200);

        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DELAY + DURATION / 2));
        assert_eq!(get_prop_value(&compo.width), 150);
        assert_eq!(get_prop_value(&compo.width_times_two), 300);

        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DELAY + DURATION));
        assert_eq!(get_prop_value(&compo.width), 200);
        assert_eq!(get_prop_value(&compo.width_times_two), 400);

        // Overshoot: Always to_value.
        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DELAY + DURATION + DURATION / 2));
        assert_eq!(get_prop_value(&compo.width), 200);
        assert_eq!(get_prop_value(&compo.width_times_two), 400);

        // the binding should be removed
        compo.width.handle.access(|binding| assert!(binding.is_none()));
    }

    #[test]
    fn properties_test_delayed_animation_fractal_iteration_triggered_by_set() {
        let compo = Component::new_test_component();

        let animation_details = PropertyAnimation {
            delay: DELAY.as_millis() as _,
            iteration_count: 1.5,
            duration: DURATION.as_millis() as _,
            ..PropertyAnimation::default()
        };

        compo.width.set(100);
        assert_eq!(get_prop_value(&compo.width), 100);
        assert_eq!(get_prop_value(&compo.width_times_two), 200);

        let start_time = crate::animations::current_tick();

        set_animated_value(&compo.width, 200, animation_details);
        assert_eq!(get_prop_value(&compo.width), 100);
        assert_eq!(get_prop_value(&compo.width_times_two), 200);

        // In delay:
        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DELAY / 2));
        assert_eq!(get_prop_value(&compo.width), 100);
        assert_eq!(get_prop_value(&compo.width_times_two), 200);

        // In animation:
        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DELAY));
        assert_eq!(get_prop_value(&compo.width), 100);
        assert_eq!(get_prop_value(&compo.width_times_two), 200);

        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DELAY + DURATION / 2));
        assert_eq!(get_prop_value(&compo.width), 150);
        assert_eq!(get_prop_value(&compo.width_times_two), 300);

        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DELAY + DURATION));
        assert_eq!(get_prop_value(&compo.width), 100);
        assert_eq!(get_prop_value(&compo.width_times_two), 200);

        // (fractal) end of animation
        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DELAY + DURATION + DURATION / 4));
        assert_eq!(get_prop_value(&compo.width), 125);
        assert_eq!(get_prop_value(&compo.width_times_two), 250);

        // End of animation:
        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DELAY + DURATION + DURATION / 2));
        assert_eq!(get_prop_value(&compo.width), 200);
        assert_eq!(get_prop_value(&compo.width_times_two), 400);

        // the binding should be removed
        compo.width.handle.access(|binding| assert!(binding.is_none()));
    }
    #[test]
    fn properties_test_delayed_animation_null_duration_triggered_by_set() {
        let compo = Component::new_test_component();

        let animation_details = PropertyAnimation {
            delay: DELAY.as_millis() as _,
            iteration_count: 1.0,
            duration: 0,
            ..PropertyAnimation::default()
        };

        compo.width.set(100);
        assert_eq!(get_prop_value(&compo.width), 100);
        assert_eq!(get_prop_value(&compo.width_times_two), 200);

        let start_time = crate::animations::current_tick();

        set_animated_value(&compo.width, 200, animation_details);
        assert_eq!(get_prop_value(&compo.width), 100);
        assert_eq!(get_prop_value(&compo.width_times_two), 200);

        // In delay:
        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DELAY / 2));
        assert_eq!(get_prop_value(&compo.width), 100);
        assert_eq!(get_prop_value(&compo.width_times_two), 200);

        // No animation:
        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DELAY));
        assert_eq!(get_prop_value(&compo.width), 200);
        assert_eq!(get_prop_value(&compo.width_times_two), 400);

        // Overshoot: Always to_value.
        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DELAY + DURATION + DURATION / 2));
        assert_eq!(get_prop_value(&compo.width), 200);
        assert_eq!(get_prop_value(&compo.width_times_two), 400);

        // the binding should be removed
        compo.width.handle.access(|binding| assert!(binding.is_none()));
    }

    #[test]
    fn properties_test_delayed_animation_negative_duration_triggered_by_set() {
        let compo = Component::new_test_component();

        let animation_details = PropertyAnimation {
            delay: DELAY.as_millis() as _,
            iteration_count: 1.0,
            duration: -25,
            ..PropertyAnimation::default()
        };

        compo.width.set(100);
        assert_eq!(get_prop_value(&compo.width), 100);
        assert_eq!(get_prop_value(&compo.width_times_two), 200);

        let start_time = crate::animations::current_tick();

        set_animated_value(&compo.width, 200, animation_details);
        assert_eq!(get_prop_value(&compo.width), 100);
        assert_eq!(get_prop_value(&compo.width_times_two), 200);

        // In delay:
        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DELAY / 2));
        assert_eq!(get_prop_value(&compo.width), 100);
        assert_eq!(get_prop_value(&compo.width_times_two), 200);

        // No animation:
        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DELAY));
        assert_eq!(get_prop_value(&compo.width), 200);
        assert_eq!(get_prop_value(&compo.width_times_two), 400);

        // Overshoot: Always to_value.
        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DELAY + DURATION + DURATION / 2));
        assert_eq!(get_prop_value(&compo.width), 200);
        assert_eq!(get_prop_value(&compo.width_times_two), 400);

        // the binding should be removed
        compo.width.handle.access(|binding| assert!(binding.is_none()));
    }

    #[test]
    fn properties_test_delayed_animation_no_iteration_triggered_by_set() {
        let compo = Component::new_test_component();

        let animation_details = PropertyAnimation {
            delay: DELAY.as_millis() as _,
            iteration_count: 0.0,
            duration: DURATION.as_millis() as _,
            ..PropertyAnimation::default()
        };

        compo.width.set(100);
        assert_eq!(get_prop_value(&compo.width), 100);
        assert_eq!(get_prop_value(&compo.width_times_two), 200);

        let start_time = crate::animations::current_tick();

        set_animated_value(&compo.width, 200, animation_details);
        assert_eq!(get_prop_value(&compo.width), 100);
        assert_eq!(get_prop_value(&compo.width_times_two), 200);

        // In delay:
        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DELAY / 2));
        assert_eq!(get_prop_value(&compo.width), 100);
        assert_eq!(get_prop_value(&compo.width_times_two), 200);

        // No animation:
        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DELAY));
        assert_eq!(get_prop_value(&compo.width), 200);
        assert_eq!(get_prop_value(&compo.width_times_two), 400);

        // Overshoot: Always to_value.
        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DELAY + DURATION + DURATION / 2));
        assert_eq!(get_prop_value(&compo.width), 200);
        assert_eq!(get_prop_value(&compo.width_times_two), 400);

        // the binding should be removed
        compo.width.handle.access(|binding| assert!(binding.is_none()));
    }

    #[test]
    fn properties_test_delayed_animation_negative_iteration_triggered_by_set() {
        let compo = Component::new_test_component();

        let animation_details = PropertyAnimation {
            delay: DELAY.as_millis() as _,
            iteration_count: -42., // loop forever!
            duration: DURATION.as_millis() as _,
            ..PropertyAnimation::default()
        };

        compo.width.set(100);
        assert_eq!(get_prop_value(&compo.width), 100);
        assert_eq!(get_prop_value(&compo.width_times_two), 200);

        let start_time = crate::animations::current_tick();

        set_animated_value(&compo.width, 200, animation_details);
        assert_eq!(get_prop_value(&compo.width), 100);
        assert_eq!(get_prop_value(&compo.width_times_two), 200);

        // In delay:
        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DELAY / 2));
        assert_eq!(get_prop_value(&compo.width), 100);
        assert_eq!(get_prop_value(&compo.width_times_two), 200);

        // In animation:
        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DELAY));
        assert_eq!(get_prop_value(&compo.width), 100);
        assert_eq!(get_prop_value(&compo.width_times_two), 200);

        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DELAY + DURATION / 2));
        assert_eq!(get_prop_value(&compo.width), 150);
        assert_eq!(get_prop_value(&compo.width_times_two), 300);

        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DELAY + DURATION));
        assert_eq!(get_prop_value(&compo.width), 100);
        assert_eq!(get_prop_value(&compo.width_times_two), 200);

        // In animation (again):
        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DELAY + 500 * DURATION));
        assert_eq!(get_prop_value(&compo.width), 100);
        assert_eq!(get_prop_value(&compo.width_times_two), 200);

        crate::animations::CURRENT_ANIMATION_DRIVER.with(|driver| {
            driver.update_animations(start_time + DELAY + 50000 * DURATION + DURATION / 2)
        });
        assert_eq!(get_prop_value(&compo.width), 150);
        assert_eq!(get_prop_value(&compo.width_times_two), 300);

        // the binding should not be removed as it is still animating!
        compo.width.handle.access(|binding| assert!(binding.is_some()));
    }

    #[test]
    fn properties_test_animation_direction_triggered_by_set() {
        let compo = Component::new_test_component();

        let animation_details = PropertyAnimation {
            delay: -25,
            duration: DURATION.as_millis() as _,
            direction: AnimationDirection::AlternateReverse,
            iteration_count: 1.,
            ..PropertyAnimation::default()
        };

        compo.width.set(100);
        assert_eq!(get_prop_value(&compo.width), 100);
        assert_eq!(get_prop_value(&compo.width_times_two), 200);

        let start_time = crate::animations::current_tick();

        set_animated_value(&compo.width, 200, animation_details);
        assert_eq!(get_prop_value(&compo.width), 200);
        assert_eq!(get_prop_value(&compo.width_times_two), 400);

        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DURATION / 2));
        assert_eq!(get_prop_value(&compo.width), 150);
        assert_eq!(get_prop_value(&compo.width_times_two), 300);

        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DURATION));
        assert_eq!(get_prop_value(&compo.width), 100);
        assert_eq!(get_prop_value(&compo.width_times_two), 200);

        // Overshoot: Always from_value.
        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DURATION + DURATION / 2));
        assert_eq!(get_prop_value(&compo.width), 100);
        assert_eq!(get_prop_value(&compo.width_times_two), 200);

        // the binding should be removed
        compo.width.handle.access(|binding| assert!(binding.is_none()));
    }

    #[test]
    fn properties_test_animation_triggered_by_binding() {
        let compo = Component::new_test_component();

        let start_time = crate::animations::current_tick();

        let animation_details = PropertyAnimation {
            duration: DURATION.as_millis() as _,
            iteration_count: 1.,
            ..PropertyAnimation::default()
        };

        let w = PinWeak::downgrade(compo.clone());
        compo.width.set_animated_binding(
            move || {
                let compo = w.upgrade().unwrap();
                get_prop_value(&compo.feed_property)
            },
            move || (animation_details.clone(), None),
        );

        compo.feed_property.set(100);
        assert_eq!(get_prop_value(&compo.width), 100);
        assert_eq!(get_prop_value(&compo.width_times_two), 200);

        compo.feed_property.set(200);
        assert_eq!(get_prop_value(&compo.width), 100);
        assert_eq!(get_prop_value(&compo.width_times_two), 200);

        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DURATION / 2));
        assert_eq!(get_prop_value(&compo.width), 150);
        assert_eq!(get_prop_value(&compo.width_times_two), 300);

        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DURATION));
        assert_eq!(get_prop_value(&compo.width), 200);
        assert_eq!(get_prop_value(&compo.width_times_two), 400);
    }

    #[test]
    fn properties_test_delayed_animation_triggered_by_binding() {
        let compo = Component::new_test_component();

        let start_time = crate::animations::current_tick();

        let animation_details = PropertyAnimation {
            delay: DELAY.as_millis() as _,
            duration: DURATION.as_millis() as _,
            iteration_count: 1.0,
            ..PropertyAnimation::default()
        };

        let w = PinWeak::downgrade(compo.clone());
        compo.width.set_animated_binding(
            move || {
                let compo = w.upgrade().unwrap();
                get_prop_value(&compo.feed_property)
            },
            move || (animation_details.clone(), None),
        );

        compo.feed_property.set(100);
        assert_eq!(get_prop_value(&compo.width), 100);
        assert_eq!(get_prop_value(&compo.width_times_two), 200);

        compo.feed_property.set(200);
        assert_eq!(get_prop_value(&compo.width), 100);
        assert_eq!(get_prop_value(&compo.width_times_two), 200);

        // In delay:
        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DELAY / 2));
        assert_eq!(get_prop_value(&compo.width), 100);
        assert_eq!(get_prop_value(&compo.width_times_two), 200);

        // In animation:
        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DELAY));
        assert_eq!(get_prop_value(&compo.width), 100);
        assert_eq!(get_prop_value(&compo.width_times_two), 200);

        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DELAY + DURATION / 2));
        assert_eq!(get_prop_value(&compo.width), 150);
        assert_eq!(get_prop_value(&compo.width_times_two), 300);

        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DELAY + DURATION));
        assert_eq!(get_prop_value(&compo.width), 200);
        assert_eq!(get_prop_value(&compo.width_times_two), 400);

        // Overshoot: Always to_value.
        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DELAY + DURATION + DURATION / 2));
        assert_eq!(get_prop_value(&compo.width), 200);
        assert_eq!(get_prop_value(&compo.width_times_two), 400);
    }

    #[test]
    fn properties_test_animation_triggered_by_binding_with_unrelated_dirty() {
        // Reproduces the dependency not changing target bug: the binding driving the
        // animated property (`row.1`) never changes, but an *unrelated* field of the
        // same source value (`row.0`) is rewritten every frame, like a repeated item's
        // model row being touched by `VecModel::set_row_data` every tick.
        #[derive(Default)]
        struct Component {
            width: Property<i32>,
            row: Property<(i32, bool)>,
        }

        let compo = Rc::pin(Component::default());

        let animation_details = PropertyAnimation {
            duration: DURATION.as_millis() as _,
            iteration_count: 1.,
            ..PropertyAnimation::default()
        };

        let w = PinWeak::downgrade(compo.clone());
        compo.width.set_animated_binding(
            move || {
                let compo = w.upgrade().unwrap();
                if get_prop_value(&compo.row).1 { 200 } else { 40 }
            },
            move || (animation_details.clone(), None),
        );

        compo.row.set((0, false));
        assert_eq!(get_prop_value(&compo.width), 40);

        let start_time = crate::animations::current_tick();

        // Flip the field the animation depends on: this should kick off a 40 -> 200
        // animation over DURATION.
        compo.row.set((0, true));
        assert_eq!(get_prop_value(&compo.width), 40);

        // Simulate ~700 real frames (16ms each -- more than DURATION worth of real time
        // in total)
        let tick = core::time::Duration::from_millis(16);
        for i in 1..=700u32 {
            compo.row.set((i as i32, true));
            crate::animations::CURRENT_ANIMATION_DRIVER
                .with(|driver| driver.update_animations(start_time + tick * i));
            // Poll every frame like a real renderer repainting
            let _ = get_prop_value(&compo.width);
        }

        // After more than DURATION worth of real time has elapsed, the animation should
        // have completed regardless of the unrelated per-frame writes to `row.0`.
        assert_eq!(get_prop_value(&compo.width), 200);
    }

    #[test]
    fn test_loop() {
        let compo = Component::new_test_component();

        let animation_details = PropertyAnimation {
            duration: DURATION.as_millis() as _,
            iteration_count: 2.,
            ..PropertyAnimation::default()
        };

        compo.width.set(100);

        let start_time = crate::animations::current_tick();

        set_animated_value(&compo.width, 200, animation_details);
        assert_eq!(get_prop_value(&compo.width), 100);

        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DURATION / 2));
        assert_eq!(get_prop_value(&compo.width), 150);

        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DURATION));
        assert_eq!(get_prop_value(&compo.width), 100);

        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DURATION + DURATION / 2));
        assert_eq!(get_prop_value(&compo.width), 150);

        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DURATION * 2));
        assert_eq!(get_prop_value(&compo.width), 200);

        // the binding should be removed
        compo.width.handle.access(|binding| assert!(binding.is_none()));
    }

    /// Compose `InfiniteTransition.run` (pin …:182-194): at duration scale 0 an
    /// infinite animation jumps to its end value, suspends without requesting
    /// further frames, and restarts when the scale rises above 0.
    #[test]
    fn infinite_animation_suspends_at_zero_scale() {
        struct DummyPlatform;
        impl crate::platform::Platform for DummyPlatform {
            fn create_window_adapter(
                &self,
            ) -> Result<Rc<dyn crate::platform::WindowAdapter>, crate::platform::PlatformError>
            {
                Err(crate::platform::PlatformError::Other("dummy".into()))
            }
        }
        let ctx = crate::SlintContext::new(Box::new(DummyPlatform));
        let compo = Component::new_test_component();

        let animation_details = PropertyAnimation {
            duration: DURATION.as_millis() as _,
            iteration_count: -1.,
            ..PropertyAnimation::default()
        };

        compo.width.set(100);
        let start_time = crate::animations::current_tick();
        set_animated_value(&compo.width, 200, animation_details);

        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DURATION / 10));
        assert_eq!(get_prop_value(&compo.width), 110);
        assert!(
            crate::animations::CURRENT_ANIMATION_DRIVER
                .with(|driver| driver.has_active_animations())
        );

        // Scale 0: the loop parks at its end value and requests no more frames.
        ctx.set_animation_duration_scale(0.);
        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DURATION));
        assert_eq!(get_prop_value(&compo.width), 200);
        assert!(
            !crate::animations::CURRENT_ANIMATION_DRIVER
                .with(|driver| driver.has_active_animations())
        );

        // It stays suspended: later ticks produce the end value and no frame request.
        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DURATION * 2));
        assert_eq!(get_prop_value(&compo.width), 200);
        assert!(
            !crate::animations::CURRENT_ANIMATION_DRIVER
                .with(|driver| driver.has_active_animations())
        );

        // Scale back above 0: the loop restarts and requests frames again.
        ctx.set_animation_duration_scale(1.);
        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DURATION * 3));
        assert_ne!(get_prop_value(&compo.width), 200);
        assert!(
            crate::animations::CURRENT_ANIMATION_DRIVER
                .with(|driver| driver.has_active_animations())
        );
    }

    #[test]
    fn test_loop_via_binding() {
        // Loop twice, restart the animation and still loop twice.

        let compo = Component::new_test_component();

        let start_time = crate::animations::current_tick();

        let animation_details = PropertyAnimation {
            duration: DURATION.as_millis() as _,
            iteration_count: 2.,
            ..PropertyAnimation::default()
        };

        let w = PinWeak::downgrade(compo.clone());
        compo.width.set_animated_binding(
            move || {
                let compo = w.upgrade().unwrap();
                get_prop_value(&compo.feed_property)
            },
            move || (animation_details.clone(), None),
        );

        compo.feed_property.set(100);
        assert_eq!(get_prop_value(&compo.width), 100);

        compo.feed_property.set(200);
        assert_eq!(get_prop_value(&compo.width), 100);

        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DURATION / 2));

        assert_eq!(get_prop_value(&compo.width), 150);

        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DURATION));

        assert_eq!(get_prop_value(&compo.width), 100);

        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DURATION + DURATION / 2));

        assert_eq!(get_prop_value(&compo.width), 150);

        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + 2 * DURATION));

        assert_eq!(get_prop_value(&compo.width), 200);

        // Overshoot a bit:
        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + 2 * DURATION + DURATION / 2));

        assert_eq!(get_prop_value(&compo.width), 200);

        // Restart the animation by setting a new value.

        let start_time = crate::animations::current_tick();

        compo.feed_property.set(300);
        assert_eq!(get_prop_value(&compo.width), 200);

        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DURATION / 2));

        assert_eq!(get_prop_value(&compo.width), 250);

        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DURATION));

        assert_eq!(get_prop_value(&compo.width), 200);

        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + DURATION + DURATION / 2));

        assert_eq!(get_prop_value(&compo.width), 250);

        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + 2 * DURATION));

        assert_eq!(get_prop_value(&compo.width), 300);

        crate::animations::CURRENT_ANIMATION_DRIVER
            .with(|driver| driver.update_animations(start_time + 2 * DURATION + DURATION / 2));

        assert_eq!(get_prop_value(&compo.width), 300);
    }

    #[test]
    fn spring_retarget_carries_velocity() {
        // A retarget mid-flight must carry the outgoing spring's velocity into the new one,
        // instead of restarting it at rest (which would produce a visible "pop").
        let compo = Component::new_test_component();

        let spring_details = PropertyAnimation {
            duration: 1000,
            easing: crate::animations::EasingCurve::Spring(0.0),
            ..PropertyAnimation::default()
        };

        compo.width.set(0);
        let start_time = crate::animations::current_tick();
        set_animated_value(&compo.width, 1000, spring_details.clone());

        // Let the spring run for a while so it picks up meaningful velocity.
        crate::animations::CURRENT_ANIMATION_DRIVER.with(|driver| {
            driver.update_animations(start_time + core::time::Duration::from_millis(300))
        });
        let before_a = get_prop_value(&compo.width) as f32;
        crate::animations::CURRENT_ANIMATION_DRIVER.with(|driver| {
            driver.update_animations(start_time + core::time::Duration::from_millis(310))
        });
        let before_b = get_prop_value(&compo.width) as f32;
        let slope_before = before_b - before_a; // units per 10ms, just prior to the retarget

        // Retarget to a new value while the spring is still moving.
        set_animated_value(&compo.width, 2000, spring_details);
        assert_eq!(
            get_prop_value(&compo.width) as f32,
            before_b,
            "retarget must not snap the value"
        );

        crate::animations::CURRENT_ANIMATION_DRIVER.with(|driver| {
            driver.update_animations(start_time + core::time::Duration::from_millis(320))
        });
        let after = get_prop_value(&compo.width) as f32;
        let slope_after = after - before_b; // units per 10ms, just after the retarget

        // With velocity carried over, the slope right after the retarget should be close to the
        // slope right before it (same order of magnitude, same direction). Without the fix, the
        // new spring starts at rest (v0 == 0), so `slope_after` would be near zero here.
        assert!(slope_before > 0.5, "sanity check: spring should be moving before retarget");
        assert!(
            slope_after > slope_before * 0.5,
            "velocity was not carried over: slope_before={slope_before}, slope_after={slope_after}"
        );
    }

    #[test]
    fn spring_retarget_via_binding_carries_velocity() {
        let compo = Component::new_test_component();

        let spring_details = PropertyAnimation {
            duration: 1000,
            easing: crate::animations::EasingCurve::Spring(0.0),
            ..PropertyAnimation::default()
        };

        let w = PinWeak::downgrade(compo.clone());
        let details = spring_details.clone();
        compo.width.set_animated_binding(
            move || {
                let compo = w.upgrade().unwrap();
                get_prop_value(&compo.feed_property)
            },
            move || (details.clone(), None),
        );

        // Establish the dependency and a baseline value (the very first read never animates).
        compo.feed_property.set(0);
        assert_eq!(get_prop_value(&compo.width), 0);

        let start_time = crate::animations::current_tick();
        compo.feed_property.set(1000);
        assert_eq!(get_prop_value(&compo.width), 0);

        // Let the spring run for a while so it picks up meaningful velocity.
        crate::animations::CURRENT_ANIMATION_DRIVER.with(|driver| {
            driver.update_animations(start_time + core::time::Duration::from_millis(300))
        });
        let before_a = get_prop_value(&compo.width) as f32;
        crate::animations::CURRENT_ANIMATION_DRIVER.with(|driver| {
            driver.update_animations(start_time + core::time::Duration::from_millis(310))
        });
        let before_b = get_prop_value(&compo.width) as f32;
        let slope_before = before_b - before_a; // units per 10ms, just prior to the retarget

        // Retarget mid-flight by changing the value the animated binding reads.
        compo.feed_property.set(2000);
        assert_eq!(
            get_prop_value(&compo.width) as f32,
            before_b,
            "retarget must not snap the value"
        );

        crate::animations::CURRENT_ANIMATION_DRIVER.with(|driver| {
            driver.update_animations(start_time + core::time::Duration::from_millis(320))
        });
        let after = get_prop_value(&compo.width) as f32;
        let slope_after = after - before_b; // units per 10ms, just after the retarget

        assert!(slope_before > 0.5, "sanity check: spring should be moving before retarget");
        assert!(
            slope_after > slope_before * 0.5,
            "velocity was not carried over through the binding-triggered retarget path: slope_before={slope_before}, slope_after={slope_after}"
        );
    }

    #[test]
    fn spring_continuous_retarget_keeps_advancing() {
        let compo = Component::new_test_component();

        let spring_details = PropertyAnimation {
            duration: 1000,
            easing: crate::animations::EasingCurve::Spring(0.7),
            ..PropertyAnimation::default()
        };

        let w = PinWeak::downgrade(compo.clone());
        let details = spring_details.clone();
        compo.width.set_animated_binding(
            move || {
                let compo = w.upgrade().unwrap();
                get_prop_value(&compo.feed_property)
            },
            move || (details.clone(), None),
        );

        compo.feed_property.set(0);
        assert_eq!(get_prop_value(&compo.width), 0);

        let start_time = crate::animations::current_tick();
        let mut mouse_x = 0i32;
        let mut final_width = 0f32;
        for frame in 1..=200 {
            mouse_x += 5; // simulate a steady mouse drag, 5px per frame
            compo.feed_property.set(mouse_x);
            let t = start_time + core::time::Duration::from_millis(frame * 16);
            crate::animations::CURRENT_ANIMATION_DRIVER.with(|driver| driver.update_animations(t));
            // Read the property every frame, like a renderer would when painting each frame --
            // this is what actually drives `evaluate()` (property evaluation is lazy).
            final_width = get_prop_value(&compo.width) as f32;
        }

        assert!(
            final_width > 500.0,
            "spring should have tracked the continuously-moving target by now, got {final_width}"
        );
    }

    #[test]
    fn spring_respects_reverse_direction() {
        let compo = Component::new_test_component();

        let spring_details = PropertyAnimation {
            duration: 200,
            easing: crate::animations::EasingCurve::Spring(0.0),
            direction: AnimationDirection::Reverse,
            ..PropertyAnimation::default()
        };

        compo.width.set(0);
        let start_time = crate::animations::current_tick();
        set_animated_value(&compo.width, 100, spring_details);

        // Reverse: the animation starts at the target and settles back at the origin.
        assert_eq!(get_prop_value(&compo.width), 100);

        crate::animations::CURRENT_ANIMATION_DRIVER.with(|driver| {
            driver.update_animations(start_time + core::time::Duration::from_millis(2000))
        });
        assert_eq!(get_prop_value(&compo.width), 0);

        // the binding should be removed once settled
        compo.width.handle.access(|binding| assert!(binding.is_none()));
    }

    #[test]
    fn spring_respects_iteration_count_bounce() {
        let compo = Component::new_test_component();

        let spring_details = PropertyAnimation {
            duration: 200,
            easing: crate::animations::EasingCurve::Spring(0.0),
            direction: AnimationDirection::Alternate,
            iteration_count: 2.,
            ..PropertyAnimation::default()
        };

        compo.width.set(0);
        let start_time = crate::animations::current_tick();
        set_animated_value(&compo.width, 100, spring_details);
        assert_eq!(get_prop_value(&compo.width), 0);

        // The first leg (forward) switches into the second (reversed) leg at exactly
        // `duration`, carrying the spring's velocity into the bounce-back.
        crate::animations::CURRENT_ANIMATION_DRIVER.with(|driver| {
            driver.update_animations(start_time + core::time::Duration::from_millis(200))
        });
        let mid = get_prop_value(&compo.width);
        assert!(mid > 90, "expected the first leg to have reached the target, got {mid}");
        compo.width.handle.access(|binding| assert!(binding.is_some()));

        // Second leg settles back at the origin, and the binding is then removed.
        crate::animations::CURRENT_ANIMATION_DRIVER.with(|driver| {
            driver.update_animations(start_time + core::time::Duration::from_millis(600))
        });
        assert_eq!(get_prop_value(&compo.width), 0);
        compo.width.handle.access(|binding| assert!(binding.is_none()));
    }

    #[test]
    fn spring_never_settling_on_its_own_settles_within_budget() {
        // bounce: 1.0 is undamped -- on its own it oscillates forever. With a finite
        // iteration-count (the default, here), it must still settle within
        // 10 multiples of `duration` (see `spring_settle_within`).
        let compo = Component::new_test_component();

        let spring_details = PropertyAnimation {
            duration: 200,
            easing: crate::animations::EasingCurve::Spring(1.0),
            ..PropertyAnimation::default()
        };

        compo.width.set(0);
        let start_time = crate::animations::current_tick();
        set_animated_value(&compo.width, 100, spring_details);

        // An undamped spring's period equals `duration`, so right at `duration` it's back near
        // its start -- nowhere near settled, and still running at full, unclamped bounce.
        crate::animations::CURRENT_ANIMATION_DRIVER.with(|driver| {
            driver.update_animations(start_time + core::time::Duration::from_millis(200))
        });
        assert!(get_prop_value(&compo.width) < 20, "should still be near the start at duration");
        compo.width.handle.access(|binding| assert!(binding.is_some()));

        // Comfortably within the settle budget (10x duration), it must have settled.
        crate::animations::CURRENT_ANIMATION_DRIVER.with(|driver| {
            driver.update_animations(start_time + core::time::Duration::from_millis(2000))
        });
        assert_eq!(get_prop_value(&compo.width), 100);
        compo.width.handle.access(|binding| assert!(binding.is_none()));
    }

    #[test]
    fn spring_never_settling_stays_infinite_with_iteration_count_minus_one() {
        // The same never-settling bounce, but with `iteration-count: -1`: it must keep
        // oscillating indefinitely instead of ever being re-damped.
        let compo = Component::new_test_component();

        let spring_details = PropertyAnimation {
            duration: 200,
            easing: crate::animations::EasingCurve::Spring(1.0),
            iteration_count: -1.,
            direction: AnimationDirection::Alternate,
            ..PropertyAnimation::default()
        };

        compo.width.set(0);
        let start_time = crate::animations::current_tick();
        set_animated_value(&compo.width, 100, spring_details);

        // Well past what would be the settle budget for a finite iteration-count: still running.
        crate::animations::CURRENT_ANIMATION_DRIVER.with(|driver| {
            driver.update_animations(start_time + core::time::Duration::from_millis(3000))
        });
        compo.width.handle.access(|binding| assert!(binding.is_some()));
    }

    /// A mid-flight retarget of a shape spring animation keeps both position and
    /// velocity continuous: the new morph's `from` is the outline on screen at the
    /// retarget frame, and the outgoing spring's scalar velocity — the mean
    /// per-anchor displacement rate — carries into the new morph (design note R6).
    #[test]
    fn shape_spring_retarget_keeps_position_and_velocity() {
        use crate::graphics::shapes::Point;
        use crate::graphics::shapes::{self, Cubic, Shape};

        #[derive(Default)]
        struct ShapeComponent {
            shape: Property<Shape>,
        }
        let compo = Rc::pin(ShapeComponent::default());

        let cubics_of = |s: &Shape| -> Vec<Cubic> {
            s.cubics().as_chunks::<8>().0.iter().map(|p| Cubic { points: *p }).collect()
        };
        // The shape's cubic anchors, in outline order.
        let anchors = |s: &Shape| -> Vec<Point> {
            cubics_of(s).iter().map(|c| Point { x: c.anchor0_x(), y: c.anchor0_y() }).collect()
        };
        // A dense polyline of the outline (32 segments per cubic).
        let segments = |s: &Shape| -> Vec<(Point, Point)> {
            cubics_of(s)
                .iter()
                .flat_map(|c| {
                    (0..32).map(|k| {
                        (c.point_on_curve(k as f32 / 32.), c.point_on_curve((k + 1) as f32 / 32.))
                    })
                })
                .collect()
        };
        let dist_to_segment = |p: Point, (a, b): (Point, Point)| -> f32 {
            let ab = b - a;
            let t = (((p - a).x * ab.x + (p - a).y * ab.y)
                / (ab.x * ab.x + ab.y * ab.y).max(1e-12))
            .clamp(0., 1.);
            ((p - a).x - t * ab.x).hypot((p - a).y - t * ab.y)
        };

        let circle = shapes::circle_shape(8);
        let triangle = shapes::regular_polygon(3, shapes::CornerRounding::UNROUNDED);
        let star = shapes::star_shape(
            5,
            0.4,
            shapes::CornerRounding::UNROUNDED,
            shapes::CornerRounding::UNROUNDED,
        );

        compo.shape.set(circle);
        let start_time = crate::animations::current_tick();
        let tick = core::time::Duration::from_millis(16);

        let spring_details = PropertyAnimation {
            easing: crate::animations::EasingCurve::PhysicalSpring {
                damping_ratio: 0.8,
                stiffness: 200.,
                mass: 1.,
            },
            ..PropertyAnimation::default()
        };

        // circle -> triangle spring, sampled a few frames into its fast phase.
        set_animated_value(&compo.shape, triangle, spring_details.clone());
        let frame = |i: u32| {
            crate::animations::CURRENT_ANIMATION_DRIVER
                .with(|driver| driver.update_animations(start_time + tick * i));
            get_prop_value(&compo.shape)
        };
        for i in 1..8 {
            frame(i);
        }
        let t = frame(8);

        // The outgoing spring's scalar velocity — the mean per-anchor displacement
        // rate normalized by the start perimeter — is what carries across.
        let speed_before = compo.shape.handle.current_velocity().unwrap_or_default()[0];

        // Retarget mid-flight to the star: the outline on screen must not jump —
        // every anchor of the new morph's start cut lies on the previous outline.
        set_animated_value(&compo.shape, star.clone(), spring_details.clone());
        let t_after = get_prop_value(&compo.shape);
        let segs = segments(&t);
        let mut max_delta = 0f32;
        for a in anchors(&t_after) {
            max_delta =
                max_delta.max(segs.iter().map(|s| dist_to_segment(a, *s)).fold(f32::MAX, f32::min));
        }
        assert!(max_delta <= 1e-3, "retarget moved the outline by {max_delta}");

        // The mean anchor speed carries over exactly: the new spring's velocity
        // at the retarget tick equals the outgoing spring's.
        let speed_after = compo.shape.handle.current_velocity().unwrap_or_default()[0];
        assert!(
            (speed_after - speed_before).abs() <= 0.05 * speed_before,
            "anchor speed jumped at retarget: {speed_before} -> {speed_after}"
        );

        // It still settles on the new target.
        for i in 9..=100 {
            frame(i);
        }
        assert_eq!(get_prop_value(&compo.shape), star);
        compo.shape.handle.access(|binding| assert!(binding.is_none()));
    }

    /// The driver samples a shape spring at every tick: consecutive samples
    /// move each anchor continuously — every anchor of the outline at T sits
    /// within a frame's step of both the T−1 and T+1 outlines rather than
    /// jumping.
    #[test]
    fn shape_physical_spring_anchors_track_continuously() {
        use crate::graphics::shapes::Point;
        use crate::graphics::shapes::{self, Cubic, Shape};

        #[derive(Default)]
        struct ShapeComponent {
            shape: Property<Shape>,
        }
        let compo = Rc::pin(ShapeComponent::default());

        let anchors = |s: &Shape| -> Vec<Point> {
            s.cubics()
                .as_chunks::<8>()
                .0
                .iter()
                .map(|p| Cubic { points: *p })
                .map(|c| Point { x: c.anchor0_x(), y: c.anchor0_y() })
                .collect()
        };
        // Dense polyline of the outline: the anchor layout of a morph result is
        // re-detected per frame, so compare each anchor against the neighboring
        // outlines, not by index.
        let outline = |s: &Shape| -> Vec<Point> {
            s.cubics()
                .as_chunks::<8>()
                .0
                .iter()
                .map(|p| Cubic { points: *p })
                .flat_map(|c| [0., 1. / 3., 2. / 3.].map(|t| c.point_on_curve(t)))
                .collect()
        };
        let dist_to_segment = |p: Point, (a, b): (Point, Point)| -> f32 {
            let ab = b - a;
            let t = (((p - a).x * ab.x + (p - a).y * ab.y)
                / (ab.x * ab.x + ab.y * ab.y).max(1e-12))
            .clamp(0., 1.);
            ((p - a).x - t * ab.x).hypot((p - a).y - t * ab.y)
        };
        let dist_to_outline = |p: Point, outline: &[Point]| -> f32 {
            outline
                .iter()
                .zip(outline.iter().cycle().skip(1))
                .map(|(&a, &b)| dist_to_segment(p, (a, b)))
                .fold(f32::MAX, f32::min)
        };

        let circle = shapes::circle_shape(8);
        let star = shapes::star_shape(
            5,
            0.4,
            shapes::CornerRounding::UNROUNDED,
            shapes::CornerRounding::UNROUNDED,
        );
        compo.shape.set(circle);
        let start_time = crate::animations::current_tick();
        let tick = core::time::Duration::from_millis(16);

        set_animated_value(
            &compo.shape,
            star,
            PropertyAnimation {
                easing: crate::animations::EasingCurve::PhysicalSpring {
                    damping_ratio: 0.9,
                    stiffness: 60.,
                    mass: 1.,
                },
                ..PropertyAnimation::default()
            },
        );
        let sample = |i: u32| {
            crate::animations::CURRENT_ANIMATION_DRIVER
                .with(|driver| driver.update_animations(start_time + tick * i));
            get_prop_value(&compo.shape)
        };

        let samples: Vec<Shape> = (2..40).map(&sample).collect();
        let outlines: Vec<Vec<Point>> = samples.iter().map(&outline).collect();
        let mut checked_ticks = 0;
        for (i, w) in samples.windows(3).enumerate() {
            // A snap to the empty shape or to an endpoint is a teleport — the
            // tolerance bounds per-frame anchor travel to a fraction of the
            // unit-square shape extent.
            let tol = 0.3;
            let (a_mid, prev, next) = (anchors(&w[1]), &outlines[i], &outlines[i + 2]);
            assert!(!a_mid.is_empty(), "tick {} produced an empty outline", i + 3);
            for (j, a) in a_mid.iter().enumerate() {
                let d = dist_to_outline(*a, prev).max(dist_to_outline(*a, next));
                assert!(d <= tol, "anchor {j} at tick {} off its trajectory by {d}", i + 3,);
            }
            checked_ticks += 1;
        }
        assert!(checked_ticks >= 30, "checked only {checked_ticks} animating ticks");
    }

    /// `spring(damping_ratio, stiffness)` — the `PhysicalSpring` variant the
    /// Material animations API uses — animates a `shape` on its displacement
    /// channel instead of snapping or producing the empty shape mid-flight.
    #[test]
    fn shape_physical_spring_animates_and_settles() {
        use crate::graphics::shapes::{self, Shape};

        #[derive(Default)]
        struct ShapeComponent {
            shape: Property<Shape>,
        }
        let compo = Rc::pin(ShapeComponent::default());

        let circle = shapes::circle_shape(8);
        let star = shapes::star_shape(
            5,
            0.4,
            shapes::CornerRounding::UNROUNDED,
            shapes::CornerRounding::UNROUNDED,
        );

        compo.shape.set(circle.clone());
        let start_time = crate::animations::current_tick();

        let spring_details = PropertyAnimation {
            easing: crate::animations::EasingCurve::PhysicalSpring {
                damping_ratio: 0.6,
                stiffness: 200.,
                mass: 1.,
            },
            iteration_count: 1.,
            ..PropertyAnimation::default()
        };
        set_animated_value(&compo.shape, star.clone(), spring_details);

        // Mid-flight the property holds an interpolated (non-empty) outline.
        crate::animations::CURRENT_ANIMATION_DRIVER.with(|driver| {
            driver.update_animations(start_time + core::time::Duration::from_millis(150))
        });
        let mid = get_prop_value(&compo.shape);
        assert!(!mid.is_empty(), "physical spring produced an empty shape mid-flight");
        assert_ne!(mid, circle);
        assert_ne!(mid, star);

        // Once settled the target is held exactly and the binding is removed.
        crate::animations::CURRENT_ANIMATION_DRIVER.with(|driver| {
            driver.update_animations(start_time + core::time::Duration::from_millis(20000))
        });
        assert_eq!(get_prop_value(&compo.shape), star);
        compo.shape.handle.access(|binding| assert!(binding.is_none()));
    }

    /// `rebuild_from_channels(write_channels(v)) == v` for every animatable type that
    /// decomposes a value pair into spring channels.
    #[test]
    fn channel_round_trips() {
        fn check<T: InterpolatedPropertyValue + core::fmt::Debug>(a: T, b: T) {
            let mut channels = alloc::vec![0.0; a.channel_count(&b)];
            a.write_channels(&b, &mut channels);
            let rebuilt = a.rebuild_from_channels(&b, &channels);
            assert_eq!(rebuilt, a, "round-trip failed for {a:?} -> {b:?}");
            let mut channels_b = alloc::vec![0.0; b.channel_count(&a)];
            b.write_channels(&a, &mut channels_b);
            let rebuilt_b = b.rebuild_from_channels(&a, &channels_b);
            assert_eq!(rebuilt_b, b, "reverse round-trip failed for {b:?} -> {a:?}");
        }

        check(0.0f32, 100.0f32);
        check(3i32, -17i32);
        check(crate::lengths::LogicalLength::new(12.5), crate::lengths::LogicalLength::new(300.0));

        // Colors animate in Oklab; the round-trip is sRGB → Oklab → sRGB and
        // loses a bit of precision to gamut clamping, so compare visually.
        for pair in [
            (crate::Color::from_rgb_u8(0, 0, 0), crate::Color::from_rgb_u8(255, 255, 255)),
            (crate::Color::from_rgb_u8(255, 0, 0), crate::Color::from_rgb_u8(0, 255, 0)),
            (
                crate::Color::from_argb_u8(0x80, 12, 34, 56),
                crate::Color::from_argb_u8(0xff, 200, 100, 50),
            ),
        ] {
            let (a, b) = pair;
            let mut channels = alloc::vec![0.0; a.channel_count(&b)];
            a.write_channels(&b, &mut channels);
            let rebuilt = a.rebuild_from_channels(&b, &channels);
            let (ra, rr) = (
                crate::graphics::RgbaColor::<u8>::from(a),
                crate::graphics::RgbaColor::<u8>::from(rebuilt),
            );
            assert!(
                (ra.red as i16 - rr.red as i16).abs() <= 1
                    && (ra.green as i16 - rr.green as i16).abs() <= 1
                    && (ra.blue as i16 - rr.blue as i16).abs() <= 1
                    && (ra.alpha as i16 - rr.alpha as i16).abs() <= 1,
                "color round-trip drifted: {a:?} -> {rebuilt:?}"
            );
        }

        // Brushes: solid and gradient pairs. Their channels carry colors, so
        // compare with the same visual tolerance as plain colors.
        fn check_brush(a: crate::Brush, b: crate::Brush) {
            use crate::graphics::GradientStop;
            let mut channels = alloc::vec![0.0; a.channel_count(&b)];
            a.write_channels(&b, &mut channels);
            let rebuilt = a.rebuild_from_channels(&b, &channels);
            fn same_stops(a: &[GradientStop], b: &[GradientStop]) -> bool {
                a.len() == b.len()
                    && a.iter().zip(b.iter()).all(|(x, y)| {
                        let (xc, yc) = (
                            crate::graphics::RgbaColor::<u8>::from(x.color),
                            crate::graphics::RgbaColor::<u8>::from(y.color),
                        );
                        (xc.red as i16 - yc.red as i16).abs() <= 1
                            && (xc.green as i16 - yc.green as i16).abs() <= 1
                            && (xc.blue as i16 - yc.blue as i16).abs() <= 1
                            && (xc.alpha as i16 - yc.alpha as i16).abs() <= 1
                            && x.position == y.position
                    })
            }
            let same = match (&a, &rebuilt) {
                (crate::Brush::SolidColor(x), crate::Brush::SolidColor(y)) => {
                    let (xc, yc) = (
                        crate::graphics::RgbaColor::<u8>::from(*x),
                        crate::graphics::RgbaColor::<u8>::from(*y),
                    );
                    (xc.red as i16 - yc.red as i16).abs() <= 1
                        && (xc.green as i16 - yc.green as i16).abs() <= 1
                        && (xc.blue as i16 - yc.blue as i16).abs() <= 1
                        && (xc.alpha as i16 - yc.alpha as i16).abs() <= 1
                }
                (crate::Brush::LinearGradient(x), crate::Brush::LinearGradient(y)) => {
                    x.angle() == y.angle()
                        && same_stops(
                            &x.stops().copied().collect::<alloc::vec::Vec<_>>(),
                            &y.stops().copied().collect::<alloc::vec::Vec<_>>(),
                        )
                }
                (crate::Brush::RadialGradient(x), crate::Brush::RadialGradient(y)) => same_stops(
                    &x.stops().copied().collect::<alloc::vec::Vec<_>>(),
                    &y.stops().copied().collect::<alloc::vec::Vec<_>>(),
                ),
                _ => a == rebuilt,
            };
            assert!(same, "brush round-trip failed for {a:?} -> {b:?}: got {rebuilt:?}");
        }

        let solid = crate::Brush::SolidColor(crate::Color::from_rgb_u8(10, 20, 30));
        let stops: crate::SharedVector<crate::graphics::GradientStop> = [
            crate::graphics::GradientStop {
                color: crate::Color::from_rgb_u8(255, 0, 0),
                position: 0.0,
            },
            crate::graphics::GradientStop {
                color: crate::Color::from_rgb_u8(0, 0, 255),
                position: 45.0,
            },
            crate::graphics::GradientStop {
                color: crate::Color::from_rgb_u8(0, 255, 0),
                position: 1.0,
            },
        ]
        .into_iter()
        .collect();
        let linear =
            crate::Brush::LinearGradient(crate::graphics::LinearGradientBrush::new(90.0, stops));
        check_brush(solid.clone(), solid.clone());
        check_brush(linear.clone(), linear.clone());
    }
}
