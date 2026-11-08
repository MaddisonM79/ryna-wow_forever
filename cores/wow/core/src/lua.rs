//! A reader for the subset of Lua that World of Warcraft writes to SavedVariables files.
//!
//! The client serialises each saved variable as `Name = <literal>`, where the literal is a table,
//! string, number, boolean or `nil`. This module turns a whole file into one JSON object holding
//! each top-level name. There is no Lua VM: anything other than literals (calls, operators other
//! than a leading minus, variables) is an error.
//!
//! Conversion rules:
//! - A table whose keys are exactly `1..n` (positional entries or `[i] =`) becomes an array, in key
//!   order. Any other table becomes an object; integer keys are written as decimal strings.
//!   An empty table becomes `[]`: Lua can't tell an empty list from an empty map, and readers of
//!   the result (the snapshot builder) accept either for an empty value.
//! - `nil` values are dropped, as Lua does. A positional `nil` still uses up its index.
//! - Lua 5.1, which WoW embeds, has one number type (a double), so a number whose value is a whole
//!   number below 2^53 becomes a JSON integer whichever way it was written (`10`, `1e1`, `0xA`).
//! - Strings are bytes in Lua; escapes produce bytes, and the result is read as UTF-8 (invalid
//!   sequences become U+FFFD).
//! - On a duplicate key the last one wins.
//!
//! [`parse`] gives JSON, which loses what JSON can't say (an integer key and the same digits as a
//! string key, bytes that aren't UTF-8, an empty table's kind). [`parse_bytes`] gives the file as
//! it is instead ([`LuaValue`], keys in the order they were written), for anything that writes the
//! file back ([`crate::lua_write`]); `parse` is that tree converted.

use std::cmp::Ordering;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

use serde_json::{Map, Number, Value};
use thiserror::Error;

/// Where and why the input isn't SavedVariables Lua. Lines and columns count from 1.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
#[error("line {line}, column {column}: {message}")]
pub struct ParseError {
    pub line: usize,
    pub column: usize,
    pub message: String,
}

/// Tables nest deeper than this only in hostile input; refusing keeps the stack bounded.
const MAX_DEPTH: usize = 200;

/// The largest magnitude at which every whole double is exactly representable (2^53).
const SAFE_INTEGER: f64 = 9_007_199_254_740_992.0;

/// Parse a SavedVariables file into `{ "<Name>": <value>, ... }`.
pub fn parse(src: &str) -> Result<Value, ParseError> {
    let vars = parse_bytes(src.as_bytes())?;
    let mut out = Map::new();
    for (name, value) in vars {
        out.insert(name, value.to_json());
    }
    Ok(Value::Object(out))
}

/// A value as the file has it: nothing lost. `nil` never appears (a `nil` entry is no entry).
#[derive(Debug, Clone, PartialEq)]
pub enum LuaValue {
    Bool(bool),
    /// A number Lua 5.1 would print as an integer (whole, below 2^53), however it was written.
    Int(i64),
    /// Any other number (finite: the reader refuses inf and nan).
    Float(f64),
    /// Lua strings are bytes.
    Str(Vec<u8>),
    /// Entries in the order they were written (a duplicate key keeps its first place and its
    /// last value). Positional entries have their `Int` keys.
    Table(Vec<(LuaKey, LuaValue)>),
}

/// A table key. A float key is never whole (whole numbers are `Int`).
#[derive(Debug, Clone)]
pub enum LuaKey {
    Int(i64),
    Float(f64),
    Str(Vec<u8>),
    Bool(bool),
}

impl LuaKey {
    /// A string key.
    pub fn str(s: &str) -> LuaKey {
        LuaKey::Str(s.as_bytes().to_vec())
    }

    fn rank(&self) -> u8 {
        match self {
            LuaKey::Int(_) | LuaKey::Float(_) => 0,
            LuaKey::Str(_) => 1,
            LuaKey::Bool(_) => 2,
        }
    }

    /// The key as JSON writes it: integers and floats as their digits, booleans as words.
    pub fn to_json_key(&self) -> String {
        match self {
            LuaKey::Int(i) => i.to_string(),
            LuaKey::Float(f) => {
                Number::from_f64(*f).map_or_else(|| f.to_string(), |n| n.to_string())
            }
            LuaKey::Str(b) => bytes_to_string(b.clone()),
            LuaKey::Bool(b) => b.to_string(),
        }
    }
}

impl PartialEq for LuaKey {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (LuaKey::Int(a), LuaKey::Int(b)) => a == b,
            (LuaKey::Float(a), LuaKey::Float(b)) => a.to_bits() == b.to_bits(),
            (LuaKey::Str(a), LuaKey::Str(b)) => a == b,
            (LuaKey::Bool(a), LuaKey::Bool(b)) => a == b,
            _ => false,
        }
    }
}

impl Eq for LuaKey {}

impl Hash for LuaKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            LuaKey::Int(i) => (0u8, *i).hash(state),
            LuaKey::Float(f) => (1u8, f.to_bits()).hash(state),
            LuaKey::Str(b) => (2u8, b).hash(state),
            LuaKey::Bool(b) => (3u8, *b).hash(state),
        }
    }
}

