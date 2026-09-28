<!-- cSpell: ignore millis -->
# Animation System Internals

> Note for AI coding assistants (agents):
> **When to load this document:** Working on `internal/core/animations.rs`,
> debugging animation timing issues, or optimizing animation performance.
> For general build commands and project structure, see `/AGENTS.md`.

## Animation Timing System

Slint animations use a **mocked time system** rather than real-time clocks. This provides:
- Deterministic animation behavior for testing
- Frame-rate independence
- Consistent behavior across platforms

The animation driver (`internal/core/animations.rs`) manages a global instant that advances each frame:

```
AnimationDriver
├── global_instant: Pin<Box<Property<Instant>>>  // Current animation time
├── active_animations: Cell<bool>                // Whether animations are running
└── update_animations(new_tick)                  // Called per frame by the backend
```

**Key components:**

| Function/Type | Location | Purpose |
|---------------|----------|---------|
| `Instant` | `internal/core/animations.rs` | Milliseconds since animation driver started |
| `current_tick()` | `internal/core/animations.rs` | Get current animation time (registers dependency) |
| `animation_tick()` | `internal/core/animations.rs` | Same, but signals a frame is needed |
| `update_timers_and_animations()` | `internal/core/platform.rs` | Called by platform each frame |
| `EasingCurve` | `internal/core/animations.rs` | Enum of easing curve types |

## Easing Curve Implementation

Easing curves are defined in the `EasingCurve` enum in `internal/core/animations.rs`, which
also holds the interpolation logic. (The compiler has its own copy of the enum for constant
folding at compile time, in `internal/compiler/expression_tree.rs`.)

For `cubic-bezier(a, b, c, d)`, Slint uses a binary search algorithm to find the t parameter for a given x value, then evaluates the y component of the bezier curve.

Standard easings (`ease-in`, `ease-out`, `ease-in-out`, etc.) are pre-defined cubic bezier curves.

`easing` is a runtime value: `Value::EasingCurve` in the interpreter, `EasingCurve`
in Rust, `slint::EasingCurve` in C++, `{ type: "spring", ... }` objects in Node.js,
and the `slint.EasingCurve` classmethods in Python all carry the same enum.

## Physical Springs

`spring(damping-ratio, stiffness[, mass])` produces `EasingCurve::PhysicalSpring`.
It has no `duration`: the animation runs until it settles, so `duration`,
`iteration-count`, and `direction` are ignored (with a compile-time warning).

The simulation lives in `internal/core/animations/simulations/spring.rs`:

- `PhysicalSpringParameters` holds ζ, stiffness, mass, and the initial velocity.
- `SpringRegime` evaluates the closed-form mass-spring-damper ODE in `f32` —
  the same formulas as `androidx.compose.animation.core.SpringSimulation`
  (underdamped / critically damped / overdamped branches). Its tests compare
  every regime against an `f64` transcription of `SpringSimulation.updateValues`
  (`reference()` in `spring.rs`) sampled at integer-millisecond times.
- `PhysicalSpringToLimit` drives one scalar channel toward a limit, re-anchoring
  on retarget so position and velocity stay continuous, and ends at the
  estimated settle time from `simulations/spring_estimation.rs` — a port of
  Compose's `estimateAnimationDurationMillis`.

### Channels and velocity carry-over

`InterpolatedPropertyValue` decomposes a value into scalar channels:
`channel_count` / `write_channels` / `from_channels` on the `(from, to)` pair.
Scalars are one channel; `Color` is four Oklab channels `(alpha, l, a, b)`; a
`Brush` is its gradient layout's headers plus five channels per stop. A physical
spring animates each channel with its own `PhysicalSpringToLimit`, and a
retarget hands the outgoing animation's per-channel velocities to the new one
(`carried_velocity`), falling back to the `initial-velocity` field per channel
when there was none.

`Flickable` captures the declared animation of `content-x`/`content-y` at press
time (`BindingCallable::declared_animation`); on release it flings with a
`PhysicalSpringToLimit` seeded by `release-velocity` when that animation was a
physical spring, else the constant-deceleration glide.

