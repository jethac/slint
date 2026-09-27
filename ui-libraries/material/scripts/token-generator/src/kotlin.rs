// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

//! Parser for the generated Kotlin token files in
//! `compose/material3/material3/src/commonMain/kotlin/androidx/compose/material3/tokens/`.
//!
//! The files are machine-generated and use a small set of declaration forms:
//! `const val`, plain `val`, `inline val ... get() =` (the inline value class
//! form used since the 14_1_0 token drop) and `val ... inline get() =` for the
//! composite `TextStyle` members of `TypographyTokens`. Anything outside these
//! forms is a hard error: the generator must never silently skip a token.

use std::fmt;

#[derive(Debug)]
pub struct ParseError {
    pub file: String,
    pub line: usize,
    pub message: String,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.line > 0 {
            write!(f, "{}:{}: {}", self.file, self.line, self.message)
        } else {
            write!(f, "{}: {}", self.file, self.message)
        }
    }
}

impl std::error::Error for ParseError {}

#[derive(Debug, Clone, PartialEq)]
pub struct Member {
    pub name: String,
    /// The declared Kotlin type if one is written, e.g. `Dp`, `ColorToken`,
    /// `androidx.compose.ui.unit.Dp` (normalized to the short name).
    pub declared_type: Option<String>,
    /// The initializer expression with all whitespace collapsed to single
    /// spaces, e.g. `24.0.dp` or `DefaultTextStyle.copy(fontFamily = ...)`.
    pub rhs: String,
    pub is_const: bool,
    /// 1-based line number of the `val` keyword in the original file.
    pub line: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectKind {
    Object,
    /// `internal class TypographyTokens(...)` holds composite members.
    Class,
}

#[derive(Debug)]
pub struct TokenObject {
    pub name: String,
    /// `object` vs `class` — informational; tests check it.
    #[allow(dead_code)]
    pub kind: ObjectKind,
    pub members: Vec<Member>,
}

#[derive(Debug)]
pub struct TokenFile {
    pub file_name: String,
    /// The `// VERSION:` marker carried by generated token files.
    pub version: Option<String>,
    /// `@JvmInline internal value class XxxToken(val id: Int)` declarations.
    pub value_classes: Vec<String>,
    pub objects: Vec<TokenObject>,
    /// File-scope `internal val` helpers such as `DefaultTextStyle` in
    /// TypographyTokens.kt. Kept as raw members; the resolver only accepts
    /// known helpers.
    pub file_members: Vec<Member>,
}

/// Remove `/* */` block comments and `//` line comments while preserving line
/// numbers (newlines are kept, comments become spaces).
fn strip_comments(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '/' if chars.peek() == Some(&'*') => {
                chars.next();
                loop {
                    match chars.next() {
                        Some('*') if chars.peek() == Some(&'/') => {
                            chars.next();
                            break;
                        }
                        Some('\n') => out.push('\n'),
                        None => break,
                        _ => out.push(' '),
                    }
                }
            }
            '/' if chars.peek() == Some(&'/') => {
                while let Some(&c) = chars.peek() {
                    if c == '\n' {
                        break;
                    }
                    chars.next();
                }
            }
            c => out.push(c),
        }
    }
    out
}

fn extract_version(text: &str) -> Option<String> {
    for line in text.lines() {
        if let Some(rest) = line.trim_start().strip_prefix("// VERSION:") {
            return Some(rest.trim().to_string());
        }
    }
    None
}

/// Count braces on a line. The token sources contain no string literals with
/// braces, so a plain count is sufficient.
fn brace_delta(line: &str) -> i64 {
    line.chars().fold(0, |acc, c| match c {
        '{' => acc + 1,
        '}' => acc - 1,
        _ => acc,
    })
}

/// A candidate start of a member declaration inside an object body.
/// Returns (name, is_const) if `line` opens a `val`/`const val`.
fn member_start(line: &str) -> Option<(String, bool)> {
    let t = line.trim_start();
    let t = t.strip_prefix("internal ").map_or(t, |r| r.trim_start());
    let t = t.strip_prefix("private ").map_or(t, |r| r.trim_start());
    let t = t.strip_prefix("inline ").map_or(t, |r| r.trim_start());
    let (t, is_const) = match t.strip_prefix("const ") {
        Some(r) => (r.trim_start(), true),
        None => (t, false),
    };
    let rest = t.strip_prefix("val ")?;
    let rest = rest.trim_start();
    let end = rest.find(|c: char| !(c.is_alphanumeric() || c == '_'))?;
    let name = &rest[..end];
    if name.is_empty() {
        return None;
    }
    Some((name.to_string(), is_const))
}

