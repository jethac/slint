// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

//! Emission of `.slint` token files under
//! `ui-libraries/material/src/ui/styling/generated/`.
//!
//! Token objects keep their upstream names (`ButtonMediumTokens` stays
//! `ButtonMediumTokens`) so PARITY.md maps every upstream token to one Slint
//! property. New container types that do not exist upstream carry a `Material`
//! prefix.

use crate::model::{Library, ResolvedObject, Shape, TypeStyle, Value};
use std::fmt::Write as _;

/// A file the generator produces, relative to the material crate root.
pub struct Output {
    pub rel_path: String,
    pub content: String,
}

/// Token objects emitted into `material_leaf_tokens.slint`: the styling
/// files (material_palette.slint and friends) need these but can't import
/// `material_component_tokens.slint`, which references `MaterialPalette`.
/// In this file `ColorRole` values emit as `ColorSchemeKeyTokens` role keys
/// instead of resolved palette colors.
const LEAF_OBJECTS: &[&str] = &["ScrimTokens"];

/// Token objects that get their own category file instead of landing in
/// `material_component_tokens.slint`.
const CATEGORY_FILES: &[&str] = &[
    "PaletteTokens",
    "ColorSchemeKeyTokens",
    "ColorLightTokens",
    "ColorDarkTokens",
    "ShapeTokens",
    "ShapeKeyTokens",
    "TypeScaleTokens",
    "TypefaceTokens",
    "TypographyTokens",
    "TypographyKeyTokens",
    "MotionTokens",
    "StandardMotionTokens",
    "ExpressiveMotionTokens",
    "MotionSchemeKeyTokens",
    "StateTokens",
    "ElevationTokens",
];

/// `ContainerShapeRound` -> `container_shape_round`, `XSmall` -> `x_small`,
/// `Level0` -> `level0`, `Neutral10` -> `neutral10`.
pub fn snake_case(name: &str) -> String {
    let chars: Vec<char> = name.chars().collect();
    let mut out = String::with_capacity(name.len() + 4);
    for (i, &c) in chars.iter().enumerate() {
        if c.is_uppercase() && i > 0 {
            let prev = chars[i - 1];
            let next_lower = chars.get(i + 1).map(|n| n.is_lowercase()).unwrap_or(false);
            if prev.is_lowercase() || prev.is_ascii_digit() || (prev.is_uppercase() && next_lower) {
                out.push('_');
            }
        }
        out.push(c.to_ascii_lowercase());
    }
    out
}

/// `0.9` -> `0.9`, `700.0` -> `700`, `-0.2` -> `-0.2`.
fn fmt_num(n: f64) -> String {
    if n.fract() == 0.0 && n.abs() < 1e15 { format!("{}", n as i64) } else { format!("{n}") }
}

fn fmt_shape(s: &Shape) -> String {
    format!(
        "{{ top_left: {}px, top_right: {}px, bottom_right: {}px, bottom_left: {}px, full: {} }}",
        fmt_num(s.top_start),
        fmt_num(s.top_end),
        fmt_num(s.bottom_end),
        fmt_num(s.bottom_start),
        s.full,
    )
}

/// `TypefaceTokens` member names bind to the theme's configurable
/// families; direct `FontFamily.*` values emit as string literals.
fn font_family_expr(fam: &str) -> String {
    match fam {
        "Brand" => "MaterialTheme.brand_family".to_string(),
        "Plain" => "MaterialTheme.plain_family".to_string(),
        other => format!("\"{other}\""),
    }
}

/// `N` (sp) -> `Npx * MaterialTextScale.factor`.
fn fmt_sp(n: f64) -> String {
    format!("{}px * MaterialTextScale.factor", fmt_num(n))
}

fn fmt_type_style(t: &TypeStyle) -> String {
    format!(
        "{{ font_family: {}, font_weight: {}, font_size: {}, line_height: {}, tracking: {} }}",
        font_family_expr(&t.font_family),
        t.font_weight,
        fmt_sp(t.font_size),
        fmt_sp(t.line_height),
        fmt_sp(t.letter_spacing),
    )
}

