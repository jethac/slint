// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

//! Emits the `MaterialShapes` global: all 35 `MaterialShapes.*` presets from
//! `MaterialShapes.kt`, translated member-by-member into `Shapes.*` calls.
//!
//! `MaterialShapes.kt` is a data file written in a small Kotlin DSL: `private
//! val` constants (`CornerRounding`, `Matrix`), `internal fun` constructors
//! and `public val` getters inside `public companion object`. This module
//! parses exactly that grammar and fails loudly on anything else, so an
//! upstream syntax change can never silently drift the generated values.

use std::collections::HashMap;
use std::fmt::Write as _;

// ------------------------------------------------------------- the grammar

/// A `CornerRounding(radius[, smoothing])` value; `Unrounded` is `(0, 0)`.
#[derive(Debug, Clone, Copy)]
struct Cr {
    radius: f64,
    smoothing: f64,
}

/// A transform applied by `.transformed(matrix)`.
#[derive(Debug, Clone, Copy)]
enum Mat {
    /// `Matrix().apply { rotateZ(deg) }`
    Rot(f64),
    /// `Matrix().apply { scale(x, y) }`
    Scale(f64, f64),
}

/// A numeric argument: a literal or a reference to an `internal fun`
/// parameter (the only upstream use is `circle(numVertices = numVertices)`).
#[derive(Debug, Clone)]
enum Num {
    Lit(f64),
    Ref(String),
}

impl Num {
    /// A literal; param references are only valid for `numVertices` in
    /// `RoundedPolygon.circle`, resolved from the call env.
    fn lit(&self) -> Result<f64, String> {
        match self {
            Num::Lit(v) => Ok(*v),
            Num::Ref(p) => Err(format!("param `{p}` is not a literal")),
        }
    }
}

/// A `RoundedPolygon`-producing expression, restricted to the constructs
/// `MaterialShapes.kt` uses.
#[derive(Debug)]
enum Expr {
    /// `customPolygon(listOf(PointNRound(Offset(x, y)[, cr]), ..), reps[, mirroring])`
    Custom { points: Vec<(f64, f64, Cr)>, reps: i64, mirror: bool },
    /// `RoundedPolygon(numVertices = n, rounding|perVertexRounding = ..)`
    Polygon { n: f64, rounding: Option<Cr>, per_vertex: Option<Vec<Cr>> },
    /// `RoundedPolygon.rectangle(w, h, rounding|perVertexRounding)`
    Rect { w: f64, h: f64, rounding: Option<Cr>, per_vertex: Option<Vec<Cr>> },
    /// `RoundedPolygon.circle([numVertices])`
    Circle { n: Option<Num> },
    /// `RoundedPolygon.star(numVerticesPerRadius, innerRadius, rounding)`
    Star { n: f64, inner: f64, rounding: Cr },
    /// `<expr>.transformed(matrix)`
    Transformed(Box<Expr>, Mat),
    /// `<expr>.normalized()`
    Normalized(Box<Expr>),
    /// A call to another `internal fun`, e.g. `circle()` / `circle(numVertices = 10)`.
    Call { name: String, args: Vec<(Option<String>, Num)> },
}

struct Fun {
    /// Parameter name -> default (only `circle(numVertices: Int = 10)` has one).
    defaults: Vec<(String, f64)>,
    body: Expr,
}

// ------------------------------------------------------------ text helpers

/// Strip `//` line comments, `/* */` block comments and `@..` annotations,
/// keeping `/** KDoc */` bodies in the returned map keyed by the byte offset
/// of the declaration that follows them.
fn strip_comments(text: &str) -> (String, Vec<(usize, String)>) {
    let chars: Vec<char> = text.char_indices().map(|(_, c)| c).collect();
    let mut out = String::with_capacity(text.len());
    let mut docs = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '/' && chars.get(i + 1) == Some(&'/') {
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
            continue;
        }
        if chars[i] == '/' && chars.get(i + 1) == Some(&'*') {
            let kdoc = chars.get(i + 2) == Some(&'*') && chars.get(i + 3) != Some(&'/');
            let start = i + if kdoc { 3 } else { 2 };
            let mut j = start;
            while j + 1 < chars.len() && !(chars[j] == '*' && chars[j + 1] == '/') {
                j += 1;
            }
            if kdoc {
                docs.push((out.len(), chars[start..j].iter().collect()));
            }
            // keep alignment with `out` offsets stable by emitting whitespace
            out.push('\n');
            i = j + 2;
            continue;
        }
        out.push(chars[i]);
        i += 1;
    }
    (out, docs)
}

