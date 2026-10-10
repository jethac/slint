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
fn button_elevation_uses_reference_timings_and_cancels_motion_when_disabled()
-> Result<(), Box<dyn Error>> {
    let instance = create_fixture("ElevationCase")?;
    let elevation = |index| -> Result<f64, Box<dyn Error>> {
        let Value::Model(values) = instance.get_property("elevations")? else {
            panic!("elevation model");
        };
        let Some(Value::Number(value)) = values.row_data(index) else {
            panic!("elevation row");
        };
        Ok(value)
    };
    let update = |name: &str, value: Value| -> Result<(), Box<dyn Error>> {
        instance.set_property(name, value)?;
        i_slint_backend_testing::mock_elapsed_time(1);
        Ok(())
    };
    update("reduced-motion", Value::Bool(false))?;
    for index in 0..8 {
        assert_eq!(elevation(index)?, 2.0);
        update("hover-index", Value::Number(index as f64))?;
        let start = elevation(index)?;
        i_slint_backend_testing::mock_elapsed_time(60);
        let middle = elevation(index)?;
        assert!(
            middle > start && middle < 8.0,
            "style {index}: incoming hover {start} -> {middle}"
        );
        i_slint_backend_testing::mock_elapsed_time(60);
        assert_eq!(elevation(index)?, 8.0, "style {index}: incoming hover settles at 120ms");
        update("hover-index", Value::Number(-1.0))?;
        let _ = elevation(index)?;
        i_slint_backend_testing::mock_elapsed_time(60);
        assert!((elevation(index)? - 5.0).abs() < 0.15, "style {index}: outgoing hover easing");
        i_slint_backend_testing::mock_elapsed_time(60);
        assert_eq!(elevation(index)?, 2.0, "style {index}: outgoing hover settles at 120ms");
        instance.invoke("focus-outside", &[])?;
        for _ in 0..=index {
            key(&instance, "\t", true);
            key(&instance, "\t", false);
        }
        i_slint_backend_testing::mock_elapsed_time(1);
        let _ = elevation(index)?;
        i_slint_backend_testing::mock_elapsed_time(120);
        assert_eq!(elevation(index)?, 10.0, "style {index}: incoming focus settles at 120ms");
        instance.invoke("focus-outside", &[])?;
        i_slint_backend_testing::mock_elapsed_time(1);
        let _ = elevation(index)?;
        i_slint_backend_testing::mock_elapsed_time(120);
        assert!(elevation(index)? > 2.0, "style {index}: outgoing focus remains active at 120ms");
        i_slint_backend_testing::mock_elapsed_time(30);
        assert_eq!(elevation(index)?, 2.0, "style {index}: outgoing focus settles at 150ms");
        update("press-index", Value::Number(index as f64))?;
        let _ = elevation(index)?;
        i_slint_backend_testing::mock_elapsed_time(120);
        assert_eq!(elevation(index)?, 6.0);
        update("press-index", Value::Number(-1.0))?;
        let _ = elevation(index)?;
        i_slint_backend_testing::mock_elapsed_time(120);
        assert!(elevation(index)? > 2.0, "style {index}: outgoing press remains active at 120ms");
        i_slint_backend_testing::mock_elapsed_time(30);
        assert_eq!(elevation(index)?, 2.0, "style {index}: outgoing press settles at 150ms");
        update("hover-index", Value::Number(index as f64))?;
        let _ = elevation(index)?;
        i_slint_backend_testing::mock_elapsed_time(60);
        let interrupted = elevation(index)?;
        update("press-index", Value::Number(index as f64))?;
        assert!(
            (elevation(index)? - interrupted).abs() < 0.2,
            "style {index}: retarget stays continuous"
        );
        i_slint_backend_testing::mock_elapsed_time(120);
        assert_eq!(elevation(index)?, 6.0);
        update("controls-enabled", Value::Bool(false))?;
        assert_eq!(elevation(index)?, 1.0, "style {index}: disable snaps");
        update("hover-index", Value::Number(-1.0))?;
        update("press-index", Value::Number(-1.0))?;
        update("controls-enabled", Value::Bool(true))?;
        assert_eq!(
            elevation(index)?,
            2.0,
            "style {index}: enable from unrelated disabled target snaps"
        );
        update("press-index", Value::Number(index as f64))?;
        let _ = elevation(index)?;
        i_slint_backend_testing::mock_elapsed_time(30);
        assert!(elevation(index)? < 6.0);
        update("reduced-motion", Value::Bool(true))?;
        assert_eq!(elevation(index)?, 6.0, "style {index}: reduced motion snaps an active tween");
        update("press-index", Value::Number(-1.0))?;
        assert_eq!(elevation(index)?, 2.0);
        update("reduced-motion", Value::Bool(false))?;
        assert_eq!(elevation(index)?, 2.0);
        i_slint_backend_testing::mock_elapsed_time(60);
        assert_eq!(elevation(index)?, 2.0, "style {index}: cancelled tween does not resume");
        update("disabled-elevation", Value::Number(2.0))?;
        update("press-index", Value::Number(index as f64))?;
        let _ = elevation(index)?;
        i_slint_backend_testing::mock_elapsed_time(120);
        assert_eq!(elevation(index)?, 6.0);
        update("press-index", Value::Number(-1.0))?;
        let _ = elevation(index)?;
        i_slint_backend_testing::mock_elapsed_time(60);
        assert!(elevation(index)? > 2.0);
        update("controls-enabled", Value::Bool(false))?;
        assert_eq!(
            elevation(index)?,
            2.0,
            "style {index}: disable snaps even when the target is unchanged"
        );
        update("controls-enabled", Value::Bool(true))?;
        update("disabled-elevation", Value::Number(1.0))?;
    }
    update("pressed-elevation", Value::Number(8.0))?;
    update("hover-index", Value::Number(0.0))?;
    let _ = elevation(0)?;
    i_slint_backend_testing::mock_elapsed_time(120);
    assert_eq!(elevation(0)?, 8.0);
    update("hover-index", Value::Number(-1.0))?;
    let _ = elevation(0)?;
    i_slint_backend_testing::mock_elapsed_time(120);
    assert!(
        elevation(0)? > 2.0,
        "equal pressed/hovered targets use the reference's press outgoing spec"
    );
    i_slint_backend_testing::mock_elapsed_time(30);
    assert_eq!(elevation(0)?, 2.0);
    update("resting-elevation", Value::Number(5.0))?;
    assert_eq!(elevation(0)?, 5.0, "a new baseline without a matching old interaction snaps");
    Ok(())
}

