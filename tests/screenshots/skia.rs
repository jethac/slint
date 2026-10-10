// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use i_slint_core::api::PhysicalSize;
use i_slint_core::platform::{Platform, PlatformError};
use i_slint_core::renderer::Renderer;
use i_slint_core::window::WindowAdapter;
use i_slint_renderer_skia::{SkiaRenderer, SkiaSharedContext};
use slint_interpreter::ComponentHandle;

use std::cell::Cell;
use std::rc::Rc;

#[derive(Default)]
pub struct SkiaScreenshotBackend;

impl Platform for SkiaScreenshotBackend {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, PlatformError> {
        Ok(Rc::new_cyclic(|self_weak| SkiaScreenshotWindow {
            window: i_slint_core::api::Window::new(self_weak.clone() as _),
            size: Default::default(),
            renderer: SkiaRenderer::default_software(&SkiaSharedContext::default()),
        }))
    }

    fn duration_since_start(&self) -> core::time::Duration {
        core::time::Duration::from_millis(i_slint_core::animations::current_tick().0)
    }
}

pub struct SkiaScreenshotWindow {
    window: i_slint_core::api::Window,
    size: Cell<PhysicalSize>,
    renderer: SkiaRenderer,
}

impl WindowAdapter for SkiaScreenshotWindow {
    fn window(&self) -> &i_slint_core::api::Window {
        &self.window
    }

    fn size(&self) -> PhysicalSize {
        if self.size.get().width == 0 { PhysicalSize::new(64, 64) } else { self.size.get() }
    }

    fn set_size(&self, size: i_slint_core::api::WindowSize) {
        self.window.dispatch_event(i_slint_core::platform::WindowEvent::Resized {
            size: size.to_logical(self.window().scale_factor()),
        });
        self.size.set(size.to_physical(self.window().scale_factor()))
    }

    fn renderer(&self) -> &dyn Renderer {
        &self.renderer
    }

    fn update_window_properties(&self, properties: i_slint_core::window::WindowProperties<'_>) {
        if self.size.get().width == 0 {
            let c = properties.layout_constraints();
            self.size.set(c.preferred.to_physical(self.window.scale_factor()));
        }
    }
}

pub fn init_skia() {
    crate::testing::force_reference_os();

    i_slint_core::platform::set_platform(Box::new(SkiaScreenshotBackend))
        .expect("platform already initialized");
}

pub struct TestCase {
    pub absolute_path: std::path::PathBuf,
    pub relative_path: std::path::PathBuf,
    pub reference_path: std::path::PathBuf,
}

pub fn run_test(testcase: TestCase) -> Result<(), Box<dyn std::error::Error>> {
    init_skia();

    let source = std::fs::read_to_string(&testcase.absolute_path)?;
    let compiled = crate::interpreter::compile(&source, &testcase.absolute_path)?;

    let def = compiled.components().last().expect("There must be at least one exported component");
    let component = def.create().unwrap();
    if let Some((w, h)) = crate::interpreter::case_size(&source) {
        component
            .window()
            .set_size(i_slint_core::api::WindowSize::Physical(PhysicalSize::new(w, h)));
    }
    component.show().unwrap();

    let screenshot = component.window().take_snapshot().unwrap();

    // `//PARITY=` cases have no driver golden: the Compose references are
    // their ground truth, compared inside `run_parity`.
    if testcase.reference_path.exists() {
        // Images are rendered a bit differently on macOs.
        // Elsewhere the tolerance covers a last-bit difference of 2 per channel: tagging the raster
        // target as sRGB puts Skia on its color managed pipeline, whose float math rounds slightly
        // differently between the Linux and the Windows build even though every conversion is sRGB to
        // sRGB and therefore a no-op.
        let base_threshold = if cfg!(target_os = "macos") { 33. } else { 4. };

        crate::testing::compare_images(
            testcase.reference_path.to_str().unwrap(),
            &screenshot,
            Default::default(),
            &crate::testing::TestCaseOptions { base_threshold, ..Default::default() },
        )?;
    }

    run_parity(&testcase, &source, &def)
}

