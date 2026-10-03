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

use std::collections::BTreeSet;
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
    /// `//TRACE_ITEMS=` — the local element ids a traced container's repeated
    /// children carry, when one container can emit items under several ids
    /// (mutually exclusive `for` loops). Omitting a container derives
    /// `{container}:item` from its `<container>item<i>` `trace_elements`
    /// entries. The Compose side ignores this field: `track` tags record
    /// `<container>item<i>` verbatim.
    #[serde(default)]
    trace_items: std::collections::BTreeMap<String, Vec<String>>,
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
    /// `//PARITY_EPS=<n>` — per-channel strict-pixel tolerance override for
    /// cases whose engine noise exceeds the default 8; carries the reason
    /// the override exists, emitted as a comment above the directive.
    #[serde(default)]
    parity_eps: Option<u8>,
    /// Why `parity_eps` is set — required when it is.
    #[serde(default)]
    parity_eps_reason: Option<String>,
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
    /// `connected-button` only: icon stem shown while checked — the
    /// upstream samples' filled/outlined swap.
    #[serde(default)]
    checked_icon: Option<String>,
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
    /// `extended-fab` expansion state (`expanded` upstream).
    #[serde(default)]
    expanded: Option<bool>,
    /// `fab`/`extended-fab` visibility — `visible` on
    /// `Modifier.animateFloatingActionButton` upstream.
    #[serde(default)]
    shown: Option<bool>,
    /// `fab` show/hide scale pivot — `alignment` on
    /// `Modifier.animateFloatingActionButton` (`bottom_end` default).
    #[serde(default)]
    alignment: Option<String>,
    /// `fab` show/hide minimum scale — `targetScale` on
    /// `Modifier.animateFloatingActionButton` (0.2 upstream).
    #[serde(default)]
    target_scale: Option<f64>,
    /// The prop a press+release click toggles — `expanded` or `shown`
    /// (fab kinds): the click flips it at the release action's `at` time.
    #[serde(default)]
    toggle: Option<String>,
    /// `extended-fab` label slot width pin — `Modifier.width` on the
    /// upstream `text` composable; 0/unset sizes the slot to the text.
    #[serde(default)]
    label_width: Option<f64>,
    /// Caster outline for `surface` widgets: a `MaterialShapes` global
    /// member name in kebab case (`"cookie-9-sided"` → `MaterialShapes.
    /// cookie-9-sided` / Compose `MaterialShapes.Cookie9Sided`), or `"rect"`
    /// (default) for the `radius` field's rounded rectangle.
    #[serde(default)]
    shape: Option<String>,
    /// Loading-indicator mode: indeterminate (default — the continuous
    /// morph loop) or driven by `progress`.
    #[serde(default)]
    indeterminate: Option<bool>,
    /// Determinate loading-indicator progress, 0–1.
    #[serde(default)]
    progress: Option<f64>,
    /// `elevated-rect` only: the elevation in dp of the Android ambient+spot
    /// shadow — the Slint side sets a plain `Rectangle`'s `elevation`, the
    /// Compose side `Modifier.shadow`'s dp.
    #[serde(default)]
    elevation: Option<f64>,
    /// What this widget deliberately gets wrong on the Slint side
    /// (`negative` scenes only). Keys shadow the widget's own fields.
    #[serde(default)]
    slint_overrides: serde_json::Map<String, serde_json::Value>,
    /// `connected-button` only: `start`/`middle`/`end` — the position's
    /// `connected*ButtonShapes` (`start` is the leading item of a
    /// horizontal group, the top item of a vertical one).
    #[serde(default)]
    position: Option<String>,
    /// `connected-button` only: the `VerticalButtonGroupSample` shapes —
    /// `CornerSize(100)` caps on `start`/`end`, uniform 6dp pressed.
    #[serde(default)]
    vertical: Option<bool>,
    /// `horizontal-divider`/`vertical-divider`/`divider` line thickness in
    /// dp — `DividerDefaults.Thickness` (1dp) when unset; `0` authors the
    /// upstream `Dp.Hairline` (one physical pixel).
    #[serde(default)]
    thickness: Option<f64>,
    /// Group rows — `button-group` `clickableItem`/`toggleableItem` calls or
    /// `connected-button-group`/`vertical-connected-button-group` items.
    #[serde(default)]
    items: Vec<GroupItem>,
    /// `button-group` selection mode: `none` (clickable items, default),
    /// `single` or `multiple` (toggle items).
    #[serde(default)]
    selection: Option<String>,
    /// `button-group` `expanded-ratio` — default `ButtonGroupDefaults.
    /// ExpandedRatio` (0.15).
    #[serde(default)]
    expanded_ratio: Option<f64>,
    /// `button-group` `spacing` — default `ButtonGroupSmallTokens.
    /// BetweenSpace` (12px).
    #[serde(default)]
    spacing: Option<f64>,
    /// `button-group` item the `pressed`/`hovered` `state` applies to
    /// (`simulate-index` on the Slint side, that item's interaction source
    /// upstream). Defaults to item 0.
    #[serde(default)]
    press_index: Option<i64>,
    /// `button-group` `current-index` for `single` selection (default -1).
    #[serde(default)]
    current_index: Option<i64>,
    /// Groups: every item carries its own checked state instead of one
    /// `selected_index`.
    #[serde(default)]
    multi_select: Option<bool>,
    /// Single-select groups: the checked item (`-1` selects none).
    #[serde(default)]
    selected_index: Option<i64>,
}

