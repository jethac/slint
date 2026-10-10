<!--
Copyright © SixtyFPS GmbH <info@slint.dev>
SPDX-License-Identifier: MIT
-->

# Material Expressive Preview

This local preview builds on commit `a41374b7cc` of the fork's `master` branch.
Fieldnotes combines the port's components into a small day planner with task editing, validation, completion filters, and appearance settings.
It uses a navigation rail on wide windows and bottom navigation on narrow windows.
Tasks stay in memory until the window closes.

## Changes in This Preview

- App bars and bottom navigation keep their intended height in expanding layouts.
- Pinned app bars let action tooltips paint below the bar and above following content.
- Dialogs expose dismissal so applications can clear their editor state after an outside click or Escape.
- Generator checks accept Windows line endings and use portable source paths in the inventory.
- Slider press actions retain the computed thumb coordinates in both orientations.
- Fieldnotes combines the components in a working application instead of isolated samples.
- Named toggle buttons expose checkbox semantics and share activation behavior across pointer, keyboard, and accessibility input.

## Run the Preview

```sh
cargo run --release --manifest-path ui-libraries/material/Cargo.toml -p material-fieldnotes
```

Fieldnotes selects Skia for desktop rendering and requires Direct3D 12 on Windows.
The software renderer remains available to the headless regression tests.
The existing component gallery remains available as `material-gallery` in the same workspace.

## Verification

Run the local PowerShell workflow from the repository root:

```powershell
./ui-libraries/material/scripts/check-preview.ps1
```

The workflow checks both generators, runs their regression tests, and renders the gallery and Fieldnotes through the testing backend.
Images are saved under `target/material-preview/`.
Use `-TokenSource <directory>` for an existing checkout of the pinned AndroidX Material3 sources.
Add `-Parity` to require committed Compose references for every Material screenshot case on Windows.

The verified local run supplies `-TokenSource target/androidx-material3-source`.
All 247 Kotlin source files in that directory were checked against the pinned Git blob hashes.

Linux also supports the headless FemtoVG parity driver:

```sh
bash ui-libraries/material/scripts/check-parity.sh
```

Both parity workflows split compilation into six shards and run tests in fresh batches of forty.
This follows the existing CI strategy for limiting compiler memory and retained renderer state.
The generated screenshot executable omits Rust debug symbols while keeping Slint element debug information for traces.
The Linux script also accepts an existing test executable as its sole argument.
It requires Python 3, Xvfb, and the Linux renderer dependencies listed in the Material CI workflow.
When running through WSL, keep the checkout and Cargo target directory on the Linux filesystem.
Compiling this generated suite through a mounted Windows drive is substantially slower.

## Verified Results

The local PowerShell workflow passes on Windows.

| Check | Result |
| --- | --- |
| Token generator | 21 regression tests pass; generated files match the pinned source |
| Scene generator | 2 regression tests pass; generated scenes match their definitions |
| Fieldnotes | 3 tests pass, including task identity, validation, keyboard input, accessible actions, layout, phone scrolling, and tooltip clipping |
| Native Fieldnotes build | Release build runs on Windows with Skia/WGPU and Direct3D 12; the local binary is `target/material-preview/Fieldnotes.exe` |
| Component gallery | 1 render test passes at phone and desktop sizes with Skia |
| Full interpreter suite | 956 tests pass without a case filter |
| Compose harness compilation | Passes with JDK 21 and Android SDK 36 |
| Compose slider motion reproduction | Both densities pass; PNG bytes and parsed traces match the committed references |
| Full Compose render suite | 504 tests pass across all 264 scenes; 24 scenes request density 1 only |
| Slint renderer baseline | All 264 scenes pass the harness on software, Skia, and FemtoVG; 792 renderer/scene checks run with expected failures active |

All 504 parsed reference traces match the committed data.
Of 1,654 PNG files, four differ at seven pixels in total, with a maximum channel difference of 1/255.
The committed references remain unchanged.
The audit is saved locally as `target/compose-reference-audit.json`; the Compose run log is `target/compose-all.log`.

The Linux renderer baseline combines a full interpreter run with six compiled software groups.
Its 1,062 passing tests include the 792 Compose-backed checks, component instantiation tests, legacy smoke tests, and harness helpers.
Software parity uses full-frame rendering.
All 621 Material source and scene files in the Linux verification copy match this worktree after line-ending normalization.
Logs, coverage summaries, and a renderer artifact archive are saved under `target/parity-artifacts/`.

