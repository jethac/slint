// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

//! PARITY.md generation and the `PARITY_STATUS.json` merge.
//!
//! Every token object maps to a component family through `family_of()` and
//! every non-token Kotlin source file is classified through
//! `source_family()`. An unmapped name is a hard error: new upstream files
//! must be consciously placed, never silently dropped.

use crate::emit::snake_case;
use crate::json::{self, Json};
use crate::model::{Library, ResolvedObject, Value};
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

/// Per-component progress, edited by hand in PARITY_STATUS.json.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ComponentStatus {
    /// "missing" | "partial" | "done"
    pub status: String,
    /// Tracking issue numbers covering this component.
    pub issues: Vec<u32>,
    /// Engine capability this component needs (free text).
    pub needs: String,
    /// Free-form note.
    pub notes: String,
}

#[derive(Debug, Default)]
pub struct Status {
    /// component name -> status
    pub components: BTreeMap<String, ComponentStatus>,
}

/// Map a token object name to its component family. Every non-category
/// `*Tokens` object must appear here.
pub fn family_of(object: &str) -> Option<&'static str> {
    Some(match object {
        "AppBarTokens"
        | "AppBarSmallTokens"
        | "AppBarMediumTokens"
        | "AppBarMediumFlexibleTokens"
        | "AppBarLargeTokens"
        | "AppBarLargeFlexibleTokens" => "App bars",
        "FilledAutocompleteTokens" | "OutlinedAutocompleteTokens" => "Autocomplete",
        "BadgeTokens" => "Badge",
        "BottomAppBarTokens" => "Bottom app bar",
        "SheetBottomTokens" | "DragHandleTokens" => "Bottom sheets",
        "BaselineButtonTokens"
        | "ButtonSmallTokens"
        | "ButtonMediumTokens"
        | "ButtonLargeTokens"
        | "ButtonXSmallTokens"
        | "ButtonXLargeTokens"
        | "ElevatedButtonTokens"
        | "FilledButtonTokens"
        | "FilledTonalButtonTokens"
        | "OutlinedButtonTokens"
        | "TextButtonTokens"
        | "TonalButtonTokens" => "Buttons",
        "ButtonGroupSmallTokens" | "ConnectedButtonGroupSmallTokens" => "Button groups",
        "ElevatedCardTokens" | "FilledCardTokens" | "OutlinedCardTokens" => "Cards",
        "CheckboxTokens" => "Checkbox",
        "AssistChipTokens"
        | "ChipsTokens"
        | "FilterChipTokens"
        | "InputChipTokens"
        | "SuggestionChipTokens" => "Chips",
        "DateInputModalTokens" | "DatePickerModalTokens" => "Date pickers",
        "DialogTokens" => "Dialogs",
        "DividerTokens" => "Divider",
        "ExtendedFabSmallTokens"
        | "ExtendedFabMediumTokens"
        | "ExtendedFabLargeTokens"
        | "ExtendedFabPrimaryTokens"
        | "FabBaselineTokens"
        | "FabSmallTokens"
        | "FabMediumTokens"
        | "FabLargeTokens"
        | "FabPrimaryContainerTokens"
        | "FabSecondaryContainerTokens" => "FAB",
        "FabMenuBaselineTokens" => "FAB menu",
        "IconButtonTokens"
        | "SmallIconButtonTokens"
        | "MediumIconButtonTokens"
        | "LargeIconButtonTokens"
        | "XSmallIconButtonTokens"
        | "XLargeIconButtonTokens"
        | "FilledIconButtonTokens"
        | "FilledTonalIconButtonTokens"
        | "OutlinedIconButtonTokens" => "Icon buttons",
        "ListTokens" | "ExpandedListTokens" | "ReorderListTokens" | "RevealListTokens" => "Lists",
        "LoadingIndicatorTokens" => "Loading indicator",
        "MenuTokens" | "StandardMenuTokens" | "VibrantMenuTokens" | "SegmentedMenuTokens" => {
            "Menus"
        }
        "NavigationBarTokens"
        | "NavigationBarHorizontalItemTokens"
        | "NavigationBarVerticalItemTokens" => "Navigation bar",
        "NavigationDrawerTokens" => "Navigation drawer",
        "NavigationRailBaselineItemTokens"
        | "NavigationRailCollapsedTokens"
        | "NavigationRailColorTokens"
        | "NavigationRailExpandedTokens"
        | "NavigationRailHorizontalItemTokens"
        | "NavigationRailVerticalItemTokens" => "Navigation rail",
        "CircularProgressIndicatorTokens"
        | "LinearProgressIndicatorTokens"
        | "ProgressIndicatorTokens" => "Progress indicators",
        "RadioButtonTokens" => "Radio button",
        "ScrimTokens" => "Scrim",
        "SearchBarTokens" | "SearchViewTokens" => "Search",
        "OutlinedSegmentedButtonTokens" => "Segmented buttons",
        "SliderTokens" => "Slider",
        "SnackbarTokens" => "Snackbar",
        "SplitButtonSmallTokens"
        | "SplitButtonMediumTokens"
        | "SplitButtonLargeTokens"
        | "SplitButtonXSmallTokens"
        | "SplitButtonXLargeTokens" => "Split buttons",
        "SwitchTokens" => "Switch",
        "PrimaryNavigationTabTokens" | "SecondaryNavigationTabTokens" => "Tabs",
        "FilledTextFieldTokens" | "OutlinedTextFieldTokens" => "Text fields",
        "TimeInputTokens" | "TimePickerTokens" => "Time pickers",
        "DockedToolbarTokens" | "FloatingToolbarTokens" => "Toolbars",
        "PlainTooltipTokens" | "RichTooltipTokens" => "Tooltips",
        _ => return None,
    })
}

