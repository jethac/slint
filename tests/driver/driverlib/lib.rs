// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::sync::LazyLock;

use regex::Regex;

pub struct TestCase {
    pub absolute_path: std::path::PathBuf,
    pub relative_path: std::path::PathBuf,
    pub requested_style: Option<&'static str>,
}

impl TestCase {
    /// Return a string which is a valid C++/Rust identifier
    pub fn identifier(&self) -> String {
        let mut result = self
            .relative_path
            .with_extension("")
            .to_string_lossy()
            .replace([std::path::MAIN_SEPARATOR, '-'], "_");
        if let Some(requested_style) = &self.requested_style {
            result.push('_');
            result.push_str(requested_style);
        }
        result
    }

    /// Returns true if the test case should be ignored for the specified driver.
    pub fn is_ignored(&self, driver: &str) -> bool {
        let source = std::fs::read_to_string(&self.absolute_path).unwrap();
        extract_ignores(&source).collect::<Vec<_>>().contains(&driver)
    }
}

/// Returns a list of all the `.slint` files in the subfolders e.g. `tests/cases` .
pub fn collect_test_cases(sub_folders: &str) -> std::io::Result<Vec<TestCase>> {
    let mut results = Vec::new();

    let mut all_styles = vec!["fluent", "material", "cupertino", "cosmic"];

    println!("cargo:rerun-if-env-changed=DEP_I_SLINT_BACKEND_QT_SUPPORTS_NATIVE_STYLE");
    if std::env::var("DEP_I_SLINT_BACKEND_QT_SUPPORTS_NATIVE_STYLE").unwrap_or_default() == "1" {
        all_styles.push("qt");
    }

    let case_root_dir: std::path::PathBuf =
        [env!("CARGO_MANIFEST_DIR"), "..", "..", sub_folders].iter().collect();

    println!("cargo:rerun-if-env-changed=SLINT_TEST_FILTER");
    let filter = std::env::var("SLINT_TEST_FILTER").ok();

    for entry in walkdir::WalkDir::new(case_root_dir.clone()).follow_links(true) {
        let entry = entry?;
        if entry.file_type().is_dir() {
            println!("cargo:rerun-if-changed={}", entry.into_path().display());
            continue;
        }
        let absolute_path = entry.into_path();
        let relative_path =
            std::path::PathBuf::from(absolute_path.strip_prefix(&case_root_dir).unwrap());
        if let Some(filter) = &filter
            && !relative_path.to_str().unwrap().contains(filter)
        {
            continue;
        }
        if let Some(ext) = absolute_path.extension()
            && (ext == "60" || ext == "slint")
        {
            let styles_to_test: Vec<&'static str> = if relative_path.starts_with("widgets") {
                let style_ignores =
                    extract_ignores(&std::fs::read_to_string(&absolute_path).unwrap())
                        .filter_map(|ignore| ignore.strip_prefix("style-").map(ToString::to_string))
                        .collect::<Vec<_>>();

                all_styles
                    .iter()
                    .filter(|available_style| {
                        !style_ignores.iter().any(|ignored_style| *available_style == ignored_style)
                    })
                    .cloned()
                    .collect::<Vec<_>>()
            } else {
                vec![""]
            };
            results.extend(styles_to_test.into_iter().map(|style| TestCase {
                absolute_path: absolute_path.clone(),
                relative_path: relative_path.clone(),
                requested_style: if style.is_empty() { None } else { Some(style) },
            }));
        }
    }
    Ok(results)
}

/// A test functions looks something like
/// ````text
/// /*
///   ```cpp
///   TestCase instance;
///   assert(instance.x.get() == 0);
///   ```
/// */
/// ````
pub struct TestFunction<'a> {
    /// In the example above: `cpp`
    pub language_id: &'a str,
    /// The content of the test function
    pub source: &'a str,
}