/// The index just past the `}` matching the `{` at `open`.
fn matching_brace(text: &str, open: usize) -> Result<usize, String> {
    let bytes = text.as_bytes();
    let mut depth = 0;
    let mut i = open;
    while i < bytes.len() {
        match bytes[i] {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Ok(i + 1);
                }
            }
            _ => {}
        }
        i += 1;
    }
    Err("unbalanced braces".into())
}

// ---------------------------------------------------------- expr tokenizer

#[derive(Debug, Clone, PartialEq)]
enum T {
    Id(String),
    Num(f64),
    LP,
    RP,
    LB,
    RB,
    Comma,
    Dot,
    Eq,
    Colon,
    Q,
    Minus,
    True,
    False,
}

fn expr_tokens(text: &str) -> Result<Vec<T>, String> {
    let chars: Vec<char> = text.chars().collect();
    let mut toks = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        if c.is_ascii_alphabetic() || c == '_' {
            let s = i;
            while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            let id: String = chars[s..i].iter().collect();
            toks.push(match id.as_str() {
                "true" => T::True,
                "false" => T::False,
                _ => T::Id(id),
            });
            continue;
        }
        if c.is_ascii_digit() || (c == '.' && chars.get(i + 1).is_some_and(|n| n.is_ascii_digit()))
        {
            let s = i;
            while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                i += 1;
            }
            if chars.get(i) == Some(&'f') || chars.get(i) == Some(&'F') {
                i += 1;
            }
            let raw: String = chars[s..i].iter().filter(|c| c.is_ascii_digit() || **c == '.').collect();
            toks.push(T::Num(
                raw.parse::<f64>().map_err(|e| format!("bad number `{raw}`: {e}"))?,
            ));
            continue;
        }
        i += 1;
        toks.push(match c {
            '(' => T::LP,
            ')' => T::RP,
            '{' => T::LB,
            '}' => T::RB,
            ',' => T::Comma,
            '.' => T::Dot,
            '=' => T::Eq,
            ':' => T::Colon,
            '?' => T::Q,
            '-' => T::Minus,
            other => return Err(format!("unexpected char `{other}` in expression")),
        });
    }
    Ok(toks)
}

// ------------------------------------------------------------------- parse

struct P<'a> {
    t: &'a [T],
    i: usize,
    roundings: &'a HashMap<String, Cr>,
    /// matrices in scope: file-level `rotateNegN` plus fun-local `val m`
    matrices: &'a HashMap<String, Mat>,
}

impl<'a> P<'a> {
    fn peek(&self) -> Option<&T> {
        self.t.get(self.i)
    }
    fn at(&self, off: usize) -> Option<&T> {
        self.t.get(self.i + off)
    }
    fn next(&mut self) -> Result<T, String> {
        let t = self.t.get(self.i).cloned().ok_or("unexpected end")?;
        self.i += 1;
        Ok(t)
    }
    fn want(&mut self, t: T) -> Result<(), String> {
        let got = self.next()?;
        if got == t {
            Ok(())
        } else {
            Err(format!("expected {t:?}, got {got:?}"))
        }
    }
    fn id(&mut self) -> Result<String, String> {
        match self.next()? {
            T::Id(s) => Ok(s),
            o => Err(format!("expected identifier, got {o:?}")),
        }
    }
    fn num(&mut self) -> Result<f64, String> {
        let neg = matches!(self.peek(), Some(T::Minus));
        if neg {
            self.i += 1;
        }
        match self.next()? {
            T::Num(n) => Ok(if neg { -n } else { n }),
            o => Err(format!("expected number, got {o:?}")),
        }
    }
    fn comma(&mut self) {
        if matches!(self.peek(), Some(T::Comma)) {
            self.i += 1;
        }
    }

