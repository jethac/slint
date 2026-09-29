// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

// Compile-only sample: `cargo check` verifies the generated Material UI
// compiles for bare-metal targets. There is no board runtime here — boards
// wire the window up through mcu-board-support like the other demos.
#![no_std]

slint::include_modules!();