/// Parse a member block (the lines of a single `val` declaration including the
/// `get() =` body) into a `Member`.
fn parse_member(file: &str, start_line: usize, block: &[String]) -> Result<Member, ParseError> {
    let err = |message: &str| ParseError {
        file: file.to_string(),
        line: start_line,
        message: message.to_string(),
    };
    let text = block.join("\n");
    let first = block.first().map(|s| s.trim()).unwrap_or("");
    let (name, is_const) = member_start(first).ok_or_else(|| err("cannot find member name"))?;

    // Declared type: `val Name: Type` up to `=` or end of the first line.
    let after_name = {
        let mut it = first.splitn(2, "val ");
        it.next();
        it.next().unwrap_or("").trim_start()[name.len()..].trim_start().to_string()
    };
    let declared_type = if let Some(rest) = after_name.strip_prefix(':') {
        let t = rest.trim_start();
        let end = t.find(|c: char| c == '=' || c == '\n').unwrap_or(t.len());
        let ty = t[..end].trim();
        let ty = ty.rsplit('.').next().unwrap_or(ty).to_string();
        Some(ty)
    } else {
        None
    };

    // The initializer is either the text after `get() =`/`inline get() =` or
    // after the first `=` in `val name(: type)? = expr`.
    let rhs = if let Some(pos) = text.find("get()") {
        let after = &text[pos + "get()".len()..];
        let after = after.trim_start();
        let after = after.strip_prefix('=').ok_or_else(|| err("expected `=` after `get()`"))?;
        normalize_ws(after)
    } else {
        let eq = text.find('=').ok_or_else(|| err("member has no initializer"))?;
        normalize_ws(&text[eq + 1..])
    };

    // A trailing `}` closing the enclosing object may share the last line.
    let rhs = rhs.trim_end().trim_end_matches('}').trim_end().to_string();

    Ok(Member { name, declared_type, rhs, is_const, line: start_line })
}

fn normalize_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Parse a whole token `.kt` file.
pub fn parse_file(file_name: &str, text: &str) -> Result<TokenFile, ParseError> {
    let version = extract_version(text);
    let clean = strip_comments(text);
    let lines: Vec<String> = clean.lines().map(|l| l.to_string()).collect();

    let err = |line: usize, message: &str| ParseError {
        file: file_name.to_string(),
        line,
        message: message.to_string(),
    };

    let mut file = TokenFile {
        file_name: file_name.to_string(),
        version,
        value_classes: Vec::new(),
        objects: Vec::new(),
        file_members: Vec::new(),
    };

    let mut i = 0;
    while i < lines.len() {
        let line = &lines[i];
        let trimmed = line.trim();
        let lineno = i + 1;

        if trimmed.is_empty()
            || trimmed.starts_with("package ")
            || trimmed.starts_with("import ")
            || trimmed.starts_with('@') && !trimmed.contains("value class")
        {
            i += 1;
            continue;
        }

        // @JvmInline internal value class XxxToken(val id: Int)
        if trimmed.contains("value class") {
            let name = trimmed
                .split("value class")
                .nth(1)
                .and_then(|s| s.trim_start().split('(').next())
                .map(|s| s.trim().to_string())
                .ok_or_else(|| err(lineno, "cannot parse value class name"))?;
            file.value_classes.push(name);
            i += 1;
            continue;
        }

        // internal object XxxTokens { / internal class TypographyTokens(...) {
        if let Some(rest) = trimmed.strip_prefix("internal ") {
            if rest.starts_with("object ") {
                let (name, body_end) = parse_container(&lines, i, ObjectKind::Object)?;
                let name = name.ok_or_else(|| err(lineno, "missing object name"))?;
                let members =
                    parse_object_members(file_name, &lines, i + 1, body_end, name.as_str())?;
                file.objects.push(TokenObject { name, kind: ObjectKind::Object, members });
                i = body_end + 1;
                continue;
            } else if let Some(rest) = rest.strip_prefix("class ") {
                let _ = rest;
                let (name, body_end) = parse_container(&lines, i, ObjectKind::Class)?;
                let name = name.ok_or_else(|| err(lineno, "missing class name"))?;
                let members =
                    parse_object_members(file_name, &lines, i + 1, body_end, name.as_str())?;
                file.objects.push(TokenObject { name, kind: ObjectKind::Class, members });
                i = body_end + 1;
                continue;
            } else if trimmed.contains("val ") {
                // File-scope `internal val` (helpers like DefaultTextStyle).
                let mut block = vec![lines[i].clone()];
                let mut depth = brace_delta(line);
                let mut j = i + 1;
                // Continue until the expression is complete: balanced parens
                // and the next line is not a continuation (does not end the
                // expression with an unmatched `(` or end with `,`).
                while j < lines.len() && !helper_complete(&block) {
                    block.push(lines[j].clone());
                    depth += brace_delta(&lines[j]);
                    let _ = depth;
                    j += 1;
                }
                file.file_members.push(parse_member(file_name, lineno, &block)?);
                i = j;
                continue;
            }
        }

        return Err(err(lineno, &format!("unrecognized top-level declaration: {trimmed}")));
    }

    Ok(file)
}