impl Ord for LuaKey {
    /// Numbers first (by value, an integer before a float of the same value), then strings (by
    /// bytes), then `false`, `true`: the order the writer puts keyed entries in.
    fn cmp(&self, other: &Self) -> Ordering {
        let num = |k: &LuaKey| match k {
            LuaKey::Int(i) => Some((*i as f64, 0u8)),
            LuaKey::Float(f) => Some((*f, 1u8)),
            _ => None,
        };
        self.rank()
            .cmp(&other.rank())
            .then_with(|| match (self, other) {
                (LuaKey::Int(a), LuaKey::Int(b)) => a.cmp(b),
                (LuaKey::Str(a), LuaKey::Str(b)) => a.cmp(b),
                (LuaKey::Bool(a), LuaKey::Bool(b)) => a.cmp(b),
                _ => match (num(self), num(other)) {
                    (Some((a, ta)), Some((b, tb))) => a.total_cmp(&b).then(ta.cmp(&tb)),
                    _ => Ordering::Equal,
                },
            })
    }
}

impl PartialOrd for LuaKey {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl LuaValue {
    /// A string value.
    pub fn str(s: &str) -> LuaValue {
        LuaValue::Str(s.as_bytes().to_vec())
    }

    pub fn as_table(&self) -> Option<&Vec<(LuaKey, LuaValue)>> {
        match self {
            LuaValue::Table(t) => Some(t),
            _ => None,
        }
    }

    pub fn as_table_mut(&mut self) -> Option<&mut Vec<(LuaKey, LuaValue)>> {
        match self {
            LuaValue::Table(t) => Some(t),
            _ => None,
        }
    }

    /// The string's bytes read as UTF-8 (lossily), for a string value.
    pub fn as_text(&self) -> Option<String> {
        match self {
            LuaValue::Str(b) => Some(String::from_utf8_lossy(b).into_owned()),
            _ => None,
        }
    }

    /// The entry under `key` in a table.
    pub fn get(&self, key: &LuaKey) -> Option<&LuaValue> {
        self.as_table()?
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v)
    }

    /// The entry under string key `key` in a table.
    pub fn field(&self, key: &str) -> Option<&LuaValue> {
        self.get(&LuaKey::str(key))
    }

    pub fn field_mut(&mut self, key: &str) -> Option<&mut LuaValue> {
        let key = LuaKey::str(key);
        self.as_table_mut()?
            .iter_mut()
            .find(|(k, _)| *k == key)
            .map(|(_, v)| v)
    }

    /// Put `value` under `key` (in its place if the key is there, else at the end). Does nothing
    /// to a value that isn't a table.
    pub fn set(&mut self, key: LuaKey, value: LuaValue) {
        if let Some(t) = self.as_table_mut() {
            match t.iter_mut().find(|(k, _)| *k == key) {
                Some(entry) => entry.1 = value,
                None => t.push((key, value)),
            }
        }
    }

    /// Take the entry under `key` out of a table.
    pub fn remove(&mut self, key: &LuaKey) -> Option<LuaValue> {
        let t = self.as_table_mut()?;
        let i = t.iter().position(|(k, _)| k == key)?;
        Some(t.remove(i).1)
    }

    /// The same value with every table's entries in the writer's order, so two values that differ
    /// only in the order their entries were written compare equal.
    pub fn canonical(&self) -> LuaValue {
        match self {
            LuaValue::Table(t) => {
                let mut entries: Vec<(LuaKey, LuaValue)> =
                    t.iter().map(|(k, v)| (k.clone(), v.canonical())).collect();
                entries.sort_by(|a, b| a.0.cmp(&b.0));
                LuaValue::Table(entries)
            }
            other => other.clone(),
        }
    }

    /// The value as [`parse`] gives it (module docs: the conversion rules).
    pub fn to_json(&self) -> Value {
        match self {
            LuaValue::Bool(b) => Value::Bool(*b),
            LuaValue::Int(i) => Value::Number(Number::from(*i)),
            LuaValue::Float(f) => Number::from_f64(*f).map_or(Value::Null, Value::Number),
            LuaValue::Str(b) => Value::String(bytes_to_string(b.clone())),
            LuaValue::Table(entries) => table_to_json(entries),
        }
    }
}

/// Parse a SavedVariables file into its top-level names and their values, in the order they were
/// first assigned, losing nothing ([`LuaValue`]). A later assignment replaces an earlier one in
/// its place; a `nil` one removes the name.
pub fn parse_bytes(src: &[u8]) -> Result<Vec<(String, LuaValue)>, ParseError> {
    let mut p = Parser {
        s: src,
        pos: 0,
        depth: 0,
    };
    if p.s.starts_with(b"\xEF\xBB\xBF") {
        p.pos = 3;
    }
    let mut out: Vec<(String, LuaValue)> = Vec::new();
    loop {
        p.skip_ws()?;
        if p.peek().is_none() {
            break;
        }
        let name = match p.peek() {
            Some(c) if is_ident_start(c) => p.ident(),
            _ => return Err(p.err("expected a name at the start of an assignment")),
        };
        if is_keyword(&name) {
            return Err(p.err(format!("`{name}` can't be assigned to")));
        }
        p.skip_ws()?;
        p.expect(b'=')?;
        let at = out.iter().position(|(n, _)| *n == name);
        match (p.value()?, at) {
            (Some(v), Some(i)) => out[i].1 = v,
            (Some(v), None) => out.push((name, v)),
            (None, Some(i)) => {
                out.remove(i);
            }
            (None, None) => {}
        }
        p.skip_ws()?;
        if p.peek() == Some(b';') {
            p.pos += 1;
        }
    }
    Ok(out)
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Num {
    Int(i64),
    Float(f64),
}

impl Num {
    /// The integer this number is, if it is one Lua 5.1 would print as an integer.
    fn as_int(self) -> Option<i64> {
        match self {
            Num::Int(i) => Some(i),
            Num::Float(f) if f.is_finite() && f.fract() == 0.0 && f.abs() < SAFE_INTEGER => {
                Some(f as i64)
            }
            Num::Float(_) => None,
        }
    }
}

struct Parser<'a> {
    s: &'a [u8],
    pos: usize,
    depth: usize,
}

