// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use std::cell::RefCell;
use std::rc::Rc;

use slint::{ComponentHandle, VecModel};

slint::include_modules!();

struct Notebook {
    tasks: Vec<Task>,
    editing: Option<i32>,
    next_id: i32,
}

impl Default for Notebook {
    fn default() -> Self {
        Self {
            tasks: [
                ("Walk before the day begins", "Twenty minutes outside, without the phone.", true),
                ("Sketch the next idea", "Leave space for a few unexpected directions.", false),
                (
                    "Make something worth sharing",
                    "A small first version is enough for today.",
                    false,
                ),
                ("Catch up with a friend", "Ask about the thing they were excited about.", false),
            ]
            .into_iter()
            .enumerate()
            .map(|(id, (title, note, completed))| Task {
                id: id as i32,
                title: title.into(),
                note: note.into(),
                completed,
            })
            .collect(),
            editing: None,
            next_id: 4,
        }
    }
}

impl Notebook {
    fn refresh(&self, ui: &Fieldnotes) {
        ui.set_total_count(self.tasks.len() as i32);
        ui.set_completed_count(self.tasks.iter().filter(|task| task.completed).count() as i32);
        ui.set_tasks(
            Rc::new(VecModel::from(
                self.tasks
                    .iter()
                    .filter(|task| match ui.get_filter() {
                        1 => !task.completed,
                        2 => task.completed,
                        _ => true,
                    })
                    .cloned()
                    .collect::<Vec<_>>(),
            ))
            .into(),
        );
    }

    fn save(&mut self, title: &str, note: &str) -> bool {
        let title = title.trim();
        if title.is_empty() {
            return false;
        }
        if let Some(id) = self.editing {
            let Some(task) = self.tasks.iter_mut().find(|task| task.id == id) else {
                return false;
            };
            task.title = title.into();
            task.note = note.trim().into();
        } else {
            self.tasks.push(Task {
                id: self.next_id,
                title: title.into(),
                note: note.trim().into(),
                completed: false,
            });
            self.next_id += 1;
        }
        self.editing = None;
        true
    }
}

fn create_ui() -> Result<Fieldnotes, slint::PlatformError> {
    let ui = Fieldnotes::new()?;
    let notebook = Rc::new(RefCell::new(Notebook::default()));
    notebook.borrow().refresh(&ui);

    ui.on_filter_changed({
        let notebook = notebook.clone();
        let weak = ui.as_weak();
        move |_| {
            if let Some(ui) = weak.upgrade() {
                notebook.borrow().refresh(&ui);
            }
        }
    });
    ui.on_new_task({
        let notebook = notebook.clone();
        let weak = ui.as_weak();
        move || {
            if let Some(ui) = weak.upgrade() {
                notebook.borrow_mut().editing = None;
                ui.set_draft_title("".into());
                ui.set_draft_note("".into());
                ui.set_form_error("".into());
                ui.invoke_open_editor();
            }
        }
    });
    ui.on_edit_task({
        let notebook = notebook.clone();
        let weak = ui.as_weak();
        move |id| {
            let Some(ui) = weak.upgrade() else { return };
            let task = notebook.borrow().tasks.iter().find(|task| task.id == id).cloned();
            if let Some(task) = task {
                notebook.borrow_mut().editing = Some(id);
                ui.set_draft_title(task.title);
                ui.set_draft_note(task.note);
                ui.set_form_error("".into());
                ui.invoke_open_editor();
            }
        }
    });
    ui.on_toggle_task({
        let notebook = notebook.clone();
        let weak = ui.as_weak();
        move |id| {
            let Some(ui) = weak.upgrade() else { return };
            let mut notebook = notebook.borrow_mut();
            if let Some(task) = notebook.tasks.iter_mut().find(|task| task.id == id) {
                task.completed = !task.completed;
            }
            notebook.refresh(&ui);
        }
    });
    ui.on_save_task({
        let weak = ui.as_weak();
        move |title, note| {
            let Some(ui) = weak.upgrade() else { return false };
            let mut notebook = notebook.borrow_mut();
            if !notebook.save(&title, &note) {
                ui.set_form_error("Enter a task name.".into());
                return false;
            }
            notebook.refresh(&ui);
            true
        }
    });
    Ok(ui)
}

