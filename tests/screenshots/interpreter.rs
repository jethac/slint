// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Helpers shared by the interpreter-based drivers (`skia`, `femtovg`,
//! `anyrender`): compiling a case, reading its `SIZE=` marker, and converting
//! interpreter values for motion traces.
//! The `interpreter-embed-assets` driver loads its cases through [`load()`].

#[cfg(feature = "interpreter-embed-assets")]
use i_slint_core::InternalToken;

/// The case at `path`, compiled like the generated embed-assets cases.
#[cfg(feature = "interpreter-embed-assets")]
pub fn load(
    path: &str,
    scale_factor: f32,
    include_paths: &[&str],
) -> slint_interpreter::ComponentInstance {
    let (source, compiler) = compiler(path, scale_factor, include_paths);
    let result = spin_on::spin_on(compiler.build_from_source(source, path.into()));
    result.print_diagnostics();
    assert!(!result.has_errors(), "compile error in {path}");
    create(result, scale_factor)
}

/// Like [`load()`], after sending the compiled unit through serialization,
/// as a microcontroller receives it.
#[cfg(feature = "interpreter-embed-assets")]
pub fn load_serialized(
    path: &str,
    scale_factor: f32,
    include_paths: &[&str],
) -> slint_interpreter::ComponentInstance {
    use i_slint_compiler::llr::serialize;
    let (source, mut compiler) = compiler(path, scale_factor, include_paths);
    let mut config = compiler.compiler_configuration(InternalToken).clone();
    // The compiler keeps these as overrides, outside of its configuration
    config.include_paths = compiler.include_paths().clone();
    config.library_paths = compiler.library_paths().clone();
    config.style = compiler.style().cloned();
    let (diagnostics, compiled) = spin_on::spin_on(serialize::compile(
        &i_slint_compiler::source_path::SourcePath::new(path),
        source,
        config,
    ));
    let (unit, resources) = compiled
        .unwrap_or_else(|| panic!("compile error in {path}: {:?}", diagnostics.to_string_vec()));
    let unit = serialize::from_bytes(&serialize::to_bytes(&unit).unwrap()).unwrap();
    let resources = resources.iter().map(|resource| {
        let bytes = serialize::resource_to_bytes(resource).unwrap();
        slint_interpreter::LoadedResource::new(
            serialize::resource_from_bytes(&bytes).unwrap(),
            InternalToken,
        )
    });
    let received =
        slint_interpreter::CompilationResult::from_compilation_unit(unit, resources, InternalToken);
    create(received, scale_factor)
}

/// The source of the case at `path`, and a compiler set up for it.
#[cfg(feature = "interpreter-embed-assets")]
fn compiler(
    path: &str,
    scale_factor: f32,
    include_paths: &[&str],
) -> (String, slint_interpreter::Compiler) {
    // The build script sets these for the compiler it runs; this one runs now.
    // Safety: the test threads only read these, and all set the same values.
    unsafe {
        std::env::set_var("SLINT_DEFAULT_FONT", env!("SLINT_DEFAULT_FONT"));
        std::env::set_var("SLINT_FONT_PATH", env!("SLINT_FONT_PATH"));
    }
    let mut compiler = slint_interpreter::Compiler::default();
    compiler.set_style("fluent".into());
    compiler.set_include_paths(include_paths.iter().map(Into::into).collect());
    compiler.set_embed_resources(i_slint_compiler::EmbedResourcesKind::EmbedTextures);
    let config = compiler.compiler_configuration(InternalToken);
    config.experimental_features = i_slint_compiler::ExperimentalFeatures::all();
    config.const_scale_factor = Some(scale_factor);
    let source = std::fs::read_to_string(path).unwrap();
    compiler.set_edition(test_driver_lib::edition_version_or_current(&source));
    compiler.set_library_paths(crate::parity::library_paths_for(
        &source,
        std::path::Path::new(path).parent().unwrap_or_else(|| std::path::Path::new("")),
    ));
    (source, compiler)
}