/// Classify a non-token Kotlin source file (by file stem) into a component
/// family or `"infra"`. Anything not listed here is an error so new files are
/// deliberately placed.
pub fn source_family(stem: &str) -> Option<&'static str> {
    Some(match stem {
        // Families that also own token objects.
        "AlertDialog" | "DatePickerDialog" => "Dialogs",
        "AppBar" | "AppBarColumn" | "AppBarDsl" | "AppBarRow" => "App bars",
        "Badge" => "Badge",
        "BottomSheet"
        | "BottomSheetScaffold"
        | "ModalBottomSheet"
        | "SheetDefaults"
        | "DragHandle" => "Bottom sheets",
        "Button" => "Buttons",
        "ButtonGroup" => "Button groups",
        "Card" => "Cards",
        "Checkbox" => "Checkbox",
        "Chip" => "Chips",
        "DateInput" | "DatePicker" | "DateRangeInput" | "DateRangePicker" => "Date pickers",
        "Divider" => "Divider",
        "FloatingActionButton" => "FAB",
        "FloatingActionButtonMenu" => "FAB menu",
        "IconButton" => "Icon buttons",
        "ListItem" => "Lists",
        "LoadingIndicator" => "Loading indicator",
        "Menu" | "ExposedDropdownMenu" => "Menus",
        "NavigationBar" | "ShortNavigationBar" => "Navigation bar",
        "NavigationDrawer" => "Navigation drawer",
        "NavigationItem" => "Navigation items",
        "NavigationRail" | "WideNavigationRail" | "WideNavigationRailState" => "Navigation rail",
        "ProgressIndicator" | "WavyProgressIndicator" => "Progress indicators",
        "RadioButton" => "Radio button",
        "Scaffold" => "Scaffold",
        "SearchBar" => "Search",
        "SegmentedButton" => "Segmented buttons",
        "Slider" => "Slider",
        "Snackbar" | "SnackbarHost" => "Snackbar",
        "SplitButton" => "Split buttons",
        "Surface" => "Surface",
        "SwipeToDismissBox" => "Swipe to dismiss",
        "Switch" => "Switch",
        "Tab" | "TabRow" => "Tabs",
        "Text" => "Text",
        "TextField" | "OutlinedTextField" | "SecureTextField" => "Text fields",
        "TimePicker" | "TimePickerDialog" => "Time pickers",
        "ToggleButton" => "Toggle buttons",
        "Tooltip" => "Tooltips",
        "FloatingToolbar" => "Toolbars",
        "Icon" => "Icon",
        "Carousel"
        | "CarouselState"
        | "CarouselItemScope"
        | "KeylineList"
        | "Keylines"
        | "KeylineSnapPosition"
        | "Strategy"
        | "CarouselParallaxScrollEffect" => "Carousel",
        "PullToRefresh" => "Pull to refresh",

        // Shared infrastructure, not a component surface.
        "Arrangement"
        | "CalendarLocale"
        | "ColorScheme"
        | "ComponentProperties"
        | "ComponentStyles"
        | "ComposeMaterial3Flags"
        | "ContentColor"
        | "ExperimentalMaterial3Api"
        | "ExperimentalMaterial3ExpressiveApi"
        | "HorizontalCenterOptically"
        | "IconButtonDefaults"
        | "InteractiveComponentSize"
        | "Label"
        | "ListItemDefaults"
        | "MaterialShapes"
        | "MaterialTheme"
        | "MenuDefaults"
        | "MotionScheme"
        | "PrecisionPointer"
        | "Ripple"
        | "Scrim"
        | "ScrollField"
        | "Scrollbar"
        | "Shapes"
        | "Strings"
        | "TextFieldDefaults"
        | "TimeFormat"
        | "TonalPalette"
        | "Typography"
        | "TouchTarget" => "infra",
        _ => return None,
    })
}

