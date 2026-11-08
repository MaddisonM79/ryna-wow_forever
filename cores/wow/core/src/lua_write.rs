//! Writing SavedVariables Lua, the form the game writes: top-level `Name = <value>` assignments,
//! tables one entry a line, tab-indented, `["key"] = value,` with the positional entries first
//! (`"value", -- [1]`) and the rest in a fixed order (numbers, then strings by bytes, then
//! booleans), so the same value always writes the same bytes. Strings are escaped the way Lua
//! reads them back (`\"`, `\\`, `\n`, `\r`, `\t`, other control bytes as `\ddd`); bytes from 0x80
//! up are written as they are, so a file that wasn't UTF-8 stays as it was.
//!
//! [`write_file`] takes [`crate::lua::parse_bytes`]'s tree and loses nothing; [`from_json`] takes
//! [`crate::lua::parse`]'s JSON instead, for a value that only ever was JSON (an object key that is
//! an integer's digits is written as that integer, which is what JSON can't tell apart).
//! [`write_atomic`] puts bytes in place through a temporary file and a rename, so a reader (the
//! game) never sees half a file.

use std::io::Write as _;
use std::path::Path;

use serde_json::Value;

use crate::lua::{LuaKey, LuaValue};

/// A whole SavedVariables file: each name and its value, in the order given.
pub fn write_file(vars: &[(String, LuaValue)]) -> Vec<u8> {
    let mut out = Vec::new();
    for (name, value) in vars {
        out.extend_from_slice(name.as_bytes());
        out.extend_from_slice(b" = ");
        write_value(&mut out, value, 0);
        out.push(b'\n');
    }
    out
}

/// [`crate::lua::parse`]'s JSON object (`{ "<Name>": <value> }`) as a file. Anything but an object
/// writes nothing.
pub fn write_json_file(v: &Value) -> Vec<u8> {
    let vars: Vec<(String, LuaValue)> = v
        .as_object()
        .map(|m| m.iter().map(|(k, v)| (k.clone(), from_json(v))).collect())
        .unwrap_or_default();
    write_file(&vars)
}

/// A JSON value as Lua: an array is positional entries, an object's keys are strings unless they
/// are an integer's digits (`"12"`, `"-3"`, not `"012"`), `null` is an empty table's absence
/// (dropped from tables, an empty table on its own).
pub fn from_json(v: &Value) -> LuaValue {
    match v {
        Value::Null => LuaValue::Table(Vec::new()),
        Value::Bool(b) => LuaValue::Bool(*b),
        Value::Number(n) => match n.as_i64() {
            Some(i) => LuaValue::Int(i),
            None => LuaValue::Float(n.as_f64().unwrap_or(0.0)),
        },
        Value::String(s) => LuaValue::Str(s.as_bytes().to_vec()),
        Value::Array(items) => LuaValue::Table(
            items
                .iter()
                .enumerate()
                .filter(|(_, v)| !v.is_null())
                .map(|(i, v)| (LuaKey::Int(i as i64 + 1), from_json(v)))
                .collect(),
        ),
        Value::Object(map) => LuaValue::Table(
            map.iter()
                .filter(|(_, v)| !v.is_null())
                .map(|(k, v)| (json_key(k), from_json(v)))
                .collect(),
        ),
    }
}

fn json_key(k: &str) -> LuaKey {
    let digits = k.strip_prefix('-').unwrap_or(k);
    let canonical = !digits.is_empty()
        && digits.bytes().all(|c| c.is_ascii_digit())
        && (digits == "0" || !digits.starts_with('0'))
        && k != "-0";
    match k.parse::<i64>() {
        Ok(i) if canonical => LuaKey::Int(i),
        _ => LuaKey::str(k),
    }
}

fn indent(out: &mut Vec<u8>, depth: usize) {
    out.extend(std::iter::repeat_n(b'\t', depth));
}

