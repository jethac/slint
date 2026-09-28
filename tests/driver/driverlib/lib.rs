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
///
/// Optional markers:
/// - `//TIMES=0,64,128` — motion capture timestamps (required for `motion`).
/// - `//TRACE_PROPS=p1,p2` — names of `out property`s on the case component to
///   record in the trace (numbers, colors, bools, strings).
/// - `//TRACE_ELEMENTS=id1,id2` — element ids whose x/y/width/height/opacity are
///   recorded in the trace (found with `ElementQuery::match_id`).
/// - `//ACTION=move:x,y` / `//ACTION=press:x,y` / `//ACTION=release:x,y` —
///   pointer input dispatched to the window before the case is rendered, in
///   declaration order (logical coordinates).
/// - `//DENSITIES=1,2` — the densities the case is rendered and compared at
///   (default: `1,2`).
/// - `//PARITY_EPS=8` — per-channel strict-pixel tolerance override; only use
///   with a comment on the case explaining why this case needs it.
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
}

/// One `//ACTION=` pointer step.
#[derive(Debug, Clone, PartialEq)]
pub enum ParityAction {
    Move { x: f32, y: f32 },
    Press { x: f32, y: f32 },
    Release { x: f32, y: f32 },
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
        let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
        rest[..end].to_string()
    });
    let (parity, negative_note) = match parity.as_deref().map(str::trim) {
        Some(v) if v.starts_with("negative") => {
            (Some("negative".to_string()), v.split_once(':').map(|(_, n)| n.trim().to_string()))
        }
        Some(v) => (Some(v.to_string()), None),
        None => (None, None),
    };

    let mut actions = Vec::new();
    static ACTION_RX: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"//ACTION=\s*([a-z]+)\s*:\s*([0-9.\-]+)\s*,\s*([0-9.\-]+)").unwrap());
    for m in ACTION_RX.captures_iter(source) {
        let (x, y) = (m[2].parse().unwrap(), m[3].parse().unwrap());
        actions.push(match &m[1] {
            "move" => ParityAction::Move { x, y },
            "press" => ParityAction::Press { x, y },
            "release" => ParityAction::Release { x, y },
            other => panic!("Unknown //ACTION= kind '{other}' (expected move|press|release)"),
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

    ParityMarkers {
        parity,
        times: csv("//TIMES=").into_iter().filter_map(|s| s.parse().ok()).collect(),
        trace_props: csv("//TRACE_PROPS="),
        trace_elements: csv("//TRACE_ELEMENTS="),
        actions,
        densities,
        eps,
        negative_note,
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
        [ParityAction::Move { x: 12.5, y: 40.0 }, ParityAction::Press { x: 30.0, y: 20.0 }]
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