/// Words in member/object names that denote component states. Words that
/// contain a shorter word (`Unselected`, `Inactive`, `Focused`, `Hovered`)
/// must come before the shorter word they contain.
const STATE_WORDS: &[&str] = &[
    "Indeterminate",
    "Unselected",
    "Inactive",
    "Selected",
    "Disabled",
    "Focused",
    "Focus",
    "Hovered",
    "Hover",
    "Pressed",
    "Dragged",
    "Active",
    "Error",
    "Checked",
    "Expanded",
    "Collapsed",
];

fn canonical_state(word: &str) -> &'static str {
    match word {
        "Focus" | "Focused" => "focused",
        "Hover" | "Hovered" => "hovered",
        "Pressed" => "pressed",
        "Dragged" => "dragged",
        "Disabled" => "disabled",
        "Selected" => "selected",
        "Unselected" => "unselected",
        "Active" => "active",
        "Inactive" => "inactive",
        "Error" => "error",
        "Checked" => "checked",
        "Indeterminate" => "indeterminate",
        "Expanded" => "expanded",
        "Collapsed" => "collapsed",
        _ => unreachable!(),
    }
}

/// Collect the states mentioned in a member/object name. Overlapping words
/// are handled by masking matched regions, so `Unselected` doesn't also
/// report `selected` and `Inactive` doesn't report `active`.
fn states_in(name: &str) -> Vec<&'static str> {
    let mut work = name.to_string();
    let mut out: Vec<&'static str> = Vec::new();
    for w in STATE_WORDS {
        while let Some(pos) = work.find(w) {
            work.replace_range(pos..pos + w.len(), &" ".repeat(w.len()));
            let c = canonical_state(w);
            if !out.contains(&c) {
                out.push(c);
            }
        }
    }
    out
}

fn sizes_in(name: &str) -> Vec<&'static str> {
    // Longest first so `XLarge`/`ExtraLarge` win over `Large`.
    const ORDERED: &[(&str, &str)] = &[
        ("XSmall", "x-small"),
        ("XLarge", "x-large"),
        ("ExtraLarge", "extra-large"),
        ("Small", "small"),
        ("Medium", "medium"),
        ("Large", "large"),
        ("Baseline", "baseline"),
    ];
    let mut work = name.to_string();
    let mut out: Vec<&'static str> = Vec::new();
    for (w, c) in ORDERED {
        while let Some(pos) = work.find(w) {
            work.replace_range(pos..pos + w.len(), &" ".repeat(w.len()));
            if !out.contains(c) {
                out.push(c);
            }
        }
    }
    out
}

