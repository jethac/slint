// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

//! Generates the two sides of each Material parity scene from the shared
//! definitions in `ui-libraries/material/parity/scenes/*.json`:
//!
//! - `tests/screenshots/cases/material/<name>.slint` — the screenshot-driver
//!   case, carrying the `//PARITY=` markers the harness reads.
//! - `ui-libraries/material/parity/compose/src/test/resources/scenes/<name>.json`
//!   — the resolved scene the Compose harness renders: the authored scene plus
//!   the fully resolved color scheme (ARGB per role, computed with the pinned
//!   `material-color-utils` crate through the same constructor
//!   `i_slint_core::material::color_scheme` uses, so both sides paint
//!   identical colors) plus the `case_rel` the references are stored under.
//!
//! Run: `cargo run -p material-parity-generator` (regenerates everything),
//! `cargo run -p material-parity-generator -- --check` (fails if the checked-in
//! generated files have drifted from the scenes — used in CI).

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use material_color_utils::dynamiccolor::{DynamicScheme, MaterialDynamicColors, Platform, SpecVersion};
use material_color_utils::hct::Hct;
use material_color_utils::scheme;

#[derive(serde::Deserialize, serde::Serialize)]
struct Scene {
    name: String,
    #[serde(rename = "type")]
    ty: String,
    parity: String,
    negative_note: Option<String>,
    size: [u32; 2],
    densities: Vec<u32>,
    theme: Theme,
    fonts: Fonts,
    #[serde(default)]
    times: Vec<u64>,
    #[serde(default)]
    trace_props: Vec<String>,
    #[serde(default)]
    trace_elements: Vec<String>,
    #[serde(default)]
    actions: Vec<Action>,
    /// Timestamps (ms) at which an element's interior ink is excluded from
    /// pixel comparison — for widgets whose overlay animation (the ripple)
    /// is legitimately mid-flight at that frame; the element's boundary band
    /// still compares strictly.
    #[serde(default)]
    mask_inner: std::collections::BTreeMap<String, Vec<u64>>,
    /// Timestamps (ms) at which an element's decoration outside its
    /// silhouette (a drop shadow, a blur) is not comparable on the
    /// reference engine; the corner zones take the normal band. Either a
    /// bare timestamp list or `{"times": [...], "margin": dp}` — the
    /// margin widens the decoration band past the default 6dp for
    /// decorations that spill further (a big elevation shadow).
    #[serde(default)]
    mask_decor: std::collections::BTreeMap<String, MaskDecor>,
    #[serde(default)]
    params: serde_json::Map<String, serde_json::Value>,
    #[serde(default)]
    widgets: Vec<Widget>,
    /// Scene-level `negative` defects, keyed by the emitter that consumes
    /// them (`x_offset` shifts the spring-motion thumb off its animated
    /// property).
    #[serde(default)]
    slint_overrides: serde_json::Map<String, serde_json::Value>,
    /// `//XFAIL_TEXT=<reason>` — the case expects Slint's ceil-quantized
    /// text widths (issue #28); without it the width check bounds text to
    /// 0.5 px of the unhinted Compose advance.
    #[serde(default)]
    xfail_text: Option<String>,
    /// `//XFAIL_SILHOUETTE=<reason>` — on the software driver (axis-aligned
    /// clip, issue #6) the silhouette findings are an expected divergence;
    /// the case fails when they stop occurring.
    #[serde(default)]
    xfail_silhouette: Option<String>,
}

#[derive(serde::Deserialize, serde::Serialize)]
struct Theme {
    seed: String,
    variant: String,
    spec: String,
    platform: String,
    dark: bool,
    contrast: f64,
}

#[derive(serde::Deserialize, serde::Serialize)]
struct Fonts {
    plain: String,
    brand: String,
}

#[derive(serde::Deserialize, serde::Serialize)]
struct Action {
    /// `move`, `press`, `release`, or `key:<name>` (a named key like `Tab`).
    kind: String,
    #[serde(default)]
    x: f64,
    #[serde(default)]
    y: f64,
    /// Dispatch time within the frame sequence (ms). `0` fires right after
    /// the pre-gesture baseline frame, as before; a later value lets a
    /// gesture sequence play out across the timed frames (e.g. a press held
    /// until a mid-sequence release).
    #[serde(default)]
    at: f64,
    /// Release velocity in logical px/s for sheet fling replays: the Slint
    /// `SwipeGestureHandler.release-velocity` the scripted gesture produces
    /// (documented in the scene), which the Compose mirror feeds into the
    /// replicated fling behavior at the release's `at`. Unused by `//ACTION=`.
    #[serde(default)]
    velocity: f64,
}

/// A `mask_decor` entry: a bare timestamp list, or `times` plus a `margin`
/// (dp) widening the element's decoration band past the 6dp default.
#[derive(serde::Deserialize, serde::Serialize)]
#[serde(untagged)]
enum MaskDecor {
    Times(Vec<u64>),
    WithMargin { times: Vec<u64>, margin: f64 },
}

impl MaskDecor {
    fn times(&self) -> &[u64] {
        match self {
            Self::Times(ts) => ts,
            Self::WithMargin { times, .. } => times,
        }
    }

    fn margin(&self) -> Option<f64> {
        match self {
            Self::Times(_) => None,
            Self::WithMargin { margin, .. } => Some(*margin),
        }
    }
}

#[derive(serde::Deserialize, serde::Serialize)]
struct Widget {
    kind: String,
    #[serde(default)]
    x: f64,
    #[serde(default)]
    y: f64,
    #[serde(default)]
    width: Option<f64>,
    #[serde(default)]
    height: Option<f64>,
    #[serde(default)]
    radius: Option<f64>,
    #[serde(default)]
    text: Option<String>,
    /// Named icon for `icon-button` kinds or a leading icon on a text
    /// button: the stem of an svg under `src/ui/icons/` (e.g. `check` for
    /// `Icons.check`). The generator copies the svg into the Compose
    /// resources so both sides rasterize the identical path.
    #[serde(default)]
    icon: Option<String>,
    #[serde(default)]
    enabled: Option<bool>,
    /// Interaction state the widget starts in: `enabled` (default),
    /// `disabled`, `hovered`, `focused`, or `pressed`. The Compose side sets
    /// the interaction source directly; the Slint side gets `simulate_*`
    /// properties for hover/press and `key:Tab` actions for focus (see
    /// `button_props`/`widget_actions`).
    #[serde(default)]
    state: Option<String>,
    #[serde(default)]
    color: Option<String>,
    /// Button size bucket: `xs`, `s`, `m`, `l`, `xl` (default `s`, the
    /// upstream `MinHeight`/`smallContainerSize` bucket).
    #[serde(default)]
    size: Option<String>,
    /// Button container corners: `round` (default stadium) or `square`.
    #[serde(default)]
    corner: Option<String>,
    /// Renders the toggle variant of the widget (`checkable` on the Slint
    /// side, `*ToggleButton` composables upstream).
    #[serde(default)]
    checkable: Option<bool>,
    #[serde(default)]
    checked: Option<bool>,
    /// Icon-button container width: `narrow`, `uniform` (default), `wide`.
    #[serde(default)]
    width_option: Option<String>,
    /// `*-split-button` kinds only: which half carries the authored `state`
    /// and receives scripted pointer input — `leading` or `trailing`
    /// (default `trailing`).
    #[serde(default)]
    side: Option<String>,
    /// `*-split-button` trailing icon stem — defaults to
    /// `keyboard_arrow_down`, the chevron the upstream samples rotate.
    #[serde(default)]
    trailing_icon: Option<String>,
    /// M3 elevation level (0–5) for `surface` widgets: the Slint side sets
    /// `Elevation.level`, the Compose side sets `Modifier.shadow`'s dp.
    #[serde(default)]
    level: Option<i64>,
    /// `top-app-bar` variant: `small` (default), `center`, `medium`,
    /// `medium-flexible`, `large`, `large-flexible`, `two-rows`. Compose maps
    /// it to `TopAppBar`/`CenterAlignedTopAppBar`/`MediumTopAppBar`/
    /// `MediumFlexibleTopAppBar`/`LargeTopAppBar`/`LargeFlexibleTopAppBar`/
    /// `TwoRowsTopAppBar`.
    #[serde(default)]
    variant: Option<String>,
    /// Second line of the flexible two-row variants.
    #[serde(default)]
    subtitle: Option<String>,
    /// `TopAppBarState.heightOffset`/`BottomAppBarState.heightOffset` —
    /// negative values render a partially collapsed bar.
    #[serde(default)]
    height_offset: Option<f64>,
    /// `TopAppBarState.contentOffset` — positive values mark the content
    /// overlapped, which flips the single-row bar's container color to the
    /// scrolled color.
    #[serde(default)]
    content_offset: Option<f64>,
    /// `top-app-bar`/`search-bar` leading (`navigationIcon`) icon stem.
    #[serde(default)]
    nav_icon: Option<String>,
    /// `top-app-bar` action / `bottom-app-bar` icon-button icon stems,
    /// rendered left to right.
    #[serde(default)]
    icons: Vec<String>,
    /// `search-bar`/`app-bar-with-search` placeholder text.
    #[serde(default)]
    placeholder: Option<String>,
    /// Caster outline for `surface` widgets: a `MaterialShapes` global
    /// member name in kebab case (`"cookie-9-sided"` → `MaterialShapes.
    /// cookie-9-sided` / Compose `MaterialShapes.Cookie9Sided`), or `"rect"`
    /// (default) for the `radius` field's rounded rectangle.
    #[serde(default)]
    shape: Option<String>,
    /// `elevated-rect` only: the elevation in dp of the Android ambient+spot
    /// shadow — the Slint side sets a plain `Rectangle`'s `elevation`, the
    /// Compose side `Modifier.shadow`'s dp.
    #[serde(default)]
    elevation: Option<f64>,
    /// Sheet kinds: the sheet content's measured height in dp (the strip the
    /// `sheetContent` composable fills, excluding the drag handle).
    #[serde(default)]
    sheet_height: Option<f64>,
    /// `bottom-sheet-scaffold` only: `sheetPeekHeight` (default 56 dp).
    #[serde(default)]
    peek_height: Option<f64>,
    /// Sheet kinds: the state's `initialValue` — `hidden`,
    /// `partially-expanded`, or `expanded`.
    #[serde(default)]
    initial: Option<String>,
    /// Sheet kinds: `skipPartiallyExpanded` (`enabledValues` without
    /// `PartiallyExpanded`).
    #[serde(default)]
    skip_partial: Option<bool>,
    /// `bottom-sheet-scaffold` only: `skipHiddenState` (default true).
    #[serde(default)]
    skip_hidden: Option<bool>,
    /// Sheet kinds: `gesturesEnabled`/`sheetSwipeEnabled` (default true).
    #[serde(default)]
    gestures: Option<bool>,
    /// Sheet kinds: scheme role for the sheet content rect
    /// (`tertiary-container` default).
    #[serde(default)]
    content_color: Option<String>,
    /// `bottom-sheet-scaffold` only: scheme role for the scaffold body.
    #[serde(default)]
    body_color: Option<String>,
    /// `bottom-sheet-scaffold` only: `sheet-elevation-level` /
    /// `sheetShadowElevation` as an M3 level (absent = the pinned default,
    /// level 1). Scenes pass 0 — platform shadows deadlock layoutlib and
    /// shadow parity lives in the elevation scenes.
    #[serde(default)]
    sheet_elevation: Option<i64>,
    /// What this widget deliberately gets wrong on the Slint side
    /// (`negative` scenes only). Keys shadow the widget's own fields.
    #[serde(default)]
    slint_overrides: serde_json::Map<String, serde_json::Value>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let check = std::env::args().any(|a| a == "--check");
    let parity_dir: PathBuf = [env!("CARGO_MANIFEST_DIR"), ".."].iter().collect();
    let repo_root: PathBuf = [env!("CARGO_MANIFEST_DIR"), "..", "..", "..", ".."].iter().collect();
    let cases_dir = repo_root.join("tests/screenshots/cases/material");
    let resources_dir = parity_dir.join("compose/src/test/resources/scenes");
    let mut scene_names = Vec::new();

