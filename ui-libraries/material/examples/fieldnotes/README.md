<!--
Copyright © SixtyFPS GmbH <info@slint.dev>
SPDX-License-Identifier: MIT
-->

# Fieldnotes

Fieldnotes is a small day planner built with the Material 3 Expressive component library.
It combines navigation, filter chips, segmented lists, task editing, validation, and appearance settings in one application.
Tasks stay in memory until the window closes.

Run from the repository root:

```sh
cargo run --release --manifest-path ui-libraries/material/Cargo.toml -p material-fieldnotes
```

The desktop application uses the Skia renderer and requires Direct3D on Windows.
Use the release build for interactive previews and performance checks.
The application uses a navigation rail at widths of 720 logical pixels or more and bottom navigation below that width.
Use Tab to move between controls and Enter or Space to activate buttons and chips.
The task editor accepts Enter to save and Escape to dismiss.
Appearance settings control dark colors and reduced motion.

## Verification

```sh
cargo test --manifest-path ui-libraries/material/Cargo.toml -p material-fieldnotes
```

Tests exercise task identity across filtering, completion, editing, validation, keyboard entry, and accessible labels.
They also render the application with the software renderer at desktop and phone sizes in light and dark colors.
The appearance tooltip is checked below the app bar to detect ancestor clipping and content overlap.
Set `FIELDNOTES_SCREENSHOTS` to a directory to save those renders as PNG files.

The gallery has a similar `MATERIAL_GALLERY_SCREENSHOTS` setting for its Skia preview test.
Run `ui-libraries/material/scripts/check-preview.ps1` from PowerShell to check both examples and their generated resources.
Add `-Parity` to run all Material screenshot cases on the renderers supported by the Windows driver.
Use `-TokenSource <directory>` to supply an existing checkout of the pinned AndroidX Material3 source directory.

## Motion captures

Export timed application frames for the separate landing page:

```powershell
$env:FIELDNOTES_MOTION = 'target/material-preview/motion'
cargo test --config 'profile.dev.package.i-slint-renderer-software.opt-level=3' --config 'profile.dev.package.image.opt-level=3' --config 'profile.dev.package.png.opt-level=3' --manifest-path ui-libraries/material/Cargo.toml -p material-fieldnotes export_landing_page_motion -- --ignored
```

The exporter renders 16 seconds at a controlled 60 Hz clock, with reduced motion off and on.
It opens the editor, types one character at a time, saves the task, then reopens and dismisses the editor.
It uses the headless software renderer and checks that saving and reopening preserve the entered title.
These frames demonstrate behavior rather than GPU performance.