/// One group item — the arguments an upstream `clickableItem`/
/// `toggleableItem` (standard `button-group`) or a `connected-button-group`
/// item takes: the label, an optional leading `icon`, a `checked_icon` swap
/// while checked, a `weight` width share, `disabled`/`enabled`, `checked`,
/// and an interaction `state` the Compose side emits on that item's source.
#[derive(serde::Deserialize, serde::Serialize)]
struct GroupItem {
    #[serde(default)]
    text: Option<String>,
    /// Icon stem like the widget's `icon`; `checked_icon` replaces it
    /// while the item is checked.
    #[serde(default)]
    icon: Option<String>,
    #[serde(default)]
    checked_icon: Option<String>,
    /// Weighted width share — `button-group` only (`Float.NaN` upstream
    /// when absent).
    #[serde(default)]
    weight: Option<f64>,
    #[serde(default)]
    disabled: Option<bool>,
    #[serde(default)]
    enabled: Option<bool>,
    #[serde(default)]
    checked: Option<bool>,
    /// `pressed`/`hovered`/`focused` — static scenes bind it through
    /// `simulate_*_index` on the Slint side.
    #[serde(default)]
    state: Option<String>,
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
        .filter(|p| p.extension().is_some_and(|e| e == "json"))
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
            let item_icons = w.items.iter().filter_map(|it| it.icon.as_ref()).collect::<Vec<_>>();
            // The overflow indicator always uses the `more_vert` glyph on
            // both sides.
            let more_vert = String::from("more_vert");
            let group_icon = (w.kind == "button-group").then_some(&more_vert);
            for icon in w
                .icon
                .iter()
                .chain(w.checked_icon.iter())
                .chain(w.nav_icon.iter())
                .chain(w.icons.iter())
                .chain(item_icons)
                .chain(group_icon)
                .chain(w.trailing_icon.iter())
                .chain(w.items.iter().flat_map(|item| item.icon.iter().chain(item.checked_icon.iter())))
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
    if let Some(eps) = scene.parity_eps {
        writeln!(
            s,
            "// {}",
            scene.parity_eps_reason.as_deref().unwrap_or("(undocumented)")
        )
        .unwrap();
        writeln!(s, "//PARITY_EPS={eps}").unwrap();
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
    // `<container>item<i>` ids name a container's repeated children — one
    // shared qualified id per instance on the Slint side, so they go to
    // `//TRACE_ITEMS=` (enumerated in tree order) instead of
    // `//TRACE_ELEMENTS=` (a literal-id lookup). The Compose side reads the
    // same scene JSON and records them verbatim.
    let (item_containers, elements): (BTreeSet<String>, Vec<&String>) = {
        let mut containers = BTreeSet::new();
        let mut plain = Vec::new();
        for id in &scene.trace_elements {
            let stem = id.trim_end_matches(|c: char| c.is_ascii_digit());
            if stem.len() > "item".len() && stem.ends_with("item") && stem.len() < id.len() {
                containers.insert(stem[..stem.len() - "item".len()].to_string());
            } else {
                plain.push(id);
            }
        }
        (containers, plain)
    };
    if !elements.is_empty() {
        let elements = elements.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(",");
        writeln!(s, "//TRACE_ELEMENTS={elements}").unwrap();
    }
    let trace_items = item_containers
        .iter()
        .map(|c| {
            let locals = scene
                .trace_items
                .get(c)
                .cloned()
                .unwrap_or_else(|| vec!["item".to_string()]);
            format!("{c}:{}", locals.join("+"))
        })
        .collect::<Vec<_>>()
        .join(",");
    if !trace_items.is_empty() {
        writeln!(s, "//TRACE_ITEMS={trace_items}").unwrap();
    }
    for a in scene.actions.iter().chain(widget_actions(scene).iter()) {
        if let Some(key) = a.kind.strip_prefix("key:") {
            writeln!(s, "//ACTION=key:{key}").unwrap();
        } else if a.at > 0.0 {
            writeln!(s, "//ACTION={}@{}:{},{}", a.kind, a.at as i64, a.x as i64, a.y as i64)
                .unwrap();
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
            "loading-indicator" => "LoadingIndicator",
            "contained-loading-indicator" => "ContainedLoadingIndicator",
            "filled-split-button" => "FilledSplitButton",
            "tonal-split-button" => "TonalSplitButton",
            "elevated-split-button" => "ElevatedSplitButton",
            "outlined-split-button" => "OutlineSplitButton",
            "fab" => "FloatingActionButton",
            "extended-fab" => "ExtendedFloatingActionButton",
            // `surface` imports `Elevation`/`MaterialShapes` below instead;
            // `rect`/`elevated-rect` are plain `Rectangle`s — no import.
            "rect" | "surface" | "elevated-rect" => continue,
            // `divider` is the deprecated `HorizontalDivider` alias upstream.
            "divider" | "horizontal-divider" => "HorizontalDivider",
            "vertical-divider" => "VerticalDivider",
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
            "button-group" => "ButtonGroup",
            "connected-button" => "ConnectedButton",
            "connected-button-group" => "ConnectedButtonGroup",
            "vertical-connected-button-group" => "VerticalConnectedButtonGroup",
            other => panic!("unknown widget kind {other:?}"),
        };
        imports.push(component);
        if w.kind.starts_with("connected-button") || w.kind == "vertical-connected-button-group" {
            imports.push("ConnectedButtonPosition");
        }
        if w.icon.is_some()
            || w.checked_icon.is_some()
            || w.nav_icon.is_some()
            || !w.icons.is_empty()
            || w.items.iter().any(|item| item.icon.is_some() || item.checked_icon.is_some())
            || w.kind.ends_with("split-button")
        {
            needs_icons = true;
        }
        if w.kind == "button-group" {
            imports.push("ButtonGroupSelection");
            if w.size.is_some() || w.corner.is_some() {
                imports.push("MaterialButtonSize");
                imports.push("MaterialButtonShape");
            }
        } else if w.size.is_some() || w.corner.is_some() || w.width_option.is_some() {
            imports.push("MaterialButtonSize");
            imports.push("MaterialButtonShape");
            imports.push("IconButtonWidth");
        }
        if w.kind == "fab" || w.kind == "extended-fab" {
            imports.push("FabSize");
            imports.push("ExtendedFabSize");
            imports.push("FabElevation");
            imports.push("FabAlignment");
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
    // `o + 1` Tabs. Later `focused` widgets continue from there. A
    // connected-button-group contributes one focusable per enabled item —
    // a `focused` item walks to its own ordinal.
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
                actions.push(Action { kind: "key:Tab".into(), x: 0.0, y: 0.0, at: 0.0 });
            }
            tabs_emitted = target + 1;
        }
        // A `*-button-group`'s layout isn't focusable — only its items
        // land in the Tab chain (handled below). Anything else is
        // `focusables` focusables (2 for a split button's halves, else 1).
        if !w.kind.ends_with("button-group") {
            ordinal += focusables;
        }
        for item in &w.items {
            // Disabled items aren't in the Tab chain.
            if item.disabled == Some(true) {
                continue;
            }
            if item.state.as_deref() == Some("focused") {
                // `ordinal` counts the items before this one already.
                for _ in tabs_emitted..=ordinal {
                    actions.push(Action { kind: "key:Tab".into(), x: 0.0, y: 0.0, at: 0.0 });
                }
                tabs_emitted = ordinal + 1;
            }
            ordinal += 1;
        }
    }
    // A motion scene animates the state change through its timed frames, so
    // `hovered`/`pressed` must be a real pointer gesture — a `simulate_*`
    // property would bind the state at construction and the morph's first
    // frame would already be flat. The point lands inside every size
    // bucket's drawn container (XS is 40x32 with a 48dp touch area).
    if !scene.times.is_empty() {
        for w in &scene.widgets {
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
            actions.push(Action { kind: kind.into(), x, y: w.y + 16.0, at: 0.0 });
        }
    }
    for w in &scene.widgets {
        match w
            .state
            .as_deref()
            .unwrap_or(if w.enabled == Some(false) { "disabled" } else { "enabled" })
        {
            "hovered" | "pressed" | "enabled" | "disabled" | "focused" => {}
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

/// The container-role name whose `on-*` counterpart fills its content —
/// `contentColorFor` upstream resolves to this table.
fn on_color_role(role: &str) -> &str {
    match role {
        "primary" => "on_primary",
        "primary-container" => "on_primary_container",
        "secondary" => "on_secondary",
        "secondary-container" => "on_secondary_container",
        "tertiary" => "on_tertiary",
        "tertiary-container" => "on_tertiary_container",
        "error" => "on_error",
        "error-container" => "on_error_container",
        "inverse-surface" => "inverse_on_surface",
        "surface" | "surface-bright" | "surface-dim" | "surface-container"
        | "surface-container-high" | "surface-container-highest" | "surface-container-low"
        | "surface-container-lowest" => "on_surface",
        "surface-variant" => "on_surface_variant",
        other => panic!("no content-color role known for {other:?}"),
    }
}

/// `fab`/`extended-fab` props — same spirit as `button_props` but the FAB
/// surface: the `FabSize`/`ExtendedFabSize`/`FabElevation`/`FabAlignment`
/// enums, `expanded`/`shown`/`target_scale`, the `label_width` text-slot
/// pin and the `toggle` click wiring for morph scenes.
fn fab_props(w: &Widget, timed: bool) -> String {
    let mut p = String::new();
    let over = &w.slint_overrides;
    let bool_over = |k: &str, authored: Option<bool>| -> bool {
        over.get(k).and_then(|v| v.as_bool()).unwrap_or(authored.unwrap_or(false))
    };
    let extended = w.kind == "extended-fab";
    if extended {
        if let Some(text) = &w.text {
            writeln!(p, "        text: \"{text}\";").unwrap();
        }
        if let Some(v) = w.label_width {
            writeln!(p, "        label-width: {v}px;").unwrap();
        }
        let expanded = bool_over("expanded", w.expanded.or(Some(true)));
        writeln!(p, "        expanded: {expanded};").unwrap();
    }
    if let Some(text) = &w.text {
        if !extended {
            // A plain FAB's `text` authors the tooltip/accessibility label.
            writeln!(p, "        tooltip: \"{text}\";").unwrap();
        }
    }
    let size = over
        .get("size")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .or_else(|| w.size.clone());
    if let Some(size) = size {
        let en = if extended { "ExtendedFabSize" } else { "FabSize" };
        writeln!(p, "        size: {en}.{size};").unwrap();
    }
    let icon = over
        .get("icon")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .or_else(|| w.icon.clone());
    if let Some(icon) = icon {
        writeln!(p, "        icon: Icons.{icon};").unwrap();
    }
    if let Some(color) = &w.color {
        let role = color.replace('-', "_");
        writeln!(p, "        container_color: MaterialPalette.{role};").unwrap();
        writeln!(
            p,
            "        content_color: MaterialPalette.{};",
            on_color_role(color)
        )
        .unwrap();
    }
    let variant = over
        .get("variant")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .or_else(|| w.variant.clone());
    match variant.as_deref() {
        Some("lowered") => p.push_str("        elevation_style: FabElevation.lowered;\n"),
        Some("bottom-app-bar") => {
            p.push_str("        elevation_style: FabElevation.bottom_app_bar;\n")
        }
        _ => {}
    }
    let shown = bool_over("shown", w.shown.or(Some(true)));
    if !shown {
        p.push_str("        shown: false;\n");
    }
    if let Some(alignment) = &w.alignment {
        writeln!(p, "        alignment: FabAlignment.{alignment};").unwrap();
    }
    if let Some(scale) = w.target_scale {
        writeln!(p, "        target_scale: {scale};").unwrap();
    }
    if !bool_over("enabled", w.enabled.or(Some(true))) {
        p.push_str("        enabled: false;\n");
    }
    if !timed {
        match w.state.as_deref() {
            Some("hovered") => p.push_str("        simulate_hover: true;\n"),
            Some("pressed") => p.push_str("        simulate_press: true;\n"),
            _ => {}
        }
    }
    // A click flips the authored toggle prop on release — the Compose side
    // toggles its hoisted state at the same `at` time.
    if let Some(toggle) = &w.toggle {
        match toggle.as_str() {
            "expanded" if extended => {
                p.push_str("        clicked => { self.expanded = !self.expanded; }\n")
            }
            "shown" => p.push_str("        clicked => { self.shown = !self.shown; }\n"),
            other => panic!("unknown fab toggle {other:?}"),
        }
    }
    if !bool_over("enforce_touch_target", None) {
        p.push_str("        enforce_touch_target: false;\n");
    }
    p
}

fn slint_canvas(s: &mut String, scene: &Scene) {
    // Buttons are named `button{n}` by count of button-family widgets, not
    // widget index — a backdrop `rect` ahead of a button leaves `button0`
    // intact.
    let mut buttons = 0;
    let mut surfaces = 0;
    let mut appbars = 0;
    let mut groups = 0;
    let mut dividers = 0;
    for w in scene.widgets.iter() {
        let component = match w.kind.as_str() {
            "button-group" => {
                let i = groups;
                groups += 1;
                let over = &w.slint_overrides;
                let mut p = String::new();
                let width = over.get("width").map(widget_num).or(w.width);
                if let Some(width) = width {
                    writeln!(p, "        width: {width}px;").unwrap();
                }
                let selection = over
                    .get("selection")
                    .and_then(|v| v.as_str())
                    .map(str::to_string)
                    .or_else(|| w.selection.clone());
                if let Some(selection) = selection {
                    writeln!(p, "        selection: ButtonGroupSelection.{selection};").unwrap();
                }
                let ratio = over
                    .get("expanded_ratio")
                    .map(widget_num)
                    .or(w.expanded_ratio);
                if let Some(ratio) = ratio {
                    writeln!(p, "        expanded-ratio: {ratio};").unwrap();
                }
                let spacing = over.get("spacing").map(widget_num).or(w.spacing);
                if let Some(spacing) = spacing {
                    writeln!(p, "        spacing: {spacing}px;").unwrap();
                }
                if let Some(size) = &w.size {
                    writeln!(p, "        size: MaterialButtonSize.{};", size_variant(size))
                        .unwrap();
                }
                if w.corner.as_deref() == Some("square") {
                    p.push_str("        button-shape: MaterialButtonShape.square;\n");
                }
                if w.enabled == Some(false) {
                    p.push_str("        enabled: false;\n");
                }
                if let Some(current) = w.current_index {
                    writeln!(p, "        current-index: {current};").unwrap();
                }
                // `state` drives `simulate_*` on `press_index`'s item — the
                // same hook every button-family component exposes. Timed
                // scenes take real pointer `actions` instead so the width
                // morph animates on camera.
                if scene.times.is_empty() {
                    match w.state.as_deref() {
                        Some("hovered") => p.push_str("        simulate-hover: true;\n"),
                        Some("pressed") => p.push_str("        simulate-press: true;\n"),
                        _ => {}
                    }
                    if matches!(w.state.as_deref(), Some("hovered") | Some("pressed"))
                        || w.press_index.is_some()
                    {
                        writeln!(
                            p,
                            "        simulate-index: {};",
                            w.press_index.unwrap_or(0)
                        )
                        .unwrap();
                    }
                }
                let items = &w.items;
                let rows = items
                    .iter()
                    .map(|it| {
                        let mut f = String::new();
                        if let Some(text) = &it.text {
                            f.push_str(&format!("text: \"{text}\", "));
                        }
                        if let Some(icon) = &it.icon {
                            f.push_str(&format!("icon: Icons.{icon}, "));
                        }
                        if let Some(weight) = it.weight {
                            f.push_str(&format!("weight: {weight}, "));
                        }
                        if it.checked == Some(true) {
                            f.push_str("checked: true, ");
                        }
                        if it.enabled == Some(false) {
                            f.push_str("enabled: false, ");
                        }
                        format!("{{ {f}}}")
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                writeln!(
                    s,
                    "    group{i} := ButtonGroup {{\n        x: {}px;\n        y: {}px;\n{p}        items: [{rows}];\n    }}\n",
                    w.x as i64,
                    w.y as i64,
                )
                .unwrap();
                // `g{i}i{k}_w`/`g{i}i{k}_x` trace props read row `k`'s live
                // measure results off the model — the same numbers the
                // Compose side pulls from `elementBounds["group{i}item{k}"]`.
                for prop in &scene.trace_props {
                    let Some(rest) = prop.strip_prefix(&format!("g{i}i")) else {
                        continue;
                    };
                    if let Some(k) =
                        rest.strip_suffix("_w").and_then(|n| n.parse::<usize>().ok())
                    {
                        writeln!(
                            s,
                            "    out property <length> {prop}: group{i}.items[{k}].final-w;\n"
                        )
                        .unwrap();
                    } else if let Some(k) =
                        rest.strip_suffix("_x").and_then(|n| n.parse::<usize>().ok())
                    {
                        writeln!(
                            s,
                            "    out property <length> {prop}: group{i}.items[{k}].x-pos;\n"
                        )
                        .unwrap();
                    } else {
                        panic!("no forwarding known for trace prop {prop:?} on a button-group");
                    }
                }

                continue;
            }
            "filled-button" => "FilledButton",
            "tonal-button" => "TonalButton",
            "elevated-button" => "ElevatedButton",
            "outlined-button" => "OutlineButton",
            "text-button" => "TextButton",
            "icon-button" => "IconButton",
            "filled-icon-button" => "FilledIconButton",
            "tonal-icon-button" => "TonalIconButton",
            "outlined-icon-button" => "OutlineIconButton",
            "loading-indicator" | "contained-loading-indicator" => {
                let component = match w.kind.as_str() {
                    "loading-indicator" => "LoadingIndicator",
                    _ => "ContainedLoadingIndicator",
                };
                let indeterminate = w.indeterminate.unwrap_or(true);
                // `progress` only binds in determinate mode — the upstream
                // indeterminate composable takes no progress parameter.
                let progress = if indeterminate {
                    String::new()
                } else {
                    format!("\n        progress: {};", w.progress.unwrap_or(0.))
                };
                writeln!(
                    s,
                    "    {component} {{\n        x: {}px;\n        y: {}px;\n        indeterminate: {indeterminate};{progress}\n    }}\n",
                    w.x as i64,
                    w.y as i64,
                )
                .unwrap();
                continue;
            }
            "filled-split-button" => "FilledSplitButton",
            "tonal-split-button" => "TonalSplitButton",
            "elevated-split-button" => "ElevatedSplitButton",
            "outlined-split-button" => "OutlineSplitButton",
            "fab" => "FloatingActionButton",
            "extended-fab" => "ExtendedFloatingActionButton",
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
            "connected-button" => {
                let i = buttons;
                buttons += 1;
                connected_button_widget(s, w, i, scene);
                continue;
            }
            "connected-button-group" | "vertical-connected-button-group" => {
                let i = groups;
                groups += 1;
                connected_group_widget(s, w, i, scene);
                continue;
            }
            "divider" | "horizontal-divider" | "vertical-divider" => {
                let i = dividers;
                dividers += 1;
                let component = if w.kind == "vertical-divider" {
                    "VerticalDivider"
                } else {
                    "HorizontalDivider"
                };
                // The band pins its long axis from the scene; the short axis
                // is the divider's own `thickness` (`DividerDefaults.
                // Thickness` upstream when unset). `slint_overrides` shadow
                // the authored values for the negative scenes.
                let thickness = w
                    .slint_overrides
                    .get("thickness")
                    .map(widget_num)
                    .or(w.thickness);
                let thickness_prop = thickness
                    .map(|t| format!("\n        thickness: {t}px;"))
                    .unwrap_or_default();
                let color = w
                    .slint_overrides
                    .get("color")
                    .and_then(|v| v.as_str())
                    .map(str::to_string)
                    .or_else(|| w.color.clone());
                let color_prop = color
                    .map(|c| format!("\n        color: MaterialPalette.{};", c.replace('-', "_")))
                    .unwrap_or_default();
                let (width, height) = if w.kind == "vertical-divider" {
                    (thickness.unwrap_or(1.0), w.height.unwrap())
                } else {
                    (w.width.unwrap(), thickness.unwrap_or(1.0))
                };
                writeln!(
                    s,
                    "    divider{i} := {component} {{\n        x: {}px;\n        y: {}px;\n        width: {width}px;\n        height: {height}px;{thickness_prop}{color_prop}\n    }}\n",
                    w.x as i64,
                    w.y as i64,
                )
                .unwrap();
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
            } else if w.kind == "fab" || w.kind == "extended-fab" {
                fab_props(w, !scene.times.is_empty())
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
                let ty = trace_prop_type(prop);
                writeln!(s, "    out property <{ty}> {prop}: button{i}.{prop};\n").unwrap();
            }
        }
        emit_button_cover(s, w, i);
    }
}

/// The Slint property type of a `//TRACE_PROPS=` name: `container_radius`,
/// the connected-button `corner_*` morph values and the FAB container/label
/// geometry are all `length`; the FAB scale/fade props are `float`.
fn trace_prop_type(prop: &str) -> &'static str {
    match prop {
        "container_radius"
        | "leading_inner_radius"
        | "trailing_inner_radius"
        | "corner_top_left"
        | "corner_top_right"
        | "corner_bottom_right"
        | "corner_bottom_left"
        | "box_width"
        | "slot_width"
        | "shadow_elevation" => "length",
        "trailing_icon_rotation" => "angle",
        "label_alpha" | "show_scale" | "show_alpha" | "expand_progress" => "float",
        other => panic!("no forwarding type known for trace prop {other:?}"),
    }
}

/// `slint_overrides.cover` paints a rectangle over the widget: an opaque
/// one masks the real fill and every state layer (and the focus ring,
/// drawn last), a translucent one adds a second overlay. `label` redraws
/// the button text on top so the defect stays in the button's body.
fn emit_button_cover(s: &mut String, w: &Widget, i: usize) {
    if let Some(cover) = w.slint_overrides.get("cover") {
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

/// One `ConnectedButton` item of a group, or a standalone one — named
/// `button{n}` like the rest of the button family so `TRACE_ELEMENTS` and
/// per-corner `TRACE_PROPS` (`corner_top_left` …) resolve on the root.
fn connected_button_widget(s: &mut String, w: &Widget, i: usize, scene: &Scene) {
    let over = &w.slint_overrides;
    let mut p = String::new();
    let position = over
        .get("position")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .or_else(|| w.position.clone())
        .unwrap_or_else(|| "middle".to_string());
    writeln!(p, "        position: ConnectedButtonPosition.{position};").unwrap();
    let vertical = over
        .get("vertical")
        .and_then(|v| v.as_bool())
        .or(w.vertical)
        .unwrap_or(false);
    if vertical {
        p.push_str("        vertical: true;\n");
    }
    if let Some(text) = &w.text {
        writeln!(p, "        text: \"{text}\";").unwrap();
    }
    let icon = over
        .get("icon")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .or_else(|| w.icon.clone());
    if let Some(icon) = icon {
        writeln!(p, "        icon: Icons.{icon};").unwrap();
    }
    let checked_icon = over
        .get("checked_icon")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .or_else(|| w.checked_icon.clone());
    if let Some(icon) = checked_icon {
        writeln!(p, "        checked_icon: Icons.{icon};").unwrap();
    }
    let checked = over
        .get("checked")
        .and_then(|v| v.as_bool())
        .or(w.checked)
        .unwrap_or(false);
    if checked {
        p.push_str("        checked: true;\n");
    }
    let enabled = over
        .get("enabled")
        .and_then(|v| v.as_bool())
        .or(w.enabled)
        .unwrap_or(true);
    if !enabled {
        p.push_str("        enabled: false;\n");
    }
    if let Some(width) = w.width {
        writeln!(p, "        width: {width}px;").unwrap();
    }
    // Static scenes bind the authored state through the simulate hooks —
    // timed scenes get the real gesture from `widget_actions`.
    if scene.times.is_empty() {
        match w.state.as_deref() {
            Some("hovered") => p.push_str("        simulate_hover: true;\n"),
            Some("pressed") => p.push_str("        simulate_press: true;\n"),
            _ => {}
        }
    }
    if !over.get("enforce_touch_target").and_then(|v| v.as_bool()).unwrap_or(false) {
        p.push_str("        enforce_touch_target: false;\n");
    }
    writeln!(
        s,
        "    button{i} := ConnectedButton {{\n        x: {}px;\n        y: {}px;\n{}    }}\n",
        w.x as i64,
        w.y as i64,
        p,
    )
    .unwrap();
    if i == 0 {
        for prop in &scene.trace_props {
            let ty = trace_prop_type(prop);
            writeln!(s, "    out property <{ty}> {prop}: button{i}.{prop};\n").unwrap();
        }
    }
    emit_button_cover(s, w, i);
}

/// A `ConnectedButtonGroup`/`VerticalConnectedButtonGroup` — named
/// `group{n}`. The items array is the `ConnectedButtonGroupItem` struct
/// literal; per-item `state` drives `simulate_*_index` (static scenes).
fn connected_group_widget(s: &mut String, w: &Widget, i: usize, scene: &Scene) {
    let component = match w.kind.as_str() {
        "connected-button-group" => "ConnectedButtonGroup",
        "vertical-connected-button-group" => "VerticalConnectedButtonGroup",
        other => panic!("unknown connected group kind {other:?}"),
    };
    let over = &w.slint_overrides;
    let mut p = String::new();
    if w.items.is_empty() {
        panic!("{} needs at least one item", w.kind);
    }
    // `ConnectedButtonGroupItem` — every struct field is required in the
    // literal, so the absent icon/checked_icon get an empty image-url.
    let empty_img = "@image-url(\"\")";
    let items = w
        .items
        .iter()
        .map(|item| {
            format!(
                "{{ icon: {}, checked_icon: {}, text: {:?}, tooltip: \"\", disabled: {}, checked: {} }}",
                item.icon.as_deref().map(|i| format!("Icons.{i}")).unwrap_or_else(|| empty_img.into()),
                item.checked_icon
                    .as_deref()
                    .map(|i| format!("Icons.{i}"))
                    .unwrap_or_else(|| empty_img.into()),
                item.text.as_deref().unwrap_or_default(),
                item.disabled.unwrap_or(false),
                item.checked.unwrap_or(false),
            )
        })
        .collect::<Vec<_>>()
        .join(", ");
    writeln!(p, "        items: [{items}];").unwrap();
    if w.multi_select.unwrap_or(false) {
        p.push_str("        multi_select: true;\n");
    }
    let selected = over
        .get("selected_index")
        .map(|v| widget_num(v) as i64)
        .or(w.selected_index)
        .unwrap_or(-1);
    writeln!(p, "        selected_index: {selected};").unwrap();
    if let Some(spacing) = over.get("between_space").map(widget_num) {
        writeln!(p, "        between_space: {spacing}px;").unwrap();
    }
    if let Some(spacing) = over.get("item_spacing").map(widget_num) {
        writeln!(p, "        item_spacing: {spacing}px;").unwrap();
    }
    if let Some(width) = w.width {
        writeln!(p, "        width: {width}px;").unwrap();
    }
    if scene.times.is_empty() {
        let press = w
            .items
            .iter()
            .position(|item| item.state.as_deref() == Some("pressed"))
            .map(|i| i as i64)
            .unwrap_or(-1);
        let hover = w
            .items
            .iter()
            .position(|item| item.state.as_deref() == Some("hovered"))
            .map(|i| i as i64)
            .unwrap_or(-1);
        if press >= 0 {
            writeln!(p, "        simulate_press_index: {press};").unwrap();
        }
        if hover >= 0 {
            writeln!(p, "        simulate_hover_index: {hover};").unwrap();
        }
    }
    if !over.get("enforce_touch_target").and_then(|v| v.as_bool()).unwrap_or(false) {
        p.push_str("        enforce_touch_target: false;\n");
    }
    writeln!(
        s,
        "    group{i} := {component} {{\n        x: {}px;\n        y: {}px;\n{}    }}\n",
        w.x as i64,
        w.y as i64,
        p,
    )
    .unwrap();
    if let Some(cover) = w.slint_overrides.get("cover") {
        let fill = cover["fill"].as_str().unwrap_or("primary");
        let fill_expr = if fill.starts_with('#') {
            fill.to_lowercase()
        } else {
            format!("MaterialPalette.{}", fill.replace('-', "_"))
        };
        writeln!(
            s,
            "    // Deliberate defect (scene `slint_overrides.cover`).\n    Rectangle {{\n        x: group{i}.x;\n        y: group{i}.y;\n        width: group{i}.width;\n        height: group{i}.height;\n        background: {fill_expr};\n        opacity: {};\n    }}\n",
            cover["opacity"].as_f64().unwrap_or(1.0),
        )
        .unwrap();
    }
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