/// The Slint property type for a resolved value.
fn slint_type(v: &Value) -> &'static str {
    match v {
        Value::Length(_) | Value::Sp(_) | Value::CornerRadius(_) => "length",
        Value::Number(_) => "float",
        Value::DurationMs(_) => "duration",
        Value::Color(..) | Value::ColorRole(_) => "color",
        Value::Easing(_) => "easing",
        Value::Shape(_) | Value::ShapeRef(_) => "MaterialCornerShape",
        Value::FontFamily(_) => "string",
        Value::FontWeight(_) | Value::KeyId(_) => "int",
        Value::TypeStyle(_) | Value::TypeStyleRef(_) => "MaterialTypeStyle",
        Value::SpringKey(_) => "int",
    }
}

/// The Slint expression for a resolved value. References point at the
/// emitted globals so a single resolution pass keeps every consumer honest.
fn value_expr(v: &Value) -> String {
    match v {
        Value::Length(n) | Value::CornerRadius(n) => format!("{}px", fmt_num(*n)),
        Value::Sp(n) => fmt_sp(*n),
        Value::Number(n) => fmt_num(*n),
        Value::DurationMs(n) => format!("{}ms", fmt_num(*n)),
        Value::Color(r, g, b) => format!("#{r:02x}{g:02x}{b:02x}"),
        Value::Easing([a, b, c, d]) => format!(
            "cubic-bezier({}, {}, {}, {})",
            fmt_num(*a),
            fmt_num(*b),
            fmt_num(*c),
            fmt_num(*d)
        ),
        Value::Shape(s) => fmt_shape(s),
        Value::FontFamily(f) => font_family_expr(f),
        Value::FontWeight(w) => w.to_string(),
        Value::ColorRole(role) => format!("MaterialPalette.{}", snake_case(role)),
        Value::ShapeRef(name) => format!("ShapeTokens.{name}"),
        Value::TypeStyleRef(name) => format!("TypographyTokens.{name}"),
        Value::TypeStyle(t) => fmt_type_style(t),
        Value::SpringKey(name) => format!("MotionSchemeKeyTokens.{}", snake_case(name)),
        Value::KeyId(id) => id.to_string(),
    }
}

fn header(commit: &str, repo: &str, path: &str) -> String {
    let mut s = String::new();
    let _ = writeln!(s, "// Copyright © SixtyFPS GmbH <info@slint.dev>");
    // The lint must not parse the generated header text below:
    // REUSE-IgnoreStart
    let _ = writeln!(s, "// SPDX-License-Identifier: MIT");
    // REUSE-IgnoreEnd
    let _ = writeln!(s);
    let _ = writeln!(s, "// Generated by ui-libraries/material/scripts/token-generator.");
    let _ = writeln!(s, "// Upstream source: {repo} @ {commit}");
    let _ = writeln!(s, "//   {path}");
    let _ = writeln!(s, "// Do not edit. Regenerate with `cargo run -p material-token-generator`");
    let _ = writeln!(s, "// from the ui-libraries/material directory.");
    s
}

/// Emit globals. `roles_as_keys` (leaf file) emits `ColorRole` values as
/// `ColorSchemeKeyTokens` int keys typed `int` instead of resolved palette
/// colors, so the file stays free of `MaterialPalette` imports.
fn emit_globals(
    out: &mut String,
    objects: &[&ResolvedObject],
    roles_as_keys: bool,
) -> Result<(), String> {
    for obj in objects {
        let _ = writeln!(out, "export global {} {{", obj.name);
        for m in &obj.members {
            let (ty, expr) = match (&m.value, obj.name.as_str(), m.name.as_str()) {
                // The theme-configurable families in TypefaceTokens bind to
                // MaterialTheme, keyed by the member name.
                (Value::FontFamily(_), "TypefaceTokens", "Brand") => {
                    (slint_type(&m.value), "MaterialTheme.brand_family".to_string())
                }
                (Value::FontFamily(_), "TypefaceTokens", "Plain") => {
                    (slint_type(&m.value), "MaterialTheme.plain_family".to_string())
                }
                (Value::FontFamily(_), "TypefaceTokens", other) => {
                    return Err(format!(
                        "TypefaceTokens.{other}: new family token; map it to a MaterialTheme property"
                    ))
                }
                (Value::ColorRole(role), _, _) if roles_as_keys => {
                    ("int", format!("ColorSchemeKeyTokens.{}", snake_case(role)))
                }
                (v, _, _) => (slint_type(v), value_expr(v)),
            };
            let _ = writeln!(out, "    out property <{ty}> {}: {expr};", snake_case(&m.name),);
        }
        let _ = writeln!(out, "}}\n");
    }
    Ok(())
}