fn main() -> Result<(), slint::PlatformError> {
    let backend =
        slint::BackendSelector::new().backend_name("winit".into()).renderer_name("skia".into());
    #[cfg(target_os = "windows")]
    let backend = backend.require_d3d();
    backend.select()?;
    create_ui()?.run()
}

#[cfg(test)]
mod tests {
    use super::*;
    use slint::Model;

    fn setup() -> Fieldnotes {
        slint::platform::set_platform(Box::new(i_slint_backend_testing::TestingBackend::new(
            i_slint_backend_testing::TestingBackendOptions {
                mock_time: true,
                renderer_name: Some("software".into()),
                ..Default::default()
            },
        )))
        .unwrap();
        create_ui().unwrap()
    }

    #[test]
    fn tasks_keep_their_identity_when_filtered_and_edited() {
        let ui = setup();
        ui.set_filter(1);
        i_slint_backend_testing::mock_elapsed_time(0);
        assert_eq!(ui.get_tasks().row_count(), 3);
        let task = ui.get_tasks().row_data(1).unwrap();
        ui.invoke_toggle_task(task.id);
        assert_eq!(ui.get_tasks().row_count(), 2);
        assert_eq!(ui.get_completed_count(), 2);
        ui.set_filter(2);
        i_slint_backend_testing::mock_elapsed_time(0);
        ui.invoke_edit_task(task.id);
        assert!(!ui.invoke_save_task("   ".into(), "".into()));
        assert_eq!(ui.get_total_count(), 4);
        assert!(ui.invoke_save_task("  A finished sketch  ".into(), "  Keep it  ".into()));
        let edited = ui.get_tasks().iter().find(|row| row.id == task.id).unwrap();
        assert_eq!(edited.title, "A finished sketch");
        assert!(edited.completed);
        ui.invoke_close_editor();
        ui.invoke_new_task();
        assert!(ui.invoke_save_task("Call Alex".into(), "".into()));
        assert_eq!(ui.get_total_count(), 5);
        assert_eq!(ui.get_tasks().row_count(), 2);
        ui.set_filter(0);
        i_slint_backend_testing::mock_elapsed_time(0);
        assert_eq!(ui.get_tasks().row_count(), 5);
    }

    #[test]
    fn appearance_tooltip_paints_below_the_app_bar() {
        let ui = setup();
        ui.show().unwrap();
        ui.set_reduced_motion(true);
        for (width, dark) in [(390, false), (1000, true)] {
            ui.window().set_size(slint::PhysicalSize::new(width, 844));
            ui.set_dark(dark);
            i_slint_backend_testing::mock_elapsed_time(1000);
            let before = ui.window().take_snapshot().unwrap();
            let button = i_slint_backend_testing::ElementHandle::find_by_accessible_label(
                &ui,
                if dark { "Use light colors" } else { "Use dark colors" },
            )
            .find(|element| {
                element.accessible_role() == Some(i_slint_backend_testing::AccessibleRole::Button)
            })
            .unwrap();
            let position = button.absolute_position();
            let size = button.size();
            ui.window().dispatch_event(slint::platform::WindowEvent::PointerMoved {
                position: slint::LogicalPosition::new(
                    position.x + size.width / 2.0,
                    position.y + size.height / 2.0,
                ),
            });
            i_slint_backend_testing::mock_elapsed_time(1000);
            let tip = i_slint_backend_testing::ElementHandle::find_by_element_type_name(
                &ui,
                "PlainTooltip",
            )
            .next()
            .unwrap();
            let position = tip.absolute_position();
            let size = tip.size();
            let x = (position.x + size.width / 2.0) as usize;
            let y = (position.y + size.height - 2.0) as usize;
            assert!(y > 64, "the tooltip must extend below the app bar");
            assert!(position.x >= 0.0 && position.x + size.width <= width as f32);
            let after = ui.window().take_snapshot().unwrap();
            let offset = (y * width as usize + x) * 4;
            assert_ne!(
                &before.as_bytes()[offset..offset + 4],
                &after.as_bytes()[offset..offset + 4],
                "the tooltip's bottom must paint beyond ancestor clipping and content"
            );
            if let Some(directory) = std::env::var_os("FIELDNOTES_SCREENSHOTS") {
                image::save_buffer(
                    std::path::Path::new(&directory).join(format!("tooltip-{width}.png")),
                    after.as_bytes(),
                    width,
                    844,
                    image::ColorType::Rgba8,
                )
                .unwrap();
            }
            ui.window().dispatch_event(slint::platform::WindowEvent::PointerExited);
            i_slint_backend_testing::mock_elapsed_time(1000);
        }
    }