/// Extract the test functions from
pub fn extract_test_functions(source: &str) -> impl Iterator<Item = TestFunction<'_>> {
    static RX: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?sU)\r?\n```([a-z]+)\r?\n(.+)\r?\n```\r?\n").unwrap());
    RX.captures_iter(source).map(|mat| TestFunction {
        language_id: mat.get(1).unwrap().as_str(),
        source: mat.get(2).unwrap().as_str(),
    })
}

#[test]
fn test_extract_test_functions() {
    let source = r"
/*
```cpp
auto xx = 0;
auto yy = 0;
```

```rust
let xx = 0;
let yy = 0;
```
*/
";
    let mut r = extract_test_functions(source);

    let r1 = r.next().unwrap();
    assert_eq!(r1.language_id, "cpp");
    assert_eq!(r1.source, "auto xx = 0;\nauto yy = 0;");

    let r2 = r.next().unwrap();
    assert_eq!(r2.language_id, "rust");
    assert_eq!(r2.source, "let xx = 0;\nlet yy = 0;");
}

#[test]
fn test_extract_test_functions_win() {
    let source = "/*\r\n```cpp\r\nfoo\r\nbar\r\n```\r\n*/\r\n";
    let mut r = extract_test_functions(source);
    let r1 = r.next().unwrap();
    assert_eq!(r1.language_id, "cpp");
    assert_eq!(r1.source, "foo\r\nbar");
}

/// Extract extra include paths from a comment in the source if present.
pub fn extract_include_paths(source: &str) -> impl Iterator<Item = &'_ str> {
    static RX: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"//include_path:\s*(.+)\s*\n").unwrap());
    RX.captures_iter(source).map(|mat| mat.get(1).unwrap().as_str().trim())
}

#[test]
fn test_extract_include_paths() {
    assert!(extract_include_paths("something").next().is_none());

    let source = r"
    //include_path: ../first
    //include_path: ../second
    Blah {}
";

    let r = extract_include_paths(source).collect::<Vec<_>>();
    assert_eq!(r, ["../first", "../second"]);

    // Windows \r\n
    let source = "//include_path: ../first\r\n//include_path: ../second\r\nBlah {}\r\n";
    let r = extract_include_paths(source).collect::<Vec<_>>();
    assert_eq!(r, ["../first", "../second"]);
}

/// Extract extra library paths from a comment in the source if present.
pub fn extract_library_paths(source: &str) -> impl Iterator<Item = (&'_ str, &'_ str)> {
    static RX: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"//library_path\((.+)\):\s*(.+)\s*\n").unwrap());
    RX.captures_iter(source)
        .map(|mat| (mat.get(1).unwrap().as_str().trim(), mat.get(2).unwrap().as_str().trim()))
}

#[test]
fn test_extract_library_paths() {
    use std::collections::HashMap;

    assert!(extract_library_paths("something").next().is_none());

    let source = r"
    //library_path(first): ../first/lib.slint
    //library_path(second): ../second/lib.slint
    Blah {}
";

    let r = extract_library_paths(source).collect::<HashMap<_, _>>();
    assert_eq!(
        r,
        HashMap::from([("first", "../first/lib.slint"), ("second", "../second/lib.slint")])
    );
}

