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
//!    text element contributed to; the harness unions in the Slint-side text
//!    rects too, since the engines place a label's ink a pixel or so apart.
//!    Slint and Compose shape and rasterize text with different engines, so
//!    inside the mask only a loose per-cell mean and a worst-pixel bound
//!    apply — a smoke test that the text is present, has the right size and
//!    color, and sits in the right place. Text width is validated against
//!    the font's own metrics (`frac_w` in the trace) within [`GEOM_EPS`];
//!    layoutlib's hinted layout inflates `w` by design, so the hinted
//!    cross-engine drift is bounded instead of checked exactly.
//! 3. Motion and geometry traces, numeric: traced `out property`s and element
//!    geometries (x/y/w/h/opacity) are compared to the Compose trace per
//!    timestamp within [`TRACE_EPS`], and the animation must settle within
//!    [`SETTLE_SLACK`] of the Compose settle time. Where an element is traced
//!    on both sides, its pixel-level disagreement band — outline
//!    antialiasing plus whatever lies between the two bounds — is skipped by
//!    the strict layer and verified numerically instead (a text-sized
//!    element's `w` additionally tolerates the hinted-text drift).
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
use i_slint_core::SharedString;
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
pub const TEXT_CELL_EPS: f64 = 40.0;
pub const TEXT_OUTLIER_EPS: u8 = 96;
pub const TEXT_OUTLIER_FRACTION: f64 = 0.25;

/// Strict layer inside the outline-disagreement band is skipped entirely;
/// pixels on a detected image edge (local gradient above `EDGE_GRADIENT_*`
/// in either render) get `EDGE_EPS` instead — the antialiasing band of a
/// shape's outline is rasterizer internals and legitimately differs between
/// Slint's and layoutlib's renderers, while a wrong color or a displaced
/// shape still produces differences far above the edge epsilon on flat
/// regions or in fill interiors.
pub const EDGE_EPS: u8 = 56;
pub const EDGE_GRADIENT_SOFT: u8 = 24;

/// Outline detection for the disagreement band inside traced elements' union
/// rects — includes faint decorations (focus rings, elevation fringes) whose
/// exact placement is engine detail, well above flat noise.
pub const EDGE_GRADIENT_OUTLINE: u8 = 10;

/// Within a traced element pair's union rect, pixels this close to a
/// detected outline are skipped as outline-disagreement zone — both sides'
/// outlines and whatever lies between them when the engines' geometry
/// legitimately differs (text-driven widths, animation phase). Covers a
/// capsule corner arc's inset (~6px) plus a few px of traced drift.
pub const DISAGREE_BAND: usize = 10;

/// Decorations attached to a traced element — elevation shadow, focus
/// outline — may spill a few dp outside its bounds. The disagreement rect
/// is grown by this many logical px so the outline-disagreement zone covers
/// them; their pixels are only skipped where a detected outline exists.
pub const DECORATION_MARGIN_DP: f64 = 6.0;

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

/// Per-pixel layer classification for the layered comparison.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PixelClass {
    /// Strict per-channel comparison (geometry and fills).
    Strict,
    /// The loose per-cell text comparison — pixels a text node contributed.
    Text,
    /// Not compared at all: pixels where the two engines legitimately
    /// disagree — the disagreement band of a traced element (both sides'
    /// outlines and everything between them when their geometry differs —
    /// text metrics drive the width, animation phase the position — and the
    /// anti-aliased outline of a traced element, which is rasterizer
    /// internals). The numeric trace layer still checks these elements'
    /// geometry; only their pixels skip.
    Skip,
}

/// A simple axis-aligned rect in device pixels, for mask painting.
#[derive(Clone, Copy, Debug)]
struct PxRect {
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
}

impl PxRect {
    fn contains(&self, x: f64, y: f64) -> bool {
        x >= self.x0 && x < self.x1 && y >= self.y0 && y < self.y1
    }

    fn dilated(&self, px: f64) -> Self {
        Self { x0: self.x0 - px, y0: self.y0 - px, x1: self.x1 + px, y1: self.y1 + px }
    }
}