    /// `CornerRounding(radius = r[, s])` | `CornerRounding(r[, s])` |
    /// `cornerRound20` | `CornerRounding.Unrounded`.
    fn cr(&mut self) -> Result<Cr, String> {
        let name = self.id()?;
        if name != "CornerRounding" {
            return self
                .roundings
                .get(&name)
                .copied()
                .ok_or_else(|| format!("unknown corner rounding `{name}`"));
        }
        match self.peek() {
            Some(T::Dot) => {
                self.i += 1;
                if self.id()? == "Unrounded" {
                    Ok(Cr { radius: 0., smoothing: 0. })
                } else {
                    Err("only CornerRounding.Unrounded is supported".into())
                }
            }
            Some(T::LP) => {
                self.i += 1;
                let mut args = Vec::new();
                while !matches!(self.peek(), Some(T::RP)) {
                    if matches!(self.at(1), Some(T::Eq)) {
                        self.id()?;
                        self.want(T::Eq)?;
                    }
                    args.push(self.num()?);
                    self.comma();
                }
                self.want(T::RP)?;
                match args.as_slice() {
                    [r] => Ok(Cr { radius: *r, smoothing: 0. }),
                    [r, s] => Ok(Cr { radius: *r, smoothing: *s }),
                    o => Err(format!("CornerRounding() got {} args", o.len())),
                }
            }
            _ => Err("expected CornerRounding(..)".into()),
        }
    }

    /// `Matrix().apply { rotateZ(d)|scale(x,y) }` or a name from `matrices`.
    fn matrix(&mut self) -> Result<Mat, String> {
        if let Some(T::Id(name)) = self.peek() {
            if name != "Matrix" {
                let m = self
                    .matrices
                    .get(name)
                    .copied()
                    .ok_or_else(|| format!("unknown matrix `{name}`"))?;
                self.i += 1;
                return Ok(m);
            }
        }
        self.want(T::Id("Matrix".into()))?;
        self.want(T::LP)?;
        self.want(T::RP)?;
        self.want(T::Dot)?;
        self.want(T::Id("apply".into()))?;
        self.want(T::LB)?;
        let kind = self.id()?;
        self.want(T::LP)?;
        let mut args = Vec::new();
        while !matches!(self.peek(), Some(T::RP)) {
            if matches!(self.at(1), Some(T::Eq)) {
                self.id()?;
                self.want(T::Eq)?;
            }
            args.push(self.num()?);
            self.comma();
        }
        self.want(T::RP)?;
        self.want(T::RB)?;
        match (kind.as_str(), args.as_slice()) {
            ("rotateZ", [d]) => Ok(Mat::Rot(*d)),
            ("scale", [x, y]) => Ok(Mat::Scale(*x, *y)),
            _ => Err(format!("unsupported Matrix().apply {{ {kind}({args:?}) }}")),
        }
    }

    /// `listOf(PointNRound(Offset(x, y)[, cr]), ..)`
    fn point_list(&mut self) -> Result<Vec<(f64, f64, Cr)>, String> {
        self.want(T::Id("listOf".into()))?;
        self.want(T::LP)?;
        let mut points = Vec::new();
        while !matches!(self.peek(), Some(T::RP)) {
            self.want(T::Id("PointNRound".into()))?;
            self.want(T::LP)?;
            self.want(T::Id("Offset".into()))?;
            self.want(T::LP)?;
            let x = self.num()?;
            self.want(T::Comma)?;
            let y = self.num()?;
            self.want(T::RP)?;
            let r = if matches!(self.peek(), Some(T::Comma)) {
                self.i += 1;
                match self.peek() {
                    Some(T::RP) => Cr { radius: 0., smoothing: 0. },
                    _ => self.cr()?,
                }
            } else {
                Cr { radius: 0., smoothing: 0. }
            };
            self.want(T::RP)?; // close PointNRound
            points.push((x, y, r));
            self.comma();
        }
        self.want(T::RP)?;
        Ok(points)
    }

    /// `rounding = cr` or `perVertexRounding = listOf(..)` (already at the name).
    fn rounding_arg(&mut self) -> Result<(Option<Cr>, Option<Vec<Cr>>), String> {
        let name = self.id()?;
        self.want(T::Eq)?;
        match name.as_str() {
            "rounding" => Ok((Some(self.cr()?), None)),
            "perVertexRounding" => {
                self.want(T::Id("listOf".into()))?;
                self.want(T::LP)?;
                let mut crs = Vec::new();
                while !matches!(self.peek(), Some(T::RP)) {
                    crs.push(self.cr()?);
                    self.comma();
                }
                self.want(T::RP)?;
                Ok((None, Some(crs)))
            }
            other => Err(format!("expected rounding/perVertexRounding, got `{other}`")),
        }
    }

