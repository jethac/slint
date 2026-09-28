// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! FemtoVG driver: renders `cases/material/` cases on the OpenGL renderer and
//! runs their parity checks. The GL context is a headless, surfaceless EGL
//! context (Mesa's `EGL_PLATFORM_SURFACELESS_MESA`, `llvmpipe` in software) —
//! no display server needed.

use i_slint_core::api::PhysicalSize;
use i_slint_core::graphics::{Rgba8Pixel, SharedPixelBuffer};
use i_slint_core::platform::{Platform, PlatformError, WindowAdapter, WindowEvent};
use i_slint_core::renderer::{Renderer, RendererSealed};
use i_slint_renderer_femtovg::FemtoVGRenderer;
use i_slint_renderer_femtovg::opengl::{OpenGLBackend, OpenGLInterface};
use slint_interpreter::ComponentHandle;
use std::cell::Cell;
use std::ffi::c_void;
use std::num::NonZeroU32;
use std::rc::Rc;

/// An EGL context with no surface: FemtoVG renders into its own framebuffer
/// objects, so no EGL surface is needed at all.
struct SurfacelessEgl {
    egl: &'static khronos_egl::Instance<khronos_egl::Static>,
    display: khronos_egl::Display,
    context: khronos_egl::Context,
}

impl SurfacelessEgl {
    fn new() -> Result<Self, String> {
        /// `EGL_PLATFORM_SURFACELESS_MESA`.
        const PLATFORM_SURFACELESS_MESA: khronos_egl::Enum = 0x31DD;

        let egl = &khronos_egl::API;
        let display = unsafe {
            egl.get_platform_display(PLATFORM_SURFACELESS_MESA, khronos_egl::DEFAULT_DISPLAY, &[])
                .ok()
                .or_else(|| egl.get_display(khronos_egl::DEFAULT_DISPLAY))
        }
        .ok_or("eglGetPlatformDisplay/eglGetDisplay: no EGL display")?;
        egl.initialize(display).map_err(|e| format!("eglInitialize: {e:?}"))?;
        egl.bind_api(khronos_egl::OPENGL_ES_API).map_err(|e| format!("eglBindAPI: {e:?}"))?;

        let attribs = [
            khronos_egl::SURFACE_TYPE,
            khronos_egl::PBUFFER_BIT,
            khronos_egl::RENDERABLE_TYPE,
            khronos_egl::OPENGL_ES3_BIT,
            khronos_egl::RED_SIZE,
            8,
            khronos_egl::GREEN_SIZE,
            8,
            khronos_egl::BLUE_SIZE,
            8,
            khronos_egl::ALPHA_SIZE,
            8,
            khronos_egl::NONE,
        ];
        let mut configs: Vec<khronos_egl::Config> = Vec::with_capacity(1);
        egl.choose_config(display, &attribs, &mut configs)
            .map_err(|e| format!("eglChooseConfig: {e:?}"))?;
        let config = *configs.first().ok_or("eglChooseConfig: no matching config")?;
        let context_attribs = [khronos_egl::CONTEXT_CLIENT_VERSION, 3, khronos_egl::NONE];
        let context = egl
            .create_context(display, config, None, &context_attribs)
            .map_err(|e| format!("eglCreateContext (ES3): {e:?}"))?;
        Ok(Self { egl, display, context })
    }
}

impl Drop for SurfacelessEgl {
    fn drop(&mut self) {
        let _ = self.egl.destroy_context(self.display, self.context);
    }
}

/// The one EGL context is shared by every window's renderer; `SharedEgl`
/// forwards `OpenGLInterface` so each `FemtoVGRenderer` owns a copy.
struct SharedEgl(Rc<SurfacelessEgl>);

#[allow(unsafe_code)]
unsafe impl OpenGLInterface for SharedEgl {
    fn ensure_current(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.0
            .egl
            .make_current(self.0.display, None, None, Some(self.0.context))
            .map_err(|e| format!("eglMakeCurrent: {e:?}").into())
    }