/// The extra checks the Material parity harness (`tests/screenshots/cases/material/`)
/// runs for a case, declared with `//PARITY=<kind>` markers in the case source:
///
/// - `//PARITY=static` — compare the render against the Jetpack Compose reference in
///   `ui-libraries/material/parity/compose/references/`, plus a strict check against
///   the Slint self-references the normal screenshot tests write.
/// - `//PARITY=motion` — same, but advance the mocked animation clock to each
///   `//TIMES=` timestamp (ms since case creation) and capture a frame plus a
///   property trace per timestamp.
/// - `//PARITY=negative` — the case is intentionally wrong; the test passes only
///   when the layered comparator rejects it.
/// - `//PARITY=xfail:<reason>` — a known divergence the harness must report,
///   not absorb: the test passes only while the comparison finds at least one
///   difference (an unexpected pass fails, so the marker can't outlive the
///   divergence it names). Prefixed with a driver scope —
///   `//PARITY=xfail:software:<reason>` — the divergence is expected only on
///   that driver (e.g. a clip mode a renderer lacks); on every other driver
///   the case must pass clean.
///
/// Optional markers:
/// - `//TIMES=0,64,128` — motion capture timestamps (required for `motion`).
/// - `//TRACE_PROPS=p1,p2` — names of `out property`s on the case component to
///   record in the trace (numbers, colors, bools, strings).
/// - `//TRACE_ELEMENTS=id1,id2` — element ids whose x/y/width/height/opacity are
///   recorded in the trace (found with `ElementQuery::match_id`).
/// - `//TRACE_ITEMS=group0:item,group1:item` — children inside element
///   `group0` whose local id is `item` (one entry per repeater instance) are
///   recorded as `group0item0`, `group0item1`, … — the same convention the
///   Compose emitter uses for repeated container items. Use it for repeated
///   children, which share one qualified id and so cannot be named in
///   `//TRACE_ELEMENTS=`. When a container can emit items under more than
///   one element id (e.g. two mutually exclusive `for` loops), the alternates
///   are joined with `+` — `group0:item+radio_item` — and recorded under the
///   first name.
/// - `//ACTION=move:x,y` / `//ACTION=press:x,y` / `//ACTION=release:x,y` —
///   pointer input dispatched to the window before the case is rendered, in
///   declaration order (logical coordinates).
/// - `//ACTION=key:Tab` — a named key press+release (`Key::Tab`, `Key::Backtab`,
///   or a character). Keyboard input moves focus without moving the pointer, so
///   `focused` widgets stay focused while the pointer hovers or presses another.
/// - `//DENSITIES=1,2` — the densities the case is rendered and compared at
///   (default: `1,2`).
/// - `//PARITY_EPS=8` — per-channel strict-pixel tolerance override; only use
///   with a comment on the case explaining why this case needs it.
/// - `//MASK_INNER=id@t1,t2` — at the listed timestamps the element's
///   interior is excluded from pixel comparison (overlay ink mid-flight,
///   e.g. a ripple whose coverage is implementation detail); its boundary
///   band still compares strictly. One line per element, may repeat.
/// - `//MASK_DECOR=id@t1,t2` — at the listed timestamps the element's
///   decoration outside its silhouette (a drop shadow, a blur) is not
///   comparable on the reference engine; the corner zones fall back to
///   the normal decoration band. One line per element, may repeat. An
///   optional `+N` after the id (`//MASK_DECOR=fab+12@2000`) widens the
///   decoration band to `N` dp for decorations that spill further than
///   [`DECORATION_MARGIN_DP`] — a big elevation shadow, for instance.
/// - `//XFAIL_TEXT=<reason>` — the text-width layer accepts Slint's
///   ceil-quantized text widths (the tracked divergence the reason names,
///   e.g. `issue #28`): `sw − unhinted advance` may land anywhere in
///   `(−0.15, 1.15]`. Without the marker the bound is 0.5 px —
///   `(−0.15, 0.65]`. A drift past a whole pixel fails either way.
/// - `//TEXT_DILATE=<dp>` — widens the `Text`/`TextInput` ink mask by `dp`
///   logical px on top of the built-in 2 device px, for cases whose
///   structure derives from a measured text width (an outlined text
///   field's label-notch edge, which tracks `labelWidth`). The engines'
///   glyph metrics differ by a fraction of a px, so a notch edge can sit
///   a device px off while every ink cell still verifies. Only use with
///   a comment on the case explaining why.
/// - `//XFAIL_SILHOUETTE=<reason>` — on the software driver the
///   `//MASK_INNER=` silhouette findings are an expected divergence (the
///   reason names the tracked gap, e.g. `issue #6` for the axis-aligned
///   clip). A marked case that comes back with zero findings fails — the
///   divergence is gone and the marker must be removed.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct ParityMarkers {
    /// `Some("static"|"motion"|"negative")` when a `//PARITY=` marker is present.
    pub parity: Option<String>,
    pub times: Vec<u64>,
    pub trace_props: Vec<String>,
    pub trace_elements: Vec<String>,
    pub actions: Vec<ParityAction>,
    pub densities: Vec<u32>,
    pub eps: Option<f32>,
    /// `PARITY=negative` cases declare what they get wrong after a `:`.
    pub negative_note: Option<String>,
    /// `PARITY=xfail` cases name the tracked divergence after a `:`.
    pub xfail_note: Option<String>,
    /// Non-empty when `PARITY=xfail` was scoped to specific drivers —
    /// `xfail:software:<reason>` expects the divergence only on `software`.
    pub xfail_renderers: Vec<String>,
    /// `//XFAIL_TEXT=<reason>` marks the text-width layer's ceil-quantization
    /// window an expected divergence rather than a failure.
    pub xfail_text: Option<String>,
    /// `//XFAIL_SILHOUETTE=<reason>` marks the software driver's
    /// `//MASK_INNER=` silhouette findings an expected divergence rather
    /// than a failure; zero findings re-arms the check.
    pub xfail_silhouette: Option<String>,
    /// `//TEXT_DILATE=<dp>` — extra logical-px dilation of the `Text` /
    /// `TextInput` ink mask, for structure whose position derives from a
    /// measured text width (see the marker docs above).
    pub text_dilate: Option<f64>,
    /// `(container-id, local-child-ids)` pairs from `//TRACE_ITEMS=`
    /// markers: the container's repeated children with any listed local id
    /// are recorded as `<container-id><first-id><i>` in tree order.
    pub trace_items: Vec<(String, Vec<String>)>,
    /// `(element-id, t_ms)` pairs from `//MASK_INNER=` markers.
    pub mask_inner: Vec<(String, u64)>,
    /// `(element-id, t_ms, margin)` triples from `//MASK_DECOR=` markers;
    /// `margin` is `Some(dp)` when the marker carried a `+dp` override.
    pub mask_decor: Vec<(String, u64, Option<f64>)>,
}

