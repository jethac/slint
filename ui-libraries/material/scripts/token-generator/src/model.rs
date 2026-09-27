// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

//! Typed model for the parsed token members: turns the raw Kotlin initializer
//! expressions into values and resolves references between token objects.
//!
//! Every expression shape must be recognized; anything else is an error so
//! that new upstream token forms fail loudly instead of being dropped.

use crate::kotlin::{Member, ParseError, TokenFile};
use std::collections::BTreeMap;

/// A Kotlin initializer expression, as parsed.
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Num(f64),
    Dp(f64),
    Sp(f64),
    Rgb {
        r: i64,
        g: i64,
        b: i64,
    },
    Bezier([f64; 4]),
    /// `RoundedCornerShape(x.dp)` — one radius for all corners.
    RoundedUniform(f64),
    /// `RoundedCornerShape(topStart = .., topEnd = .., bottomEnd = .., bottomStart = ..)`
    RoundedNamed {
        top_start: f64,
        top_end: f64,
        bottom_end: f64,
        bottom_start: f64,
    },
    Circle,
    Rectangle,
    CornerSizeDp(f64),
    /// `FontFamily.SansSerif` → "SansSerif".
    FontFamily(String),
    FontWeight(u16),
    /// `ShapeToken(6)` — a key-token value class wrapping an id.
    KeyId {
        class: String,
        id: i64,
    },
    /// `XxxTokens.Member`.
    Ref {
        object: String,
        member: String,
    },
    /// A bare member name, resolved within the same object.
    LocalRef(String),
    /// `DefaultTextStyle.copy(fontFamily = .., fontWeight = .., fontSize = ..,
    /// lineHeight = .., letterSpacing = ..)`.
    TextStyle {
        font_family: Box<Expr>,
        font_weight: Box<Expr>,
        font_size: Box<Expr>,
        line_height: Box<Expr>,
        letter_spacing: Box<Expr>,
    },
}