/// Runs the `//PARITY=` checks on the Skia renderer, re-instantiating the
/// component definition per density.
fn run_parity(
    testcase: &TestCase,
    source: &str,
    def: &slint_interpreter::ComponentDefinition,
) -> Result<(), Box<dyn std::error::Error>> {
    let spec = test_driver_lib::extract_parity(source);
    if spec.parity.is_none() {
        return Ok(());
    }
    let (w, h) = crate::interpreter::case_size(source).unwrap_or((64, 64));

    let rel = testcase.relative_path.with_extension("").to_string_lossy().replace('\\', "/");
    crate::parity::run_parity_case(
        "skia",
        &rel,
        &spec,
        &|component: &slint_interpreter::ComponentInstance, name: &str| {
            component.get_property(name).ok().map(crate::interpreter::trace_value)
        },
        |density| {
            let component = def.create().unwrap();
            component.window().dispatch_event(
                i_slint_core::platform::WindowEvent::ScaleFactorChanged {
                    scale_factor: density as f32,
                },
            );
            component.window().set_size(i_slint_core::api::WindowSize::Physical(
                PhysicalSize::new(w * density, h * density),
            ));
            component.show().unwrap();
            component
        },
        |component| component.window().take_snapshot().unwrap(),
    )
}

// Compare renders within one run so font rasterization differences between platforms don't need golden images.
#[test]
fn text_alignment_anchor_stays_fixed() {
    init_skia();
    for (horizontal, x_fraction) in [("left", 0.0), ("center", 0.5), ("right", 1.0)] {
        for (vertical, y_fraction) in [("top", 0.0), ("center", 0.5), ("bottom", 1.0)] {
            for input in [false, true] {
                let item_type = if input { "TextInput" } else { "Text" };
                let selection = if input {
                    "selection-background-color: blue; selection-foreground-color: white;"
                } else {
                    ""
                };
                let prepare =
                    if input { "field.focus(); field.set-selection-offsets(1, 3);" } else { "" };
                let source = format!(
                    r#"
                    export component TestCase inherits Window {{
                        width: 180px;
                        height: 140px;
                        background: white;
                        in property <length> box-width: 80px;
                        in property <length> box-height: 40px;
                        callback prepare();
                        prepare => {{ {prepare} }}
                        field := {item_type} {{
                            x: 80.2px - {x_fraction} * root.box-width;
                            y: 60.2px - {y_fraction} * root.box-height;
                            width: root.box-width;
                            height: root.box-height;
                            text: "Hello";
                            font-size: 14px;
                            color: black;
                            horizontal-alignment: {horizontal};
                            vertical-alignment: {vertical};
                            {selection}
                        }}
                    }}
                    "#
                );
                let mut compiler = slint_interpreter::Compiler::default();
                compiler.set_style("fluent".into());
                let result = crate::interpreter::poll_once(
                    compiler.build_from_source(source, Default::default()),
                )
                .unwrap();
                assert!(!result.has_errors(), "{:?}", result.diagnostics().collect::<Vec<_>>());
                let definition = result.components().last().unwrap();
                for scale_factor in [1.0, 1.25, 1.5, 2.0] {
                    let component = definition.create().unwrap();
                    component.window().dispatch_event(
                        i_slint_core::platform::WindowEvent::ScaleFactorChanged { scale_factor },
                    );
                    component.show().unwrap();
                    component.invoke("prepare", &[]).unwrap();
                    let reference = component.window().take_snapshot().unwrap();
                    assert!(reference.as_slice().iter().any(|p| p.r < 128));
                    if input {
                        assert!(reference.as_slice().iter().any(|p| p.b > 200 && p.r < 50));
                    }
                    for delta in [0.25, 0.5, 0.75, 1.0] {
                        component.set_property("box-width", (80.0 + delta).into()).unwrap();
                        component.set_property("box-height", (40.0 + delta).into()).unwrap();
                        let actual = component.window().take_snapshot().unwrap();
                        let max_difference = actual
                            .as_bytes()
                            .iter()
                            .zip(reference.as_bytes())
                            .map(|(a, b)| a.abs_diff(*b))
                            .max()
                            .unwrap();
                        // Selection clips can change coverage by a few color levels as the box resizes.
                        let tolerance = if input { 4 } else { 0 };
                        assert!(
                            max_difference <= tolerance,
                            "{item_type} {horizontal}/{vertical}, scale {scale_factor}, delta {delta}: difference {max_difference}"
                        );
                    }
                    component.hide().unwrap();
                }
            }
        }
    }
}

