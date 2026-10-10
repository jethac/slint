// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use i_slint_backend_testing::{AccessibleRole, ElementHandle};
use i_slint_core::api::LogicalPosition;
use i_slint_core::model::Model;
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
    load_fixture(name)
}

fn load_fixture(name: &str) -> Result<slint_interpreter::ComponentInstance, Box<dyn Error>> {
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

fn pointer_click(instance: &slint_interpreter::ComponentInstance, control: &ElementHandle) {
    let origin = control.absolute_position();
    let size = control.size();
    let position = LogicalPosition::new(origin.x + size.width / 2.0, origin.y + size.height / 2.0);
    instance.window().dispatch_event(WindowEvent::PointerMoved { position });
    instance
        .window()
        .dispatch_event(WindowEvent::PointerPressed { position, button: PointerEventButton::Left });
    instance.window().dispatch_event(WindowEvent::PointerReleased {
        position,
        button: PointerEventButton::Left,
    });
}

fn key(instance: &slint_interpreter::ComponentInstance, text: &str, pressed: bool) {
    instance.window().dispatch_event(if pressed {
        WindowEvent::KeyPressed { text: text.into() }
    } else {
        WindowEvent::KeyReleased { text: text.into() }
    });
}

#[test]
fn controlled_toggle_requests_preserve_bindings_in_every_style() -> Result<(), Box<dyn Error>> {
    let instance = create_fixture("ControlledCase")?;
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
        for _ in 0..2 {
            control.invoke_accessible_default_action();
            assert_eq!(control.accessible_checked(), Some(false), "{label}: rejected request");
            assert_eq!(instance.get_property("last-request")?, Value::Bool(true));
        }
        instance.set_property("selected", true.into())?;
        assert_eq!(control.accessible_checked(), Some(true), "{label}: binding after activation");
        control.invoke_accessible_default_action();
        assert_eq!(instance.get_property("last-request")?, Value::Bool(false));
        assert_eq!(control.accessible_checked(), Some(true));
        instance.set_property("selected", false.into())?;
        assert_eq!(control.accessible_checked(), Some(false));
        pointer_click(&instance, &control);
        assert_eq!(control.accessible_checked(), Some(false));
        key(&instance, "\t", true);
        key(&instance, "\t", false);
        key(&instance, " ", true);
        assert_eq!(instance.get_property("requests")?, Value::Number((index * 6 + 4) as f64));
        key(&instance, " ", false);
        assert_eq!(control.accessible_checked(), Some(false));
        assert_eq!(instance.get_property("last-request")?, Value::Bool(true));
        instance.set_property("accept-requests", true.into())?;
        control.invoke_accessible_default_action();
        assert_eq!(control.accessible_checked(), Some(true), "{label}: accepted request");
        instance.set_property("selected", false.into())?;
        assert_eq!(control.accessible_checked(), Some(false), "{label}: binding after callback");
        instance.set_property("accept-requests", false.into())?;
        assert_eq!(instance.get_property("requests")?, Value::Number(((index + 1) * 6) as f64));
        assert_eq!(instance.get_property("clicks")?, instance.get_property("requests")?);
        assert_eq!(instance.get_property("ordered")?, Value::Bool(true));
    }
    instance.set_property("controls-enabled", false.into())?;
    for label in [
        "Filled",
        "Tonal",
        "Elevated",
        "Outlined",
        "Icon",
        "Filled icon",
        "Tonal icon",
        "Outlined icon",
    ] {
        let control = ElementHandle::find_by_accessible_label(&instance, label)
            .find(|element| element.accessible_role() == Some(AccessibleRole::Checkbox))
            .unwrap();
        control.invoke_accessible_default_action();
    }
    assert_eq!(instance.get_property("requests")?, Value::Number(48.0));
    Ok(())
}

fn set_first_model_checked(
    instance: &slint_interpreter::ComponentInstance,
    checked: bool,
) -> Result<(), Box<dyn Error>> {
    let Value::Model(model) = instance.get_property("items")? else {
        panic!("items must be a model")
    };
    let Value::Struct(mut row) = model.row_data(0).unwrap() else {
        panic!("item must be a struct")
    };
    row.set_field("checked".into(), checked.into());
    model.set_row_data(0, row.into());
    Ok(())
}