/// Read the committed status file.
pub fn load_status(path: &Path) -> Result<Status, String> {
    if !path.exists() {
        return Ok(Status::default());
    }
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    let v = json::parse(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    let obj = v.obj().ok_or_else(|| format!("{}: top level must be an object", path.display()))?;
    let mut status = Status::default();
    for (name, entry) in obj {
        let e =
            entry.obj().ok_or_else(|| format!("{}: `{name}` must be an object", path.display()))?;
        let mut cs = ComponentStatus::default();
        for (k, val) in e {
            match k.as_str() {
                "status" => {
                    cs.status = val
                        .str()
                        .ok_or_else(|| format!("{name}.status must be a string"))?
                        .to_string();
                    if !["missing", "partial", "done"].contains(&cs.status.as_str()) {
                        return Err(format!(
                            "{name}: bad status `{}` (missing|partial|done)",
                            cs.status
                        ));
                    }
                }
                "issues" => {
                    cs.issues = val
                        .arr()
                        .ok_or_else(|| format!("{name}.issues must be an array"))?
                        .iter()
                        .map(|x| match x {
                            Json::Num(n) => Ok(*n as u32),
                            _ => Err(format!("{name}.issues must be numbers")),
                        })
                        .collect::<Result<_, String>>()?;
                }
                "needs" => {
                    cs.needs = val
                        .str()
                        .ok_or_else(|| format!("{name}.needs must be a string"))?
                        .to_string()
                }
                "notes" => {
                    cs.notes = val
                        .str()
                        .ok_or_else(|| format!("{name}.notes must be a string"))?
                        .to_string()
                }
                other => return Err(format!("{name}: unknown key `{other}`")),
            }
        }
        status.components.insert(name.clone(), cs);
    }
    Ok(status)
}

/// Serialize the status (sorted keys) back to the file format.
pub fn status_json(status: &Status) -> Json {
    let mut map = BTreeMap::new();
    for (name, cs) in &status.components {
        let mut e = BTreeMap::new();
        e.insert("status".to_string(), Json::Str(cs.status.clone()));
        e.insert(
            "issues".to_string(),
            Json::Arr(cs.issues.iter().map(|i| Json::Num(*i as f64)).collect()),
        );
        e.insert("needs".to_string(), Json::Str(cs.needs.clone()));
        e.insert("notes".to_string(), Json::Str(cs.notes.clone()));
        map.insert(name.clone(), Json::Obj(e));
    }
    Json::Obj(map)
}

/// Components discovered by scanning the Compose sources for `@Composable`
/// declarations: `(family, component name, source file)`.
pub struct SourceComponent {
    pub family: String,
    pub name: String,
    pub file: String,
}

/// Scan `.kt` files under `material3_dir` for public `@Composable fun`s.
/// `skip_dirs` are subdirectory names to ignore (e.g. `internal`, `tokens`).
pub fn scan_components(
    material3_dir: &Path,
    skip_dirs: &[&str],
) -> Result<Vec<SourceComponent>, String> {
    let mut out = Vec::new();
    let mut stack = vec![material3_dir.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let mut entries: Vec<_> = std::fs::read_dir(&dir)
            .map_err(|e| format!("cannot read {}: {e}", dir.display()))?
            .collect::<Result<_, _>>()
            .map_err(|e| format!("read_dir {}: {e}", dir.display()))?;
        entries.sort_by_key(|e| e.file_name());
        for entry in entries {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            if path.is_dir() {
                if !skip_dirs.contains(&name.as_str()) {
                    stack.push(path);
                }
                continue;
            }
            if !name.ends_with(".kt") {
                continue;
            }
            let stem = name.trim_end_matches(".kt");
            let family = source_family(stem).ok_or_else(|| {
                format!("{name}: no family classification — add it to source_family()")
            })?;
            if family == "infra" {
                continue;
            }
            let text = std::fs::read_to_string(&path)
                .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
            let rel =
                path.strip_prefix(material3_dir).unwrap_or(&path).to_string_lossy().to_string();
            for fn_name in composable_functions(&text) {
                out.push(SourceComponent {
                    family: family.to_string(),
                    name: fn_name,
                    file: rel.clone(),
                });
            }
        }
    }
    Ok(out)
}

/// Names of capitalized `fun`s within ~10 lines of an `@Composable`
/// annotation, excluding `internal`/`private` declarations.
fn composable_functions(text: &str) -> Vec<String> {
    let lines: Vec<&str> = text.lines().collect();
    // Brace depth at the start of each line (string literals ignored, so
    // "${...}" interpolations don't skew it). Only top-level (depth 0)
    // @Composable functions are components — members of classes, interfaces
    // and object literals are helpers.
    let mut depth = vec![0i64; lines.len()];
    let mut d = 0i64;
    for (i, line) in lines.iter().enumerate() {
        depth[i] = d;
        let mut chars = line.chars().peekable();
        let mut in_str = false;
        while let Some(c) = chars.next() {
            match c {
                '"' => in_str = !in_str,
                '/' if chars.peek() == Some(&'/') => break,
                '{' if !in_str => d += 1,
                '}' if !in_str => d -= 1,
                _ => {}
            }
        }
    }
    let mut out = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        if !line.contains("@Composable") {
            continue;
        }
        for j in i..(i + 10).min(lines.len()) {
            let l = lines[j].trim_start();
            if let Some(pos) = l.find("fun ") {
                if depth[j] != 0
                    || l.contains("internal ")
                    || l.contains("private ")
                    || l.contains("override ")
                {
                    break;
                }
                let rest = &l[pos + 4..];
                let name_end =
                    rest.find(|c: char| !(c.is_alphanumeric() || c == '_')).unwrap_or(rest.len());
                let name = &rest[..name_end];
                if name.chars().next().map(|c| c.is_uppercase()).unwrap_or(false)
                    && !out.iter().any(|n| n == name)
                {
                    out.push(name.to_string());
                }
                break;
            }
        }
    }
    out
}