#[test]
fn material_custom_button_paths_clip_content_and_follow_state() {
    init_skia();
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("material_button_shapes.slint");
    for name in [
        "ToggleButton",
        "FilledTonalToggleButton",
        "ElevatedToggleButton",
        "OutlinedToggleButton",
        "IconToggleButton",
        "FilledIconToggleButton",
        "FilledTonalIconToggleButton",
        "OutlinedIconToggleButton",
    ] {
        let padding = if name.contains("Icon") {
            ""
        } else {
            "padding-leading: 0px; padding-trailing: 0px; padding-vertical: 0px;"
        };
        let source = format!(
            r#"
            import {{ {name}, MaterialPalette }} from "../../ui-libraries/material/src/material.slint";
            export component TestCase inherits Window {{
                width: 140px; height: 80px; background: white;
                in property <bool> selected;
                in property <bool> active: true;
                in property <bool> press;
                in property <bool> content;
                in property <length> stroke;
                in property <bool> corners;
                property <shape> empty-shape;
                init => {{ MaterialPalette.reduced-motion = true; }}
                {name} {{
                    x: 20px; y: 20px; width: 100px; height: 40px;
                    enforce-touch-target: false;
                    {padding}
                    enabled: root.active; checked: root.selected; simulate-press: root.press;
                    resting-shape: root.corners ? root.empty-shape : Shapes.path("M0.5 0L1 0.5L0.5 1L0 0.5Z");
                    shaped-corners: root.corners;
                    resting-corners: {{ top-left: 20px, top-right: 0px, bottom-right: 0px, bottom-left: 0px }};
                    pressed-shape: Shapes.path("M0 0L1 0L1 1L0 1Z");
                    checked-shape: Shapes.path("M0 0L1 0L0.5 1Z");
                    container-color: blue; checked-container-color: green;
                    disabled-container-color: yellow;
                    content-color: transparent; checked-content-color: transparent;
                    disabled-content-color: transparent;
                    border-width: root.stroke; checked-border-width: root.stroke; border-color: black;
                    elevation-default: 0px; elevation-hovered: 0px;
                    elevation-focused: 0px; elevation-pressed: 0px; elevation-disabled: 0px;
                    Rectangle {{ visible: root.content; width: 40px; height: 40px; background: red; }}
                }}
            }}
        "#
        );
        let compiler = slint_interpreter::Compiler::default();
        let result =
            crate::interpreter::poll_once(compiler.build_from_source(source, path.clone()))
                .unwrap();
        assert!(!result.has_errors(), "{:?}", result.diagnostics().collect::<Vec<_>>());
        let component = result.components().last().unwrap().create().unwrap();
        component.show().unwrap();
        let snapshot = || component.window().take_snapshot().unwrap();
        let pixel = |image: &i_slint_core::graphics::SharedPixelBuffer<
            i_slint_core::graphics::Rgba8Pixel,
        >,
                     x,
                     y| { image.as_slice()[y * image.width() as usize + x] };
        let rest = snapshot();
        assert_eq!(pixel(&rest, 70, 40).b, 255, "{name}: resting color");
        assert_eq!(pixel(&rest, 53, 23).r, 255, "{name}: diamond corner");
        component.set_property("content", true.into()).unwrap();
        let content = snapshot();
        assert_eq!(pixel(&content, 70, 40).r, 255, "{name}: custom content");
        assert_eq!(pixel(&content, 53, 23).g, 255, "{name}: clipped content");
        component.set_property("content", false.into()).unwrap();
        component.set_property("selected", true.into()).unwrap();
        let selected = snapshot();
        assert_eq!(pixel(&selected, 70, 40).g, 128, "{name}: checked color");
        assert_eq!(pixel(&selected, 53, 23).b, 0, "{name}: checked path");
        component.set_property("press", true.into()).unwrap();
        let pressed = snapshot();
        assert_eq!(pixel(&pressed, 53, 57).b, 0, "{name}: pressed path wins over checked");
        component.set_property("press", false.into()).unwrap();
        component.set_property("active", false.into()).unwrap();
        let disabled = snapshot();
        assert_eq!(pixel(&disabled, 70, 40).r, 255, "{name}: disabled color");
        assert_eq!(pixel(&disabled, 70, 40).g, 255, "{name}: disabled color");
        component.set_property("active", true.into()).unwrap();
        component.set_property("selected", false.into()).unwrap();
        component.set_property("stroke", 3.0.into()).unwrap();
        let bordered = snapshot();
        assert!(
            bordered.as_slice().iter().any(|p| p.r == 0 && p.g == 0 && p.b == 0),
            "{name}: custom border"
        );
        assert_eq!(pixel(&bordered, 53, 23).g, 255, "{name}: border follows the path");
        component.set_property("stroke", 0.0.into()).unwrap();
        let unfocused = snapshot();
        component
            .window()
            .dispatch_event(i_slint_core::platform::WindowEvent::KeyPressed { text: "\t".into() });
        component
            .window()
            .dispatch_event(i_slint_core::platform::WindowEvent::KeyReleased { text: "\t".into() });
        let focused = snapshot();
        assert_ne!(focused.as_bytes(), unfocused.as_bytes(), "{name}: keyboard focus ring");
        assert_eq!(pixel(&focused, 70, 40).b, 255, "{name}: focus ring preserves the center");
        assert_eq!(pixel(&focused, 53, 23).g, 255, "{name}: focus ring follows the path");
        component.hide().unwrap();
        let corners = result.components().last().unwrap().create().unwrap();
        corners.set_property("corners", true.into()).unwrap();
        corners.show().unwrap();
        let corner_image = corners.window().take_snapshot().unwrap();
        let left = if name.contains("Icon") { 50 } else { 20 };
        let right = if name.contains("Icon") { 90 } else { 120 };
        assert_eq!(pixel(&corner_image, left + 3, 23).g, 255, "{name}: rounded top-left");
        assert_eq!(pixel(&corner_image, right - 3, 23).r, 0, "{name}: square top-right");
        if let Some(directory) = std::env::var_os("SLINT_MATERIAL_SHAPE_ARTIFACTS") {
            let directory = std::path::PathBuf::from(directory);
            std::fs::create_dir_all(&directory).unwrap();
            for (state, image) in [
                ("rest", &rest),
                ("content", &content),
                ("selected", &selected),
                ("pressed", &pressed),
                ("disabled", &disabled),
                ("border", &bordered),
                ("focus", &focused),
                ("corners", &corner_image),
            ] {
                image::save_buffer(
                    directory.join(format!("{name}-{state}.png")),
                    image.as_bytes(),
                    image.width(),
                    image.height(),
                    image::ColorType::Rgba8,
                )
                .unwrap();
            }
        }
        corners.hide().unwrap();
    }
}