The render tests produce seven Fieldnotes images and two gallery images.
The images were inspected for layout, wrapping, colors, and dialog presentation.
The local workflow log is `target/material-preview/check-preview.log`.
These headless checks exercise accessibility properties and actions, but do not validate a screen reader on a native platform.

The named-toggle milestone passes all 957 unfiltered interpreter tests and 27 preview checks.
The toggle regression covers all eight styles through pointer, keyboard, accessibility, and disabled activation.
Skia comparisons pass for the text-toggle, icon-toggle, and selection-motion scenes.
The deliberate toggle-selection defect is still detected.
Logs are `target/material-preview/toggle-full-interpreter.log`, `toggle-preview-check.log`, `toggle-skia-parity.log`, and `toggle-skia-negative.log`.
The first Skia run caught a generator override defect; the negative-case rerun passes after honoring that override.

Keyboard activation now follows the pinned Compose click handler: Space and Enter activate on matching release.
Focus loss and disabling a control cancel pending keyboard presses.
Both touch-area helpers share this behavior, including repeat and overlapping-key handling.
The full interpreter suite passes 959 tests; its log is `target/material-preview/keyboard-full-interpreter.log`.

## Scope and Remaining Work

All eight toggle styles now expose `toggle-on-click` and `checked-changed`.
Controlled activation requests the opposite state without overwriting the application's `checked` binding.
Default activation updates local state before invoking the state callback and `clicked`, once each.
Both connected-group orientations expose `selection-on-click` and preserve application-owned selection in controlled mode.
Multi-select groups update model rows instead of replacing delegate bindings.
Connected accessibility actions use the shared activation handler and reject disabled items.
The gallery's application-owned theme, motion, and shape selections use controlled mode.
All 965 interpreter tests, ten Skia checks with required references, the native gallery check, and three Fieldnotes regressions pass.
Generator drift and formatting checks pass.
Evidence logs use the `target/material-preview/controlled-selection-` prefix, including `full-interpreter-final.log`, `render.log`, `native-gallery.log`, and `native-fieldnotes.log`.
Standard ButtonGroup and legacy SegmentedButton selection still require their complete API and behavior audits.

Their selection requests now preserve application-owned index bindings.
Standard ButtonGroup row and overflow activation share one guarded handler, including deselection.
Default mode retains synchronization between programmatic model checked flags and the selected index.
Overflow activation resolves its item before dismissing the popup, preserving the correct delegate index.
Toggle items expose checkbox semantics; overflow menu items expose enabled accessibility actions.
SegmentedButton supports controlled selection and rejects disabled pointer, keyboard, and accessibility activation.
All 967 interpreter tests, ten required-reference Skia checks, and the native gallery check pass.
Skia checks cover standard groups, weighted sizing, overflow, selection motion, and deliberate negative cases.
Generator drift and formatting checks pass.
Evidence logs use the `target/material-preview/standard-selection-` prefix, including `full-interpreter-final.log`, `final-render.log`, and `native-gallery-final.log`.

The reviewed spike inventory contained 61 done, 72 partial, and 17 missing entries.
The completion audit corrected extension function names that had been recorded as receiver types.
The current inventory contains 60 done, 82 partial, and 7 missing entries.
Affected entries retain their implementation notes and remain partial until their API and behavior are audited.
Entries include component variants and Compose functions, so these counts do not represent distinct Slint widgets.
The missing entries include carousel and wavy progress families and drawer-sheet APIs.
Eight named toggle APIs are implemented and remain partial while custom styling and renderer verification continue.

Scoped typography and content colors now pass through reusable components and repeaters.
`ProvideTextStyle` merges font fields with the enclosing style; `ProvideContentColor` supplies descendant text and icon colors.
Text buttons provide label typography, while icon buttons preserve their enclosing text style.
Explicit child bindings override these defaults, and context updates remain reactive.
All 963 interpreter tests, the native inheritance regression, and the full compiler suite pass.
Eight Skia checks pass, including exact inherited-child render comparisons and required Compose references for existing toggle scenes.
Logs use the `target/material-preview/inherited-context-` prefix.
Full Compose TextStyle fields, adaptive behavior, and provider reference scenes remain required completion work.

The harness contains 264 scenes, including 79 deliberate negative cases and 53 cases with expected-failure declarations.
An expected failure records a known difference; it does not establish visual parity.
Keep those differences visible when deciding whether a component is ready for an application.

Tooltips remain in-tree overlays.
The preview fixes pinned app bars, but other clipped containers, including collapsing app bars, still need an overlay solution.

Further work should prioritize interaction, accessibility, and renderer gaps in the components an application uses.
The generated inventory remains the source for broader port coverage.
No inventory entries are promoted to done by the preview or the receiver-name correction.
