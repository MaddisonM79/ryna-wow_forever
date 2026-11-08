//! Copying settings from one character to another on the same account, and the diff between two.
//!
//! Two tiers (docs/ryna-agent-cores-mods.md 7.2). A per-character file
//! (`<Realm>/<Name>/SavedVariables/<Addon>.lua`, or one of the game's own files) is copied as it
//! is. An addon whose account-wide file holds AceDB databases has the target's entries made from
//! the source's in that file ([`super::acedb`]), and the file written back. An account-wide file
//! that isn't AceDB is already every character's on the account, so a copy between characters
//! leaves it and says so. Faction plays no part: a copy to a Horde character from an Alliance one
//! is a copy to another character.
//!
//! [`plan`] reads and decides everything and writes nothing (the dry run, and the app's preview);
//! [`apply`] writes a plan, each replaced file kept as `<file>.ryna.bak`, and refuses while the game
//! runs unless forced.

use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use serde::Serialize;

use super::acedb;
use super::inventory::{AddonSettings, Character, Inventory, GAME_FILES};
use crate::lua::{self, LuaValue};
use crate::lua_write;

/// How a planned file comes to be.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", tag = "how")]
pub enum WriteKind {
    /// The source character's file, as it is (`from` is its account-relative path).
    File { from: String },
    /// The account-wide file, its AceDB databases (`variables`) given the target's entries.
    AceDb { variables: Vec<String> },
}

/// One file a copy writes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlannedWrite {
    /// The addon, or the game file's name.
    pub item: String,
    /// Account-relative, `/`-separated.
    pub path: String,
    #[serde(flatten)]
    pub kind: WriteKind,
    /// A file is there now, and will be kept as `<path>.ryna.bak`.
    pub replaces: bool,
    pub bytes: u64,
    #[serde(skip)]
    pub data: Vec<u8>,
}

/// Something asked for (or found) that the copy leaves, and why.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Skipped {
    pub item: String,
    pub reason: String,
}

/// What a copy would do.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CopyPlan {
    pub from: Character,
    pub to: Character,
    pub writes: Vec<PlannedWrite>,
    pub skipped: Vec<Skipped>,
    /// The target has no folder yet (it's made: the game uses it at that character's first login).
    pub new_folder: bool,
    #[serde(skip)]
    root: PathBuf,
}

/// What [`apply`] wrote: account-relative paths, and the `.ryna.bak` copies made.
pub type Applied = super::Written;

fn read(inv: &Inventory, rel: &str) -> Option<Vec<u8>> {
    std::fs::read(inv.abs(rel)).ok()
}