fn is_ident_start(c: u8) -> bool {
    c.is_ascii_alphabetic() || c == b'_'
}

fn is_ident_char(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_'
}

fn is_keyword(w: &str) -> bool {
    matches!(
        w,
        "and"
            | "break"
            | "do"
            | "else"
            | "elseif"
            | "end"
            | "false"
            | "for"
            | "function"
            | "goto"
            | "if"
            | "in"
            | "local"
            | "nil"
            | "not"
            | "or"
            | "repeat"
            | "return"
            | "then"
            | "true"
            | "until"
            | "while"
    )
}

impl<'a> Parser<'a> {
    fn peek(&self) -> Option<u8> {
        self.s.get(self.pos).copied()
    }

    fn peek_at(&self, ahead: usize) -> Option<u8> {
        self.s.get(self.pos + ahead).copied()
    }

    fn err_at(&self, pos: usize, message: impl Into<String>) -> ParseError {
        let pos = pos.min(self.s.len());
        let before = &self.s[..pos];
        let line = before.iter().filter(|&&c| c == b'\n').count() + 1;
        let line_start = before
            .iter()
            .rposition(|&c| c == b'\n')
            .map_or(0, |i| i + 1);
        // Columns count characters, not bytes, so a message points at the right place in an
        // editor even after a multi-byte name.
        let column = String::from_utf8_lossy(&self.s[line_start..pos])
            .chars()
            .count()
            + 1;
        ParseError {
            line,
            column,
            message: message.into(),
        }
    }

    fn err(&self, message: impl Into<String>) -> ParseError {
        self.err_at(self.pos, message)
    }

    fn expect(&mut self, c: u8) -> Result<(), ParseError> {
        if self.peek() == Some(c) {
            self.pos += 1;
            Ok(())
        } else {
            Err(self.err(format!("expected `{}`", c as char)))
        }
    }

    /// Skip whitespace, `--` line comments and `--[[ ]]` / `--[==[ ]==]` block comments.
    fn skip_ws(&mut self) -> Result<(), ParseError> {
        loop {
            match self.peek() {
                Some(b' ' | b'\t' | b'\r' | b'\n' | 0x0b | 0x0c) => self.pos += 1,
                Some(b'-') if self.peek_at(1) == Some(b'-') => {
                    let start = self.pos;
                    self.pos += 2;
                    if let Some(level) = self.long_bracket_level() {
                        self.long_bracket_body(level, start, "comment")?;
                    } else {
                        while let Some(c) = self.peek() {
                            if c == b'\n' {
                                break;
                            }
                            self.pos += 1;
                        }
                    }
                }
                _ => return Ok(()),
            }
        }
    }

    /// At `[` followed by zero or more `=` and another `[`: consume the opener and return the
    /// number of `=`. Anywhere else: consume nothing and return None.
    fn long_bracket_level(&mut self) -> Option<usize> {
        if self.peek() != Some(b'[') {
            return None;
        }
        let mut i = 1;
        while self.peek_at(i) == Some(b'=') {
            i += 1;
        }
        if self.peek_at(i) == Some(b'[') {
            self.pos += i + 1;
            Some(i - 1)
        } else {
            None
        }
    }