fn get<'a>(lib: &'a Library, name: &str) -> Result<&'a ResolvedObject, String> {
    lib.objects
        .iter()
        .find(|o| o.name == name)
        .ok_or_else(|| format!("missing token object {name}"))
}

/// The four named values of the upstream
/// `RippleThemeConfiguration.Focus.InsetRing` default (Ripple.kt).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FocusRing {
    pub outer_stroke_inset: f64,
    pub outer_stroke_width: f64,
    pub inner_stroke_inset: f64,
    pub inner_stroke_width: f64,
}

/// Extract the default inset focus ring values from `Ripple.kt`. The M3 focus
/// indicator is defined by `RippleThemeConfiguration.Focus.InsetRing`, not by
/// a token file, so it is parsed directly from the component source.
pub fn parse_focus_ring(text: &str) -> Result<FocusRing, String> {
    let start = text
        .find("InsetRing(")
        .ok_or_else(|| "Ripple.kt: cannot find InsetRing( initializer".to_string())?;
    let rest = &text[start + "InsetRing(".len()..];
    let end = rest.find(')').ok_or_else(|| "Ripple.kt: unbalanced InsetRing( call".to_string())?;
    let inner = &rest[..end];
    let mut vals = [None; 4];
    for part in inner.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let (k, v) =
            part.split_once('=').ok_or_else(|| format!("Ripple.kt: bad InsetRing arg `{part}`"))?;
        let num = v
            .trim()
            .strip_suffix(".dp")
            .and_then(|s| s.parse::<f64>().ok())
            .ok_or_else(|| format!("Ripple.kt: bad InsetRing value `{part}`"))?;
        let idx = match k.trim() {
            "outerStrokeInset" => 0,
            "outerStrokeWidth" => 1,
            "innerStrokeInset" => 2,
            "innerStrokeWidth" => 3,
            other => return Err(format!("Ripple.kt: unknown InsetRing arg `{other}`")),
        };
        vals[idx] = Some(num);
    }
    let [a, b, c, d] = vals;
    Ok(FocusRing {
        outer_stroke_inset: a.ok_or("Ripple.kt: missing outerStrokeInset")?,
        outer_stroke_width: b.ok_or("Ripple.kt: missing outerStrokeWidth")?,
        inner_stroke_inset: c.ok_or("Ripple.kt: missing innerStrokeInset")?,
        inner_stroke_width: d.ok_or("Ripple.kt: missing innerStrokeWidth")?,
    })
}

