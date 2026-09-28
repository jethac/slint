// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Exposes `embedded-vector-fonts` to dependents' build scripts as
//! `DEP_SLINT_EMBEDDED_VECTOR_FONTS`, so `slint-build` skips the
//! bare-metal `exclude_vector_fonts` auto-detection when a no_std
//! rasterizer is compiled in.

fn main() {
    println!("cargo:embedded_vector_fonts={}", cfg!(feature = "embedded-vector-fonts") as u8);
}