/// What copying `from`'s settings to `to` would write: every addon with settings when `items` is
/// None, else just those named (addons by folder name, or the game's files by name). Writes
/// nothing.
pub fn plan(
    inv: &Inventory,
    from: &Character,
    to: &Character,
    items: Option<&[String]>,
) -> Result<CopyPlan> {
    if from == to {
        bail!("{from} and {to} are the same character");
    }
    let mut chosen_addons: Vec<&AddonSettings> = Vec::new();
    let mut chosen_game: Vec<&str> = Vec::new();
    match items {
        None => chosen_addons.extend(inv.addons.iter()),
        Some(names) => {
            for n in names {
                if let Some(g) = GAME_FILES.iter().find(|g| g.eq_ignore_ascii_case(n)) {
                    chosen_game.push(g);
                } else if let Some(a) = inv.addon(n) {
                    chosen_addons.push(a);
                } else {
                    bail!("there's no addon with settings, or game file, called {n}");
                }
            }
        }
    }
    let mut p = CopyPlan {
        from: from.clone(),
        to: to.clone(),
        writes: Vec::new(),
        skipped: Vec::new(),
        new_folder: !inv.abs(&to.rel_dir()).is_dir(),
        root: inv.root.clone(),
    };
    let add = |p: &mut CopyPlan, item: &str, path: String, kind: WriteKind, data: Vec<u8>| {
        if read(inv, &path).as_deref() == Some(&data[..]) {
            p.skipped.push(Skipped {
                item: item.to_string(),
                reason: format!("{path} is already the same for {to}"),
            });
            return;
        }
        p.writes.push(PlannedWrite {
            item: item.to_string(),
            replaces: inv.abs(&path).is_file(),
            bytes: data.len() as u64,
            path,
            kind,
            data,
        });
    };
    for a in chosen_addons {
        let before = p.writes.len() + p.skipped.len();
        let file = format!("{}.lua", a.addon);
        // Tier 1: the source's own file.
        let src = format!("{}/SavedVariables/{file}", from.rel_dir());
        if let Some(data) = read(inv, &src) {
            let dst = format!("{}/SavedVariables/{file}", to.rel_dir());
            add(&mut p, &a.addon, dst, WriteKind::File { from: src }, data);
        }
        // Tier 2: the account file's AceDB databases.
        let account_rel = format!("SavedVariables/{file}");
        if let Some(bytes) = read(inv, &account_rel) {
            match lua::parse_bytes(&bytes) {
                Ok(mut vars) if vars.iter().any(|(_, v)| acedb::is_acedb(v)) => {
                    let (src_key, dst_key) = (from.acedb_key(), to.acedb_key());
                    let has_source = vars.iter().any(|(_, v)| {
                        acedb::is_acedb(v) && acedb::settings_of(v, &src_key).is_some()
                    });
                    let mut changed = Vec::new();
                    if has_source {
                        for (name, v) in vars.iter_mut() {
                            if acedb::copy_character(v, &src_key, &dst_key) {
                                changed.push(name.clone());
                            }
                        }
                    }
                    if !changed.is_empty() {
                        let data = lua_write::write_file(&vars);
                        add(
                            &mut p,
                            &a.addon,
                            account_rel,
                            WriteKind::AceDb { variables: changed },
                            data,
                        );
                    } else if has_source {
                        p.skipped.push(Skipped {
                            item: a.addon.clone(),
                            reason: format!("its account settings are already the same for {to}"),
                        });
                    }
                }
                Ok(_) => {
                    if p.writes.len() + p.skipped.len() == before {
                        p.skipped.push(Skipped {
                            item: a.addon.clone(),
                            reason: "its settings are account-wide: every character on this account already shares them".into(),
                        });
                    }
                }
                Err(e) => p.skipped.push(Skipped {
                    item: a.addon.clone(),
                    reason: format!(
                        "{account_rel} couldn't be read ({e}), so its account settings were left"
                    ),
                }),
            }
        }
        if p.writes.len() + p.skipped.len() == before && items.is_some() {
            p.skipped.push(Skipped {
                item: a.addon.clone(),
                reason: format!("{from} has no settings of its own for it"),
            });
        }
    }
    for g in chosen_game {
        let src = format!("{}/{g}", from.rel_dir());
        match read(inv, &src) {
            Some(data) => {
                let dst = format!("{}/{g}", to.rel_dir());
                add(&mut p, g, dst, WriteKind::File { from: src }, data);
            }
            None => p.skipped.push(Skipped {
                item: g.to_string(),
                reason: format!("{from} has no {g}"),
            }),
        }
    }
    Ok(p)
}

/// Write `plan` ([`super::write_all`]): each file it replaces kept as `<file>.ryna.bak` first,
/// `game_running` asked right before the first write (refused unless `force`), and everything
/// written put back if a write fails.
pub fn apply(plan: &CopyPlan, game_running: &dyn Fn() -> bool, force: bool) -> Result<Applied> {
    let files: Vec<(&str, &[u8])> = plan
        .writes
        .iter()
        .map(|w| (w.path.as_str(), w.data.as_slice()))
        .collect();
    super::write_all(&plan.root, &files, game_running, force)
        .with_context(|| format!("copying {}'s settings to {}", plan.from, plan.to))
}

/// How one addon (or game file) compares between two characters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Difference {
    Same,
    Differ,
    /// Only the first has settings for it.
    OnlyA,
    /// Only the second has.
    OnlyB,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffRow {
    pub item: String,
    pub game_file: bool,
    pub difference: Difference,
}

