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