`PropertyAnimation` (`internal/core/items.rs`) is `#[repr(C)]` and exported to
C++ through cbindgen, so its fields are ABI — adding one changes the generated
header's layout. Every field must then be emitted by `animation_fields()` in
`internal/compiler/llr/lower_expression.rs` (which lists them all, including
the internal-only `visibility-threshold` settle-threshold override the
interpreter uses for integer-typed properties — `0` means the animated type's
own threshold). Internal fields go on `PropertyAnimation` only, not in the
`animate` property surface (`builtin_elements.rs`).

### Duration scale and reduced motion

`SlintContext::animation_duration_scale` multiplies every animation's progress
(`animations::duration_scale()`); at `0` a finite animation completes
immediately and an infinite one suspends at its end value until the scale
returns — Compose's `MotionDurationScale` / `InfiniteTransition` behavior. It
is fed by Android's `animator_duration_scale` (the only platform scale) and
`SLINT_SLOW_ANIMATIONS`.

`SlintContext::reduced_motion` is a separate *semantic* flag — it does not
force the scale. Platform accessibility settings feed it: the XDG
`org.freedesktop.appearance` `reduce-motion` portal key combined with GNOME's
`enable-animations` (reduced if either says so), Windows'
`SPI_GETCLIENTAREAANIMATION` (plus a `WM_SETTINGCHANGE` window subclass for
live changes), `prefers-reduced-motion` on the web, macOS
`accessibilityDisplayShouldReduceMotion`, iOS
`UIAccessibilityIsReduceMotionEnabled`, Qt's animate-UI setting, and Android
when `animator_duration_scale` is 0. `.slint` reads it through
`SlintInternal.reduced-motion` / `Palette.reduced-motion`, and the widget
styles gate their `animate` blocks on it (`ReducedMotionSelector` allows an
app override). The testing backend exposes `set_reduced_motion()` /
`set_animation_duration_scale()` (also over FFI and `slint_testing::` in C++).

## Animation Performance

Each animated property:
1. Re-evaluates its binding every frame
2. Marks dependents dirty
3. Triggers re-rendering of affected items

**Efficient to animate** (no layout recalculation):
- `x`, `y` - Position
- `opacity` - Transparency
- `rotation-angle` - Rotation
- `background` - Colors/gradients

**Expensive to animate** (triggers layout):
- `width`, `height`
- `preferred-width`, `preferred-height`
- Any property that affects sibling positioning

## Debugging Animations

### Slow Motion

```sh
# Slow animations by factor of 4
SLINT_SLOW_ANIMATIONS=4 cargo run

# Slow by factor of 10 for detailed inspection
SLINT_SLOW_ANIMATIONS=10 cargo run
```

Useful for:
- Verifying easing curves
- Checking animation start/end states
- Debugging timing between multiple animations

### Checking Active Animations

```rust
// In application code
if window.has_active_animations() {
    // Animations are in progress
}
```

### Mock Time in Tests

For deterministic testing without real-time waits:

```rust
use i_slint_backend_testing::mock_elapsed_time;
use core::time::Duration;

// Advance animation time by 100ms
mock_elapsed_time(Duration::from_millis(100));

// Complete a 300ms animation
mock_elapsed_time(Duration::from_millis(300));
```

This is implemented in `internal/backends/testing/` and used throughout the test suite.

## Key Files

| File | Purpose |
|------|---------|
| `internal/core/animations.rs` | Animation driver, timing, interpolation, `EasingCurve` enum |
| `internal/core/timers.rs` | Timer integration with animation system |
| `internal/core/platform.rs` | `update_timers_and_animations()` entry point |

## Common Modification Patterns

### Adding a New Easing Curve

1. Add a variant to the `EasingCurve` enum in `internal/core/animations.rs` (and its
   compiler-side copy in `internal/compiler/expression_tree.rs`)
2. Handle interpolation in `internal/core/animations.rs`
3. Add parsing support in `internal/compiler/` if new syntax needed
4. Add tests in `tests/cases/`

### Debugging Animation Glitches

1. Use `SLINT_SLOW_ANIMATIONS=10` to slow down
2. Check if issue is in timing (`animations.rs`) or rendering (`renderers/`)
3. Add `eprintln!` in `update_animations()` to trace tick values
4. Use screenshot tests to capture specific animation frames
