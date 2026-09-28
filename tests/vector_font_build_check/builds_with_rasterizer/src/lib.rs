// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Same UI as `fails_without_rasterizer`, but with the `embedded-vector-fonts`
//! rasterizer: the embedded vector font must compile.

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