    /// `reps = 2` / `mirroring = true` / `numVertices = numVertices` / a bare
    /// number inside a call's args.
    fn call_arg(&mut self) -> Result<(Option<String>, Option<Num>, Option<bool>), String> {
        if let Some(T::Id(id)) = self.peek() {
            if matches!(self.at(1), Some(T::Eq)) && id != "listOf" && id != "CornerRounding" {
                let name = self.id()?;
                self.want(T::Eq)?;
                return match self.peek() {
                    Some(T::True) | Some(T::False) => {
                        let b = matches!(self.next()?, T::True);
                        Ok((Some(name), None, Some(b)))
                    }
                    Some(T::Id(p)) => {
                        let p = p.clone();
                        self.i += 1;
                        Ok((Some(name), Some(Num::Ref(p)), None))
                    }
                    _ => Ok((Some(name), Some(Num::Lit(self.num()?)), None)),
                };
            }
        }
        Ok((None, Some(Num::Lit(self.num()?)), None))
    }

    /// `customPolygon(listOf(..), reps[, mirroring = true])`.
    fn custom_polygon(&mut self) -> Result<Expr, String> {
        self.want(T::LP)?;
        let (mut points, mut reps, mut mirror) = (None, None, false);
        while !matches!(self.peek(), Some(T::RP)) {
            if matches!(self.peek(), Some(T::Id(id)) if id == "listOf") {
                points = Some(self.point_list()?);
            } else {
                match self.call_arg()? {
                    (Some(name), v, b) => match name.as_str() {
                        "reps" => reps = v.map(|n| n.lit()).transpose()?,
                        "mirroring" => mirror = b.unwrap_or(false),
                        "center" => {
                            // `Offset(0.5f, 0.5f)`: every upstream call site uses
                            // the default; verified by the parity test.
                            self.want(T::Id("Offset".into()))?;
                            self.want(T::LP)?;
                            let cx = self.num()?;
                            self.want(T::Comma)?;
                            let cy = self.num()?;
                            self.want(T::RP)?;
                            if cx != 0.5 || cy != 0.5 {
                                return Err(format!(
                                    "customPolygon center {cx},{cy}: generator emits (0.5, 0.5)"
                                ));
                            }
                        }
                        o => return Err(format!("customPolygon: unknown arg `{o}`")),
                    },
                    (None, v, _) => reps = v.map(|n| n.lit()).transpose()?,
                }
            }
            self.comma();
        }
        self.want(T::RP)?;
        Ok(Expr::Custom {
            points: points.ok_or("customPolygon: missing listOf")?,
            reps: reps.ok_or("customPolygon: missing reps")? as i64,
            mirror,
        })
    }

    /// `RoundedPolygon(..)` constructor / `.rectangle` / `.circle` / `.star`.
    fn rounded_polygon(&mut self) -> Result<Expr, String> {
        if matches!(self.peek(), Some(T::Dot)) {
            self.i += 1;
            return match self.id()?.as_str() {
                "rectangle" => {
                    self.want(T::LP)?;
                    let (mut w, mut h, mut rounding, mut pv) = (None, None, None, None);
                    while !matches!(self.peek(), Some(T::RP)) {
                        if matches!(self.peek(), Some(T::Id(id)) if id == "rounding" || id == "perVertexRounding")
                        {
                            let (r, p) = self.rounding_arg()?;
                            rounding = r;
                            pv = p;
                        } else {
                            let (name, v, _) = self.call_arg()?;
                            let v = v.map(|n| n.lit()).transpose()?;
                            match name.as_deref() {
                                Some("width") => w = v,
                                Some("height") => h = v,
                                _ => {
                                    if w.is_none() {
                                        w = v
                                    } else {
                                        h = v
                                    }
                                }
                            }
                        }
                        self.comma();
                    }
                    self.want(T::RP)?;
                    Ok(Expr::Rect {
                        w: w.ok_or("rectangle: missing width")?,
                        h: h.ok_or("rectangle: missing height")?,
                        rounding,
                        per_vertex: pv,
                    })
                }
                "circle" => {
                    self.want(T::LP)?;
                    let mut n = None;
                    while !matches!(self.peek(), Some(T::RP)) {
                        let (_, v, _) = self.call_arg()?;
                        n = v;
                        self.comma();
                    }
                    self.want(T::RP)?;
                    Ok(Expr::Circle { n })
                }
                "star" => {
                    self.want(T::LP)?;
                    let (mut n, mut inner, mut rounding) = (None, None, None);
                    while !matches!(self.peek(), Some(T::RP)) {
                        if matches!(self.peek(), Some(T::Id(id)) if id == "rounding") {
                            self.id()?;
                            self.want(T::Eq)?;
                            rounding = Some(self.cr()?);
                        } else {
                            let (name, v, _) = self.call_arg()?;
                            let v = v.map(|n| n.lit()).transpose()?;
                            match name.as_deref() {
                                Some("numVerticesPerRadius") | None => n = v,
                                Some("innerRadius") => inner = v,
                                Some(o) => return Err(format!("star: unsupported arg `{o}`")),
                            }
                        }
                        self.comma();
                    }
                    self.want(T::RP)?;
                    Ok(Expr::Star {
                        n: n.ok_or("star: missing numVerticesPerRadius")?,
                        inner: inner.ok_or("star: missing innerRadius")?,
                        rounding: rounding.ok_or("star: missing rounding")?,
                    })
                }
                o => Err(format!("unsupported RoundedPolygon.{o}")),
            };
        }
        // `RoundedPolygon(numVertices = n, rounding|perVertexRounding = ..)`
        self.want(T::LP)?;
        let (mut n, mut rounding, mut pv) = (None, None, None);
        while !matches!(self.peek(), Some(T::RP)) {
            if matches!(self.peek(), Some(T::Id(id)) if id == "rounding" || id == "perVertexRounding")
            {
                let (r, p) = self.rounding_arg()?;
                rounding = r;
                pv = p;
            } else {
                let (_, v, _) = self.call_arg()?;
                n = v.map(|n| n.lit()).transpose()?;
            }
            self.comma();
        }
        self.want(T::RP)?;
        Ok(Expr::Polygon { n: n.ok_or("RoundedPolygon: missing numVertices")?, rounding, per_vertex: pv })
    }