    let mut entries: Vec<_> = std::fs::read_dir(parity_dir.join("scenes"))?
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().map_or(false, |e| e == "json"))
        .collect();
    entries.sort();
    for path in entries {
        let scene: Scene = serde_json::from_slice(&std::fs::read(&path)?)
            .map_err(|e| format!("{}: {e}", path.display()))?;
        let case_rel = format!("material/{}", scene.name.replace('-', "_"));

        emit_or_check(&cases_dir.join(format!("{}.slint", scene.name.replace('-', "_"))), &slint_case(&scene), check)?;
        emit_or_check(
            &resources_dir.join(format!("{}.json", scene.name)),
            &serde_json::to_string_pretty(&resolved_scene(&scene, &case_rel))?,
            check,
        )?;
        // Icons the scene uses: the svg is copied next to the font so the
        // Compose side rasterizes literally the same path the Slint
        // `Icons.<name>` image does.
        for w in &scene.widgets {
            for icon in w
                .icon
                .iter()
                .chain(w.nav_icon.iter())
                .chain(w.icons.iter())
                .chain(w.trailing_icon.iter())
            {
                let src = repo_root
                    .join("ui-libraries/material/src/ui/icons")
                    .join(format!("{icon}.svg"));
                let dst = resources_dir
                    .parent()
                    .unwrap()
                    .join("icons")
                    .join(format!("{icon}.svg"));
                let svg = std::fs::read_to_string(&src)
                    .map_err(|e| format!("{}: {e}", src.display()))?;
                emit_or_check(&dst, &svg, check)?;
            }
        }
        scene_names.push(scene.name.clone());
    }
    emit_or_check(&resources_dir.join("index.txt"), &(scene_names.join("\n") + "\n"), check)?;

    // The Compose harness renders text in the same font the Slint driver
    // registers (the variable Roboto in `tests/screenshots/fonts/`).
    let font_src =
        repo_root.join("tests/screenshots/fonts/Roboto-VariableFont.ttf");
    let font_dst = parity_dir.join("compose/src/test/resources/fonts/roboto.ttf");
    let font = std::fs::read(&font_src)
        .map_err(|e| format!("{}: {e}", font_src.display()))?;
    if check {
        let on_disk = std::fs::read(&font_dst)
            .map_err(|e| format!("{}: {e} (regenerate)", font_dst.display()))?;
        if on_disk != font {
            return Err(format!("{} is stale — regenerate", font_dst.display()).into());
        }
    } else {
        std::fs::create_dir_all(font_dst.parent().unwrap())?;
        std::fs::write(&font_dst, font)?;
        eprintln!("wrote {}", font_dst.display());
    }
    Ok(())
}

/// Writes `content` at `path`, or — in `--check` mode — fails when `path`
/// doesn't hold exactly `content`.
fn emit_or_check(path: &Path, content: &str, check: bool) -> Result<(), Box<dyn std::error::Error>> {
    if check {
        let on_disk = std::fs::read_to_string(path)
            .map_err(|e| format!("{}: {e} (regenerate with `cargo run -p material-parity-generator`)", path.display()))?;
        if on_disk != content {
            return Err(format!(
                "{} is stale — regenerate with `cargo run -p material-parity-generator`",
                path.display()
            )
            .into());
        }
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, content)?;
    eprintln!("wrote {}", path.display());
    Ok(())
}

/// The scene JSON the Compose harness consumes: the authored fields verbatim,
/// plus `case_rel` and the resolved color scheme.
fn resolved_scene(scene: &Scene, case_rel: &str) -> serde_json::Value {
    let mut v = serde_json::json!({
        "name": scene.name,
        "type": scene.ty,
        "parity": scene.parity,
        "case_rel": case_rel,
        "size": scene.size,
        "densities": scene.densities,
        "theme": scene.theme,
        "fonts": scene.fonts,
        "times": scene.times,
        "trace_props": scene.trace_props,
        "trace_elements": scene.trace_elements,
        "actions": scene.actions.iter().chain(widget_actions(scene).iter()).collect::<Vec<_>>(),
        "mask_inner": scene.mask_inner,
        "mask_decor": scene.mask_decor,
        "params": scene.params,
        "widgets": scene.widgets,
        "scheme": scheme_argbs(&scene.theme),
    });
    if let Some(note) = &scene.negative_note {
        v["negative_note"] = note.clone().into();
    }
    v
}

/// Color role name → ARGB, mirroring `i_slint_core::material`'s
/// `color_scheme_to_struct` role list with Kotlin `ColorScheme` parameter
/// naming.
fn scheme_argbs(theme: &Theme) -> serde_json::Map<String, serde_json::Value> {
    let seed = u32::from_str_radix(theme.seed.trim_start_matches('#'), 16)
        .unwrap_or_else(|_| panic!("bad seed color {:?} (expected RRGGBB)", theme.seed));
    let seeds = vec![Hct::from_int((0xFF00_0000u64 | seed as u64) as i64)];
    let spec = match theme.spec.as_str() {
        "spec2021" => SpecVersion::Spec2021,
        "spec2025" => SpecVersion::Spec2025,
        "spec2026" => SpecVersion::Spec2026,
        other => panic!("unknown spec {other:?}"),
    };
    let platform = match theme.platform.as_str() {
        "phone" => Platform::Phone,
        "watch" => Platform::Watch,
        other => panic!("unknown platform {other:?}"),
    };
    let variant: fn(Vec<Hct>, bool, f64, SpecVersion, Platform) -> DynamicScheme =
        match theme.variant.as_str() {
            "monochrome" => scheme::SchemeMonochrome::new,
            "neutral" => scheme::SchemeNeutral::new,
            "tonal-spot" => scheme::SchemeTonalSpot::new,
            "vibrant" => scheme::SchemeVibrant::new,
            "expressive" => scheme::SchemeExpressive::new,
            "fidelity" => scheme::SchemeFidelity::new,
            "content" => scheme::SchemeContent::new,
            "rainbow" => scheme::SchemeRainbow::new,
            "fruit-salad" => scheme::SchemeFruitSalad::new,
            "cmf" => scheme::SchemeCmf::new,
            other => panic!("unknown variant {other:?}"),
        };
    let dynamic = variant(seeds, theme.dark, theme.contrast, spec, platform);
    let c = MaterialDynamicColors::new();
    let roles: Vec<(&str, _)> = vec![
        ("primary", c.primary()),
        ("onPrimary", c.on_primary()),
        ("primaryContainer", c.primary_container()),
        ("onPrimaryContainer", c.on_primary_container()),
        ("inversePrimary", c.inverse_primary()),
        ("secondary", c.secondary()),
        ("onSecondary", c.on_secondary()),
        ("secondaryContainer", c.secondary_container()),
        ("onSecondaryContainer", c.on_secondary_container()),
        ("tertiary", c.tertiary()),
        ("onTertiary", c.on_tertiary()),
        ("tertiaryContainer", c.tertiary_container()),
        ("onTertiaryContainer", c.on_tertiary_container()),
        ("background", c.background()),
        ("onBackground", c.on_background()),
        ("surface", c.surface()),
        ("onSurface", c.on_surface()),
        ("surfaceVariant", c.surface_variant()),
        ("surfaceTint", c.surface_tint()),
        ("inverseSurface", c.inverse_surface()),
        ("inverseOnSurface", c.inverse_on_surface()),
        ("error", c.error()),
        ("onError", c.on_error()),
        ("errorContainer", c.error_container()),
        ("onErrorContainer", c.on_error_container()),
        ("outline", c.outline()),
        ("outlineVariant", c.outline_variant()),
        ("scrim", c.scrim()),
        ("surfaceBright", c.surface_bright()),
        ("surfaceDim", c.surface_dim()),
        ("surfaceContainer", c.surface_container()),
        ("surfaceContainerHigh", c.surface_container_high()),
        ("surfaceContainerHighest", c.surface_container_highest()),
        ("surfaceContainerLow", c.surface_container_low()),
        ("surfaceContainerLowest", c.surface_container_lowest()),
        ("primaryFixed", c.primary_fixed()),
        ("primaryFixedDim", c.primary_fixed_dim()),
        ("onPrimaryFixed", c.on_primary_fixed()),
        ("onPrimaryFixedVariant", c.on_primary_fixed_variant()),
        ("secondaryFixed", c.secondary_fixed()),
        ("secondaryFixedDim", c.secondary_fixed_dim()),
        ("onSecondaryFixed", c.on_secondary_fixed()),
        ("onSecondaryFixedVariant", c.on_secondary_fixed_variant()),
        ("tertiaryFixed", c.tertiary_fixed()),
        ("tertiaryFixedDim", c.tertiary_fixed_dim()),
        ("onTertiaryFixed", c.on_tertiary_fixed()),
        ("onTertiaryFixedVariant", c.on_tertiary_fixed_variant()),
    ];
    roles
        .into_iter()
        .map(|(name, color)| {
            (
                name.to_string(),
                format!("{:08X}", dynamic.get_argb(&color) as u64 as u32).into(),
            )
        })
        .collect()
}