#[test]
fn connected_groups_preserve_selection_and_model_bindings() -> Result<(), Box<dyn Error>> {
    i_slint_backend_testing::init_no_event_loop();
    i_slint_backend_testing::configure_test_fonts();
    for name in ["HorizontalGroupCase", "VerticalGroupCase"] {
        let instance = load_fixture(name)?;
        let radio = |label: &str| {
            ElementHandle::find_by_accessible_label(&instance, label)
                .find(|element| element.accessible_role() == Some(AccessibleRole::RadioButton))
                .unwrap()
        };
        let first = radio("First");
        let second = radio("Second");
        radio("Disabled").invoke_accessible_default_action();
        assert_eq!(instance.get_property("requests")?, Value::Number(0.0));
        second.invoke_accessible_default_action();
        assert_eq!(instance.get_property("requested-index")?, Value::Number(1.0));
        assert_eq!(instance.get_property("observed-selection")?, Value::Number(0.0));
        assert_eq!(first.accessible_checked(), Some(true));
        assert_eq!(second.accessible_checked(), Some(false));
        instance.set_property("selected", 1.into())?;
        assert_eq!(first.accessible_checked(), Some(false));
        assert_eq!(second.accessible_checked(), Some(true));
        second.invoke_accessible_default_action();
        assert_eq!(second.accessible_checked(), Some(true));
        instance.set_property("selected", 0.into())?;
        instance.set_property("accept-requests", true.into())?;
        second.invoke_accessible_default_action();
        assert_eq!(instance.get_property("selected")?, Value::Number(1.0));
        assert_eq!(second.accessible_checked(), Some(true));
        instance.set_property("selected", 0.into())?;
        assert_eq!(first.accessible_checked(), Some(true));
        assert_eq!(second.accessible_checked(), Some(false));

        instance.set_property("accept-requests", false.into())?;
        instance.set_property("multiple", true.into())?;
        let checkbox = |label: &str| {
            ElementHandle::find_by_accessible_label(&instance, label)
                .find(|element| element.accessible_role() == Some(AccessibleRole::Checkbox))
                .unwrap()
        };
        let first = checkbox("First");
        for _ in 0..2 {
            first.invoke_accessible_default_action();
            assert_eq!(first.accessible_checked(), Some(false), "{name}: rejected request");
            assert_eq!(instance.get_property("requested-checked")?, Value::Bool(true));
        }
        set_first_model_checked(&instance, true)?;
        assert_eq!(
            first.accessible_checked(),
            Some(true),
            "{name}: model binding after activation"
        );
        first.invoke_accessible_default_action();
        assert_eq!(instance.get_property("requested-checked")?, Value::Bool(false));
        assert_eq!(first.accessible_checked(), Some(true));
        set_first_model_checked(&instance, false)?;
        assert_eq!(first.accessible_checked(), Some(false));
        instance.set_property("accept-requests", true.into())?;
        first.invoke_accessible_default_action();
        assert_eq!(instance.get_property("model-checked")?, Value::Bool(true));
        assert_eq!(first.accessible_checked(), Some(true));
        set_first_model_checked(&instance, false)?;
        assert_eq!(first.accessible_checked(), Some(false), "{name}: model binding after callback");
        checkbox("Disabled").invoke_accessible_default_action();
        assert_eq!(instance.get_property("requests")?, Value::Number(7.0));

        instance.set_property("accept-requests", false.into())?;
        instance.set_property("automatic", true.into())?;
        first.invoke_accessible_default_action();
        assert_eq!(instance.get_property("model-checked")?, Value::Bool(true));
        set_first_model_checked(&instance, false)?;
        assert_eq!(
            first.accessible_checked(),
            Some(false),
            "{name}: automatic mode preserves row binding"
        );
        instance.hide()?;
        let input = load_fixture(name)?;
        let second = ElementHandle::find_by_accessible_label(&input, "Second")
            .find(|element| element.accessible_role() == Some(AccessibleRole::RadioButton))
            .unwrap();
        pointer_click(&input, &second);
        assert_eq!(input.get_property("requested-index")?, Value::Number(1.0));
        assert_eq!(input.get_property("observed-selection")?, Value::Number(0.0));
        key(&input, "\t", true);
        key(&input, "\t", false);
        key(&input, " ", true);
        assert_eq!(input.get_property("requests")?, Value::Number(1.0));
        key(&input, " ", false);
        assert_eq!(input.get_property("requested-index")?, Value::Number(0.0));
        key(&input, "\t", true);
        key(&input, "\t", false);
        key(&input, "\n", true);
        key(&input, "\n", false);
        assert_eq!(input.get_property("requests")?, Value::Number(3.0));
        assert_eq!(input.get_property("requested-index")?, Value::Number(1.0));
        assert_eq!(second.accessible_checked(), Some(false));
        input.set_property("accept-requests", true.into())?;
        key(&input, " ", true);
        key(&input, " ", false);
        assert_eq!(second.accessible_checked(), Some(true));
        input.set_property("selected", 0.into())?;
        assert_eq!(second.accessible_checked(), Some(false));
        input.set_property("accept-requests", false.into())?;
        input.set_property("automatic", true.into())?;
        key(&input, " ", true);
        key(&input, " ", false);
        assert_eq!(input.get_property("observed-selection")?, Value::Number(1.0));
        assert_eq!(input.get_property("selected")?, Value::Number(0.0));
        assert_eq!(second.accessible_checked(), Some(true));
        input.hide()?;
    }
    Ok(())
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
        assert_eq!(instance.get_property("requests")?, instance.get_property("clicks")?);
        assert_eq!(instance.get_property("callback-state-matches")?, Value::Bool(true));
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
    assert_eq!(instance.get_property("requests")?, Value::Number(32.0));
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
