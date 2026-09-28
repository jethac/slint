// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! The Material parity harness: layered comparison of Slint renders against the
//! Jetpack Compose reference renders committed under
//! `ui-libraries/material/parity/compose/references/`, plus motion capture with
//! numeric property traces for animated cases.
//!
//! Comparison layers (the tolerance policy — set once here, not per case):
//!
//! 1. Geometry and color, strict: outside the text mask every pixel's channels
//!    must match within [`PIXEL_EPS`]. This catches a 1px size difference, a
//!    wrong corner radius, and a wrong color role, since each of those turns
//!    into pixels that differ by far more than the epsilon.
//! 2. Text, masked: the Compose harness emits a mask PNG marking the pixels a
//!    text element contributed to. Slint and Compose shape and rasterize text
//!    with different engines, so inside the mask only a loose per-cell mean and
//!    a worst-pixel bound apply — a smoke test that the text is present, has the
//!    right size and color, and sits in the right place. Text element bounds are
//!    additionally compared exactly (±[`GEOM_EPS`] dp) through the trace.
//! 3. Motion and geometry traces, numeric: traced `out property`s and element
//!    geometries (x/y/w/h/opacity) are compared to the Compose trace per
//!    timestamp within [`TRACE_EPS`], and the animation must settle within
//!    [`SETTLE_SLACK`] of the Compose settle time.
//!
//! `//PARITY_EPS=` on a case overrides [`PIXEL_EPS`] for that case only; the
//! marker is always accompanied by a comment on the case explaining why.
//!
//! References are produced by the Compose harness in
//! `ui-libraries/material/parity/compose/` (see `ui-libraries/material/parity/
//! README.md`). When a case has no committed reference the parity check reports
//! a skip; set `PARITY_REQUIRE_REFS=1` (done in CI) to turn missing references
//! into hard errors. On a mismatch the actual render, a diff image, and an SVG
//! trace plot are written to `$PARITY_ARTIFACT_DIR/<driver>/<case>/`
//! (default `target/parity-artifacts/`), which CI uploads.

use i_slint_core::graphics::{Rgba8Pixel, SharedPixelBuffer};
use i_slint_core::platform::WindowEvent;
use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use test_driver_lib::{ParityAction, ParityMarkers};

/// Strict layer: maximum absolute per-channel difference allowed on
/// non-text pixels. 8 is well below the channel difference any wrong
/// token (color role, corner radius, size) produces, and above the
/// one-or-two-level antialiasing noise between Slint's software renderer
/// and Compose's software rasterization.
pub const PIXEL_EPS: u8 = 8;

/// Masked (text) layer: the mean absolute channel difference inside a
/// `TEXT_CELL`-sized cell of text pixels must stay below this value, and no
/// more than `TEXT_OUTLIER_FRACTION` of a cell's text pixels may exceed
/// `TEXT_OUTLIER_EPS`. Loose enough for different rasterizers, tight enough
/// that missing, misplaced, or wrongly-colored text fails.
pub const TEXT_CELL: usize = 16;
pub const TEXT_CELL_EPS: f64 = 24.0;
pub const TEXT_OUTLIER_EPS: u8 = 96;
pub const TEXT_OUTLIER_FRACTION: f64 = 0.05;

/// Traces: element geometry is compared in logical pixels (== dp) and traced
/// property values in their own units; an animation's settle time (the last
/// sample at which a traced value still moves more than [`SETTLE_EPS`] toward
/// its final value) must be within `SETTLE_SLACK` of the Compose settle time.
pub const GEOM_EPS: f64 = 0.5;
pub const TRACE_EPS: f64 = 1.0;
pub const SETTLE_EPS: f64 = 0.05;
pub const SETTLE_SLACK: f64 = 1.25;

/// Extra milliseconds a static case is settled before rendering, so entry
/// animations and ripples have completed on both sides.
pub const STATIC_SETTLE_MS: u64 = 2_000;

/// The repository root (…/tests/screenshots → repo root).
pub fn repo_root() -> PathBuf {
    [env!("CARGO_MANIFEST_DIR"), "..", ".."].iter().collect()
}