    /// Consume up to and including the `]`, `level` `=`, `]` that closes a long bracket, and
    /// return what was inside. A newline straight after the opener isn't part of the content.
    fn long_bracket_body(
        &mut self,
        level: usize,
        start: usize,
        what: &str,
    ) -> Result<&'a [u8], ParseError> {
        if self.peek() == Some(b'\r') {
            self.pos += 1;
            if self.peek() == Some(b'\n') {
                self.pos += 1;
            }
        } else if self.peek() == Some(b'\n') {
            self.pos += 1;
            if self.peek() == Some(b'\r') {
                self.pos += 1;
            }
        }
        let body_start = self.pos;
        while self.pos < self.s.len() {
            if self.s[self.pos] == b']' {
                let mut i = 1;
                while i <= level && self.peek_at(i) == Some(b'=') {
                    i += 1;
                }
                if i == level + 1 && self.peek_at(i) == Some(b']') {
                    let body = &self.s[body_start..self.pos];
                    self.pos += level + 2;
                    return Ok(body);
                }
            }
            self.pos += 1;
        }
        Err(self.err_at(start, format!("unfinished long {what}")))
    }

    fn ident(&mut self) -> String {
        let start = self.pos;
        while matches!(self.peek(), Some(c) if is_ident_char(c)) {
            self.pos += 1;
        }
        // Identifiers are ASCII by construction.
        String::from_utf8_lossy(&self.s[start..self.pos]).into_owned()
    }

    /// A value, or None for `nil`.
    fn value(&mut self) -> Result<Option<LuaValue>, ParseError> {
        self.skip_ws()?;
        match self.peek() {
            None => Err(self.err("unexpected end of input: expected a value")),
            Some(b'{') => self.table().map(Some),
            Some(b'"' | b'\'') => Ok(Some(LuaValue::Str(self.quoted()?))),
            Some(b'[') => {
                let start = self.pos;
                match self.long_bracket_level() {
                    Some(level) => {
                        let body = self.long_bracket_body(level, start, "string")?;
                        Ok(Some(LuaValue::Str(body.to_vec())))
                    }
                    None => Err(self.err("unexpected `[`: expected a value")),
                }
            }
            Some(b'-' | b'.' | b'0'..=b'9') => {
                let start = self.pos;
                let n = self.number()?;
                Ok(Some(self.num_value(n, start)?))
            }
            Some(c) if is_ident_start(c) => {
                let start = self.pos;
                match self.ident().as_str() {
                    "true" => Ok(Some(LuaValue::Bool(true))),
                    "false" => Ok(Some(LuaValue::Bool(false))),
                    "nil" => Ok(None),
                    other => Err(self.err_at(
                        start,
                        format!("unexpected name `{other}`: only literals are allowed"),
                    )),
                }
            }
            Some(c) => Err(self.err(format!("unexpected `{}`: expected a value", c as char))),
        }
    }

    fn num_value(&self, n: Num, start: usize) -> Result<LuaValue, ParseError> {
        if let Some(i) = n.as_int() {
            return Ok(LuaValue::Int(i));
        }
        match n {
            Num::Float(f) if f.is_finite() => Ok(LuaValue::Float(f)),
            Num::Float(_) => Err(self.err_at(start, "a number JSON can't hold (inf or nan)")),
            Num::Int(i) => Ok(LuaValue::Int(i)),
        }
    }

    /// A number literal with an optional leading minus: decimal (with fraction and exponent) or
    /// hexadecimal (integer only).
    fn number(&mut self) -> Result<Num, ParseError> {
        let start = self.pos;
        let negative = if self.peek() == Some(b'-') {
            self.pos += 1;
            // `- 5` is still a negative number in Lua; `--` would have been a comment.
            while matches!(self.peek(), Some(b' ' | b'\t')) {
                self.pos += 1;
            }
            true
        } else {
            false
        };
        let n = if self.peek() == Some(b'0') && matches!(self.peek_at(1), Some(b'x' | b'X')) {
            self.pos += 2;
            let digits_start = self.pos;
            while matches!(self.peek(), Some(c) if c.is_ascii_hexdigit()) {
                self.pos += 1;
            }
            if self.pos == digits_start {
                return Err(self.err_at(start, "malformed hexadecimal number"));
            }
            if matches!(self.peek(), Some(b'.' | b'p' | b'P')) {
                return Err(self.err_at(start, "hexadecimal fractions aren't supported"));
            }
            let text = std::str::from_utf8(&self.s[digits_start..self.pos]).unwrap_or("");
            match u64::from_str_radix(text, 16) {
                Ok(u) if u <= i64::MAX as u64 => Num::Int(u as i64),
                _ => Num::Float(text.bytes().fold(0.0, |acc, c| {
                    acc * 16.0 + f64::from((c as char).to_digit(16).unwrap_or(0))
                })),
            }
        } else {
            let digits_start = self.pos;
            let mut integral = true;
            let mut saw_digit = false;
            while matches!(self.peek(), Some(b'0'..=b'9')) {
                self.pos += 1;
                saw_digit = true;
            }
            if self.peek() == Some(b'.') {
                integral = false;
                self.pos += 1;
                while matches!(self.peek(), Some(b'0'..=b'9')) {
                    self.pos += 1;
                    saw_digit = true;
                }
            }
            if !saw_digit {
                return Err(self.err_at(start, "malformed number"));
            }
            if matches!(self.peek(), Some(b'e' | b'E')) {
                integral = false;
                self.pos += 1;
                if matches!(self.peek(), Some(b'+' | b'-')) {
                    self.pos += 1;
                }
                let exp_start = self.pos;
                while matches!(self.peek(), Some(b'0'..=b'9')) {
                    self.pos += 1;
                }
                if self.pos == exp_start {
                    return Err(self.err_at(start, "malformed number: exponent has no digits"));
                }
            }
            let text = std::str::from_utf8(&self.s[digits_start..self.pos]).unwrap_or("");
            match (integral, text.parse::<i64>()) {
                (true, Ok(i)) => Num::Int(i),
                _ => Num::Float(
                    text.parse::<f64>()
                        .map_err(|_| self.err_at(start, "malformed number"))?,
                ),
            }
        };
        if matches!(self.peek(), Some(c) if is_ident_char(c) || c == b'.') {
            return Err(self.err_at(start, "malformed number"));
        }
        Ok(match (negative, n) {
            (false, n) => n,
            (true, Num::Int(i)) => Num::Int(-i),
            (true, Num::Float(f)) => Num::Float(-f),
        })
    }

    /// A string in double or single quotes, with Lua's escapes.
    fn quoted(&mut self) -> Result<Vec<u8>, ParseError> {
        let start = self.pos;
        let quote = self.s[self.pos];
        self.pos += 1;
        let mut buf: Vec<u8> = Vec::new();
        loop {
            let Some(c) = self.peek() else {
                return Err(self.err_at(start, "unfinished string"));
            };
            match c {
                _ if c == quote => {
                    self.pos += 1;
                    break;
                }
                b'\n' | b'\r' => {
                    return Err(self.err_at(start, "unfinished string (a newline inside quotes)"))
                }
                b'\\' => {
                    let escape_at = self.pos;
                    self.pos += 1;
                    let Some(e) = self.peek() else {
                        return Err(self.err_at(start, "unfinished string"));
                    };
                    self.pos += 1;
                    match e {
                        b'n' => buf.push(b'\n'),
                        b't' => buf.push(b'\t'),
                        b'r' => buf.push(b'\r'),
                        b'a' => buf.push(0x07),
                        b'b' => buf.push(0x08),
                        b'f' => buf.push(0x0c),
                        b'v' => buf.push(0x0b),
                        b'\\' | b'"' | b'\'' => buf.push(e),
                        // A backslash before a line break keeps the line break.
                        b'\n' | b'\r' => {
                            buf.push(b'\n');
                            let other = if e == b'\n' { b'\r' } else { b'\n' };
                            if self.peek() == Some(other) {
                                self.pos += 1;
                            }
                        }
                        b'x' => {
                            let hex = self
                                .s
                                .get(self.pos..self.pos + 2)
                                .and_then(|h| std::str::from_utf8(h).ok())
                                .and_then(|h| u8::from_str_radix(h, 16).ok());
                            match hex {
                                Some(b) => {
                                    buf.push(b);
                                    self.pos += 2;
                                }
                                None => {
                                    return Err(self.err_at(
                                        escape_at,
                                        "`\\x` needs exactly two hexadecimal digits",
                                    ))
                                }
                            }
                        }
                        b'z' => {
                            while matches!(
                                self.peek(),
                                Some(b' ' | b'\t' | b'\r' | b'\n' | 0x0b | 0x0c)
                            ) {
                                self.pos += 1;
                            }
                        }
                        b'0'..=b'9' => {
                            let mut v = u32::from(e - b'0');
                            for _ in 0..2 {
                                match self.peek() {
                                    Some(d @ b'0'..=b'9') => {
                                        v = v * 10 + u32::from(d - b'0');
                                        self.pos += 1;
                                    }
                                    _ => break,
                                }
                            }
                            let byte = u8::try_from(v).map_err(|_| {
                                self.err_at(escape_at, "decimal escape is larger than 255")
                            })?;
                            buf.push(byte);
                        }
                        b'u' => {
                            if self.peek() != Some(b'{') {
                                return Err(self.err_at(escape_at, "`\\u` needs `{`"));
                            }
                            self.pos += 1;
                            let hex_start = self.pos;
                            while matches!(self.peek(), Some(c) if c.is_ascii_hexdigit()) {
                                self.pos += 1;
                            }
                            let ch = std::str::from_utf8(&self.s[hex_start..self.pos])
                                .ok()
                                .and_then(|h| u32::from_str_radix(h, 16).ok())
                                .and_then(char::from_u32);
                            if self.peek() != Some(b'}') {
                                return Err(self.err_at(escape_at, "`\\u{...}` needs `}`"));
                            }
                            self.pos += 1;
                            match ch {
                                Some(ch) => {
                                    let mut tmp = [0u8; 4];
                                    buf.extend_from_slice(ch.encode_utf8(&mut tmp).as_bytes());
                                }
                                None => {
                                    return Err(
                                        self.err_at(escape_at, "`\\u{...}` isn't a character")
                                    )
                                }
                            }
                        }
                        other => {
                            return Err(self.err_at(
                                escape_at,
                                format!("invalid escape `\\{}`", other as char),
                            ))
                        }
                    }
                }
                _ => {
                    // Copy the run of plain bytes up to the next quote, backslash or newline.
                    let run_start = self.pos;
                    while matches!(self.peek(), Some(c) if c != quote && c != b'\\' && c != b'\n' && c != b'\r')
                    {
                        self.pos += 1;
                    }
                    buf.extend_from_slice(&self.s[run_start..self.pos]);
                }
            }
        }
        Ok(buf)
    }

    fn table(&mut self) -> Result<LuaValue, ParseError> {
        let open = self.pos;
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            return Err(self.err(format!("tables nest deeper than {MAX_DEPTH}")));
        }
        self.pos += 1; // `{`
        let mut entries: Vec<(LuaKey, Option<LuaValue>)> = Vec::new();
        let mut index: HashMap<LuaKey, usize> = HashMap::new();
        let mut next_positional: i64 = 1;
        loop {
            self.skip_ws()?;
            match self.peek() {
                None => return Err(self.err_at(open, "unfinished table (no closing `}`)")),
                Some(b'}') => {
                    self.pos += 1;
                    break;
                }
                _ => {}
            }
            let (key, value) =
                if self.peek() == Some(b'[') && !matches!(self.peek_at(1), Some(b'[' | b'=')) {
                    self.pos += 1;
                    let key = self.bracket_key()?;
                    self.skip_ws()?;
                    self.expect(b']')?;
                    self.skip_ws()?;
                    self.expect(b'=')?;
                    (key, self.value()?)
                } else if let Some(name) = self.name_key()? {
                    (LuaKey::Str(name.into_bytes()), self.value()?)
                } else {
                    let key = LuaKey::Int(next_positional);
                    next_positional += 1;
                    (key, self.value()?)
                };
            match index.get(&key) {
                Some(&i) => entries[i].1 = value,
                None => {
                    index.insert(key.clone(), entries.len());
                    entries.push((key, value));
                }
            }
            self.skip_ws()?;
            match self.peek() {
                Some(b',' | b';') => self.pos += 1,
                Some(b'}') => {}
                None => return Err(self.err_at(open, "unfinished table (no closing `}`)")),
                Some(c) => {
                    return Err(self.err(format!(
                        "expected `,` or `}}` after a table entry, found `{}`",
                        c as char
                    )))
                }
            }
        }
        self.depth -= 1;
        Ok(LuaValue::Table(
            entries
                .into_iter()
                .filter_map(|(k, v)| v.map(|v| (k, v)))
                .collect(),
        ))
    }

    /// The key inside `[ ]`: a string, a number or a boolean.
    fn bracket_key(&mut self) -> Result<LuaKey, ParseError> {
        self.skip_ws()?;
        let start = self.pos;
        match self.peek() {
            Some(b'"' | b'\'') => Ok(LuaKey::Str(self.quoted()?)),
            Some(b'-' | b'.' | b'0'..=b'9') => {
                let n = self.number()?;
                Ok(match self.num_value(n, start)? {
                    LuaValue::Int(i) => LuaKey::Int(i),
                    LuaValue::Float(f) => LuaKey::Float(f),
                    _ => unreachable!("a number is an Int or a Float"),
                })
            }
            Some(c) if is_ident_start(c) => match self.ident().as_str() {
                "true" => Ok(LuaKey::Bool(true)),
                "false" => Ok(LuaKey::Bool(false)),
                "nil" => Err(self.err_at(start, "a table key can't be nil")),
                other => Err(self.err_at(
                    start,
                    format!("unexpected name `{other}` as a key: only literals are allowed"),
                )),
            },
            _ => Err(self.err("expected a string, number or boolean key")),
        }
    }

    /// `name =` (not `name ==`): consume it and return the name. Otherwise consume nothing.
    fn name_key(&mut self) -> Result<Option<String>, ParseError> {
        if !matches!(self.peek(), Some(c) if is_ident_start(c)) {
            return Ok(None);
        }
        let save = self.pos;
        let name = self.ident();
        if is_keyword(&name) {
            self.pos = save;
            return Ok(None);
        }
        self.skip_ws()?;
        if self.peek() == Some(b'=') && self.peek_at(1) != Some(b'=') {
            self.pos += 1;
            Ok(Some(name))
        } else {
            self.pos = save;
            Ok(None)
        }
    }
}