    /// `<primary>` plus `.transformed(..)` / `.normalized()` / `.also {..}`.
    fn expr(&mut self) -> Result<Expr, String> {
        let name = self.id()?;
        let mut e = match name.as_str() {
            "customPolygon" => self.custom_polygon()?,
            "RoundedPolygon" => self.rounded_polygon()?,
            fun => {
                self.want(T::LP)?;
                let mut args = Vec::new();
                while !matches!(self.peek(), Some(T::RP)) {
                    let (n, v, b) = self.call_arg()?;
                    if b.is_some() {
                        return Err(format!("{fun}: unexpected bool arg"));
                    }
                    args.push((n, v.ok_or_else(|| format!("{fun}: missing arg value"))?));
                    self.comma();
                }
                self.want(T::RP)?;
                Expr::Call { name: fun.to_string(), args }
            }
        };
        loop {
            match self.peek() {
                Some(T::Dot) => {
                    self.i += 1;
                    match self.id()?.as_str() {
                        "transformed" => {
                            self.want(T::LP)?;
                            let m = self.matrix()?;
                            self.want(T::RP)?;
                            e = Expr::Transformed(Box::new(e), m);
                        }
                        "normalized" => {
                            self.want(T::LP)?;
                            self.want(T::RP)?;
                            e = Expr::Normalized(Box::new(e));
                        }
                        "also" => {
                            self.want(T::LB)?;
                            let mut depth = 1;
                            while depth > 0 {
                                match self.next()? {
                                    T::LB => depth += 1,
                                    T::RB => depth -= 1,
                                    _ => {}
                                }
                            }
                        }
                        o => return Err(format!("unsupported postfix `.{o}`")),
                    }
                }
                _ => break,
            }
        }
        Ok(e)
    }
}

// ------------------------------------------------------------------- emit

fn fnum(n: f64) -> String {
    if n.fract() == 0.0 && n.abs() < 1e15 { format!("{}", n as i64) } else { format!("{n}") }
}

fn cr_expr(c: &Cr) -> String {
    format!("{{radius: {}, smoothing: {}}}", fnum(c.radius), fnum(c.smoothing))
}

