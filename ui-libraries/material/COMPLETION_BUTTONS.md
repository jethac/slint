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
The Slint styles currently resolve most of these internally.
The pinned [IconButton.kt](https://github.com/androidx/androidx/blob/23327507f7fc7d5b19d65fec4b090f60c970079b/compose/material3/material3/src/commonMain/kotlin/androidx/compose/material3/IconButton.kt) also has static-shape and interaction-shape overloads.
The existing round/square selector does not cover arbitrary shape overrides.

| Slint toggle | Existing style | Additional API checks |
| --- | --- | --- |
| `ToggleButton` | `FilledButton` | Leading/trailing/vertical padding exists; arbitrary border, content, colors, shapes, and elevation remain open. |
| `FilledTonalToggleButton` | `TonalButton` | Add padding overrides and verify all custom style parameters. |
| `ElevatedToggleButton` | `ElevatedButton` | Add padding overrides; verify custom elevation in every interaction state. |
| `OutlinedToggleButton` | `OutlineButton` | Add padding overrides; support custom borders without removing the selected default behavior. |
| `IconToggleButton` | `IconButton` | Cover both pinned color families and static/interaction shape overloads. |
| `FilledIconToggleButton` | `FilledIconButton` | Cover custom state colors, content, and both shape overloads. |
| `FilledTonalIconToggleButton` | `TonalIconButton` | Cover custom state colors, content, and both shape overloads. |
| `OutlinedIconToggleButton` | `OutlineIconButton` | Cover custom borders, state colors, content, and both shape overloads. |

- [ ] Expose enabled, disabled, checked, and unchecked container/content color overrides for each style.
- [ ] Expose resting, pressed, and checked shapes, including per-corner geometry and interruption behavior.
- [ ] Expose interaction elevation overrides where the pinned API supports them.
- [ ] Expose border color and width overrides, including disabled and checked states.
- [ ] Provide padding overrides for all four text-toggle styles.
- [ ] Support arbitrary child content and document how it inherits typography and content color.
- [ ] Document the mapping from Compose's externally owned `checked`/`onCheckedChange` to Slint properties and callbacks.
- [ ] Verify controlled state bindings and single-selection groups without replacing their bindings during activation.
- [ ] Compare every default size, icon spacing, padding, typography, and minimum touch target against the pinned helpers.

## Input and accessibility

The interpreter fixture verifies checkbox roles, names, activation, and disabled accessibility actions across all eight styles.
Keyboard activation must follow the pinned [Clickable.kt](https://github.com/androidx/androidx/blob/23327507f7fc7d5b19d65fec4b090f60c970079b/compose/foundation/foundation/src/commonMain/kotlin/androidx/compose/foundation/Clickable.kt).
Key-down creates a press; matching key-up activates; focus loss cancels pending keys.

- [x] Verify pointer, keyboard, and accessibility activation for all eight toggle styles.
- [x] Reject disabled accessibility activation.
- [x] Verify keyboard repeat, overlapping Space/Enter presses, mismatched releases, focus loss, and disabling during a press.
- [x] Verify the shared keyboard contract through the public `ExtendedTouchArea` API.
- [ ] Verify touch cancellation, drag outside, shape-clipped hit testing, and disabling during a pointer press.
- [ ] Verify accessible state announcements and focus order with a native Windows screen reader.

## Parity and motion

The named APIs run in the existing text-toggle, icon-toggle, and selection-motion scenes.
The deliberate negative selection case must continue rejecting a missing shape morph.

- [ ] Add Compose and Slint scenes for each supported custom style parameter.
- [ ] Add keyboard press/release and cancellation motion traces.
- [ ] Cover every size, light/dark theme, density, disabled state, long label, and large-text setting.
- [ ] Check repeated and interrupted shape/elevation transitions with expressive and reduced motion.
- [ ] Run the complete button scenes through software, Skia, and FemtoVG with required Compose references.
- [ ] Inspect the output and record evidence for each API before promoting its inventory entry to done.