/// One comparable piece: a file's bytes (read as Lua where it parses) or an AceDB character's
/// settings.
#[derive(Debug, Clone, PartialEq)]
enum Part {
    Lua(Vec<(String, LuaValue)>),
    Bytes(Vec<u8>),
    Db(LuaValue),
}

fn file_part(bytes: Vec<u8>) -> Part {
    match lua::parse_bytes(&bytes) {
        Ok(vars) => Part::Lua(vars.into_iter().map(|(n, v)| (n, v.canonical())).collect()),
        Err(_) => Part::Bytes(bytes),
    }
}

fn compare(
    item: &str,
    game_file: bool,
    pairs: Vec<(Option<Part>, Option<Part>)>,
) -> Option<DiffRow> {
    let has_a = pairs.iter().any(|(a, _)| a.is_some());
    let has_b = pairs.iter().any(|(_, b)| b.is_some());
    let difference = match (has_a, has_b) {
        (false, false) => return None,
        (true, false) => Difference::OnlyA,
        (false, true) => Difference::OnlyB,
        (true, true) if pairs.iter().all(|(a, b)| a == b) => Difference::Same,
        _ => Difference::Differ,
    };
    Some(DiffRow {
        item: item.to_string(),
        game_file,
        difference,
    })
}

/// Which addons (and game files) differ between `a` and `b`: their own files compared by content
/// (as Lua, so the order the game wrote entries in doesn't count), and their AceDB settings
/// compared by what they are, not by the profile's name. Addons neither has settings for are left
/// out.
pub fn diff(inv: &Inventory, a: &Character, b: &Character) -> Vec<DiffRow> {
    let mut rows = Vec::new();
    for addon in &inv.addons {
        let file = format!("{}.lua", addon.addon);
        let own = |c: &Character| {
            read(inv, &format!("{}/SavedVariables/{file}", c.rel_dir())).map(file_part)
        };
        let mut pairs = vec![(own(a), own(b))];
        if addon.acedb {
            if let Some(Ok(vars)) =
                read(inv, &format!("SavedVariables/{file}")).map(|b| lua::parse_bytes(&b))
            {
                for (_, v) in vars.iter().filter(|(_, v)| acedb::is_acedb(v)) {
                    pairs.push((
                        acedb::settings_of(v, &a.acedb_key()).map(Part::Db),
                        acedb::settings_of(v, &b.acedb_key()).map(Part::Db),
                    ));
                }
            }
        }
        rows.extend(compare(&addon.addon, false, pairs));
    }
    for g in GAME_FILES {
        let one = |c: &Character| read(inv, &format!("{}/{g}", c.rel_dir())).map(Part::Bytes);
        rows.extend(compare(g, true, vec![(one(a), one(b))]));
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lua::LuaKey;
    use crate::settings::inventory::inventory;
    use crate::settings::testing::{account_dir, elvui_tree};

    fn ch(s: &str) -> Character {
        Character::parse(s).unwrap()
    }

    fn names(v: &[String]) -> Vec<&str> {
        v.iter().map(String::as_str).collect()
    }

    #[test]
    fn a_copy_to_a_new_character_plans_files_and_acedb_and_writes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let i = elvui_tree(dir.path());
        let inv = inventory(&i, "ACCOUNT1").unwrap();
        let before = std::fs::read(account_dir(&i).join("SavedVariables/ElvUI.lua")).unwrap();
        let p = plan(&inv, &ch("Aerin-Stormcall"), &ch("Cael-Doomhowl"), None).unwrap();
        let writes: Vec<(&str, &str)> = p
            .writes
            .iter()
            .map(|w| (w.item.as_str(), w.path.as_str()))
            .collect();
        assert_eq!(
            writes,
            [
                ("Details", "Doomhowl/Cael/SavedVariables/Details.lua"),
                ("ElvUI", "Doomhowl/Cael/SavedVariables/ElvUI.lua"),
                ("ElvUI", "SavedVariables/ElvUI.lua"),
            ]
        );
        assert!(!p.new_folder);
        assert!(!p.writes[0].replaces && p.writes[2].replaces);
        assert_eq!(
            p.writes[2].kind,
            WriteKind::AceDb {
                variables: vec!["ElvDB".into(), "ElvPrivateDB".into()]
            }
        );
        assert!(
            matches!(&p.writes[0].kind, WriteKind::File { from } if from == "Stormcall/Aerin/SavedVariables/Details.lua")
        );
        // Plain and Gone are account-wide and not AceDB: already every character's.
        let skipped: Vec<&str> = p.skipped.iter().map(|s| s.item.as_str()).collect();
        assert_eq!(skipped, ["Gone", "Plain"]);
        assert!(p.skipped.iter().all(|s| s.reason.contains("account-wide")));
        // A plan writes nothing.
        assert_eq!(
            std::fs::read(account_dir(&i).join("SavedVariables/ElvUI.lua")).unwrap(),
            before
        );
        assert!(!account_dir(&i)
            .join("Doomhowl/Cael/SavedVariables")
            .exists());
        // The JSON a front end gets has no file contents in it.
        let json = serde_json::to_value(&p).unwrap();
        assert!(json["writes"][0].get("data").is_none());
        assert_eq!(json["writes"][2]["how"], "aceDb");
    }

    #[test]
    fn applying_writes_backups_and_the_target_reads_as_the_source() {
        let dir = tempfile::tempdir().unwrap();
        let i = elvui_tree(dir.path());
        let root = account_dir(&i);
        let inv = inventory(&i, "ACCOUNT1").unwrap();
        let before = std::fs::read(root.join("SavedVariables/ElvUI.lua")).unwrap();
        let p = plan(&inv, &ch("Aerin-Stormcall"), &ch("Cael-Doomhowl"), None).unwrap();

        // The game running refuses, and writes nothing.
        assert!(format!("{:#}", apply(&p, &|| true, false).unwrap_err()).contains("close it first"));
        assert_eq!(
            std::fs::read(root.join("SavedVariables/ElvUI.lua")).unwrap(),
            before
        );

        let done = apply(&p, &|| false, false).unwrap();
        assert_eq!(done.written.len(), 3);
        assert_eq!(names(&done.backups), ["SavedVariables/ElvUI.lua.ryna.bak"]);
        assert_eq!(
            std::fs::read(root.join("SavedVariables/ElvUI.lua.ryna.bak")).unwrap(),
            before
        );
        // The game's own .bak is untouched.
        assert_eq!(
            std::fs::read_to_string(root.join("SavedVariables/ElvUI.lua.bak")).unwrap(),
            "ElvDB = {}\n"
        );
        assert_eq!(
            std::fs::read(root.join("Doomhowl/Cael/SavedVariables/Details.lua")).unwrap(),
            std::fs::read(root.join("Stormcall/Aerin/SavedVariables/Details.lua")).unwrap()
        );
        let vars = lua::parse_bytes(&std::fs::read(root.join("SavedVariables/ElvUI.lua")).unwrap())
            .unwrap();
        let elv = &vars.iter().find(|(n, _)| n == "ElvDB").unwrap().1;
        assert_eq!(
            acedb::profile_of(elv, "Cael - Doomhowl").as_deref(),
            Some("Default")
        );
        let private = &vars.iter().find(|(n, _)| n == "ElvPrivateDB").unwrap().1;
        assert!(private
            .field("profiles")
            .unwrap()
            .get(&LuaKey::str("Cael - Doomhowl"))
            .is_some());

        // Now the two read the same, and a second copy has nothing to do.
        let inv = inventory(&i, "ACCOUNT1").unwrap();
        let rows = diff(&inv, &ch("Aerin-Stormcall"), &ch("Cael-Doomhowl"));
        let elv_row = rows.iter().find(|r| r.item == "ElvUI").unwrap();
        assert_eq!(elv_row.difference, Difference::Same);
        let again = plan(&inv, &ch("Aerin-Stormcall"), &ch("Cael-Doomhowl"), None).unwrap();
        assert!(again.writes.is_empty(), "{:?}", again.writes);
    }

    #[test]
    fn named_items_game_files_and_what_cant_be_copied() {
        let dir = tempfile::tempdir().unwrap();
        let i = elvui_tree(dir.path());
        let inv = inventory(&i, "ACCOUNT1").unwrap();
        let items: Vec<String> = ["details", "config-cache.wtf", "bindings-cache.wtf", "Gone"]
            .map(String::from)
            .to_vec();
        let p = plan(
            &inv,
            &ch("Aerin-Stormcall"),
            &ch("Dara-Newrealm"),
            Some(&items),
        )
        .unwrap();
        assert!(p.new_folder);
        let paths: Vec<&str> = p.writes.iter().map(|w| w.path.as_str()).collect();
        assert_eq!(
            paths,
            [
                "Newrealm/Dara/SavedVariables/Details.lua",
                "Newrealm/Dara/config-cache.wtf"
            ]
        );
        let skipped: Vec<&str> = p.skipped.iter().map(|s| s.item.as_str()).collect();
        assert_eq!(skipped, ["Gone", "bindings-cache.wtf"]);
        apply(&p, &|| false, false).unwrap();
        assert!(account_dir(&i)
            .join("Newrealm/Dara/config-cache.wtf")
            .is_file());

        assert!(plan(&inv, &ch("Aerin-Stormcall"), &ch("Aerin-Stormcall"), None).is_err());
        let bad = vec!["NoSuchAddon".to_string()];
        assert!(plan(
            &inv,
            &ch("Aerin-Stormcall"),
            &ch("Bryn-Doomhowl"),
            Some(&bad)
        )
        .unwrap_err()
        .to_string()
        .contains("NoSuchAddon"));
    }

    #[test]
    fn forced_while_running_writes() {
        let dir = tempfile::tempdir().unwrap();
        let i = elvui_tree(dir.path());
        let inv = inventory(&i, "ACCOUNT1").unwrap();
        let items = vec!["config-cache.wtf".to_string()];
        let p = plan(
            &inv,
            &ch("Aerin-Stormcall"),
            &ch("Bryn-Doomhowl"),
            Some(&items),
        )
        .unwrap();
        assert!(p.writes[0].replaces);
        let done = apply(&p, &|| true, true).unwrap();
        assert_eq!(
            names(&done.backups),
            ["Doomhowl/Bryn/config-cache.wtf.ryna.bak"]
        );
        assert_eq!(
            std::fs::read_to_string(account_dir(&i).join("Doomhowl/Bryn/config-cache.wtf"))
                .unwrap(),
            "SET uiScale \"0.7\"\n"
        );
    }

    #[test]
    fn the_diff_between_two_characters() {
        let dir = tempfile::tempdir().unwrap();
        let i = elvui_tree(dir.path());
        let inv = inventory(&i, "ACCOUNT1").unwrap();
        let rows = diff(&inv, &ch("Aerin-Stormcall"), &ch("Bryn-Doomhowl"));
        let got: Vec<(&str, Difference)> = rows
            .iter()
            .map(|r| (r.item.as_str(), r.difference))
            .collect();
        assert_eq!(
            got,
            [
                ("Details", Difference::OnlyA),
                ("ElvUI", Difference::Differ),
                ("config-cache.wtf", Difference::Differ),
                ("macros-cache.txt", Difference::OnlyA),
                ("AddOns.txt", Difference::OnlyA),
            ]
        );
        assert!(rows[2].game_file && !rows[0].game_file);
        // The same file, its entries in another order, is the same.
        let root = account_dir(&i);
        std::fs::create_dir_all(root.join("Doomhowl/Bryn/SavedVariables")).unwrap();
        std::fs::write(
            root.join("Stormcall/Aerin/SavedVariables/Details.lua"),
            "DetailsCharDB = { [\"a\"] = 1, [\"b\"] = 2 }\n",
        )
        .unwrap();
        std::fs::write(
            root.join("Doomhowl/Bryn/SavedVariables/Details.lua"),
            "DetailsCharDB = {\n\t[\"b\"] = 2,\n\t[\"a\"] = 1,\n}\n",
        )
        .unwrap();
        let rows = diff(&inv, &ch("Aerin-Stormcall"), &ch("Bryn-Doomhowl"));
        assert_eq!(rows[0].difference, Difference::Same);
    }
}
