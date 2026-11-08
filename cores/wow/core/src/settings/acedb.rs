//! The AceDB layout (AceDB-3.0, which ElvUI, Details, WeakAuras, Questie, Plater and Bartender
//! use): one account-wide SavedVariable per database, the per-character parts under
//! `"Name - Realm"` keys.
//!
//! ```lua
//! ElvDB = {
//!     ["profileKeys"] = { ["Aerin - Stormcall"] = "Default" },   -- which profile each character uses
//!     ["profiles"]    = { ["Default"] = { … } },                  -- the settings, by profile name
//!     ["char"]        = { ["Aerin - Stormcall"] = { … } },        -- per-character data
//!     ["namespaces"]  = { ["Module"] = { ["profiles"] = …, ["char"] = … } },
//!     ["global"] = …, ["realm"] = …, ["faction"] = …, ["class"] = …,
//! }
//! ```
//!
//! Copying one character's settings to another ([`copy_character`]) makes the target what the source
//! is: the target uses the source's profile (a shared one like `Default` stays shared; a profile of
//! the source's own, named for it, is duplicated under the target's name, in every namespace too),
//! and the source's `char` entries are duplicated for the target. Where the source has no entry,
//! the target's is removed; a database with nothing of the source's is left alone. The `realm`, `faction`,
//! `factionrealm`, `class` and `race` sections aren't any one character's and are left alone (an
//! auction history keyed by faction is not settings). ElvUI's `ElvPrivateDB` is a second database
//! of the same layout in the same file, and is copied the same way.

use std::collections::BTreeSet;

use crate::lua::{LuaKey, LuaValue};

/// Whether `v` is an AceDB database: a table with a `profileKeys` table, or a `char` table keyed by
/// `"Name - Realm"`.
pub fn is_acedb(v: &LuaValue) -> bool {
    let table = |k: &str| v.field(k).and_then(LuaValue::as_table);
    table("profileKeys").is_some()
        || table("char").is_some_and(|t| t.iter().any(|(k, _)| is_char_key(k)))
}

fn is_char_key(k: &LuaKey) -> bool {
    matches!(k, LuaKey::Str(b) if String::from_utf8_lossy(b).contains(" - "))
}

fn key_text(k: &LuaKey) -> Option<String> {
    match k {
        LuaKey::Str(b) => Some(String::from_utf8_lossy(b).into_owned()),
        _ => None,
    }
}

/// The `"Name - Realm"` keys a database names (in `profileKeys`, `char`, and each namespace's `char`).
pub fn characters(v: &LuaValue) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let mut take = |t: Option<&LuaValue>| {
        for (k, _) in t.and_then(LuaValue::as_table).into_iter().flatten() {
            if is_char_key(k) {
                out.extend(key_text(k));
            }
        }
    };
    take(v.field("profileKeys"));
    take(v.field("char"));
    for (_, ns) in namespaces(v) {
        take(ns.field("char"));
    }
    out
}

fn namespaces(v: &LuaValue) -> Vec<(LuaKey, LuaValue)> {
    v.field("namespaces")
        .and_then(LuaValue::as_table)
        .cloned()
        .unwrap_or_default()
}

/// The profile `who` uses (its `profileKeys` entry), if it names one.
pub fn profile_of(v: &LuaValue, who: &str) -> Option<String> {
    v.field("profileKeys")?.field(who)?.as_text()
}

/// Make `to`'s entries in `v` what `from`'s are (`profileKeys`, `char`, and the profile when it's
/// `from`'s own, in every namespace too): where `from` has an entry `to` gets a copy, and where
/// `from` has none (AceDB drops empty tables at logout) `to`'s is removed. A database with nothing
/// of `from`'s is left exactly as it is. Returns whether anything changed.
pub fn copy_character(v: &mut LuaValue, from: &str, to: &str) -> bool {
    if from == to || !is_acedb(v) || settings_of(v, from).is_none() {
        return false;
    }
    let before = v.clone();
    let from_key = LuaKey::str(from);
    let to_key = LuaKey::str(to);
    // The profile: shared stays shared; the source's own is duplicated as the target's own.
    let profile = profile_of(v, from);
    let own_profile = profile.as_deref() == Some(from);
    if let Some(pk) = v.field_mut("profileKeys") {
        match &profile {
            Some(_) if own_profile => pk.set(to_key.clone(), LuaValue::str(to)),
            Some(p) => pk.set(to_key.clone(), LuaValue::str(p)),
            // The source uses the addon's default: so does the target.
            None => {
                pk.remove(&to_key);
            }
        }
    }
    let mirror = |t: Option<&mut LuaValue>| {
        if let Some(t) = t {
            match t.get(&from_key).cloned() {
                Some(src) => t.set(to_key.clone(), src),
                None => {
                    t.remove(&to_key);
                }
            }
        }
    };
    if own_profile {
        mirror(v.field_mut("profiles"));
    }
    mirror(v.field_mut("char"));
    if let Some(nss) = v.field_mut("namespaces").and_then(LuaValue::as_table_mut) {
        for (_, ns) in nss.iter_mut() {
            if own_profile {
                mirror(ns.field_mut("profiles"));
            }
            mirror(ns.field_mut("char"));
        }
    }
    *v != before
}