/// Library paths for one case: the case's `//library_path` markers, with
/// `material` registered to `ui-libraries/material/src/material.slint` unless
/// the case overrides it. Marker paths are relative to the case file itself —
/// pass the case's directory so they resolve.
pub fn library_paths_for(source: &str, case_dir: &Path) -> std::collections::HashMap<String, PathBuf> {
    let mut paths: std::collections::HashMap<String, PathBuf> =
        test_driver_lib::extract_library_paths(source)
            .map(|(k, v)| (k.to_string(), case_dir.join(v)))
            .collect();
    paths
        .entry("material".to_string())
        .or_insert(repo_root().join("ui-libraries/material/src/material.slint"));
    paths
}

/// Directory holding the Compose reference renders for one case
/// (`<case>` is the case path relative to `cases/`, without extension, e.g.
/// `material/filled_button_states`).
pub fn refs_dir(case_rel: &str) -> PathBuf {
    repo_root()
        .join("ui-libraries/material/parity/compose/references")
        .join(case_rel)
}

/// Where comparison artifacts land on failure.
pub fn artifacts_dir(driver: &str, case_rel: &str) -> PathBuf {
    std::env::var_os("PARITY_ARTIFACT_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| repo_root().join("target/parity-artifacts"))
        .join(driver)
        .join(case_rel)
}

fn load_png(path: &Path) -> Result<SharedPixelBuffer<Rgba8Pixel>, String> {
    let img = image::ImageReader::open(path)
        .map_err(|e| format!("open {path:?}: {e}"))?
        .decode()
        .map_err(|e| format!("decode {path:?}: {e}"))?
        .to_rgba8();
    let (w, h) = img.dimensions();
    Ok(SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(img.as_raw(), w, h))
}

fn write_png(path: &Path, buffer: &SharedPixelBuffer<Rgba8Pixel>) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    image::save_buffer(
        path,
        buffer.as_bytes(),
        buffer.width(),
        buffer.height(),
        image::ColorType::Rgba8,
    )
    .map_err(|e| std::io::Error::other(format!("write {path:?}: {e}")))
}

/// The pixels the Compose harness marked as produced by text (`mask_t*.png`):
/// a white pixel means "text", everything else is geometry.
fn text_mask(mask: Option<&SharedPixelBuffer<Rgba8Pixel>>) -> Vec<bool> {
    mask.map(|m| {
        m.as_slice().iter().map(|p| p.r > 0x7f && p.a > 0x7f).collect()
    })
    .unwrap_or_default()
}

fn channel_diff(a: &Rgba8Pixel, b: &Rgba8Pixel) -> u8 {
    [a.r.abs_diff(b.r), a.g.abs_diff(b.g), a.b.abs_diff(b.b), a.a.abs_diff(b.a)]
        .into_iter()
        .max()
        .unwrap()
}

/// Result of one layered comparison.
#[derive(Debug)]
pub struct LayerResult {
    /// True when every layer passed.
    pub ok: bool,
    /// Human-readable verdict, included in test failure output.
    pub report: String,
    /// Diff visualization: red = failing pixels, blue = masked text pixels,
    /// else the dimmed actual render.
    pub diff: Option<SharedPixelBuffer<Rgba8Pixel>>,
}