#[cfg(feature = "interpreter-embed-assets")]
fn create(
    result: slint_interpreter::CompilationResult,
    scale_factor: f32,
) -> slint_interpreter::ComponentInstance {
    let instance = result.component("TestCase").expect("no TestCase component").create().unwrap();
    // The window must have the scale factor that the compiler pre-rendered for.
    slint::ComponentHandle::window(&instance)
        .dispatch_event(slint::platform::WindowEvent::ScaleFactorChanged { scale_factor });
    instance
}

/// Compiles `source` with the case's library and include paths and the fluent
/// style, aborting the test on diagnostics.
pub fn compile(
    source: &str,
    path: &std::path::Path,
) -> Result<slint_interpreter::CompilationResult, Box<dyn std::error::Error>> {
    let mut compiler = slint_interpreter::Compiler::default();
    compiler.set_style("fluent".into());
    compiler.set_edition(test_driver_lib::edition_version_or_current(source));
    compiler.set_library_paths(crate::parity::library_paths_for(
        source,
        path.parent().unwrap_or_else(|| std::path::Path::new("")),
    ));
    compiler.set_include_paths(
        test_driver_lib::extract_include_paths(source).map(Into::into).collect(),
    );
    // `//PARITY=` cases query elements by id (`//TRACE_ELEMENTS=`, text
    // metrics) — without debug info every query walks an empty tree (and
    // warns per element), silently verifying nothing.
    if source.contains("//PARITY=") {
        compiler.compiler_configuration(i_slint_core::InternalToken).debug_info = true;
    }
    let compiled = match poll_once(compiler.build_from_source(source.to_string(), path.to_owned()))
    {
        Some(result) => result,
        None => return Err("compilation never finished".into()),
    };
    if compiled.has_errors() {
        compiled.print_diagnostics();
        return Err(format!("build error in {path:?}").into());
    }
    Ok(compiled)
}

/// Compiles `source` and instantiates its last exported component.
pub fn compile_case(
    source: &str,
    path: &std::path::Path,
) -> Result<slint_interpreter::ComponentInstance, Box<dyn std::error::Error>> {
    let compiled = compile(source, path)?;
    let def = compiled.components().last().expect("There must be at least one exported component");
    def.create().map_err(|e| e.into())
}

/// The `(width, height)` of the `SIZE=WxH` marker, in physical pixels at
/// density 1. None when the case has no marker (the adapter falls back to the
/// component's preferred size).
pub fn case_size(source: &str) -> Option<(u32, u32)> {
    let size = source.lines().find_map(|l| l.trim().strip_prefix("//SIZE="))?.trim();
    let (w, h) = size.split_once('x')?;
    Some((w.trim().parse().ok()?, h.trim().parse().ok()?))
}

/// Converts an interpreter property value to a `TraceValue` for motion
/// capture.
pub fn trace_value(value: slint_interpreter::Value) -> crate::parity::TraceValue {
    use crate::parity::TraceValue;
    match value {
        slint_interpreter::Value::Number(n) => TraceValue::Number(n),
        slint_interpreter::Value::Bool(b) => TraceValue::Bool(b),
        slint_interpreter::Value::String(s) => TraceValue::Text(s.to_string()),
        slint_interpreter::Value::Brush(b) => {
            let c = b.color();
            TraceValue::Color([
                c.red() as f64 / 255.,
                c.green() as f64 / 255.,
                c.blue() as f64 / 255.,
                c.alpha() as f64 / 255.,
            ])
        }
        other => TraceValue::Other(format!("{other:?}")),
    }
}

pub fn poll_once<F: std::future::Future>(future: F) -> Option<F::Output> {
    let mut ctx = std::task::Context::from_waker(std::task::Waker::noop());
    let future = std::pin::pin!(future);
    match future.poll(&mut ctx) {
        std::task::Poll::Ready(result) => Some(result),
        std::task::Poll::Pending => None,
    }
}
