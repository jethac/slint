// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

fn main() {
    slint_build::compile_with_config(
        "ui/fieldnotes.slint",
        slint_build::CompilerConfiguration::new()
            .with_debug_info(std::env::var("PROFILE").as_deref() != Ok("release")),
    )
    .expect("Fieldnotes UI compilation failed");
}