/// Group an object's members into the state/size/motion summary used in the
/// component tables.
fn object_stats(obj: &ResolvedObject) -> (Vec<&'static str>, Vec<String>) {
    let mut states: Vec<&'static str> = Vec::new();
    let mut motion: Vec<String> = Vec::new();
    for m in &obj.members {
        for s in states_in(&m.name) {
            if !states.contains(&s) {
                states.push(s);
            }
        }
        if let Value::SpringKey(k) = &m.value {
            let k = snake_case(k);
            if !motion.contains(&k) {
                motion.push(k);
            }
        }
    }
    (states, motion)
}

/// Default `(issues, needs)` for a component family. Every row of the
/// inventory must carry both: an empty field means nothing, so `merge_status`
/// fills any unset field from this table and a family not listed here fails
/// the run loudly.
fn family_defaults(family: &str) -> Result<(Vec<u32>, &'static str), String> {
    // Issue map: #9 buttons/toggle/icon buttons; #10 button groups, split
    // buttons, FAB menu, toolbars, loading indicator; #11 expressive rework
    // of the remaining components; #12 adaptive layout/navigation. `needs`
    // names engine capabilities from #5 (springs), #6 (shapes), #7
    // (variable fonts/text scale), #8 (dynamic color), #12 (adaptive).
    Ok(match family {
        "App bars" => (vec![11], "springs (#5), shapes (#6)"),
        "Autocomplete" => (vec![11], "none"),
        "Badge" => (vec![11], "none"),
        "Bottom app bar" => (vec![10], "adaptive (#12)"),
        "Bottom sheets" => (vec![11], "springs (#5)"),
        "Buttons" => (vec![9], "springs (#5), shapes (#6)"),
        "Button groups" => (vec![10], "springs (#5), shapes (#6)"),
        "Cards" => (vec![11], "none"),
        "Carousel" => (vec![11], "springs (#5), shapes (#6)"),
        "Checkbox" => (vec![11], "shapes (#6)"),
        "Chips" => (vec![9], "shapes (#6)"),
        "Date pickers" => (vec![11], "none"),
        "Dialogs" => (vec![11], "none"),
        "Divider" => (vec![11], "none"),
        "FAB" => (vec![9], "springs (#5), shapes (#6)"),
        "FAB menu" => (vec![10], "springs (#5), shapes (#6)"),
        "Icon" => (vec![11], "none"),
        "Icon buttons" => (vec![9], "springs (#5), shapes (#6)"),
        "Lists" => (vec![11], "shapes (#6)"),
        "Loading indicator" => (vec![10], "wavy shapes (#6), springs (#5)"),
        "Menus" => (vec![11], "springs (#5), shapes (#6)"),
        "Navigation bar" => (vec![11, 12], "adaptive (#12), springs (#5), shapes (#6)"),
        "Navigation drawer" => (vec![11, 12], "adaptive (#12)"),
        "Navigation items" => (vec![11, 12], "adaptive (#12), springs (#5)"),
        "Navigation rail" => (vec![11, 12], "adaptive (#12), springs (#5)"),
        "Progress indicators" => (vec![11], "wavy shapes (#6), springs (#5)"),
        "Pull to refresh" => (vec![11], "springs (#5)"),
        "Radio button" => (vec![11], "shapes (#6)"),
        "Scaffold" => (vec![11, 12], "adaptive (#12)"),
        "Scrim" => (vec![11], "none"),
        "Search" => (vec![11], "springs (#5)"),
        "Segmented buttons" => (vec![10], "springs (#5), shapes (#6)"),
        "Slider" => (vec![11], "shapes (#6)"),
        "Snackbar" => (vec![11], "none"),
        "Split buttons" => (vec![10], "springs (#5), shapes (#6)"),
        "Surface" => (vec![11], "dynamic color (#8)"),
        "Swipe to dismiss" => (vec![11], "springs (#5)"),
        "Switch" => (vec![11], "springs (#5), shapes (#6)"),
        "Tabs" => (vec![11], "none"),
        "Text" => (vec![7], "variable fonts (#7), text scale (#12)"),
        "Text fields" => (vec![11], "variable fonts (#7), shapes (#6)"),
        "Time pickers" => (vec![11], "none"),
        "Toggle buttons" => (vec![9], "springs (#5), shapes (#6)"),
        "Toolbars" => (vec![10], "springs (#5), adaptive (#12)"),
        "Tooltips" => (vec![11], "none"),
        other => return Err(format!("{other}: no defaults — add it to family_defaults()")),
    })
}