/// Per-pixel comparison layers for [`layered_compare`].
pub struct PixelMask {
    layer: Vec<PixelClass>,
    /// Union of each traced element pair's bounds: inside one of these rects
    /// the strict layer skips the outline-disagreement band — pixels within
    /// [`DISAGREE_BAND`] of a strong image edge. The elements' geometry is
    /// verified numerically by the trace layer instead.
    disagreement: Vec<PxRect>,
    w: usize,
    h: usize,
}

impl PixelMask {
    /// Every pixel strict.
    fn new(w: u32, h: u32) -> Self {
        Self {
            layer: vec![PixelClass::Strict; (w * h) as usize],
            disagreement: Vec::new(),
            w: w as usize,
            h: h as usize,
        }
    }

    /// The Compose text mask PNG (`mask_t*.png`): white pixels are text.
    fn from_text_png(m: &SharedPixelBuffer<Rgba8Pixel>) -> Self {
        let layer = m
            .as_slice()
            .iter()
            .map(|p| {
                if p.r > 0x7f && p.a > 0x7f { PixelClass::Text } else { PixelClass::Strict }
            })
            .collect();
        Self { layer, disagreement: Vec::new(), w: m.width() as usize, h: m.height() as usize }
    }

    fn set(&mut self, x: usize, y: usize, class: PixelClass) {
        if x < self.w && y < self.h {
            self.layer[y * self.w + x] = class;
        }
    }

    /// Fill `r` (device px) with `class`.
    fn fill_rect(&mut self, r: PxRect, class: PixelClass) {
        for y in r.y0.floor().max(0.0) as usize..(r.y1.ceil() as usize).min(self.h) {
            for x in r.x0.floor().max(0.0) as usize..(r.x1.ceil() as usize).min(self.w) {
                self.set(x, y, class);
            }
        }
    }

    /// Record the traced element pair `slint`/`compose` (device px): its
    /// symmetric difference is skipped, and the pair's union — grown by
    /// `margin` for decorations spilling outside the bounds — becomes a
    /// `disagreement` rect inside which the strict layer also skips the
    /// outline-disagreement band (see [`layered_compare`]).
    fn mark_element(&mut self, slint: PxRect, compose: PxRect, margin: f64) {
        let (a, b) = (slint.dilated(1.0), compose.dilated(1.0));
        let u = PxRect {
            x0: a.x0.min(b.x0),
            y0: a.y0.min(b.y0),
            x1: a.x1.max(b.x1),
            y1: a.y1.max(b.y1),
        };
        self.disagreement.push(u.dilated(margin));
        let (x0, y0) =
            (a.x0.min(b.x0).floor().max(0.0) as usize, a.y0.min(b.y0).floor().max(0.0) as usize);
        let (x1, y1) = (a.x1.max(b.x1).ceil() as usize, a.y1.max(b.y1).ceil() as usize);
        for y in y0..y1.min(self.h) {
            for x in x0..x1.min(self.w) {
                let (px, py) = (x as f64 + 0.5, y as f64 + 0.5);
                if a.contains(px, py) != b.contains(px, py) {
                    self.set(x, y, PixelClass::Skip);
                }
            }
        }
    }
}

fn channel_diff(a: &Rgba8Pixel, b: &Rgba8Pixel) -> u8 {
    [a.r.abs_diff(b.r), a.g.abs_diff(b.g), a.b.abs_diff(b.b), a.a.abs_diff(b.a)]
        .into_iter()
        .max()
        .unwrap()
}

/// Per-pixel local contrast: the strongest channel gradient between the
/// pixel and its horizontal/vertical neighbors in `img`. A shape outline
/// shows up as high contrast; flat regions read zero.
fn local_gradient(img: &SharedPixelBuffer<Rgba8Pixel>) -> Vec<u8> {
    let (w, h) = (img.width() as usize, img.height() as usize);
    let px = img.as_slice();
    let mut g = vec![0u8; px.len()];
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            let mut m = 0;
            if x + 1 < w {
                m = m.max(channel_diff(&px[i], &px[i + 1]));
            }
            if x > 0 {
                m = m.max(channel_diff(&px[i], &px[i - 1]));
            }
            if y + 1 < h {
                m = m.max(channel_diff(&px[i], &px[i + w]));
            }
            if y > 0 {
                m = m.max(channel_diff(&px[i], &px[i - w]));
            }
            g[i] = m;
        }
    }
    g
}