/// One value at `depth` (its entries are indented `depth + 1` tabs).
fn write_value(out: &mut Vec<u8>, v: &LuaValue, depth: usize) {
    match v {
        LuaValue::Bool(b) => out.extend_from_slice(if *b { b"true" } else { b"false" }),
        LuaValue::Int(i) => out.extend_from_slice(i.to_string().as_bytes()),
        LuaValue::Float(f) => out.extend_from_slice(float_text(*f).as_bytes()),
        LuaValue::Str(b) => write_string(out, b),
        LuaValue::Table(entries) => {
            out.extend_from_slice(b"{\n");
            // The array part: 1, 2, 3 … for as long as it runs.
            let mut positional = Vec::new();
            let mut n = 1i64;
            while let Some(v) = entries
                .iter()
                .find(|(k, _)| *k == LuaKey::Int(n))
                .map(|(_, v)| v)
            {
                positional.push(v);
                n += 1;
            }
            for (i, v) in positional.iter().enumerate() {
                indent(out, depth + 1);
                write_value(out, v, depth + 1);
                out.extend_from_slice(format!(", -- [{}]\n", i + 1).as_bytes());
            }
            let mut keyed: Vec<&(LuaKey, LuaValue)> = entries
                .iter()
                .filter(|(k, _)| !matches!(k, LuaKey::Int(i) if *i >= 1 && *i < n))
                .collect();
            keyed.sort_by(|a, b| a.0.cmp(&b.0));
            for (k, v) in keyed {
                indent(out, depth + 1);
                out.push(b'[');
                match k {
                    LuaKey::Int(i) => out.extend_from_slice(i.to_string().as_bytes()),
                    LuaKey::Float(f) => out.extend_from_slice(float_text(*f).as_bytes()),
                    LuaKey::Str(b) => write_string(out, b),
                    LuaKey::Bool(b) => out.extend_from_slice(if *b { b"true" } else { b"false" }),
                }
                out.extend_from_slice(b"] = ");
                write_value(out, v, depth + 1);
                out.extend_from_slice(b",\n");
            }
            indent(out, depth);
            out.push(b'}');
        }
    }
}

/// A float as text Lua reads back to the same double: plain digits where that's short and exact,
/// else Rust's shortest exponent form (`1e-7`, `9.007199254740994e15`), which also keeps a huge
/// whole float from reading back as an integer literal.
fn float_text(f: f64) -> String {
    if !f.is_finite() {
        // The reader never produces one; write something Lua reads as the same.
        return if f.is_nan() {
            "(0/0)".into()
        } else if f > 0.0 {
            "1e999".into()
        } else {
            "-1e999".into()
        };
    }
    let a = f.abs();
    if f.fract() != 0.0 && (1e-5..1e15).contains(&a) {
        format!("{f}")
    } else {
        format!("{f:e}")
    }
}

fn write_string(out: &mut Vec<u8>, b: &[u8]) {
    out.push(b'"');
    for &c in b {
        match c {
            b'"' => out.extend_from_slice(b"\\\""),
            b'\\' => out.extend_from_slice(b"\\\\"),
            b'\n' => out.extend_from_slice(b"\\n"),
            b'\r' => out.extend_from_slice(b"\\r"),
            b'\t' => out.extend_from_slice(b"\\t"),
            // Three digits always, so a digit after it can't join the escape.
            0..=0x1f | 0x7f => out.extend_from_slice(format!("\\{c:03}").as_bytes()),
            _ => out.push(c),
        }
    }
    out.push(b'"');
}

