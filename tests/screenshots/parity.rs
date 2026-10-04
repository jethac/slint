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
//!    on both sides, the strict layer skips the bounds' symmetric difference
//!    and a perimeter band (the anti-aliased outline plus decorations) — its
//!    geometry is verified numerically instead (a text-sized element's `w`
//!    additionally tolerates the hinted-text drift), while its interior
//!    stays strict.
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

use i_slint_core::SharedString;
use i_slint_core::graphics::{Rgba8Pixel, SharedPixelBuffer};
use i_slint_core::platform::WindowEvent;
use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use test_driver_lib::{ParityAction, ParityMarkers};

/// `<container>item<i>` — the repeated-child convention `//TRACE_ITEMS=`
/// emits (`group0item0`) and the Compose emitter's `track` tags share.
fn parse_item_ref(id: &str) -> Option<(String, usize)> {
    let digits = id.len() - id.trim_end_matches(|c: char| c.is_ascii_digit()).len();
    if digits == 0 {
        return None;
    }
    let (stem, num) = id.split_at(id.len() - digits);
    let container = stem.strip_suffix("item")?;
    if container.is_empty() {
        return None;
    }
    Some((container.to_string(), num.parse().ok()?))
}

/// Intersect an `[x, y, w, h, opacity]` geometry with a clip rect in the
/// same format. Slint's trace reports an element's unclipped bounds
/// inside a `Flickable` while the Compose harness reports the clipped
/// rect — intersecting with the container's bounds puts both sides on
/// the visible portion, so a partially-scrolled item stays verifiable.
fn clip_to(geo: &[f64; 5], clip: &[f64; 5]) -> [f64; 5] {
    let x0 = geo[0].max(clip[0]);
    let y0 = geo[1].max(clip[1]);
    let x1 = (geo[0] + geo[2]).min(clip[0] + clip[2]);
    let y1 = (geo[1] + geo[3]).min(clip[1] + clip[3]);
    [x0, y0, (x1 - x0).max(0.0), (y1 - y0).max(0.0), geo[4]]
}

/// The `nth`-th *visible* `{container}item{j}` of a Compose frame. The
/// two sides number repeated items differently under a clip: Slint's
/// item tree culls elements fully outside a `Flickable`'s viewport
/// (`ItemRc::is_visible`), so the Slint trace's `item{i}` is the i-th
/// visible item, while the Compose harness reports every model item and
/// gives clipped-away ones an empty rect. Slint `item{i}` pairs with the
/// i-th Compose item whose bounds aren't empty — not with `item{i}`.
fn nth_visible_item(cf: &serde_json::Value, container: &str, nth: usize) -> Option<(String, usize)> {
    let elements = cf["elements"].as_object()?;
    let mut indices: Vec<usize> = elements
        .keys()
        .filter_map(|k| {
            let (c, j) = parse_item_ref(k)?;
            (c == container).then_some(j)
        })
        .collect();
    indices.sort_unstable();
    indices
        .into_iter()
        .filter(|j| {
            let ce = &elements[&format!("{container}item{j}")];
            ce["w"].as_f64().unwrap_or_default() > 0.0
                && ce["h"].as_f64().unwrap_or_default() > 0.0
        })
        .nth(nth)
        .map(|j| (format!("{container}item{j}"), j))
}

/// Strict layer: maximum absolute per-channel difference allowed on
/// non-text pixels. 8 is well below the channel difference any wrong
/// token (color role, corner radius, size) produces, and above the
/// one-or-two-level antialiasing noise between Slint's software renderer
/// and Compose's software rasterization.
pub const PIXEL_EPS: u8 = 8;

/// Masked (text) layer: the mean absolute channel difference inside a
/// `TEXT_CELL`-sized cell of text pixels must stay below this value, and no
/// more than `TEXT_OUTLIER_FRACTION` of a cell's text pixels may exceed
/// `TEXT_OUTLIER_EPS`. Calibrated to the engines' hinted-vs-fractional
/// advance drift (sub-pixel glyph shifts inside a cell), so different
/// rasterizers pass; missing, misplaced, or wrongly-colored text still
/// fails well above these bounds.
pub const TEXT_CELL: usize = 16;
pub const TEXT_CELL_EPS: f64 = 48.0;
pub const TEXT_OUTLIER_EPS: u8 = 96;
pub const TEXT_OUTLIER_FRACTION: f64 = 0.35;

/// Strict layer pixels on a detected image edge get `EDGE_EPS` — but only
/// when the edge exists in *both* renders within [`EDGE_BAND`]: antialiasing
/// drift on a shared outline is a fraction of its contrast and legitimately
/// differs between Slint's and layoutlib's renderers. An edge present in
/// only one render (a missing or added decoration) is compared at
/// [`PIXEL_EPS`] instead, so dropping the focus ring, an overlay, or a
/// recolor fails crisply.
pub const EDGE_EPS: u8 = 96;
pub const EDGE_GRADIENT_SOFT: u8 = 24;

/// Outline detection — includes faint decorations (focus rings, elevation
/// fringes) whose exact placement is engine detail, well above flat noise.
pub const EDGE_GRADIENT_OUTLINE: u8 = 10;

/// Pixels within this distance of an edge *in the same image* count as its
/// edge neighborhood; an edge that exists in both images within this range
/// is the same outline rendered by both engines.
pub const EDGE_BAND: usize = 2;

/// Within a traced element's bounds, pixels closer to the bounds' edge than
/// this are the outline zone skipped by [`PixelMask::mark_element`] — deep
/// enough to cover a rendered outline inset from its bounds (capsule arcs
/// sit ~6px in when the corner radius hits the element's edge) plus
/// antialiasing, never deep enough to cover an element's interior: a 40dp
/// button still has a 24dp-wide strict strip through its middle, so a
/// recolor or a missing fill fails.
pub const OUTLINE_INSET_DP: f64 = 8.0;

/// Decorations attached to a traced element — elevation shadow, focus
/// outline — may spill a few dp outside its bounds; the perimeter band
/// [`PixelMask::mark_element`] builds extends this far out to cover them.
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
pub fn library_paths_for(
    source: &str,
    case_dir: &Path,
) -> std::collections::HashMap<String, PathBuf> {
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
    repo_root().join("ui-libraries/material/parity/compose/references").join(case_rel)
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
    /// disagree — a traced element's bounds' symmetric difference and the
    /// perimeter band around each bounds (the anti-aliased outline and the
    /// decorations that hug it: focus ring, elevation shadow). The numeric
    /// trace layer still checks these elements' geometry; the interior of
    /// every element stays strict, so a recolor, a missing fill, or a
    /// one-sided decoration always fails.
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

    fn width(&self) -> f64 {
        self.x1 - self.x0
    }

    fn height(&self) -> f64 {
        self.y1 - self.y0
    }

    fn dilated(&self, px: f64) -> Self {
        Self { x0: self.x0 - px, y0: self.y0 - px, x1: self.x1 + px, y1: self.y1 + px }
    }
}

/// Half-width of the boundary band a corner zone keeps masked, in device px.
/// Anti-aliasing on the true corner shape lives inside this distance of the
/// boundary; a fill bleeding past it (the corner leak) lands on strict pixels.
const CORNER_BAND: f64 = 2.0;

/// Growth a `//MASK_INNER=` element's ink coverage must still have left one
/// frame after the gesture: a bounded ripple expands over hundreds of ms,
/// so coverage at the first post-action frame sits well below the settled
/// coverage. An instantly full-size ripple (f579301e2's `width =
/// root.width * 2 * 1.4142`) violates it at every density on every driver.
/// Renderer- and engine-neutral: each image is measured against its own
/// pre-action baseline and its own settled coverage.
const INK_GROWTH_MARGIN: f64 = 0.30;
/// Settled coverage below this means the ink never showed — layoutlib's
/// ripple can legitimately stay sub-threshold, so the growth bound only
/// applies once the settle frame proves ink reached the interior.
const INK_SETTLED_MIN: f64 = 0.30;

/// Corner-zone classification for [`PixelMask::mark_element`]: `Band` sits
/// within `CORNER_BAND + drift` of the silhouette, `Outside` lies beyond it
/// outside the maximal shape the bounds admit (only a spill or a wrong
/// radius paints there), `Inside` lies further in — the element interior.
enum CornerCell {
    Band,
    Outside,
    Inside,
}

/// Per-pixel comparison layers for [`layered_compare`].
struct PixelMask {
    layer: Vec<PixelClass>,
    w: usize,
    h: usize,
}

impl PixelMask {
    /// Every pixel strict.
    fn new(w: u32, h: u32) -> Self {
        Self { layer: vec![PixelClass::Strict; (w * h) as usize], w: w as usize, h: h as usize }
    }

