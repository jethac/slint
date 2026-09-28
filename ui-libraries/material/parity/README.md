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

- `compose/` — Android library module with a Paparazzi test (`RenderTest`)
  that composes every scene at density 1 and 2, renders frames through
  layoutlib, and writes `references/<case>/d{1,2}/frame_<tag>.png` plus a
  text-bounds mask (`mask_<tag>.png`) and `trace.json` (element rects, text
  metrics, motion samples). The references are committed; the Slint driver
  fails when they're missing or stale.

## Running the Slint side

```sh
SLINT_TEST_FILTER=material cargo test --manifest-path tests/Cargo.toml -p test-driver-screenshots
```

Every `//PARITY=` case runs on the software renderer, the software line-by-line
renderer, and Skia — plus FemtoVG when built with `--features femtovg` (uses a
surfaceless EGL context, so it works headless under Mesa llvmpipe).

References are required in CI: `PARITY_REQUIRE_REFS=1` turns a missing or
stale reference into a hard failure. Without the env var a case with no
committed reference warns and skips — the Slint-side harness (scene loading,
actions, mocked-time capture, trace recording, self-tests) still runs.

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
| Geometry/color | per-pixel RGBA on flat regions | `PARITY_EPS` (8) per channel |
| Outlines | pixels near a detected image edge: AA drift is a fraction of the edge's contrast | `min(EDGE_EPS 56, contrast/2)` |
| Traced elements | the outline-disagreement zone between the Slint and Compose bounds (dilated for decorations like focus ring and elevation shadow) is skipped; geometry is compared numerically instead | `GEOM_EPS` 0.5px, drift-aware |
| Text | pixels in the union of both sides' text bounds: per-cell mean/outlier bound, plus numeric metrics — width validates against the font's own fractional advance (`frac_w`), placement compares the label's offset inside its container | `TEXT_CELL_EPS` 48, ≤35% outliers at >96, `w` bound 0.5 + 0.25px/glyph, offset ±0.5px |
| Motion | trace values at identical timestamps after measuring the fixed phase offset between the engines' animation clocks, plus settle time | `TRACE_EPS` 1.0px, settle within 25% |
| Negative | must fail the comparator | — |

Why text tolerances look loose: layoutlib's hinted, integer-advance text
layout renders measurably bolder and up to ~1.6px/label wider than
unhinted font metrics; Slint's text is closer to the font's own numbers.
The trace layer validates Slint's text width against `frac_w` (the
engine's own fractional line advance, emitted by the Compose harness) with
a per-glyph bound, so the cross-engine drift is bounded, not
pixel-compared. A negative case asserting a wrong label would fail well
above these bounds.

## Version mapping

`androidx.compose.material3:material3:1.5.0-alpha18` is the Maven Central
name for androidx.dev main build **23327507** (released 2026-04-22). It is
the newest material3 alpha whose transitive Compose version Paparazzi's
layoutlib can consume: alpha19+ pull `compose-ui:1.12.0-alpha*` which
requires AGP 9.1 and compileSdk 37, while Paparazzi 2.0.0-alpha05 supports
at most AGP 8.13.x. alpha18 carries every Expressive API used here:
`MaterialExpressiveTheme`, `MotionScheme.expressive()`, the `shapes()`
button overload with `contentPaddingFor` (16dp), and spec2025
dynamiccolor defaults.

Diffs, masks and trace plots land in `target/parity-artifacts/<driver>/<case>/`
(`$PARITY_ARTIFACT_DIR` overrides).

## Regenerating Compose references

Needs an Android SDK (`ANDROID_HOME`, `platforms;android-36`, build-tools 36)
and JDK 21 (Paparazzi 2.x requires a Java 21 toolchain):

```sh
cd ui-libraries/material/parity/compose
./gradlew testDebugUnitTest -Pparity.record=1
```

References are written to `compose/references/`; commit them.

### Rendering choice (recorded for #4)

Paparazzi/layoutlib — the same rasterizer Android Studio previews use —
instead of Robolectric's `graphicsMode=NATIVE`. Two reasons:

- Robolectric's native runtime fails to initialize the system font map on
  this host (`Typeface.loadPreinstalledSystemFontMap` NPE, upstream
  [robolectric#9039](https://github.com/robolectric/robolectric/issues/9039)).
  Paparazzi avoids the JNI font path and is what Cash App's own screenshot
  tests use.
- The harness needs deterministic frame times and scripted interaction
  states. Paparazzi's `gif()` at `fps = 1000` makes frame indexes equal
  milliseconds, and interaction states are emitted directly into the
  scene's `MutableInteractionSource` (`PressInteraction`,
  `HoverInteraction`, `FocusInteraction`) rather than pointer injection —
  the state under test is the same, only the input path differs.

Regenerating references needs JDK 21 and an Android SDK with
`platforms;android-36` and `build-tools` (`ANDROID_HOME` or
`sdk.dir` in `local.properties`).

CI verifies the committed references are reproducible: the `material_parity`
job re-records every scene and fails if `references/` differs.