fn emit(e: &Expr, funs: &HashMap<String, Fun>, env: &HashMap<String, f64>) -> Result<String, String> {
    Ok(match e {
        Expr::Custom { points, reps, mirror } => {
            let verts = points
                .iter()
                .map(|(x, y, _)| format!("{{x: {}px, y: {}px}}", fnum(*x), fnum(*y)))
                .collect::<Vec<_>>()
                .join(", ");
            let crs = points.iter().map(|(_, _, r)| cr_expr(r)).collect::<Vec<_>>().join(", ");
            // `customPolygon(pnr, reps, center = Offset(0.5, 0.5), mirroring)`:
            // every upstream call site uses the default center.
            format!("Shapes.custom([{verts}], [{crs}], {reps}, {{x: 0.5px, y: 0.5px}}, {mirror})")
        }
        Expr::Polygon { n, rounding, per_vertex } => match per_vertex {
            Some(pv) => format!(
                "Shapes.regular-polygon-per-vertex({}, [{}])",
                fnum(*n),
                pv.iter().map(cr_expr).collect::<Vec<_>>().join(", ")
            ),
            None => format!(
                "Shapes.regular-polygon({}, {})",
                fnum(*n),
                cr_expr(&rounding.unwrap_or(Cr { radius: 0., smoothing: 0. }))
            ),
        },
        Expr::Rect { w, h, rounding, per_vertex } => {
            let crs = match (rounding, per_vertex) {
                (_, Some(pv)) => pv.iter().map(cr_expr).collect::<Vec<_>>().join(", "),
                (Some(r), None) => cr_expr(r),
                (None, None) => cr_expr(&Cr { radius: 0., smoothing: 0. }),
            };
            format!("Shapes.rectangle({}, {}, [{crs}])", fnum(*w), fnum(*h))
        }
        Expr::Circle { n } => {
            // `RoundedPolygon.circle()` defaults to 8 vertices upstream.
            let n = match n {
                Some(n) => num_value(n, env)?,
                None => 8.,
            };
            format!("Shapes.circle({})", fnum(n))
        }
        Expr::Star { n, inner, rounding } => {
            // `innerRounding = null` upstream -> inner vertices use `rounding`.
            format!("Shapes.star({}, {}, {}, {})", fnum(*n), fnum(*inner), cr_expr(rounding), cr_expr(rounding))
        }
        Expr::Transformed(base, m) => {
            let inner = emit(base, funs, env)?;
            match m {
                Mat::Rot(d) => format!("Shapes.rotated({inner}, {}deg)", fnum(*d)),
                Mat::Scale(x, y) => format!("Shapes.scaled({inner}, {}, {})", fnum(*x), fnum(*y)),
            }
        }
        Expr::Normalized(inner) => format!("Shapes.normalized({})", emit(inner, funs, env)?),
        Expr::Call { name, args } => {
            let f = funs.get(name).ok_or_else(|| format!("unknown function `{name}`"))?;
            let mut env = env.clone();
            // seed param defaults, then apply passed args (named or positional)
            for (pname, default) in &f.defaults {
                env.insert(pname.clone(), *default);
            }
            for (i, (an, v)) in args.iter().enumerate() {
                let key = an
                    .clone()
                    .or_else(|| f.defaults.get(i).map(|(n, _)| n.clone()))
                    .ok_or_else(|| format!("{name}: unmapped arg {i}"))?;
                env.insert(key, num_value(v, &env)?);
            }
            emit(&f.body, funs, &env)?
        }
    })
}

fn num_value(n: &Num, env: &HashMap<String, f64>) -> Result<f64, String> {
    match n {
        Num::Lit(v) => Ok(*v),
        Num::Ref(p) => env.get(p).copied().ok_or_else(|| format!("unbound param `{p}`")),
    }
}

/// `Cookie9Sided` -> `cookie-9-sided`, `ClamShell` -> `clam-shell`.
fn kebab_case(name: &str) -> String {
    let chars: Vec<char> = name.chars().collect();
    let mut out = String::with_capacity(name.len() + 4);
    for (i, &c) in chars.iter().enumerate() {
        if i > 0 && (c.is_uppercase() || (c.is_ascii_digit() && !chars[i - 1].is_ascii_digit())) {
            out.push('-');
        }
        out.push(c.to_ascii_lowercase());
    }
    out
}

// ------------------------------------------------------------- structure