/// Square max-dilation: every pixel takes the largest value within `r`.
fn dilate_max(g: &[u8], w: usize, h: usize, r: usize) -> Vec<u8> {
    let mut out = g.to_vec();
    let r = r as i64;
    for y in 0..h {
        for x in 0..w {
            if g[y * w + x] == 0 {
                continue;
            }
            for dy in -r..=r {
                for dx in -r..=r {
                    let (nx, ny) = (x as i64 + dx, y as i64 + dy);
                    if nx >= 0 && ny >= 0 && (nx as usize) < w && (ny as usize) < h {
                        let j = ny as usize * w + nx as usize;
                        out[j] = out[j].max(g[y * w + x]);
                    }
                }
            }
        }
    }
    out
}

/// Square dilation of a boolean mask by `r` pixels.
fn dilate(mask: &[bool], w: usize, h: usize, r: usize) -> Vec<bool> {
    let mut out = mask.to_vec();
    let r = r as i64;
    for y in 0..h {
        for x in 0..w {
            if !mask[y * w + x] {
                continue;
            }
            for dy in -r..=r {
                for dx in -r..=r {
                    let (nx, ny) = (x as i64 + dx, y as i64 + dy);
                    if nx >= 0 && ny >= 0 && (nx as usize) < w && (ny as usize) < h {
                        out[ny as usize * w + nx as usize] = true;
                    }
                }
            }
        }
    }
    out
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

/// Compare `actual` against `expected` under the per-pixel layer mask:
/// strict per-channel outside text and skip regions, per-cell mean inside
/// the text mask, nothing inside skip bands.
pub fn layered_compare(
    actual: &SharedPixelBuffer<Rgba8Pixel>,
    expected: &SharedPixelBuffer<Rgba8Pixel>,
    mask: Option<&PixelMask>,
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

    let empty_mask = PixelMask::new(actual.width(), actual.height());
    let mask = mask.unwrap_or(&empty_mask);
    if mask.w != actual.width() as usize || mask.h != actual.height() as usize {
        return LayerResult {
            ok: false,
            report: format!(
                "mask size mismatch: {}x{} vs actual {}x{}",
                mask.w,
                mask.h,
                actual.width(),
                actual.height()
            ),
            diff: None,
        };
    }
    let w = actual.width() as usize;
    let h = actual.height() as usize;
    let (a, e) = (actual.as_slice(), expected.as_slice());

    // Edge strength from both images: pixels within 2px of an outline get
    // an epsilon proportional to the local contrast — antialiasing drift is
    // a fraction of the edge's own contrast, while a real coverage
    // difference (displaced or misshapen outline) produces diffs at full
    // contrast and still fails. Inside a traced disagreement rect, the
    // wider band around detected outlines skips the legitimate
    // outline-disagreement zone.
    let gradient: Vec<u8> = local_gradient(actual)
        .into_iter()
        .zip(local_gradient(&expected))
        .map(|(a, b)| a.max(b))
        .collect();
    let strength = dilate_max(&gradient, w, h, 2);
    let outline_zone = if mask.disagreement.is_empty() {
        Vec::new()
    } else {
        let edges: Vec<bool> = gradient.iter().map(|&g| g > EDGE_GRADIENT_OUTLINE).collect();
        dilate(&edges, w, h, DISAGREE_BAND)
    };

    let mut diff_img = SharedPixelBuffer::<Rgba8Pixel>::new(actual.width(), actual.height());
    let mut strict_failures = 0usize;
    let mut worst = 0u8;

    // Accumulate per text-mask cell: (sum of mean channel diff, outlier count, text pixel count)
    let mut cells: BTreeMap<usize, (f64, usize, usize)> = BTreeMap::new();

    for i in 0..a.len() {
        let d = channel_diff(&a[i], &e[i]);
        let (x, y) = (i % w, i / w);
        let in_disagreement = mask.disagreement.iter().any(|r| {
            x as f64 >= r.x0 && x as f64 + 1.0 <= r.x1 && y as f64 >= r.y0 && y as f64 + 1.0 <= r.y1
        });
        let class = if in_disagreement && outline_zone.get(i).copied().unwrap_or(false) {
            PixelClass::Skip
        } else {
            mask.layer[i]
        };
        let dim = |p: &Rgba8Pixel| Rgba8Pixel {
            r: p.r / 4 + 140,
            g: p.g / 4 + 140,
            b: p.b / 4 + 140,
            a: 255,
        };
        diff_img.make_mut_slice()[i] = match class {
            PixelClass::Skip => Rgba8Pixel { r: 0x80, g: 0x40, b: 0x80, a: 255 },
            PixelClass::Text => {
                let cell = (x / TEXT_CELL) + (y / TEXT_CELL) * (w / TEXT_CELL + 1);
                let entry = cells.entry(cell).or_default();
                entry.0 += d as f64;
                entry.1 += (d > TEXT_OUTLIER_EPS) as usize;
                entry.2 += 1;
                if d > pixel_eps * 2 {
                    Rgba8Pixel { r: 0, g: 0x60, b: 0xff, a: 255 }
                } else {
                    Rgba8Pixel { r: 0x40, g: 0x40, b: 0xa0, a: 255 }
                }
            }
            PixelClass::Strict => {
                // AA drift on an outline is bounded by the outline's own
                // contrast: a difference larger than half the local
                // contrast means real coverage disagreement.
                let eps = if strength[i] > EDGE_GRADIENT_SOFT {
                    ((strength[i] as u16) / 2).clamp(pixel_eps as u16, EDGE_EPS as u16) as u8
                } else {
                    pixel_eps
                };
                if d > eps {
                    strict_failures += 1;
                    worst = worst.max(d);
                    Rgba8Pixel { r: 0xff, g: 0, b: 0, a: 255 }
                } else {
                    dim(&a[i])
                }
            }
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
        // Element ids are emitted component-qualified (`TestCase::thumb`).
        let id_owned = id.clone();
        let handle = i_slint_backend_testing::ElementQuery::from_root(component)
            .match_predicate(move |e| {
                e.id()
                    .map(|candidate| {
                        let candidate = candidate.as_str();
                        candidate == id_owned || candidate.ends_with(&format!("::{id_owned}"))
                    })
                    .unwrap_or(false)
            })
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

/// Dispatch the case's `//ACTION=` input steps to the window, in order.
pub fn apply_actions(window: &i_slint_core::api::Window, spec: &ParityMarkers) {
    use i_slint_core::api::LogicalPosition;
    use i_slint_core::items::PointerEventButton;
    for action in &spec.actions {
        if let ParityAction::Key { name } = action {
            use i_slint_core::input::key_codes::Key;
            let text: SharedString = match name.as_str() {
                "Tab" => Key::Tab.into(),
                "Backtab" => Key::Backtab.into(),
                "Escape" => Key::Escape.into(),
                "Space" => SharedString::from(" "),
                "Return" => Key::Return.into(),
                other => {
                    let mut c = other.chars();
                    match (c.next(), c.next()) {
                        (Some(c), None) => c.to_string().into(),
                        _ => panic!("Unknown //ACTION= key '{other}'"),
                    }
                }
            };
            window.dispatch_event(WindowEvent::KeyPressed { text: text.clone() });
            window.dispatch_event(WindowEvent::KeyReleased { text });
            continue;
        }
        let (x, y) = match *action {
            ParityAction::Move { x, y }
            | ParityAction::Press { x, y }
            | ParityAction::Release { x, y } => (x, y),
            ParityAction::Key { .. } => unreachable!(),
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
            ParityAction::Key { .. } => unreachable!(),
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

    // Slint and Compose quantize the spring's start tick differently (the
    // pointer event lands mid-frame on Slint's mocked clock; Compose starts
    // the animation on the next frame callback) — the same ODE off by a frame
    // or two. Compare each Slint sample against Compose frames within
    // PHASE_MS of its timestamp and take the nearest.
    const PHASE_MS: i64 = 4;
    let compose_at = |t: u64| -> Vec<&serde_json::Value> {
        let lo = t.saturating_sub(PHASE_MS as u64);
        (lo..=t + PHASE_MS as u64)
            .filter_map(|tt| by_time.get(&tt).copied())
            .collect()
    };

    for frame in slint {
        let cands = compose_at(frame.t_ms);
        if cands.is_empty() {
            errors.push(format!("t={}ms has no Compose trace frame", frame.t_ms));
            continue;
        }
        for (name, value) in &frame.props {
            let Some(actual) = value.components() else { continue };
            let mut best: Option<(Vec<f64>, f64)> = None;
            for cf in &cands {
                let Some(cv) = cf["props"].get(name) else { continue };
                let expected: Vec<f64> = match cv {
                    serde_json::Value::Number(n) => vec![n.as_f64().unwrap_or_default()],
                    serde_json::Value::Bool(b) => vec![*b as u8 as f64],
                    serde_json::Value::Array(a) => {
                        a.iter().map(|v| v.as_f64().unwrap_or_default()).collect()
                    }
                    _ => continue,
                };
                if expected.len() != actual.len() {
                    errors.push(format!(
                        "t={}ms prop '{name}': {actual:?} vs {expected:?} arity mismatch",
                        frame.t_ms
                    ));
                    continue;
                }
                let err: f64 = actual
                    .iter()
                    .zip(&expected)
                    .map(|(a, e)| (a - e).abs())
                    .fold(0.0, f64::max);
                if best.as_ref().is_none_or(|(_, e)| err < *e) {
                    best = Some((expected, err));
                }
            }
            match best {
                None => errors.push(format!(
                    "t={}ms prop '{name}' missing from Compose trace",
                    frame.t_ms
                )),
                Some((expected, err)) if err > TRACE_EPS => {
                    for (i, (a, e)) in actual.iter().zip(&expected).enumerate() {
                        if (a - e).abs() > TRACE_EPS {
                            errors.push(format!(
                                "t={}ms prop '{name}'[{i}]: slint {a} vs compose {e} (eps {TRACE_EPS})",
                                frame.t_ms
                            ));
                        }
                    }
                }
                _ => {}
            }
        }
        for (id, geo) in &frame.elements {
            // An element named `button<N>` (or any `*<N>`) sizes itself to
            // `text:<N>`: its w legitimately differs by that text's hinting
            // drift, bounded here instead of pixel-strict.
            let drift = id
                .chars()
                .rev()
                .take_while(char::is_ascii_digit)
                .collect::<String>()
                .chars()
                .rev()
                .collect::<String>();
            let drift_eps = |cf: &serde_json::Value| -> f64 {
                let m = cf["text"].get(format!("text:{drift}"));
                m.and_then(|m| {
                    m["frac_w"].as_f64().map(|f| (m["w"].as_f64().unwrap_or(f) - f).abs())
                })
                .unwrap_or(0.0)
                    + GEOM_EPS
                    + 1.0
            };
            let mut best: Option<(&serde_json::Value, f64, u64)> = None;
            for cf in &cands {
                let Some(ce) = cf["elements"].get(id) else { continue };
                let w_eps = if drift.is_empty() { GEOM_EPS } else { drift_eps(cf) };
                let err: f64 = ["x", "y", "w", "h", "opacity"]
                    .iter()
                    .enumerate()
                    .filter_map(|(i, key)| {
                        let eps = if *key == "w" { w_eps } else { GEOM_EPS };
                        ce[key].as_f64().map(|e| (geo[i] - e).abs() / eps.max(GEOM_EPS))
                    })
                    .fold(0.0, f64::max);
                if best.as_ref().is_none_or(|(_, e, _)| err < *e) {
                    best = Some((cf, err, cf["t_ms"].as_u64().unwrap_or(0)));
                }
            }
            match best {
                None => errors.push(format!(
                    "t={}ms element '{id}' missing from Compose trace",
                    frame.t_ms
                )),
                Some((cf, err, ct)) if err > 1.0 => {
                    let ce = &cf["elements"][id];
                    let w_eps =
                        if drift.is_empty() { GEOM_EPS } else { drift_eps(cf) };
                    for (i, key) in ["x", "y", "w", "h", "opacity"].iter().enumerate() {
                        let Some(e) = ce[key].as_f64() else { continue };
                        let eps = if *key == "w" { w_eps } else { GEOM_EPS };
                        if (geo[i] - e).abs() > eps {
                            errors.push(format!(
                                "t={}ms element '{id}'.{key}: slint {} vs compose@{ct}ms {e} (eps {eps:.2})",
                                frame.t_ms, geo[i]
                            ));
                        }
                    }
                }
                _ => {}
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

/// Compare laid-out text geometry against the Compose trace's `text` entries.
/// `text:<n>` is the n-th text-bearing widget in scene order; it pairs with
/// the n-th `Text` element in the Slint component tree (both are document
/// order).
///
/// The two engines measure the same font differently by design: layoutlib
/// shapes with hinted integer advances while Slint's text layout uses
/// fractional advances, so Compose's `w` runs ~0.5px wider per glyph plus
/// letter-spacing quantization — a few pixels on a label, not a bug. The
/// Compose harness therefore also emits `frac_w`, the unhinted
/// `Paint.measureText` width of the same string: the font's own metrics.
/// Slint's `w` is validated against `frac_w` within [`GEOM_EPS`], and the
/// cross-engine positions are compared after centering away the width
/// difference (text nodes are centered in their containers). `h` still
/// compares at 1px (line-height rounding), `baseline`/`lines` have no Slint
/// introspection — the masked pixel layer covers their visual effect.
fn compare_text_metrics<C: i_slint_core::api::ComponentHandle>(
    component: &C,
    slint: &[TraceFrame],
    compose: &serde_json::Value,
) -> Vec<String> {
    let mut errors = Vec::new();
    let frames = compose["frames"].as_array().cloned().unwrap_or_default();
    let by_time: BTreeMap<u64, &serde_json::Value> =
        frames.iter().filter_map(|f| f["t_ms"].as_u64().map(|t| (t, f))).collect();
    let handles = i_slint_backend_testing::ElementQuery::from_root(component)
        .match_inherits("Text")
        .find_all();

    for frame in slint {
        // Same ±PHASE_MS matching as compare_traces: the Compose harness
        // labels the settled frame one tick after the requested time.
        let lo = frame.t_ms.saturating_sub(4);
        let Some(cf) = (lo..=frame.t_ms + 4)
            .filter_map(|t| by_time.get(&t))
            .next_back()
        else {
            continue;
        };
        let Some(texts) = cf["text"].as_object() else { continue };
        if texts.is_empty() {
            continue;
        }
        // `text:<n>` keys sorted numerically — document order, so they line
        // up with the `Text` handles' tree order.
        let mut entries: Vec<(u64, &serde_json::Value)> = texts
            .iter()
            .filter_map(|(k, v)| {
                k.strip_prefix("text:").and_then(|n| n.parse().ok()).map(|n| (n, v))
            })
            .collect();
        entries.sort_by_key(|(n, _)| *n);
        for (i, (n, m)) in entries.iter().enumerate() {
            let Some(handle) = handles.get(i) else {
                errors.push(format!(
                    "t={}ms text:{n}: no Slint Text element ({} found)",
                    frame.t_ms,
                    handles.len()
                ));
                continue;
            };
            let pos = handle.absolute_position();
            let size = handle.size();
            let (sx, sy, sw, sh) =
                (pos.x as f64, pos.y as f64, size.width as f64, size.height as f64);
            let Some(cw) = m["w"].as_f64() else { continue };
            let Some(cx) = m["x"].as_f64() else { continue };

            // Slint w vs the font's own metrics; Compose's hinted w stays a
            // few px wider by design and is not re-checked here.
            match m["frac_w"].as_f64() {
                Some(frac_w) if frac_w.is_finite() => {
                    // Slint advances glyph positions at integer px; frac_w
                    // is the font's exact advance — allow 1px rounding.
                    if (sw - frac_w).abs() > 1.0 {
                        errors.push(format!(
                            "t={}ms text:{n}.w: slint {sw} vs font metric {frac_w} (eps 1.0)",
                            frame.t_ms
                        ));
                    }
                }
                // Fallback for references recorded before frac_w existed:
                // bound the cross-engine drift to ~0.5px per glyph.
                _ => {
                    let chars = m["chars"].as_f64().unwrap_or(1.0).max(1.0);
                    let bound = GEOM_EPS + 0.6 * chars;
                    if (sw - cw).abs() > bound {
                        errors.push(format!(
                            "t={}ms text:{n}.w: slint {sw} vs compose {cw} (bound {bound:.1})",
                            frame.t_ms
                        ));
                    }
                }
            }

            // Center-x and y compare after absorbing the width difference —
            // the text is centered in equal-width containers, so a Slint text
            // whose w is narrower still sits at the same center.
            let c_center = cx + cw / 2.0;
            let drift = m["frac_w"]
                .as_f64()
                .filter(|f| f.is_finite())
                .map(|f| (cw - f).abs())
                .unwrap_or(1.0);
            let c_eps = drift / 2.0 + GEOM_EPS;
            if (sx + sw / 2.0 - c_center).abs() > c_eps {
                errors.push(format!(
                    "t={}ms text:{n}.cx: slint {:.2} vs compose {c_center:.2} (eps {c_eps:.2})",
                    frame.t_ms,
                    sx + sw / 2.0,
                ));
            }
            for (key, actual, eps) in [("y", sy, GEOM_EPS), ("h", sh, 1.0)] {
                let Some(e) = m[key].as_f64() else { continue };
                if (actual - e).abs() > eps {
                    errors.push(format!(
                        "t={}ms text:{n}.{key}: slint {actual} vs compose {e} (eps {eps})",
                        frame.t_ms
                    ));
                }
            }
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

/// The Compose trace frame nearest `t` — for masking, the frame at the
/// image's nominal capture time, not the phase-matched one compare_traces
/// uses. The Compose harness labels frames by its own tick, a couple of ms
/// off ours.
fn compose_frame_at(compose: &serde_json::Value, t: u64) -> Option<&serde_json::Value> {
    let frames = compose["frames"].as_array()?;
    frames
        .iter()
        .filter_map(|f| f["t_ms"].as_u64().map(|tt| (tt, f)))
        .filter(|(tt, _)| tt.abs_diff(t) <= 4)
        .min_by_key(|(tt, _)| tt.abs_diff(t))
        .map(|(_, f)| f)
}

/// Build the layered mask for one rendered frame: the Compose text mask,
/// unioned with the Slint text rects and the Compose text rects (each
/// dilated 1px — the engines place a label's ink a pixel apart), plus a
/// `Skip` disagreement band around every element both sides trace.
fn build_frame_mask<C: i_slint_core::api::ComponentHandle>(
    component: &C,
    slint_frame: &TraceFrame,
    compose_frame: Option<&serde_json::Value>,
    png_mask: Option<&SharedPixelBuffer<Rgba8Pixel>>,
    density: f64,
    width: u32,
    height: u32,
) -> PixelMask {
    let mut mask = png_mask
        .filter(|m| m.width() == width && m.height() == height)
        .map(PixelMask::from_text_png)
        .unwrap_or_else(|| PixelMask::new(width, height));
    let d = density;
    // Text regions from both sides — a label's ink lives inside its bounds.
    for handle in
        i_slint_backend_testing::ElementQuery::from_root(component).match_inherits("Text").find_all()
    {
        let p = handle.absolute_position();
        let s = handle.size();
        mask.fill_rect(
            PxRect {
                x0: p.x as f64 * d,
                y0: p.y as f64 * d,
                x1: (p.x + s.width) as f64 * d,
                y1: (p.y + s.height) as f64 * d,
            }
            .dilated(1.0),
            PixelClass::Text,
        );
    }
    let Some(cf) = compose_frame else { return mask };
    if let Some(texts) = cf["text"].as_object() {
        for (_, m) in texts {
            let (Some(x), Some(y), Some(w), Some(h)) =
                (m["x"].as_f64(), m["y"].as_f64(), m["w"].as_f64(), m["h"].as_f64())
            else {
                continue;
            };
            mask.fill_rect(
                PxRect { x0: x * d, y0: y * d, x1: (x + w) * d, y1: (y + h) * d }.dilated(1.0),
                PixelClass::Text,
            );
        }
    }
    if let Some(celements) = cf["elements"].as_object() {
        for (id, geo) in &slint_frame.elements {
            let Some(ce) = celements.get(id) else { continue };
            let (Some(x), Some(y), Some(w), Some(h)) =
                (ce["x"].as_f64(), ce["y"].as_f64(), ce["w"].as_f64(), ce["h"].as_f64())
            else {
                continue;
            };
            mask.mark_element(
                PxRect {
                    x0: geo[0] * d,
                    y0: geo[1] * d,
                    x1: (geo[0] + geo[2]) * d,
                    y1: (geo[1] + geo[3]) * d,
                },
                PxRect { x0: x * d, y0: y * d, x1: (x + w) * d, y1: (y + h) * d },
                DECORATION_MARGIN_DP * d,
            );
        }
    }
    mask
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

        // The Compose trace drives the per-frame masks as well as the
        // numeric comparisons — load it once per density.
        let dir = refs_dir(case_rel).join(format!("d{density}"));
        let compose: Option<serde_json::Value> = if references_missing {
            None
        } else {
            match std::fs::read_to_string(dir.join("trace.json"))
                .map_err(|e| format!("{e}"))
                .and_then(|json| {
                    serde_json::from_str(&json).map_err(|e| format!("{e}"))
                }) {
                Ok(v) => Some(v),
                Err(e) => {
                    failures.push(format!("d{density} trace.json: {e}"));
                    None
                }
            }
        };

        // The actions are input delivered at t=0: the frame at the first time
        // captures the pre-gesture state, so they dispatch right after it.
        // (Static cases only record the settled frame — apply first.)
        let actions_before_first_frame = kind != "motion";
        if actions_before_first_frame {
            apply_actions(component.window(), spec);
        }

        // Static cases settle out any entry/ripple animation before the shot.
        let times: Vec<u64> = if kind == "motion" {
            spec.times.clone()
        } else {
            vec![STATIC_SETTLE_MS]
        };
        let start = i_slint_backend_testing::get_mocked_time();

        let mut frames = Vec::new();
        let mut actions_applied = actions_before_first_frame;
        let mut artifacts_written = false;
        for &t in &times {
            advance_mock_time_to(start, t);
            let actual = render_frame(&component);
            frames.push(capture_trace(&component, t, spec, prop_value));
            if !actions_applied {
                apply_actions(component.window(), spec);
                actions_applied = true;
            }

            if references_missing {
                continue;
            }
            let tag = if kind == "motion" { format!("{t}ms") } else { "settled".to_string() };
            let frame_path = dir.join(format!("frame_{tag}.png"));
            let mask_path = dir.join(format!("mask_{tag}.png"));
            let expected = match load_png(&frame_path) {
                Ok(e) => e,
                Err(e) => {
                    failures.push(format!("d{density} {e}"));
                    continue;
                }
            };
            let png_mask = load_png(&mask_path).ok();
            let mask = build_frame_mask(
                &component,
                frames.last().unwrap(),
                compose.as_ref().and_then(|c| compose_frame_at(c, t)),
                png_mask.as_ref(),
                *density as f64,
                actual.width(),
                actual.height(),
            );
            let result = layered_compare(&actual, &expected, Some(&mask), pixel_eps);
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

        if let Some(compose) = &compose {
            let mut errors = if kind == "motion" {
                compare_traces(&frames, compose)
            } else {
                Vec::new()
            };
            errors.extend(compare_text_metrics(&component, &frames, compose));
            if !errors.is_empty() {
                let dir = artifacts_dir(driver, case_rel);
                std::fs::create_dir_all(&dir)?;
                std::fs::write(dir.join("slint_trace.json"), trace_to_json(&frames))?;
                let svg = trace_svg(&frames, Some(compose));
                if !svg.is_empty() {
                    std::fs::write(dir.join(format!("trace_d{density}.svg")), svg)?;
                }
                failures.append(&mut errors);
                artifacts_written = true;
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
    let mut all_text = PixelMask::new(size, size);
    all_text.fill_rect(
        PxRect { x0: 0.0, y0: 0.0, x1: size as f64, y1: size as f64 },
        PixelClass::Text,
    );
    assert!(layered_compare(&a, &b, Some(&all_text), PIXEL_EPS).ok, "identical images must pass");

    // 1px color nudge outside text → strict layer fails.
    b.make_mut_slice()[0] = Rgba8Pixel { r: 20, g: 0, b: 0, a: 255 };
    let no_text = PixelMask::new(size, size);
    assert!(!layered_compare(&a, &b, Some(&no_text), PIXEL_EPS).ok, "1px diff must fail");

    // Same nudge but fully inside the text mask → tolerated by the loose layer.
    assert!(
        layered_compare(&a, &b, Some(&all_text), PIXEL_EPS).ok,
        "small diff inside text mask must pass"
    );

    // Blanket wrong color across the mask → loose layer fails.
    let mut c = a.clone();
    for p in c.make_mut_slice() {
        *p = Rgba8Pixel { r: 255, g: 0, b: 0, a: 255 };
    }
    assert!(!layered_compare(&a, &c, Some(&all_text), PIXEL_EPS).ok, "wrong text color must fail");
}