/// What `who`'s settings are in a database, for comparing two characters: the profile it uses
/// (the profile's settings, not its name), its `char` entry, and the same per namespace. None when
/// the database has nothing of `who`'s.
pub fn settings_of(v: &LuaValue, who: &str) -> Option<LuaValue> {
    let key = LuaKey::str(who);
    let profile = profile_of(v, who);
    let mut parts = Vec::new();
    let mut add = |name: &str, x: Option<&LuaValue>| {
        if let Some(x) = x {
            parts.push((LuaKey::str(name), x.canonical()));
        }
    };
    let from_profiles = |db: &LuaValue| {
        profile
            .as_deref()
            .and_then(|p| db.field("profiles")?.field(p))
            .cloned()
    };
    add("profile", from_profiles(v).as_ref());
    add("char", v.field("char").and_then(|c| c.get(&key)));
    let mut ns_parts = Vec::new();
    for (name, ns) in namespaces(v) {
        let mut one = Vec::new();
        if let Some(p) = from_profiles(&ns) {
            one.push((LuaKey::str("profile"), p.canonical()));
        }
        if let Some(c) = ns.field("char").and_then(|c| c.get(&key)) {
            one.push((LuaKey::str("char"), c.canonical()));
        }
        if !one.is_empty() {
            ns_parts.push((name, LuaValue::Table(one)));
        }
    }
    let has_key = profile.is_some();
    if !ns_parts.is_empty() {
        ns_parts.sort_by(|a, b| a.0.cmp(&b.0));
        parts.push((LuaKey::str("namespaces"), LuaValue::Table(ns_parts)));
    }
    (has_key || !parts.is_empty()).then_some(LuaValue::Table(parts))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lua::parse_bytes;

    fn db(src: &str) -> LuaValue {
        parse_bytes(format!("X = {src}").as_bytes())
            .unwrap()
            .remove(0)
            .1
    }

    const ELV: &str = r#"{
        ["profileKeys"] = { ["Aerin - Stormcall"] = "Default", ["Bryn - Doomhowl"] = "Bryn - Doomhowl" },
        ["profiles"] = { ["Default"] = { ["a"] = 1 }, ["Bryn - Doomhowl"] = { ["a"] = 2 } },
        ["char"] = { ["Aerin - Stormcall"] = { ["c"] = 1 } },
        ["namespaces"] = {
            ["Mod"] = {
                ["profiles"] = { ["Bryn - Doomhowl"] = { ["m"] = 2 } },
                ["char"] = { ["Aerin - Stormcall"] = { ["n"] = 1 } },
            },
        },
        ["faction"] = { ["Alliance"] = { ["auctions"] = 9 } },
        ["global"] = { ["g"] = 1 },
    }"#;

    #[test]
    fn recognises_the_layout() {
        assert!(is_acedb(&db(ELV)));
        assert!(is_acedb(&db(r#"{ ["char"] = { ["A - B"] = {} } }"#)));
        assert!(!is_acedb(&db(r#"{ ["char"] = { ["plain"] = {} } }"#)));
        assert!(!is_acedb(&db(r#"{ ["x"] = 1 }"#)));
        assert!(!is_acedb(&db("3")));
        let names: Vec<String> = characters(&db(ELV)).into_iter().collect();
        assert_eq!(names, ["Aerin - Stormcall", "Bryn - Doomhowl"]);
    }

    #[test]
    fn a_shared_profile_stays_shared_and_char_is_duplicated() {
        let mut v = db(ELV);
        assert!(copy_character(
            &mut v,
            "Aerin - Stormcall",
            "Cael - Doomhowl"
        ));
        assert_eq!(
            profile_of(&v, "Cael - Doomhowl").as_deref(),
            Some("Default")
        );
        let cael = LuaKey::str("Cael - Doomhowl");
        assert_eq!(
            v.field("char").unwrap().get(&cael),
            Some(&db(r#"{ ["c"] = 1 }"#))
        );
        let ns = v.field("namespaces").unwrap().field("Mod").unwrap();
        assert_eq!(
            ns.field("char").unwrap().get(&cael),
            Some(&db(r#"{ ["n"] = 1 }"#))
        );
        // No profile was made for it, and what's no one character's is untouched.
        assert!(v.field("profiles").unwrap().get(&cael).is_none());
        assert_eq!(v.field("faction"), db(ELV).field("faction"));
        assert_eq!(
            settings_of(&v, "Cael - Doomhowl"),
            settings_of(&v, "Aerin - Stormcall")
        );
        // Again: nothing more changes.
        assert!(!copy_character(
            &mut v,
            "Aerin - Stormcall",
            "Cael - Doomhowl"
        ));
    }

    #[test]
    fn an_own_profile_is_duplicated_under_the_targets_name() {
        let mut v = db(ELV);
        assert!(copy_character(
            &mut v,
            "Bryn - Doomhowl",
            "Aerin - Stormcall"
        ));
        let aerin = LuaKey::str("Aerin - Stormcall");
        assert_eq!(
            profile_of(&v, "Aerin - Stormcall").as_deref(),
            Some("Aerin - Stormcall")
        );
        assert_eq!(
            v.field("profiles").unwrap().get(&aerin),
            Some(&db(r#"{ ["a"] = 2 }"#))
        );
        let ns = v.field("namespaces").unwrap().field("Mod").unwrap();
        assert_eq!(
            ns.field("profiles").unwrap().get(&aerin),
            Some(&db(r#"{ ["m"] = 2 }"#))
        );
        // Bryn has no char entry, so Aerin's goes: the target is made what the source is.
        assert!(v.field("char").unwrap().get(&aerin).is_none());
        // The source and the "Default" profile others use are as they were.
        assert_eq!(
            v.field("profiles").unwrap().field("Default"),
            db(ELV).field("profiles").unwrap().field("Default")
        );
        assert_eq!(
            profile_of(&v, "Bryn - Doomhowl").as_deref(),
            Some("Bryn - Doomhowl")
        );
    }

    #[test]
    fn a_database_with_nothing_of_the_sources_is_left_alone() {
        // ElvPrivateDB-like: the target chose a profile; the source isn't in it at all.
        let mut v = db(
            r#"{ ["profileKeys"] = { ["T - R"] = "Other" }, ["profiles"] = { ["Other"] = { ["x"] = 1 } } }"#,
        );
        let before = v.clone();
        assert!(!copy_character(&mut v, "S - R", "T - R"));
        assert_eq!(v, before);
        assert_eq!(profile_of(&v, "T - R").as_deref(), Some("Other"));
        assert!(!copy_character(&mut v, "T - R", "T - R"));
        let mut plain = db(r#"{ ["x"] = 1 }"#);
        assert!(!copy_character(&mut plain, "S - R", "T - R"));
    }

    #[test]
    fn what_the_source_lacks_the_target_loses() {
        // The source has char data but no profileKeys entry (the addon's default) and no own
        // profile; the target had a chosen profile, its own profile, char and namespace data.
        let mut v = db(r#"{
            ["profileKeys"] = { ["T - R"] = "T - R" },
            ["profiles"] = { ["T - R"] = { ["mine"] = 1 } },
            ["char"] = { ["S - R"] = { ["x"] = 1 }, ["T - R"] = { ["old"] = 1 } },
            ["namespaces"] = { ["Mod"] = { ["char"] = { ["T - R"] = { ["old"] = 2 } } } },
        }"#);
        assert!(copy_character(&mut v, "S - R", "T - R"));
        assert_eq!(
            profile_of(&v, "T - R"),
            None,
            "the target falls back to the default too"
        );
        let t = LuaKey::str("T - R");
        assert_eq!(
            v.field("char").unwrap().get(&t),
            Some(&db(r#"{ ["x"] = 1 }"#))
        );
        let ns = v.field("namespaces").unwrap().field("Mod").unwrap();
        assert_eq!(ns.field("char").unwrap().get(&t), None);
        // The target's own profile is no one's now; it's left (AceDB keeps unused profiles).
        assert!(v.field("profiles").unwrap().get(&t).is_some());
        assert_eq!(settings_of(&v, "T - R"), settings_of(&v, "S - R"));

        // An own profile the source no longer has a table for: the target's goes too.
        let mut w = db(r#"{
            ["profileKeys"] = { ["S - R"] = "S - R", ["T - R"] = "T - R" },
            ["profiles"] = { ["T - R"] = { ["mine"] = 1 } },
        }"#);
        assert!(copy_character(&mut w, "S - R", "T - R"));
        assert_eq!(profile_of(&w, "T - R").as_deref(), Some("T - R"));
        assert!(w.field("profiles").unwrap().get(&t).is_none());
    }

    #[test]
    fn settings_compare_by_content_not_by_profile_name() {
        let v = db(ELV);
        let a = settings_of(&v, "Aerin - Stormcall").unwrap();
        let b = settings_of(&v, "Bryn - Doomhowl").unwrap();
        assert_ne!(a, b);
        assert_eq!(settings_of(&v, "Nobody - Here"), None);
        let same = db(
            r#"{ ["profileKeys"] = { ["A - R"] = "P", ["B - R"] = "Q" }, ["profiles"] = { ["P"] = { ["x"] = 1, ["y"] = 2 }, ["Q"] = { ["y"] = 2, ["x"] = 1 } } }"#,
        );
        assert_eq!(settings_of(&same, "A - R"), settings_of(&same, "B - R"));
    }
}