/// One `//ACTION=` input step. `at_ms` is the dispatch time within the
/// frame sequence: `0` (no `@` suffix) fires right after the pre-gesture
/// baseline frame; a later value fires at that mock-clock time so a gesture
/// sequence can play out across the timed frames.
#[derive(Debug, Clone, PartialEq)]
pub enum ParityAction {
    Move { x: f32, y: f32, at_ms: u64 },
    Press { x: f32, y: f32, at_ms: u64 },
    Release { x: f32, y: f32, at_ms: u64 },
    /// A named key (`Tab`, `Backtab`, `Escape`, ...) dispatched as a
    /// press+release pair; anything else is dispatched as the literal text.
    Key { name: String, at_ms: u64 },
}

impl ParityAction {
    pub fn at_ms(&self) -> u64 {
        match *self {
            Self::Move { at_ms, .. }
            | Self::Press { at_ms, .. }
            | Self::Release { at_ms, .. }
            | Self::Key { at_ms, .. } => at_ms,
        }
    }
}

/// Extract the parity markers listed on [`ParityMarkers`] from a case's source.
pub fn extract_parity(source: &str) -> ParityMarkers {
    let csv = |needle: &str| -> Vec<String> {
        source
            .find(needle)
            .map(|p| {
                let rest = &source[p + needle.len()..];
                let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
                rest[..end]
                    .split(',')
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(ToString::to_string)
                    .collect()
            })
            .unwrap_or_default()
    };

    let parity = source.find("//PARITY=").map(|p| {
        let rest = &source[p + "//PARITY=".len()..];
        let end = rest.find('\n').unwrap_or(rest.len());
        rest[..end].trim().to_string()
    });
    let (parity, negative_note, xfail_note, xfail_renderers) =
        match parity.as_deref().map(str::trim) {
            Some(v) if v.starts_with("negative") => (
                Some("negative".to_string()),
                v.split_once(':').map(|(_, n)| n.trim().to_string()),
                None,
                Vec::new(),
            ),
            Some(v) if v.starts_with("xfail") => {
            // `xfail[:<driver>[,<driver>…]]: <reason>` — an optional scope of
            // known driver names limits where the divergence is expected.
            const DRIVERS: &[&str] = &["software", "skia", "femtovg", "interpreter"];
            let note = v.split_once(':').map(|(_, n)| n.trim()).unwrap_or("");
            let (renderers, reason) = match note.split_once(':') {
                Some((scope, reason))
                    if !scope.is_empty()
                        && scope
                            .split(',')
                            .all(|d| DRIVERS.contains(&d.trim())) =>
                {
                    (
                        scope.split(',').map(|d| d.trim().to_string()).collect(),
                        reason.trim().to_string(),
                    )
                }
                _ => (Vec::new(), note.to_string()),
            };
                (
                    Some("xfail".to_string()),
                    None,
                    (!reason.is_empty()).then_some(reason),
                    renderers,
                )
            }
            Some(v) => (Some(v.to_string()), None, None, Vec::new()),
            None => (None, None, None, Vec::new()),
        };

    let mut actions = Vec::new();
    static ACTION_RX: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
            r"//ACTION=\s*([a-z]+)(?:@([0-9.]+))?\s*:\s*(?:([0-9.\-]+)\s*,\s*([0-9.\-]+)|([A-Za-z]+))",
        )
        .unwrap()
    });
    for m in ACTION_RX.captures_iter(source) {
        let at_ms = m.get(2).and_then(|g| g.as_str().parse().ok()).unwrap_or(0);
        let num = |i| m.get(i).unwrap().as_str().parse().unwrap();
        actions.push(match &m[1] {
            "move" => ParityAction::Move { x: num(3), y: num(4), at_ms },
            "press" => ParityAction::Press { x: num(3), y: num(4), at_ms },
            "release" => ParityAction::Release { x: num(3), y: num(4), at_ms },
            "key" => ParityAction::Key { name: m[5].to_string(), at_ms },
            other => {
                panic!("Unknown //ACTION= kind '{other}' (expected move|press|release|key)")
            }
        });
    }

    let densities = {
        let v: Vec<u32> = csv("//DENSITIES=").into_iter().filter_map(|s| s.parse().ok()).collect();
        if v.is_empty() { vec![1, 2] } else { v }
    };

    let eps = source.find("PARITY_EPS=").map(|p| {
        let rest = &source[p + "PARITY_EPS=".len()..];
        rest.find(char::is_whitespace)
            .and_then(|end| rest[..end].parse().ok())
            .expect("Cannot parse PARITY_EPS=")
    });

    let mut mask_inner = Vec::new();
    static MASK_INNER_RX: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"//MASK_INNER=\s*([A-Za-z0-9_]+)\s*@\s*([0-9,\s]+)").unwrap());
    for m in MASK_INNER_RX.captures_iter(source) {
        for t in m[2].split(',').map(str::trim).filter(|s| !s.is_empty()) {
            mask_inner.push((
                m[1].to_string(),
                t.parse().expect("Cannot parse //MASK_INNER= timestamp"),
            ));
        }
    }

    let mut mask_decor = Vec::new();
    static MASK_DECOR_RX: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"//MASK_DECOR=\s*([A-Za-z0-9_]+)(?:\s*\+\s*([0-9.]+))?\s*@\s*([0-9,\s]+)")
            .unwrap()
    });
    for m in MASK_DECOR_RX.captures_iter(source) {
        let margin = m.get(2).map(|g| {
            g.as_str().parse().expect("Cannot parse //MASK_DECOR= margin override")
        });
        for t in m[3].split(',').map(str::trim).filter(|s| !s.is_empty()) {
            mask_decor.push((
                m[1].to_string(),
                t.parse().expect("Cannot parse //MASK_DECOR= timestamp"),
                margin,
            ));
        }
    }

    ParityMarkers {
        parity,
        times: csv("//TIMES=").into_iter().filter_map(|s| s.parse().ok()).collect(),
        trace_props: csv("//TRACE_PROPS="),
        trace_elements: csv("//TRACE_ELEMENTS="),
        actions,
        densities,
        eps,
        negative_note,
        xfail_note,
        xfail_renderers,
        xfail_text: source.find("//XFAIL_TEXT=").map(|p| {
            let rest = &source[p + "//XFAIL_TEXT=".len()..];
            let end = rest.find('\n').unwrap_or(rest.len());
            rest[..end].trim().to_string()
        }),
        xfail_silhouette: source.find("//XFAIL_SILHOUETTE=").map(|p| {
            let rest = &source[p + "//XFAIL_SILHOUETTE=".len()..];
            let end = rest.find('\n').unwrap_or(rest.len());
            rest[..end].trim().to_string()
        }),
        text_dilate: source.find("//TEXT_DILATE=").map(|p| {
            let rest = &source[p + "//TEXT_DILATE=".len()..];
            rest.find(char::is_whitespace)
                .and_then(|end| rest[..end].parse().ok())
                .expect("Cannot parse //TEXT_DILATE=")
        }),
        trace_items: csv("//TRACE_ITEMS=")
            .into_iter()
            .map(|entry| {
                entry
                    .split_once(':')
                    .map(|(container, locals)| {
                        (
                            container.to_string(),
                            locals.split('+').map(str::to_string).collect::<Vec<_>>(),
                        )
                    })
                    .unwrap_or_else(|| panic!("Cannot parse //TRACE_ITEMS= entry '{entry}' (expected <container>:<child-id>)"))
            })
            .collect(),
        mask_inner,
        mask_decor,
    }
}