/// Sync the status file with the current scan: add new upstream components as
/// `missing` with the family defaults, drop entries for components upstream
/// no longer ships, and fill any unset `issues`/`needs` from the family
/// defaults so no row renders empty. Returns true if the status changed (the
/// file must be rewritten).
pub fn merge_status(
    status: &mut Status,
    components: &[SourceComponent],
) -> Result<bool, String> {
    let mut changed = false;
    for c in components {
        let (issues, needs) = family_defaults(&c.family)?;
        match status.components.get_mut(&c.name) {
            Some(cs) => {
                if cs.issues.is_empty() {
                    cs.issues = issues;
                    changed = true;
                }
                if cs.needs.is_empty() {
                    cs.needs = needs.to_string();
                    changed = true;
                }
            }
            None => {
                status.components.insert(
                    c.name.clone(),
                    ComponentStatus {
                        status: "missing".into(),
                        issues,
                        needs: needs.to_string(),
                        notes: String::new(),
                    },
                );
                changed = true;
            }
        }
    }
    let scanned: std::collections::BTreeSet<&str> =
        components.iter().map(|c| c.name.as_str()).collect();
    let before = status.components.len();
    status.components.retain(|name, _| scanned.contains(name.as_str()));
    if status.components.len() != before {
        changed = true;
    }
    Ok(changed)
}

