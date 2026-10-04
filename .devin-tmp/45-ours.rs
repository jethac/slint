/// One navigation-rail family widget (`navigation-rail`,
/// `wide-navigation-rail`, `modal-navigation-rail`): geometry plus the
/// props the Compose mirror sets. Elements are named `rail{n}` in scene
/// order.
///
/// `params.rail_events` (`[["expand", ms], ["collapse", ms], ...]`)
/// emits one-shot `Timer`s that `expand()`/`collapse()` the rail at the
/// same mock-clock beats the Compose runnables fire — Paparazzi can't
/// dispatch a pointer click on the rail's menu button, and the Slint
/// driver's `press` would ripple the button on the pre-motion frames, so
/// both sides are clock-driven instead.
fn rail_widget(s: &mut String, scene: &Scene, w: &Widget, i: usize) {
    let component = match w.kind.as_str() {
        "navigation-rail" => "NavigationRail",
        "wide-navigation-rail" => "WideNavigationRail",
        "modal-navigation-rail" => "ModalWideNavigationRail",
        other => panic!("unknown rail kind {other:?}"),
    };
    let mut p = String::new();
    writeln!(p, "        x: {}px;\n        y: {}px;", w.x as i64, w.y as i64).unwrap();
    if let Some(h) = w.height {
        writeln!(p, "        height: {}px;", h as i64).unwrap();
    }
    // The rail keeps the real 48dp `LocalMinimumInteractiveComponentSize`
    // (the Compose side restores it: upstream animates `itemMinHeight` to
    // the minimum with an underdamped spring and crashes on the 0dp
    // scene default) — the Slint default already matches.
    let expanded = w
        .slint_overrides
        .get("expanded")
        .and_then(|v| v.as_bool())
        .or(w.expanded);
    if let Some(v) = expanded {
        writeln!(p, "        expanded: {v};").unwrap();
    }
    if let Some(v) = w.hide_on_collapse {
        writeln!(p, "        hide-on-collapse: {v};").unwrap();
    }
    if let Some(a) = &w.arrangement {
        writeln!(
            p,
            "        arrangement: NavigationRailArrangement.{};",
            a.replace('-', "_")
        )
        .unwrap();
    }
    if let Some(v) = w.always_show_label {
        writeln!(p, "        always-show-label: {v};").unwrap();
    }
    if let Some(v) = w.selected_index {
        writeln!(p, "        current-index: {v};").unwrap();
    }
    if w.nav_icon.is_some() {
        writeln!(p, "        has-menu: true;").unwrap();
    }
    if let Some(f) = &w.fab_icon {
        writeln!(p, "        fab-icon: Icons.{f};").unwrap();
    }
    if !w.rail_items.is_empty() {
        writeln!(p, "        items: [").unwrap();
        for item in &w.rail_items {
            let mut entry = String::new();
            if let Some(icon) = &item.icon {
                entry.push_str(&format!("icon: Icons.{icon}, "));
            }
            if let Some(icon) = &item.selected_icon {
                entry.push_str(&format!("selected-icon: Icons.{icon}, "));
            }
            entry.push_str(&format!("text: {:?}", item.text));
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
        "    rail{i} := {component} {{\n{p}    }}\n",
    )
    .unwrap();
    if let Some(cover) = w.slint_overrides.get("cover").and_then(|v| v.as_object()) {
        let fill = cover
            .get("fill")
            .and_then(|v| v.as_str())
            .unwrap_or("on-primary")
            .replace('-', "_");
        let opacity = cover.get("opacity").and_then(|v| v.as_f64()).unwrap_or(0.1);
        writeln!(
            s,
            "    Rectangle {{\n        x: rail{i}.x;\n        y: rail{i}.y;\n        width: rail{i}.width;\n        height: rail{i}.height;\n        background: MaterialPalette.{fill};\n        opacity: {opacity};\n    }}\n"
        )
        .unwrap();
    }
    // `rail_width` is the only rail property the trace compares: forward
    // the layout's animated `rail-width` so `prop_value` can read it.
    if scene.trace_props.contains(&"rail_width".to_string()) {
        writeln!(s, "    out property <length> rail_width: rail{i}.rail-width;\n").unwrap();
    }
    if let Some(events) = scene.params.get("rail_events").and_then(|v| v.as_array()) {
        // A `Timer` can only fire when the mocked clock lands on or past its
        // deadline — the driver jumps the clock straight to each trace
        // sample, so a 30ms timer actually triggers at the next sample, one
        // frame late against Compose's `advanceTimeBy` events. `//ACTION=`
        // markers are dispatched at their exact `at_ms` instead: timed key
        // presses reach this FocusScope (the case's `forward-focus` target)
        // and call `expand()`/`collapse()` at the event's own tick, like
        // upstream's `emitPress` runnables.
        //
        // Bound properties evaluate lazily, so an `animate` installs its
        // spring when the binding next evaluates — at the next rendered
        // frame under the mocked clock, one sample after the event.
        // Reading the animated progress values in the handler installs
        // them on the event's tick like `animateDpAsState`.
        writeln!(s, "    in-out property <float> rail-event-sync;\n").unwrap();
        writeln!(s, "    forward-focus: rail-event-scope;").unwrap();
        for ev in events.iter() {
            let command = ev[0].as_str().unwrap_or("expand");
            let at = ev[1].as_i64().unwrap_or(0);
            let key = if command == "expand" { "e" } else { "c" };
            writeln!(s, "    //ACTION=key@{at}:{key}").unwrap();
        }
        writeln!(
            s,
            "    rail-event-scope := FocusScope {{\n        key-pressed(event) => {{\n            if event.text == \"e\" {{\n                rail{i}.expand();\n                rail-event-sync = rail{i}.expansion + rail{i}.width-expansion;\n            }}\n            if event.text == \"c\" {{\n                rail{i}.collapse();\n                rail-event-sync = rail{i}.expansion + rail{i}.width-expansion;\n            }}\n            accept\n        }}\n    }}\n"
        )
        .unwrap();
    }
}