/// Emit every generated `.slint` file.
pub fn emit(
    lib: &Library,
    repo: &str,
    commit: &str,
    path: &str,
    focus: &FocusRing,
) -> Result<Vec<Output>, String> {
    let mut outputs = Vec::new();
    let head = header(commit, repo, path);

    // Colors.
    {
        let mut s = head.clone();
        let _ = writeln!(s);
        emit_globals(
            &mut s,
            &[
                get(lib, "PaletteTokens")?,
                get(lib, "ColorLightTokens")?,
                get(lib, "ColorDarkTokens")?,
                get(lib, "ColorSchemeKeyTokens")?,
            ],
            false,
        )?;
        outputs.push(Output {
            rel_path: "src/ui/styling/generated/material_color_tokens.slint".into(),
            content: s,
        });
    }

    // Shapes.
    {
        let mut s = head.clone();
        let _ = writeln!(s);
        let _ = writeln!(
            s,
            "/// Corner radii for one element. `full` marks `CornerFull`\n\
             /// (CircleShape upstream): fully rounded at any size, i.e.\n\
             /// `min(width, height) / 2` at the use site.\n\
             export struct MaterialCornerShape {{\n\
             \x20   top_left: length,\n\
             \x20   top_right: length,\n\
             \x20   bottom_right: length,\n\
             \x20   bottom_left: length,\n\
             \x20   full: bool,\n\
             }}\n"
        );
        emit_globals(&mut s, &[get(lib, "ShapeTokens")?, get(lib, "ShapeKeyTokens")?], false)?;
        outputs.push(Output {
            rel_path: "src/ui/styling/generated/material_shape_tokens.slint".into(),
            content: s,
        });
    }

    // Typography.
    {
        let mut s = head.clone();
        let _ = writeln!(s);
        let _ = writeln!(
            s,
            "/// One type-scale entry. `font_size`, `line_height` and\n\
             /// `tracking` are `sp` upstream and scale with\n\
             /// `MaterialTextScale.factor` (the platform text scale).\n\
             /// `font_family` binds to `MaterialTheme`'s configurable\n\
             /// brand/plain families.\n\
             export struct MaterialTypeStyle {{\n\
             \x20   font_family: string,\n\
             \x20   font_weight: int,\n\
             \x20   font_size: length,\n\
             \x20   line_height: length,\n\
             \x20   tracking: length,\n\
             }}\n"
        );
        let _ = writeln!(
            s,
            "/// Multiplies every `sp` token to follow the platform text\n\
             /// scale. The adaptive layer (#12) writes it.\n\
             export global MaterialTextScale {{\n\
             \x20   in-out property <float> factor: 1.0;\n\
             }}\n\
             \n\
             /// Theme-configurable brand and plain typeface families;\n\
             /// the font work (#7) replaces the defaults with the bundled\n\
             /// Google Sans Flex.\n\
             export global MaterialTheme {{\n\
             \x20   in-out property <string> brand_family: \"sans-serif\";\n\
             \x20   in-out property <string> plain_family: \"sans-serif\";\n\
             }}\n"
        );
        emit_globals(&mut s, &[get(lib, "TypefaceTokens")?, get(lib, "TypeScaleTokens")?], false)?;

        // Composed type scale: group the per-role parts of TypeScaleTokens.
        let scale = get(lib, "TypeScaleTokens")?;
        let mut roles: Vec<String> = Vec::new();
        for m in &scale.members {
            for suffix in ["Font", "Size", "LineHeight", "Tracking", "Weight"] {
                if let Some(role) = m.name.strip_suffix(suffix) {
                    if !roles.iter().any(|r| r == role) {
                        roles.push(role.to_string());
                    }
                }
            }
        }
        let _ = writeln!(s, "export global MaterialTypeScale {{");
        for role in &roles {
            let style = compose_style(scale, role)
                .ok_or_else(|| format!("TypeScaleTokens missing a part of {role}"))?;
            let _ = writeln!(
                s,
                "    out property <MaterialTypeStyle> {}: {};",
                snake_case(role),
                fmt_type_style(&style),
            );
        }
        let _ = writeln!(s, "}}\n");

        // TypographyTokens resolves to the same styles; emit it as references
        // into MaterialTypeScale so the two stay identical by construction.
        let typo = get(lib, "TypographyTokens")?;
        let _ = writeln!(s, "export global TypographyTokens {{");
        for m in &typo.members {
            let composed = match &m.value {
                Value::TypeStyle(t) => t,
                _ => return Err(format!("TypographyTokens.{} is not a TextStyle", m.name)),
            };
            let scale_style = compose_style(scale, &m.name)
                .ok_or_else(|| format!("no TypeScaleTokens entry for {}", m.name))?;
            if &scale_style != composed {
                return Err(format!("TypographyTokens.{} differs from the type scale", m.name));
            }
            let _ = writeln!(
                s,
                "    out property <MaterialTypeStyle> {}: MaterialTypeScale.{};",
                snake_case(&m.name),
                snake_case(&m.name),
            );
        }
        let _ = writeln!(s, "}}\n");
        emit_globals(&mut s, &[get(lib, "TypographyKeyTokens")?], false)?;

        outputs.push(Output {
            rel_path: "src/ui/styling/generated/material_typography_tokens.slint".into(),
            content: s,
        });
    }

    // Motion: spring specs and the standard/expressive schemes.
    {
        let mut s = head.clone();
        let _ = writeln!(s);
        let _ = writeln!(
            s,
            "/// One spring in the M3 motion scheme: a damping ratio and a\n\
             /// stiffness, consumed by spring animation support (#5).\n\
             export struct MaterialSpringSpec {{\n\
             \x20   damping_ratio: float,\n\
             \x20   stiffness: float,\n\
             }}\n\
             \n\
             /// The six springs of one motion scheme.\n\
             export struct MaterialMotionSchemeSpec {{\n\
             \x20   default_spatial: MaterialSpringSpec,\n\
             \x20   fast_spatial: MaterialSpringSpec,\n\
             \x20   slow_spatial: MaterialSpringSpec,\n\
             \x20   default_effects: MaterialSpringSpec,\n\
             \x20   fast_effects: MaterialSpringSpec,\n\
             \x20   slow_effects: MaterialSpringSpec,\n\
             }}\n"
        );

        let scheme = |obj: &ResolvedObject| -> Result<String, String> {
            let mut fields: Vec<(String, [Option<f64>; 2])> = Vec::new();
            for m in &obj.members {
                let n = m
                    .name
                    .strip_prefix("Spring")
                    .ok_or_else(|| format!("{}: bad member {}", obj.name, m.name))?;
                let (variant, which) = if let Some(v) = n.strip_suffix("Damping") {
                    (v, 0)
                } else if let Some(v) = n.strip_suffix("Stiffness") {
                    (v, 1)
                } else {
                    return Err(format!("{}: bad spring member {}", obj.name, m.name));
                };
                let value = match m.value {
                    Value::Number(v) => v,
                    _ => return Err(format!("{}.{} is not a number", obj.name, m.name)),
                };
                let entry = match fields.iter_mut().find(|(k, _)| k == variant) {
                    Some(e) => e,
                    None => {
                        fields.push((variant.to_string(), [None, None]));
                        fields.last_mut().unwrap()
                    }
                };
                entry.1[which] = Some(value);
            }
            let mut lit = String::from("{ ");
            for (i, (variant, [damping, stiffness])) in fields.iter().enumerate() {
                if i > 0 {
                    lit.push_str(", ");
                }
                let _ = write!(
                    lit,
                    "{}: {{ damping_ratio: {}, stiffness: {} }}",
                    snake_case(variant),
                    fmt_num(damping.ok_or_else(|| format!("missing damping for {variant}"))?),
                    fmt_num(stiffness.ok_or_else(|| format!("missing stiffness for {variant}"))?),
                );
            }
            lit.push_str(" }");
            Ok(lit)
        };
        let standard = scheme(get(lib, "StandardMotionTokens")?)?;
        let expressive = scheme(get(lib, "ExpressiveMotionTokens")?)?;
        let _ = writeln!(s, "export global MaterialMotion {{");
        let _ = writeln!(s, "    out property <MaterialMotionSchemeSpec> standard: {standard};");
        let _ =
            writeln!(s, "    out property <MaterialMotionSchemeSpec> expressive: {expressive};");
        let _ = writeln!(s, "}}\n");
        emit_globals(
            &mut s,
            &[
                get(lib, "MotionTokens")?,
                get(lib, "StandardMotionTokens")?,
                get(lib, "ExpressiveMotionTokens")?,
                get(lib, "MotionSchemeKeyTokens")?,
            ],
            false,
        )?;
        outputs.push(Output {
            rel_path: "src/ui/styling/generated/material_motion_tokens.slint".into(),
            content: s,
        });
    }

    // State layer opacities + focus indicator ring.
    {
        let mut s = head.clone();
        let _ = writeln!(s);
        emit_globals(&mut s, &[get(lib, "StateTokens")?], false)?;
        let _ = writeln!(
            s,
            "/// The M3 focus indicator ring. Upstream these defaults live in\n\
             /// `RippleThemeConfiguration.Focus.InsetRing` (Ripple.kt), not in a\n\
             /// token file; the generator parses them from there.\n\
             export struct MaterialFocusRing {{\n\
             \x20   outer_stroke_inset: length,\n\
             \x20   outer_stroke_width: length,\n\
             \x20   inner_stroke_inset: length,\n\
             \x20   inner_stroke_width: length,\n\
             }}\n\
             \n\
             export global MaterialFocusIndicator {{\n\
             \x20   out property <MaterialFocusRing> ring: {{\n\
             \x20       outer_stroke_inset: {}px,\n\
             \x20       outer_stroke_width: {}px,\n\
             \x20       inner_stroke_inset: {}px,\n\
             \x20       inner_stroke_width: {}px,\n\
             \x20   }};\n\
             }}",
            fmt_num(focus.outer_stroke_inset),
            fmt_num(focus.outer_stroke_width),
            fmt_num(focus.inner_stroke_inset),
            fmt_num(focus.inner_stroke_width),
        );
        let _ = writeln!(s);
        outputs.push(Output {
            rel_path: "src/ui/styling/generated/material_state_tokens.slint".into(),
            content: s,
        });
    }

    // Elevation.
    {
        let mut s = head.clone();
        let _ = writeln!(s);
        emit_globals(&mut s, &[get(lib, "ElevationTokens")?], false)?;
        outputs.push(Output {
            rel_path: "src/ui/styling/generated/material_elevation_tokens.slint".into(),
            content: s,
        });
    }

    // Leaf objects: consumed by the styling files, so the file must not
    // import MaterialPalette (the palette imports this file). `ColorRole`
    // members emit as `ColorSchemeKeyTokens` role keys.
    {
        let mut s = head.clone();
        let leaf_objs: Vec<&ResolvedObject> = LEAF_OBJECTS
            .iter()
            .map(|name| get(lib, name))
            .collect::<Result<_, _>>()?;
        let needs_keys = leaf_objs
            .iter()
            .any(|o| o.members.iter().any(|m| matches!(m.value, Value::ColorRole(_))));
        let _ = writeln!(s);
        if needs_keys {
            let _ = writeln!(
                s,
                "import {{ ColorSchemeKeyTokens }} from \"./material_color_tokens.slint\";"
            );
            let _ = writeln!(s);
        }
        emit_globals(&mut s, &leaf_objs, true)?;
        outputs.push(Output {
            rel_path: "src/ui/styling/generated/material_leaf_tokens.slint".into(),
            content: s,
        });
    }

    // Every remaining per-component token object.
    {
        let mut s = head.clone();
        let mut needs_palette = false;
        let mut needs_shape = false;
        let mut needs_type_style = false;
        let mut needs_typo = false;
        let mut needs_scale = false;
        let mut needs_theme = false;
        let mut needs_motion = false;
        let component_objs: Vec<&ResolvedObject> = lib
            .objects
            .iter()
            .filter(|o| {
                !CATEGORY_FILES.contains(&o.name.as_str())
                    && !LEAF_OBJECTS.contains(&o.name.as_str())
            })
            .collect();
        for obj in &component_objs {
            for m in &obj.members {
                match m.value {
                    Value::ColorRole(_) => needs_palette = true,
                    Value::Shape(_) | Value::ShapeRef(_) => needs_shape = true,
                    Value::TypeStyle(_) => {
                        needs_type_style = true;
                        needs_scale = true;
                        needs_theme = true;
                    }
                    Value::TypeStyleRef(_) => {
                        needs_type_style = true;
                        needs_typo = true;
                    }
                    Value::Sp(_) => needs_scale = true,
                    Value::FontFamily(_) => needs_theme = true,
                    Value::SpringKey(_) => needs_motion = true,
                    _ => {}
                }
            }
        }
        let _ = writeln!(s);
        if needs_palette {
            let _ = writeln!(s, "import {{ MaterialPalette }} from \"../material_palette.slint\";");
        }
        if needs_shape {
            let _ = writeln!(
                s,
                "import {{ MaterialCornerShape, ShapeTokens }} from \"./material_shape_tokens.slint\";"
            );
        }
        {
            let mut names: Vec<&str> = Vec::new();
            if needs_scale {
                names.push("MaterialTextScale");
            }
            if needs_theme {
                names.push("MaterialTheme");
            }
            if needs_type_style {
                names.push("MaterialTypeStyle");
            }
            if needs_typo {
                names.push("TypographyTokens");
            }
            if !names.is_empty() {
                let _ = writeln!(
                    s,
                    "import {{ {} }} from \"./material_typography_tokens.slint\";",
                    names.join(", ")
                );
            }
        }
        if needs_motion {
            let _ = writeln!(
                s,
                "import {{ MotionSchemeKeyTokens }} from \"./material_motion_tokens.slint\";"
            );
        }
        let _ = writeln!(s);
        emit_globals(&mut s, &component_objs, false)?;
        outputs.push(Output {
            rel_path: "src/ui/styling/generated/material_component_tokens.slint".into(),
            content: s,
        });
    }

    // Index: named re-exports for every generated global/struct.
    // `export *` is allowed only once per file, so `material.slint` pulls all
    // generated tokens in through this one index.
    {
        let mut s = head.clone();
        let _ = writeln!(s);
        for o in &outputs {
            let file =
                o.rel_path.rsplit('/').next().ok_or_else(|| format!("bad path {}", o.rel_path))?;
            let mut names: Vec<&str> = Vec::new();
            for line in o.content.lines() {
                let t = line.trim_start();
                for kw in ["export global ", "export struct "] {
                    if let Some(rest) = t.strip_prefix(kw) {
                        let end = rest
                            .find(|c: char| !(c.is_alphanumeric() || c == '_'))
                            .unwrap_or(rest.len());
                        names.push(&rest[..end]);
                    }
                }
            }
            if names.is_empty() {
                return Err(format!("{file}: no exported types found"));
            }
            let _ = writeln!(s, "export {{ {} }} from \"./{file}\";", names.join(", "));
        }
        outputs.push(Output {
            rel_path: "src/ui/styling/generated/material_tokens.slint".into(),
            content: s,
        });
    }

    Ok(outputs)
}