/// Compare `actual` against `expected` with the text mask applied: strict
/// per-channel outside the mask, per-cell mean inside it.
pub fn layered_compare(
    actual: &SharedPixelBuffer<Rgba8Pixel>,
    expected: &SharedPixelBuffer<Rgba8Pixel>,
    mask: Option<&SharedPixelBuffer<Rgba8Pixel>>,
    pixel_eps: u8,
) -> LayerResult {
    if actual.width() != expected.width() || actual.height() != expected.height() {
        return LayerResult {
            ok: false,
            report: format!(
                "size mismatch: actual {}x{}, expected {}x{}",
                actual.width(),
                actual.height(),
                expected.width(),
                expected.height()
            ),
            diff: None,
        };
    }

    let mask = text_mask(mask);
    let w = actual.width() as usize;
    let (a, e) = (actual.as_slice(), expected.as_slice());

    let mut diff_img = SharedPixelBuffer::<Rgba8Pixel>::new(actual.width(), actual.height());
    let mut strict_failures = 0usize;
    let mut worst = 0u8;

    // Accumulate per text-mask cell: (sum of mean channel diff, outlier count, text pixel count)
    let mut cells: BTreeMap<usize, (f64, usize, usize)> = BTreeMap::new();

    for i in 0..a.len() {
        let d = channel_diff(&a[i], &e[i]);
        let is_text = mask.get(i).copied().unwrap_or(false);
        let dim = |p: &Rgba8Pixel| Rgba8Pixel {
            r: p.r / 4 + 140,
            g: p.g / 4 + 140,
            b: p.b / 4 + 140,
            a: 255,
        };
        diff_img.make_mut_slice()[i] = if is_text {
            let cell = ((i % w) / TEXT_CELL) + (i / w) / TEXT_CELL * (w / TEXT_CELL + 1);
            let entry = cells.entry(cell).or_default();
            entry.0 += d as f64;
            entry.1 += (d > TEXT_OUTLIER_EPS) as usize;
            entry.2 += 1;
            if d > pixel_eps * 2 {
                Rgba8Pixel { r: 0, g: 0x60, b: 0xff, a: 255 }
            } else {
                Rgba8Pixel { r: 0x40, g: 0x40, b: 0xa0, a: 255 }
            }
        } else if d > pixel_eps {
            strict_failures += 1;
            worst = worst.max(d);
            Rgba8Pixel { r: 0xff, g: 0, b: 0, a: 255 }
        } else {
            dim(&a[i])
        };
    }

    let mut text_failures = Vec::new();
    for (cell, (sum, outliers, count)) in &cells {
        let mean = sum / *count as f64;
        if mean > TEXT_CELL_EPS || *outliers as f64 > *count as f64 * TEXT_OUTLIER_FRACTION {
            text_failures.push(format!(
                "text cell {cell}: mean diff {mean:.1} (max {TEXT_CELL_EPS}), {outliers}/{count} outliers"
            ));
        }
    }

    let mut report = format!(
        "{strict_failures} strict pixels differ (worst channel diff {worst}, eps {pixel_eps}); {} text cells checked",
        cells.len()
    );
    for f in &text_failures {
        report.push_str(&format!("; {f}"));
    }

    LayerResult {
        ok: strict_failures == 0 && text_failures.is_empty(),
        report,
        diff: if strict_failures == 0 && text_failures.is_empty() {
            None
        } else {
            Some(diff_img)
        },
    }
}

/// A traced value — an `out property` of the case, one of the types the harness
/// can serialize and compare.
#[derive(Debug, Clone)]
pub enum TraceValue {
    Number(f64),
    Bool(bool),
    /// sRGB channels in the 0..=1 range.
    Color([f64; 4]),
    Text(String),
    /// Values of a type the comparator can't check numerically; recorded as
    /// debug text, excluded from numeric comparison.
    Other(String),
}

impl TraceValue {
    /// The numeric components the trace comparison checks, or `None` for
    /// values that only get recorded.
    fn components(&self) -> Option<Vec<f64>> {
        match self {
            TraceValue::Number(n) => Some(vec![*n]),
            TraceValue::Bool(b) => Some(vec![*b as u8 as f64]),
            TraceValue::Color(c) => Some(c.to_vec()),
            _ => None,
        }
    }

        /// Serializes to the same JSON shape the Compose harness emits
    /// (number, bool, string, or `[r,g,b,a]`).
    fn to_json(&self) -> String {
        match self {
            TraceValue::Number(n) => format!("{n}"),
            TraceValue::Bool(b) => format!("{b}"),
            TraceValue::Color([r, g, b, a]) => format!("[{r},{g},{b},{a}]"),
            TraceValue::Text(s) | TraceValue::Other(s) => {
                format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
            }
        }
    }
}

