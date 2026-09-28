// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! A dynamic `font-variation-settings` binding makes the compiler embed the
//! font as vector data. Without `std` or `embedded-vector-fonts` there is no
//! rasterizer for it, so this crate must fail to compile with:
//! "the compiled UI embeds vector font data, but this build has no vector
//! font rasterizer"

#![no_std]

slint::slint! {
    import "../../../../tests/screenshots/fonts/NotoSans-Regular.ttf";

    export component Main inherits Window {
        in property <float> w;
        Text {
            font-family: "Noto Sans";
            font-variation-settings: [{ tag: "wght", value: w }];
            text: "sweep";
        }
    }
}
