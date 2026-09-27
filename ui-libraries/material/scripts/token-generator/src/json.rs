// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

//! Minimal JSON support for `PARITY_STATUS.json`.
//!
//! The status file is written by the generator itself, so the reader only
//! needs the subset of JSON it emits: objects, arrays, strings and numbers.
//! It is still strict — anything else fails loudly rather than being skipped.

use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq)]
pub enum Json {
    Str(String),
    Num(f64),
    Arr(Vec<Json>),
    Obj(BTreeMap<String, Json>),
}

impl Json {
    pub fn str(&self) -> Option<&str> {
        match self {
            Json::Str(s) => Some(s),
            _ => None,
        }
    }
    pub fn obj(&self) -> Option<&BTreeMap<String, Json>> {
        match self {
            Json::Obj(m) => Some(m),
            _ => None,
        }
    }
    pub fn arr(&self) -> Option<&Vec<Json>> {
        match self {
            Json::Arr(a) => Some(a),
            _ => None,
        }
    }
}

struct P<'a> {
    b: &'a [u8],
    i: usize,
}

impl<'a> P<'a> {
    fn ws(&mut self) {
        while self.i < self.b.len() && self.b[self.i].is_ascii_whitespace() {
            self.i += 1;
        }
    }
    fn peek(&self) -> Option<u8> {
        self.b.get(self.i).copied()
    }
    fn expect(&mut self, c: u8) -> Result<(), String> {
        self.ws();
        if self.peek() == Some(c) {
            self.i += 1;
            Ok(())
        } else {
            Err(format!("expected `{}` at byte {}", c as char, self.i))
        }
    }
    fn value(&mut self) -> Result<Json, String> {
        self.ws();
        match self.peek() {
            Some(b'{') => self.object(),
            Some(b'[') => self.array(),
            Some(b'"') => Ok(Json::Str(self.string()?)),
            Some(c) if c.is_ascii_digit() || c == b'-' => self.number(),
            other => Err(format!("unexpected {:?} at byte {}", other.map(|c| c as char), self.i)),
        }
    }
    fn object(&mut self) -> Result<Json, String> {
        self.expect(b'{')?;
        let mut map = BTreeMap::new();
        self.ws();
        if self.peek() == Some(b'}') {
            self.i += 1;
            return Ok(Json::Obj(map));
        }
        loop {
            self.ws();
            let key = self.string()?;
            self.expect(b':')?;
            let val = self.value()?;
            map.insert(key, val);
            self.ws();
            match self.peek() {
                Some(b',') => {
                    self.i += 1;
                }
                Some(b'}') => {
                    self.i += 1;
                    return Ok(Json::Obj(map));
                }
                other => {
                    return Err(format!(
                        "expected `,` or `}}` at byte {}, found {:?}",
                        self.i,
                        other.map(|c| c as char)
                    ));
                }
            }
        }
    }
    fn array(&mut self) -> Result<Json, String> {
        self.expect(b'[')?;
        let mut items = Vec::new();
        self.ws();
        if self.peek() == Some(b']') {
            self.i += 1;
            return Ok(Json::Arr(items));
        }
        loop {
            items.push(self.value()?);
            self.ws();
            match self.peek() {
                Some(b',') => {
                    self.i += 1;
                }
                Some(b']') => {
                    self.i += 1;
                    return Ok(Json::Arr(items));
                }
                other => {
                    return Err(format!(
                        "expected `,` or `]` at byte {}, found {:?}",
                        self.i,
                        other.map(|c| c as char)
                    ));
                }
            }
        }
    }
    fn string(&mut self) -> Result<String, String> {
        self.ws();
        if self.peek() != Some(b'"') {
            return Err(format!("expected string at byte {}", self.i));
        }
        self.i += 1;
        let mut s = String::new();
        loop {
            match self.b.get(self.i).copied() {
                None => return Err("unterminated string".to_string()),
                Some(b'"') => {
                    self.i += 1;
                    return Ok(s);
                }
                Some(b'\\') => {
                    self.i += 1;
                    match self.b.get(self.i).copied() {
                        Some(b'"') => s.push('"'),
                        Some(b'\\') => s.push('\\'),
                        Some(b'/') => s.push('/'),
                        Some(b'n') => s.push('\n'),
                        Some(b't') => s.push('\t'),
                        Some(b'u') => {
                            let hex = self
                                .b
                                .get(self.i + 1..self.i + 5)
                                .and_then(|h| std::str::from_utf8(h).ok())
                                .ok_or("bad \\u escape")?;
                            let cp = u32::from_str_radix(hex, 16)
                                .map_err(|_| "bad \\u escape".to_string())?;
                            s.push(char::from_u32(cp).ok_or("bad \\u codepoint")?);
                            self.i += 4;
                        }
                        other => return Err(format!("bad escape {:?}", other.map(|c| c as char))),
                    }
                    self.i += 1;
                }
                Some(c) => {
                    // Consume a full UTF-8 char.
                    let len = utf8_len(c);
                    let end = self.i + len;
                    let slice = self
                        .b
                        .get(self.i..end)
                        .and_then(|s| std::str::from_utf8(s).ok())
                        .ok_or("bad utf-8 in string")?;
                    s.push_str(slice);
                    self.i = end;
                }
            }
        }
    }
    fn number(&mut self) -> Result<Json, String> {
        self.ws();
        let start = self.i;
        while let Some(c) = self.peek() {
            if c.is_ascii_digit() || matches!(c, b'-' | b'+' | b'.' | b'e' | b'E') {
                self.i += 1;
            } else {
                break;
            }
        }
        let text =
            std::str::from_utf8(&self.b[start..self.i]).map_err(|_| "bad number".to_string())?;
        text.parse::<f64>().map(Json::Num).map_err(|_| format!("bad number `{text}`"))
    }
}