    fn swap_buffers(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    fn resize(
        &self,
        _width: NonZeroU32,
        _height: NonZeroU32,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    fn get_proc_address(&self, name: &std::ffi::CStr) -> *const c_void {
        self.0
            .egl
            .get_proc_address(name.to_str().unwrap_or_default())
            .map_or(std::ptr::null(), |f| f as *const c_void)
    }
}

pub struct FemtoVGScreenshotWindow {
    window: i_slint_core::api::Window,
    size: Cell<PhysicalSize>,
    renderer: FemtoVGRenderer<OpenGLBackend>,
}

impl WindowAdapter for FemtoVGScreenshotWindow {
    fn window(&self) -> &i_slint_core::api::Window {
        &self.window
    }

    fn size(&self) -> PhysicalSize {
        if self.size.get().width == 0 { PhysicalSize::new(64, 64) } else { self.size.get() }
    }

    fn set_size(&self, size: i_slint_core::api::WindowSize) {
        self.window.dispatch_event(WindowEvent::Resized {
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

/// One EGL context per test thread; every created window gets its own
/// FemtoVGRenderer on top of it.
pub struct FemtoVGScreenshotBackend {
    egl: Rc<SurfacelessEgl>,
}

impl Platform for FemtoVGScreenshotBackend {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, PlatformError> {
        // `FemtoVGRenderer::new` builds its glow context eagerly, which reads
        // `GL_VERSION`: the surfaceless context must be current first.
        self.egl
            .egl
            .make_current(self.egl.display, None, None, Some(self.egl.context))
            .map_err(|e| PlatformError::from(format!("eglMakeCurrent: {e:?}")))?;
        let renderer = FemtoVGRenderer::<OpenGLBackend>::new(SharedEgl(self.egl.clone()))
            .map_err(|e| PlatformError::from(format!("FemtoVGRenderer::new: {e}")))?;
        let adapter = Rc::new_cyclic(|self_weak| FemtoVGScreenshotWindow {
            window: i_slint_core::api::Window::new(self_weak.clone() as _),
            size: Default::default(),
            renderer,
        });
        let dyn_adapter: Rc<dyn WindowAdapter> = adapter.clone();
        adapter.renderer.set_window_adapter(&dyn_adapter);
        Ok(adapter)
    }

    fn duration_since_start(&self) -> core::time::Duration {
        core::time::Duration::from_millis(i_slint_core::animations::current_tick().0)
    }
}

/// Set the platform to a headless FemtoVG backend. Returns `Err` when no
/// EGL/GLES context can be created so the caller can skip rather than fail on
/// hosts without GL.
pub fn init_femtovg() -> Result<(), String> {
    crate::testing::force_reference_os();
    let egl = Rc::new(SurfacelessEgl::new()?);
    i_slint_core::platform::set_platform(Box::new(FemtoVGScreenshotBackend { egl }))
        .map_err(|e| e.to_string())
}

pub struct TestCase {
    pub absolute_path: std::path::PathBuf,
    pub relative_path: std::path::PathBuf,
    pub reference_path: std::path::PathBuf,
}

pub fn run_test(testcase: TestCase) -> Result<(), Box<dyn std::error::Error>> {
    if let Err(e) = init_femtovg() {
        eprintln!("femtovg: skipping {} ({e})", testcase.relative_path.display());
        return Ok(());
    }

    let source = std::fs::read_to_string(&testcase.absolute_path)?;
    let component = crate::interpreter::compile_case(&source, &testcase.absolute_path)?;

    if let Some((w, h)) = crate::interpreter::case_size(&source) {
        component
            .window()
            .set_size(i_slint_core::api::WindowSize::Physical(PhysicalSize::new(w, h)));
    }
    component.show().unwrap();

    let screenshot: SharedPixelBuffer<Rgba8Pixel> = component.window().take_snapshot().unwrap();

    // FemtoVG has no golden set of its own; when a case does carry one under
    // `references/femtovg/` it is compared, otherwise only the parity checks run.
    if testcase.reference_path.exists() {
        crate::testing::compare_images(
            testcase.reference_path.to_str().unwrap(),
            &screenshot,
            Default::default(),
            &crate::testing::TestCaseOptions { base_threshold: 8., ..Default::default() },
        )?;
    }

    run_parity(&testcase, &source)
}

/// Runs the `//PARITY=` checks on the FemtoVG renderer, re-instantiating the
/// component per density.
fn run_parity(
    testcase: &TestCase,
    source: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let spec = test_driver_lib::extract_parity(source);
    if spec.parity.is_none() {
        return Ok(());
    }
    let (w, h) = crate::interpreter::case_size(source).unwrap_or((64, 64));
    let compiled = crate::interpreter::compile(source, &testcase.absolute_path)?;
    let def = compiled.components().last().expect("There must be at least one exported component");

    let rel = testcase.relative_path.with_extension("").to_string_lossy().replace('\\', "/");
    crate::parity::run_parity_case(
        "femtovg",
        &rel,
        &spec,
        &|component: &slint_interpreter::ComponentInstance, name: &str| {
            component.get_property(name).ok().map(crate::interpreter::trace_value)
        },
        |density| {
            let component = def.create().unwrap();
            component.window().dispatch_event(WindowEvent::ScaleFactorChanged {
                scale_factor: density as f32,
            });
            component.window().set_size(i_slint_core::api::WindowSize::Physical(
                PhysicalSize::new(w * density, h * density),
            ));
            component.show().unwrap();
            component
        },
        |component| component.window().take_snapshot().unwrap(),
    )
}