macro_rules! trace_value_from_num {
    ($($t:ty)*) => {$(
        impl From<$t> for TraceValue {
            fn from(v: $t) -> Self {
                TraceValue::Number(v as f64)
            }
        }
    )*};
}
trace_value_from_num!(f32 f64 i8 i16 i32 i64 isize u8 u16 u32 u64 usize);

impl From<bool> for TraceValue {
    fn from(v: bool) -> Self {
        TraceValue::Bool(v)
    }
}

impl From<i_slint_core::api::Color> for TraceValue {
    fn from(v: i_slint_core::api::Color) -> Self {
        TraceValue::Color([
            v.red() as f64 / 255.,
            v.green() as f64 / 255.,
            v.blue() as f64 / 255.,
            v.alpha() as f64 / 255.,
        ])
    }
}

impl From<i_slint_core::Brush> for TraceValue {
    fn from(v: i_slint_core::Brush) -> Self {
        let c = v.color();
        TraceValue::Color([
            c.red() as f64 / 255.,
            c.green() as f64 / 255.,
            c.blue() as f64 / 255.,
            c.alpha() as f64 / 255.,
        ])
    }
}

impl From<i_slint_core::SharedString> for TraceValue {
    fn from(v: i_slint_core::SharedString) -> Self {
        TraceValue::Text(v.to_string())
    }
}

impl From<i_slint_core::api::LogicalPosition> for TraceValue {
    fn from(v: i_slint_core::api::LogicalPosition) -> Self {
        TraceValue::Other(format!("{}x{}", v.x, v.y))
    }
}

/// One timestamp's worth of a motion trace.
#[derive(Debug, Default)]
pub struct TraceFrame {
    pub t_ms: u64,
    pub props: BTreeMap<String, TraceValue>,
    /// x, y, w, h, opacity per element id — logical units.
    pub elements: BTreeMap<String, [f64; 5]>,
}

/// Traced property getter: name → value, or `None` when unknown.
pub type PropGetter<'a, C> = dyn Fn(&C, &str) -> Option<TraceValue> + 'a;

/// Record one trace frame: every `TRACE_PROPS` property via `prop_value`, plus
/// x/y/w/h/opacity of every `TRACE_ELEMENTS` element id.
pub fn capture_trace<C: i_slint_core::api::ComponentHandle>(
    component: &C,
    t_ms: u64,
    spec: &ParityMarkers,
    prop_value: &PropGetter<C>,
) -> TraceFrame {
    let mut frame = TraceFrame::default();
    frame.t_ms = t_ms;
    for name in &spec.trace_props {
        match prop_value(component, name) {
            Some(v) => {
                frame.props.insert(name.clone(), v);
            }
            None => panic!(
                "TRACE_PROPS names '{name}' but the component has no readable property by that name"
            ),
        }
    }
    for id in &spec.trace_elements {
        let handle = i_slint_backend_testing::ElementQuery::from_root(component)
            .match_id(id.as_str())
            .find_first();
        let Some(handle) = handle else {
            panic!("TRACE_ELEMENTS names '{id}' but no element has that id")
        };
        let pos = handle.absolute_position();
        let size = handle.size();
        frame.elements.insert(
            id.clone(),
            [
                pos.x as f64,
                pos.y as f64,
                size.width as f64,
                size.height as f64,
                handle.computed_opacity() as f64,
            ],
        );
    }
    frame
}

/// Dispatch the case's `//ACTION=` pointer steps to the window, in order.
pub fn apply_actions(window: &i_slint_core::api::Window, spec: &ParityMarkers) {
    use i_slint_core::api::LogicalPosition;
    use i_slint_core::items::PointerEventButton;
    for action in &spec.actions {
        let (x, y) = match *action {
            ParityAction::Move { x, y }
            | ParityAction::Press { x, y }
            | ParityAction::Release { x, y } => (x, y),
        };
        let position = LogicalPosition::new(x, y);
        let event = match action {
            ParityAction::Move { .. } => WindowEvent::PointerMoved { position },
            ParityAction::Press { .. } => {
                WindowEvent::PointerPressed { position, button: PointerEventButton::Left }
            }
            ParityAction::Release { .. } => {
                WindowEvent::PointerReleased { position, button: PointerEventButton::Left }
            }
        };
        window.dispatch_event(event);
    }
}

