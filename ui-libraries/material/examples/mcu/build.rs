// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

fn main() {
    let config = slint_build::CompilerConfiguration::new()
        .embed_resources(slint_build::EmbedResourcesKind::EmbedForSoftwareRenderer)
        // This crate enables Slint's `embedded-vector-fonts` feature, so
        // non-constant font bindings (the Material theme's bound family and
        // style-driven weight/size) embed vector font data instead of failing.
        .exclude_vector_fonts(false);
    slint_build::compile_with_config("ui/main.slint", config).unwrap();
    slint_build::print_rustc_flags().unwrap();
}