/// The `.slint` case for one scene.
fn slint_case(scene: &Scene) -> String {
    let mut s = String::new();
    s.push_str(
        "// Copyright © SixtyFPS GmbH <info@slint.dev>\n// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0\n\n",
    );
    writeln!(
        s,
        "// Scene: {name} — generated from `ui-libraries/material/parity/scenes/{name}.json`\n// (edit the scene, regenerate with `cargo run -p material-parity-generator`).",
        name = scene.name
    )
    .unwrap();
    match scene.parity.as_str() {
        "negative" => {
            writeln!(
                s,
                "//PARITY=negative: {}",
                scene.negative_note.as_deref().unwrap_or("(undocumented)")
            )
            .unwrap();
        }
        "xfail" => {
            writeln!(
                s,
                "//PARITY=xfail: {}",
                scene.negative_note.as_deref().unwrap_or("(undocumented)")
            )
            .unwrap();
        }
        scoped if scoped.starts_with("xfail:") => {
            // `xfail:<driver>` expects the divergence only on the named
            // driver; the note follows the scope in the emitted marker.
            writeln!(
                s,
                "//PARITY={}: {}",
                scoped,
                scene.negative_note.as_deref().unwrap_or("(undocumented)")
            )
            .unwrap();
        }
        kind => writeln!(s, "//PARITY={kind}").unwrap(),
    }
    writeln!(s, "//SIZE={}x{}", scene.size[0], scene.size[1]).unwrap();
    if !scene.times.is_empty() {
        writeln!(
            s,
            "//TIMES={}",
            scene.times.iter().map(u64::to_string).collect::<Vec<_>>().join(",")
        )
        .unwrap();
    }
    if !scene.trace_props.is_empty() {
        writeln!(s, "//TRACE_PROPS={}", scene.trace_props.join(",")).unwrap();
    }
    if !scene.trace_elements.is_empty() {
        writeln!(s, "//TRACE_ELEMENTS={}", scene.trace_elements.join(",")).unwrap();
    }
    for a in scene.actions.iter().chain(widget_actions(scene).iter()) {
        if SHEET_OPS.contains(&a.kind.as_str()) {
            // Programmatic sheet ops aren't input events — the Compose side
            // maps them to SheetState calls; the Slint side gets a Timer.
            continue;
        }
        if let Some(key) = a.kind.strip_prefix("key:") {
            writeln!(s, "//ACTION=key:{key}").unwrap();
        } else if a.at > 0.0 {
            // A `release` lands 1 ms later on the Slint timeline: the Compose
            // side fires the post-draw runnable at `at` and the spring's first
            // write shows one frame of state→draw latency later (≈`at`+2),
            // while Slint's pointer release anchors its write at `at`+1.
            let at = a.at as i64 + if a.kind == "release" { 1 } else { 0 };
            writeln!(s, "//ACTION={}@{}:{},{}", a.kind, at, a.x as i64, a.y as i64).unwrap();
        } else {
            writeln!(s, "//ACTION={}:{},{}", a.kind, a.x as i64, a.y as i64).unwrap();
        }
    }
    for (id, ts) in &scene.mask_inner {
        writeln!(
            s,
            "//MASK_INNER={id}@{}",
            ts.iter().map(u64::to_string).collect::<Vec<_>>().join(",")
        )
        .unwrap();
    }
    for (id, spec) in &scene.mask_decor {
        let margin = match spec.margin() {
            Some(m) => format!("+{m}"),
            None => String::new(),
        };
        writeln!(
            s,
            "//MASK_DECOR={id}{margin}@{}",
            spec.times().iter().map(u64::to_string).collect::<Vec<_>>().join(",")
        )
        .unwrap();
    }
    if let Some(reason) = &scene.xfail_text {
        writeln!(s, "//XFAIL_TEXT={reason}").unwrap();
    }
    if let Some(reason) = &scene.xfail_silhouette {
        writeln!(s, "//XFAIL_SILHOUETTE={reason}").unwrap();
    }
    writeln!(
        s,
        "//DENSITIES={}",
        scene.densities.iter().map(u32::to_string).collect::<Vec<_>>().join(",")
    )
    .unwrap();

    let mut imports = vec!["MaterialPalette", "MaterialTheme", "MaterialWindow"];
    let mut needs_icons = false;
    for w in &scene.widgets {
        let component = match w.kind.as_str() {
            "filled-button" => "FilledButton",
            "tonal-button" => "TonalButton",
            "elevated-button" => "ElevatedButton",
            "outlined-button" => "OutlineButton",
            "text-button" => "TextButton",
            "icon-button" => "IconButton",
            "filled-icon-button" => "FilledIconButton",
            "tonal-icon-button" => "TonalIconButton",
            "outlined-icon-button" => "OutlineIconButton",
            "filled-split-button" => "FilledSplitButton",
            "tonal-split-button" => "TonalSplitButton",
            "elevated-split-button" => "ElevatedSplitButton",
            "outlined-split-button" => "OutlineSplitButton",
            // `surface` imports `Elevation`/`MaterialShapes` below instead;
            // `rect`/`elevated-rect` are plain `Rectangle`s — no import.
            "rect" | "surface" | "elevated-rect" => continue,
            "top-app-bar" => match w.variant.as_deref().unwrap_or("small") {
                "small" => "TopAppBar",
                "center" => "CenterAlignedTopAppBar",
                "medium" => "MediumTopAppBar",
                "medium-flexible" => "MediumFlexibleTopAppBar",
                "large" => "LargeTopAppBar",
                "large-flexible" => "LargeFlexibleTopAppBar",
                "two-rows" => "TwoRowsTopAppBar",
                other => panic!("unknown top-app-bar variant {other:?}"),
            },
            "bottom-app-bar" => "BottomAppBar",
            "search-bar" => "SearchBar",
            "app-bar-with-search" => "AppBarWithSearch",
            "drag-handle" => "BottomSheetDragHandle",
            "vertical-drag-handle" => "VerticalDragHandle",
            "bottom-sheet" => "BottomSheet",
            "bottom-sheet-scaffold" => "BottomSheetScaffold",
            "modal-bottom-sheet" => "ModalBottomSheet",
            other => panic!("unknown widget kind {other:?}"),
        };
        imports.push(component);
        if matches!(
            w.kind.as_str(),
            "bottom-sheet" | "bottom-sheet-scaffold" | "modal-bottom-sheet"
        ) {
            imports.push("SheetValue");
        }
        if w.icon.is_some()
            || w.nav_icon.is_some()
            || !w.icons.is_empty()
            || w.kind.ends_with("split-button")
        {
            needs_icons = true;
        }
        if w.size.is_some() || w.corner.is_some() || w.width_option.is_some() {
            imports.push("MaterialButtonSize");
            imports.push("MaterialButtonShape");
            imports.push("IconButtonWidth");
        }
    }
    if needs_icons {
        imports.push("Icons");
    }
    if scene.widgets.iter().any(|w| w.kind == "surface") {
        imports.push("Elevation");
        if scene
            .widgets
            .iter()
            .any(|w| w.shape.as_deref().is_some_and(|sh| sh != "rect"))
        {
            imports.push("MaterialShapes");
        }
    }
    if scene
        .widgets
        .iter()
        .any(|w| w.slint_overrides.contains_key("sheet_shape"))
    {
        imports.push("ShapeTokens");
    }
    imports.sort();
    imports.dedup();
    // The Compose side renders text in the variable Roboto under
    // `compose/src/test/resources/fonts/roboto.ttf` — the same file, kept
    // byte-identical by the staleness check above. Import the shared copy
    // so the driver registers it under the `Roboto` family name the scene
    // assigns to `MaterialTheme.{plain,brand}-family`.
    writeln!(
        s,
        "import \"../../fonts/Roboto-VariableFont.ttf\";\nimport {{ {} }} from \"@material\";\n",
        imports.join(", ")
    )
    .unwrap();
    writeln!(s, "export component TestCase inherits MaterialWindow {{").unwrap();
    writeln!(
        s,
        "    init => {{\n        // The scene pins seed #{seed}, {variant}, {spec}, {lightdark}, contrast {contrast}\n        // (all the library defaults; restated here so a default change can't\n        // silently move the parity baseline).\n        MaterialTheme.seed-color = #{seed};\n        MaterialTheme.dark = {dark};\n        MaterialTheme.contrast-level = {contrast};\n        MaterialTheme.use-platform-color = false;\n        MaterialTheme.plain-family = \"{plain}\";\n        MaterialTheme.brand-family = \"{brand}\";\n        // Compose builds its scene scheme from the same seed — use the\n        // runtime-generated scheme, not the static expressive table.\n        MaterialPalette.dynamic = true;\n    }}\n",
        seed = scene.theme.seed,
        variant = scene.theme.variant,
        spec = scene.theme.spec,
        lightdark = if scene.theme.dark { "dark" } else { "light" },
        contrast = scene.theme.contrast,
        dark = scene.theme.dark,
        plain = scene.fonts.plain,
        brand = scene.fonts.brand,
    )
    .unwrap();
    match scene.ty.as_str() {
        "canvas" => slint_canvas(&mut s, scene),
        "spring-motion" => slint_spring_motion(&mut s, scene),
        other => panic!("unknown scene type {other:?}"),
    }
    writeln!(s, "}}").unwrap();
    s
}