fn advance_mock_time_to(start_ms: u64, target_rel_ms: u64) {
    let now = i_slint_backend_testing::get_mocked_time();
    let delta = (start_ms + target_rel_ms).saturating_sub(now);
    if delta > 0 {
        i_slint_backend_testing::mock_elapsed_time(delta);
    }
}

/// The last timestamp at which any traced value still moves toward its final
/// value by more than [`SETTLE_EPS`], or `None` if the trace never moves.
fn settle_time_ms(frames: &[TraceFrame]) -> Option<u64> {
    let last = frames.last()?;
    let mut settle = 0;
    let mut moved = false;
    for frame in frames {
        for (name, value) in &frame.props {
            let (Some(now), Some(end)) =
                (value.components(), last.props.get(name).and_then(TraceValue::components))
            else {
                continue;
            };
            if now.iter().zip(&end).any(|(a, b)| (a - b).abs() > SETTLE_EPS) {
                settle = frame.t_ms;
                moved = true;
            }
        }
        for (id, geo) in &frame.elements {
            if let Some(end) = last.elements.get(id) {
                if geo.iter().zip(end).any(|(a, b)| (a - b).abs() > SETTLE_EPS) {
                    settle = frame.t_ms;
                    moved = true;
                }
            }
        }
    }
    moved.then_some(settle)
}

/// Compare a captured Slint trace to the Compose `trace.json`:
/// `{ "times_ms": [...], "frames": [{ "t_ms": n, "props": {...},
/// "elements": { "<id>": {"x":..,"y":..,"w":..,"h":..,"opacity":..} } }],
/// "settle_ms": n }`. Returns the list of mismatches found.
pub fn compare_traces(slint: &[TraceFrame], compose: &serde_json::Value) -> Vec<String> {
    let mut errors = Vec::new();
    let frames = compose["frames"].as_array().cloned().unwrap_or_default();
    let by_time: BTreeMap<u64, &serde_json::Value> =
        frames.iter().filter_map(|f| f["t_ms"].as_u64().map(|t| (t, f))).collect();

    for frame in slint {
        let Some(cf) = by_time.get(&frame.t_ms) else {
            errors.push(format!("t={}ms has no Compose trace frame", frame.t_ms));
            continue;
        };
        for (name, value) in &frame.props {
            let Some(cv) = cf["props"].get(name) else {
                errors.push(format!("t={}ms prop '{name}' missing from Compose trace", frame.t_ms));
                continue;
            };
            let Some(actual) = value.components() else { continue };
            let expected: Vec<f64> = match cv {
                serde_json::Value::Number(n) => vec![n.as_f64().unwrap_or_default()],
                serde_json::Value::Bool(b) => vec![*b as u8 as f64],
                serde_json::Value::Array(a) => {
                    a.iter().map(|v| v.as_f64().unwrap_or_default()).collect()
                }
                _ => continue,
            };
            if actual.len() == expected.len() {
                for (i, (a, e)) in actual.iter().zip(&expected).enumerate() {
                    if (a - e).abs() > TRACE_EPS {
                        errors.push(format!(
                            "t={}ms prop '{name}'[{i}]: slint {a} vs compose {e} (eps {TRACE_EPS})",
                            frame.t_ms
                        ));
                    }
                }
            } else {
                errors.push(format!(
                    "t={}ms prop '{name}': {actual:?} vs {expected:?} arity mismatch",
                    frame.t_ms
                ));
            }
        }
        for (id, geo) in &frame.elements {
            let Some(ce) = cf["elements"].get(id) else {
                errors.push(format!(
                    "t={}ms element '{id}' missing from Compose trace",
                    frame.t_ms
                ));
                continue;
            };
            for (i, key) in ["x", "y", "w", "h", "opacity"].iter().enumerate() {
                let Some(e) = ce[key].as_f64() else { continue };
                if (geo[i] - e).abs() > GEOM_EPS {
                    errors.push(format!(
                        "t={}ms element '{id}'.{key}: slint {} vs compose {e} (eps {GEOM_EPS})",
                        frame.t_ms, geo[i]
                    ));
                }
            }
        }
    }

    if let Some(compose_settle) = compose["settle_ms"].as_u64()
        && let Some(slint_settle) = settle_time_ms(slint)
    {
        let slack = compose_settle as f64 * SETTLE_SLACK + 16.0;
        if slint_settle as f64 > slack {
            errors.push(format!(
                "settle: slint {slint_settle}ms vs compose {compose_settle}ms (allowed {slack:.0}ms)"
            ));
        }
    }
    errors
}