#[test]
fn button_elevation_tracks_the_latest_live_interaction() -> Result<(), Box<dyn Error>> {
    let instance = create_fixture("ElevationCase")?;
    let elevation = |index| -> Result<f64, Box<dyn Error>> {
        let Value::Model(values) = instance.get_property("elevations")? else {
            panic!("elevation model");
        };
        let Some(Value::Number(value)) = values.row_data(index) else {
            panic!("elevation row");
        };
        Ok(value)
    };
    let update = |name: &str, value: Value| -> Result<(), Box<dyn Error>> {
        instance.set_property(name, value)?;
        i_slint_backend_testing::mock_elapsed_time(1);
        Ok(())
    };
    for index in 0..8 {
        instance.invoke("focus-outside", &[])?;
        i_slint_backend_testing::mock_elapsed_time(1);
        assert_eq!(elevation(index)?, 2.0);
        update("hover-index", Value::Number(index as f64))?;
        assert_eq!(elevation(index)?, 8.0);
        for _ in 0..=index {
            key(&instance, "\t", true);
            key(&instance, "\t", false);
        }
        i_slint_backend_testing::mock_elapsed_time(1);
        assert_eq!(elevation(index)?, 10.0, "style {index}: focus supersedes hover");
        update("press-index", Value::Number(index as f64))?;
        assert_eq!(elevation(index)?, 6.0);
        update("hover-index", Value::Number(-1.0))?;
        assert_eq!(elevation(index)?, 6.0);
        update("hover-index", Value::Number(index as f64))?;
        assert_eq!(elevation(index)?, 8.0, "style {index}: new hover supersedes press");
        update("hover-index", Value::Number(-1.0))?;
        assert_eq!(elevation(index)?, 6.0, "style {index}: removal restores press");
        update("press-index", Value::Number(-1.0))?;
        assert_eq!(elevation(index)?, 10.0, "style {index}: removal restores focus");
        update("controls-enabled", Value::Bool(false))?;
        assert_eq!(elevation(index)?, 1.0);
        update("controls-enabled", Value::Bool(true))?;
        instance.invoke("focus-outside", &[])?;
        i_slint_backend_testing::mock_elapsed_time(1);
        assert_eq!(elevation(index)?, 2.0);
    }
    Ok(())
}