/// Input actions that put each widget in its authored `state` on the Slint
/// side. Emitted as `//ACTION=` markers plus passed through in the resolved
/// scene's `actions` so both sides drive the same gesture.
///
/// `hovered`/`pressed` go through the widget's `simulate_*` properties (see
/// `button_props`) — the single pointer could only hold one widget in the
/// state at a time. Slint material buttons take keyboard focus only via
/// Tab navigation — pointer presses don't steal focus (the FocusScope is
/// size zero) — so a `focused` widget gets `key:Tab` steps to reach it in
/// the scene's declaration order.
fn widget_actions(scene: &Scene) -> Vec<Action> {
    let mut actions = Vec::new();
    // Tab steps walk the focusable widgets in declaration order, starting
    // from no focus: reaching the widget at focusable ordinal `o` takes
    // `o + 1` Tabs. Later `focused` widgets continue from there.
    let mut tabs_emitted = 0usize;
    let mut ordinal = 0usize;
    for w in &scene.widgets {
        // A split button has two focusable halves — the trailing one is the
        // second Tab stop.
        let focusables = if w.kind.ends_with("split-button") { 2 } else { 1 };
        if w.enabled == Some(false) {
            // A disabled widget takes no Tab stop on either side, so it
            // does not consume focus ordinals.
            continue;
        }
        if w.state.as_deref() == Some("focused") {
            // Focused split halves: `leading` is the first of the pair.
            let target = if w.kind.ends_with("split-button")
                && w.side.as_deref() == Some("trailing")
            {
                ordinal + 1
            } else {
                ordinal
            };
            for _ in tabs_emitted..=target {
                actions.push(Action {
                    kind: "key:Tab".into(),
                    x: 0.0,
                    y: 0.0,
                    at: 0.0,
                    velocity: 0.0,
                });
            }
            tabs_emitted = target + 1;
        }
        ordinal += focusables;
    }
    // A motion scene animates the state change through its timed frames, so
    // `hovered`/`pressed` must be a real pointer gesture — a `simulate_*`
    // property would bind the state at construction and the morph's first
    // frame would already be flat. The point lands inside every size
    // bucket's drawn container (XS is 40x32 with a 48dp touch area).
    if !scene.times.is_empty() {
        for w in &scene.widgets {
            // Drag-handle states ride `simulate_*` props (emitted with the
            // component), not pointer gestures — one pointer can't hold
            // several handles at once.
            if w.kind == "vertical-drag-handle" {
                continue;
            }
            let kind = match w.state.as_deref() {
                Some("pressed") => "press",
                Some("hovered") => "move",
                _ => continue,
            };
            // Split buttons aim the gesture at the authored half: the
            // trailing half hugs the right edge (`w.width` pins the total —
            // the trailing keeps its intrinsic width, so a point near the
            // right edge lands inside it).
            let x = if w.kind.ends_with("split-button")
                && w.side.as_deref() == Some("trailing")
                && w.width.is_some()
            {
                w.x + w.width.unwrap() - 16.0
            } else {
                w.x + 20.0
            };
            actions.push(Action { kind: kind.into(), x, y: w.y + 16.0, at: 0.0, velocity: 0.0 });
        }
    }
    for w in &scene.widgets {
        match w
            .state
            .as_deref()
            .unwrap_or(if w.enabled == Some(false) { "disabled" } else { "enabled" })
        {
            "hovered" | "pressed" | "enabled" | "disabled" | "focused" | "dragged" => {}
            other => panic!("unknown widget state {other:?}"),
        }
    }
    actions
}

fn widget_num(v: &serde_json::Value) -> f64 {
    v.as_f64().unwrap_or_else(|| panic!("expected number, got {v}"))
}

/// `xs`/`s`/`m`/`l`/`xl` → the `MaterialButtonSize` enum variant.
fn size_variant(size: &str) -> &'static str {
    match size {
        "xs" => "extra_small",
        "s" => "small",
        "m" => "medium",
        "l" => "large",
        "xl" => "extra_large",
        other => panic!("unknown button size {other:?}"),
    }
}

/// The shared property lines every button-family component takes. A
/// `slint_overrides` entry shadows the widget's authored value — the
/// negative scenes use it to inject a defect only the Slint side renders.
fn button_props(w: &Widget, timed: bool) -> String {
    let mut p = String::new();
    let over = &w.slint_overrides;
    let bool_over = |k: &str, authored: Option<bool>| -> bool {
        over.get(k).and_then(|v| v.as_bool()).unwrap_or(authored.unwrap_or(false))
    };
    if let Some(text) = &w.text {
        writeln!(p, "        text: \"{text}\";").unwrap();
    }
    let size = over
        .get("size")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .or_else(|| w.size.clone());
    if let Some(size) = size {
        writeln!(p, "        size: MaterialButtonSize.{};", size_variant(&size)).unwrap();
    }
    let corner = over
        .get("corner")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .or_else(|| w.corner.clone());
    if corner.as_deref() == Some("square") {
        p.push_str("        button_shape: MaterialButtonShape.square;\n");
    }
    if bool_over("checkable", w.checkable) {
        p.push_str("        checkable: true;\n");
    }
    if bool_over("checked", w.checked) {
        p.push_str("        checked: true;\n");
    }
    if bool_over("inline", None) {
        p.push_str("        inline: true;\n");
    }
    let width = over
        .get("width_option")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .or_else(|| w.width_option.clone());
    if let Some(width) = width {
        writeln!(p, "        width_option: IconButtonWidth.{width};").unwrap();
    }
    let icon = over
        .get("icon")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .or_else(|| w.icon.clone());
    if let Some(icon) = icon {
        writeln!(p, "        icon: Icons.{icon};").unwrap();
    }
    if !bool_over("enabled", w.enabled.or(Some(true))) {
        p.push_str("        enabled: false;\n");
    }
    // Pin the drawn width when the scene does (Compose sets `Modifier.width`
    // the same way) — the corner band's drift term only stays tight when
    // text-metric drift can't widen the traced bounds.
    if let Some(width) = w.width {
        writeln!(p, "        width: {width}px;").unwrap();
    }
    // A declared `state` maps to the per-widget state hook — the same
    // interaction the Compose side emits on the widget's
    // `InteractionSource`; a pointer-driven `move`/`press` could only ever
    // hold one widget in the state at a time. Timed scenes get the gesture
    // from `widget_actions` instead so the morph animates on camera.
    if !timed {
        match w.state.as_deref() {
            Some("hovered") => p.push_str("        simulate_hover: true;\n"),
            Some("pressed") => p.push_str("        simulate_press: true;\n"),
            _ => {}
        }
    }
    // The Compose side sets `LocalMinimumInteractiveComponentSize` to 0 —
    // the scene coordinates place the drawn component on both sides.
    if !bool_over("enforce_touch_target", None) {
        p.push_str("        enforce_touch_target: false;\n");
    }
    p
}
/// `*-split-button` props — same spirit as `button_props` but the
/// component's split-specific surface: `trailing_checkable`, the trailing
/// chevron icon, per-side `simulate_*` hooks and the token override props
/// the negative scenes inject defects through.
fn split_button_props(w: &Widget, timed: bool) -> String {
    let mut p = String::new();
    let over = &w.slint_overrides;
    let bool_over = |k: &str, authored: Option<bool>| -> bool {
        over.get(k).and_then(|v| v.as_bool()).unwrap_or(authored.unwrap_or(false))
    };
    let len_over = |k: &str| -> Option<f64> { over.get(k).and_then(|v| v.as_f64()) };
    if let Some(text) = &w.text {
        writeln!(p, "        text: \"{text}\";").unwrap();
    }
    let size = over
        .get("size")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .or_else(|| w.size.clone());
    if let Some(size) = size {
        writeln!(p, "        size: MaterialButtonSize.{};", size_variant(&size)).unwrap();
    }
    if bool_over("checkable", w.checkable) {
        p.push_str("        trailing_checkable: true;\n");
    }
    if bool_over("checked", w.checked) {
        p.push_str("        checked: true;\n");
    }
    let icon = over
        .get("icon")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .or_else(|| w.icon.clone());
    if let Some(icon) = icon {
        writeln!(p, "        icon: Icons.{icon};").unwrap();
    }
    // The trailing chevron — the upstream samples' `KeyboardArrowDown`;
    // `trailing_icon` swaps the glyph (a wrong-icon negative defect).
    let trailing_icon = over
        .get("trailing_icon")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .or_else(|| w.trailing_icon.clone())
        .unwrap_or_else(|| "keyboard_arrow_down".into());
    writeln!(p, "        trailing_icon: Icons.{trailing_icon};").unwrap();
    if !bool_over("enabled", w.enabled.or(Some(true))) {
        p.push_str("        enabled: false;\n");
    }
    if let Some(width) = w.width {
        writeln!(p, "        width: {width}px;").unwrap();
    }
    if !timed {
        let side = w.side.as_deref().unwrap_or("trailing");
        match w.state.as_deref() {
            Some("hovered") => writeln!(p, "        {side}_simulate_hover: true;").unwrap(),
            Some("pressed") => writeln!(p, "        {side}_simulate_press: true;").unwrap(),
            _ => {}
        }
    }
    // Token overrides for `negative` scenes (defaults are the generated
    // token values; only a set key reaches the component).
    for k in ["spacing", "inner_corner", "inner_pressed_corner", "outer_corner"] {
        if let Some(v) = len_over(k) {
            writeln!(p, "        {k}: {v}px;").unwrap();
        }
    }
    if over.get("checked_overlay").and_then(|v| v.as_bool()) == Some(false) {
        p.push_str("        checked_overlay: false;\n");
    }
    if !bool_over("enforce_touch_target", None) {
        p.push_str("        enforce_touch_target: false;\n");
    }
    p
}