/// Compose a `TypeStyle` for `role` from the flat `TypeScaleTokens` members.
fn compose_style(scale: &ResolvedObject, role: &str) -> Option<TypeStyle> {
    let part = |suffix: &str| -> Option<&Value> {
        scale.members.iter().find(|m| m.name == format!("{role}{suffix}")).map(|m| &m.value)
    };
    Some(TypeStyle {
        font_family: match part("Font")? {
            Value::FontFamily(f) => f.clone(),
            _ => return None,
        },
        font_weight: match part("Weight")? {
            Value::FontWeight(w) => *w,
            _ => return None,
        },
        font_size: match part("Size")? {
            Value::Sp(v) => *v,
            _ => return None,
        },
        line_height: match part("LineHeight")? {
            Value::Sp(v) => *v,
            _ => return None,
        },
        letter_spacing: match part("Tracking")? {
            Value::Sp(v) => *v,
            _ => return None,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snake_case_works() {
        assert_eq!(snake_case("ContainerShapeRound"), "container_shape_round");
        assert_eq!(snake_case("XSmall"), "x_small");
        assert_eq!(snake_case("XLarge"), "x_large");
        assert_eq!(snake_case("Level0"), "level0");
        assert_eq!(snake_case("Neutral10"), "neutral10");
        assert_eq!(snake_case("BodyLargeEmphasized"), "body_large_emphasized");
        assert_eq!(snake_case("OnSurfaceVariant"), "on_surface_variant");
        assert_eq!(snake_case("DurationExtraLong1"), "duration_extra_long1");
        assert_eq!(snake_case("URLValue"), "url_value");
    }

    #[test]
    fn focus_ring_parse() {
        let text = "RippleThemeConfiguration.Focus.InsetRing(\n    outerStrokeInset = 0.dp,\n    outerStrokeWidth = 2.dp,\n    innerStrokeInset = 1.dp,\n    innerStrokeWidth = 3.dp,\n)";
        let r = parse_focus_ring(text).unwrap();
        assert_eq!(r.outer_stroke_width, 2.0);
        assert_eq!(r.inner_stroke_inset, 1.0);
    }
}