fn utf8_len(b: u8) -> usize {
    if b < 0x80 {
        1
    } else if b >> 5 == 0b110 {
        2
    } else if b >> 4 == 0b1110 {
        3
    } else {
        4
    }
}

pub fn parse(text: &str) -> Result<Json, String> {
    let mut p = P { b: text.as_bytes(), i: 0 };
    let v = p.value()?;
    p.ws();
    if p.i != p.b.len() {
        return Err(format!("trailing bytes at {}", p.i));
    }
    Ok(v)
}

fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Serialize with sorted keys and two-space indentation so the committed file
/// is stable byte for byte.
pub fn write(v: &Json) -> String {
    let mut out = String::new();
    write_into(v, 0, &mut out);
    out.push('\n');
    out
}

fn write_into(v: &Json, indent: usize, out: &mut String) {
    let pad = "  ".repeat(indent);
    let pad2 = "  ".repeat(indent + 1);
    match v {
        Json::Str(s) => out.push_str(&escape(s)),
        Json::Num(n) => {
            if n.fract() == 0.0 {
                out.push_str(&format!("{}", *n as i64));
            } else {
                out.push_str(&format!("{n}"));
            }
        }
        Json::Arr(items) => {
            if items.is_empty() {
                out.push_str("[]");
                return;
            }
            out.push_str("[\n");
            for (i, item) in items.iter().enumerate() {
                out.push_str(&pad2);
                write_into(item, indent + 1, out);
                if i + 1 < items.len() {
                    out.push(',');
                }
                out.push('\n');
            }
            out.push_str(&pad);
            out.push(']');
        }
        Json::Obj(map) => {
            if map.is_empty() {
                out.push_str("{}");
                return;
            }
            out.push_str("{\n");
            let len = map.len();
            for (i, (k, val)) in map.iter().enumerate() {
                out.push_str(&pad2);
                out.push_str(&escape(k));
                out.push_str(": ");
                write_into(val, indent + 1, out);
                if i + 1 < len {
                    out.push(',');
                }
                out.push('\n');
            }
            out.push_str(&pad);
            out.push('}');
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let src = "{\n  \"a\": {\n    \"issues\": [\n      3,\n      9\n    ],\n    \"status\": \"partial\"\n  }\n}\n";
        let v = parse(src).unwrap();
        assert_eq!(write(&v), src);
        let obj = v.obj().unwrap();
        let a = obj["a"].obj().unwrap();
        assert_eq!(a["status"].str(), Some("partial"));
        assert_eq!(a["issues"].arr().unwrap().len(), 2);
    }

    #[test]
    fn rejects_garbage() {
        assert!(parse("{a: 1}").is_err());
        assert!(parse("[1, 2").is_err());
    }
}