fn slint_canvas(s: &mut String, scene: &Scene) {
    // Buttons are named `button{n}` by count of button-family widgets, not
    // widget index — a backdrop `rect` ahead of a button leaves `button0`
    // intact. `sheet{n}`/`handle{n}` count sheet-family and handle widgets.
    let mut buttons = 0;
    let mut surfaces = 0;
    let mut appbars = 0;
    let mut sheets = 0;
    // `handle{n}` counts `drag-handle`s, `vhandle{n}` counts
    // `vertical-drag-handle`s — the Compose mirror numbers each kind alone.
    let mut vhandles = 0usize;
    let mut drags = 0usize;
    for w in scene.widgets.iter() {
        let component = match w.kind.as_str() {
            "filled-button" => "FilledButton",
            "tonal-button" => "TonalButton",
            "elevated-button" => "ElevatedButton",
            "outlined-button" => "OutlineButton",
            "text-button" => "TextButton",
            "icon-button" => "IconButton",
            "filled-icon-button" => "FilledIconButton",
            "tonal-icon-button" => "TonalIconButton",
            "outlined-icon-button" => "OutlineIconButton",
            "filled-split-button" => "FilledSplitButton",
            "tonal-split-button" => "TonalSplitButton",
            "elevated-split-button" => "ElevatedSplitButton",
            "outlined-split-button" => "OutlineSplitButton",
            "surface" => {
                let i = surfaces;
                surfaces += 1;
                // A `slint_overrides.level` makes the Slint side cast the
                // shadow of a different z than Compose renders — the
                // negative scene's deliberate wrong-shadow defect.
                let level = w
                    .slint_overrides
                    .get("level")
                    .map(|v| widget_num(v) as i64)
                    .unwrap_or_else(|| w.level.unwrap_or(0));
                let outline = match w.shape.as_deref() {
                    None | Some("rect") => {
                        format!("border-radius: {}px;", w.radius.unwrap_or(0.) as i64)
                    }
                    Some(name) => format!("shape: MaterialShapes.{name};"),
                };
                // A `slint_overrides.shape_fit` maps the outline the wrong
                // way on the Slint side — e.g. `fill` stretches the outline's
                // bounds where Compose maps the normalized (0,0)-(1,1) space —
                // the negative scene's deliberate silhouette defect.
                let fit_override = w
                    .slint_overrides
                    .get("shape_fit")
                    .and_then(|v| v.as_str())
                    .map(|f| format!("\n        shape-fit: ShapeFit.{f};"))
                    .unwrap_or_default();
                writeln!(
                    s,
                    "    surface{i} := Elevation {{\n        x: {}px;\n        y: {}px;\n        width: {}px;\n        height: {}px;\n        level: {level};\n        {outline}{fit_override}\n        background: MaterialPalette.{};\n    }}\n",
                    w.x as i64,
                    w.y as i64,
                    w.width.unwrap() as i64,
                    w.height.unwrap() as i64,
                    w.color.as_deref().unwrap_or("surface").replace('-', "_"),
                )
                .unwrap();
                if let Some(cover) = w.slint_overrides.get("cover") {
                    let fill = cover["fill"].as_str().unwrap_or("primary");
                    let fill_expr = if fill.starts_with('#') {
                        fill.to_lowercase()
                    } else {
                        format!("MaterialPalette.{}", fill.replace('-', "_"))
                    };
                    let cover_radius = cover["radius"]
                        .as_f64()
                        .map(|r| format!("{r}px"))
                        .unwrap_or_else(|| "0px".to_string());
                    writeln!(
                        s,
                        "    // Deliberate defect (scene `slint_overrides.cover`).\n    Rectangle {{\n        x: surface{i}.x;\n        y: surface{i}.y;\n        width: surface{i}.width;\n        height: surface{i}.height;\n        border-radius: {cover_radius};\n        background: {fill_expr};\n        opacity: {};\n    }}\n",
                        cover["opacity"].as_f64().unwrap_or(1.0),
                    )
                    .unwrap();
                }
                continue;
            }
            // Unlike `surface` (the Material `Elevation` component, which sets
            // the layer colors explicitly), `elevated-rect` exercises a plain
            // `Rectangle`'s `elevation` — the compiler-default shadow colors.
            "elevated-rect" => {
                writeln!(
                    s,
                    "    Rectangle {{\n        x: {}px;\n        y: {}px;\n        width: {}px;\n        height: {}px;\n        border-radius: {}px;\n        background: MaterialPalette.{};\n        elevation: {}px;\n    }}\n",
                    w.x as i64,
                    w.y as i64,
                    w.width.unwrap() as i64,
                    w.height.unwrap() as i64,
                    w.radius.unwrap_or(0.0) as i64,
                    w.color.as_deref().unwrap_or("primary").replace('-', "_"),
                    w.elevation.unwrap_or(0.0) as i64,
                )
                .unwrap();
                continue;
            }
            "top-app-bar" | "bottom-app-bar" | "search-bar" | "app-bar-with-search" => {
                let i = appbars;
                appbars += 1;
                appbar_widget(s, w, i);
                continue;
            }
            "drag-handle" | "vertical-drag-handle" => {
                // `handle{n}`/`vhandle{n}` are numbered per kind to match the
                // Compose mirror's tag counters.
                let i = if w.kind == "vertical-drag-handle" {
                    vhandles += 1;
                    vhandles - 1
                } else {
                    drags += 1;
                    drags - 1
                };
                handle_widget(s, w, i);
                continue;
            }
            "bottom-sheet" | "bottom-sheet-scaffold" | "modal-bottom-sheet" => {
                let i = sheets;
                sheets += 1;
                sheet_widget(s, w, i, scene);
                continue;
            }
            "rect" => {
                let radius = w
                    .slint_overrides
                    .get("radius")
                    .map(widget_num)
                    .or(w.radius)
                    .unwrap_or(0.0);
                writeln!(
                    s,
                    "    Rectangle {{\n        x: {}px;\n        y: {}px;\n        width: {}px;\n        height: {}px;\n        border-radius: {}px;\n        background: MaterialPalette.{};\n    }}\n",
                    w.x as i64,
                    w.y as i64,
                    w.width.unwrap() as i64,
                    w.height.unwrap() as i64,
                    radius as i64,
                    w.color.as_deref().unwrap_or("primary").replace('-', "_")
                )
                .unwrap();
                continue;
            }
            other => panic!("unknown widget kind {other:?}"),
        };
        let i = buttons;
        buttons += 1;
        writeln!(
            s,
            "    button{i} := {component} {{\n        x: {}px;\n        y: {}px;\n{}    }}\n",
            w.x as i64,
            w.y as i64,
            if w.kind.ends_with("split-button") {
                split_button_props(w, !scene.times.is_empty())
            } else {
                button_props(w, !scene.times.is_empty())
            },
        )
        .unwrap();
        // `TRACE_PROPS` reads properties on the test-case root — forward the
        // widget's live values through. `container_radius` is the corner
        // morph's animated value, which every button component exposes.
        if i == 0 {
            for prop in &scene.trace_props {
                let ty = match prop.as_str() {
                    "container_radius"
                    | "leading_inner_radius"
                    | "trailing_inner_radius" => "length",
                    "trailing_icon_rotation" => "angle",
                    other => panic!("no forwarding type known for trace prop {other:?}"),
                };
                writeln!(s, "    out property <{ty}> {prop}: button{i}.{prop};\n").unwrap();
            }
        }
        if let Some(cover) = w.slint_overrides.get("cover") {
            // `slint_overrides.cover` paints a rectangle over the whole
            // widget: an opaque one masks the real fill and every state
            // layer (and the focus ring, drawn last), a translucent one
            // adds a second overlay. `label` redraws the button text on
            // top so the defect stays in the button's body.
            let fill = cover["fill"].as_str().unwrap_or("primary");
            let fill_expr = if fill.starts_with('#') {
                fill.to_lowercase()
            } else {
                format!("MaterialPalette.{}", fill.replace('-', "_"))
            };
            let label = if cover["label"].as_bool().unwrap_or(true) {
                let label_fill =
                    cover["label_fill"].as_str().unwrap_or("on-primary").replace('-', "_");
                format!(
                    "        Text {{\n            text: \"{}\";\n            color: MaterialPalette.{label_fill};\n            font-family: \"Roboto\";\n            font-weight: 500;\n            font-size: 14px;\n            horizontal-alignment: center;\n            vertical-alignment: center;\n        }}\n",
                    w.text.as_deref().unwrap_or_default()
                )
            } else {
                String::new()
            };
            let cover_radius = cover["radius"]
                .as_f64()
                .map(|r| format!("{r}px"))
                .unwrap_or_else(|| format!("button{i}.height / 2"));
            writeln!(
                s,
                "    // Deliberate defect (scene `slint_overrides.cover`).\n    Rectangle {{\n        x: button{i}.x;\n        y: button{i}.y;\n        width: button{i}.width;\n        height: button{i}.height;\n        border-radius: {cover_radius};\n        background: {fill_expr};\n        opacity: {};\n{label}    }}\n",
                cover["opacity"].as_f64().unwrap_or(1.0),
            )
            .unwrap();
        }
    }
    sheet_trace_forwards(s, scene);
    sheet_op_timers(s, scene);
}