    /// The Compose text mask PNG (`mask_t*.png`): white pixels are text.
    fn from_text_png(m: &SharedPixelBuffer<Rgba8Pixel>) -> Self {
        let layer = m
            .as_slice()
            .iter()
            .map(|p| if p.r > 0x7f && p.a > 0x7f { PixelClass::Text } else { PixelClass::Strict })
            .collect();
        Self { layer, w: m.width() as usize, h: m.height() as usize }
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

    /// Record the traced element pair `slint`/`compose` (device px). Two
    /// kinds of pixels skip strict comparison, and nothing else:
    ///
    /// * the symmetric difference of the two bounds — where one side's shape
    ///   covers a pixel the other's doesn't (traced geometry drift), and
    /// * a band around each bounds' perimeter, `margin` out and `inset`
    ///   in — where the rendered outline may legitimately sit (capsule arcs
    ///   inset a few px, focus rings and elevation shadows just outside,
    ///   text-driven width drift).
    ///
    /// The mask is derived from the traced geometry, never from detected
    /// image edges: a label's ink inside a button, a flat fill, or a
    /// recolored element can never expand it. An element's interior always
    /// stays strict — the trace layer verifies x/y/w/h numerically, so a
    /// pixel-level outline band is all the tolerance the outline gets.
    ///
    /// `margin` is the outer decoration allowance and `inset` the inner
    /// outline allowance, both in device px (see [`DECORATION_MARGIN_DP`]
    /// and [`OUTLINE_INSET_DP`]). Inside a corner zone — the square that can
    /// hold any corner radius the bounds admit — the band collapses to
    /// `CORNER_BAND` around the true boundary: the sharp edge lines or the
    /// maximal pill end-cap arc. Pixels further from that boundary stay
    /// strict, so a fill spilling past a rounded corner is caught instead of
    /// absorbed by the decoration margin. `corners_strict` disables that
    /// carve-out (`//MASK_DECOR=`): when the element carries a decoration
    /// the reference engine cannot render, corner zones take the normal
    /// margin band.
    fn mark_element(
        &mut self,
        slint: PxRect,
        compose: PxRect,
        margin: f64,
        inset: f64,
        corners_strict: bool,
        inked: bool,
    ) {
        let (a, b) = (slint.dilated(1.0), compose.dilated(1.0));
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
        // `Some(Band)` when (px,py) sits within `CORNER_BAND` of a corner
        // boundary of `r`, `Some(Outside)` when it lies in a corner zone
        // outside the maximal silhouette the bounds admit — the region only
        // a spill or a wrong radius can paint — `Some(Inside)` when it lies
        // inside that silhouette, `None` when outside every corner zone (the
        // normal margin/inset band applies). The band grows by the bounds'
        // displacement: an edge or corner arc that moved by the drift the
        // traced bounds report is still the same boundary, while a wrong
        // radius or spill shows up strictly off it.
        let drift = (slint.x0 - compose.x0)
            .abs()
            .max((slint.x1 - compose.x1).abs())
            .max((slint.y0 - compose.y0).abs())
            .max((slint.y1 - compose.y1).abs());
        let band = CORNER_BAND + drift;
        let corner_cell = |r: PxRect, px: f64, py: f64| -> Option<CornerCell> {
            let r_pill = r.width().min(r.height()) / 2.0;
            let zone = r_pill + margin;
            let corners = [
                (r.x0, r.y0, r.x0 + r_pill, r.y0 + r_pill),
                (r.x1, r.y0, r.x1 - r_pill, r.y0 + r_pill),
                (r.x0, r.y1, r.x0 + r_pill, r.y1 - r_pill),
                (r.x1, r.y1, r.x1 - r_pill, r.y1 - r_pill),
            ];
            corners
                .iter()
                .find(|(cx, cy, ..)| (px - cx).abs() <= zone && (py - cy).abs() <= zone)
                .map(|(.., ax, ay)| {
                    let near_edge = (px - r.x0).abs() <= band
                        || (px - r.x1).abs() <= band
                        || (py - r.y0).abs() <= band
                        || (py - r.y1).abs() <= band;
                    if near_edge {
                        return CornerCell::Band;
                    }
                    let (u, v) = ((px - ax).abs(), (py - ay).abs());
                    if u <= r_pill && v <= r_pill {
                        let arc_d = (u * u + v * v).sqrt() - r_pill;
                        if arc_d.abs() <= band {
                            CornerCell::Band
                        } else if arc_d > 0.0 {
                            CornerCell::Outside
                        } else {
                            CornerCell::Inside
                        }
                    } else if r.contains(px, py) {
                        CornerCell::Inside
                    } else {
                        CornerCell::Outside
                    }
                })
        };
        for r in [a, b] {
            let outer = r.dilated(margin);
            let inner =
                PxRect { x0: r.x0 + inset, y0: r.y0 + inset, x1: r.x1 - inset, y1: r.y1 - inset };
            // `//MASK_DECOR=` widens the masked zone inside the outline too:
            // inset decorations (a focus ring's inner strokes, which reach
            // only `inner_stroke_inset + inner_stroke_width` under the edge)
            // share the bounds' drift exactly like the outline itself does,
            // so the decor band applies both ways, not just outward.
            let inner_decor = PxRect {
                x0: r.x0 + margin,
                y0: r.y0 + margin,
                x1: r.x1 - margin,
                y1: r.y1 - margin,
            };
            for y in outer.y0.floor().max(0.0) as usize..(outer.y1.ceil() as usize).min(self.h) {
                for x in outer.x0.floor().max(0.0) as usize..(outer.x1.ceil() as usize).min(self.w)
                {
                    let (px, py) = (x as f64 + 0.5, y as f64 + 0.5);
                    match corner_cell(r, px, py) {
                        Some(CornerCell::Band) => self.set(x, y, PixelClass::Skip),
                        // Outside the maximal silhouette but inside the
                        // bounds: nothing that belongs to the shape paints
                        // here, so the cell is strict — a fill spill past
                        // the corner arc lands here. Restore it where an
                        // earlier pass (`//MASK_INNER=`, the bounds xor, a
                        // neighboring element's band) set Skip, so a spill
                        // can't hide under a mask. Cells inside the
                        // silhouette keep their class — under an ink mask
                        // they may legitimately carry engine-divergent ink.
                        // The same goes for the outside-silhouette cells of
                        // an `inked` element: ripple ink is bounded by the
                        // rect, not the pill, so it legitimately fills these
                        // corners mid-press — `corner_silhouette_findings`
                        // verifies that silhouette instead. For a decor
                        // element the wedge itself is decor (shadow spill,
                        // a ring), so it is masked instead.
                        Some(CornerCell::Outside) => {
                            if !corners_strict {
                                self.set(x, y, PixelClass::Skip)
                            } else if !inked
                                && r.contains(px, py)
                                && self.layer[y * self.w + x] == PixelClass::Skip
                            {
                                self.set(x, y, PixelClass::Strict)
                            }
                        }
                        // For a decor element, decorations inset from the
                        // outline (a focus ring's strokes) share the bounds'
                        // drift exactly like the outline itself does, so the
                        // widened inner band applies inside the corner zones
                        // too — deeper cells keep their class.
                        Some(CornerCell::Inside)
                            if !corners_strict && !inner_decor.contains(px, py) =>
                        {
                            self.set(x, y, PixelClass::Skip)
                        }
                        Some(_) => {}
                        None if !inner.contains(px, py) => self.set(x, y, PixelClass::Skip),
                        _ => {}
                    }
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
struct LayerResult {
    /// True when every layer passed.
    pub ok: bool,
    /// Human-readable verdict, included in test failure output.
    pub report: String,
    /// Diff visualization: red = failing pixels, blue = masked text pixels,
    /// else the dimmed actual render.
    pub diff: Option<SharedPixelBuffer<Rgba8Pixel>>,
    /// Number of strict-layer pixels that differed — what a negative case
    /// must produce (any other failure could be a missing file or a trace
    /// mismatch, not the render rejecting the defect).
    pub strict_failures: usize,
    /// Strict failures inside `count_in` — a negative case asserts its catch
    /// within the region the mutation affected, not anywhere in the frame.
    pub strict_failures_in_region: usize,
    /// Text cells whose mean diff needed the relaxed `text_cell_eps` bound —
    /// nonzero means an `xfail_text` relaxation was actually used.
    pub text_cells_relaxed: usize,
}

/// Compare `actual` against `expected` under the per-pixel layer mask:
/// strict per-channel outside text and skip regions, per-cell mean inside
/// the text mask, nothing inside skip bands. `count_in` additionally counts
/// strict failures whose pixel lies inside that region.
fn layered_compare(
    actual: &SharedPixelBuffer<Rgba8Pixel>,
    expected: &SharedPixelBuffer<Rgba8Pixel>,
    mask: Option<&PixelMask>,
    pixel_eps: u8,
    count_in: Option<PxRect>,
    text_cell_eps: f64,
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
            strict_failures: 0,
            strict_failures_in_region: 0,
            text_cells_relaxed: 0,
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
            strict_failures: 0,
            strict_failures_in_region: 0,
            text_cells_relaxed: 0,
        };
    }
    let w = actual.width() as usize;
    let h = actual.height() as usize;
    let (a, e) = (actual.as_slice(), expected.as_slice());

    // Edge strength from both images: pixels within EDGE_BAND of an outline
    // present in BOTH renders get an epsilon proportional to the local
    // contrast — antialiasing drift is a fraction of the edge's own
    // contrast, while a real coverage difference (an outline in only one
    // render, a flat fill mismatch) produces diffs at full contrast and
    // still fails.
    let gradient_actual = local_gradient(actual);
    let gradient_expected = local_gradient(expected);
    let gradient: Vec<u8> =
        gradient_actual.iter().zip(&gradient_expected).map(|(a, b)| (*a).max(*b)).collect();
    let strength = dilate_max(&gradient, w, h, EDGE_BAND);
    let edges_actual: Vec<bool> =
        gradient_actual.iter().map(|&g| g > EDGE_GRADIENT_OUTLINE).collect();
    let edges_expected: Vec<bool> =
        gradient_expected.iter().map(|&g| g > EDGE_GRADIENT_OUTLINE).collect();
    let near_actual = dilate(&edges_actual, w, h, EDGE_BAND);
    let near_expected = dilate(&edges_expected, w, h, EDGE_BAND);
    let both_edge: Vec<bool> =
        near_actual.iter().zip(&near_expected).map(|(a, e)| *a && *e).collect();

    let mut diff_img = SharedPixelBuffer::<Rgba8Pixel>::new(actual.width(), actual.height());
    let mut strict_failures = 0usize;
    let mut strict_failures_in_region = 0usize;
    let mut strict_bbox = None;
    let mut worst = 0u8;

    // Accumulate per text-mask cell: (sum of mean channel diff, outlier count, text pixel count)
    let mut cells: BTreeMap<usize, (f64, usize, usize)> = BTreeMap::new();

    for i in 0..a.len() {
        let d = channel_diff(&a[i], &e[i]);
        let (x, y) = (i % w, i / w);
        let class = mask.layer[i];
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
                // AA drift on a shared outline is bounded by the outline's
                // own contrast: a difference larger than half the local
                // contrast means real coverage disagreement. An edge in
                // only one render gets no scaling — the missing or added
                // feature is what the strict layer exists to catch.
                let eps = if strength[i] > EDGE_GRADIENT_SOFT && both_edge[i] {
                    ((strength[i] as u16) / 2).clamp(pixel_eps as u16, EDGE_EPS as u16) as u8
                } else {
                    pixel_eps
                };
                if d > eps {
                    strict_failures += 1;
                    if count_in.is_some_and(|r| r.contains(x as f64 + 0.5, y as f64 + 0.5)) {
                        strict_failures_in_region += 1;
                    }
                    strict_bbox = match strict_bbox {
                        None => Some((x, y, x + 1, y + 1)),
                        Some((x0, y0, x1, y1)) => {
                            Some((x0.min(x), y0.min(y), x1.max(x + 1), y1.max(y + 1)))
                        }
                    };
                    worst = worst.max(d);
                    Rgba8Pixel { r: 0xff, g: 0, b: 0, a: 255 }
                } else {
                    dim(&a[i])
                }
            }
        };
    }

    let mut text_failures = Vec::new();
    let mut text_cells_relaxed = 0usize;
    for (cell, (sum, outliers, count)) in &cells {
        let mean = sum / *count as f64;
        if mean > TEXT_CELL_EPS {
            text_cells_relaxed += 1;
        }
        // A `//XFAIL_TEXT=` case relaxes the outlier fraction by the same
        // factor as the mean: advance drift translates the ink inside the
        // cell, so nearly every inked pixel can be an outlier by the end of
        // a longer run — still bounded, so a cell of wholly-missing ink
        // keeps failing.
        let outlier_fraction = TEXT_OUTLIER_FRACTION * (text_cell_eps / TEXT_CELL_EPS);
        if mean > text_cell_eps || *outliers as f64 > *count as f64 * outlier_fraction {
            text_failures.push(format!(
                "text cell {cell}: mean diff {mean:.1} (max {text_cell_eps:.0}), {outliers}/{count} outliers"
            ));
        }
    }

    let mut report = format!(
        "{strict_failures} strict pixels differ (worst channel diff {worst}, eps {pixel_eps}); {} text cells checked",
        cells.len()
    );
    if let Some((bx0, by0, bx1, by1)) = strict_bbox {
        report.push_str(&format!("; strict region ({bx0},{by0})-({bx1},{by1})"));
    }
    for f in &text_failures {
        report.push_str(&format!("; {f}"));
    }

    LayerResult {
        ok: strict_failures == 0 && text_failures.is_empty(),
        report,
        diff: if strict_failures == 0 && text_failures.is_empty() { None } else { Some(diff_img) },
        strict_failures,
        strict_failures_in_region,
        text_cells_relaxed,
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
    let mut frame = TraceFrame { t_ms, ..Default::default() };
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
    for (container, locals) in &spec.trace_items {
        let container_owned = container.clone();
        let container_handle = i_slint_backend_testing::ElementQuery::from_root(component)
            .match_predicate(move |e| {
                e.id()
                    .map(|candidate| {
                        let candidate = candidate.as_str();
                        candidate == container_owned
                            || candidate.ends_with(&format!("::{container_owned}"))
                    })
                    .unwrap_or(false)
            })
            .find_first();
        let Some(container_handle) = container_handle else {
            panic!("TRACE_ITEMS names container '{container}' but no element has that id")
        };
        let suffixes: Vec<String> = locals.iter().map(|l| format!("::{l}")).collect();
        let handles = container_handle
            .query_descendants()
            .match_predicate(move |e| {
                e.id()
                    .map(|id| suffixes.iter().any(|s| id.as_str().ends_with(s.as_str())))
                    .unwrap_or(false)
            })
            .find_all();
        let local = &locals[0];
        if handles.is_empty() {
            let mut seen = Vec::new();
            let _: Option<()> = container_handle.visit_descendants(|e| {
                seen.push(format!("{:?}", e.id()));
                std::ops::ControlFlow::Continue(())
            });
            panic!(
                "TRACE_ITEMS names '{container}:{local}' but no such child exists; descendants: {seen:?}"
            )
        }
        for (i, handle) in handles.iter().enumerate() {
            let pos = handle.absolute_position();
            let size = handle.size();
            frame.elements.insert(
                format!("{container}{local}{i}"),
                [
                    pos.x as f64,
                    pos.y as f64,
                    size.width as f64,
                    size.height as f64,
                    handle.computed_opacity() as f64,
                ],
            );
        }
    }
    frame
}

/// Dispatch the case's `//ACTION=` input steps to the window, in order.
pub fn apply_actions(window: &i_slint_core::api::Window, spec: &ParityMarkers) {
    apply_actions_filtered(window, spec, |_| true)
}

fn apply_actions_filtered(
    window: &i_slint_core::api::Window,
    spec: &ParityMarkers,
    mut due: impl FnMut(usize) -> bool,
) {
    for (i, action) in spec.actions.iter().enumerate() {
        if !due(i) {
            continue;
        }
        dispatch_action(window, action);
    }
}

fn dispatch_action(window: &i_slint_core::api::Window, action: &ParityAction) {
    use i_slint_core::api::LogicalPosition;
    use i_slint_core::items::PointerEventButton;
    if let ParityAction::Key { name, .. } = action {
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
        return;
    }
    let (x, y) = match *action {
        ParityAction::Move { x, y, .. }
        | ParityAction::Press { x, y, .. }
        | ParityAction::Release { x, y, .. } => (x, y),
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
            if let Some(end) = last.elements.get(id)
                && geo.iter().zip(end).any(|(a, b)| (a - b).abs() > SETTLE_EPS)
            {
                settle = frame.t_ms;
                moved = true;
            }
        }
    }
    moved.then_some(settle)
}

/// Compare a captured Slint trace to the Compose `trace.json`:
/// `{ "times_ms": [...], "frames": [{ "t_ms": n, "props": {...},
/// "elements": { "<id>": {"x":..,"y":..,"w":..,"h":..,"opacity":..} } }],
/// "settle_ms": n }`. Returns the list of mismatches found.
/// The Compose-side clock offset relative to the Slint timeline, in ms.
///
/// Comparison happens at identical timestamps — the Compose frame clock is
/// known to start one frame late (#27), and that lead is a real divergence,
/// not something a fitted shift may hide. The offset is still measured and
/// reported (for the `xfail` note that tracks #27), but it is not applied.
fn estimate_phase(slint: &[TraceFrame], by_time: &BTreeMap<u64, &serde_json::Value>) -> i64 {
    let frame_err = |sf: &TraceFrame, ct: i64| -> f64 {
        let Some(cf) = (ct >= 0).then(|| by_time.get(&(ct as u64))).flatten() else {
            return f64::MAX;
        };
        let mut err = 0.0f64;
        for (name, value) in &sf.props {
            let Some(actual) = value.components() else { continue };
            let Some(cv) = cf["props"].get(name) else { continue };
            let expected: Vec<f64> = match cv {
                serde_json::Value::Number(n) => vec![n.as_f64().unwrap_or_default()],
                serde_json::Value::Bool(b) => vec![*b as u8 as f64],
                serde_json::Value::Array(a) => {
                    a.iter().map(|v| v.as_f64().unwrap_or_default()).collect()
                }
                _ => continue,
            };
            if expected.len() == actual.len() {
                err += actual.iter().zip(&expected).map(|(a, e)| (a - e).abs()).fold(0.0, f64::max);
            }
        }
        // Geometry fallback when the scene traces no props: use element x.
        if sf.props.is_empty() {
            for (id, geo) in &sf.elements {
                if let Some(e) = cf["elements"].get(id).and_then(|ce| ce["x"].as_f64()) {
                    err += (geo[0] - e).abs();
                }
            }
        }
        err
    };

    (-16i64..=16)
        .map(|shift| {
            let mut total = 0.0f64;
            let mut compared = 0usize;
            let mut missed = 0usize;
            for sf in slint {
                let e = frame_err(sf, sf.t_ms as i64 + shift);
                if e == f64::MAX {
                    missed += 1;
                } else {
                    total += e;
                    compared += 1;
                }
            }
            (shift, if compared > 0 { total / compared as f64 } else { f64::MAX }, missed)
        })
        .min_by(|a, b| {
            // Lowest mean error over frames present on both clocks, then
            // the fewest boundary misses, then the smallest |shift|.
            a.1.total_cmp(&b.1).then(a.2.cmp(&b.2)).then(a.0.abs().cmp(&b.0.abs()))
        })
        .map(|(s, _, _)| s)
        .unwrap_or(0)
}

/// `(findings, measured compose clock offset)` — the offset is reported for
/// the record but never applied to the comparison (#27).
pub fn compare_traces(
    slint: &[TraceFrame],
    compose: &serde_json::Value,
) -> (Vec<String>, Option<i64>) {
    let mut errors = Vec::new();
    let frames = compose["frames"].as_array().cloned().unwrap_or_default();
    let by_time: BTreeMap<u64, &serde_json::Value> =
        frames.iter().filter_map(|f| f["t_ms"].as_u64().map(|t| (t, f))).collect();

    // Identical timestamps: the Compose trace records a frame every
    // millisecond, so a missing frame is itself a finding, not something to
    // approximate by a neighbor. The measured clock offset is reported below
    // for the xfail note; it is never applied to the comparison.
    let compose_at =
        |t: u64| -> Vec<&serde_json::Value> { by_time.get(&t).copied().into_iter().collect() };

    // The hinted drift `|w - unhinted advance|` of `text:<N>` is a constant
    // of the string, face and density — `onTextLayout` hasn't run yet on the
    // earliest frames, so a same-frame lookup would degrade the bound there.
    // Collect each text's drift from every frame that has metrics.
    let mut text_drift: BTreeMap<String, f64> = BTreeMap::new();
    for cf in &frames {
        if let Some(texts) = cf["text"].as_object() {
            for (tid, m) in texts {
                let Some(unhinted) = m["unhint_w"].as_f64().or_else(|| m["frac_w"].as_f64()) else {
                    continue;
                };
                let d = (m["w"].as_f64().unwrap_or(unhinted) - unhinted).abs();
                let e = text_drift.entry(tid.clone()).or_insert(d);
                *e = e.max(d);
            }
        }
    }

    for frame in slint {
        let candidates = compose_at(frame.t_ms);
        if candidates.is_empty() {
            errors.push(format!("t={}ms has no Compose trace frame", frame.t_ms));
            continue;
        }
        for (name, value) in &frame.props {
            let Some(actual) = value.components() else { continue };
            let mut best: Option<(Vec<f64>, f64)> = None;
            for cf in &candidates {
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
                let err: f64 =
                    actual.iter().zip(&expected).map(|(a, e)| (a - e).abs()).fold(0.0, f64::max);
                if best.as_ref().is_none_or(|(_, e)| err < *e) {
                    best = Some((expected, err));
                }
            }
            match best {
                None => errors
                    .push(format!("t={}ms prop '{name}' missing from Compose trace", frame.t_ms)),
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
        // The text a traced item's bounds contain: its label's hinted drift
        // bounds how far the item's own `w` may diverge, and a row of items
        // shifts each following item's `x` by the summed label drift of the
        // items before it.
        let drift_inside = |cf: &serde_json::Value, rect: &serde_json::Value| -> f64 {
            let (Some(rx), Some(ry), Some(rw), Some(rh)) =
                (rect["x"].as_f64(), rect["y"].as_f64(), rect["w"].as_f64(), rect["h"].as_f64())
            else {
                return 0.0;
            };
            let Some(texts) = cf["text"].as_object() else { return 0.0 };
            for (tid, m) in texts {
                let (Some(x), Some(y), Some(w), Some(h)) =
                    (m["x"].as_f64(), m["y"].as_f64(), m["w"].as_f64(), m["h"].as_f64())
                else {
                    continue;
                };
                let (cx, cy) = (x + w / 2.0, y + h / 2.0);
                if cx >= rx && cx <= rx + rw && cy >= ry && cy <= ry + rh {
                    return text_drift.get(tid).copied().unwrap_or(0.0);
                }
            }
            0.0
        };
        // `<container>item<i>`: summed hinted drift of the labels inside the
        // same container's items `0..i` — the slack `x` of item `i` inherits.
        let item_x_eps = |cf: &serde_json::Value, container: &str, idx: usize| -> f64 {
            let Some(elements) = cf["elements"].as_object() else { return GEOM_EPS };
            let mut eps = GEOM_EPS;
            for k in 0..idx {
                let key = format!("{container}item{k}");
                if let Some(rect) = elements.get(&key) {
                    eps += drift_inside(cf, rect) + 1.15;
                }
            }
            eps
        };
        for (id, geo) in &frame.elements {
            let item_ref = parse_item_ref(id);
            // An element named `button<N>` (or any `*<N>`) sizes itself to
            // `text:<N>`: its w legitimately differs by that text's hinting
            // drift, bounded here instead of pixel-strict. A `<c>item<i>`
            // element instead sizes to the text inside its own bounds, and a
            // container `<c>` widens its w bound by every item's drift.
            let drift = id
                .chars()
                .rev()
                .take_while(char::is_ascii_digit)
                .collect::<String>()
                .chars()
                .rev()
                .collect::<String>();
            let drift_eps = |cf: &serde_json::Value, ce_key: &str| -> f64 {
                if item_ref.is_some() {
                    return cf["elements"]
                        .get(ce_key)
                        .map(|rect| drift_inside(cf, rect))
                        .unwrap_or(0.0)
                        + GEOM_EPS
                        + 1.0;
                }
                if let Some(elements) = cf["elements"].as_object()
                    && elements.keys().any(|k| k.starts_with(&format!("{id}item")))
                {
                    return elements
                        .keys()
                        .filter(|k| k.starts_with(&format!("{id}item")))
                        .fold(GEOM_EPS, |eps, k| eps + drift_inside(cf, &elements[k]) + 1.15);
                }
                text_drift.get(&format!("text:{drift}")).copied().unwrap_or(0.0) + GEOM_EPS + 1.0
            };
            let mut best: Option<(&serde_json::Value, String, [f64; 5], f64, u64)> = None;
            for cf in &candidates {
                // A `<c>item<i>` pairs by visible order — Slint's id is the
                // i-th item the viewport leaves unclipped — and compares
                // after clipping to the container's bounds, matching the
                // clipped rects the Compose trace reports.
                let (ce_key, geo_cmp) = if let Some((container, idx)) = &item_ref {
                    let Some(clip) = frame.elements.get(container) else { continue };
                    match nth_visible_item(cf, container, *idx) {
                        Some((key, _)) => (key, clip_to(geo, clip)),
                        None => continue,
                    }
                } else {
                    (id.clone(), *geo)
                };
                let Some(ce) = cf["elements"].get(&ce_key) else { continue };
                let w_eps = if drift.is_empty() { GEOM_EPS } else { drift_eps(cf, &ce_key) };
                let x_eps = item_ref
                    .as_ref()
                    .and_then(|(container, _)| parse_item_ref(&ce_key).map(|(_, j)| (container, j)))
                    .map(|(container, idx)| item_x_eps(cf, container, idx))
                    .unwrap_or(GEOM_EPS);
                let err: f64 = ["x", "y", "w", "h", "opacity"]
                    .iter()
                    .enumerate()
                    .filter_map(|(i, key)| {
                        let eps = if *key == "w" {
                            w_eps
                        } else if *key == "x" {
                            x_eps
                        } else {
                            GEOM_EPS
                        };
                        ce[key].as_f64().map(|e| (geo_cmp[i] - e).abs() / eps.max(GEOM_EPS))
                    })
                    .fold(0.0, f64::max);
                if best.as_ref().is_none_or(|(_, _, _, e, _)| err < *e) {
                    best = Some((cf, ce_key, geo_cmp, err, cf["t_ms"].as_u64().unwrap_or(0)));
                }
            }
            match best {
                None => errors
                    .push(format!("t={}ms element '{id}' missing from Compose trace", frame.t_ms)),
                Some((cf, ce_key, geo_cmp, err, ct)) if err > 1.0 => {
                    let ce = &cf["elements"][&ce_key];
                    let w_eps = if drift.is_empty() { GEOM_EPS } else { drift_eps(cf, &ce_key) };
                    let x_eps = item_ref
                        .as_ref()
                        .and_then(|(container, _)| {
                            parse_item_ref(&ce_key).map(|(_, j)| (container, j))
                        })
                        .map(|(container, idx)| item_x_eps(cf, container, idx))
                        .unwrap_or(GEOM_EPS);
                    for (i, key) in ["x", "y", "w", "h", "opacity"].iter().enumerate() {
                        let Some(e) = ce[key].as_f64() else { continue };
                        let eps = if *key == "w" {
                            w_eps
                        } else if *key == "x" {
                            x_eps
                        } else {
                            GEOM_EPS
                        };
                        if (geo_cmp[i] - e).abs() > eps {
                            errors.push(format!(
                                "t={}ms element '{id}'.{key}: slint {} vs compose@{ct}ms {e} (eps {eps:.2})",
                                frame.t_ms, geo_cmp[i]
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

    // Report the measured clock offset for the record (the comparison itself
    // stays at identical timestamps — see #27).
    let phase = (!slint.is_empty() && !by_time.is_empty()).then(|| estimate_phase(slint, &by_time));
    if let Some(phase) = phase {
        eprintln!("parity: measured compose clock offset {phase:+}ms (Slint leads while >0)");
    }

    (errors, phase)
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
/// Returns the findings plus whether any compared width/offset showed the
/// ceil-quantization drift `//XFAIL_TEXT=` covers, so the caller can decide
/// whether the case's marker is still needed.
fn compare_text_metrics<C: i_slint_core::api::ComponentHandle>(
    component: &C,
    slint: &[TraceFrame],
    compose: &serde_json::Value,
    xfail_text: Option<&str>,
) -> (Vec<String>, bool) {
    let mut errors = Vec::new();
    let mut saw_drift = false;
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
        let Some(cf) = (lo..=frame.t_ms + 4).filter_map(|t| by_time.get(&t)).next_back() else {
            continue;
        };
        let Some(texts) = cf["text"].as_object() else { continue };
        if texts.is_empty() {
            continue;
        }
        // `text:<n>` keys sorted numerically — document order, so they line
        // up with the `Text` handles' tree order. Entries the viewport
        // clipped to an empty rect are dropped first: Slint's item tree
        // culls invisible `Text` elements (see `nth_visible_item`), so only
        // the visible subsequence pairs with the found handles.
        let mut entries: Vec<(u64, &serde_json::Value)> = texts
            .iter()
            .filter_map(|(k, v)| {
                let n: u64 = k.strip_prefix("text:")?.parse().ok()?;
                (v["w"].as_f64().unwrap_or_default() > 0.0
                    && v["h"].as_f64().unwrap_or_default() > 0.0)
                    .then_some((n, v))
            })
            .collect();
        entries.sort_by_key(|(n, _)| *n);
        if entries.len() != handles.len() {
            errors.push(format!(
                "t={}ms: {} visible compose texts vs {} Slint Text elements",
                frame.t_ms,
                entries.len(),
                handles.len()
            ));
        }
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
            // Clip the Slint bounds to the row the text scrolls inside:
            // the `*item*` element containing the text's center names its
            // container, and a `Flickable` clips its content at its own
            // bounds — the same rect the Compose harness clips text to.
            // The item's own key is kept as well: on the Slint side `item{i}`
            // counts visible items only, so the Compose-side container name
            // is not the Slint element to compare against.
            let slint_item = frame
                .elements
                .iter()
                .find_map(|(eid, g)| {
                    let (container, _) = parse_item_ref(eid)?;
                    let (tx, ty) = (
                        pos.x as f64 + size.width as f64 / 2.0,
                        pos.y as f64 + size.height as f64 / 2.0,
                    );
                    let inside = tx >= g[0] && tx <= g[0] + g[2] && ty >= g[1] && ty <= g[1] + g[3];
                    inside
                        .then(|| frame.elements.get(&container).copied().map(|row| (eid, row)))
                        .flatten()
                });
            let [sx, sy, sw, sh, _] = slint_item
                .map(|(_, row)| clip_to(&[pos.x as f64, pos.y as f64, size.width as f64, size.height as f64, 0.0], &row))
                .unwrap_or([pos.x as f64, pos.y as f64, size.width as f64, size.height as f64, 0.0]);
            let Some(cw) = m["w"].as_f64() else { continue };
            let Some(cx) = m["x"].as_f64() else { continue };

            // Slint's Text element width is `ceil(unhinted advance)` (the
            // layout ceils min/preferred — see issue #28) while Compose
            // keeps fractional advances: `sw − unhint_w` legitimately lands
            // in `(0, 1]`, so text is not within the 0.5px bound by design.
            // The strict bound is `(−0.15, 0.5]`; a case marked
            // `//XFAIL_TEXT=` accepts the whole ceil window `(−0.15, 1.15]`
            // as the tracked divergence, reports the measured drift — and
            // re-arms when #28 lands: if no entry drifts past 0.5px the
            // marker is stale and the case fails so it gets unmarked. A
            // drift past a whole pixel fails either way.
            // `unhint_w` is measured at 8x and scaled down, which leaves
            // ~0.125dp of residual quantization on both sides. Compose's
            // hinted `w`/`frac_w` stay a few px wider by design and are not
            // re-checked here.
            match m["unhint_w"].as_f64() {
                Some(unhint_w) if unhint_w.is_finite() => {
                    let slack = sw - unhint_w;
                    let bound = if xfail_text.is_some() { 1.15 } else { 0.5 };
                    if !(-0.15..=bound).contains(&slack) {
                        errors.push(format!(
                            "t={}ms text:{n}.w: slint {sw} vs unhinted compose {unhint_w} (bound {bound:.2})",
                            frame.t_ms
                        ));
                    } else if slack > 0.5 {
                        saw_drift = true;
                        if let Some(reason) = xfail_text {
                            eprintln!(
                                "parity: xfail-text t={}ms text:{n}.w: slint {sw} vs unhinted compose {unhint_w} — expected ceil-quantization drift {slack:+.2}px ({reason})",
                                frame.t_ms
                            );
                        }
                    }
                }
                // Fallback for references recorded before unhint_w existed:
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

            // Label placement inside its container: compare the text's x
            // offset within the element that holds it on both sides — the
            // container widths legitimately differ by the summed hinting
            // drift, so absolute centers would carry half of it. The
            // container is the `*item*` element whose bounds contain the
            // text (a `group{n}item{i}` inside a connected button group),
            // else `button<N>` (or `*<N>`), else absolute center.
            let container = cf["elements"]
                .as_object()
                .and_then(|elements| {
                    elements.iter().find_map(|(eid, ce)| {
                        parse_item_ref(eid)?;
                        let (Some(ex), Some(ey), Some(ew), Some(eh)) = (
                            ce["x"].as_f64(),
                            ce["y"].as_f64(),
                            ce["w"].as_f64(),
                            ce["h"].as_f64(),
                        ) else {
                            return None;
                        };
                        let (mx, my) = (cx + cw / 2.0, m["y"].as_f64()? + m["h"].as_f64()? / 2.0);
                        (mx >= ex && mx <= ex + ew && my >= ey && my <= ey + eh)
                            .then(|| eid.clone())
                    })
                })
                .unwrap_or_else(|| format!("button{n}"));
            // The Slint container's own clip applies to its x too: an
            // `item{i}` scrolled partly out of its row reports unclipped
            // bounds while the Compose side gives the clipped left edge.
            // Inside an item container the Slint counterpart is the item
            // whose bounds held the text — `item{i}` numbers differ under
            // a clip (see `nth_visible_item`).
            let slint_off = slint_item
                .and_then(|(eid, _)| frame.elements.get_key_value(eid))
                .or_else(|| frame.elements.get_key_value(&container))
                .map(|(eid, g)| {
                    let clip_x = parse_item_ref(eid)
                        .and_then(|(row, _)| frame.elements.get(&row))
                        .map(|r| r[0])
                        .unwrap_or(f64::NEG_INFINITY);
                    sx - g[0].max(clip_x)
                });
            let compose_off =
                cf["elements"].get(&container).and_then(|ce| ce["x"].as_f64()).map(|bx| cx - bx);
            match (slint_off, compose_off) {
                (Some(s_off), Some(c_off)) => {
                    // A centered label's offset within its container carries
                    // half the width slack: `(W − w)/2` shifts by `−Δw/2`.
                    // `//XFAIL_TEXT=` admits the same ceil-quantization
                    // window here: Compose's reported text width need not
                    // equal the box it centers in, so placement inherits
                    // the tracked #28 divergence too.
                    let off_eps = (GEOM_EPS + if xfail_text.is_some() { 1.15 } else { 0.0 })
                        + (cw - sw).abs() / 2.0;
                    if (s_off - c_off).abs() > off_eps {
                        errors.push(format!(
                            "t={}ms text:{n}.x-offset: slint {s_off:.2} vs compose {c_off:.2} (eps {off_eps:.2})",
                            frame.t_ms
                        ));
                    } else if xfail_text.is_some() && (s_off - c_off).abs() > GEOM_EPS {
                        saw_drift = true;
                    }
                }
                _ => {
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
                }
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
    (errors, saw_drift)
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
        let (v_min, v_max) =
            vals.iter().fold((f64::MAX, f64::MIN), |(lo, hi), (_, v)| (lo.min(*v), hi.max(*v)));
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
/// merged with the Slint text rects and the Compose text rects (each
/// dilated 1px — the engines place a label's ink a pixel apart), plus a
/// `Skip` disagreement band around every element both sides trace.
fn build_frame_mask<C: i_slint_core::api::ComponentHandle>(
    component: &C,
    slint_frame: &TraceFrame,
    compose_frame: Option<&serde_json::Value>,
    inner_masked: &[String],
    decor_masked: &[(String, f64)],
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
    // The 2px dilation covers a centered label's position drift: inside a
    // pinned-width container the label block itself lands ~1px off between
    // engines (half the width slack), and its cells still check the ink.
    for handle in i_slint_backend_testing::ElementQuery::from_root(component)
        .match_inherits("Text")
        .find_all()
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
            .dilated(2.0),
            PixelClass::Text,
        );
    }
    // Rasterized icon content inside fixed bounds is the same drift class as
    // text: a centered icon's position carries half the label's width drift,
    // so per-cell checks apply instead of the strict layer.
    for handle in i_slint_backend_testing::ElementQuery::from_root(component)
        .match_inherits("Image")
        .find_all()
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
            .dilated(2.0),
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
                PxRect { x0: x * d, y0: y * d, x1: (x + w) * d, y1: (y + h) * d }.dilated(2.0),
                PixelClass::Text,
            );
        }
    }
    if let Some(compose_elements) = cf["elements"].as_object() {
        for (id, geo) in &slint_frame.elements {
            let Some(ce) = compose_elements.get(id) else { continue };
            let (Some(x), Some(y), Some(w), Some(h)) =
                (ce["x"].as_f64(), ce["y"].as_f64(), ce["w"].as_f64(), ce["h"].as_f64())
            else {
                continue;
            };
            let slint_rect = PxRect {
                x0: geo[0] * d,
                y0: geo[1] * d,
                x1: (geo[0] + geo[2]) * d,
                y1: (geo[1] + geo[3]) * d,
            };
            let compose_rect = PxRect { x0: x * d, y0: y * d, x1: (x + w) * d, y1: (y + h) * d };
            if inner_masked.contains(id) {
                // `//MASK_INNER=` — overlay ink (the ripple) is mid-animation
                // at this timestamp: skip the element interior. Ripple ink
                // is bounded by the rect, not the silhouette, so the
                // outside-silhouette corner cells stay masked here too
                // (mark_element's `inked` flag); the shape itself is still
                // verified by `corner_silhouette_findings` comparing the
                // boundary edge positions numerically.
                mask.fill_rect(
                    PxRect {
                        x0: slint_rect.x0.min(compose_rect.x0),
                        y0: slint_rect.y0.min(compose_rect.y0),
                        x1: slint_rect.x1.max(compose_rect.x1),
                        y1: slint_rect.y1.max(compose_rect.y1),
                    },
                    PixelClass::Skip,
                );
            }
            let decor_margin = decor_masked.iter().find(|(did, _)| did == id).map(|(_, m)| *m);
            mask.mark_element(
                slint_rect,
                compose_rect,
                decor_margin.unwrap_or(DECORATION_MARGIN_DP) * d,
                OUTLINE_INSET_DP * d,
                decor_margin.is_none(),
                inner_masked.contains(id),
            );
        }
    }
    mask
}

/// Fraction of an element's interior pixels that differ from the same
/// render's own pre-action baseline frame — the ink coverage. Comparing
/// each side against its own baseline keeps the measure renderer-neutral:
/// overlay color, antialiasing and clip behavior cancel out.
fn ink_coverage(
    img: &SharedPixelBuffer<Rgba8Pixel>,
    baseline: &SharedPixelBuffer<Rgba8Pixel>,
    rect: &PxRect,
    inset: f64,
    eps: u8,
) -> f64 {
    let r = rect.dilated(-inset);
    let (a, b) = (img.as_slice(), baseline.as_slice());
    let (aw, ah) = (img.width() as i64, img.height() as i64);
    let (mut total, mut inked) = (0u64, 0u64);
    for y in (r.y0.ceil() as i64)..(r.y1.floor() as i64) {
        for x in (r.x0.ceil() as i64)..(r.x1.floor() as i64) {
            if x < 0 || y < 0 || x >= aw || y >= ah {
                continue;
            }
            total += 1;
            let i = (y * aw + x) as usize;
            if channel_diff(&a[i], &b[i]) > eps {
                inked += 1;
            }
        }
    }
    if total == 0 {
        return 0.0;
    }
    inked as f64 / total as f64
}

/// For each `(slint, compose, id)` pair, compare the painted silhouette in
/// every corner zone: scan each boundary row/column from outside the bounds
/// inward and compare the position where each image first departs from the
/// outside color. Interior ink — legitimately engine-divergent under
/// `//MASK_INNER=` — never moves that edge, so the element's shape itself
/// stays verified where the strict layer is masked: a morph radius tracking
/// the reference stays within `CORNER_BAND + drift`, a dead morph or a
/// square corner does not. Rows/columns whose outside sample differs
/// between the images (a neighbor element or decoration) are skipped.
///
/// Returns `(message, x, y)` findings with device-px coordinates.
fn corner_silhouette_findings(
    actual: &SharedPixelBuffer<Rgba8Pixel>,
    expected: &SharedPixelBuffer<Rgba8Pixel>,
    pairs: &[(PxRect, PxRect, String)],
    margin: f64,
    eps: u8,
) -> Vec<(String, f64, f64)> {
    let mut out = Vec::new();
    let (a, e) = (actual.as_slice(), expected.as_slice());
    let (aw, ah) = (actual.width() as i64, actual.height() as i64);
    let get = |img: &[Rgba8Pixel], x: i64, y: i64| -> Option<Rgba8Pixel> {
        (x >= 0 && y >= 0 && x < aw && y < ah).then(|| img[(y * aw + x) as usize])
    };
    for (slint, compose, id) in pairs {
        let u = PxRect {
            x0: slint.x0.min(compose.x0),
            y0: slint.y0.min(compose.y0),
            x1: slint.x1.max(compose.x1),
            y1: slint.y1.max(compose.y1),
        };
        let r_pill = u.width().min(u.height()) / 2.0;
        let zone = r_pill + margin;
        for (along_x, side) in [(true, "left"), (true, "right"), (false, "top"), (false, "bottom")]
        {
            // Scan each boundary side inward from just outside each render's
            // own bound: the outside origin, the inward step, the scan's
            // inner end, and the bound the edge position is measured
            // against are all per render. The comparison is therefore the
            // silhouette's shape — edge offsets relative to each side's own
            // bounds — not its placement: bounds legitimately differ by the
            // text-hinting drift the trace layer bounds, and a width drift
            // moves an arc tangent's measured edge by more than the drift
            // itself.
            let params = |r: &PxRect| -> (f64, i64, f64, f64) {
                match side {
                    "left" => (r.x0 - margin - 1.0, 1, r.x0 + zone, r.x0),
                    "right" => (r.x1 + margin + 1.0, -1, r.x1 - zone, r.x1),
                    "top" => (r.y0 - margin - 1.0, 1, r.y0 + zone, r.y0),
                    _ => (r.y1 + margin + 1.0, -1, r.y1 - zone, r.y1),
                }
            };
            // Corner offsets on the perpendicular axis: +1 measures the
            // offset from that render's own lo bound, −1 from its hi bound.
            for corner in [1i64, -1i64] {
                for off in 0..(zone.ceil() as i64) {
                    let coord = |r: &PxRect| -> f64 {
                        let (lo, hi) = if along_x { (r.y0, r.y1) } else { (r.x0, r.x1) };
                        if corner > 0 { lo + off as f64 } else { hi - off as f64 }
                    };
                    let (fa, fe) = (coord(slint), coord(compose));
                    // A scan line outside a render's own bounds isn't a
                    // corner of its silhouette — nothing comparable there.
                    let claims = |r: &PxRect, f: f64| {
                        let (lo, hi) = if along_x { (r.y0, r.y1) } else { (r.x0, r.x1) };
                        f >= lo && f < hi
                    };
                    if !claims(slint, fa) || !claims(compose, fe) {
                        continue;
                    }
                    let (outer_a, step_a, inner_a, bound_a) = params(slint);
                    let (outer_e, step_e, inner_e, bound_e) = params(compose);
                    let p_out_a =
                        if step_a > 0 { outer_a.floor() as i64 } else { outer_a.ceil() as i64 };
                    let p_out_e =
                        if step_e > 0 { outer_e.floor() as i64 } else { outer_e.ceil() as i64 };
                    let p_end_a =
                        if step_a > 0 { inner_a.floor() as i64 } else { inner_a.ceil() as i64 };
                    let p_end_e =
                        if step_e > 0 { inner_e.floor() as i64 } else { inner_e.ceil() as i64 };
                    let (fai, fei) = (fa.round() as i64, fe.round() as i64);
                    let (sa, se) = if along_x {
                        (get(a, p_out_a, fai), get(e, p_out_e, fei))
                    } else {
                        (get(a, fai, p_out_a), get(e, fei, p_out_e))
                    };
                    let (Some(sa), Some(se)) = (sa, se) else { continue };
                    if channel_diff(&sa, &se) > eps {
                        continue;
                    }
                    let edge = |img: &[Rgba8Pixel],
                                outside: &Rgba8Pixel,
                                p_out: i64,
                                step: i64,
                                p_end: i64,
                                f: i64|
                     -> Option<i64> {
                        let mut p = p_out + step;
                        while (p - p_end) * step <= 0 {
                            let cell = if along_x { get(img, p, f) } else { get(img, f, p) };
                            match cell {
                                Some(c) if channel_diff(&c, outside) > eps => return Some(p),
                                Some(_) => p += step,
                                None => break,
                            }
                        }
                        None
                    };
                    let ea = edge(a, &sa, p_out_a, step_a, p_end_a, fai);
                    let ee = edge(e, &se, p_out_e, step_e, p_end_e, fei);
                    // Edge position relative to each render's own bound —
                    // the silhouette's depth, positive inward.
                    let rel = |pos: i64, step: i64, bound: f64| step as f64 * (pos as f64 - bound);
                    let ra = ea.map(|p| rel(p, step_a, bound_a));
                    let re = ee.map(|p| rel(p, step_e, bound_e));
                    // One side has an edge where the other has none: a real
                    // difference only when the pixel content actually
                    // differs at the same bound-relative position.
                    let bad = match (ra, re) {
                        (Some(ra), Some(re)) => (ra - re).abs() > CORNER_BAND,
                        (Some(ra), None) => {
                            let pos_in_other = bound_e + step_e as f64 * ra;
                            let (x, y) = if along_x {
                                (ea.unwrap(), fa.round() as i64)
                            } else {
                                (fa.round() as i64, ea.unwrap())
                            };
                            let (cx, cy) = if along_x {
                                (pos_in_other.round() as i64, fe.round() as i64)
                            } else {
                                (fe.round() as i64, pos_in_other.round() as i64)
                            };
                            match (get(a, x, y), get(e, cx, cy)) {
                                (Some(ca), Some(ce)) => channel_diff(&ca, &ce) > eps,
                                _ => false,
                            }
                        }
                        (None, Some(re)) => {
                            let pos_in_other = bound_a + step_a as f64 * re;
                            let (x, y) = if along_x {
                                (ee.unwrap(), fe.round() as i64)
                            } else {
                                (fe.round() as i64, ee.unwrap())
                            };
                            let (cx, cy) = if along_x {
                                (pos_in_other.round() as i64, fa.round() as i64)
                            } else {
                                (fa.round() as i64, pos_in_other.round() as i64)
                            };
                            match (get(a, cx, cy), get(e, x, y)) {
                                (Some(ca), Some(ce)) => channel_diff(&ca, &ce) > eps,
                                _ => false,
                            }
                        }
                        (None, None) => false,
                    };
                    if bad {
                        let (x, y) = if along_x {
                            (ea.or(ee).unwrap() as f64, if ea.is_some() { fa } else { fe })
                        } else {
                            (if ea.is_some() { fa } else { fe }, ea.or(ee).unwrap() as f64)
                        };
                        out.push((
                            format!(
                                "{id} {side} silhouette edge at corner offset {off}: slint {ra:?} vs compose {re:?} (band {CORNER_BAND:.1})"
                            ),
                            x,
                            y,
                        ));
                    }
                }
            }
        }
    }
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
    let xfail = kind == "xfail";
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
        if !negative && !xfail {
            return Ok(());
        }
    }

    let mut failures: Vec<String> = Vec::new();
    let mut strict_caught = 0usize;
    // Trace/geometry/text-metric findings — the layer a motion-class defect
    // (e.g. an offset on a moving element) is *supposed* to be caught by:
    // its pixels legitimately sit inside the elements' disagreement band.
    let mut compare_findings = 0usize;
    // A negative case must be caught at EVERY density, and a pixel defect
    // must be caught inside the region the mutation actually touched — the
    // traced elements' bounds union at the settle frame, dilated to cover
    // decorations and corner spill. (`None` = whole frame.)
    let mut caught_at_density = vec![false; spec.densities.len()];
    let mut measured_phase: Option<i64> = None;
    // Software-driver silhouette findings waived by `//XFAIL_SILHOUETTE=` —
    // counted across every density and timestamp so a marked case that
    // stops producing any fails "unexpectedly passing" below.
    let mut silhouette_xfail_total = 0usize;
    // Whether any frame at any density needed the `//XFAIL_TEXT=` marker's
    // relaxed per-cell pixel bound or showed its metric drift — the marker
    // is case-level, so the staleness verdict aggregates all densities.
    let mut xfail_text_pixels_used = false;
    let mut xfail_text_saw_drift = false;
    for (di, density) in spec.densities.iter().enumerate() {
        let component = make_instance(*density);

        // The Compose trace drives the per-frame masks as well as the
        // numeric comparisons — load it once per density.
        let dir = refs_dir(case_rel).join(format!("d{density}"));
        let compose: Option<serde_json::Value> = if references_missing {
            None
        } else {
            match std::fs::read_to_string(dir.join("trace.json"))
                .map_err(|e| format!("{e}"))
                .and_then(|json| serde_json::from_str(&json).map_err(|e| format!("{e}")))
            {
                Ok(v) => Some(v),
                Err(e) => {
                    failures.push(format!("d{density} trace.json: {e}"));
                    None
                }
            }
        };

        // Region the mutation may affect: the traced elements' bounds union
        // from the Compose trace's last (settled) frame, dilated past the
        // decoration margin so a defect bleeding outside the bounds — the
        // corner spill — still lands inside it. Without traced elements the
        // whole frame counts.
        let region: Option<PxRect> = compose
            .as_ref()
            .and_then(|c| c["frames"].as_array().and_then(|f| f.last()))
            .and_then(|f| f["elements"].as_object())
            .and_then(|els| {
                let mut u: Option<PxRect> = None;
                for id in &spec.trace_elements {
                    let Some(e) = els.get(id) else { continue };
                    let (Some(x), Some(y), Some(w), Some(h)) =
                        (e["x"].as_f64(), e["y"].as_f64(), e["w"].as_f64(), e["h"].as_f64())
                    else {
                        continue;
                    };
                    let r = PxRect {
                        x0: x * *density as f64,
                        y0: y * *density as f64,
                        x1: (x + w) * *density as f64,
                        y1: (y + h) * *density as f64,
                    };
                    u = Some(match u {
                        None => r,
                        Some(u) => PxRect {
                            x0: u.x0.min(r.x0),
                            y0: u.y0.min(r.y0),
                            x1: u.x1.max(r.x1),
                            y1: u.y1.max(r.y1),
                        },
                    });
                }
                u.map(|u| u.dilated((DECORATION_MARGIN_DP + 4.0) * *density as f64))
            });

        // The actions are input delivered at t=0: the frame at the first time
        // captures the pre-gesture state, so they dispatch right after it.
        // (Cases without //TIMES= only record the settled frame — apply first.)
        let actions_before_first_frame = spec.times.is_empty();
        if actions_before_first_frame {
            apply_actions(component.window(), spec);
        }

        // Static cases settle out any entry/ripple animation before the shot.
        // A negative case may still declare //TIMES= when its defect lives in
        // the trace layer (e.g. an offset on a moving element).
        let times: Vec<u64> =
            if !spec.times.is_empty() { spec.times.clone() } else { vec![STATIC_SETTLE_MS] };
        let start = i_slint_backend_testing::get_mocked_time();

        let mut frames = Vec::new();
        // `dispatched[i]` — each action fires at its `at_ms`; the untimed
        // (`0`) ones land just after the pre-gesture baseline frame. Static
        // scenes applied everything up front.
        let mut dispatched = vec![actions_before_first_frame; spec.actions.len()];
        let mut artifacts_written = false;
        // The first listed-time frames on each side are the pre-action
        // baseline `ink_coverage` measures against; `early_coverage` holds
        // the first post-action frame's coverage per element.
        let mut baseline_actual: Option<SharedPixelBuffer<Rgba8Pixel>> = None;
        let mut baseline_expected: Option<SharedPixelBuffer<Rgba8Pixel>> = None;
        let mut early_coverage: std::collections::HashMap<String, (f64, f64)> = Default::default();
        for &t in &times {
            // Actions timed inside the frame sequence fire at their own
            // clock time before this frame renders.
            for (i, action) in spec.actions.iter().enumerate() {
                if !dispatched[i] && action.at_ms() > 0 && action.at_ms() <= t {
                    advance_mock_time_to(start, action.at_ms());
                    dispatch_action(component.window(), action);
                    dispatched[i] = true;
                }
            }
            advance_mock_time_to(start, t);
            let actual = render_frame(&component);
            frames.push(capture_trace(&component, t, spec, prop_value));
            if t == times[0] {
                for (i, action) in spec.actions.iter().enumerate() {
                    if !dispatched[i] && action.at_ms() == 0 {
                        dispatch_action(component.window(), action);
                        dispatched[i] = true;
                    }
                }
            }

            if references_missing {
                continue;
            }
            let tag = if !spec.times.is_empty() { format!("{t}ms") } else { "settled".to_string() };
            let frame_path = dir.join(format!("frame_{tag}.png"));
            let mask_path = dir.join(format!("mask_{tag}.png"));
            let expected = match load_png(&frame_path) {
                Ok(e) => e,
                Err(e) => {
                    failures.push(format!("d{density} {e}"));
                    continue;
                }
            };
            if t == times[0] {
                baseline_actual = Some(actual.clone());
                baseline_expected = Some(expected.clone());
            }
            let png_mask = load_png(&mask_path).ok();
            let inner_masked: Vec<String> = spec
                .mask_inner
                .iter()
                .filter(|(_, ts)| *ts == t)
                .map(|(id, _)| id.clone())
                .collect();
            let decor_masked: Vec<(String, f64)> = spec
                .mask_decor
                .iter()
                .filter(|(_, ts, _)| *ts == t)
                .map(|(id, _, margin)| (id.clone(), margin.unwrap_or(DECORATION_MARGIN_DP)))
                .collect();
            let mask = build_frame_mask(
                &component,
                frames.last().unwrap(),
                compose.as_ref().and_then(|c| compose_frame_at(c, t)),
                &inner_masked,
                &decor_masked,
                png_mask.as_ref(),
                *density as f64,
                actual.width(),
                actual.height(),
            );
            // `xfail_text` scenes carry the documented issue-#28 advance drift
            // (Slint ceils text layout widths where Compose keeps fractional
            // advances): at headline sizes the accumulated drift moves glyph
            // edges ~1px inside a cell, just past the calibrated bound, so the
            // annotation also relaxes the per-cell mean — same tolerated
            // divergence the trace layer already grants it. The drift is a
            // constant logical-px error per advance, so its device-px
            // magnitude scales with the scene density: at 2x a barely-1dp
            // shift lands ~2 device px inside the cell. And where a label's
            // position follows the width of preceding siblings — every item
            // of a button-group row centering its own label — the per-advance
            // error accumulates into the label's placement before the glyph
            // even starts, so the relaxation needs headroom past the
            // single-label 1.25×.
            let text_cell_eps = if spec.xfail_text.is_some() {
                TEXT_CELL_EPS * 1.5 * *density as f64
            } else {
                TEXT_CELL_EPS
            };
            let result =
                layered_compare(&actual, &expected, Some(&mask), pixel_eps, region, text_cell_eps);
            strict_caught += result.strict_failures;
            xfail_text_pixels_used |= result.text_cells_relaxed > 0;
            if negative
                && (region.map_or(result.strict_failures, |_| result.strict_failures_in_region) > 0)
            {
                caught_at_density[di] = true;
            }
            // `//MASK_INNER=` masks the strict layer in the element interior,
            // so verify the shape silhouette itself: ink never moves a
            // boundary edge.
            let mut silhouette_failed = false;
            let mut silhouette_xfail = 0usize;
            // The software renderer can't clip to a rounded shape (#6): its
            // `clip` is axis-aligned, so bounded ripple ink legitimately
            // fills the corner-outside cells inside the element rect and the
            // silhouette edge can't be measured under it. Cases that
            // genuinely diverge there carry `//XFAIL_SILHOUETTE=` — their
            // findings are counted and reported, not added to errors —
            // while every unmarked case runs the check strictly on all
            // drivers. Negative cases run it strictly regardless: a defect
            // finding inside the mutated region counts wherever it fires.
            if !inner_masked.is_empty()
                && let Some(compose_elements) = compose
                    .as_ref()
                    .and_then(|c| compose_frame_at(c, t))
                    .and_then(|cf| cf["elements"].as_object())
            {
                let d = *density as f64;
                let slint_frame = frames.last().unwrap();
                let pairs: Vec<(PxRect, PxRect, String)> = inner_masked
                    .iter()
                    .filter_map(|id| {
                        let geo = slint_frame.elements.get(id)?;
                        let ce = compose_elements.get(id)?;
                        let (x, y, w, h) = (
                            ce["x"].as_f64()?,
                            ce["y"].as_f64()?,
                            ce["w"].as_f64()?,
                            ce["h"].as_f64()?,
                        );
                        Some((
                            PxRect {
                                x0: geo[0] * d,
                                y0: geo[1] * d,
                                x1: (geo[0] + geo[2]) * d,
                                y1: (geo[1] + geo[3]) * d,
                            },
                            PxRect { x0: x * d, y0: y * d, x1: (x + w) * d, y1: (y + h) * d },
                            id.clone(),
                        ))
                    })
                    .collect();
                for (msg, sx, sy) in corner_silhouette_findings(
                    &actual,
                    &expected,
                    &pairs,
                    DECORATION_MARGIN_DP * d,
                    pixel_eps,
                ) {
                    strict_caught += 1;
                    if driver == "software" && !negative && spec.xfail_silhouette.is_some() {
                        silhouette_xfail += 1;
                        silhouette_xfail_total += 1;
                        continue;
                    }
                    silhouette_failed = true;
                    if negative && region.is_none_or(|r| r.contains(sx, sy)) {
                        caught_at_density[di] = true;
                    }
                    failures.push(format!("d{density} t={tag}: {msg}"));
                }
                // `//MASK_INNER=` skips the ripple ink's coverage
                // entirely — the circle's shape and growth rate differ
                // legitimately between engines. What cannot differ is
                // that it grows: an instantly full-size ripple (the
                // f579301e2 defect) covers ~the whole interior from the
                // first post-action frame. Measure each image's ink
                // coverage against its own baseline, and require the
                // first post-action frame to sit well below the settled
                // coverage. Layoutlib's recorded ripple fades early on
                // Compose, so the bound applies only where the settle
                // frame proves ink actually reached the interior.
                //
                // The metric is coverage *against the baseline frame* —
                // it isolates ink only where ink is the only interior
                // change. Motion scenes (and mutated negative cases built
                // on them) animate the container shape on the same frames,
                // so coverage at the first post-action frame is dominated
                // by the morph, not the ripple — skip it there.
                let morphs = matches!(spec.parity.as_deref(), Some("motion") | Some("negative"));
                if let (Some(ba), Some(be), false) = (&baseline_actual, &baseline_expected, morphs)
                {
                    for (slint_r, compose_r, id) in &pairs {
                        let inset = CORNER_BAND + 2.0 * d;
                        let ca = ink_coverage(&actual, ba, slint_r, inset, pixel_eps);
                        let ce = ink_coverage(&expected, be, compose_r, inset, pixel_eps);
                        if Some(&t) == times.get(1) {
                            early_coverage.insert(id.clone(), (ca, ce));
                        }
                        if t == *times.last().unwrap() {
                            let Some(&(early_a, early_c)) = early_coverage.get(id) else {
                                continue;
                            };
                            for (early, cov, img_name) in
                                [(early_a, ca, "slint"), (early_c, ce, "compose")]
                            {
                                if cov > INK_SETTLED_MIN && early > cov - INK_GROWTH_MARGIN {
                                    strict_caught += 1;
                                    silhouette_failed = true;
                                    failures.push(format!(
                                        "d{density} t={tag}: {id} {img_name} ink coverage already {early:.2} at t={}ms, the first post-action frame; settled {cov:.2} — expected a growing ripple",
                                        times[1]
                                    ));
                                    if negative
                                        && region.is_none_or(|r| {
                                            r.contains(
                                                (slint_r.x0 + slint_r.x1) / 2.0,
                                                (slint_r.y0 + slint_r.y1) / 2.0,
                                            )
                                        })
                                    {
                                        caught_at_density[di] = true;
                                    }
                                }
                            }
                        }
                    }
                }
            }
            if silhouette_xfail > 0 {
                eprintln!(
                    "parity: xfail-silhouette {case_rel} d{density} t={tag}: {silhouette_xfail} findings on software (issue #6: software clip is axis-aligned, bounded ripple ink fills the corner cells — silhouette edge unverifiable)"
                );
            }
            if !result.ok || silhouette_failed || std::env::var_os("PARITY_DUMP_ACTUALS").is_some()
            {
                let dir = artifacts_dir(driver, case_rel);
                write_png(&dir.join(format!("actual_d{density}_{tag}.png")), &actual)?;
                write_png(&dir.join(format!("expected_d{density}_{tag}.png")), &expected)?;
                if let Some(diff) = &result.diff {
                    write_png(&dir.join(format!("diff_d{density}_{tag}.png")), diff)?;
                }
                if !result.ok || silhouette_failed {
                    artifacts_written = true;
                    failures.push(format!("d{density} t={tag}: {}", result.report));
                }
            }
        }

        if let Some(compose) = &compose {
            let (mut errors, phase) = if !spec.times.is_empty() {
                compare_traces(&frames, compose)
            } else {
                (Vec::new(), None)
            };
            measured_phase = measured_phase.or(phase);
            let (metric_errors, saw_drift) =
                compare_text_metrics(&component, &frames, compose, spec.xfail_text.as_deref());
            errors.extend(metric_errors);
            xfail_text_saw_drift |= saw_drift;
            compare_findings += errors.len();
            // A motion-class defect is caught by the trace layer; record the
            // density as caught when any trace/geometry/text finding fired.
            if negative && !errors.is_empty() {
                caught_at_density[di] = true;
            }
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

    if let Some(reason) = &spec.xfail_text {
        // The marker covers both the trace layer (width/center drift) and
        // the pixel layer (the relaxed per-cell mean it feeds) — only flag
        // it stale when neither consumer needed it at any density.
        if !xfail_text_saw_drift && !xfail_text_pixels_used {
            failures.push(format!(
                "no #28 text divergence (widths and pixel cells all within strict bounds) but the case is marked XFAIL_TEXT ({reason}) — remove the marker"
            ));
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
        // The case is deliberately wrong (see its `PARITY=negative:` note): a
        // pixel-class defect (color, overlay, shape) must fail at the strict
        // layer *inside the mutated region*; a motion-class defect (a
        // //TIMES= case) may instead be caught by the numeric trace layer —
        // a moved element's displaced pixels legitimately sit inside the
        // mask's disagreement band. The defect must be caught at EVERY
        // density: a comparator strong enough only at d1 is half a harness.
        // A failure reported for any other reason (missing file, IO) proves
        // nothing.
        let defect_caught = caught_at_density.iter().all(|&c| c);
        if !defect_caught {
            let missed: Vec<String> = spec
                .densities
                .iter()
                .zip(&caught_at_density)
                .filter(|(_, c)| !*c)
                .map(|(d, _)| format!("d{d}"))
                .collect();
            return Err(format!(
                "negative case {case_rel} produced no strict-pixel{} differences at {}{} — the harness did not catch the deliberate defect: {}",
                if spec.times.is_empty() { "" } else { " or trace" },
                missed.join(","),
                if failures.is_empty() {
                    String::new()
                } else {
                    format!(" (only non-comparison findings: {})", failures.join("; "))
                },
                spec.negative_note.as_deref().unwrap_or("(undocumented)")
            )
            .into());
        }
        eprintln!(
            "parity: negative case {case_rel} correctly rejected at every density ({strict_caught} strict pixels, {compare_findings} trace findings, {} failures)",
            failures.len()
        );
        return Ok(());
    }

    // `xfail:<driver>:` expects the divergence only on the named drivers;
    // everywhere else the case is a positive and must pass clean.
    let xfail_here = xfail
        && (spec.xfail_renderers.is_empty() || spec.xfail_renderers.iter().any(|d| d == driver));

    if xfail_here {
        if references_missing {
            // Same rule as negative: without references there is nothing
            // real to diverge from — skip, don't count IO errors as the
            // tracked divergence.
            return Ok(());
        }
        // A known, tracked divergence: the comparison MUST report at least
        // one finding — if it stops finding any, either the divergence was
        // fixed (retire the marker, the case graduates to a positive) or the
        // comparator went blind.
        let phase =
            measured_phase.map(|p| format!("; measured offset {p:+}ms")).unwrap_or_default();
        return if failures.is_empty() {
            Err(format!(
                "xfail case {case_rel} passed — expected a failure for: {} (remove the marker or reinstate the defect)",
                spec.xfail_note.as_deref().unwrap_or("(undocumented)")
            )
            .into())
        } else {
            eprintln!(
                "parity: xfail {case_rel} diverges as expected ({}{phase}): {} findings",
                spec.xfail_note.as_deref().unwrap_or("(undocumented)"),
                failures.len()
            );
            Ok(())
        };
    }

    // `//XFAIL_SILHOUETTE=` waives the software driver's silhouette
    // findings for the tracked axis-aligned-clip gap (#6). When the gap
    // closes — or the scene stops diverging for any other reason — the
    // waiver outlives it silently unless the case re-arms: a marked case
    // with zero findings must fail so the marker gets removed.
    if driver == "software" && spec.xfail_silhouette.is_some() && silhouette_xfail_total == 0 {
        return Err(format!(
            "parity: {case_rel} silhouette check unexpectedly passing on software — the #6 divergence is gone; remove the //XFAIL_SILHOUETTE= marker"
        )
        .into());
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
    all_text
        .fill_rect(PxRect { x0: 0.0, y0: 0.0, x1: size as f64, y1: size as f64 }, PixelClass::Text);
    assert!(
        layered_compare(&a, &b, Some(&all_text), PIXEL_EPS, None, TEXT_CELL_EPS).ok,
        "identical images must pass"
    );

    // 1px color nudge outside text → strict layer fails.
    b.make_mut_slice()[0] = Rgba8Pixel { r: 20, g: 0, b: 0, a: 255 };
    let no_text = PixelMask::new(size, size);
    assert!(
        !layered_compare(&a, &b, Some(&no_text), PIXEL_EPS, None, TEXT_CELL_EPS).ok,
        "1px diff must fail"
    );

    // Same nudge but fully inside the text mask → tolerated by the loose layer.
    assert!(
        layered_compare(&a, &b, Some(&all_text), PIXEL_EPS, None, TEXT_CELL_EPS).ok,
        "small diff inside text mask must pass"
    );

    // Blanket wrong color across the mask → loose layer fails.
    let mut c = a.clone();
    for p in c.make_mut_slice() {
        *p = Rgba8Pixel { r: 255, g: 0, b: 0, a: 255 };
    }
    assert!(
        !layered_compare(&a, &c, Some(&all_text), PIXEL_EPS, None, TEXT_CELL_EPS).ok,
        "wrong text color must fail"
    );
}
