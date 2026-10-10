<!--
Copyright © SixtyFPS GmbH <info@slint.dev>
SPDX-License-Identifier: MIT
-->

# Button completion audit

Reference: AndroidX commit `23327507f7fc7d5b19d65fec4b090f60c970079b`, as recorded in `TOKENS_SOURCE`.
This checklist covers the eight named toggle APIs and the ordinary button styles they inherit.
All sixteen component entries remain partial until their API and verification checklists pass.

## Public API gaps

The pinned [ToggleButton.kt](https://github.com/androidx/androidx/blob/23327507f7fc7d5b19d65fec4b090f60c970079b/compose/material3/material3/src/commonMain/kotlin/androidx/compose/material3/ToggleButton.kt) exposes colors, shapes, elevation, border, content padding, and composable content.
The Slint styles expose state colors, uniform and per-corner radii, arbitrary paths, physical elevation, borders, padding, and child content.
MaterialText and Icon children inherit reactive content colors through reusable components and repeaters.
Text buttons provide label typography; icon buttons preserve the enclosing text style.
Explicit child bindings override inherited defaults.
The pinned [IconButton.kt](https://github.com/androidx/androidx/blob/23327507f7fc7d5b19d65fec4b090f60c970079b/compose/material3/material3/src/commonMain/kotlin/androidx/compose/material3/IconButton.kt) also has static-shape and interaction-shape overloads.
The path overrides cover static and interaction paths; corner radii retain the pinned spring behavior.
The pinned `shapeByInteraction` switches arbitrary paths immediately and animates corner-based geometry.

| Slint toggle | Existing style | Additional API checks |
| --- | --- | --- |
| `ToggleButton` | `FilledButton` | Verify custom style parameters, arbitrary shapes, and child styling. |
| `FilledTonalToggleButton` | `TonalButton` | Verify custom style parameters, arbitrary shapes, and child styling. |
| `ElevatedToggleButton` | `ElevatedButton` | Verify custom elevation in every interaction state, arbitrary shapes, and child styling. |
| `OutlinedToggleButton` | `OutlineButton` | Verify custom borders in every state, arbitrary shapes, and child styling. |
| `IconToggleButton` | `IconButton` | Cover both pinned color families and static/interaction shape overloads. |
| `FilledIconToggleButton` | `FilledIconButton` | Cover custom state colors, content, and both shape overloads. |
| `FilledTonalIconToggleButton` | `TonalIconButton` | Cover custom state colors, content, and both shape overloads. |
| `OutlinedIconToggleButton` | `OutlineIconButton` | Cover custom borders, state colors, content, and both shape overloads. |

- [x] Expose enabled, disabled, checked, and unchecked container/content color overrides for each style.
- [x] Expose resting, pressed, and checked paths and per-corner geometry.
- [ ] Verify corner-transition interruption behavior against pinned Compose traces.
- [x] Expose interaction elevation overrides where the pinned API supports them.
- [x] Expose border color and width overrides, including disabled and checked states.
- [x] Provide padding overrides for all four text-toggle styles.
- [x] Support arbitrary child content and document how it inherits typography and content color.
- [x] Document the mapping from Compose's externally owned `checked`/`onCheckedChange` to Slint properties and callbacks.
- [x] Verify controlled checked bindings across all eight styles and both connected-group orientations without replacing bindings during activation.
- [x] Verify application-owned selection in the standard ButtonGroup and mapped SegmentedButton APIs.
- [ ] Compare every default size, icon spacing, padding, typography, and minimum touch target against the pinned helpers.

## Input and accessibility

The interpreter fixture verifies checkbox roles, names, activation, and disabled accessibility actions across all eight styles.
Controlled toggle activation requests changes without overwriting application bindings.
Connected groups preserve selection bindings in controlled mode and model-row bindings in both modes.
Pointer, keyboard release, and accessibility checks cover rejected and accepted requests.
Standard ButtonGroup rows and overflow menu items share one activation handler.
Controlled requests retain the selected-index binding; default mode preserves programmatic model-to-index synchronization.
SegmentedButton also preserves application-owned selection and rejects disabled activation.
Keyboard activation must follow the pinned [Clickable.kt](https://github.com/androidx/androidx/blob/23327507f7fc7d5b19d65fec4b090f60c970079b/compose/foundation/foundation/src/commonMain/kotlin/androidx/compose/foundation/Clickable.kt).
Key-down creates a press; matching key-up activates; focus loss cancels pending keys.

- [x] Verify pointer, keyboard, and accessibility activation for all eight toggle styles.
- [x] Reject disabled accessibility activation.
- [x] Verify keyboard repeat, overlapping Space/Enter presses, mismatched releases, focus loss, and disabling during a press.
- [x] Verify the shared keyboard contract through the public `ExtendedTouchArea` API.
- [x] Reject pointer activation outside custom paths across all eight toggle styles.
- [ ] Verify touch cancellation, drag outside, shape-clipped hit testing, and disabling during a pointer press.
- [ ] Verify accessible state announcements and focus order with a native Windows screen reader.

## Parity and motion

The named APIs run in the existing text-toggle, icon-toggle, and selection-motion scenes.
The deliberate negative selection case must continue rejecting a missing shape morph.
Skia regressions check custom path states, content clipping, state colors, borders, focus rings, and asymmetric corners across all eight styles.
Exact rendered comparisons check inherited typography and icon tint across checked, unchecked, enabled, and disabled states.

- [ ] Add Compose and Slint scenes for each supported custom style parameter.
- [ ] Add keyboard press/release and cancellation motion traces.
- [ ] Cover every size, light/dark theme, density, disabled state, long label, and large-text setting.
- [ ] Check repeated and interrupted shape/elevation transitions with expressive and reduced motion.
- [ ] Run the complete button scenes through software, Skia, and FemtoVG with required Compose references.
- [ ] Inspect the output and record evidence for each API before promoting its inventory entry to done.