fn slint_spring_motion(s: &mut String, scene: &Scene) {
    let p = &scene.params;
    let get = |k: &str| widget_num(&p[k]);
    writeln!(
        s,
        "    // The property the trace and the Compose side both watch — the\n    // spring's animated value itself. Animating the property (not `thumb.x`,\n    // which would jump the source and animate only the binding) keeps the\n    // `get_thumb-x()` trace the live value. Pressing anywhere moves the\n    // thumb to `{target}px`; the spring settles it.\n    in-out property <length> thumb-x: {start}px;\n    animate thumb-x {{ duration: {dur}ms; easing: spring({bounce}); }}\n\n    thumb := Rectangle {{\n        x: root.thumb-x{x_offset};\n        y: {y}px;\n        width: {size}px;\n        height: {size}px;\n        border-radius: {radius}px;\n        background: MaterialPalette.{color};\n    }}\n\n    TouchArea {{\n        pointer-event(event) => {{\n            if event.kind == PointerEventKind.down {{\n                root.thumb-x = {target}px;\n            }}\n        }}\n    }}",
        start = get("start") as i64,
        target = get("target") as i64,
        // Scene-level `slint_overrides.x_offset` renders the thumb off its
        // animated property — the motion-class defect the trace layer must
        // catch (a pure shift leaves zero strict pixels).
        x_offset = scene
            .slint_overrides
            .get("x_offset")
            .map(|v| format!(" + {}px", widget_num(v) as i64))
            .unwrap_or_default(),
        y = get("y") as i64,
        size = get("size") as i64,
        radius = get("radius") as i64,
        color = p["color"].as_str().unwrap().replace('-', "_"),
        dur = get("duration_ms") as i64,
        bounce = get("bounce"),
    )
    .unwrap();
}

/// One app-bar family widget (`top-app-bar`, `bottom-app-bar`, `search-bar`,
/// `app-bar-with-search`): geometry plus the props the Compose mirror sets.
/// Elements are named `appbar{n}` in scene order.
fn appbar_widget(s: &mut String, w: &Widget, i: usize) {
    let component = match w.kind.as_str() {
        "top-app-bar" => match w.variant.as_deref().unwrap_or("small") {
            "small" => "TopAppBar",
            "center" => "CenterAlignedTopAppBar",
            "medium" => "MediumTopAppBar",
            "medium-flexible" => "MediumFlexibleTopAppBar",
            "large" => "LargeTopAppBar",
            "large-flexible" => "LargeFlexibleTopAppBar",
            "two-rows" => "TwoRowsTopAppBar",
            other => panic!("unknown top-app-bar variant {other:?}"),
        },
        "bottom-app-bar" => "BottomAppBar",
        "search-bar" => "SearchBar",
        "app-bar-with-search" => "AppBarWithSearch",
        other => panic!("unknown app-bar kind {other:?}"),
    };
    let mut p = String::new();
    if let Some(text) = &w.text {
        if w.kind == "search-bar" || w.kind == "app-bar-with-search" {
            writeln!(p, "        text: \"{text}\";").unwrap();
        } else {
            writeln!(p, "        title: \"{text}\";").unwrap();
        }
    }
    if let Some(subtitle) = &w.subtitle {
        writeln!(p, "        subtitle: \"{subtitle}\";").unwrap();
    }
    if let Some(placeholder) = &w.placeholder {
        writeln!(p, "        placeholder-text: \"{placeholder}\";").unwrap();
    }
    if let Some(icon) = &w.nav_icon {
        match w.kind.as_str() {
            "bottom-app-bar" => writeln!(p, "        fab-icon: Icons.{icon};").unwrap(),
            "search-bar" => writeln!(p, "        leading-icon: Icons.{icon};").unwrap(),
            "app-bar-with-search" => {
                writeln!(p, "        leading-icon: Icons.{icon};").unwrap()
            }
            _ => {
                writeln!(
                    p,
                    "        leading-button: {{ icon: Icons.{icon}, enabled: true }};"
                )
                .unwrap()
            }
        }
    }
    if !w.icons.is_empty() {
        match w.kind.as_str() {
            "bottom-app-bar" => {
                let items = w
                    .icons
                    .iter()
                    .map(|ic| format!("{{ icon: Icons.{ic}, enabled: true }}"))
                    .collect::<Vec<_>>()
                    .join(", ");
                writeln!(p, "        icon-buttons: [{items}];").unwrap();
            }
            "search-bar" => {
                if let Some(ic) = w.icons.first() {
                    writeln!(p, "        trailing-icon: Icons.{ic};").unwrap();
                }
            }
            _ => {
                let items = w
                    .icons
                    .iter()
                    .map(|ic| format!("{{ icon: Icons.{ic}, enabled: true }}"))
                    .collect::<Vec<_>>()
                    .join(", ");
                writeln!(p, "        trailing-buttons: [{items}];").unwrap();
            }
        }
    }
    if let Some(offset) = w.height_offset {
        writeln!(p, "        height-offset: {offset}px;").unwrap();
    }
    if let Some(offset) = w.content_offset {
        writeln!(p, "        content-offset: {offset}px;").unwrap();
    }
    if let Some(width) = w.width {
        writeln!(p, "        width: {width}px;").unwrap();
    }
    if w.enabled == Some(false) {
        p.push_str("        enabled: false;\n");
    }
    // Touch-target expansion matches `LocalMinimumInteractiveComponentSize`
    // being 0 on the Compose side; the search-bar family routes its buttons
    // through fixed-size icon slots instead.
    if w.kind != "search-bar" && w.kind != "app-bar-with-search" {
        p.push_str("        touch-target: false;\n");
    }
    writeln!(
        s,
        "    appbar{i} := {component} {{\n        x: {}px;\n        y: {}px;\n{}    }}\n",
        w.x as i64, w.y as i64, p,
    )
    .unwrap();
}

/// The content rect every sheet carries: a fixed-height fill so the sheet's
/// measured size is identical on both sides.
fn sheet_content_rect(w: &Widget) -> String {
    let h = w.sheet_height.unwrap_or(120.0);
    let color = w
        .content_color
        .as_deref()
        .unwrap_or("tertiary-container")
        .replace('-', "_");
    format!(
        "            Rectangle {{\n                height: {h}px;\n                background: MaterialPalette.{color};\n            }}\n",
        h = h as i64,
    )
}

/// `hidden`/`partially-expanded`/`expanded` → the `SheetValue` variant.
fn sheet_initial(w: &Widget) -> &'static str {
    match w.initial.as_deref() {
        None => "hidden",
        Some("hidden") => "hidden",
        Some("partially-expanded") => "partially-expanded",
        Some("expanded") => "expanded",
        Some(other) => panic!("unknown sheet initial value {other:?}"),
    }
}

/// The expressions `TRACE_PROPS` reads from a sheet component. Modal sheets
/// can't be read from outside their window, so those sync `changed`-mirrors
/// into `in-out` root properties instead (see `sheet_widget`).
fn sheet_trace_prop(prop: &str) -> (&'static str, &'static str) {
    match prop {
        "sheet-offset" => ("float", "sheet-offset"),
        "sheet-height" => ("float", "sheet-height"),
        "current-value" => ("float", "current-value"),
        "target-value" => ("float", "target-value"),
        "dragging" => ("bool", "dragging"),
        "is-visible" => ("bool", "is-visible"),
        "has-expanded-state" => ("bool", "has-expanded-state"),
        "has-partially-expanded-state" => ("bool", "has-partially-expanded-state"),
        other => panic!("no sheet trace expression known for {other:?}"),
    }
}

/// `SheetValue` → a plain number so both engines serialize the same scalar:
/// hidden = 0, partially-expanded = 1, expanded = 2.
fn sheet_value_num(expr: &str) -> String {
    format!("{expr} == SheetValue.hidden ? 0 : {expr} == SheetValue.partially-expanded ? 1 : 2")
}