fn bytes_to_string(buf: Vec<u8>) -> String {
    match String::from_utf8(buf) {
        Ok(s) => s,
        Err(e) => String::from_utf8_lossy(e.as_bytes()).into_owned(),
    }
}

fn table_to_json(live: &[(LuaKey, LuaValue)]) -> Value {
    if live.is_empty() {
        return Value::Array(Vec::new());
    }
    let n = live.len() as i64;
    // Keys are unique, so n integer keys all within 1..=n are exactly 1..=n.
    if live
        .iter()
        .all(|(k, _)| matches!(k, LuaKey::Int(i) if *i >= 1 && *i <= n))
    {
        let mut items: Vec<&(LuaKey, LuaValue)> = live.iter().collect();
        items.sort_by_key(|(k, _)| match k {
            LuaKey::Int(i) => *i,
            _ => 0,
        });
        return Value::Array(items.into_iter().map(|(_, v)| v.to_json()).collect());
    }
    let mut map = Map::new();
    for (k, v) in live {
        map.insert(k.to_json_key(), v.to_json());
    }
    Value::Object(map)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn one(src: &str) -> Value {
        let v = parse(&format!("X = {src}")).unwrap_or_else(|e| panic!("{src}: {e}"));
        v["X"].clone()
    }

    fn fails(src: &str) -> ParseError {
        parse(src).expect_err(src)
    }

    #[test]
    fn several_top_level_assignments() {
        let v = parse("A = 1\nB = \"two\"; C = { 3 }\nD = nil\n").unwrap();
        assert_eq!(v, json!({ "A": 1, "B": "two", "C": [3] }));
    }

    #[test]
    fn empty_file_is_an_empty_object() {
        assert_eq!(parse("").unwrap(), json!({}));
        assert_eq!(parse("\n-- only a comment\n").unwrap(), json!({}));
        assert_eq!(parse("\u{feff}A = 1").unwrap(), json!({ "A": 1 }));
    }

    #[test]
    fn later_nil_removes_a_top_level_name() {
        assert_eq!(parse("A = 1\nA = nil").unwrap(), json!({}));
    }

    #[test]
    fn numbers() {
        assert_eq!(one("0"), json!(0));
        assert_eq!(one("42"), json!(42));
        assert_eq!(one("-42"), json!(-42));
        assert_eq!(one("- 7"), json!(-7));
        assert_eq!(one("3.5"), json!(3.5));
        assert_eq!(one("-0.25"), json!(-0.25));
        assert_eq!(one(".5"), json!(0.5));
        assert_eq!(one("5."), json!(5));
        assert_eq!(one("1e10"), json!(10_000_000_000_i64));
        assert_eq!(one("1E+3"), json!(1000));
        assert_eq!(one("2.5e-3"), json!(0.0025));
        assert_eq!(one("1.0"), json!(1));
        assert_eq!(one("0x1F"), json!(31));
        assert_eq!(one("0XfF"), json!(255));
        assert_eq!(one("-0x10"), json!(-16));
        assert_eq!(one("1e300"), json!(1e300));
        assert_eq!(one("9007199254740993"), json!(9_007_199_254_740_993_i64));
        assert_eq!(one("99999999999999999999"), json!(1e20));
        assert_eq!(
            one("0xFFFFFFFFFFFFFFFF"),
            json!(18_446_744_073_709_551_615.0)
        );
    }

    #[test]
    fn malformed_numbers() {
        assert!(fails("X = 1e").message.contains("exponent"));
        assert!(fails("X = 0x").message.contains("hexadecimal"));
        assert!(fails("X = 12abc").message.contains("malformed"));
        assert!(fails("X = 1.2.3").message.contains("malformed"));
        assert!(fails("X = -").message.contains("malformed"));
        assert!(fails("X = 0x1p4").message.contains("hexadecimal"));
        assert!(fails("X = 1e999").message.contains("inf"));
    }

    #[test]
    fn strings_and_escapes() {
        assert_eq!(one(r#""plain""#), json!("plain"));
        assert_eq!(one("'single'"), json!("single"));
        assert_eq!(one(r#""say \"hi\"""#), json!("say \"hi\""));
        assert_eq!(one(r#"'it\'s'"#), json!("it's"));
        assert_eq!(one(r#""back\\slash""#), json!("back\\slash"));
        assert_eq!(one(r#""a\nb\tc\rd""#), json!("a\nb\tc\rd"));
        assert_eq!(one(r#""\a\b\f\v""#), json!("\u{7}\u{8}\u{c}\u{b}"));
        assert_eq!(one(r#""\65\066\0677""#), json!("ABC7"));
        assert_eq!(one(r#""\x41\x7a""#), json!("Az"));
        assert_eq!(one(r#""\195\169""#), json!("é"));
        assert_eq!(one(r#""\xC3\xA9""#), json!("é"));
        assert_eq!(one(r#""\u{48}\u{e9}""#), json!("Hé"));
        assert_eq!(one("\"a\\z   \n   b\""), json!("ab"));
        assert_eq!(one("\"line\\\nnext\""), json!("line\nnext"));
        assert_eq!(one(r#""\0""#), json!("\u{0}"));
        assert_eq!(one(r#""""#), json!(""));
        assert_eq!(one("\"Ünïcödé ✓\""), json!("Ünïcödé ✓"));
        assert_eq!(
            one(r#""|cff1eff00|Hitem:2589::::::::23:::::|h[Linen Cloth]|h|r""#),
            json!("|cff1eff00|Hitem:2589::::::::23:::::|h[Linen Cloth]|h|r")
        );
        assert_eq!(one(r#""\255""#), json!("\u{fffd}"));
    }

    #[test]
    fn long_strings() {
        assert_eq!(one("[[two\nlines]]"), json!("two\nlines"));
        assert_eq!(
            one("[[\nfirst newline dropped]]"),
            json!("first newline dropped")
        );
        assert_eq!(one("[==[has ]] inside]==]"), json!("has ]] inside"));
    }

    #[test]
    fn bad_strings() {
        assert!(fails("X = \"open").message.contains("unfinished"));
        assert!(fails("X = \"two\nlines\"").message.contains("newline"));
        assert!(fails(r#"X = "\q""#).message.contains("invalid escape"));
        assert!(fails(r#"X = "\256""#).message.contains("255"));
        assert!(fails(r#"X = "\xZZ""#).message.contains("two hexadecimal"));
        assert!(fails("X = [[open")
            .message
            .contains("unfinished long string"));
    }

    #[test]
    fn booleans_and_nil() {
        assert_eq!(one("true"), json!(true));
        assert_eq!(one("false"), json!(false));
        assert_eq!(
            one(r#"{ a = true, b = false, c = nil, ["d"] = nil }"#),
            json!({ "a": true, "b": false })
        );
    }

    #[test]
    fn arrays() {
        assert_eq!(one("{}"), json!([]));
        assert_eq!(one("{ 1, 2, 3 }"), json!([1, 2, 3]));
        assert_eq!(one("{ 1, 2, 3, }"), json!([1, 2, 3]));
        assert_eq!(one("{ 1; 2; 3; }"), json!([1, 2, 3]));
        assert_eq!(one("{ [1] = \"a\", [2] = \"b\" }"), json!(["a", "b"]));
        assert_eq!(one("{ [2] = \"b\", [1] = \"a\" }"), json!(["a", "b"]));
        assert_eq!(one("{ \"a\", [2] = \"b\" }"), json!(["a", "b"]));
        assert_eq!(one("{ { 1 }, { 2, { 3 } } }"), json!([[1], [2, [3]]]));
        // WoW's own writer: positional values with an index comment.
        assert_eq!(
            one("{\n\t\"x\", -- [1]\n\t\"y\", -- [2]\n}"),
            json!(["x", "y"])
        );
    }

    #[test]
    fn sparse_and_mixed_tables_are_objects() {
        assert_eq!(
            one("{ [1] = \"a\", [3] = \"c\" }"),
            json!({ "1": "a", "3": "c" })
        );
        assert_eq!(one("{ 1, nil, 3 }"), json!({ "1": 1, "3": 3 }));
        assert_eq!(one("{ 1, 2, x = 3 }"), json!({ "1": 1, "2": 2, "x": 3 }));
        assert_eq!(one("{ [0] = \"z\", \"a\" }"), json!({ "0": "z", "1": "a" }));
        assert_eq!(
            one("{ [-1] = 1, [1.5] = 2, [true] = 3 }"),
            json!({ "-1": 1, "1.5": 2, "true": 3 })
        );
        assert_eq!(one("{ [1e1] = 1 }"), json!({ "10": 1 }));
        // A nil at the end leaves exactly 1..n, which is an array.
        assert_eq!(one("{ 1, 2, nil }"), json!([1, 2]));
    }

    #[test]
    fn keys() {
        assert_eq!(
            one(r#"{ ["Player-91-0F00A001"] = 1, name = 2, ['q'] = 3, _x9 = 4 }"#),
            json!({ "Player-91-0F00A001": 1, "name": 2, "q": 3, "_x9": 4 })
        );
        assert_eq!(one(r#"{ ["a"] = 1, ["a"] = 2 }"#), json!({ "a": 2 }));
        assert_eq!(one(r#"{ ["a"] = 1, ["a"] = nil }"#), json!([]));
        assert_eq!(one(r#"{ [ "spaced" ] = 1 }"#), json!({ "spaced": 1 }));
        // Numeric-looking string keys stay strings and never make an array.
        assert_eq!(
            one(r#"{ ["1"] = { n = 1 }, ["2"] = 2 }"#),
            json!({ "1": { "n": 1 }, "2": 2 })
        );
        assert_eq!(one(r#"{ ["6"] = 1 }"#), json!({ "6": 1 }));
    }

    #[test]
    fn bad_tables() {
        assert!(fails("X = { 1, 2").message.contains("unfinished table"));
        assert!(fails("X = { 1 2 }").message.contains("expected `,`"));
        assert!(fails("X = { [nil] = 1 }").message.contains("nil"));
        assert!(fails("X = { [foo] = 1 }").message.contains("only literals"));
        assert!(fails("X = { a == 1 }").message.contains("only literals"));
        assert!(fails("X = { foo }").message.contains("only literals"));
        assert!(fails("X = print(1)").message.contains("only literals"));
        let deep = format!(
            "X = {}{}",
            "{".repeat(MAX_DEPTH + 1),
            "}".repeat(MAX_DEPTH + 1)
        );
        assert!(fails(&deep).message.contains("nest deeper"));
        let ok = format!("X = {}{}", "{".repeat(MAX_DEPTH), "}".repeat(MAX_DEPTH));
        assert!(parse(&ok).is_ok());
    }

    #[test]
    fn bad_top_level() {
        assert!(fails("= 1").message.contains("expected a name"));
        assert!(fails("X 1").message.contains("expected `=`"));
        assert!(fails("X =").message.contains("end of input"));
        assert!(fails("nil = 1").message.contains("can't be assigned"));
    }

    #[test]
    fn comments() {
        let src = "-- header\nA = { -- trailing\n  1, --[[ inline ]] 2,\n  --[==[\n multi ]] line\n ]==]\n  3,\n} -- end\n--[[ last ]]";
        assert_eq!(parse(src).unwrap(), json!({ "A": [1, 2, 3] }));
        assert!(fails("A = 1 --[[ open")
            .message
            .contains("unfinished long comment"));
    }

    #[test]
    fn error_positions() {
        let e = fails("A = {\n  1,\n  @\n}");
        assert_eq!((e.line, e.column), (3, 3));
        assert_eq!(
            e.to_string(),
            "line 3, column 3: unexpected `@`: expected a value"
        );
        let e = fails("A = \"é\" B");
        assert_eq!((e.line, e.column), (1, 10));
    }

    #[test]
    fn crlf_line_endings() {
        let v = parse("A = {\r\n\t[\"x\"] = 1,\r\n\t\"y\", -- [1]\r\n}\r\n").unwrap();
        assert_eq!(v, json!({ "A": { "1": "y", "x": 1 } }));
    }

    #[test]
    fn realistic_fixture() {
        let src = include_str!("../../fixtures/Ryna.lua");
        assert!(src.len() > 4096, "the fixture should be a realistic size");
        let v = parse(src).expect("fixture parses");
        let db = &v["RynaDB"];
        assert_eq!(db["schema"], json!(1));
        assert_eq!(db["flavor"], json!("wowf"));
        let chars = db["characters"]
            .as_object()
            .expect("characters keyed by GUID");
        assert_eq!(chars.len(), 2);
        let aerin = &chars["Player-91-0F00A001"];
        assert_eq!(aerin["fullName"], json!("Aerin-Stormcall"));
        assert!(aerin["gear"].is_array());
        assert!(aerin["containers"].as_array().unwrap().len() > 5);
        assert!(db["journal"].as_array().unwrap().len() >= 10);
        // The client declares one SavedVariable for the addon, so that is all the file holds.
        assert_eq!(v.as_object().unwrap().len(), 1);
    }
}
