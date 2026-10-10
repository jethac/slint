// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use i_slint_backend_testing::{AccessibleRole, ElementHandle};
use i_slint_core::api::LogicalPosition;
use i_slint_core::platform::{PointerEventButton, WindowEvent};
use slint_interpreter::{ComponentHandle, Value};
use std::error::Error;

#[test]
fn toggle_buttons_accessibility_activation() -> Result<(), Box<dyn Error>> {
    let instance = create_fixture("TestCase")?;
    check_activation(&instance)?;
    Ok(())
}

fn create_fixture(name: &str) -> Result<slint_interpreter::ComponentInstance, Box<dyn Error>> {
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
    let instance = result.component(name).unwrap().create()?;
    instance.show()?;
    Ok(instance)
}

fn check_activation(instance: &slint_interpreter::ComponentInstance) -> Result<(), Box<dyn Error>> {
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
        let control = ElementHandle::find_by_accessible_label(instance, label)
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
        let control = ElementHandle::find_by_accessible_label(instance, label)
            .find(|element| element.accessible_role() == Some(AccessibleRole::Checkbox))
            .unwrap();
        control.invoke_accessible_default_action();
        assert_eq!(control.accessible_checked(), Some(false));
    }
    assert_eq!(instance.get_property("clicks")?, Value::Number(32.0));
    assert_eq!(instance.get_property("test")?, Value::Bool(true));
    Ok(())
}

#[test]
fn keyboard_press_cancels_when_focus_leaves_or_control_is_disabled() -> Result<(), Box<dyn Error>> {
    let instance = create_fixture("TestCase")?;
    let key = |text: &str, pressed: bool| {
        instance.window().dispatch_event(if pressed {
            WindowEvent::KeyPressed { text: text.into() }
        } else {
            WindowEvent::KeyReleased { text: text.into() }
        });
    };
    key("\t", true);
    key("\t", false);
    key(" ", true);
    assert_eq!(instance.get_property("first-pressed")?, Value::Bool(true));
    assert_eq!(instance.get_property("clicks")?, Value::Number(0.0));
    key("\t", true);
    key("\t", false);
    assert_eq!(instance.get_property("first-pressed")?, Value::Bool(false));
    key(" ", false);
    for _ in 0..7 {
        key("\t", true);
        key("\t", false);
    }
    key(" ", true);
    key(" ", false);
    assert_eq!(instance.get_property("first-checked")?, Value::Bool(true));
    assert_eq!(instance.get_property("clicks")?, Value::Number(1.0));
    key(" ", true);
    assert_eq!(instance.get_property("first-pressed")?, Value::Bool(true));
    instance.set_property("controls-enabled", Value::Bool(false))?;
    assert_eq!(instance.get_property("first-pressed")?, Value::Bool(false));
    key(" ", false);
    key("\n", true);
    key("\n", false);
    assert_eq!(instance.get_property("clicks")?, Value::Number(1.0));
    instance.set_property("controls-enabled", Value::Bool(true))?;
    for _ in 0..8 {
        if instance.get_property("first-focused")? == Value::Bool(true) {
            break;
        }
        key("\t", true);
        key("\t", false);
    }
    assert_eq!(instance.get_property("first-focused")?, Value::Bool(true));
    key("\n", true);
    key("\n", false);
    assert_eq!(instance.get_property("first-checked")?, Value::Bool(false));
    assert_eq!(instance.get_property("first-pressed")?, Value::Bool(false));
    assert_eq!(instance.get_property("clicks")?, Value::Number(2.0));
    key(" ", true);
    key(" ", true);
    key("\n", false);
    assert_eq!(instance.get_property("clicks")?, Value::Number(2.0));
    assert_eq!(instance.get_property("first-pressed")?, Value::Bool(true));
    key("\n", true);
    key("\n", false);
    assert_eq!(instance.get_property("clicks")?, Value::Number(3.0));
    assert_eq!(instance.get_property("first-pressed")?, Value::Bool(true));
    key(" ", false);
    assert_eq!(instance.get_property("clicks")?, Value::Number(4.0));
    assert_eq!(instance.get_property("first-pressed")?, Value::Bool(false));
    assert_eq!(instance.get_property("first-checked")?, Value::Bool(false));
    Ok(())
}

#[test]
fn extended_touch_area_uses_release_activation() -> Result<(), Box<dyn Error>> {
    let instance = create_fixture("ExtendedCase")?;
    instance.window().dispatch_event(WindowEvent::KeyPressed { text: "\t".into() });
    instance.window().dispatch_event(WindowEvent::KeyReleased { text: "\t".into() });
    instance.window().dispatch_event(WindowEvent::KeyPressed { text: " ".into() });
    assert_eq!(instance.get_property("active")?, Value::Bool(true));
    assert_eq!(instance.get_property("clicks")?, Value::Number(0.0));
    instance.window().dispatch_event(WindowEvent::KeyReleased { text: " ".into() });
    assert_eq!(instance.get_property("active")?, Value::Bool(false));
    assert_eq!(instance.get_property("clicks")?, Value::Number(1.0));
    instance.window().dispatch_event(WindowEvent::KeyPressed { text: "\n".into() });
    instance.set_property("controls-enabled", Value::Bool(false))?;
    assert_eq!(instance.get_property("active")?, Value::Bool(false));
    instance.window().dispatch_event(WindowEvent::KeyReleased { text: "\n".into() });
    assert_eq!(instance.get_property("clicks")?, Value::Number(1.0));
    Ok(())
}

#[test]
fn custom_shapes_clip_pointer_activation_in_every_toggle_style() -> Result<(), Box<dyn Error>> {
    let instance = create_fixture("ShapeCase")?;
    for (index, label) in [
        "Filled",
        "Tonal",
        "Elevated",
        "Outlined",
        "Icon",
        "Filled icon",
        "Tonal icon",
        "Outlined icon",
    ]
    .into_iter()
    .enumerate()
    {
        let control = ElementHandle::find_by_accessible_label(&instance, label)
            .find(|element| element.accessible_role() == Some(AccessibleRole::Checkbox))
            .unwrap();
        let origin = control.absolute_position();
        let size = control.size();
        let visual_width = if label.to_lowercase().contains("icon") { 40.0 } else { size.width };
        let visual_x = origin.x + (size.width - visual_width) / 2.0;
        let visual_y = origin.y + (size.height - 40.0) / 2.0;
        for (x, y, checked) in [(0.1, 0.35, false), (0.5, 0.5, true)] {
            let position = LogicalPosition::new(visual_x + visual_width * x, visual_y + 40.0 * y);
            instance.window().dispatch_event(WindowEvent::PointerMoved { position });
            instance.window().dispatch_event(WindowEvent::PointerPressed {
                position,
                button: PointerEventButton::Left,
            });
            instance.window().dispatch_event(WindowEvent::PointerReleased {
                position,
                button: PointerEventButton::Left,
            });
            assert_eq!(control.accessible_checked(), Some(checked), "shape hit test: {label}");
        }
        assert_eq!(instance.get_property("clicks")?, Value::Number((index + 1) as f64));
    }
    Ok(())
}