#[test]
fn test_extract_parity() {
    let source = r"
//PARITY=motion
//TIMES=0,64,128,240
//TRACE_PROPS=progress
//TRACE_ELEMENTS=thumb,track
//ACTION=move: 12.5, 40
//ACTION=press:30,20
//DENSITIES=1,2
//PARITY_EPS=10
Blah {}
";
    let m = extract_parity(source);
    assert_eq!(m.parity.as_deref(), Some("motion"));
    assert_eq!(m.times, [0, 64, 128, 240]);
    assert_eq!(m.trace_props, ["progress"]);
    assert_eq!(m.trace_elements, ["thumb", "track"]);
    assert_eq!(
        m.actions,
        [
            ParityAction::Move { x: 12.5, y: 40.0, at_ms: 0 },
            ParityAction::Press { x: 30.0, y: 20.0, at_ms: 0 }
        ]
    );
    assert_eq!(m.densities, [1, 2]);
    assert_eq!(m.eps, Some(10.0));

    let neg = extract_parity("//PARITY=negative: wrong corner radius\nBlah {}");
    assert_eq!(neg.parity.as_deref(), Some("negative"));
    assert_eq!(neg.negative_note.as_deref(), Some("wrong corner radius"));
    assert_eq!(neg.densities, [1, 2]);
}

