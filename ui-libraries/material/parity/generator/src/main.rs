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
    /// `//PARITY_EPS=<n>` — per-channel strict-pixel tolerance override;
    /// the paired `eps_note` is emitted as the required justification
    /// comment above the directive.
    #[serde(default)]
    eps: Option<u32>,
    /// Why this case needs a raised `eps` — emitted as a `//` line just
    /// above `//PARITY_EPS=`.
    #[serde(default)]
    eps_note: Option<String>,
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
    /// `alert-dialog`/`basic-alert-dialog`: the dialog title.
    #[serde(default)]
    title: Option<String>,
    /// `date-picker`/`date-range-picker` initial `DisplayMode` — `picker`
    /// (default) or `input`.
    #[serde(default)]
    display_mode: Option<String>,
    /// `date-picker` selected day — ISO `YYYY-MM-DD` — and the `ListItem`
    /// `selected` visual state (`item_selected_*` colors + selected
    /// container shape; `checked` plays the same role for `checkable`
    /// widgets) — also `menu-item`/`menu` item `selected`/`checked` flags.
    /// The Kotlin side reads the same key polymorphically per widget kind,
    /// so the field stays a raw value here.
    #[serde(default)]
    selected: Option<serde_json::Value>,
    /// `date-range-picker` selection bounds — ISO dates.
    #[serde(default)]
    selected_start: Option<String>,
    #[serde(default)]
    selected_end: Option<String>,
    /// `displayedMonth` — `YYYY-MM-DD` or `YYYY-MM` (day 1).
    #[serde(default)]
    displayed: Option<String>,
    /// `SelectableDates` contiguous window — ISO dates, missing bound is
    /// unrestricted.
    #[serde(default)]
    selectable_from: Option<String>,
    #[serde(default)]
    selectable_to: Option<String>,
    /// `yearRange` — the upstream default 1900–2100.
    #[serde(default)]
    year_min: Option<i64>,
    #[serde(default)]
    year_max: Option<i64>,
    /// `showModeToggle` on the pickers — upstream default true.
    #[serde(default)]
    show_mode_toggle: Option<bool>,
    /// The dialog confirm `TextButton`'s enabled state.
    #[serde(default)]
    confirm_enabled: Option<bool>,
    /// `date-range-picker` months composed — the slint side renders a
    /// bounded list while upstream's `LazyColumn` composes every month in
    /// `yearRange`; the viewport only shows the same leading months.
    #[serde(default)]
    months_to_show: Option<i64>,
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
    /// `ListItem(onClick)` overload — `interactive` on the Slint side;
    /// `selectable`/`checkable` already imply it.
    #[serde(default)]
    interactive: Option<bool>,
    /// `ListItem(selected, onClick)` overload — `selectable` on the Slint side.
    #[serde(default)]
    selectable: Option<bool>,
    /// `DragInteraction` visual state on a list item (`ReorderListTokens`
    /// colors, dragged shape, dragged elevation). Scenes should not use it:
    /// the platform shadow it needs deadlocks layoutlib's renderer.
    #[serde(default)]
    dragged: Option<bool>,
    /// `segmentedShapes(index, count)` position — emitted as `index:`/`count:`
    /// on `SegmentedListItem` and as the `segmentedShapes(index, count)` call
    /// on the Compose side.
    #[serde(default)]
    index: Option<i64>,
    #[serde(default)]
    count: Option<i64>,
    /// `overlineContent` text on a list item.
    #[serde(default)]
    overline: Option<String>,
    /// `supportingContent` text on a list item.
    #[serde(default)]
    supporting: Option<String>,
    /// The supporting text wraps to a second line — the upstream
    /// `isSupportingMultiline` heuristic input to `ListItemType`.
    #[serde(default)]
    supporting_multiline: Option<bool>,
    /// 40px avatar circle with this label in the leading slot
    /// (`avatar_text` on the Slint side, `ItemLeadingAvatar*` upstream).
    #[serde(default)]
    avatar: Option<String>,
    /// Icon stem (`Icons.*`) in the trailing slot; `icon` fills the leading
    /// slot. `trailing_text` adds the label-small meta text. On a
    /// `*-split-button` it is the trailing-half chevron — defaults to
    /// `keyboard_arrow_down`, the chevron the upstream samples rotate.
    #[serde(default)]
    trailing_icon: Option<String>,
    #[serde(default)]
    trailing_text: Option<String>,
    /// `leading_image`: the `icon` stem doubles as the 56x56 image source —
    /// both sides rasterize the identical svg path, clipped to the
    /// corner-small image shape.
    #[serde(default)]
    leading_image: Option<String>,
    /// `*-split-button` kinds only: which half carries the authored `state`
    /// and receives scripted pointer input — `leading` or `trailing`
    /// (default `trailing`).
    #[serde(default)]
    side: Option<String>,
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
    /// Menu item kind for `menu`/`menu-popup`/`menu-item`/`menu-group`
    /// widgets: `standard` (default), `selectable`, or `checkable`.
    #[serde(default)]
    item_kind: Option<String>,
    /// Item models for `menu`/`menu-popup`/`menu-group` widgets: objects with
    /// `text`, `icon`, `selected_icon`, `trailing_icon`, `trailing_text`,
    /// `supporting_text`, `selected`, `checked`, `enabled` and an optional
    /// `state` (`hovered`/`pressed`/`focused` — the state hook reaches the
    /// item's `simulate_*`/`focused` properties on the Slint side and its
    /// `InteractionSource` on the Compose side).
    #[serde(default)]
    menu_items: Option<Vec<serde_json::Value>>,
    /// Group models for `menu-popup`: `[{label, items:[…]}]` — each entry's
    /// `items` uses the same fields as `menu_items`.
    #[serde(default)]
    groups: Option<Vec<serde_json::Value>>,
    /// `menu-item`/`menu-group` shape position (`MenuDefaults.itemShape` /
    /// `groupShape`): `standalone` (default), `leading`, `middle`, `trailing`.
    #[serde(default)]
    shape_position: Option<String>,
    /// Icon stems for the standalone `menu-item` widget.
    #[serde(default)]
    selected_icon: Option<String>,
    /// `menu-item`/`menu` item `trailingText` (the `trailingContent` text
    /// slot) and `supportingText` are the `trailing_text`/`supporting_text`
    /// fields declared above.
    #[serde(default)]
    supporting_text: Option<String>,
    /// `menu-group`/`menu-group-label` label text (upstream
    /// `MenuDefaults.DropdownMenuGroupLabel` content).
    #[serde(default)]
    label: Option<String>,
    /// `menu` only: hide the leading `first-index` items (overflow menus).
    #[serde(default)]
    first_index: Option<i64>,
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
    /// `NavigationBarItem.alwaysShowLabel` — the tall navigation bar only.
    #[serde(default)]
    always_show_label: Option<bool>,
    /// `ShortNavigationBarArrangement` — `equal-weight` (default) or
    /// `centered`.
    #[serde(default)]
    nav_arrangement: Option<String>,
    /// `NavigationItemIconPosition` — `top` (default) or `start`.
    #[serde(default)]
    icon_position: Option<String>,
    /// `material-surface`: upstream `tonalElevation` in dp.
    #[serde(default)]
    tonal_elevation: Option<f64>,
    /// `material-surface`: the ambient `LocalAbsoluteTonalElevation` a parent
    /// Surface would provide (dp) — the absolute-elevation sum is the tint
    /// key, so `tonal_elevation` + `parent_elevation` tints like a nested
    /// surface.
    #[serde(default)]
    parent_elevation: Option<f64>,
    /// `material-surface`: `BorderStroke` — `border_width` in dp and
    /// `border_color` as a palette role (default `outline`).
    #[serde(default)]
    border_width: Option<f64>,
    #[serde(default)]
    border_color: Option<String>,
    /// `material-surface` overloads: `clickable` (`onClick`) and
    /// `toggleable` (`checked`, reuses the `checked` field) — `selectable`
    /// and `selected` are declared above with the list-item overloads.
    #[serde(default)]
    clickable: Option<bool>,
    #[serde(default)]
    toggleable: Option<bool>,
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
    /// `connected-button-group`/`vertical-connected-button-group` items —
    /// and rail/navigation-bar items (`navigation-rail`,
    /// `wide-navigation-rail`, `modal-navigation-rail`, `navigation-bar`,
    /// `short-navigation-bar`): `[{ "text": "Inbox", "icon": "inbox",
    /// "selected_icon": "inbox", "badge": "3", "enabled": false }]`.
    /// The two kinds never coexist on one widget, so the field shapes
    /// are disjoint keys of the merged `GroupItem`.
    #[serde(default)]
    items: Vec<GroupItem>,
    /// Groups: every item carries its own checked state instead of one
    /// `selected_index`.
    #[serde(default)]
    multi_select: Option<bool>,
    /// Groups: the checked item (`-1` selects none); rails and
    /// navigation bars: the selected item (`current-index` on the Slint
    /// side).
    #[serde(default)]
    selected_index: Option<i64>,
}

