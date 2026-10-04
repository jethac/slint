/// One progress-indicator family widget (`linear-progress`,
/// `circular-progress`, `linear-wavy-progress`, `circular-wavy-progress`):
/// `progress` and `indeterminate` mirror the `Widget` fields; explicit
/// `width`/`height` pin the drawn size like the Compose `Modifier.size`.
/// Elements are named `progress{n}` in scene order.
fn progress_widget(s: &mut String, w: &Widget, i: usize) {
    let component = match w.kind.as_str() {
        "linear-progress" => "LinearProgressIndicator",
        "circular-progress" => "CircularProgressIndicator",
        "linear-wavy-progress" => "LinearWavyProgressIndicator",
        "circular-wavy-progress" => "CircularWavyProgressIndicator",
        other => panic!("unknown progress kind {other:?}"),
    };
    let mut p = String::new();
    let over = &w.slint_overrides;
    if w.indeterminate == Some(true) {
        p.push_str("        indeterminate: true;\n");
    } else if let Some(progress) = w.progress {
        writeln!(p, "        progress: {progress};").unwrap();
    }
    // `slint_overrides` — the negative scenes' deliberate defects: a wrong
    // `gap-size`, `wavelength` or a flat `amplitude` only the Slint side
    // renders.
    for (key, prop) in [("gap_size", "gap-size"), ("wavelength", "wavelength")] {
        if let Some(v) = over.get(key) {
            writeln!(p, "        {prop}: {}px;", widget_num(v)).unwrap();
        }
    }
    if let Some(v) = over.get("amplitude") {
        writeln!(p, "        amplitude: {};", widget_num(v)).unwrap();
    }
    if let Some(width) = w.width {
        writeln!(p, "        width: {width}px;").unwrap();
    }
    if let Some(height) = w.height {
        writeln!(p, "        height: {height}px;").unwrap();
    }
    writeln!(
        s,
        "    progress{i} := {component} {{\n        x: {}px;\n        y: {}px;\n{}    }}\n",
        w.x as i64, w.y as i64, p,
    )
    .unwrap();
}

