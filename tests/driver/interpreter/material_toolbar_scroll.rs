// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// End-to-end check of the floating toolbar's scroll wiring: a wheel scroll on
// the `Flickable` below the bar feeds `scroll-position`, which the
// `ExitAlways`-style offset and the `expand-on-scroll` hysteresis consume —
// the `tests/cases` drivers can't express this for the material library
// because the interpreter is the only driver that compiles it cheaply.

use i_slint_backend_testing as slint_testing;
use slint_interpreter::{ComponentHandle, Value};
use std::error::Error;

#[test]
fn floating_toolbar_scroll_collapse() -> Result<(), Box<dyn Error>> {
    slint_testing::init_no_event_loop();
    slint_testing::configure_test_fonts();

    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("material_toolbar_scroll.slint");
    let compiler = slint_interpreter::Compiler::default();
    let result = spin_on::spin_on(
        compiler.build_from_source(std::fs::read_to_string(&path)?, path),
    );
    slint_interpreter::print_diagnostics(&result.diagnostics().collect::<Vec<_>>());
    assert!(!result.has_errors());
    let instance = result.component("TestCase").unwrap().create().unwrap();

    use i_slint_core::api::LogicalPosition;
    use i_slint_core::platform::WindowEvent;
    let window = instance.window();

    let scroll = |delta_y: f32| {
        window.dispatch_event(WindowEvent::PointerScrolled {
            position: LogicalPosition::new(200.0, 200.0),
            delta_x: 0.0,
            delta_y,
        });
    };
    let number = |name: &str| -> f64 {
        match instance.get_property(name).unwrap() {
            Value::Number(n) => n,
            other => panic!("{name} is not a number: {other:?}"),
        }
    };
    let boolean = |name: &str| -> bool {
        match instance.get_property(name).unwrap() {
            Value::Bool(b) => b,
            other => panic!("{name} is not a bool: {other:?}"),
        }
    };

    // `bottom` exit: the limit is the rest distance from the bar's top edge to
    // the parent's bottom edge — 400 - 320 = 80.
    assert_eq!(number("offset_limit"), -80.0);

    // 50dp down: -50 of travel, and past the 40dp collapse hysteresis.
    scroll(-50.0);
    assert_eq!(number("content_y"), -50.0);
    assert!(!boolean("toolbar_expanded"));
    assert_eq!(number("toolbar_offset"), -50.0);

    // The release settle snaps to the nearer end — 62.5% collapsed means exit.
    slint_testing::mock_elapsed_time(200);
    assert_eq!(number("toolbar_offset"), -80.0);
    assert_eq!(number("collapsed_fraction"), 1.0);

    // 50dp back up re-expands through the hysteresis, mid-exit.
    scroll(50.0);
    assert_eq!(number("content_y"), 0.0);
    assert!(boolean("toolbar_expanded"));
    assert_eq!(number("toolbar_offset"), -30.0);

    // 37.5% collapsed settles back to the rest position.
    slint_testing::mock_elapsed_time(200);
    assert_eq!(number("toolbar_offset"), 0.0);
    assert_eq!(number("collapsed_fraction"), 0.0);

    Ok(())
}