#[test]
fn pointer_cancellation_and_shape_release_do_not_activate_toggles() -> Result<(), Box<dyn Error>> {
    let instance = create_fixture("ShapeCase")?;
    for (case, custom_outline) in [true, false].into_iter().enumerate() {
        instance.set_property("custom-outline", custom_outline.into())?;
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
            let center =
                LogicalPosition::new(origin.x + size.width / 2.0, origin.y + size.height / 2.0);
            let visual_width =
                if label.to_lowercase().contains("icon") { 40.0 } else { size.width };
            let (x, y) = if custom_outline { (0.1, 0.35) } else { (0.03, 0.03) };
            let dead_corner = LogicalPosition::new(
                origin.x + (size.width - visual_width) / 2.0 + visual_width * x,
                origin.y + (size.height - 40.0) / 2.0 + 40.0 * y,
            );
            let expected_clicks = ((case * 8 + index) * 2) as f64;
            let press = || {
                instance.window().dispatch_event(WindowEvent::PointerMoved { position: center });
                instance.window().dispatch_event(WindowEvent::PointerPressed {
                    position: center,
                    button: PointerEventButton::Left,
                });
            };
            let release = |position| {
                instance.window().dispatch_event(WindowEvent::PointerReleased {
                    position,
                    button: PointerEventButton::Left,
                })
            };
            press();
            assert_eq!(instance.get_property("any-pressed")?, Value::Bool(true));
            release(dead_corner);
            assert_eq!(
                control.accessible_checked(),
                Some(false),
                "{label}: release outside outline"
            );
            assert_eq!(instance.get_property("any-pressed")?, Value::Bool(false));
            press();
            let outside = LogicalPosition::new(-20.0, -20.0);
            instance.window().dispatch_event(WindowEvent::PointerMoved { position: outside });
            release(outside);
            press();
            instance.window().dispatch_event(WindowEvent::PointerExited);
            release(center);
            assert_eq!(instance.get_property("clicks")?, Value::Number(expected_clicks));
            let touch = |phase| {
                instance.window().dispatch_event(WindowEvent::internal(
                    i_slint_core::platform::InternalEvent::Touch {
                        id: 1,
                        position: i_slint_core::lengths::LogicalPoint::new(center.x, center.y),
                        phase,
                    },
                ))
            };
            touch(i_slint_core::input::TouchPhase::Started);
            assert_eq!(instance.get_property("any-pressed")?, Value::Bool(true));
            touch(i_slint_core::input::TouchPhase::Cancelled);
            assert_eq!(instance.get_property("any-pressed")?, Value::Bool(false));
            assert_eq!(control.accessible_checked(), Some(false), "{label}: cancelled touch");
            assert_eq!(instance.get_property("clicks")?, Value::Number(expected_clicks));
            press();
            instance.set_property("controls-enabled", false.into())?;
            assert_eq!(
                instance.get_property("any-pressed")?,
                Value::Bool(false),
                "{label}: disabled visual state"
            );
            i_slint_backend_testing::mock_elapsed_time(1);
            instance.set_property("controls-enabled", true.into())?;
            instance.window().dispatch_event(WindowEvent::PointerPressed {
                position: center,
                button: PointerEventButton::Right,
            });
            release(center);
            instance.window().dispatch_event(WindowEvent::PointerReleased {
                position: center,
                button: PointerEventButton::Right,
            });
            assert_eq!(
                control.accessible_checked(),
                Some(false),
                "{label}: disabling cancels pending press"
            );
            assert_eq!(instance.get_property("clicks")?, Value::Number(expected_clicks));
            pointer_click(&instance, &control);
            assert_eq!(
                control.accessible_checked(),
                Some(true),
                "{label}: fresh press after cancellation"
            );
            control.invoke_accessible_default_action();
            assert_eq!(control.accessible_checked(), Some(false));
        }
    }
    Ok(())
}