/// One item of a `connected-button-group`: the label, an optional leading
/// icon, a `checked_icon` swap, `disabled`, `checked` (multi-select), and
/// an interaction `state` the Compose side emits on that item's source.
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
    #[serde(default)]
    disabled: Option<bool>,
    #[serde(default)]
    checked: Option<bool>,
    /// `pressed`/`hovered`/`focused` — static scenes bind it through
    /// `simulate_*_index` on the Slint side.
    #[serde(default)]
    state: Option<String>,
    // --- rail / navigation-bar item fields ---
    /// Replaces `icon` while the rail/bar item is selected.
    #[serde(default)]
    selected_icon: Option<String>,
    /// Badge text on the rail/bar item.
    #[serde(default)]
    badge: Option<String>,
    /// `NavigationItem.enabled` on the Slint side.
    #[serde(default)]
    enabled: Option<bool>,
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
            for icon in w
                .icon
                .iter()
                .chain(w.checked_icon.iter())
                .chain(w.nav_icon.iter())
                .chain(w.icons.iter())
                .chain(w.selected_icon.iter())
                .chain(w.trailing_icon.iter())
                .chain(w.leading_image.iter())
                .chain(w.items.iter().flat_map(|item| {
                    [item.icon.iter(), item.checked_icon.iter(), item.selected_icon.iter()]
                        .into_iter()
                        .flatten()
                }))
                .map(String::as_str)
                .chain(item_icons(&w.menu_items))
                .chain(group_item_icons(&w.groups))
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
    if let Some(eps) = scene.eps {
        if let Some(note) = &scene.eps_note {
            writeln!(s, "// {note}").unwrap();
        }
        writeln!(s, "//PARITY_EPS={eps}").unwrap();
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
            "list-item" => "ListTile",
            "segmented-list-item" => "SegmentedListItem",
            "loading-indicator" => "LoadingIndicator",
            "contained-loading-indicator" => "ContainedLoadingIndicator",
            "filled-split-button" => "FilledSplitButton",
            "tonal-split-button" => "TonalSplitButton",
            "elevated-split-button" => "ElevatedSplitButton",
            "outlined-split-button" => "OutlineSplitButton",
            "fab" => "FloatingActionButton",
            "extended-fab" => "ExtendedFloatingActionButton",
            "radio-button" => "RadioButton",
            "switch" => "Switch",
            "material-surface" => "Surface",
            // `surface` imports `Elevation`/`MaterialShapes` below instead;
            // `rect`/`elevated-rect` are plain `Rectangle`s — no import.
            "rect" | "surface" | "elevated-rect" => continue,
            "icon" => "Icon",
            // `divider` is the deprecated `HorizontalDivider` alias upstream.
            "divider" | "horizontal-divider" => "HorizontalDivider",
            "vertical-divider" => "VerticalDivider",
            "badge" => "Badge",
            "badged-box" => "BadgedBox",
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
            "menu" => {
                imports.push("MenuVariant");
                imports.push("MenuItemKind");
                "MenuInner"
            }
            "menu-popup" => {
                imports.push("MenuVariant");
                imports.push("MenuItemKind");
                "MenuPopupContent"
            }
            "menu-group" => {
                imports.push("MenuVariant");
                imports.push("MenuItemKind");
                imports.push("MenuShapePosition");
                "MenuGroupContent"
            }
            "menu-item" => {
                imports.push("MenuVariant");
                imports.push("MenuItemKind");
                imports.push("MenuShapePosition");
                "MenuItemContent"
            }
            "menu-divider" => "MenuDivider",
            "menu-group-label" => "MenuGroupLabel",
            "drag-handle" => "BottomSheetDragHandle",
            "vertical-drag-handle" => "VerticalDragHandle",
            "bottom-sheet" => "BottomSheet",
            "bottom-sheet-scaffold" => "BottomSheetScaffold",
            "modal-bottom-sheet" => "ModalBottomSheet",
            "connected-button" => "ConnectedButton",
            "connected-button-group" => "ConnectedButtonGroup",
            "vertical-connected-button-group" => "VerticalConnectedButtonGroup",
            // The dialog emitters draw the scrim inline; only their
            // content uses components.
            "alert-dialog" => "AlertDialogContent",
            "basic-alert-dialog" => "MaterialText",
            "date-picker" => "DatePicker",
            "date-range-picker" => "DateRangePicker",

            "navigation-bar" => "NavigationBar",
            "short-navigation-bar" => "ShortNavigationBar",
            other => panic!("unknown widget kind {other:?}"),
        };
        imports.push(component);
        if w.kind == "basic-alert-dialog" {
            imports.extend(["MaterialStyleMetrics", "MaterialTypography", "TextButton"]);
        }
        if w.kind == "date-picker" || w.kind == "date-range-picker" {
            imports.push("DatePickerDialogContent");
            imports.push("DatePickerDisplayMode");
        }
        if matches!(
            w.kind.as_str(),
            "bottom-sheet" | "bottom-sheet-scaffold" | "modal-bottom-sheet"
        ) {
            imports.push("SheetValue");
        }
        if w.kind == "badged-box" {
            imports.push("Icon");
        }
        if w.kind.starts_with("connected-button") || w.kind == "vertical-connected-button-group" {
            imports.push("ConnectedButtonPosition");
        }

        if w.nav_arrangement.is_some() {
            imports.push("ShortNavigationBarArrangement");
        }
        if w.icon_position.is_some() {
            imports.push("NavigationItemIconPosition");
        }
        if w.icon.is_some()
            || w.trailing_icon.is_some()
            || w.leading_image.is_some()
            || w.checked_icon.is_some()
            || w.nav_icon.is_some()
            || !w.icons.is_empty()
            || w.selected_icon.is_some()
            || w.trailing_icon.is_some()
            || w.kind.ends_with("split-button")
            || item_icons(&w.menu_items).next().is_some()
            || group_item_icons(&w.groups).next().is_some()
            || w.items.iter().any(|item| {
                item.icon.is_some() || item.checked_icon.is_some() || item.selected_icon.is_some()
            })
        {
            needs_icons = true;
        }
        if w.size.is_some() || w.corner.is_some() || w.width_option.is_some() {
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
    if scene
        .widgets
        .iter()
        .any(|w| w.slint_overrides.contains_key("sheet_shape"))
    {
        imports.push("ShapeTokens");
    }
    if scene.widgets.iter().any(|w| w.kind == "material-surface" && w.text.is_some()) {
        imports.push("MaterialText");
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
        // A Tab step only lands on a widget Slint can actually focus:
        // buttons are always focusable; a list item is focusable only when
        // one of its interactive flags makes `is_interactive` true.
        let focusable = match w.kind.as_str() {
            "list-item" | "segmented-list-item" => {
                w.interactive == Some(true)
                    || w.selectable == Some(true)
                    || w.checkable == Some(true)
            }
            "rect" | "surface" => false,
            _ => true,
        };
        // A split button has two focusable halves — the trailing one is the
        // second Tab stop.
        let focusables = if w.kind.ends_with("split-button") { 2 } else { 1 };
        if !focusable || w.enabled == Some(false) {
            // A disabled widget takes no Tab stop on either side, so it
            // does not consume focus ordinals.
            continue;
        }
        if w.state.as_deref() == Some("focused") {
            // Menu widgets take focus through the `menu-focused` prop — the
            // parallel of the Compose side's `FocusInteraction.Focus` on the
            // interaction source — not through FocusScope Tab focus.
            if w.kind.starts_with("menu") {
                ordinal += 1;
                continue;
            }
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
        // A `*-button-group`'s layout isn't focusable — only its items
        // land in the Tab chain (handled below); a dialog's action
        // buttons likewise. Anything else is `focusables` focusables
        // (2 for a split button's halves, else 1).
        if !w.kind.ends_with("button-group") && !w.kind.ends_with("alert-dialog") {
            ordinal += focusables;
        }
        // The dialog's `AlertDialogFlowRow` lands its actions in tree
        // order confirm-first, so the Tab chain walks `items` reversed.
        let items: Box<dyn Iterator<Item = &GroupItem>> = if w.kind == "alert-dialog" {
            Box::new(w.items.iter().rev())
        } else {
            Box::new(w.items.iter())
        };
        for item in items {
            // Disabled items aren't in the Tab chain.
            if item.disabled == Some(true) {
                continue;
            }
            if item.state.as_deref() == Some("focused") {
                // `ordinal` counts the items before this one already.
                for _ in tabs_emitted..=ordinal {
                    actions.push(Action {
                        kind: "key:Tab".into(),
                        x: 0.0,
                        y: 0.0,
                        at: 0.0,
                        velocity: 0.0,
                    });
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
            } else if w.kind == "switch" {
                // A switch's gesture lands on the thumb's resting center:
                // 16px in when unchecked (offset 8 + radius 8), 36px when
                // checked (offset 24 + radius 12).
                w.x + if w.checked == Some(true) { 36.0 } else { 16.0 }
            } else {
                w.x + 20.0
            };
            // A `radio-button`'s drawn control is a 24dp slot — aim at its
            // center, not the button-bucket point.
            let (x, y) = if w.kind == "radio-button" {
                (w.x + 12.0, w.y + 12.0)
            } else {
                (x, w.y + 16.0)
            };
            actions.push(Action { kind: kind.into(), x, y, at: 0.0, velocity: 0.0 });
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

/// Icon stems referenced by a menu `items` list (`icon`, `selected_icon`,
/// `trailing_icon`) — the generator copies the svgs for the Compose side.
fn item_icons(items: &Option<Vec<serde_json::Value>>) -> impl Iterator<Item = &str> {
    items
        .iter()
        .flatten()
        .flat_map(|it| {
            ["icon", "selected_icon", "trailing_icon"]
                .iter()
                .filter_map(|k| it.get(*k).and_then(|v| v.as_str()))
        })
}

/// Icon stems referenced inside a `groups` list's item models.
fn group_item_icons(groups: &Option<Vec<serde_json::Value>>) -> impl Iterator<Item = &str> {
    groups
        .iter()
        .flatten()
        .flat_map(|g| {
            g.get("items")
                .and_then(|v| v.as_array())
                .into_iter()
                .flatten()
                .flat_map(|it| {
                    ["icon", "selected_icon", "trailing_icon"]
                        .iter()
                        .filter_map(|k| it.get(*k).and_then(|v| v.as_str()))
                })
        })
}

/// `standard`/`selectable`/`checkable` → the `MenuItemKind` enum variant.
fn menu_item_kind(kind: Option<&str>) -> &'static str {
    match kind.unwrap_or("standard") {
        "standard" => "MenuItemKind.standard",
        "selectable" => "MenuItemKind.selectable",
        "checkable" => "MenuItemKind.checkable",
        other => panic!("unknown menu item kind {other:?}"),
    }
}

/// `standard`/`vibrant` → the `MenuVariant` enum variant.
fn menu_variant(variant: Option<&str>) -> &'static str {
    match variant.unwrap_or("standard") {
        "standard" => "MenuVariant.standard",
        "vibrant" => "MenuVariant.vibrant",
        other => panic!("unknown menu variant {other:?}"),
    }
}

/// Shape position name → the `MenuShapePosition` enum variant.
fn menu_position(position: Option<&str>) -> &'static str {
    match position.unwrap_or("standalone") {
        "standalone" => "MenuShapePosition.standalone",
        "leading" => "MenuShapePosition.leading",
        "middle" => "MenuShapePosition.middle",
        "trailing" => "MenuShapePosition.trailing",
        other => panic!("unknown menu shape position {other:?}"),
    }
}

/// One menu item model → the `MenuItem` struct literal the `items`/`groups`
/// bindings take.
fn slint_menu_item(it: &serde_json::Value) -> String {
    let mut p = String::from("{ ");
    if let Some(t) = it.get("text").and_then(|v| v.as_str()) {
        write!(p, "text: \"{}\", ", slint_str(t)).unwrap();
    }
    for (field, prop) in [
        ("icon", "icon"),
        ("selected_icon", "selected_icon"),
        ("trailing_icon", "trailing_icon"),
    ] {
        if let Some(i) = it.get(field).and_then(|v| v.as_str()) {
            write!(p, "{prop}: Icons.{i}, ").unwrap();
        }
    }
    if let Some(t) = it.get("trailing_text").and_then(|v| v.as_str()) {
        write!(p, "trailing_text: \"{}\", ", slint_str(t)).unwrap();
    }
    if let Some(t) = it.get("supporting_text").and_then(|v| v.as_str()) {
        write!(p, "supporting_text: \"{}\", ", slint_str(t)).unwrap();
    }
    for (field, prop) in [("selected", "selected"), ("checked", "checked")] {
        if let Some(b) = it.get(field).and_then(|v| v.as_bool()) {
            write!(p, "{prop}: {b}, ").unwrap();
        }
    }
    // The struct models `disabled` — upstream's `enabled = true` default is
    // unexpressible as a Slint struct default.
    if it.get("enabled").and_then(|v| v.as_bool()) == Some(false) {
        p.push_str("disabled: true, ");
    }
    p.push('}');
    p
}

/// `items` JSON → the `.slint` `[MenuItem]` literal.
fn slint_menu_items(items: &Option<Vec<serde_json::Value>>) -> String {
    format!(
        "[{}]",
        items
            .iter()
            .flatten()
            .map(slint_menu_item)
            .collect::<Vec<_>>()
            .join(", ")
    )
}

/// `groups` JSON → the `.slint` `[MenuGroup]` literal.
fn slint_menu_groups(groups: &Option<Vec<serde_json::Value>>) -> String {
    let parts: Vec<String> = groups
        .iter()
        .flatten()
        .map(|g| {
            let label = g.get("label").and_then(|v| v.as_str()).unwrap_or("");
            let items = g.get("items").and_then(|v| v.as_array());
            format!(
                "{{ label: \"{}\", items: {} }}",
                slint_str(label),
                slint_menu_items(&items.cloned())
            )
        })
        .collect();
    format!("[{}]", parts.join(", "))
}

/// Per-item `state` entries → the container's `simulate-*`/focused index
/// bindings. With `grouped` the emitted props carry the `-group` companion
/// the `menu-popup` content takes. Timed scenes skip this — gestures come
/// from `//ACTION=` instead.
fn menu_item_states(s: &mut String, w: &Widget, grouped: bool) {
    let mut collect = |items: &Option<Vec<serde_json::Value>>, group: Option<usize>| {
        for (i, it) in items.iter().flatten().enumerate() {
            let state = it.get("state").and_then(|v| v.as_str());
            let prop = match state {
                Some("hovered") => Some("simulate-hover"),
                Some("pressed") => Some("simulate-press"),
                Some("focused") => Some("focused"),
                Some("enabled") | Some("disabled") | None => None,
                Some(other) => panic!("unknown menu item state {other:?}"),
            };
            if let Some(prop) = prop {
                if let Some(g) = group {
                    if prop == "focused" {
                        writeln!(s, "        focused-group: {g};").unwrap();
                        writeln!(s, "        focused-item: {i};").unwrap();
                    } else {
                        writeln!(s, "        {prop}-group: {g};").unwrap();
                        writeln!(s, "        {prop}-item: {i};").unwrap();
                    }
                } else {
                    let name = if prop == "focused" { "focused-item" } else { &format!("{prop}-item") };
                    writeln!(s, "        {name}: {i};").unwrap();
                }
            }
        }
    };
    if grouped {
        for (g, grp) in w.groups.iter().flatten().enumerate() {
            collect(
                &grp.get("items").and_then(|v| v.as_array()).cloned(),
                Some(g),
            );
        }
    } else {
        collect(&w.menu_items, None);
    }
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
/// `radio-button` props — the 24dp visual slot: `checked` (the upstream
/// `selected`), `enabled`, the `simulate_*` state hooks and the MICS pin
/// off so the scene's coordinates place the drawn control.
fn radio_props(w: &Widget, timed: bool) -> String {
    let mut p = String::new();
    let over = &w.slint_overrides;
    let bool_over = |k: &str, authored: Option<bool>| -> bool {
        over.get(k).and_then(|v| v.as_bool()).unwrap_or(authored.unwrap_or(false))
    };
    if bool_over("checked", w.checked) {
        p.push_str("        checked: true;\n");
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
    if !bool_over("enforce_touch_target", None) {
        p.push_str("        enforce_touch_target: false;\n");
    }
    // A timed scene clicks through a real press+release: flip `checked`
    // so the morph animates — the same flip the Compose side runs in its
    // onRelease hook. A plain `in` property takes the write from the
    // declaring scope here.
    if timed {
        p.push_str("        clicked => { self.checked = !self.checked; }\n");
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

/// `switch` props — the 52x32dp control slot: `checked`, `enabled`, the
/// `simulate_*` states, an `icon` drawn as the thumb content in both
/// states (upstream `thumbContent` is one composable — emitted as both
/// `on_icon` and `off_icon`), and `enforce_touch_target: false` like
/// Compose's `LocalMinimumInteractiveComponentSize provides 0.dp`. The
/// component's own `clicked` flips `in_out checked`, so a scene
/// press+release needs no extra wiring (unlike `RadioButton`, whose
/// `checked` is a pure input).
fn switch_props(w: &Widget, timed: bool) -> String {
    let mut p = String::new();
    let over = &w.slint_overrides;
    let bool_over = |k: &str, authored: Option<bool>| -> bool {
        over.get(k).and_then(|v| v.as_bool()).unwrap_or(authored.unwrap_or(false))
    };
    if w.checked == Some(true) {
        p.push_str("        checked: true;\n");
    }
    if let Some(icon) = &w.icon {
        writeln!(p, "        on_icon: Icons.{icon};").unwrap();
        writeln!(p, "        off_icon: Icons.{icon};").unwrap();
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
    if !bool_over("enforce_touch_target", None) {
        p.push_str("        enforce_touch_target: false;\n");
    }
    p
}

/// The property lines every list-item widget takes — the `ListTile` /
/// `SegmentedListItem` slot contents plus the interaction-state inputs.
/// `slint_overrides` entries shadow the authored values for the negative
/// scenes' deliberate defects.
fn list_props(w: &Widget, timed: bool, segmented: bool) -> String {
    let mut p = String::new();
    let over = &w.slint_overrides;
    let bool_over = |k: &str, authored: Option<bool>| -> bool {
        over.get(k).and_then(|v| v.as_bool()).unwrap_or(authored.unwrap_or(false))
    };
    let str_over = |k: &str, authored: &Option<String>| -> Option<String> {
        over.get(k)
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .or_else(|| authored.clone())
    };
    if let Some(text) = str_over("text", &w.text) {
        writeln!(p, "        text: \"{text}\";").unwrap();
    }
    if let Some(overline) = str_over("overline", &w.overline) {
        writeln!(p, "        overline_text: \"{overline}\";").unwrap();
    }
    if let Some(supporting) = str_over("supporting", &w.supporting) {
        writeln!(p, "        supporting_text: \"{supporting}\";").unwrap();
    }
    if bool_over("supporting_multiline", w.supporting_multiline) {
        p.push_str("        supporting_multiline: true;\n");
    }
    if let Some(icon) = str_over("icon", &w.icon) {
        writeln!(p, "        leading_icon: Icons.{icon};").unwrap();
    }
    if let Some(image) = str_over("leading_image", &w.leading_image) {
        writeln!(p, "        leading_image: Icons.{image};").unwrap();
    }
    if let Some(avatar) = str_over("avatar", &w.avatar) {
        writeln!(p, "        avatar_text: \"{avatar}\";").unwrap();
    }
    if let Some(trailing_icon) = str_over("trailing_icon", &w.trailing_icon) {
        writeln!(p, "        trailing_icon: Icons.{trailing_icon};").unwrap();
    }
    if let Some(trailing_text) = str_over("trailing_text", &w.trailing_text) {
        writeln!(p, "        trailing_text: \"{trailing_text}\";").unwrap();
    }
    if segmented {
        // `slint_overrides.index`/`count` shadow the position — the
        // negative scene's wrong-corners defect.
        let index = over.get("index").and_then(|v| v.as_i64()).or(w.index).unwrap_or(0);
        let count = over.get("count").and_then(|v| v.as_i64()).or(w.count).unwrap_or(1);
        writeln!(p, "        index: {index};\n        count: {count};").unwrap();
    }
    if bool_over("interactive", w.interactive) {
        p.push_str("        interactive: true;\n");
    }
    if bool_over("selectable", w.selectable) {
        p.push_str("        selectable: true;\n");
    }
    if bool_over("checkable", w.checkable) {
        p.push_str("        checkable: true;\n");
    }
    if bool_over("selected", w.selected.as_ref().and_then(|v| v.as_bool())) {
        p.push_str("        selected: true;\n");
    }
    if bool_over("checked", w.checked) {
        p.push_str("        checked: true;\n");
    }
    if bool_over("dragged", w.dragged) {
        p.push_str("        dragged: true;\n");
    }
    if !bool_over("enabled", w.enabled.or(Some(true))) {
        p.push_str("        enabled: false;\n");
    }
    if let Some(width) = w.width {
        writeln!(p, "        width: {width}px;").unwrap();
    }
    if !timed {
        match w.state.as_deref() {
            Some("hovered") => p.push_str("        simulate_hover: true;\n"),
            Some("pressed") => p.push_str("        simulate_press: true;\n"),
            _ => {}
        }
    }
    if !bool_over("enforce_touch_target", None) {
        p.push_str("        enforce_touch_target: false;\n");
    }
    // `ListItem(selected, onClick)` upstream: the host owns the selection —
    // the generated case flips it so a press+release action animates the
    // morph, like `checkable`'s self-toggle.
    if bool_over("selectable", w.selectable) {
        p.push_str("        clicked => {\n            self.selected = !self.selected;\n        }\n");
    }
    p
}

fn slint_canvas(s: &mut String, scene: &Scene) {
    // Buttons are named `button{n}` by count of button-family widgets, not
    // widget index — a backdrop `rect` ahead of a button leaves `button0`
    // intact; list items follow the same rule as `item{n}`.
    // `sheet{n}`/`handle{n}` count sheet-family and handle widgets.
    let mut buttons = 0;
    let mut surfaces = 0;
    let mut items = 0;
    let mut material_surfaces = 0;
    let mut appbars = 0;
    let mut sheets = 0;
    // `handle{n}` counts `drag-handle`s, `vhandle{n}` counts
    // `vertical-drag-handle`s — the Compose mirror numbers each kind alone.
    let mut vhandles = 0usize;
    let mut drags = 0usize;
    let mut groups = 0;
    let mut dialogs = 0;
    let mut pickers = 0;
    let mut icons = 0;
    let mut dividers = 0;
    let mut badges = 0;
    let mut badged_boxes = 0;
    let mut menus = 0;
    let mut nav_bars = 0;
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
            "radio-button" => "RadioButton",
            "switch" => "Switch",
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
            "list-item" | "segmented-list-item" => {
                let i = items;
                items += 1;
                let component = if w.kind == "list-item" { "ListTile" } else { "SegmentedListItem" };
                writeln!(
                    s,
                    "    item{i} := {component} {{\n        x: {}px;\n        y: {}px;\n{}    }}\n",
                    w.x as i64,
                    w.y as i64,
                    list_props(w, !scene.times.is_empty(), w.kind == "segmented-list-item"),
                )
                .unwrap();
                // `container_radius` forwards the live corner morph to the
                // tracer, exactly like the button arm.
                if i == 0 {
                    for prop in &scene.trace_props {
                        let ty = match prop.as_str() {
                            "container_radius" => "length",
                            other => panic!("no forwarding type known for trace prop {other:?}"),
                        };
                        writeln!(s, "    out property <{ty}> {prop}: item{i}.{prop};\n").unwrap();
                    }
                }
                continue;
            }

            "material-surface" => {
                let i = material_surfaces;
                material_surfaces += 1;
                let mut body = String::new();
                // `slint_overrides.color`/`tonal_elevation`/`radius` plant the
                // wrong token on the Slint side — the negative scenes'
                // deliberate defects.
                let color = w
                    .slint_overrides
                    .get("color")
                    .and_then(|v| v.as_str())
                    .map(str::to_string)
                    .or_else(|| w.color.clone());
                if let Some(c) = &color {
                    writeln!(body, "        color: MaterialPalette.{};", c.replace('-', "_")).unwrap();
                }
                let tonal = w
                    .slint_overrides
                    .get("tonal_elevation")
                    .map(|v| widget_num(v))
                    .or(w.tonal_elevation);
                if let Some(t) = tonal {
                    writeln!(body, "        tonal_elevation: {t}px;").unwrap();
                }
                if let Some(p) = w.parent_elevation {
                    writeln!(body, "        parent_absolute_tonal_elevation: {p}px;").unwrap();
                }
                // Compose can't draw `Modifier.shadow` under Paparazzi —
                // scenes keep `elevation` unset (0); the prop is still
                // emitted so a scene can opt in where both sides render.
                if let Some(e) = w.elevation {
                    writeln!(body, "        shadow_elevation: {e}px;").unwrap();
                }
                if let Some(bw) = w.border_width {
                    writeln!(body, "        border_width: {bw}px;").unwrap();
                    writeln!(
                        body,
                        "        border_color: MaterialPalette.{};",
                        w.border_color.as_deref().unwrap_or("outline").replace('-', "_")
                    )
                    .unwrap();
                }
                let radius = w
                    .slint_overrides
                    .get("radius")
                    .map(|v| widget_num(v))
                    .or(w.radius);
                if let Some(r) = radius {
                    writeln!(body, "        border_radius: {r}px;").unwrap();
                }
                if w.clickable.unwrap_or(false) {
                    writeln!(body, "        clickable: true;").unwrap();
                }
                if w.selectable.unwrap_or(false) {
                    writeln!(body, "        selectable: true;").unwrap();
                    if w.selected.as_ref().and_then(|v| v.as_bool()).unwrap_or(false) {
                        writeln!(body, "        selected: true;").unwrap();
                    }
                }
                if w.toggleable.unwrap_or(false) {
                    writeln!(body, "        toggleable: true;").unwrap();
                    if w.checked.unwrap_or(false) {
                        writeln!(body, "        checked: true;").unwrap();
                    }
                }
                if w.enabled == Some(false) {
                    writeln!(body, "        enabled: false;").unwrap();
                }
                writeln!(
                    s,
                    "    msurface{i} := Surface {{\n        x: {}px;\n        y: {}px;\n        width: {}px;\n        height: {}px;\n{body}    }}\n",
                    w.x as i64,
                    w.y as i64,
                    w.width.unwrap() as i64,
                    w.height.unwrap() as i64,
                )
                .unwrap();
                // An optional text child draws in the surface's provided
                // `content_color` — the `LocalContentColor` stand-in.
                if let Some(text) = &w.text {
                    writeln!(
                        s,
                        "    MaterialText {{\n        x: msurface{i}.x + 8px;\n        y: msurface{i}.y + 8px;\n        text: \"{text}\";\n        color: msurface{i}.content_color;\n    }}\n"
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
            "menu" | "menu-popup" | "menu-group" | "menu-item" | "menu-divider"
            | "menu-group-label" => {
                let i = menus;
                menus += 1;
                menu_widget(s, w, i, scene);
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

            "alert-dialog" | "basic-alert-dialog" => {
                let i = dialogs;
                dialogs += 1;
                dialog_widget(s, w, i, scene);
                continue;
            }
            "date-picker" | "date-range-picker" => {
                let i = pickers;
                pickers += 1;
                date_picker_widget(s, w, i);
                continue;
            }
            "icon" => {
                let i = icons;
                icons += 1;
                // `color` maps to upstream's `tint` (the `colorize` prop);
                // `slint_overrides` shadow it for the negative scenes.
                let color = w
                    .slint_overrides
                    .get("color")
                    .and_then(|v| v.as_str())
                    .map(str::to_string)
                    .or_else(|| w.color.clone());
                let colorize_prop = color
                    .map(|c| format!("\n        colorize: MaterialPalette.{};", c.replace('-', "_")))
                    .unwrap_or_default();
                // No `width`/`height` authored → the icon takes the
                // source's natural size (`defaultSizeFor` upstream).
                let size_prop = |v: Option<f64>, name: &str| {
                    v.map(|v| format!("\n        {name}: {v}px;"))
                        .unwrap_or_default()
                };
                let width = w
                    .slint_overrides
                    .get("width")
                    .map(widget_num)
                    .or(w.width);
                let height = w
                    .slint_overrides
                    .get("height")
                    .map(widget_num)
                    .or(w.height);
                let x = w.slint_overrides.get("x").map(widget_num).unwrap_or(w.x);
                let y = w.slint_overrides.get("y").map(widget_num).unwrap_or(w.y);
                writeln!(
                    s,
                    "    icon{i} := Icon {{\n        x: {}px;\n        y: {}px;\n        source: Icons.{};{}{}{}\n    }}\n",
                    x as i64,
                    y as i64,
                    w.icon.as_deref().unwrap_or("check"),
                    size_prop(width, "width"),
                    size_prop(height, "height"),
                    colorize_prop,
                )
                .unwrap();
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
            "badge" => {
                let i = badges;
                badges += 1;
                // `color` is the upstream `containerColor`; the content
                // color follows `contentColorFor` on both sides.
                // `slint_overrides` shadow the authored values for the
                // negative scenes.
                let container = w
                    .slint_overrides
                    .get("color")
                    .and_then(|v| v.as_str())
                    .map(str::to_string)
                    .or_else(|| w.color.clone());
                let container_prop = container
                    .map(|c| format!("\n        container_color: MaterialPalette.{};", c.replace('-', "_")))
                    .unwrap_or_default();
                let text_prop = w
                    .text
                    .as_deref()
                    .map(|t| format!("\n        text: \"{t}\";"))
                    .unwrap_or_default();
                let x = w.slint_overrides.get("x").map(widget_num).unwrap_or(w.x);
                let y = w.slint_overrides.get("y").map(widget_num).unwrap_or(w.y);
                writeln!(
                    s,
                    "    badge{i} := Badge {{\n        x: {}px;\n        y: {}px;{text_prop}{container_prop}\n    }}\n",
                    x as i64,
                    y as i64,
                )
                .unwrap();
                continue;
            }
            "badged-box" => {
                let i = badged_boxes;
                badged_boxes += 1;
                let container = w
                    .slint_overrides
                    .get("color")
                    .and_then(|v| v.as_str())
                    .map(str::to_string)
                    .or_else(|| w.color.clone());
                let container_prop = container
                    .map(|c| format!("\n        badge_container_color: MaterialPalette.{};", c.replace('-', "_")))
                    .unwrap_or_default();
                let badge_text = w
                    .slint_overrides
                    .get("text")
                    .and_then(|v| v.as_str())
                    .map(str::to_string)
                    .or_else(|| w.text.clone());
                let text_prop = badge_text
                    .as_deref()
                    .map(|t| format!("\n        badge_text: \"{t}\";"))
                    .unwrap_or_default();
                writeln!(
                    s,
                    "    badged_box{i} := BadgedBox {{\n        x: {}px;\n        y: {}px;{text_prop}{container_prop}\n\n        // The upstream demos anchor to a 24dp icon (the badge hangs off\n        // the anchor's measured bounds); the Compose side draws\n        // `Icon(sceneIcon)` at its intrinsic 24dp.\n        Icon {{\n            width: 24px;\n            height: 24px;\n            source: Icons.{};\n        }}\n    }}\n",
                    w.x as i64,
                    w.y as i64,
                    w.icon.as_deref().unwrap_or("check"),
                )
                .unwrap();
                continue;
            }
            "navigation-bar" | "short-navigation-bar" => {
                let i = nav_bars;
                nav_bars += 1;
                navbar_widget(s, w, i);
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
            } else if w.kind == "radio-button" {
                radio_props(w, !scene.times.is_empty())
            } else if w.kind == "switch" {
                switch_props(w, !scene.times.is_empty())
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
    sheet_trace_forwards(s, scene);
    sheet_op_timers(s, scene);

    // `nav_events` drive a selection change on `navbar{i}` after the baseline
    // frame: each `["select", bar, item, ms]` becomes a letter-key `//ACTION=`
    // dispatched at `ms`, and the FocusScope below maps the letter back to a
    // `current-index` write — a real key event on the Slint mock clock,
    // matching the Compose side's `emitPress` runnable that flips the bar's
    // `selectedIndex` state.
    if let Some(events) = scene.params.get("nav_events").and_then(|v| v.as_array()) {
        let mut keys = String::new();
        for (e, ev) in events.iter().enumerate() {
            assert_eq!(ev[0].as_str().unwrap(), "select", "unknown nav event {ev:?}");
            let key = (b'a' + e as u8) as char;
            let bar = ev[1].as_i64().unwrap();
            let item = ev[2].as_i64().unwrap();
            let at = ev[3].as_i64().unwrap();
            writeln!(s, "    //ACTION=key@{at}:{key}").unwrap();
            writeln!(
                keys,
                "            if event.text == \"{key}\" {{ navbar{bar}.current-index = {item}; }}"
            )
            .unwrap();
        }
        writeln!(
            s,
            "\n    forward-focus: fs;\n    fs := FocusScope {{\n        key-pressed(event) => {{\n{keys}            accept\n        }}\n    }}"
        )
        .unwrap();
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
        | "thumb_size"
        | "thumb_offset"
        | "shadow_elevation"
        | "dot_radius" => "length",
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

/// One menu-family widget (`menu`, `menu-popup`, `menu-group`, `menu-item`,
/// `menu-divider`, `menu-group-label`). Elements are named `menu{n}` in
/// scene order; the standalone kinds (`menu-item`, `menu-group`,
/// `menu-divider`, `menu-group-label`) render a single composable, the
/// container kinds render a menu `items`/`groups` model.
fn menu_widget(s: &mut String, w: &Widget, i: usize, scene: &Scene) {
    let over = &w.slint_overrides;
    let str_over = |k: &str, authored: Option<&str>| -> Option<String> {
        over.get(k).and_then(|v| v.as_str()).map(str::to_string).or_else(|| authored.map(str::to_string))
    };
    let variant = menu_variant(str_over("variant", w.variant.as_deref()).as_deref());
    let item_kind = menu_item_kind(str_over("item_kind", w.item_kind.as_deref()).as_deref());
    let position = menu_position(str_over("shape_position", w.shape_position.as_deref()).as_deref());
    let mut p = String::new();
    let x = w.x as i64;
    let y = w.y as i64;
    match w.kind.as_str() {
        "menu" => {
            writeln!(
                p,
                "        items: {};\n        item-kind: {item_kind};\n        variant: {variant};\n        // The references cannot render platform shadows (layoutlib\n        // deadlocks on `Surface.shadowElevation`).\n        cast-shadow: false;",
                slint_menu_items(&w.menu_items),
            )
            .unwrap();
            if let Some(fi) = over.get("first_index").map(widget_num).map(|v| v as i64).or(w.first_index) {
                writeln!(p, "        first-index: {fi};").unwrap();
            }
            if let Some(width) = w.width {
                writeln!(p, "        width: {width}px;").unwrap();
            }
            menu_item_states(&mut p, w, false);
            writeln!(
                s,
                "    menu{i} := MenuInner {{\n        x: {x}px;\n        y: {y}px;\n{p}    }}\n",
            )
            .unwrap();
        }
        "menu-popup" => {
            writeln!(
                p,
                "        groups: {};\n        item-kind: {item_kind};\n        variant: {variant};\n        // The references cannot render platform shadows (layoutlib\n        // deadlocks on `Surface.shadowElevation`).\n        cast-shadow: false;",
                slint_menu_groups(&w.groups),
            )
            .unwrap();
            if let Some(width) = w.width {
                writeln!(p, "        width: {width}px;").unwrap();
            }
            menu_item_states(&mut p, w, true);
            writeln!(
                s,
                "    menu{i} := MenuPopupContent {{\n        x: {x}px;\n        y: {y}px;\n{p}    }}\n",
            )
            .unwrap();
        }
        "menu-group" => {
            if let Some(label) = str_over("label", w.label.as_deref()) {
                writeln!(p, "        label: \"{}\";", slint_str(&label)).unwrap();
            }
            if let Some(width) = w.width {
                writeln!(p, "        width: {width}px;").unwrap();
            }
            writeln!(
                p,
                "        items: {};\n        item-kind: {item_kind};\n        variant: {variant};\n        shape-position: {position};\n        // The references cannot render platform shadows (layoutlib\n        // deadlocks on `Surface.shadowElevation`).\n        cast-shadow: false;",
                slint_menu_items(&w.menu_items),
            )
            .unwrap();
            if w.state.as_deref() == Some("hovered") {
                p.push_str("        simulate-hover: true;\n");
            }
            menu_item_states(&mut p, w, false);
            writeln!(
                s,
                "    menu{i} := MenuGroupContent {{\n        x: {x}px;\n        y: {y}px;\n{p}    }}\n",
            )
            .unwrap();
        }
        "menu-item" => {
            if let Some(width) = w.width {
                writeln!(p, "        width: {width}px;").unwrap();
            }
            let over_icon = |k: &str, authored: Option<&String>| -> Option<String> {
                over.get(k).and_then(|v| v.as_str()).map(str::to_string).or_else(|| authored.cloned())
            };
            if let Some(t) = str_over("text", w.text.as_deref()) {
                writeln!(p, "        text: \"{}\";", slint_str(&t)).unwrap();
            }
            if let Some(t) = str_over("supporting_text", w.supporting_text.as_deref()) {
                writeln!(p, "        supporting-text: \"{}\";", slint_str(&t)).unwrap();
            }
            if let Some(t) = str_over("trailing_text", w.trailing_text.as_deref()) {
                writeln!(p, "        trailing-text: \"{}\";", slint_str(&t)).unwrap();
            }
            if let Some(ic) = over_icon("icon", w.icon.as_ref()) {
                writeln!(p, "        icon: Icons.{ic};").unwrap();
            }
            if let Some(ic) = over_icon("selected_icon", w.selected_icon.as_ref()) {
                writeln!(p, "        selected-icon: Icons.{ic};").unwrap();
            }
            if let Some(ic) = over_icon("trailing_icon", w.trailing_icon.as_ref()) {
                writeln!(p, "        trailing-icon: Icons.{ic};").unwrap();
            }
            let bool_of = |k: &str, authored: Option<bool>| -> bool {
                over.get(k).and_then(|v| v.as_bool()).unwrap_or(authored.unwrap_or(false))
            };
            if bool_of("selected", w.selected.as_ref().and_then(|v| v.as_bool())) {
                p.push_str("        selected: true;\n");
            }
            if bool_of("checked", w.checked) {
                p.push_str("        checked: true;\n");
            }
            if !bool_of("enabled", w.enabled.or(Some(true))) {
                p.push_str("        enabled: false;\n");
            }
            writeln!(
                p,
                "        item-kind: {item_kind};\n        variant: {variant};\n        shape-position: {position};",
            )
            .unwrap();
            if !scene.times.is_empty() {
                // Timed scenes drive the state through `//ACTION=` gestures.
            } else {
                match w.state.as_deref() {
                    Some("hovered") => p.push_str("        simulate-hover: true;\n"),
                    Some("pressed") => p.push_str("        simulate-press: true;\n"),
                    Some("focused") => {
                        if bool_of("menu_focused", Some(true)) {
                            p.push_str("        menu-focused: true;\n")
                        }
                    }
                    _ => {}
                }
            }
            writeln!(
                s,
                "    menu{i} := MenuItemContent {{\n        x: {x}px;\n        y: {y}px;\n{p}    }}\n",
            )
            .unwrap();
        }
        "menu-divider" => {
            if let Some(width) = w.width {
                writeln!(p, "        width: {width}px;").unwrap();
            }
            writeln!(
                s,
                "    menu{i} := MenuDivider {{\n        x: {x}px;\n        y: {y}px;\n{p}    }}\n",
            )
            .unwrap();
        }
        "menu-group-label" => {
            if let Some(t) = str_over("text", w.text.as_deref()) {
                writeln!(p, "        text: \"{}\";", slint_str(&t)).unwrap();
            }
            writeln!(
                s,
                "    menu{i} := MenuGroupLabel {{\n        x: {x}px;\n        y: {y}px;\n{p}    }}\n",
            )
            .unwrap();
        }
        other => panic!("unknown menu widget kind {other:?}"),
    }
    // `container_radius` traces the shape morph — forwarded from the widget's
    // top-left corner (uniform for the standalone positions the scenes use).
    if scene.trace_props.iter().any(|p| p == "container_radius") && i == 0 {
        match w.kind.as_str() {
            "menu-item" | "menu-group" => writeln!(
                s,
                "    out property <length> container_radius: menu{i}.container-radius-top-left;\n",
            )
            .unwrap(),
            _ => {}
        }
    }
    // `slint_overrides.cover` paints a rectangle over the widget — the
    // negative scenes' deliberate defect.
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
        let xo = cover["x"].as_f64().unwrap_or(0.0) as i64;
        let yo = cover["y"].as_f64().unwrap_or(0.0) as i64;
        let cw = cover["width"].as_f64();
        let ch = cover["height"].as_f64();
        let w_expr = cw.map(|v| format!("{v}px")).unwrap_or_else(|| format!("menu{i}.width"));
        let h_expr = ch.map(|v| format!("{v}px")).unwrap_or_else(|| format!("menu{i}.height"));
        writeln!(
            s,
            "    // Deliberate defect (scene `slint_overrides.cover`).\n    Rectangle {{\n        x: menu{i}.x + {xo}px;\n        y: menu{i}.y + {yo}px;\n        width: {w_expr};\n        height: {h_expr};\n        border-radius: {cover_radius};\n        background: {fill_expr};\n        opacity: {};\n    }}\n",
            cover["opacity"].as_f64().unwrap_or(1.0),
        )
        .unwrap();
    }
}

/// Escapes a string for embedding in a generated `.slint` string literal.
fn slint_str(text: &str) -> String {
    text.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', "\\n")
}

/// An `alert-dialog`/`basic-alert-dialog` — the scrim `Modal` paints when the
/// popup opens (`background_modal`), plus the centered content inline. The
/// recorded Compose side draws the same inline: `Dialog` opens a platform
/// window and `Surface` shadows deadlock layoutlib, so the scene content is
/// `cast_shadow: false`. Widget `x`/`y` are ignored — the pane centers like
/// the dialog window.
fn dialog_widget(s: &mut String, w: &Widget, i: usize, scene: &Scene) {
    let over = &w.slint_overrides;
    // `Modal`'s scrim — `ScrimTokens.container_opacity` (0.32) via
    // `background_modal`.
    writeln!(
        s,
        "    scrim{i} := Rectangle {{\n        x: 0px;\n        y: 0px;\n        width: 100%;\n        height: 100%;\n        background: MaterialPalette.background_modal;\n    }}\n",
    )
    .unwrap();

    if w.kind == "basic-alert-dialog" {
        // `BasicAlertDialog` hands `content` only the `sizeIn` clamp — the
        // pane chrome here is the caller's own, like the canonical sample's.
        let mut content = String::new();
        if let Some(title) = &w.title {
            // The pane title is content-sized (`root.width` is the scene
            // width — outside any layout chain, so no binding loop).
            writeln!(
                content,
                "            MaterialText {{\n                text: \"{}\";\n                style: MaterialTypography.headline_small;\n                color: MaterialPalette.on_surface;\n                wrap: word_wrap;\n                width: min(self.preferred-width, root.width - 48px);\n            }}\n",
                slint_str(title),
            )
            .unwrap();
        }
        if !w.items.is_empty() {
            let labels = w
                .items
                .iter()
                .map(|item| {
                    format!("\"{}\"", slint_str(item.text.as_deref().unwrap_or_default()))
                })
                .collect::<Vec<_>>()
                .join(", ");
            // The repeated `action` id is what `//TRACE_ITEMS=` enumerates.
            let hover = w
                .items
                .iter()
                .position(|it| it.state.as_deref() == Some("hovered"));
            let press = w
                .items
                .iter()
                .position(|it| it.state.as_deref() == Some("pressed"));
            content.push_str("            // align(End) on the actions box\n            HorizontalLayout {\n                alignment: end;\n                spacing: 8px;\n");
            writeln!(
                content,
                "                for action_text[index] in [{labels}] : action := TextButton {{\n                    text: action_text;\n                    enforce_touch_target: false;\n                    // The mirror renders the stable `TextButton` overload.\n                    expressive: false;\n{hover}{press}                }}\n",
                hover = hover
                    .map(|i| format!("                    simulate_hover: index == {i};\n"))
                    .unwrap_or_default(),
                press = press
                    .map(|i| format!("                    simulate_press: index == {i};\n"))
                    .unwrap_or_default(),
            )
            .unwrap();
            content.push_str("            }\n");
        }
        let title_lit = w
            .title
            .as_ref()
            .map(|t| format!("\"{}\"", slint_str(t)))
            .unwrap_or_else(|| "\"\"".to_string());
        writeln!(
            s,
            "    basic{i} := Rectangle {{\n        x: (parent.width - self.width) / 2;\n        y: (parent.height - self.height) / 2;\n        // The Compose `sizeIn` clamp grows the pane to the widest child's\n        // preferred width; the wrapping title under-measures as min-width,\n        // so `measure{i}` (invisible twin) supplies the preferred term.\n        measure{i} := MaterialText {{\n            visible: false;\n            text: {title_lit};\n            style: MaterialTypography.headline_small;\n        }}\n        width: min(max(280px, inner.min_width, measure{i}.preferred-width + 48px), min(560px, parent.width));\n        height: inner.min_height;\n        border-radius: MaterialStyleMetrics.border_radius_28;\n        background: MaterialPalette.surface_container_high;\n        clip: true;\n        inner := VerticalLayout {{\n            padding: 24px;\n            spacing: 24px;\n{content}        }}\n    }}\n",
        )
        .unwrap();
        return;
    }

    let mut p = String::new();
    p.push_str("        cast_shadow: false;\n");
    // `LocalMinimumInteractiveComponentSize` is 0 on the Compose side —
    // the action buttons drop their 48dp touch padding (upstream's
    // crossAxis math then supplies the 8dp gap itself).
    p.push_str("        action_enforce_touch_target: false;\n");
    if let Some(title) = &w.title {
        writeln!(p, "        title: \"{}\";", slint_str(title)).unwrap();
    }
    if let Some(icon) = &w.icon {
        writeln!(p, "        icon: Icons.{icon};").unwrap();
    }
    if let Some(text) = &w.text {
        writeln!(p, "        text: \"{}\";", slint_str(text)).unwrap();
    }
    // `actions` is display order — dismiss first, confirm last (the
    // component's flipped FlowRow lands the confirm rightmost/on top).
    // `slint_overrides.actions_reversed` emits the array backwards: the
    // negative scene's forgot-the-flip defect.
    let reversed = over.get("actions_reversed").and_then(|v| v.as_bool()).unwrap_or(false);
    let mut items: Vec<&GroupItem> = w.items.iter().collect();
    if reversed {
        items.reverse();
    }
    let actions = items
        .iter()
        .map(|item| format!("\"{}\"", slint_str(item.text.as_deref().unwrap_or_default())))
        .collect::<Vec<_>>()
        .join(", ");
    writeln!(p, "        actions: [{actions}];").unwrap();
    // A `state` on `items[i]` drives `MaterialButtonBase.simulate_*` on the
    // `actions[i]` button (static scenes); `focused` gets `key:Tab` steps
    // from `widget_actions` instead.
    if scene.times.is_empty() {
        for (index, item) in w.items.iter().enumerate() {
            // The override's swapped array moves the state index with it.
            let action_index = if reversed { w.items.len() - 1 - index } else { index };
            match item.state.as_deref() {
                Some("hovered") => {
                    writeln!(p, "        action_simulate_hover: {action_index};").unwrap()
                }
                Some("pressed") => {
                    writeln!(p, "        action_simulate_press: {action_index};").unwrap()
                }
                _ => {}
            }
        }
    }
    // `slint_overrides` defects for `negative` scenes — token overrides the
    // component exposes (`corner` widens the whole shape to a fixed radius).
    if let Some(corner) = over.get("corner").and_then(|v| v.as_f64()) {
        writeln!(
            p,
            "        container_shape: {{ top_left: {corner}px, top_right: {corner}px, bottom_right: {corner}px, bottom_left: {corner}px, full: false }};",
        )
        .unwrap();
    }
    if let Some(fill) = over.get("container").and_then(|v| v.as_str()) {
        writeln!(p, "        container_color: MaterialPalette.{};", fill.replace('-', "_"))
            .unwrap();
    }
    for (k, prop) in [("padding", "content_padding"), ("actions_spacing", "actions_spacing")] {
        if let Some(v) = over.get(k).and_then(|v| v.as_f64()) {
            writeln!(p, "        {prop}: {v}px;").unwrap();
        }
    }
    writeln!(
        s,
        "    dialog{i} := AlertDialogContent {{\n        x: (parent.width - self.width) / 2;\n        y: (parent.height - self.height) / 2;\n        available_width: parent.width;\n{p}    }}\n",
    )
    .unwrap();
}


/// ISO `YYYY-MM-DD`/`YYYY-MM` → a `Date` struct literal for the emitted
/// `.slint` — `{ day: D, month: M, year: Y }`.
fn slint_date_literal(iso: &str) -> String {
    let mut it = iso.split('-');
    let year: i64 = it.next().unwrap().parse().unwrap();
    let month: i64 = it.next().unwrap().parse().unwrap();
    let day: i64 = it.next().map(|d| d.parse().unwrap()).unwrap_or(1);
    format!("{{ day: {day}, month: {month}, year: {year} }}")
}

/// A `date-picker`/`date-range-picker` inside `DatePickerDialogContent` —
/// `DatePickerDialog` inline: the `Modal` scrim plus the centered 360dp
/// pane capped at `ContainerHeight` (568), mirrored by
/// `StateDatePickerDialog`.
fn date_picker_widget(s: &mut String, w: &Widget, i: usize) {
    let over = &w.slint_overrides;
    writeln!(
        s,
        "    scrim{i} := Rectangle {{\n        x: 0px;\n        y: 0px;\n        width: 100%;\n        height: 100%;\n        background: MaterialPalette.background_modal;\n    }}\n",
    )
    .unwrap();

    let mut p = String::new();
    // `LocalMinimumInteractiveComponentSize` is 0 on the Compose side.
    p.push_str("        enforce_touch_target: false;\n");
    writeln!(p, "        confirm_enabled: {};", w.confirm_enabled.unwrap_or(true)).unwrap();
    // `slint_overrides` defects for `negative` scenes.
    if let Some(corner) = over.get("corner").and_then(|v| v.as_f64()) {
        writeln!(
            p,
            "        container_shape: {{ top_left: {corner}px, top_right: {corner}px, bottom_right: {corner}px, bottom_left: {corner}px, full: false }};",
        )
        .unwrap();
    }
    if let Some(fill) = over.get("container").and_then(|v| v.as_str()) {
        writeln!(p, "        container_color: MaterialPalette.{};", fill.replace('-', "_"))
            .unwrap();
    }
    if let Some(v) = over.get("actions_spacing").and_then(|v| v.as_f64()) {
        writeln!(p, "        actions_spacing: {v}px;").unwrap();
    }

    let mut inner = String::new();
    let component = if w.kind == "date-range-picker" { "DateRangePicker" } else { "DatePicker" };
    let display = match w.display_mode.as_deref() {
        Some("input") => "DatePickerDisplayMode.input",
        _ => "DatePickerDisplayMode.picker",
    };
    writeln!(inner, "            display_mode: {display};").unwrap();
    if let Some(t) = &w.title {
        writeln!(inner, "            title: \"{}\";", slint_str(t)).unwrap();
    }
    if let Some(v) = w.show_mode_toggle {
        writeln!(inner, "            show_mode_toggle: {v};").unwrap();
    }
    inner.push_str("            enforce_touch_target: false;\n");
    if let Some(d) = &w.displayed {
        writeln!(inner, "            displayed_date: {};", slint_date_literal(d)).unwrap();
    }
    if w.kind == "date-picker" {
        if let Some(d) = w.selected.as_ref().and_then(|v| v.as_str()) {
            writeln!(inner, "            selected_date: {};", slint_date_literal(d)).unwrap();
        }
        if let Some(v) = w.year_min {
            writeln!(inner, "            year_min: {v};").unwrap();
        }
        if let Some(v) = w.year_max {
            writeln!(inner, "            year_max: {v};").unwrap();
        }
    } else {
        if let Some(d) = &w.selected_start {
            writeln!(inner, "            selected_start_date: {};", slint_date_literal(d))
                .unwrap();
        }
        if let Some(d) = &w.selected_end {
            writeln!(inner, "            selected_end_date: {};", slint_date_literal(d)).unwrap();
        }
        if let Some(v) = w.months_to_show {
            writeln!(inner, "            months_to_show: {v};").unwrap();
        }
    }
    if let Some(d) = &w.selectable_from {
        writeln!(inner, "            selectable_from: {};", slint_date_literal(d)).unwrap();
    }
    if let Some(d) = &w.selectable_to {
        writeln!(inner, "            selectable_to: {};", slint_date_literal(d)).unwrap();
    }
    if let Some(fill) = over.get("selected_day_container").and_then(|v| v.as_str()) {
        writeln!(
            inner,
            "            selected_day_container_color: MaterialPalette.{};",
            fill.replace('-', "_"),
        )
        .unwrap();
    }
    if w.kind == "date-range-picker" {
        if let Some(fill) = over.get("range_band").and_then(|v| v.as_str()) {
            writeln!(
                inner,
                "            day_in_range_container_color: MaterialPalette.{};",
                fill.replace('-', "_"),
            )
            .unwrap();
        }
    }

            // Compose lays out in physical pixels: `Center` snaps an odd
        // (size - content) delta to an integer *device* pixel. `phx`
        // rounds the same quantity in the same space - logical `px`
        // rounding would be a half-pixel off at density 1 and a full one
        // at density 2.
writeln!(
        s,
        "    picker{i} := DatePickerDialogContent {{\n        x: Math.round((parent.width - self.width) / 1phx / 2) * 1phx;\n        y: Math.round((parent.height - self.height) / 1phx / 2) * 1phx;\n{p}\n        {component} {{\n{inner}        }}\n    }}\n",
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
                "    sheet{i}_box := Rectangle {{\n        x: {}px;\n        y: {}px;\n        width: {}px;\n        height: {}px;\n\n        sheet{i}_comp := BottomSheet {{\n            x: 0px;\n            y: 0px;\n            width: 100%;\n            height: 100%;\n            initial-value: SheetValue.{};\n            skip-partially-expanded: {};\n            gestures-enabled: {};{}{}\n{}        }}\n\n        sheet{i} := Rectangle {{{}}}\n\n        // The drag-handle pill: 32x4 dp centered in the sheet's 48 dp\n        // handle strip — `SheetDefaults.kt`'s DragHandleVerticalPadding\n        // is 22 dp, so the pill sits 22 dp below the surface's top.\n        sheet-pill{i} := Rectangle {{\n            x: sheet{i}_comp.surface-x + (sheet{i}_comp.surface-width - 32px) / 2;\n            y: sheet{i}_comp.surface-y + 22px;\n            width: 32px;\n            height: 4px;\n        }}\n\n        // The moving surface — `sheet{i}` stays on the static widget\n        // container, so `mask_decor` against a mid-flight edge needs the\n        // surface's own bounds.\n        sheet-surface{i} := Rectangle {{\n            x: sheet{i}_comp.surface-x;\n            y: sheet{i}_comp.surface-y;\n            width: sheet{i}_comp.surface-width;\n            height: sheet{i}_comp.sheet-height;\n        }}\n\n        // The content region below the 48 dp handle strip — the\n        // handle/content boundary is painted inside the surface, so it\n        // needs its own element for `mask_decor` to reach it.\n        sheet-content{i} := Rectangle {{\n            x: sheet{i}_comp.surface-x;\n            y: sheet{i}_comp.surface-y + 48px;\n            width: sheet{i}_comp.surface-width;\n            height: sheet{i}_comp.sheet-height - 48px;\n        }}\n    }}\n",
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
                "    sheet{i}_box := Rectangle {{\n        x: {}px;\n        y: {}px;\n        width: {}px;\n        height: {}px;\n\n        Rectangle {{\n            x: 0px;\n            y: 0px;\n            width: 100%;\n            height: 100%;\n            background: MaterialPalette.{body_color};\n        }}\n\n        sheet{i}_comp := BottomSheetScaffold {{\n            x: 0px;\n            y: 0px;\n            width: 100%;\n            height: 100%;\n            initial-value: SheetValue.{};\n            sheet-peek-height: {}px;\n            skip-hidden-state: {};\n            sheet-swipe-enabled: {};{}{}\n{}        }}\n\n        sheet{i} := Rectangle {{{}}}\n\n        // The drag-handle pill: 32x4 dp centered in the sheet's 48 dp\n        // handle strip — `SheetDefaults.kt`'s DragHandleVerticalPadding\n        // is 22 dp, so the pill sits 22 dp below the surface's top.\n        sheet-pill{i} := Rectangle {{\n            x: sheet{i}_comp.surface-x + (sheet{i}_comp.surface-width - 32px) / 2;\n            y: sheet{i}_comp.surface-y + 22px;\n            width: 32px;\n            height: 4px;\n        }}\n\n        // The moving surface — `sheet{i}` stays on the static widget\n        // container, so `mask_decor` against a mid-flight edge needs the\n        // surface's own bounds.\n        sheet-surface{i} := Rectangle {{\n            x: sheet{i}_comp.surface-x;\n            y: sheet{i}_comp.surface-y;\n            width: sheet{i}_comp.surface-width;\n            height: sheet{i}_comp.sheet-height;\n        }}\n\n        // The content region below the 48 dp handle strip — the\n        // handle/content boundary is painted inside the surface, so it\n        // needs its own element for `mask_decor` to reach it.\n        sheet-content{i} := Rectangle {{\n            x: sheet{i}_comp.surface-x;\n            y: sheet{i}_comp.surface-y + 48px;\n            width: sheet{i}_comp.surface-width;\n            height: sheet{i}_comp.sheet-height - 48px;\n        }}\n    }}\n",
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

/// One navigation-bar family widget (`navigation-bar`, the 80dp tall bar;
/// `short-navigation-bar`, the 64dp expressive bar). Elements are named
/// `navbar{n}` in scene order.
///
/// `nav_arrangement` selects `ShortNavigationBarArrangement` (`equal-weight`
/// default, `centered`), `icon_position` `NavigationItemIconPosition`
/// (`top` default, `start`), `always_show_label` the tall bar's
/// `NavigationBarItem.alwaysShowLabel`, and `selected_index` the bar's
/// `current-index`.
///
/// The Compose harness clears `LocalMinimumInteractiveComponentSize` to
/// `0.dp`; `item-min-size: 0px` mirrors that on the Slint side.
fn navbar_widget(s: &mut String, w: &Widget, i: usize) {
    let component = match w.kind.as_str() {
        "navigation-bar" => "NavigationBar",
        "short-navigation-bar" => "ShortNavigationBar",
        other => panic!("unknown navigation-bar kind {other:?}"),
    };
    let mut p = String::new();
    writeln!(p, "        x: {}px;\n        y: {}px;", w.x as i64, w.y as i64).unwrap();
    if let Some(width) = w.width {
        writeln!(p, "        width: {width}px;").unwrap();
    }
    if w.kind == "short-navigation-bar" {
        let arrangement = w
            .slint_overrides
            .get("nav_arrangement")
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .or_else(|| w.nav_arrangement.clone());
        if let Some(a) = arrangement {
            writeln!(
                p,
                "        arrangement: ShortNavigationBarArrangement.{};",
                a.replace('-', "_")
            )
            .unwrap();
        }
        let icon_position = w
            .slint_overrides
            .get("icon_position")
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .or_else(|| w.icon_position.clone());
        if let Some(pos) = icon_position {
            writeln!(p, "        icon-position: NavigationItemIconPosition.{pos};").unwrap();
        }
        p.push_str("        item-min-size: 0px;\n");
    } else {
        let always = w
            .slint_overrides
            .get("always_show_label")
            .and_then(|v| v.as_bool())
            .or(w.always_show_label);
        if let Some(v) = always {
            writeln!(p, "        always-show-label: {v};").unwrap();
        }
    }
    let selected = w
        .slint_overrides
        .get("selected_index")
        .and_then(|v| v.as_i64())
        .or(w.selected_index);
    if let Some(v) = selected {
        writeln!(p, "        current-index: {v};").unwrap();
    }
    if !w.items.is_empty() {
        writeln!(p, "        items: [").unwrap();
        for item in &w.items {
            let mut entry = String::new();
            if let Some(icon) = &item.icon {
                entry.push_str(&format!("icon: Icons.{icon}, "));
            }
            if let Some(icon) = &item.selected_icon {
                entry.push_str(&format!("selected-icon: Icons.{icon}, "));
            }
            entry.push_str(&format!("text: {:?}", item.text.as_deref().unwrap_or_default()));
            if let Some(b) = &item.badge {
                entry.push_str(&format!(", badge: {b:?}, show-badge: true"));
            }
            // `NavigationItem.enabled` is a plain bool — unset means `false`
            // in Slint (the Compose `RailItem` defaults it to `true`), so it
            // must be emitted unconditionally.
            entry.push_str(&format!(", enabled: {}", item.enabled.unwrap_or(true)));
            writeln!(p, "            {{ {entry} }},").unwrap();
        }
        writeln!(p, "        ];").unwrap();
    }
    writeln!(
        s,
        "    navbar{i} := {component} {{\n{p}    }}\n",
    )
    .unwrap();
}