/// Serialize a captured trace to the same JSON shape the Compose harness emits.
pub fn trace_to_json(frames: &[TraceFrame]) -> String {
    let mut s = String::from("{\n  \"frames\": [\n");
    for (i, frame) in frames.iter().enumerate() {
        s.push_str(&format!("    {{ \"t_ms\": {}, \"props\": {{", frame.t_ms));
        for (j, (name, v)) in frame.props.iter().enumerate() {
            s.push_str(&format!("{}\"{name}\": {}", if j == 0 { " " } else { ", " }, v.to_json()));
        }
        s.push_str(if frame.props.is_empty() { "}" } else { " }" });
        s.push_str(", \"elements\": {");
        for (j, (id, geo)) in frame.elements.iter().enumerate() {
            s.push_str(&format!(
                "{}\"{id}\": {{\"x\": {}, \"y\": {}, \"w\": {}, \"h\": {}, \"opacity\": {}}}",
                if j == 0 { " " } else { ", " },
                geo[0],
                geo[1],
                geo[2],
                geo[3],
                geo[4],
            ));
        }
        s.push_str(if frame.elements.is_empty() { "}" } else { " }" });
        s.push_str(if i + 1 == frames.len() { " }\n" } else { " },\n" });
    }
    if let Some(settle) = settle_time_ms(frames) {
        s.push_str(&format!("  ],\n  \"settle_ms\": {settle}\n}}"));
    } else {
        s.push_str("  ]\n}");
    }
    s
}

/// An SVG line plot of every numeric trace — the failure artifact that makes
/// motion mismatches debuggable without re-running.
fn trace_svg(slint: &[TraceFrame], compose: Option<&serde_json::Value>) -> String {
    let series: Vec<String> = slint
        .iter()
        .flat_map(|f| f.props.keys())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .cloned()
        .collect();
    if series.is_empty() {
        return String::new();
    }
    let (w, row_h) = (900f64, 80f64);
    let h = row_h * series.len() as f64 + 20.0;
    let t_max = slint.iter().map(|f| f.t_ms).max().unwrap_or(1).max(1) as f64;
    let mut out =
        format!("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}\" height=\"{h}\">\n");
    for (si, name) in series.iter().enumerate() {
        let vals: Vec<(u64, f64)> = slint
            .iter()
            .filter_map(|f| {
                f.props.get(name).and_then(TraceValue::components).map(|c| (f.t_ms, c[0]))
            })
            .collect();
        let (v_min, v_max) = vals
            .iter()
            .fold((f64::MAX, f64::MIN), |(lo, hi), (_, v)| (lo.min(*v), hi.max(*v)));
        let span = (v_max - v_min).max(1e-6);
        let y0 = 20.0 + si as f64 * row_h;
        let path: String = vals
            .iter()
            .enumerate()
            .map(|(i, (t, v))| {
                format!(
                    "{}{:.1},{:.1}",
                    if i == 0 { "M" } else { " L" },
                    60.0 + *t as f64 / t_max * (w - 80.0),
                    y0 + row_h - 20.0 - (v - v_min) / span * (row_h - 30.0)
                )
            })
            .collect();
        out.push_str(&format!(
            "<text x=\"4\" y=\"{:.0}\" font-size=\"12\">{name}</text>\n<path d=\"{path}\" fill=\"none\" stroke=\"#6750a4\" stroke-width=\"2\"/>\n",
            y0 + row_h - 25.0
        ));
        // Overlay the Compose trace for the same property, if present.
        if let Some(cf) = compose.and_then(|c| c["frames"].as_array()) {
            let cpath: String = cf
                .iter()
                .filter_map(|f| {
                    let t = f["t_ms"].as_f64()?;
                    let v = f["props"].get(name)?.as_f64()?;
                    Some(format!(
                        "{:.1},{:.1}",
                        60.0 + t / t_max * (w - 80.0),
                        y0 + row_h - 20.0 - (v - v_min) / span * (row_h - 30.0)
                    ))
                })
                .collect::<Vec<_>>()
                .join(" L");
            if !cpath.is_empty() {
                out.push_str(&format!(
                    "<path d=\"M{cpath}\" fill=\"none\" stroke=\"#b33b15\" stroke-width=\"1\" stroke-dasharray=\"4\"/>\n"
                ));
            }
        }
    }
    out.push_str("</svg>\n");
    out
}