/// One handle widget (`drag-handle` → `BottomSheetDragHandle`,
/// `vertical-drag-handle` → `VerticalDragHandle`), named `handle{n}`/
/// `vhandle{n}`.
fn handle_widget(s: &mut String, w: &Widget, i: usize) {
    match w.kind.as_str() {
        "drag-handle" => {
            writeln!(
                s,
                "    handle{i} := BottomSheetDragHandle {{\n        x: {}px;\n        y: {}px;\n        width: {}px;\n        enabled: true;\n    }}\n",
                w.x as i64,
                w.y as i64,
                w.width.unwrap_or(360.0) as i64,
            )
            .unwrap();
        }
        "vertical-drag-handle" => {
            // Drag-handle states ride the `simulate_*` hooks, like
            // `StateLayerArea.simulate_press`: the Compose mirror drives the
            // states through the hoisted `interactionSource`, and one pointer
            // can't hold several handles at once.
            let simulate = match w.state.as_deref() {
                Some("pressed") => "\n        simulate-press: true;",
                Some("dragged") => "\n        simulate-drag: true;",
                _ => "",
            };
            writeln!(
                s,
                "    vhandle{i} := VerticalDragHandle {{\n        x: {}px;\n        y: {}px;{}\n    }}\n",
                w.x as i64,
                w.y as i64,
                simulate,
            )
            .unwrap();
        }
        other => panic!("unknown handle kind {other:?}"),
    }
}

/// Mirror of `Scenes.kt`'s `sheetNeedsGestureReplay`: a scene with a pointer
/// action inside the widget bounds drives the Compose `ParitySheet`
/// (synthetic gesture replay, tracked surface) instead of `SheetWidget`
/// (the pinned composable, `Modifier.track` on the composable node). The
/// tracked bounds differ: `ParitySheet`'s surface moves with the drag while
/// `SheetWidget` reports the composable's own static node — the docked
/// `BottomSheet` measures `surface-width` x `sheet-height` at its box's top
/// edge and `BottomSheetScaffold`'s root `Box` fills the widget — so the
/// Slint `sheet{i}` element follows whichever one the scene picks.
fn sheet_needs_gesture_replay(w: &Widget, scene: &Scene) -> bool {
    let (ww, wh) = (w.width.unwrap_or(0.0), w.height.unwrap_or(0.0));
    scene.actions.iter().any(|a| {
        matches!(a.kind.as_str(), "press" | "move" | "release")
            && a.x >= w.x
            && a.x <= w.x + ww
            && a.y >= w.y
            && a.y <= w.y + wh
    })
}

/// One sheet widget, named `sheet{n}` (`modal{n}` for the popup). The widget
/// bounds are the anchor space: the emit wraps the sheet in a sized
/// `Rectangle` so `container-height`/`container-width` resolve against the
/// authored box. `modal-bottom-sheet` spans the whole scene (the popup is
/// the test window) and gets an `init` call to `PopupWindow.show()`.
fn sheet_widget(s: &mut String, w: &Widget, i: usize, scene: &Scene) {
    let over = &w.slint_overrides;
    let content = sheet_content_rect(w);
    match w.kind.as_str() {
        "bottom-sheet" => {
            let shape_override = over
                .get("sheet_shape")
                .map(|v| format!("\n            sheet-shape: {};", corner_shape_expr(v)));
            let color_override = over
                .get("container_color")
                .and_then(|v| v.as_str())
                .map(|c| format!("\n            container-color: MaterialPalette.{};", c.replace('-', "_")));
            let skip_partial = over
                .get("skip_partial")
                .and_then(|v| v.as_bool())
                .unwrap_or_else(|| w.skip_partial.unwrap_or(false));
            writeln!(
                s,
                "    sheet{i}_box := Rectangle {{\n        x: {}px;\n        y: {}px;\n        width: {}px;\n        height: {}px;\n\n        sheet{i}_comp := BottomSheet {{\n            x: 0px;\n            y: 0px;\n            width: 100%;\n            height: 100%;\n            initial-value: SheetValue.{};\n            skip-partially-expanded: {};\n            gestures-enabled: {};{}{}\n{}        }}\n\n        sheet{i} := Rectangle {{ {} }}\n\n        // The drag-handle pill: 32x4 dp centered in the sheet's 48 dp\n        // handle strip — `SheetDefaults.kt`'s DragHandleVerticalPadding\n        // is 22 dp, so the pill sits 22 dp below the surface's top.\n        sheet-pill{i} := Rectangle {{\n            x: sheet{i}_comp.surface-x + (sheet{i}_comp.surface-width - 32px) / 2;\n            y: sheet{i}_comp.surface-y + 22px;\n            width: 32px;\n            height: 4px;\n        }}\n\n        // The moving surface — `sheet{i}` stays on the static widget\n        // container, so `mask_decor` against a mid-flight edge needs the\n        // surface's own bounds.\n        sheet-surface{i} := Rectangle {{\n            x: sheet{i}_comp.surface-x;\n            y: sheet{i}_comp.surface-y;\n            width: sheet{i}_comp.surface-width;\n            height: sheet{i}_comp.sheet-height;\n        }}\n\n        // The content region below the 48 dp handle strip — the\n        // handle/content boundary is painted inside the surface, so it\n        // needs its own element for `mask_decor` to reach it.\n        sheet-content{i} := Rectangle {{\n            x: sheet{i}_comp.surface-x;\n            y: sheet{i}_comp.surface-y + 48px;\n            width: sheet{i}_comp.surface-width;\n            height: sheet{i}_comp.sheet-height - 48px;\n        }}\n    }}\n",
                w.x as i64,
                w.y as i64,
                w.width.unwrap() as i64,
                w.height.unwrap() as i64,
                sheet_initial(w),
                skip_partial,
                w.gestures.unwrap_or(true),
                shape_override.unwrap_or_default(),
                color_override.unwrap_or_default(),
                content,
                if sheet_needs_gesture_replay(w, scene) {
                    format!("\n            x: sheet{i}_comp.surface-x;\n            y: sheet{i}_comp.surface-y;\n            width: sheet{i}_comp.surface-width;\n            height: sheet{i}_comp.surface-height;\n        ")
                } else {
                    format!("\n            x: sheet{i}_comp.surface-x;\n            y: 0px;\n            width: sheet{i}_comp.surface-width;\n            height: sheet{i}_comp.sheet-height;\n        ")
                },
            )
            .unwrap();
        }
        "bottom-sheet-scaffold" => {
            let body_color = w
                .body_color
                .as_deref()
                .unwrap_or("surface")
                .replace('-', "_");
            let shape_override = over
                .get("sheet_shape")
                .map(|v| format!("\n            sheet-shape: {};", corner_shape_expr(v)));
            let elevation = w
                .sheet_elevation
                .map(|l| format!("\n            sheet-elevation-level: {l};"))
                .unwrap_or_default();
            writeln!(
                s,
                "    sheet{i}_box := Rectangle {{\n        x: {}px;\n        y: {}px;\n        width: {}px;\n        height: {}px;\n\n        Rectangle {{\n            x: 0px;\n            y: 0px;\n            width: 100%;\n            height: 100%;\n            background: MaterialPalette.{body_color};\n        }}\n\n        sheet{i}_comp := BottomSheetScaffold {{\n            x: 0px;\n            y: 0px;\n            width: 100%;\n            height: 100%;\n            initial-value: SheetValue.{};\n            sheet-peek-height: {}px;\n            skip-hidden-state: {};\n            sheet-swipe-enabled: {};{}{}\n{}        }}\n\n        sheet{i} := Rectangle {{ {} }}\n\n        // The drag-handle pill: 32x4 dp centered in the sheet's 48 dp\n        // handle strip — `SheetDefaults.kt`'s DragHandleVerticalPadding\n        // is 22 dp, so the pill sits 22 dp below the surface's top.\n        sheet-pill{i} := Rectangle {{\n            x: sheet{i}_comp.surface-x + (sheet{i}_comp.surface-width - 32px) / 2;\n            y: sheet{i}_comp.surface-y + 22px;\n            width: 32px;\n            height: 4px;\n        }}\n\n        // The moving surface — `sheet{i}` stays on the static widget\n        // container, so `mask_decor` against a mid-flight edge needs the\n        // surface's own bounds.\n        sheet-surface{i} := Rectangle {{\n            x: sheet{i}_comp.surface-x;\n            y: sheet{i}_comp.surface-y;\n            width: sheet{i}_comp.surface-width;\n            height: sheet{i}_comp.sheet-height;\n        }}\n\n        // The content region below the 48 dp handle strip — the\n        // handle/content boundary is painted inside the surface, so it\n        // needs its own element for `mask_decor` to reach it.\n        sheet-content{i} := Rectangle {{\n            x: sheet{i}_comp.surface-x;\n            y: sheet{i}_comp.surface-y + 48px;\n            width: sheet{i}_comp.surface-width;\n            height: sheet{i}_comp.sheet-height - 48px;\n        }}\n    }}\n",
                w.x as i64,
                w.y as i64,
                w.width.unwrap() as i64,
                w.height.unwrap() as i64,
                if w.initial.is_some() { sheet_initial(w) } else { "partially-expanded" },
                w.peek_height.unwrap_or(56.0) as i64,
                w.skip_hidden.unwrap_or(true),
                w.gestures.unwrap_or(true),
                shape_override.unwrap_or_default(),
                elevation,
                content,
                if sheet_needs_gesture_replay(w, scene) {
                    format!("\n            x: sheet{i}_comp.surface-x;\n            y: sheet{i}_comp.surface-y;\n            width: sheet{i}_comp.surface-width;\n            height: sheet{i}_comp.surface-height;\n        ")
                } else {
                    "\n            x: 0px;\n            y: 0px;\n            width: 100%;\n            height: 100%;\n        ".to_string()
                },
            )
            .unwrap();
        }
        "modal-bottom-sheet" => {
            // The popup covers the test window: x/y pin to the origin and the
            // size to the scene, the convention `ModalBottomSheet` documents.
            let (sw, sh) = (scene.size[0], scene.size[1]);
            for prop in &scene.trace_props {
                let (ty, name) = sheet_trace_prop(prop);
                writeln!(s, "    in-out property <{ty}> {name};").unwrap();
            }
            let mut handlers = String::new();
            for prop in &scene.trace_props {
                match prop.as_str() {
                    "sheet-offset" | "sheet-height" => {
                        writeln!(
                            handlers,
                            "        changed {} => {{\n            root.{name} = self.{name} / 1px;\n        }}\n",
                            prop.replace('-', "_"),
                            name = prop,
                        )
                        .unwrap();
                    }
                    "current-value" | "target-value" => {
                        // `ParitySheet` tracks `AnchoredDraggableState`'s
                        // bound-crossing `currentValue`; `SheetWidget`
                        // reports `SheetState.currentValue` = `settledValue`.
                        let src = if prop == "current-value"
                            && sheet_needs_gesture_replay(w, scene)
                        {
                            "anchor_current".to_string()
                        } else {
                            prop.replace('-', "_")
                        };
                        writeln!(
                            handlers,
                            "        changed {src} => {{\n            root.{name} = {value};\n        }}\n",
                            name = prop,
                            value = sheet_value_num(&format!("self.{src}")),
                        )
                        .unwrap();
                    }
                    "dragging" | "is-visible" | "has-expanded-state"
                    | "has-partially-expanded-state" => {
                        writeln!(
                            handlers,
                            "        changed {} => {{\n            root.{name} = self.{name};\n        }}\n",
                            prop.replace('-', "_"),
                            name = prop,
                        )
                        .unwrap();
                    }
                    other => panic!("no modal trace mirror known for {other:?}"),
                }
            }
            // Popup properties are unreachable from outside the window, so
            // the surface geometry is mirrored into `in-out` root properties
            // like the trace props; `modal{i}` is the tracked surface frame
            // (a PopupWindow has no element in this component's item tree).
            writeln!(s, "    in-out property <length> surface-x;").unwrap();
            writeln!(s, "    in-out property <length> surface-y;").unwrap();
            writeln!(s, "    in-out property <length> surface-width;").unwrap();
            writeln!(s, "    in-out property <length> surface-height;").unwrap();
            write!(
                handlers,
                "        changed surface_x => {{\n            root.surface-x = self.surface-x;\n        }}\n        changed surface_y => {{\n            root.surface-y = self.surface-y;\n        }}\n        changed surface_width => {{\n            root.surface-width = self.surface-width;\n        }}\n        changed surface_height => {{\n            root.surface-height = self.surface-height;\n        }}\n"
            )
            .unwrap();
            writeln!(
                s,
                "    modal{i}_win := ModalBottomSheet {{\n        x: 0px;\n        y: 0px;\n        width: {sw}px;\n        height: {sh}px;\n        skip-partially-expanded: {};\n        gestures-enabled: {};\n{handlers}{}    }}\n\n    // `boundsInRoot` inside the popup is empty on the Compose side too.\n    modal{i} := Rectangle {{\n        x: 0px;\n        y: 0px;\n        width: 0px;\n        height: 0px;\n    }}\n",
                w.skip_partial.unwrap_or(false),
                w.gestures.unwrap_or(true),
                content,
            )
            .unwrap();
        }
        other => panic!("unknown sheet kind {other:?}"),
    }
}