#[test]
fn material_button_children_inherit_text_and_icon_style() {
    init_skia();
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("material_content_style.slint");
    for name in [
        "ToggleButton",
        "FilledTonalToggleButton",
        "ElevatedToggleButton",
        "OutlinedToggleButton",
        "IconToggleButton",
        "FilledIconToggleButton",
        "FilledTonalIconToggleButton",
        "OutlinedIconToggleButton",
    ] {
        let expected_color = "root.active ? (root.selected ? #007000 : #d00000) : #777777";
        let expected_style = if name.contains("Icon") {
            "MaterialTypography.body_large"
        } else {
            "MaterialTypography.label_large"
        };
        let mut definitions = Vec::new();
        for explicit in [false, true] {
            let text_style = if explicit {
                format!("style: {expected_style}; color: {expected_color};")
            } else {
                String::new()
            };
            let icon_style =
                if explicit { format!("colorize: {expected_color};") } else { String::new() };
            let source = format!(
                r#"
                import {{ {name}, MaterialText, MaterialTypography, MaterialPalette, Icon, Icons }}
                    from "../../ui-libraries/material/src/material.slint";
                export component TestCase inherits Window {{
                    width: 220px; height: 70px; background: white;
                    in property <bool> selected;
                    in property <bool> active: true;
                    init => {{ MaterialPalette.reduced-motion = true; }}
                    {name} {{
                        x: 10px; y: 10px; width: 180px; height: 48px;
                        enabled: root.active; checked: root.selected;
                        container-color: transparent; checked-container-color: transparent;
                        disabled-container-color: transparent;
                        content-color: #d00000; checked-content-color: #007000; disabled-content-color: #777777;
                        elevation-default: 0px; elevation-hovered: 0px; elevation-focused: 0px;
                        elevation-pressed: 0px; elevation-disabled: 0px;
                        border-width: 0px; checked-border-width: 0px;
                        MaterialText {{ text: "Ag"; {text_style} }}
                        Icon {{ source: Icons.add; {icon_style} }}
                    }}
                }}
            "#
            );
            let compiler = slint_interpreter::Compiler::default();
            let result =
                crate::interpreter::poll_once(compiler.build_from_source(source, path.clone()))
                    .unwrap();
            assert!(!result.has_errors(), "{:?}", result.diagnostics().collect::<Vec<_>>());
            definitions.push(result.components().last().unwrap());
        }
        let actual = definitions[0].create().unwrap();
        let expected = definitions[1].create().unwrap();
        actual.show().unwrap();
        expected.show().unwrap();
        for (active, selected) in [(true, false), (true, true), (false, false), (false, true)] {
            for component in [&actual, &expected] {
                component.set_property("active", active.into()).unwrap();
                component.set_property("selected", selected.into()).unwrap();
            }
            let image = actual.window().take_snapshot().unwrap();
            let reference = expected.window().take_snapshot().unwrap();
            assert!(
                image.as_slice().iter().any(|p| p.r < 200 || p.g < 200 || p.b < 200),
                "{name}: content must render"
            );
            assert_eq!(
                image.as_bytes(),
                reference.as_bytes(),
                "{name}: active={active}, selected={selected}"
            );
        }
        actual.hide().unwrap();
        expected.hide().unwrap();
    }
}