/// Parse `MaterialShapes.kt` into `(Name, KDoc, slint_expr)` in file order.
pub fn parse(text: &str) -> Result<Vec<(String, String, String)>, String> {
    let (clean, docs) = strip_comments(text);
    let comp = clean
        .find("companion object")
        .ok_or("no `companion object` in MaterialShapes.kt")?;
    let open = clean[comp..].find('{').map(|i| comp + i).ok_or("companion object body")?;
    let end = matching_brace(&clean, open)?;
    let body = &clean[open + 1..end - 1];
    let body_off = open + 1;

    // Split the body into top-level declarations on `private`/`internal`/
    // `public` boundaries at brace depth 0.
    let mut roundings: HashMap<String, Cr> = HashMap::new();
    let mut matrices: HashMap<String, Mat> = HashMap::new();
    let mut funs: HashMap<String, Fun> = HashMap::new();
    let mut public: Vec<(String, String, Expr)> = Vec::new();

    let mut i = 0;
    let bytes = body.as_bytes();
    let mut depth = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'{' => depth += 1,
            b'}' => depth -= 1,
            _ => {}
        }
        if depth == 0 {
            let rest = &body[i..];
            let ws = rest.len() - rest.trim_start().len();
            i += ws;
            let rest = &body[i..];
            let mut matched = false;
            for kw in ["private val ", "private var ", "public val ", "internal fun ", "private fun ", "private data class "] {
                if let Some(decl) = rest.strip_prefix(kw) {
                    // find the end of the declaration: the next top-level keyword
                    // at depth 0, or end of body.
                    let mut j = i + kw.len();
                    let mut d = 0;
                    while j < bytes.len() {
                        match bytes[j] {
                            b'{' => d += 1,
                            b'}' => {
                                if d == 0 {
                                    break;
                                }
                                d -= 1;
                            }
                            b'\n' if d == 0 => {
                                let ahead = body[j + 1..].trim_start();
                                if ["private ", "public ", "internal ", "@", "}"]
                                    .iter()
                                    .any(|k| ahead.starts_with(k))
                                {
                                    break;
                                }
                            }
                            _ => {}
                        }
                        j += 1;
                    }
                    let decl_text = decl[..j - i - kw.len()].to_string();
                    handle_decl(
                        kw.trim_end(),
                        &decl_text,
                        &mut roundings,
                        &mut matrices,
                        &mut funs,
                        &mut public,
                        &docs,
                        body_off + i,
                    )
                    .map_err(|e| format!("`{kw}{}..`: {e}", decl_text.chars().take(60).collect::<String>()))?;
                    i = j;
                    matched = true;
                    break;
                }
            }
            if !matched {
                // `@Suppress(..)` annotations and other non-decl text between
                // declarations — advance one byte.
                i += 1;
            }
            continue;
        }
        i += 1;
    }

    if public.is_empty() {
        return Err("no public shapes parsed".into());
    }

    let mut out = Vec::new();
    for (name, doc, expr) in &public {
        let slint = emit(expr, &funs, &HashMap::new())
            .map_err(|e| format!("{name}: {e}"))?;
        out.push((name.clone(), doc.clone(), slint));
    }
    Ok(out)
}