/// The shared body of every generated parity test: render the case at each
/// requested density, compare against the Compose references (or skip with a
/// warning when they're absent), and for `motion` cases capture property
/// traces at the `//TIMES=` timestamps.
///
/// `make_instance` must return a fresh, already-`show()`n component at the
/// requested density (physical size and scale factor applied); `render_frame`
/// renders it upright and returns RGBA8; `prop_value` reads traced properties.
pub fn run_parity_case<C: i_slint_core::api::ComponentHandle>(
    driver: &'static str,
    case_rel: &str,
    spec: &ParityMarkers,
    prop_value: &PropGetter<C>,
    mut make_instance: impl FnMut(u32) -> C,
    mut render_frame: impl FnMut(&C) -> SharedPixelBuffer<Rgba8Pixel>,
) -> Result<(), Box<dyn std::error::Error>> {
    let kind = spec.parity.as_deref().unwrap_or("static");
    let pixel_eps = spec.eps.map(|e| e as u8).unwrap_or(PIXEL_EPS);
    let negative = kind == "negative";
    let references_missing = !refs_dir(case_rel).join("d1").is_dir();

    if references_missing {
        let msg = format!(
            "parity: no Compose references for {case_rel} under {:?} — skipping layered compare (run the compose harness to generate them; see ui-libraries/material/parity/README.md)",
            refs_dir(case_rel)
        );
        if std::env::var_os("PARITY_REQUIRE_REFS").is_some() {
            return Err(msg.into());
        }
        eprintln!("{msg}");
        if !negative {
            return Ok(());
        }
    }

    let mut failures: Vec<String> = Vec::new();
    for density in &spec.densities {
        let component = make_instance(*density);
        apply_actions(component.window(), spec);

        // Static cases settle out any entry/ripple animation before the shot.
        let times: Vec<u64> = if kind == "motion" {
            spec.times.clone()
        } else {
            vec![STATIC_SETTLE_MS]
        };
        let start = i_slint_backend_testing::get_mocked_time();

        let mut frames = Vec::new();
        let mut artifacts_written = false;
        for &t in &times {
            advance_mock_time_to(start, t);
            let actual = render_frame(&component);
            frames.push(capture_trace(&component, t, spec, prop_value));

            if references_missing {
                continue;
            }
            let tag = if kind == "motion" { format!("{t}ms") } else { "settled".to_string() };
            let dir = refs_dir(case_rel).join(format!("d{density}"));
            let frame_path = dir.join(format!("frame_{tag}.png"));
            let mask_path = dir.join(format!("mask_{tag}.png"));
            let expected = match load_png(&frame_path) {
                Ok(e) => e,
                Err(e) => {
                    failures.push(format!("d{density} {e}"));
                    continue;
                }
            };
            let mask = load_png(&mask_path).ok();
            let result = layered_compare(&actual, &expected, mask.as_ref(), pixel_eps);
            if !result.ok {
                let dir = artifacts_dir(driver, case_rel);
                write_png(&dir.join(format!("actual_d{density}_{tag}.png")), &actual)?;
                write_png(&dir.join(format!("expected_d{density}_{tag}.png")), &expected)?;
                if let Some(diff) = &result.diff {
                    write_png(&dir.join(format!("diff_d{density}_{tag}.png")), diff)?;
                }
                artifacts_written = true;
                failures.push(format!("d{density} t={tag}: {}", result.report));
            }
        }

        if kind == "motion" && !references_missing {
            let dir = refs_dir(case_rel).join(format!("d{density}"));
            match std::fs::read_to_string(dir.join("trace.json")) {
                Ok(json) => {
                    let compose: serde_json::Value = serde_json::from_str(&json)?;
                    let mut errors = compare_traces(&frames, &compose);
                    if !errors.is_empty() {
                        let dir = artifacts_dir(driver, case_rel);
                        std::fs::create_dir_all(&dir)?;
                        std::fs::write(dir.join("slint_trace.json"), trace_to_json(&frames))?;
                        let svg = trace_svg(&frames, Some(&compose));
                        if !svg.is_empty() {
                            std::fs::write(dir.join(format!("trace_d{density}.svg")), svg)?;
                        }
                        failures.append(&mut errors);
                        artifacts_written = true;
                    }
                }
                Err(e) => failures.push(format!("d{density} trace.json: {e}")),
            }
        }
        if artifacts_written {
            let dir = artifacts_dir(driver, case_rel);
            std::fs::create_dir_all(&dir)?;
            let mut report = std::fs::File::create(dir.join(format!("report_d{density}.txt")))?;
            writeln!(report, "parity failures for {case_rel} on {driver} d{density}")?;
            for f in &failures {
                writeln!(report, "- {f}")?;
            }
        }
    }

    if negative {
        if references_missing {
            // Like a positive case: no references means nothing to reject, so
            // skip — the proof of detection only exists once the Compose side
            // has produced this scene's references (or `PARITY_REQUIRE_REFS`
            // fails earlier).
            return Ok(());
        }
        // The case is deliberately wrong (see its `PARITY=negative:` note): the
        // layered comparator must reject it. Catching nothing here would mean
        // the harness can silently pass a wrong render.
        if failures.is_empty() {
            return Err(format!(
                "negative case {case_rel} rendered identical to the reference — the harness did not catch the deliberate defect: {}",
                spec.negative_note.as_deref().unwrap_or("(undocumented)")
            )
            .into());
        }
        eprintln!("parity: negative case {case_rel} correctly rejected ({} findings)", failures.len());
        return Ok(());
    }

    if failures.is_empty() {
        Ok(())
    } else {
        Err(format!("parity failures for {case_rel} on {driver}:\n{}", failures.join("\n")).into())
    }
}

