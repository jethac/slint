<!--
Copyright © SixtyFPS GmbH <info@slint.dev>
SPDX-License-Identifier: MIT
-->

# Material parity harness

This directory pins the Slint Material 3 Expressive implementation against
Jetpack Compose Material3: the same scene renders on both sides and the outputs
are compared pixel-per-pixel (with text masked) plus numerically for motion.

## Layout

- `scenes/*.json` — shared scene definitions. One file describes a scene once:
  geometry, widgets, theme seed, scripted pointer actions, animation timings,
  and which properties/elements to trace.
- `generator/` — a small Rust crate (`material-parity-generator`) that turns
  each scene JSON into the `.slint` test case under
  `tests/screenshots/cases/material/` and a resolved JSON (with the ARGB color
  scheme computed by `material-color-utils`, the same code that
  `i-slint-core::material` uses at runtime) consumed by the Compose side.
  Regenerate after editing a scene:

  ```sh
  cargo run --manifest-path ui-libraries/material/Cargo.toml -p material-parity-generator
  # drift check (fails if generated files are stale):
  cargo run --manifest-path ui-libraries/material/Cargo.toml -p material-parity-generator -- --check
  ```

- `compose/` — Android library module with a Robolectric unit test
  (`ParityTest`) that composes every scene under `LocalDensity` 1.0 and 2.0,
  renders frames and writes `references/<case>/d{1,2}/frame_<tag>.png` plus a
  text-bounds mask (`mask_<tag>.png`) and `trace.json` for motion scenes.

## Running the Slint side

```sh
SLINT_TEST_FILTER=material cargo test --manifest-path tests/Cargo.toml -p test-driver-screenshots
```

Every `//PARITY=` case runs on the software renderer, the software line-by-line
renderer, and Skia — plus FemtoVG when built with `--features femtovg` (uses a
surfaceless EGL context, so it works headless under Mesa llvmpipe).

Without committed Compose references the parity comparison warns and skips; the
Slint-side harness (scene loading, actions, mocked-time capture, trace
recording, self-tests) still runs. Set `PARITY_REQUIRE_REFS=1` to turn missing
references into hard failures — CI does this for cases that have references.

## Case markers

A parity case is a normal `.slint` file with marker comments:

- `//PARITY=static|motion|negative[: note]` — required.
- `//SIZE=WxH` — canvas size in physical pixels at density 1.
- `//TIMES=t0,t1,...` — mocked-time frames (ms) to capture for motion scenes.
- `//TRACE_PROPS=foo,bar` — component properties recorded into the trace
  (`in-out` or `out` properties of the component root).
- `//TRACE_ELEMENTS=id,...` — elements (`:= id`) whose bounds/opacity are
  recorded each frame.
- `//ACTION=move|press|release:x,y` — pointer input applied before the scene
  settles or between captured frames.
- `//DENSITIES=1,2` — densities to render (default `1,2`).
- `//PARITY_EPS=N` — per-case pixel tolerance override (default 8/255 per
  channel).

## Comparison layers

| Layer | Check | Tolerance |
| --- | --- | --- |
| Geometry/color | per-pixel RGBA, excluding masked cells | `PARITY_EPS` (8) per channel |
| Text | pixels inside `mask_*.png` cells: metric-agreement — outlier cells may differ in ≤5% of pixels at ≤96 | `TEXT_CELL_EPS` 24 |
| Motion | trace values per timestamp, plus settle time | `TRACE_EPS` 1.0px, settle within 25% |
| Negative | must fail the comparator | — |

Diffs, masks and trace plots land in `target/parity-artifacts/<driver>/<case>/`
(`$PARITY_ARTIFACT_DIR` overrides).

## Regenerating Compose references

Needs an Android SDK (`ANDROID_HOME`, `platforms;android-35+`, build-tools 35+)
and JDK 17:

```sh
cd ui-libraries/material/parity/compose
./gradlew testDebugUnitTest -Pparity.record=1
```

References are written to `compose/references/`; commit them.

### Rendering choice (recorded for #4)

Robolectric with `graphicsMode=NATIVE` — real rasterization through the Android
native graphics stack — instead of Paparazzi/layoutlib, because the harness
needs the test-runtime clock (`mainClock.autoAdvance = false` +
`advanceTimeBy`) and pointer injection (`performTouchInput`,
`performMouseInput`), which the compose-ui-test rule provides. Paparazzi
renders a single composed frame per invocation and has neither.

### Known blocker: reference generation

On some hosts the Robolectric native runtime fails to initialize the system
font map — `Typeface.loadPreinstalledSystemFontMap` throws an NPE in
`Typeface.create` ("Cannot read field mStyle because family is null",
upstream [robolectric#9039](https://github.com/robolectric/robolectric/issues/9039),
open since 4.11). The Gradle project builds and the test is wired correctly;
on a host where native font initialization works, `-Pparity.record=1` produces
the references. Until the first set lands, Slint-side parity comparisons
warn-skip.