fn handle_decl(
    kind: &str,
    decl: &str,
    roundings: &mut HashMap<String, Cr>,
    matrices: &mut HashMap<String, Mat>,
    funs: &mut HashMap<String, Fun>,
    public: &mut Vec<(String, String, Expr)>,
    docs: &[(usize, String)],
    off: usize,
) -> Result<(), String> {
    match kind {
        "private val" => {
            // `cornerRound15 = CornerRounding(radius = .15f)` or
            // `rotateNeg45 = Matrix().apply { rotateZ(-45f) }`
            let (name, rhs) =
                decl.split_once('=').ok_or("private val without `=`")?;
            let name = name.trim();
            let rhs = rhs.trim();
            let toks = expr_tokens(rhs)?;
            let mut p = P { t: &toks, i: 0, roundings, matrices };
            if rhs.starts_with("CornerRounding") {
                roundings.insert(name.to_string(), p.cr()?);
            } else if rhs.starts_with("Matrix") {
                matrices.insert(name.to_string(), p.matrix()?);
            } else {
                return Err(format!("unsupported private val `{name}` = {rhs}"));
            }
        }
        "private var" => {} // `_x: RoundedPolygon? = null` lazy cache
        "public val" => {
            // `Circle: RoundedPolygon\n  get() = _circle ?: circle().normalized().also { _circle = it }`
            let (head, tail) = decl
                .split_once(':')
                .ok_or("public val without type")?;
            let name = head.trim().to_string();
            let tail = tail.trim();
            let tail = tail
                .strip_prefix("RoundedPolygon")
                .ok_or("public val is not a RoundedPolygon")?
                .trim();
            let getter = tail
                .strip_prefix("get()")
                .ok_or_else(|| format!("{name}: expected `get()`"))?
                .trim();
            let getter = getter
                .strip_prefix('=')
                .ok_or_else(|| format!("{name}: expected `=` in getter"))?
                .trim();
            // `_x ?: <expr>` — drop the cache var
            let getter = getter
                .split_once("?:")
                .map(|(_, e)| e.trim())
                .ok_or_else(|| format!("{name}: expected `_x ?:` in getter"))?;
            let toks = expr_tokens(getter)?;
            let mut p = P { t: &toks, i: 0, roundings, matrices };
            let expr = p.expr()?;
            if p.i != toks.len() {
                return Err(format!("{name}: trailing tokens in getter"));
            }
            // the KDoc whose comment ended closest before this declaration
            let doc = docs
                .iter()
                .rev()
                .find(|(pos, _)| *pos <= off)
                .map(|(_, d)| d.clone())
                .unwrap_or_default();
            public.push((name, doc, expr));
        }
        "internal fun" => {
            // `name(params): RoundedPolygon { [val m = Matrix()..] return <expr> }`
            let (sig, rest) = decl.split_once('{').ok_or("fun without body")?;
            let (name, params) = sig.split_once('(').ok_or("fun without params")?;
            let name = name.trim().to_string();
            let params = params.split(')').next().unwrap_or("").trim();
            let mut defaults = Vec::new();
            if !params.is_empty() {
                for part in params.split(',') {
                    let part = part.trim();
                    if part.is_empty() {
                        continue;
                    }
                    // `numVertices: Int = 10`
                    if let Some((lhs, rhs)) = part.split_once('=') {
                        let pname = lhs.split(':').next().unwrap().trim().to_string();
                        let dv: f64 = rhs
                            .trim()
                            .trim_end_matches('f')
                            .parse()
                            .map_err(|e| format!("{name}: bad default `{rhs}`: {e}"))?;
                        defaults.push((pname, dv));
                    }
                }
            }
            // body: optional `val <name> = Matrix().apply { .. }` lines, then
            // `return <expr>`; the decl includes the fun's closing `}`.
            let mut locals: HashMap<String, Mat> = matrices.clone();
            let mut body = rest.trim().strip_suffix('}').unwrap_or(rest.trim()).trim();
            while let Some(v) = body.strip_prefix("val ") {
                let (vname, rhs) = v.split_once('=').ok_or("val without `=`")?;
                let vname = vname.trim().to_string();
                let rhs = rhs.trim();
                let toks = expr_tokens(rhs)?;
                let m = (P { t: &toks, i: 0, roundings, matrices: &locals }).matrix()?;
                locals.insert(vname, m);
                // continue after the `}` that closes `apply { .. }`
                let close = matching_brace(rhs, rhs.find('{').ok_or("val body without `apply {`")?)?;
                body = rhs[close..].trim_start();
            }
            let ret = body
                .strip_prefix("return")
                .ok_or_else(|| format!("{name}: no `return`"))?
                .trim();
            let toks = expr_tokens(ret)?;
            let mut p = P { t: &toks, i: 0, roundings, matrices: &locals };
            let expr = p.expr()?;
            if p.i != toks.len() {
                return Err(format!("{name}: trailing tokens in body"));
            }
            funs.insert(name, Fun { defaults, body: expr });
        }
        "private fun" | "private data class" => {} // doRepeat/customPolygon impl details
        _ => {}
    }
    Ok(())
}

/// Emit `material_shapes.slint` content (header included).
pub fn emit_file(text: &str, head: &str) -> Result<String, String> {
    let shapes = parse(text)?;
    if shapes.len() != 35 {
        return Err(format!("expected 35 MaterialShapes members, parsed {}", shapes.len()));
    }
    let mut s = head.to_string();
    let _ = writeln!(s);
    let _ = writeln!(
        s,
        "/// The 35 preset shapes of `MaterialShapes.kt` (`@material3expressive`),\n\
         /// each the unit-square-normalized `RoundedPolygon` of its upstream getter.\n\
         /// Assign them to a `shape` property, e.g. `shape: MaterialShapes.cookie-9-sided`."
    );
    let _ = writeln!(s, "export global MaterialShapes {{");
    for (name, doc, expr) in shapes {
        for line in doc.lines() {
            let line = line.trim().trim_start_matches('*').trim();
            if line.is_empty() || line.starts_with('@') || line.starts_with('!') {
                continue;
            }
            let _ = writeln!(s, "    /// {line}");
        }
        let _ = writeln!(s, "    out property <shape> {}: {};", kebab_case(&name), expr);
    }
    let _ = writeln!(s, "}}");
    Ok(s)
}