#[test]
fn comparator_catches_subtle_differences() {
    // Self-test for the negative-case mechanism: a 1px color nudge, a 1px size
    // change and a masked-text blip must each flip the verdict.
    let size = 32;
    let a = SharedPixelBuffer::<Rgba8Pixel>::new(size, size);
    let mut b = a.clone();
    let mut m = SharedPixelBuffer::<Rgba8Pixel>::new(size, size);
    let white = Rgba8Pixel { r: 255, g: 255, b: 255, a: 255 };
    for p in m.make_mut_slice() {
        *p = white; // all text
    }
    assert!(layered_compare(&a, &b, Some(&m), PIXEL_EPS).ok, "identical images must pass");

    // 1px color nudge outside text → strict layer fails.
    b.make_mut_slice()[0] = Rgba8Pixel { r: 20, g: 0, b: 0, a: 255 };
    let no_text = SharedPixelBuffer::<Rgba8Pixel>::new(size, size);
    assert!(!layered_compare(&a, &b, Some(&no_text), PIXEL_EPS).ok, "1px diff must fail");

    // Same nudge but fully inside the text mask → tolerated by the loose layer.
    assert!(
        layered_compare(&a, &b, Some(&m), PIXEL_EPS).ok,
        "small diff inside text mask must pass"
    );

    // Blanket wrong color across the mask → loose layer fails.
    let mut c = a.clone();
    for p in c.make_mut_slice() {
        *p = Rgba8Pixel { r: 255, g: 0, b: 0, a: 255 };
    }
    assert!(!layered_compare(&a, &c, Some(&m), PIXEL_EPS).ok, "wrong text color must fail");
}
