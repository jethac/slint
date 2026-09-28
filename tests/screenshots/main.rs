// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#[cfg(test)]
pub mod testing;

/// Layered pixel/property-trace comparison against Compose references; used
/// by every driver for `//PARITY=` cases in `cases/material/`.
#[cfg(test)]
pub mod parity;

/// Compilation helpers shared by the interpreter-based drivers.
#[cfg(all(
    test,
    any(feature = "skia", feature = "femtovg", feature = "anyrender")
))]
pub mod interpreter;

#[cfg(all(test, feature = "software"))]
pub mod software;

#[cfg(all(test, feature = "skia"))]
pub mod skia;

#[cfg(all(test, feature = "femtovg"))]
pub mod femtovg;

#[cfg(all(test, feature = "anyrender"))]
pub mod anyrender;

#[cfg(test)]
include!(concat!(env!("OUT_DIR"), "/generated.rs"));

fn main() {
    println!("Nothing to see here, please run me through cargo test :)");
}