/// A file-scope helper is complete once its parentheses balance and the last
/// line does not end with `,`, `=` or `(` (the initializer may start on the
/// next line).
fn helper_complete(block: &[String]) -> bool {
    let text = block.join("\n");
    let mut parens: i64 = 0;
    for c in text.chars() {
        match c {
            '(' => parens += 1,
            ')' => parens -= 1,
            _ => {}
        }
    }
    let t = text.trim_end();
    parens <= 0 && !t.ends_with(',') && !t.ends_with('=') && !t.ends_with('(')
}

/// Given the index of the `internal object/class` line, find the object name
/// and the index of the line that closes its body (depth back to zero).
fn parse_container(
    lines: &[String],
    start: usize,
    kind: ObjectKind,
) -> Result<(Option<String>, usize), ParseError> {
    let mut depth: i64 = 0;
    let mut name = None;
    let mut j = start;
    let mut found_open = false;
    while j < lines.len() {
        let line = &lines[j];
        if !found_open {
            // The declaration header may span lines (`class X(...)`).
            if name.is_none() {
                let kw = match kind {
                    ObjectKind::Object => "object ",
                    ObjectKind::Class => "class ",
                };
                if let Some(pos) = line.find(kw) {
                    let after = &line[pos + kw.len()..];
                    let end = after
                        .find(|c: char| !(c.is_alphanumeric() || c == '_'))
                        .unwrap_or(after.len());
                    name = Some(after[..end].to_string());
                }
            }
            if line.contains('{') {
                found_open = true;
            }
        }
        depth += brace_delta(line);
        if found_open && depth == 0 {
            return Ok((name, j));
        }
        j += 1;
    }
    Err(ParseError {
        file: String::new(),
        line: start + 1,
        message: "unbalanced braces in object/class".to_string(),
    })
}

/// Collect member blocks between `body_start` and `body_end` (the index of the
/// line closing the object).
fn parse_object_members(
    file_name: &str,
    lines: &[String],
    body_start: usize,
    body_end: usize,
    _object: &str,
) -> Result<Vec<Member>, ParseError> {
    let mut members = Vec::new();
    let mut i = body_start;
    while i < body_end {
        let line = &lines[i];
        let trimmed = line.trim();
        if trimmed.is_empty() {
            i += 1;
            continue;
        }
        if member_start(line).is_none() {
            return Err(ParseError {
                file: file_name.to_string(),
                line: i + 1,
                message: format!("unrecognized statement in object body: {trimmed}"),
            });
        }
        // Accumulate the member until the next member start at the same level
        // or the end of the object body.
        let mut block = vec![lines[i].clone()];
        let mut j = i + 1;
        while j < body_end {
            if member_start(&lines[j]).is_some() {
                break;
            }
            block.push(lines[j].clone());
            j += 1;
        }
        members.push(parse_member(file_name, i + 1, &block)?);
        i = j;
    }
    Ok(members)
}

#[cfg(test)]
mod tests {
    use super::*;

    const HEADER: &str = "/* license */\n// VERSION: 14_1_0\n// GENERATED CODE - DO NOT MODIFY BY HAND\n\npackage androidx.compose.material3.tokens\n\nimport androidx.compose.ui.unit.dp\n\n";

    #[test]
    fn const_vals() {
        let src = format!(
            "{HEADER}internal object StateTokens {{\n    const val DraggedStateLayerOpacity = 0.16f\n    const val FocusStateLayerOpacity = 0.1f\n}}\n"
        );
        let f = parse_file("StateTokens.kt", &src).unwrap();
        assert_eq!(f.version.as_deref(), Some("14_1_0"));
        assert_eq!(f.objects.len(), 1);
        let obj = &f.objects[0];
        assert_eq!(obj.name, "StateTokens");
        assert_eq!(obj.members.len(), 2);
        assert_eq!(obj.members[0].name, "DraggedStateLayerOpacity");
        assert_eq!(obj.members[0].rhs, "0.16f");
        assert!(obj.members[0].is_const);
        assert_eq!(obj.members[0].declared_type, None);
    }