/// Write `bytes` to `path` through `<path>.ryna-tmp` in the same folder, flushed to disk, then
/// renamed over `path` (which replaces it on every OS). The folder must exist.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let name = path
        .file_name()
        .ok_or_else(|| std::io::Error::other(format!("{} has no file name", path.display())))?;
    let mut tmp_name = name.to_os_string();
    tmp_name.push(".ryna-tmp");
    let tmp = path.with_file_name(tmp_name);
    let result = (|| {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
        drop(f);
        std::fs::rename(&tmp, path)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lua::{parse, parse_bytes};
    use serde_json::json;

    fn canonical(vars: &[(String, LuaValue)]) -> Vec<(String, LuaValue)> {
        vars.iter()
            .map(|(n, v)| (n.clone(), v.canonical()))
            .collect()
    }

    const SAMPLE: &str = r#"
ElvDB = {
	["profileKeys"] = {
		["Aerin - Stormcall"] = "Default",
		["Bryn - Doomhowl"] = "Bryn - Doomhowl",
	},
	["profiles"] = {
		["Default"] = {
			["general"] = { ["fontSize"] = 12, ["scale"] = 0.711111111111111, ["on"] = true },
			["list"] = { "a", "b\"q\\", nil, "d" },
			[5] = "five", ["5"] = "string five", [1.5] = "x", [true] = false,
			["empty"] = {},
		},
	},
}
ElvPrivateDB = { ["profileKeys"] = { ["Aerin - Stormcall"] = "Aerin - Stormcall" } }
Count = 3
Big = 1e300
Tiny = -2.5e-7
Text = "tab\there\nnew\0zero\127del é"
"#;

    #[test]
    fn writes_what_it_read_back_to_the_same_tree() {
        let vars = parse_bytes(SAMPLE.as_bytes()).unwrap();
        let written = write_file(&vars);
        let again = parse_bytes(&written).unwrap();
        assert_eq!(canonical(&again), canonical(&vars));
        // And the JSON reader sees the same file in both.
        let text = String::from_utf8(written.clone()).unwrap();
        assert_eq!(parse(&text).unwrap(), parse(SAMPLE).unwrap());
        // Deterministic: writing the re-read tree gives the same bytes.
        assert_eq!(write_file(&again), written);
    }

    #[test]
    fn the_games_layout() {
        let vars = parse_bytes(
            b"X = { [\"b\"] = 1, [\"a\"] = { \"p\", \"q\" }, [3] = true, [\"e\"] = {} }\nN = 4\n",
        )
        .unwrap();
        let text = String::from_utf8(write_file(&vars)).unwrap();
        assert_eq!(
            text,
            "X = {\n\t[3] = true,\n\t[\"a\"] = {\n\t\t\"p\", -- [1]\n\t\t\"q\", -- [2]\n\t},\n\t[\"b\"] = 1,\n\t[\"e\"] = {\n\t},\n}\nN = 4\n"
        );
    }

    #[test]
    fn integer_keys_and_their_digits_as_strings_stay_apart() {
        let vars = parse_bytes(br#"X = { [1] = "int", ["1"] = "str", [2] = "two" }"#).unwrap();
        let again = parse_bytes(&write_file(&vars)).unwrap();
        let x = &again[0].1;
        assert_eq!(x.get(&LuaKey::Int(1)), Some(&LuaValue::str("int")));
        assert_eq!(x.get(&LuaKey::str("1")), Some(&LuaValue::str("str")));
        assert_eq!(x.get(&LuaKey::Int(2)), Some(&LuaValue::str("two")));
    }

    #[test]
    fn bytes_that_arent_utf8_survive() {
        let src = b"X = \"\xff\xfe raw\"\n";
        let vars = parse_bytes(src).unwrap();
        assert_eq!(vars[0].1, LuaValue::Str(b"\xff\xfe raw".to_vec()));
        assert_eq!(write_file(&vars), src.to_vec());
    }

    #[test]
    fn floats_read_back_exactly() {
        for f in [
            0.1,
            -0.5,
            1.0 / 3.0,
            1e-7,
            123_456.789,
            9.007_199_254_740_994e15,
            1e300,
        ] {
            let vars = vec![("F".to_string(), LuaValue::Float(f))];
            let again = parse_bytes(&write_file(&vars)).unwrap();
            assert_eq!(again[0].1, LuaValue::Float(f), "{f}");
        }
    }

    #[test]
    fn from_json_round_trips_through_parse() {
        let src = include_str!("../../fixtures/Ryna.lua");
        let v = parse(src).unwrap();
        let written = String::from_utf8(write_json_file(&v)).unwrap();
        assert_eq!(parse(&written).unwrap(), v);
        let v = json!({ "A": { "12": "n", "012": "s", "-3": 1, "x": [1, { "y": null }] } });
        let back = parse(&String::from_utf8(write_json_file(&v)).unwrap()).unwrap();
        assert_eq!(
            back,
            json!({ "A": { "12": "n", "012": "s", "-3": 1, "x": [1, []] } })
        );
        let a = from_json(&v["A"]);
        assert_eq!(a.get(&LuaKey::Int(12)), Some(&LuaValue::str("n")));
        assert_eq!(a.get(&LuaKey::str("012")), Some(&LuaValue::str("s")));
    }

    #[test]
    fn atomic_write_replaces_and_leaves_no_temp_file() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("ElvUI.lua");
        std::fs::write(&p, "old").unwrap();
        write_atomic(&p, b"new").unwrap();
        assert_eq!(std::fs::read(&p).unwrap(), b"new");
        let names: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names, vec![std::ffi::OsString::from("ElvUI.lua")]);
        assert!(write_atomic(&dir.path().join("missing/x.lua"), b"x").is_err());
    }
}