/// Extract `//ignore` comments from the source.
fn extract_ignores(source: &str) -> impl Iterator<Item = &'_ str> {
    static RX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"//ignore:\s*(.+)\s*\n").unwrap());
    RX.captures_iter(source).flat_map(|mat| {
        mat.get(1).unwrap().as_str().split(&[' ', ',']).map(str::trim).filter(|s| !s.is_empty())
    })
}

#[test]
fn test_extract_ignores() {
    assert!(extract_ignores("something").next().is_none());

    let source = r"
    //ignore: cpp
    //ignore: rust, nodejs
    Blah {}
";

    let r = extract_ignores(source).collect::<Vec<_>>();
    assert_eq!(r, ["cpp", "rust", "nodejs"]);
}

pub fn extract_cpp_namespace(source: &str) -> Option<String> {
    static RX: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"//cpp-namespace:\s*(.+)\s*\n").unwrap());
    RX.captures(source).map(|mat| mat.get(1).unwrap().as_str().trim().to_string())
}

#[test]
fn test_extract_cpp_namespace() {
    assert!(extract_cpp_namespace("something").is_none());

    let source = r"
    //cpp-namespace: ui
    Blah {}
";

    let r = extract_cpp_namespace(source);
    assert_eq!(r, Some("ui".to_string()));
}