/// `MaterialCornerShape` struct literal for `slint_overrides.sheet_shape` —
/// `{ "top_left": 12, "top_right": 12, "bottom_right": 0, "bottom_left": 0 }`
/// or a `ShapeTokens.corner_*` name string.
fn corner_shape_expr(v: &serde_json::Value) -> String {
    if let Some(name) = v.as_str() {
        return format!("ShapeTokens.{name}");
    }
    format!(
        "{{ top_left: {}px, top_right: {}px, bottom_right: {}px, bottom_left: {}px }}",
        widget_num(&v["top_left"]),
        widget_num(&v["top_right"]),
        widget_num(&v["bottom_right"]),
        widget_num(&v["bottom_left"]),
    )
}

/// Action kinds that mean "call the sheet's state method" rather than
/// deliver an input event — emitted as `Timer`s so the op lands at `at` ms
/// like the Compose runnable.
const SHEET_OPS: [&str; 4] = ["show", "hide", "expand", "partial-expand"];

/// One `Timer` per sheet-op action, firing `sheet0`/`modal0`'s public
/// functions at the action's `at` time (1 ms minimum — `at: 0` lands right
/// after the baseline frame like `//ACTION=` at 0 and the Compose runnable
/// at 0 do). A timer whose deadline falls in a gap fires inside the next
/// `mock_elapsed_time` advance — the op's animation anchors at that
/// advance's tick, so `at` should sit just before the listed frame where
/// the motion is meant to begin (the Compose runnable launches the same
/// `animateTo` there, ~2 ms of coroutine dispatch later).
fn sheet_op_timers(s: &mut String, scene: &Scene) {
    let ops: Vec<&Action> = scene
        .actions
        .iter()
        .filter(|a| SHEET_OPS.contains(&a.kind.as_str()))
        .collect();
    if ops.is_empty() {
        return;
    }
    // The target is the scene's first sheet widget; the trace guard already
    // keeps op scenes to a single sheet.
    let first = scene
        .widgets
        .iter()
        .find(|w| {
            matches!(
                w.kind.as_str(),
                "bottom-sheet" | "bottom-sheet-scaffold" | "modal-bottom-sheet"
            )
        })
        .expect("sheet op actions need a sheet widget");
    let name = if first.kind == "modal-bottom-sheet" { "modal0_win" } else { "sheet0_comp" };
    for a in ops {
        let fn_name = a.kind.replace('-', "_");
        writeln!(
            s,
            "    Timer {{\n        interval: {}ms;\n        triggered => {{\n            {name}.{fn_name}();\n            self.running = false;\n        }}\n    }}\n",
            (a.at as i64).max(1),
        )
        .unwrap();
    }
}

/// The trace-prop forwards for sheet and handle widgets: `out` bindings for
/// readable component properties (the modal case already emitted its `in-out`
/// mirrors in `sheet_widget`).
fn sheet_trace_forwards(s: &mut String, scene: &Scene) {
    let mut sheet_idx = 0usize;
    let mut vhandles = 0usize;
    for w in &scene.widgets {
        match w.kind.as_str() {
            "bottom-sheet" | "bottom-sheet-scaffold" => {
                let i = sheet_idx;
                sheet_idx += 1;
                assert!(
                    scene.trace_props.is_empty() || sheet_idx == 1,
                    "trace_props on a multi-sheet scene would emit duplicate root properties — split the scene"
                );
                for prop in &scene.trace_props {
                    let (ty, name) = sheet_trace_prop(prop);
                    // `ParitySheet` registers `AnchoredDraggableState` — its
                    // `currentValue` is the bound-crossing tracker — while
                    // `SheetWidget`'s `SheetState.currentValue` maps to
                    // `settledValue` (`SheetDefaults.kt`).
                    let replay = sheet_needs_gesture_replay(w, scene);
                    let expr = match prop.as_str() {
                        "current-value" if replay => {
                            sheet_value_num(&format!("sheet{i}_comp.anchor-current"))
                        }
                        "current-value" | "target-value" => {
                            sheet_value_num(&format!("sheet{i}_comp.{name}"))
                        }
                        // `sheet-offset` reports the `scrollBy`-clamped offset
                        // (`clamped-offset`), not the raw spring value.
                        "sheet-offset" => format!("sheet{i}_comp.clamped-offset / 1px"),
                        "sheet-height" => {
                            format!("sheet{i}_comp.{name} / 1px")
                        }
                        _ => format!("sheet{i}_comp.{name}"),
                    };
                    writeln!(s, "    out property <{ty}> {name}: {expr};").unwrap();
                }
            }
            "modal-bottom-sheet" => {
                // Mirrors were emitted with the widget.
                sheet_idx += 1;
                assert!(
                    scene.trace_props.is_empty() || sheet_idx == 1,
                    "trace_props on a multi-sheet scene would emit duplicate root properties — split the scene"
                );
            }
            "vertical-drag-handle" => {
                let i = vhandles;
                vhandles += 1;
                assert!(
                    scene.trace_props.is_empty() || vhandles == 1,
                    "trace_props on a multi-handle scene would emit duplicate root properties — split the scene"
                );
                for prop in &scene.trace_props {
                    let (ty, expr) = match prop.as_str() {
                        "handle-width" => ("float", format!("vhandle{i}.handle-width / 1px")),
                        "handle-height" => ("float", format!("vhandle{i}.handle-height / 1px")),
                        "active" | "pressed" | "dragged" => {
                            ("bool", format!("vhandle{i}.{p}", p = prop.replace('-', "_")))
                        }
                        other => panic!("no handle trace expression known for {other:?}"),
                    };
                    writeln!(s, "    out property <{ty}> {prop}: {expr};").unwrap();
                }
            }
            _ => {}
        }
    }
}