    #[test]
    fn keyboard_entry_and_responsive_rendering() {
        let ui = setup();
        ui.show().unwrap();
        ui.window().set_size(slint::PhysicalSize::new(1000, 760));
        assert!(!ui.get_compact());
        ui.invoke_new_task();
        let field =
            i_slint_backend_testing::ElementHandle::find_by_accessible_label(&ui, "Task name")
                .next()
                .expect("the visible field label must be exposed to assistive technology");
        assert_eq!(
            field.accessible_role(),
            Some(i_slint_backend_testing::AccessibleRole::TextInput)
        );
        i_slint_backend_testing::send_keyboard_string_sequence(&ui, "Read a chapter");
        assert_eq!(ui.get_draft_title(), "Read a chapter");
        assert_eq!(field.accessible_value().unwrap(), "Read a chapter");
        i_slint_backend_testing::send_keyboard_char(&ui, '\n', true);
        i_slint_backend_testing::send_keyboard_char(&ui, '\n', false);
        assert_eq!(ui.get_total_count(), 5);
        assert!(!ui.get_editor_open());

        let open_filter =
            i_slint_backend_testing::ElementHandle::find_by_accessible_label(&ui, "Open")
                .find(|element| {
                    element.accessible_role()
                        == Some(i_slint_backend_testing::AccessibleRole::Checkbox)
                })
                .unwrap();
        open_filter.invoke_accessible_default_action();
        assert_eq!(ui.get_filter(), 1);
        open_filter.invoke_accessible_default_action();
        assert_eq!(open_filter.accessible_checked(), Some(true));
        let all_filter =
            i_slint_backend_testing::ElementHandle::find_by_accessible_label(&ui, "All")
                .find(|element| {
                    element.accessible_role()
                        == Some(i_slint_backend_testing::AccessibleRole::Checkbox)
                })
                .unwrap();
        all_filter.invoke_accessible_default_action();
        assert_eq!(open_filter.accessible_checked(), Some(false));
        assert_eq!(ui.get_filter(), 0);
        ui.invoke_new_task();
        i_slint_backend_testing::send_keyboard_char(&ui, slint::platform::Key::Escape.into(), true);
        i_slint_backend_testing::send_keyboard_char(
            &ui,
            slint::platform::Key::Escape.into(),
            false,
        );
        assert!(!ui.get_editor_open());
        assert_eq!(ui.get_total_count(), 5);
        ui.invoke_new_task();
        i_slint_backend_testing::mock_elapsed_time(100);
        i_slint_backend_testing::send_mouse_click(&ui, 10.0, 10.0);
        assert!(!ui.get_editor_open());
        assert_eq!(ui.get_total_count(), 5);

        let task = i_slint_backend_testing::ElementHandle::find_by_accessible_label(
            &ui,
            "Sketch the next idea",
        )
        .find(|element| {
            element.accessible_role() == Some(i_slint_backend_testing::AccessibleRole::Checkbox)
        })
        .unwrap();
        assert_eq!(task.accessible_checked(), Some(false));
        task.invoke_accessible_default_action();
        assert_eq!(ui.get_completed_count(), 2);
        let task = i_slint_backend_testing::ElementHandle::find_by_accessible_label(
            &ui,
            "Sketch the next idea",
        )
        .find(|element| {
            element.accessible_role() == Some(i_slint_backend_testing::AccessibleRole::Checkbox)
        })
        .unwrap();
        assert_eq!(task.accessible_checked(), Some(true));
        task.invoke_accessible_default_action();
        assert_eq!(ui.get_completed_count(), 1);
        i_slint_backend_testing::mock_elapsed_time(0);
        let edit = i_slint_backend_testing::ElementHandle::find_by_accessible_label(
            &ui,
            "Edit Sketch the next idea",
        )
        .find(|element| {
            element.accessible_role() == Some(i_slint_backend_testing::AccessibleRole::Button)
        })
        .unwrap();
        let position = edit.absolute_position();
        let size = edit.size();
        i_slint_backend_testing::send_mouse_click(
            &ui,
            position.x + size.width / 2.0,
            position.y + size.height / 2.0,
        );
        assert!(ui.get_editor_open());
        assert_eq!(ui.get_draft_title(), "Sketch the next idea");
        assert_eq!(ui.get_completed_count(), 1, "editing must not toggle completion");
        ui.invoke_close_editor();

        let directory = std::env::var_os("FIELDNOTES_SCREENSHOTS");
        for (name, width, height, dark, page, editor) in [
            ("desktop-light", 1000, 760, false, 0, false),
            ("mobile-light", 390, 844, false, 0, false),
            ("mobile-dark", 390, 844, true, 0, false),
            ("settings-dark", 1000, 760, true, 1, false),
            ("editor-mobile", 390, 844, false, 0, true),
        ] {
            ui.window().set_size(slint::PhysicalSize::new(width, height));
            ui.set_dark(dark);
            ui.set_page(page);
            ui.set_reduced_motion(true);
            if editor {
                ui.invoke_new_task();
                assert!(!ui.invoke_save_task("".into(), "".into()));
            }
            i_slint_backend_testing::mock_elapsed_time(1000);
            let snapshot = ui.window().take_snapshot().unwrap();
            assert_eq!(snapshot.width(), width);
            assert_eq!(snapshot.height(), height);
            assert_eq!(ui.get_compact(), width < 720);
            if page == 0 && !editor {
                let add = i_slint_backend_testing::ElementHandle::find_by_accessible_label(
                    &ui, "Add task",
                )
                .find(|element| {
                    element.accessible_role()
                        == Some(i_slint_backend_testing::AccessibleRole::Button)
                })
                .unwrap();
                assert!(add.absolute_position().y < 400.0, "the app bar must leave room for tasks");
                assert!(add.size().width <= 160.0);
                let first_task = i_slint_backend_testing::ElementHandle::find_by_accessible_label(
                    &ui,
                    "Walk before the day begins",
                )
                .find(|element| {
                    element.accessible_role()
                        == Some(i_slint_backend_testing::AccessibleRole::ListItem)
                })
                .unwrap();
                assert!(first_task.absolute_position().y < 500.0);
                if width < 720 {
                    assert!(first_task.size().height >= 88.0);
                }
            }
            if let Some(directory) = &directory {
                std::fs::create_dir_all(directory).unwrap();
                image::save_buffer(
                    std::path::Path::new(directory).join(format!("{name}.png")),
                    snapshot.as_bytes(),
                    width,
                    height,
                    image::ColorType::Rgba8,
                )
                .unwrap();
            }
        }
        ui.invoke_close_editor();
        ui.window().dispatch_event(slint::platform::WindowEvent::PointerScrolled {
            position: slint::LogicalPosition::new(200.0, 550.0),
            delta_x: 0.0,
            delta_y: -1000.0,
        });
        i_slint_backend_testing::mock_elapsed_time(1000);
        let last_task =
            i_slint_backend_testing::ElementHandle::find_by_accessible_label(&ui, "Read a chapter")
                .find(|element| {
                    element.accessible_role()
                        == Some(i_slint_backend_testing::AccessibleRole::ListItem)
                })
                .unwrap();
        assert!(last_task.absolute_position().y >= 64.0);
        assert!(
            last_task.absolute_position().y + last_task.size().height <= 764.0,
            "the final task must scroll clear of bottom navigation"
        );
    }
}
