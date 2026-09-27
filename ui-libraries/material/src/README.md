
# Material Design 3 component set for Slint

[![Gallery image](https://material.slint.dev/tablet-material.webp)](examples/gallery)

Welcome to the official Material Design 3 component set for [Slint](https://slint.dev). These UI components adhere to [Material Design 3 guidelines](https://m3.material.io/).
The components are intended to use for development of user interfaces with Slint for Android apps, touch friendly interfaces for embedded devices and even for desktop application development.
Contributions and feedback from the community are welcome.

Material Components 1.1 requires Slint 1.18 or newer.
See the [changelog](https://material.slint.dev/changelog/) for new features, fixes, and migration notes.

## Demos

[WebAssembly build in the web browser](https://material.slint.dev/wasm/)

Download and install the [APK for android](https://material.slint.dev/apk/slint_material.apk)

## Documentation

View the documentation online at https://material.slint.dev/getting-started/

### Get Started

Clone one of our Material Components for Slint templates and follow the instruction from their README:

 - Rust: https://github.com/slint-ui/material-rust-template
 - C++: https://github.com/slint-ui/material-cpp-template
 - Node.js/Deno: https://github.com/slint-ui/material-nodejs-template
 - Python: https://github.com/slint-ui/material-python-template

## Token pipeline

The Material Design tokens under `src/ui/styling/generated/` are generated from
the androidx Material 3 sources pinned in `TOKENS_SOURCE`.
To regenerate them or to bump the pin, run from this directory:

```sh
cargo run -p material-token-generator
```

`cargo run -p material-token-generator -- check` verifies that the committed
files match what the generator produces at the pinned commit; CI runs it.
`PARITY.md` is the generated component/token parity inventory; its statuses
live in `PARITY_STATUS.json`.