#[test]
fn standard_group_selection_requests_preserve_row_and_menu_bindings() -> Result<(), Box<dyn Error>>
{
    let instance = create_fixture("StandardGroupCase")?;
    let button = |label: &str| {
        ElementHandle::find_by_accessible_label(&instance, label)
            .find(|element| element.accessible_role() == Some(AccessibleRole::Checkbox))
            .unwrap()
    };
    let first = button("First");
    let second = button("Second");
    second.invoke_accessible_default_action();
    assert_eq!(instance.get_property("requests")?, Value::Number(1.0));
    assert_eq!(instance.get_property("observed-selection")?, Value::Number(0.0));
    instance.set_property("selected", 1.into())?;
    assert_eq!(second.accessible_checked(), Some(true));
    second.invoke_accessible_default_action();
    assert_eq!(instance.get_property("requested-checked")?, Value::Bool(false));
    assert_eq!(second.accessible_checked(), Some(true));
    instance.set_property("selected", 0.into())?;
    instance.set_property("accept-requests", true.into())?;
    pointer_click(&instance, &second);
    assert_eq!(instance.get_property("selected")?, Value::Number(1.0));
    key(&instance, "\t", true);
    key(&instance, "\t", false);
    key(&instance, " ", true);
    key(&instance, " ", false);
    assert_eq!(instance.get_property("selected")?, Value::Number(0.0));
    instance.set_property("accept-requests", false.into())?;
    instance.set_property("multiple", true.into())?;
    set_first_model_checked(&instance, false)?;
    first.invoke_accessible_default_action();
    assert_eq!(first.accessible_checked(), Some(false));
    assert_eq!(instance.get_property("requested-checked")?, Value::Bool(true));
    set_first_model_checked(&instance, true)?;
    assert_eq!(first.accessible_checked(), Some(true));
    first.invoke_accessible_default_action();
    assert_eq!(first.accessible_checked(), Some(true));
    assert_eq!(instance.get_property("requested-checked")?, Value::Bool(false));
    instance.set_property("automatic", true.into())?;
    first.invoke_accessible_default_action();
    assert_eq!(instance.get_property("model-checked")?, Value::Bool(false));
    set_first_model_checked(&instance, true)?;
    assert_eq!(first.accessible_checked(), Some(true));
    let requests = instance.get_property("requests")?;
    button("Disabled").invoke_accessible_default_action();
    instance.set_property("controls-enabled", false.into())?;
    first.invoke_accessible_default_action();
    assert_eq!(instance.get_property("requests")?, requests);
    instance.set_property("controls-enabled", true.into())?;
    instance.set_property("automatic", false.into())?;
    instance.set_property("available-width", 90.into())?;
    i_slint_backend_testing::mock_elapsed_time(1);
    let more = ElementHandle::find_by_accessible_label(&instance, "More options")
        .find(|element| element.accessible_role() == Some(AccessibleRole::Button))
        .unwrap();
    more.invoke_accessible_default_action();
    let disabled = ElementHandle::find_by_element_type_name(&instance, "MenuItemTemplate")
        .find(|element| element.accessible_label().as_deref() == Some("Disabled"))
        .unwrap();
    let before_menu = instance.get_property("requests")?;
    disabled.invoke_accessible_default_action();
    assert_eq!(instance.get_property("requests")?, before_menu);
    let menu = ElementHandle::find_by_element_type_name(&instance, "MenuItemTemplate")
        .find(|element| element.accessible_label().as_deref() == Some("Second"))
        .unwrap();
    menu.invoke_accessible_default_action();
    assert_eq!(instance.get_property("requested-index")?, Value::Number(1.0));
    assert_eq!(
        instance.get_property("requests")?,
        Value::Number(f64::try_from(requests).unwrap() + 1.0)
    );
    assert_eq!(instance.get_property("requests")?, instance.get_property("activations")?);
    let requests = instance.get_property("requests")?;
    let activations = instance.get_property("activations")?;
    instance.set_property("clickable", true.into())?;
    instance.set_property("available-width", 400.into())?;
    i_slint_backend_testing::mock_elapsed_time(1);
    let clickable = ElementHandle::find_by_accessible_label(&instance, "First")
        .find(|element| element.accessible_role() == Some(AccessibleRole::Button))
        .unwrap();
    clickable.invoke_accessible_default_action();
    assert_eq!(instance.get_property("requests")?, requests);
    assert_eq!(
        instance.get_property("activations")?,
        Value::Number(f64::try_from(activations).unwrap() + 1.0)
    );
    instance.hide()?;
    let owned = load_fixture("StandardGroupCase")?;
    owned.set_property("automatic", true.into())?;
    i_slint_backend_testing::mock_elapsed_time(1);
    let Value::Model(model) = owned.get_property("items")? else { panic!("items must be a model") };
    let Value::Struct(mut row) = model.row_data(1).unwrap() else {
        panic!("item must be a struct")
    };
    row.set_field("checked".into(), true.into());
    model.set_row_data(1, row.into());
    i_slint_backend_testing::mock_elapsed_time(1);
    assert_eq!(owned.get_property("observed-selection")?, Value::Number(1.0));
    let Value::Struct(mut row) = model.row_data(1).unwrap() else {
        panic!("item must be a struct")
    };
    row.set_field("checked".into(), false.into());
    model.set_row_data(1, row.into());
    i_slint_backend_testing::mock_elapsed_time(1);
    assert_eq!(owned.get_property("observed-selection")?, Value::Number(-1.0));
    assert_eq!(owned.get_property("requests")?, Value::Number(0.0));
    Ok(())
}