/// Render `PARITY.md`.
pub fn render(
    lib: &Library,
    components: &[SourceComponent],
    status: &Status,
    repo: &str,
    commit: &str,
) -> Result<String, String> {
    let mut s = String::new();
    let _ = writeln!(s, "# Material 3 Expressive parity inventory\n");
    let _ = writeln!(
        s,
        "Generated by `ui-libraries/material/scripts/token-generator` from\n\
         {repo} @ {commit} (`TOKENS_SOURCE`).\n\
         Upstream token file VERSION markers: {}.\n\
         Token values live in `src/ui/styling/generated/`; statuses live in\n\
         `PARITY_STATUS.json` — re-running the generator never loses them.\n\
         Do not edit this file by hand; run `cargo run -p material-token-generator`\n\
         from `ui-libraries/material/`.\n",
        lib.token_version
    );

    // ---- Components -------------------------------------------------------
    let _ = writeln!(s, "## Components\n");
    let _ = writeln!(
        s,
        "Status: `missing` = no Slint component, `partial` = exists but not at\n\
         parity, `done` = matches the upstream spec. The issue column lists the\n\
         tracking issues; `needs` names the engine capability the component\n\
         depends on.\n"
    );

    // family -> token objects
    let mut families: BTreeMap<&str, Vec<&ResolvedObject>> = BTreeMap::new();
    for obj in &lib.objects {
        if let Some(fam) = family_of(&obj.name) {
            families.entry(fam).or_default().push(obj);
        }
    }
    // family -> components
    let mut fam_components: BTreeMap<&str, Vec<&SourceComponent>> = BTreeMap::new();
    for c in components {
        fam_components.entry(c.family.as_str()).or_default().push(c);
    }

    let mut all_families: Vec<&str> = families.keys().copied().collect();
    for f in fam_components.keys() {
        if !all_families.contains(f) {
            all_families.push(f);
        }
    }
    all_families.sort();

    for fam in all_families {
        let _ = writeln!(s, "### {fam}\n");
        let objects = families.get(fam).cloned().unwrap_or_default();
        if !objects.is_empty() {
            let mut sizes: Vec<&str> = Vec::new();
            let mut states: Vec<&str> = Vec::new();
            let mut motion: Vec<String> = Vec::new();
            for obj in &objects {
                for sz in sizes_in(&obj.name) {
                    if !sizes.contains(&sz) {
                        sizes.push(sz);
                    }
                }
                let (st, mo) = object_stats(obj);
                for s2 in st {
                    if !states.contains(&s2) {
                        states.push(s2);
                    }
                }
                for m in mo {
                    if !motion.contains(&m) {
                        motion.push(m);
                    }
                }
            }
            let _ = writeln!(
                s,
                "Token objects: {}\n",
                objects.iter().map(|o| o.name.as_str()).collect::<Vec<_>>().join(", ")
            );
            if !sizes.is_empty() {
                let _ = writeln!(s, "Sizes: {}\n", sizes.join(", "));
            }
            if !states.is_empty() {
                let _ = writeln!(s, "States: {}\n", states.join(", "));
            }
            if !motion.is_empty() {
                let _ = writeln!(s, "Motion keys: {}\n", motion.join(", "));
            }
        }
        let comps = fam_components.get(fam).cloned().unwrap_or_default();
        if !comps.is_empty() {
            let _ = writeln!(
                s,
                "| Component | Source | Status | Issues | Needs | Notes |\n\
                 |---|---|---|---|---|---|"
            );
            for c in &comps {
                let st = status.components.get(&c.name).cloned().unwrap_or_default();
                let issues = if st.issues.is_empty() {
                    "-".to_string()
                } else {
                    st.issues.iter().map(|i| format!("#{i}")).collect::<Vec<_>>().join(", ")
                };
                let status_s = if st.status.is_empty() { "missing" } else { &st.status };
                let needs = if st.needs.is_empty() { "-" } else { &st.needs };
                let notes = if st.notes.is_empty() { "-" } else { &st.notes };
                let _ = writeln!(
                    s,
                    "| {} | {} | {} | {} | {} | {} |",
                    c.name, c.file, status_s, issues, needs, notes
                );
            }
            let _ = writeln!(s);
        }
        if objects.is_empty() && comps.is_empty() {
            let _ = writeln!(s, "_no token objects or component sources_\n");
        }
    }

    // ---- Token reference ---------------------------------------------------
    let _ = writeln!(s, "## Token reference\n");
    let _ = writeln!(
        s,
        "Every token in the pinned sources, its official value as written in\n\
         Kotlin, and the generated Slint property. References resolve to the\n\
         generated globals listed under “Resolved”.\n"
    );
    for obj in &lib.objects {
        let _ = writeln!(s, "### {} ({})\n", obj.name, obj.file);
        let _ = writeln!(
            s,
            "| Token | Official value | Slint |\n\
             |---|---|---|"
        );
        for m in &obj.members {
            let slint = slint_ref(obj, m);
            let _ = writeln!(s, "| {} | `{}` | `{}` |", m.name, m.rhs, slint);
        }
        let _ = writeln!(s);
    }
    Ok(s)
}

/// The `GlobalName.member` path a resolved member emits to.
fn slint_ref(obj: &ResolvedObject, m: &crate::model::ResolvedMember) -> String {
    let global = obj.name.as_str();
    match &m.value {
        Value::TypeStyle(_) if global == "TypographyTokens" => {
            format!("TypographyTokens.{}", snake_case(&m.name))
        }
        _ => format!("{global}.{}", snake_case(&m.name)),
    }
}
