// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Helpers shared by the interpreter-based drivers (`skia`, `femtovg`,
//! `anyrender`): compiling a case, reading its `SIZE=` marker, and converting
//! interpreter values for motion traces.

/// Compiles `source` with the case's library and include paths and the fluent
/// style, aborting the test on diagnostics.
pub fn compile(
    source: &str,
    path: &std::path::Path,
) -> Result<slint_interpreter::CompilationResult, Box<dyn std::error::Error>> {
    let mut compiler = slint_interpreter::Compiler::default();
    compiler.set_style("fluent".into());
    compiler.set_library_paths(crate::parity::library_paths_for(
        source,
        path.parent().unwrap_or_else(|| std::path::Path::new("")),
    ));
    compiler.set_include_paths(
        test_driver_lib::extract_include_paths(source).map(Into::into).collect(),
    );
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