#[test]
fn segmented_selection_requests_preserve_external_index() -> Result<(), Box<dyn Error>> {
    let instance = create_fixture("SegmentedCase")?;
    let second = ElementHandle::find_by_accessible_label(&instance, "Second")
        .find(|element| element.accessible_role() == Some(AccessibleRole::ListItem))
        .unwrap();
    second.invoke_accessible_default_action();
    assert_eq!(instance.get_property("requests")?, Value::Number(1.0));
    assert_eq!(instance.get_property("observed-selection")?, Value::Number(0.0));
    instance.set_property("selected", 1.into())?;
    assert_eq!(instance.get_property("observed-selection")?, Value::Number(1.0));
    instance.set_property("selected", 0.into())?;
    instance.set_property("accept-requests", true.into())?;
    pointer_click(&instance, &second);
    assert_eq!(instance.get_property("selected")?, Value::Number(1.0));
    instance.set_property("selected", 0.into())?;
    assert_eq!(instance.get_property("observed-selection")?, Value::Number(0.0));
    key(&instance, "\t", true);
    key(&instance, "\t", false);
    key(&instance, "\n", true);
    key(&instance, "\n", false);
    assert_eq!(instance.get_property("requested-index")?, Value::Number(0.0));
    key(&instance, "\t", true);
    key(&instance, "\t", false);
    key(&instance, " ", true);
    key(&instance, " ", false);
    assert_eq!(instance.get_property("selected")?, Value::Number(1.0));
    instance.set_property("selected", 0.into())?;
    assert_eq!(instance.get_property("observed-selection")?, Value::Number(0.0));
    instance.set_property("accept-requests", false.into())?;
    instance.set_property("automatic", true.into())?;
    second.invoke_accessible_default_action();
    assert_eq!(instance.get_property("observed-selection")?, Value::Number(1.0));
    instance.set_property("controls-enabled", false.into())?;
    let requests = instance.get_property("requests")?;
    second.invoke_accessible_default_action();
    pointer_click(&instance, &second);
    assert_eq!(instance.get_property("requests")?, requests);
    Ok(())
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

#[test]
fn state_layer_retains_hover_during_press_and_uses_reference_timings() -> Result<(), Box<dyn Error>>
{
    let instance = create_fixture("StateLayerCase")?;
    let opacity = || -> Result<f64, Box<dyn Error>> {
        let Value::Number(value) = instance.get_property("layer-opacity")? else {
            panic!("opacity");
        };
        Ok(value)
    };
    let update = |name: &str, value: bool| -> Result<(), Box<dyn Error>> {
        instance.set_property(name, Value::Bool(value))?;
        i_slint_backend_testing::mock_elapsed_time(1);
        let _ = opacity()?;
        Ok(())
    };
    let close = |value: f64, expected: f64| {
        assert!((value - expected).abs() < 0.002, "{value} != {expected}")
    };
    close(opacity()?, 0.0);
    update("hover", true)?;
    i_slint_backend_testing::mock_elapsed_time(7);
    close(opacity()?, 0.08 * 7.0 / 15.0);
    i_slint_backend_testing::mock_elapsed_time(8);
    close(opacity()?, 0.08);
    update("press", true)?;
    i_slint_backend_testing::mock_elapsed_time(60);
    close(opacity()?, 0.08);
    update("focused", true)?;
    i_slint_backend_testing::mock_elapsed_time(22);
    close(opacity()?, 0.08 + 0.02 * 22.0 / 45.0);
    i_slint_backend_testing::mock_elapsed_time(23);
    close(opacity()?, 0.10);
    update("drag", true)?;
    i_slint_backend_testing::mock_elapsed_time(45);
    close(opacity()?, 0.16);
    update("hover", false)?;
    update("hover", true)?;
    i_slint_backend_testing::mock_elapsed_time(15);
    close(opacity()?, 0.08);
    update("hover", false)?;
    i_slint_backend_testing::mock_elapsed_time(45);
    close(opacity()?, 0.16);
    update("focused", false)?;
    update("drag", false)?;
    i_slint_backend_testing::mock_elapsed_time(75);
    close(opacity()?, 0.08);
    i_slint_backend_testing::mock_elapsed_time(75);
    close(opacity()?, 0.0);
    update("focused", true)?;
    i_slint_backend_testing::mock_elapsed_time(45);
    close(opacity()?, 0.10);
    update("ring", true)?;
    i_slint_backend_testing::mock_elapsed_time(45);
    close(opacity()?, 0.0);
    update("hover", true)?;
    i_slint_backend_testing::mock_elapsed_time(5);
    assert!(opacity()? > 0.0 && opacity()? < 0.08);
    update("reduced-motion", true)?;
    close(opacity()?, 0.08);
    update("hover", false)?;
    close(opacity()?, 0.0);
    update("ring", false)?;
    close(opacity()?, 0.10);
    update("control-enabled", false)?;
    close(opacity()?, 0.0);
    update("control-enabled", true)?;
    close(opacity()?, 0.10);
    update("reduced-motion", false)?;
    update("ring", true)?;
    i_slint_backend_testing::mock_elapsed_time(15);
    assert!(opacity()? > 0.0);
    update("control-enabled", false)?;
    close(opacity()?, 0.0);
    Ok(())
}

#[test]
fn button_elevation_preserves_event_order_before_the_next_frame() -> Result<(), Box<dyn Error>> {
    let instance = create_fixture("ElevationCase")?;
    let elevation = |index| -> Result<f64, Box<dyn Error>> {
        let Value::Model(values) = instance.get_property("elevations")? else {
            panic!("elevation model");
        };
        let Some(Value::Number(value)) = values.row_data(index) else {
            panic!("elevation row");
        };
        Ok(value)
    };
    for (index, label) in [
        "ToggleButton",
        "FilledTonalToggleButton",
        "ElevatedToggleButton",
        "OutlinedToggleButton",
        "IconToggleButton",
        "FilledIconToggleButton",
        "FilledTonalIconToggleButton",
        "OutlinedIconToggleButton",
    ]
    .into_iter()
    .enumerate()
    {
        instance.invoke("focus-outside", &[])?;
        instance.window().dispatch_event(WindowEvent::PointerMoved {
            position: LogicalPosition::new(-20.0, -20.0),
        });
        i_slint_backend_testing::mock_elapsed_time(1);
        let control = ElementHandle::find_by_accessible_label(&instance, label)
            .find(|element| element.accessible_role() == Some(AccessibleRole::Checkbox))
            .unwrap();
        let origin = control.absolute_position();
        let size = control.size();
        let position =
            LogicalPosition::new(origin.x + size.width / 2.0, origin.y + size.height / 2.0);
        // A press without a preceding move establishes hover in the same input dispatch.
        instance.window().dispatch_event(WindowEvent::PointerPressed {
            position,
            button: PointerEventButton::Left,
        });
        i_slint_backend_testing::mock_elapsed_time(1);
        assert_eq!(elevation(index)?, 6.0, "style {index}: press follows hover in one frame");
        for _ in 0..=index {
            key(&instance, "\t", true);
            key(&instance, "\t", false);
        }
        i_slint_backend_testing::mock_elapsed_time(1);
        assert_eq!(elevation(index)?, 10.0, "style {index}: focus follows held press");
        instance.window().dispatch_event(WindowEvent::PointerReleased {
            position,
            button: PointerEventButton::Left,
        });
        i_slint_backend_testing::mock_elapsed_time(1);
        assert_eq!(elevation(index)?, 10.0, "style {index}: release retains newer focus");
    }
    Ok(())
}