    #[test]
    fn inline_value_class_form() {
        let src = format!(
            "{HEADER}internal object ElevationTokens {{\n    inline val Level0: androidx.compose.ui.unit.Dp\n        get() = 0.0.dp\n\n    inline val Level1: androidx.compose.ui.unit.Dp\n        get() = 1.0.dp\n}}\n"
        );
        let f = parse_file("ElevationTokens.kt", &src).unwrap();
        let obj = &f.objects[0];
        assert_eq!(obj.members.len(), 2);
        assert_eq!(obj.members[0].name, "Level0");
        assert_eq!(obj.members[0].declared_type.as_deref(), Some("Dp"));
        assert_eq!(obj.members[0].rhs, "0.0.dp");
        assert_eq!(obj.members[1].rhs, "1.0.dp");
    }

    #[test]
    fn reference_form() {
        let src = format!(
            "{HEADER}internal object ButtonSmallTokens {{\n    inline val ContainerShapeRound: ShapeToken\n        get() = ShapeKeyTokens.CornerFull\n\n    inline val ContainerHeight: androidx.compose.ui.unit.Dp\n        get() = 40.0.dp\n}}\n"
        );
        let f = parse_file("ButtonSmallTokens.kt", &src).unwrap();
        let obj = &f.objects[0];
        assert_eq!(obj.members[0].name, "ContainerShapeRound");
        assert_eq!(obj.members[0].declared_type.as_deref(), Some("ShapeToken"));
        assert_eq!(obj.members[0].rhs, "ShapeKeyTokens.CornerFull");
    }

    #[test]
    fn value_class_and_key_tokens() {
        let src = format!(
            "{HEADER}@JvmInline internal value class ShapeToken(val id: Int)\n\ninternal object ShapeKeyTokens {{\n    val CornerExtraExtraLarge = ShapeToken(0)\n    val CornerFull = ShapeToken(6)\n}}\n"
        );
        let f = parse_file("ShapeKeyTokens.kt", &src).unwrap();
        assert_eq!(f.value_classes, vec!["ShapeToken".to_string()]);
        let obj = &f.objects[0];
        assert_eq!(obj.members[0].name, "CornerExtraExtraLarge");
        assert_eq!(obj.members[0].rhs, "ShapeToken(0)");
        assert_eq!(obj.members[1].rhs, "ShapeToken(6)");
    }

    #[test]
    fn multiline_rounded_corner() {
        let src = format!(
            "{HEADER}internal object ShapeTokens {{\n    val CornerExtraLargeTop =\n        RoundedCornerShape(\n            topStart = 28.0.dp,\n            topEnd = 28.0.dp,\n            bottomEnd = 0.0.dp,\n            bottomStart = 0.0.dp,\n        )\n    val CornerFull = CircleShape\n    val CornerNone = RectangleShape\n}}\n"
        );
        let f = parse_file("ShapeTokens.kt", &src).unwrap();
        let obj = &f.objects[0];
        assert_eq!(
            obj.members[0].rhs,
            "RoundedCornerShape( topStart = 28.0.dp, topEnd = 28.0.dp, bottomEnd = 0.0.dp, bottomStart = 0.0.dp, )"
        );
        assert_eq!(obj.members[1].rhs, "CircleShape");
        assert_eq!(obj.members[2].rhs, "RectangleShape");
    }

    #[test]
    fn typography_class_composite() {
        let src = format!(
            "{HEADER}internal class TypographyTokens(val fontFamily: FontFamily? = null) {{\n    val BodyLarge: TextStyle\n        inline get() =\n            DefaultTextStyle.copy(\n                fontFamily = fontFamily ?: TypeScaleTokens.BodyLargeFont,\n                fontWeight = TypeScaleTokens.BodyLargeWeight,\n                fontSize = TypeScaleTokens.BodyLargeSize,\n                lineHeight = TypeScaleTokens.BodyLargeLineHeight,\n                letterSpacing = TypeScaleTokens.BodyLargeTracking,\n            )\n}}\n\ninternal val DefaultTextStyle =\n    TextStyle.Default.copy(\n        platformStyle = defaultPlatformTextStyle(),\n        lineHeightStyle = DefaultLineHeightStyle,\n    )\n"
        );
        let f = parse_file("TypographyTokens.kt", &src).unwrap();
        assert_eq!(f.objects[0].kind, ObjectKind::Class);
        let m = &f.objects[0].members[0];
        assert_eq!(m.name, "BodyLarge");
        assert_eq!(m.declared_type.as_deref(), Some("TextStyle"));
        assert!(m.rhs.starts_with("DefaultTextStyle.copy("));
        assert!(m.rhs.contains("fontWeight = TypeScaleTokens.BodyLargeWeight"));
        assert_eq!(f.file_members.len(), 1);
        assert_eq!(f.file_members[0].name, "DefaultTextStyle");
    }

    #[test]
    fn fails_on_unknown_statement() {
        let src =
            format!("{HEADER}internal object BrokenTokens {{\n    fun something() = 42\n}}\n");
        assert!(parse_file("BrokenTokens.kt", &src).is_err());
    }
}