/// A fully resolved value ready to emit.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// A dimension in dp; maps to logical pixels (px).
    Length(f64),
    /// A text-unit dimension in sp; scales with the platform text scale
    /// (emitted as `Npx * MaterialTextScale.factor`).
    Sp(f64),
    /// A unitless float such as a state-layer opacity or a spring constant.
    Number(f64),
    /// A duration in milliseconds (MotionTokens `Duration*` constants).
    DurationMs(f64),
    Color(u8, u8, u8),
    Easing([f64; 4]),
    /// A resolved corner shape. `full` is `CornerFull` (CircleShape): fully
    /// rounded at any size. `none` is `RectangleShape`.
    Shape(Shape),
    /// A `CornerSize` value without a shape (the `CornerValue*` tokens).
    CornerRadius(f64),
    FontFamily(String),
    FontWeight(u16),
    /// A reference to a color scheme role name (e.g. `OnSurface`).
    ColorRole(String),
    /// A reference to a `MaterialShapeTokens` member (e.g. `corner_medium`).
    ShapeRef(String),
    /// A reference to a `MaterialTypeScale` member (e.g. `label_large`).
    TypeStyleRef(String),
    /// A composed type style from `TypographyTokens`.
    TypeStyle(TypeStyle),
    /// A motion-scheme key (e.g. `DefaultSpatial`).
    SpringKey(String),
    /// A key-token id (only inside `*KeyTokens` objects).
    KeyId(i64),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Shape {
    pub top_start: f64,
    pub top_end: f64,
    pub bottom_end: f64,
    pub bottom_start: f64,
    /// `CircleShape`: fully rounded at any size (Slint `min(w,h)/2`).
    pub full: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypeStyle {
    pub font_family: String,
    pub font_weight: u16,
    pub font_size: f64,
    pub line_height: f64,
    pub letter_spacing: f64,
}

#[derive(Debug)]
pub struct ResolvedMember {
    pub name: String,
    /// Source line in the token file (kept for diagnostics).
    #[allow(dead_code)]
    pub line: usize,
    /// The initializer as written in the Kotlin source, for the PARITY.md
    /// "official value" column.
    pub rhs: String,
    /// The parsed form of `rhs` (kept for diagnostics).
    #[allow(dead_code)]
    pub expr: Expr,
    pub value: Value,
}

#[derive(Debug)]
pub struct ResolvedObject {
    pub name: String,
    pub file: String,
    pub members: Vec<ResolvedMember>,
}

#[derive(Debug)]
pub struct Library {
    pub objects: Vec<ResolvedObject>,
    /// All distinct `// VERSION:` markers seen across the token files.
    pub token_version: String,
}

fn err(file: &str, line: usize, message: impl Into<String>) -> ParseError {
    ParseError { file: file.to_string(), line, message: message.into() }
}

fn parse_number(s: &str) -> Result<f64, ()> {
    let s = s.strip_suffix('f').unwrap_or(s);
    s.parse::<f64>().map_err(|_| ())
}

/// Split `a = 1, b = 2` into `(name, value)` pairs, splitting only at commas
/// at parenthesis depth zero.
fn named_args(inner: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut depth: i64 = 0;
    let mut cur = String::new();
    for c in inner.chars() {
        match c {
            '(' => {
                depth += 1;
                cur.push(c);
            }
            ')' => {
                depth -= 1;
                cur.push(c);
            }
            ',' if depth == 0 => {
                if !cur.trim().is_empty() {
                    out.push(split_kv(cur.trim()));
                }
                cur.clear();
            }
            _ => cur.push(c),
        }
    }
    if !cur.trim().is_empty() {
        out.push(split_kv(cur.trim()));
    }
    out
}

fn split_kv(s: &str) -> (String, String) {
    match s.split_once('=') {
        Some((k, v)) => (k.trim().to_string(), v.trim().to_string()),
        None => (String::new(), s.trim().to_string()),
    }
}

/// Parse the initializer expression text into an `Expr`.
pub fn parse_expr(file: &str, line: usize, rhs: &str) -> Result<Expr, ParseError> {
    let e = |m: &str| err(file, line, m);
    let rhs = rhs.trim();
    let rhs = rhs.strip_suffix(',').unwrap_or(rhs).trim();

    // Number with unit suffix
    if let Some(num) = rhs.strip_suffix(".dp") {
        return parse_number(num).map(Expr::Dp).map_err(|_| e(&format!("bad dp value: {rhs}")));
    }
    if let Some(num) = rhs.strip_suffix(".sp") {
        return parse_number(num).map(Expr::Sp).map_err(|_| e(&format!("bad sp value: {rhs}")));
    }

    // Calls: Name(...)
    if let Some(open) = rhs.find('(') {
        if !rhs.ends_with(')') {
            return Err(e(&format!("unbalanced call: {rhs}")));
        }
        let callee = rhs[..open].trim();
        let inner = rhs[open + 1..rhs.len() - 1].trim();
        match callee {
            "Color" => {
                let mut rgb = [None; 3];
                for (k, v) in named_args(inner) {
                    let idx = match k.as_str() {
                        "red" => 0,
                        "green" => 1,
                        "blue" => 2,
                        "alpha" => continue, // not present today, but valid
                        _ => return Err(e(&format!("Color arg {k}: {rhs}"))),
                    };
                    rgb[idx] = Some(v.parse::<i64>().map_err(|_| e(&format!("Color arg: {rhs}")))?);
                }
                let [r, g, b] = rgb;
                return Ok(Expr::Rgb {
                    r: r.ok_or_else(|| e(&format!("Color missing red: {rhs}")))?,
                    g: g.ok_or_else(|| e(&format!("Color missing green: {rhs}")))?,
                    b: b.ok_or_else(|| e(&format!("Color missing blue: {rhs}")))?,
                });
            }
            "CubicBezierEasing" => {
                let nums: Result<Vec<f64>, ()> =
                    inner.split(',').map(|s| parse_number(s.trim())).collect();
                let nums = nums.map_err(|_| e(&format!("bad bezier: {rhs}")))?;
                let [a, b, c, d] =
                    nums.try_into().map_err(|_| e(&format!("bezier needs 4 args: {rhs}")))?;
                return Ok(Expr::Bezier([a, b, c, d]));
            }
            "RoundedCornerShape" => {
                let args = named_args(inner);
                if args.len() == 1 && args[0].0.is_empty() {
                    let num = args[0]
                        .1
                        .strip_suffix(".dp")
                        .and_then(|s| parse_number(s).ok())
                        .ok_or_else(|| e(&format!("bad RoundedCornerShape: {rhs}")))?;
                    return Ok(Expr::RoundedUniform(num));
                }
                let mut corners = [None; 4];
                for (k, v) in args {
                    let idx = match k.as_str() {
                        "topStart" => 0,
                        "topEnd" => 1,
                        "bottomEnd" => 2,
                        "bottomStart" => 3,
                        _ => return Err(e(&format!("RoundedCornerShape arg {k}: {rhs}"))),
                    };
                    let num = v
                        .strip_suffix(".dp")
                        .and_then(|s| parse_number(s).ok())
                        .ok_or_else(|| e(&format!("bad corner value: {rhs}")))?;
                    corners[idx] = Some(num);
                }
                let [ts, te, be, bs] = corners;
                return Ok(Expr::RoundedNamed {
                    top_start: ts.ok_or_else(|| e(&format!("missing topStart: {rhs}")))?,
                    top_end: te.ok_or_else(|| e(&format!("missing topEnd: {rhs}")))?,
                    bottom_end: be.ok_or_else(|| e(&format!("missing bottomEnd: {rhs}")))?,
                    bottom_start: bs.ok_or_else(|| e(&format!("missing bottomStart: {rhs}")))?,
                });
            }
            "CornerSize" => {
                let num = inner
                    .strip_suffix(".dp")
                    .and_then(|s| parse_number(s).ok())
                    .ok_or_else(|| e(&format!("bad CornerSize: {rhs}")))?;
                return Ok(Expr::CornerSizeDp(num));
            }
            "DefaultTextStyle.copy" => {
                let mut fields: BTreeMap<String, Expr> = BTreeMap::new();
                for (k, v) in named_args(inner) {
                    // `fontFamily = fontFamily ?: TypeScaleTokens.X` — the
                    // class parameter falls back to the scale value.
                    let v = v.trim_start_matches("fontFamily ?:").trim().to_string();
                    fields.insert(k, parse_expr(file, line, &v)?);
                }
                let mut take = |name: &str| -> Result<Expr, ParseError> {
                    fields.remove(name).ok_or_else(|| e(&format!("copy() missing {name}: {rhs}")))
                };
                let style = Expr::TextStyle {
                    font_family: Box::new(take("fontFamily")?),
                    font_weight: Box::new(take("fontWeight")?),
                    font_size: Box::new(take("fontSize")?),
                    line_height: Box::new(take("lineHeight")?),
                    letter_spacing: Box::new(take("letterSpacing")?),
                };
                return Ok(style);
            }
            callee => {
                // `ShapeToken(0)` and friends: key-token value classes end in `Token`.
                if let Some(cls) = callee.strip_suffix("Token") {
                    if !cls.is_empty()
                        && cls.chars().next().map(|c| c.is_uppercase()).unwrap_or(false)
                    {
                        let id =
                            inner.parse::<i64>().map_err(|_| e(&format!("bad token id: {rhs}")))?;
                        return Ok(Expr::KeyId { class: callee.to_string(), id });
                    }
                }
                return Err(e(&format!("unrecognized call {callee}(...): {rhs}")));
            }
        }
    }

    // Plain number (const val)
    if let Ok(n) = parse_number(rhs) {
        return Ok(Expr::Num(n));
    }

    // Keywords / singletons
    match rhs {
        "CircleShape" => return Ok(Expr::Circle),
        "RectangleShape" => return Ok(Expr::Rectangle),
        _ => {}
    }

    // FontFamily.SansSerif / FontWeight.Bold
    if let Some(fam) = rhs.strip_prefix("FontFamily.") {
        return Ok(Expr::FontFamily(fam.to_string()));
    }
    if let Some(w) = rhs.strip_prefix("FontWeight.") {
        let weight = match w {
            "Thin" => 100,
            "ExtraLight" => 200,
            "Light" => 300,
            "Normal" => 400,
            "Medium" => 500,
            "SemiBold" => 600,
            "Bold" => 700,
            "ExtraBold" => 800,
            "Black" => 900,
            _ => return Err(e(&format!("unknown FontWeight: {rhs}"))),
        };
        return Ok(Expr::FontWeight(weight));
    }

    // XxxTokens.Member
    if let Some((obj, member)) = rhs.split_once('.') {
        if !obj.is_empty()
            && !member.is_empty()
            && member.chars().all(|c| c.is_alphanumeric() || c == '_')
        {
            return Ok(Expr::Ref { object: obj.to_string(), member: member.to_string() });
        }
    }

    // Bare identifier = same-object reference
    if rhs.chars().all(|c| c.is_alphanumeric() || c == '_')
        && rhs.chars().next().map(|c| c.is_uppercase()).unwrap_or(false)
    {
        return Ok(Expr::LocalRef(rhs.to_string()));
    }

    Err(e(&format!("unrecognized expression: {rhs}")))
}

struct Ctx<'a> {
    /// object name -> (file, member name -> (member, expr))
    objects: BTreeMap<String, (String, BTreeMap<String, &'a Member>)>,
    exprs: BTreeMap<(usize, usize), Expr>,
    files: &'a [TokenFile],
}

fn font_family_name(fam: &str) -> String {
    match fam {
        "SansSerif" | "Default" => "sans-serif",
        "Serif" => "serif",
        "Monospace" => "monospace",
        "Cursive" => "cursive",
        other => other,
    }
    .to_string()
}

/// Parse every member's RHS into an `Expr`, then resolve references.
pub fn resolve(files: &[TokenFile]) -> Result<Library, ParseError> {
    let mut ctx_objs: BTreeMap<String, (String, BTreeMap<String, &Member>)> = BTreeMap::new();
    let mut exprs: BTreeMap<(usize, usize), Expr> = BTreeMap::new();
    let mut versions: Vec<String> = Vec::new();

    for (fi, f) in files.iter().enumerate() {
        if let Some(v) = &f.version {
            if !versions.contains(v) {
                versions.push(v.clone());
            }
        }
        for obj in &f.objects {
            let mut members = BTreeMap::new();
            for (mi, m) in obj.members.iter().enumerate() {
                let expr = parse_expr(&f.file_name, m.line, &m.rhs)?;
                exprs.insert((fi, mi), expr);
                members.insert(m.name.clone(), m);
            }
            if ctx_objs.insert(obj.name.clone(), (f.file_name.clone(), members)).is_some() {
                return Err(err(&f.file_name, 0, format!("duplicate object {}", obj.name)));
            }
        }
    }

    // File-scope helpers: only the two known helpers in TypographyTokens.kt.
    for f in files {
        for m in &f.file_members {
            if !matches!(m.name.as_str(), "DefaultTextStyle" | "DefaultLineHeightStyle") {
                return Err(err(
                    &f.file_name,
                    m.line,
                    format!("unexpected file-scope member {}", m.name),
                ));
            }
        }
    }

    let ctx = Ctx { objects: ctx_objs, exprs, files };

    let mut objects = Vec::new();
    for (fi, f) in files.iter().enumerate() {
        for obj in &f.objects {
            let mut members = Vec::new();
            for (mi, m) in obj.members.iter().enumerate() {
                let expr = ctx.exprs[&(fi, mi)].clone();
                let mut value = resolve_expr(&ctx, f, obj.name.as_str(), &expr, m.line, 0)?;
                // `const val DurationXxx = 50.0` in MotionTokens is a duration
                // in milliseconds.
                if m.name.starts_with("Duration") {
                    if let Value::Number(ms) = value {
                        value = Value::DurationMs(ms);
                    }
                }
                members.push(ResolvedMember {
                    name: m.name.clone(),
                    line: m.line,
                    rhs: m.rhs.clone(),
                    expr,
                    value,
                });
            }
            objects.push(ResolvedObject {
                name: obj.name.clone(),
                file: f.file_name.clone(),
                members,
            });
        }
    }

    Ok(Library { objects, token_version: versions.join(", ") })
}

fn resolve_expr(
    ctx: &Ctx,
    file: &TokenFile,
    current_obj: &str,
    expr: &Expr,
    line: usize,
    depth: usize,
) -> Result<Value, ParseError> {
    let e = |m: &str| err(&file.file_name, line, m);
    if depth > 16 {
        return Err(e("reference chain too deep (cyclic reference?)"));
    }
    Ok(match expr {
        Expr::Num(n) => Value::Number(*n),
        Expr::Dp(n) => Value::Length(*n),
        Expr::Sp(n) => Value::Sp(*n),
        Expr::Rgb { r, g, b } => Value::Color(
            u8::try_from(*r).map_err(|_| e("color channel > 255"))?,
            u8::try_from(*g).map_err(|_| e("color channel > 255"))?,
            u8::try_from(*b).map_err(|_| e("color channel > 255"))?,
        ),
        Expr::Bezier(b) => Value::Easing(*b),
        Expr::RoundedUniform(n) => Value::Shape(Shape {
            top_start: *n,
            top_end: *n,
            bottom_end: *n,
            bottom_start: *n,
            full: false,
        }),
        Expr::RoundedNamed { top_start, top_end, bottom_end, bottom_start } => {
            Value::Shape(Shape {
                top_start: *top_start,
                top_end: *top_end,
                bottom_end: *bottom_end,
                bottom_start: *bottom_start,
                full: false,
            })
        }
        Expr::Circle => Value::Shape(Shape {
            top_start: 0.0,
            top_end: 0.0,
            bottom_end: 0.0,
            bottom_start: 0.0,
            full: true,
        }),
        Expr::Rectangle => Value::Shape(Shape {
            top_start: 0.0,
            top_end: 0.0,
            bottom_end: 0.0,
            bottom_start: 0.0,
            full: false,
        }),
        Expr::CornerSizeDp(n) => Value::CornerRadius(*n),
        Expr::FontFamily(f) => Value::FontFamily(font_family_name(f)),
        Expr::FontWeight(w) => Value::FontWeight(*w),
        Expr::KeyId { id, .. } => Value::KeyId(*id),
        Expr::LocalRef(member) => {
            // Same-object reference: chase the member in this object.
            let (_f, members) = ctx
                .objects
                .get(current_obj)
                .ok_or_else(|| e(&format!("unknown object {current_obj}")))?;
            if !members.contains_key(member) {
                return Err(e(&format!("unknown member {current_obj}.{member}")));
            }
            // Resolve by name via the object table.
            return resolve_named_member(ctx, file, current_obj, member, line, depth);
        }
        Expr::Ref { object, member } => {
            match object.as_str() {
                "ColorSchemeKeyTokens" => {
                    require_member(ctx, file, object, member, line)?;
                    Value::ColorRole(member.clone())
                }
                "ShapeKeyTokens" => {
                    require_member(ctx, file, object, member, line)?;
                    // The key resolves to the same member name in ShapeTokens.
                    require_member(ctx, file, "ShapeTokens", member, line)?;
                    Value::ShapeRef(crate::emit::snake_case(member))
                }
                "TypographyKeyTokens" => {
                    require_member(ctx, file, object, member, line)?;
                    // Every role must exist in the type scale.
                    require_member(ctx, file, "TypeScaleTokens", &format!("{member}Size"), line)?;
                    Value::TypeStyleRef(crate::emit::snake_case(member))
                }
                "MotionSchemeKeyTokens" => {
                    require_member(ctx, file, object, member, line)?;
                    Value::SpringKey(member.clone())
                }
                "TypefaceTokens" => {
                    // Keep the member name (`Brand`, `Plain`, ...): the
                    // emitted value binds to a configurable theme family,
                    // not to the resolved `sans-serif` literal.
                    match resolve_named_member(ctx, file, object, member, line, depth)? {
                        Value::FontFamily(_) => Value::FontFamily(member.clone()),
                        v => v,
                    }
                }
                _ => return resolve_named_member(ctx, file, object, member, line, depth),
            }
        }
        Expr::TextStyle { font_family, font_weight, font_size, line_height, letter_spacing } => {
            let font_family =
                match resolve_expr(ctx, file, current_obj, font_family, line, depth + 1)? {
                    Value::FontFamily(f) => f,
                    _ => return Err(e("text style fontFamily is not a family")),
                };
            let font_weight =
                match resolve_expr(ctx, file, current_obj, font_weight, line, depth + 1)? {
                    Value::FontWeight(w) => w,
                    _ => return Err(e("text style fontWeight is not a weight")),
                };
            let font_size = match resolve_expr(ctx, file, current_obj, font_size, line, depth + 1)?
            {
                Value::Sp(v) => v,
                _ => return Err(e("text style fontSize is not an sp value")),
            };
            let line_height =
                match resolve_expr(ctx, file, current_obj, line_height, line, depth + 1)? {
                    Value::Sp(v) => v,
                    _ => return Err(e("text style lineHeight is not an sp value")),
                };
            let letter_spacing =
                match resolve_expr(ctx, file, current_obj, letter_spacing, line, depth + 1)? {
                    Value::Sp(v) => v,
                    _ => return Err(e("text style letterSpacing is not an sp value")),
                };
            Value::TypeStyle(TypeStyle {
                font_family,
                font_weight,
                font_size,
                line_height,
                letter_spacing,
            })
        }
    })
}

fn require_member(
    ctx: &Ctx,
    file: &TokenFile,
    object: &str,
    member: &str,
    line: usize,
) -> Result<(), ParseError> {
    match ctx.objects.get(object) {
        Some((_, members)) if members.contains_key(member) => Ok(()),
        _ => Err(err(&file.file_name, line, format!("unresolved reference {object}.{member}"))),
    }
}

/// Chase `object.member` through the object table to a terminal value.
fn resolve_named_member(
    ctx: &Ctx,
    file: &TokenFile,
    object: &str,
    member: &str,
    line: usize,
    depth: usize,
) -> Result<Value, ParseError> {
    let e = |m: &str| err(&file.file_name, line, m);
    let (target_file, members) =
        ctx.objects.get(object).ok_or_else(|| e(&format!("unknown object {object}")))?;
    let m = members.get(member).ok_or_else(|| e(&format!("unknown member {object}.{member}")))?;
    let expr = parse_expr(target_file, m.line, &m.rhs)?;
    let target = ctx
        .files
        .iter()
        .find(|f| &f.file_name == target_file)
        .ok_or_else(|| e(&format!("unknown file {target_file}")))?;
    resolve_expr(ctx, target, object, &expr, m.line, depth + 1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kotlin::parse_file;

    fn parse_one(name: &str, body: &str) -> TokenFile {
        let src = format!("// VERSION: 14_1_0\npackage x\n\ninternal object {name} {{\n{body}}}\n");
        parse_file(&format!("{name}.kt"), &src).unwrap()
    }

    #[test]
    fn expr_forms() {
        assert_eq!(parse_expr("f", 1, "24.0.dp").unwrap(), Expr::Dp(24.0));
        assert_eq!(parse_expr("f", 1, "0.5.sp").unwrap(), Expr::Sp(0.5));
        assert_eq!(parse_expr("f", 1, "-0.2.sp").unwrap(), Expr::Sp(-0.2));
        assert_eq!(parse_expr("f", 1, "0.16f").unwrap(), Expr::Num(0.16));
        assert_eq!(
            parse_expr("f", 1, "Color(red = 65, green = 14, blue = 11)").unwrap(),
            Expr::Rgb { r: 65, g: 14, b: 11 }
        );
        assert_eq!(
            parse_expr("f", 1, "CubicBezierEasing(0.2f, 0.0f, 0.0f, 1.0f)").unwrap(),
            Expr::Bezier([0.2, 0.0, 0.0, 1.0])
        );
        assert_eq!(parse_expr("f", 1, "CircleShape").unwrap(), Expr::Circle);
        assert_eq!(parse_expr("f", 1, "RectangleShape").unwrap(), Expr::Rectangle);
        assert_eq!(parse_expr("f", 1, "CornerSize(48.0.dp)").unwrap(), Expr::CornerSizeDp(48.0));
        assert_eq!(
            parse_expr("f", 1, "ShapeToken(6)").unwrap(),
            Expr::KeyId { class: "ShapeToken".to_string(), id: 6 }
        );
        assert_eq!(
            parse_expr("f", 1, "FontFamily.SansSerif").unwrap(),
            Expr::FontFamily("SansSerif".to_string())
        );
        assert_eq!(parse_expr("f", 1, "FontWeight.Medium").unwrap(), Expr::FontWeight(500));
        assert_eq!(
            parse_expr("f", 1, "ElevationTokens.Level3").unwrap(),
            Expr::Ref { object: "ElevationTokens".to_string(), member: "Level3".to_string() }
        );
        assert_eq!(parse_expr("f", 1, "Primary").unwrap(), Expr::LocalRef("Primary".to_string()));
    }

    #[test]
    fn fails_on_unknown_expr() {
        assert!(parse_expr("f", 1, "someFunction(1, 2)").is_err());
        assert!(parse_expr("f", 1, "a + b").is_err());
        assert!(parse_expr("f", 1, "\"a string\"").is_err());
    }

    #[test]
    fn resolves_references() {
        let elev =
            parse_one("ElevationTokens", "    inline val Level3: Dp\n        get() = 6.0.dp\n");
        let comp = parse_one(
            "DialogTokens",
            "    inline val ContainerElevation: Dp\n        get() = ElevationTokens.Level3\n",
        );
        let lib = resolve(&[elev, comp]).unwrap();
        let obj = lib.objects.iter().find(|o| o.name == "DialogTokens").unwrap();
        assert_eq!(obj.members[0].value, Value::Length(6.0));
    }

    #[test]
    fn resolves_key_token_reference() {
        let keys = parse_one(
            "ShapeKeyTokens",
            "    val CornerFull = ShapeToken(6)\n    val CornerMedium = ShapeToken(12)\n",
        );
        let shapes = parse_one(
            "ShapeTokens",
            "    val CornerFull = CircleShape\n    val CornerMedium = RoundedCornerShape(12.0.dp)\n",
        );
        let btn = parse_one(
            "ButtonSmallTokens",
            "    inline val ContainerShapeRound: ShapeToken\n        get() = ShapeKeyTokens.CornerFull\n    inline val ContainerShapeSquare: ShapeToken\n        get() = ShapeKeyTokens.CornerMedium\n",
        );
        let lib = resolve(&[keys, shapes, btn]).unwrap();
        let obj = lib.objects.iter().find(|o| o.name == "ButtonSmallTokens").unwrap();
        assert_eq!(obj.members[0].value, Value::ShapeRef("corner_full".to_string()));
        assert_eq!(obj.members[1].value, Value::ShapeRef("corner_medium".to_string()));
    }

    #[test]
    fn same_object_reference() {
        let light = parse_one(
            "ColorLightTokens",
            "    inline val Primary: Color\n        get() = PaletteTokens.Primary40\n    inline val SurfaceTint: Color\n        get() = Primary\n",
        );
        let palette = parse_one(
            "PaletteTokens",
            "    inline val Primary40: Color\n        get() = Color(red = 103, green = 80, blue = 164)\n",
        );
        let lib = resolve(&[light, palette]).unwrap();
        let obj = lib.objects.iter().find(|o| o.name == "ColorLightTokens").unwrap();
        assert_eq!(obj.members[1].value, Value::Color(103, 80, 164));
    }

    #[test]
    fn text_style_composite() {
        let scale = parse_one(
            "TypeScaleTokens",
            "    inline val BodyLargeSize: TextUnit\n        get() = 16.sp\n    inline val BodyLargeLineHeight: TextUnit\n        get() = 24.0.sp\n    inline val BodyLargeTracking: TextUnit\n        get() = 0.5.sp\n    inline val BodyLargeWeight: FontWeight\n        get() = TypefaceTokens.WeightRegular\n    inline val BodyLargeFont: FontFamily\n        get() = TypefaceTokens.Plain\n",
        );
        let face = parse_one(
            "TypefaceTokens",
            "    inline val WeightRegular: FontWeight\n        get() = FontWeight.Normal\n    inline val Plain: FontFamily\n        get() = FontFamily.SansSerif\n",
        );
        let typo = parse_file(
            "TypographyTokens.kt",
            "// VERSION: 14_1_0\npackage x\ninternal class TypographyTokens(val fontFamily: FontFamily? = null) {\n    val BodyLarge: TextStyle\n        inline get() =\n            DefaultTextStyle.copy(\n                fontFamily = fontFamily ?: TypeScaleTokens.BodyLargeFont,\n                fontWeight = TypeScaleTokens.BodyLargeWeight,\n                fontSize = TypeScaleTokens.BodyLargeSize,\n                lineHeight = TypeScaleTokens.BodyLargeLineHeight,\n                letterSpacing = TypeScaleTokens.BodyLargeTracking,\n            )\n}\n",
        )
        .unwrap();
        let lib = resolve(&[scale, face, typo]).unwrap();
        let obj = lib.objects.iter().find(|o| o.name == "TypographyTokens").unwrap();
        assert_eq!(
            obj.members[0].value,
            Value::TypeStyle(TypeStyle {
                // The TypefaceTokens member name survives so the emitter can
                // bind the theme-configurable family.
                font_family: "Plain".to_string(),
                font_weight: 400,
                font_size: 16.0,
                line_height: 24.0,
                letter_spacing: 0.5,
            })
        );
    }

    #[test]
    fn unknown_reference_fails() {
        let comp = parse_one(
            "WidgetTokens",
            "    inline val X: Dp\n        get() = NoSuchTokens.Missing\n",
        );
        assert!(resolve(&[comp]).is_err());
    }

    #[test]
    fn durations_are_marked() {
        let f = parse_one("MotionTokens", "    const val DurationShort1 = 50.0\n");
        let lib = resolve(&[f]).unwrap();
        assert_eq!(lib.objects[0].members[0].value, Value::DurationMs(50.0));
    }
}
