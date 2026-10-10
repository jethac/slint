// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use i_slint_backend_testing::{AccessibleRole, ElementHandle};
use i_slint_core::api::LogicalPosition;
use i_slint_core::platform::{PointerEventButton, WindowEvent};
use slint_interpreter::{ComponentHandle, Value};
use std::error::Error;

#[test]
fn toggle_buttons_accessibility_activation() -> Result<(), Box<dyn Error>> {
    i_slint_backend_testing::init_no_event_loop();
    i_slint_backend_testing::configure_test_fonts();
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("material_button_accessibility.slint");
    let mut compiler = slint_interpreter::Compiler::default();
    compiler.compiler_configuration(i_slint_core::InternalToken).debug_info = true;
    let result =
        spin_on::spin_on(compiler.build_from_source(std::fs::read_to_string(&path)?, path));
    slint_interpreter::print_diagnostics(&result.diagnostics().collect::<Vec<_>>());
    assert!(!result.has_errors());
    let instance = result.component("TestCase").unwrap().create()?;
    instance.show()?;
    let labels = [
        "Filled",
        "Tonal",
        "Elevated",
        "Outlined",
        "Icon",
        "Filled icon",
        "Tonal icon",
        "Outlined icon",
    ];
    for (index, label) in labels.into_iter().enumerate() {
        let control = ElementHandle::find_by_accessible_label(&instance, label)
            .find(|element| element.accessible_role() == Some(AccessibleRole::Checkbox))
            .expect("toggle controls must expose the checkbox role");
        assert_eq!(control.accessible_checked(), Some(false));
        control.invoke_accessible_default_action();
        assert_eq!(control.accessible_checked(), Some(true));
        control.invoke_accessible_default_action();
        assert_eq!(control.accessible_checked(), Some(false));
        let origin = control.absolute_position();
        let size = control.size();
        let position =
            LogicalPosition::new(origin.x + size.width / 2.0, origin.y + size.height / 2.0);
        instance.window().dispatch_event(WindowEvent::PointerMoved { position });
        instance.window().dispatch_event(WindowEvent::PointerPressed {
            position,
            button: PointerEventButton::Left,
        });
        instance.window().dispatch_event(WindowEvent::PointerReleased {
            position,
            button: PointerEventButton::Left,
        });
        assert_eq!(control.accessible_checked(), Some(true), "pointer activation: {label}");
        instance.window().dispatch_event(WindowEvent::KeyPressed { text: "\t".into() });
        instance.window().dispatch_event(WindowEvent::KeyReleased { text: "\t".into() });
        instance.window().dispatch_event(WindowEvent::KeyPressed { text: " ".into() });
        instance.window().dispatch_event(WindowEvent::KeyReleased { text: " ".into() });
        assert_eq!(control.accessible_checked(), Some(false), "keyboard activation: {label}");
        assert_eq!(instance.get_property("clicks")?, Value::Number((index + 1) as f64 * 4.0));
    }
    instance.set_property("controls-enabled", Value::Bool(false))?;
    for label in labels {
        let control = ElementHandle::find_by_accessible_label(&instance, label)
            .find(|element| element.accessible_role() == Some(AccessibleRole::Checkbox))
            .unwrap();
        control.invoke_accessible_default_action();
        assert_eq!(control.accessible_checked(), Some(false));
    }
    assert_eq!(instance.get_property("clicks")?, Value::Number(32.0));
    assert_eq!(instance.get_property("test")?, Value::Bool(true));
    Ok(())
}
