//! What settings an account has: each addon's SavedVariables files (which addon owns which file
//! comes from its own TOC, `## SavedVariables:` and `## SavedVariablesPerCharacter:`), the realm and
//! character folders, and the game's own per-character files.
//!
//! The game writes an addon's account-wide variables to `WTF/Account/<account>/SavedVariables/
//! <Addon>.lua` and its per-character ones to `WTF/Account/<account>/<Realm>/<Character>/
//! SavedVariables/<Addon>.lua`, the file named for the addon's folder. A file whose addon isn't
//! installed any more is listed too (`installed: false`). `Ryna.lua` (the core's own) is never part
//! of any of it, nor the game's `.bak` files or this mod's `.ryna.bak` ones.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anyhow::{bail, Result};
use serde::Serialize;

use super::acedb;
use crate::discovery::{Install, SAVED_VARIABLES_FILE};
use crate::lua;
use crate::snapshot::account_key;

/// The game's own per-character files, offered by name.
pub const GAME_FILES: &[&str] = &[
    "config-cache.wtf",
    "bindings-cache.wtf",
    "macros-cache.txt",
    "chat-cache.txt",
    "layout-local.txt",
    "AddOns.txt",
];

/// The core's own addon, whose settings are the core's data and never part of this mod.
pub const CORE_ADDON: &str = "Ryna";

/// A character, by its folder names: `Name` under `Realm`. Written `Name-Realm`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub struct Character {
    pub name: String,
    pub realm: String,
}

impl Character {
    /// `Name-Realm` (a name has no `-`, a realm may).
    pub fn parse(s: &str) -> Result<Character> {
        match s.trim().split_once('-') {
            Some((name, realm)) if !name.trim().is_empty() && !realm.trim().is_empty() => {
                Ok(Character {
                    name: name.trim().to_string(),
                    realm: realm.trim().to_string(),
                })
            }
            _ => bail!("{s:?} isn't a character: write it Name-Realm"),
        }
    }

    /// AceDB's key for it: `Name - Realm`.
    pub fn acedb_key(&self) -> String {
        format!("{} - {}", self.name, self.realm)
    }

    /// From AceDB's `Name - Realm`.
    pub fn from_acedb_key(key: &str) -> Option<Character> {
        let (name, realm) = key.split_once(" - ")?;
        (!name.is_empty() && !realm.is_empty()).then(|| Character {
            name: name.to_string(),
            realm: realm.to_string(),
        })
    }

    /// Its folder, relative to the account's: `Realm/Name`.
    pub fn rel_dir(&self) -> String {
        format!("{}/{}", self.realm, self.name)
    }
}

impl std::fmt::Display for Character {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}-{}", self.name, self.realm)
    }
}

/// Where an addon keeps its settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Scope {
    /// One file for the whole account.
    Account,
    /// A file per character.
    Character,
    /// Both.
    Both,
}

/// One file on disk.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsFile {
    /// Relative to the account's folder, `/`-separated: `SavedVariables/ElvUI.lua`,
    /// `Stormcall/Aerin/SavedVariables/ElvUI.lua`, `Stormcall/Aerin/config-cache.wtf`.
    pub path: String,
    /// The character it belongs to (none: account-wide).
    pub character: Option<Character>,
    pub bytes: u64,
    /// Unix seconds.
    pub modified: Option<i64>,
}

/// One addon's settings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AddonSettings {
    /// Its folder name, which its files are named for.
    pub addon: String,
    /// Its TOC's `## Title:` with the colour codes taken out.
    pub title: Option<String>,
    /// Its folder is in `Interface/AddOns` (false: only its settings are left).
    pub installed: bool,
    pub account_variables: Vec<String>,
    pub character_variables: Vec<String>,
    pub scope: Scope,
    pub files: Vec<SettingsFile>,
    /// The characters it holds settings for: those with a file of their own, and those an AceDB
    /// table in its account file names.
    pub characters: Vec<Character>,
    /// Its account file holds AceDB tables, so a copy edits them for the target character.
    pub acedb: bool,
    pub bytes: u64,
    /// The newest file's time (unix seconds).
    pub modified: Option<i64>,
}

/// One account's settings in one install.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Inventory {
    pub install: String,
    pub flavor: Option<String>,
    /// The account's folder name (shown on this computer; never sent).
    pub account: String,
    /// What the API knows the account by (the snapshot's `account.key`).
    pub account_key: String,
    /// The account's folder.
    #[serde(skip)]
    pub root: PathBuf,
    /// Every character: those with a folder, and those only an AceDB table names.
    pub characters: Vec<Character>,
    /// The characters with a folder of their own here.
    pub character_folders: Vec<Character>,
    pub addons: Vec<AddonSettings>,
    /// The game's own per-character files ([`GAME_FILES`]).
    pub game_files: Vec<SettingsFile>,
}

impl Inventory {
    pub fn addon(&self, name: &str) -> Option<&AddonSettings> {
        self.addons
            .iter()
            .find(|a| a.addon.eq_ignore_ascii_case(name))
    }

    /// The absolute path of an account-relative `path`.
    pub fn abs(&self, path: &str) -> PathBuf {
        path.split('/').fold(self.root.clone(), |p, c| p.join(c))
    }

    /// A character this account has, from `Name-Realm`, matched without regard to case.
    pub fn character(&self, s: &str) -> Result<Character> {
        let c = Character::parse(s)?;
        Ok(self
            .characters
            .iter()
            .find(|k| {
                k.name.eq_ignore_ascii_case(&c.name) && k.realm.eq_ignore_ascii_case(&c.realm)
            })
            .cloned()
            .unwrap_or(c))
    }
}

/// What an addon's TOC files declare.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TocInfo {
    pub title: Option<String>,
    pub account: Vec<String>,
    pub character: Vec<String>,
}

/// One TOC's text: its title and its two SavedVariables lines (names split on commas). The
/// directive names are read without regard to case.
pub fn parse_toc(text: &str) -> TocInfo {
    let mut info = TocInfo::default();
    for line in text.trim_start_matches('\u{feff}').lines() {
        let Some(rest) = line.trim().strip_prefix("##") else {
            continue;
        };
        let Some((key, value)) = rest.split_once(':') else {
            continue;
        };
        let names = || {
            value
                .split(',')
                .map(str::trim)
                .filter(|n| !n.is_empty())
                .map(str::to_string)
                .collect::<Vec<_>>()
        };
        match key.trim().to_ascii_lowercase().as_str() {
            "title" if info.title.is_none() => {
                let t = strip_colours(value.trim());
                info.title = (!t.is_empty()).then_some(t);
            }
            "savedvariables" => info.account.extend(names()),
            "savedvariablespercharacter" => info.character.extend(names()),
            _ => {}
        }
    }
    info
}

/// `|cffrrggbbText|r` as `Text`.
fn strip_colours(s: &str) -> String {
    let mut out = String::new();
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '|' {
            match chars.peek() {
                Some('c') | Some('C') => {
                    chars.next();
                    for _ in 0..8 {
                        chars.next();
                    }
                    continue;
                }
                Some('r') | Some('R') => {
                    chars.next();
                    continue;
                }
                _ => {}
            }
        }
        out.push(c);
    }
    out.trim().to_string()
}

fn dirs(p: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(p)
        .map(|rd| {
            rd.filter_map(Result::ok)
                .map(|e| e.path())
                .filter(|p| p.is_dir())
                .collect()
        })
        .unwrap_or_default();
    v.sort();
    v
}

fn name_of(p: &Path) -> String {
    p.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Every installed addon's TOC declarations, by folder name. A folder's TOCs are `<Folder>.toc` and
/// the suffixed ones (`<Folder>_Vanilla.toc`, `<Folder>-Classic.toc`, …); what they declare is
/// taken together, since which one a client reads differs by client.
pub fn installed_addons(install: &Install) -> BTreeMap<String, TocInfo> {
    let mut out = BTreeMap::new();
    for dir in dirs(&install.path.join("Interface").join("AddOns")) {
        let folder = name_of(&dir);
        let mut info = TocInfo::default();
        let mut found = false;
        let mut tocs: Vec<PathBuf> = std::fs::read_dir(&dir)
            .map(|rd| rd.filter_map(Result::ok).map(|e| e.path()).collect())
            .unwrap_or_default();
        tocs.sort();
        for toc in tocs {
            let file = name_of(&toc);
            let Some(stem) = file
                .strip_suffix(".toc")
                .or_else(|| file.strip_suffix(".TOC"))
            else {
                continue;
            };
            let ours = stem.eq_ignore_ascii_case(&folder)
                || stem.len() > folder.len()
                    && stem[..folder.len()].eq_ignore_ascii_case(&folder)
                    && matches!(stem.as_bytes()[folder.len()], b'_' | b'-');
            if !ours {
                continue;
            }
            let Ok(bytes) = std::fs::read(&toc) else {
                continue;
            };
            let t = parse_toc(&String::from_utf8_lossy(&bytes));
            found = true;
            if info.title.is_none() {
                info.title = t.title;
            }
            for n in t.account {
                if !info.account.contains(&n) {
                    info.account.push(n);
                }
            }
            for n in t.character {
                if !info.character.contains(&n) {
                    info.character.push(n);
                }
            }
        }
        if found {
            out.insert(folder, info);
        }
    }
    out
}

/// The account folders in an install (`WTF/Account/*`), by name.
pub fn accounts(install: &Install) -> Vec<String> {
    dirs(&install.path.join("WTF").join("Account"))
        .iter()
        .map(|d| name_of(d))
        .filter(|n| n != "SavedVariables")
        .collect()
}

fn file_info(root: &Path, rel: &str, character: Option<&Character>) -> Option<SettingsFile> {
    let p = rel.split('/').fold(root.to_path_buf(), |p, c| p.join(c));
    let meta = std::fs::metadata(&p).ok().filter(|m| m.is_file())?;
    let modified = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64);
    Some(SettingsFile {
        path: rel.to_string(),
        character: character.cloned(),
        bytes: meta.len(),
        modified,
    })
}

/// The `.lua` files in a SavedVariables folder, by addon name (the file's stem), less `Ryna.lua`.
fn lua_files(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir)
        .map(|rd| {
            rd.filter_map(Result::ok)
                .filter(|e| e.path().is_file())
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .filter(|n| n.ends_with(".lua") && n != SAVED_VARIABLES_FILE)
                .map(|n| n.trim_end_matches(".lua").to_string())
                .collect()
        })
        .unwrap_or_default();
    v.sort();
    v
}

/// The characters an account-wide file's AceDB tables name, if any of its variables is one.
fn acedb_characters(path: &Path) -> Option<Vec<Character>> {
    let bytes = std::fs::read(path).ok()?;
    // Reading a big file costs; an AceDB file always has this key.
    if !bytes.windows(13).any(|w| w == b"\"profileKeys\"") {
        return None;
    }
    let vars = lua::parse_bytes(&bytes).ok()?;
    let mut keys = BTreeSet::new();
    let mut any = false;
    for (_, v) in &vars {
        if acedb::is_acedb(v) {
            any = true;
            keys.extend(acedb::characters(v));
        }
    }
    any.then(|| {
        keys.iter()
            .filter_map(|k| Character::from_acedb_key(k))
            .collect()
    })
}

/// Everything the Settings mod sees for `account` in `install`.
pub fn inventory(install: &Install, account: &str) -> Result<Inventory> {
    let root = install.path.join("WTF").join("Account").join(account);
    if !root.is_dir() {
        bail!(
            "there's no account folder {account} in {}",
            install.path.display()
        );
    }
    let tocs = installed_addons(install);
    // The characters with folders: every folder in every realm folder.
    let mut character_folders = Vec::new();
    for realm in dirs(&root) {
        let realm_name = name_of(&realm);
        if realm_name == "SavedVariables" {
            continue;
        }
        for ch in dirs(&realm) {
            character_folders.push(Character {
                name: name_of(&ch),
                realm: realm_name.clone(),
            });
        }
    }
    let mut addons: BTreeMap<String, AddonSettings> = BTreeMap::new();
    let entry = |addons: &mut BTreeMap<String, AddonSettings>, name: &str| -> String {
        let key = addons
            .keys()
            .find(|k| k.eq_ignore_ascii_case(name))
            .cloned()
            .unwrap_or_else(|| name.to_string());
        addons.entry(key.clone()).or_insert_with(|| {
            let toc = tocs
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case(name))
                .map(|(_, v)| v.clone());
            AddonSettings {
                addon: key.clone(),
                title: toc.as_ref().and_then(|t| t.title.clone()),
                installed: toc.is_some(),
                account_variables: toc.as_ref().map(|t| t.account.clone()).unwrap_or_default(),
                character_variables: toc.map(|t| t.character).unwrap_or_default(),
                scope: Scope::Account,
                files: Vec::new(),
                characters: Vec::new(),
                acedb: false,
                bytes: 0,
                modified: None,
            }
        });
        key
    };
    // Installed addons that declare settings, whether or not they've written any yet.
    for (folder, toc) in &tocs {
        if folder.eq_ignore_ascii_case(CORE_ADDON)
            || (toc.account.is_empty() && toc.character.is_empty())
        {
            continue;
        }
        entry(&mut addons, folder);
    }
    for name in lua_files(&root.join("SavedVariables")) {
        if name.eq_ignore_ascii_case(CORE_ADDON) {
            continue;
        }
        let key = entry(&mut addons, &name);
        let rel = format!("SavedVariables/{name}.lua");
        if let Some(f) = file_info(&root, &rel, None) {
            let a = addons.get_mut(&key).expect("just made");
            if let Some(chars) =
                acedb_characters(&root.join("SavedVariables").join(format!("{name}.lua")))
            {
                a.acedb = true;
                a.characters.extend(chars);
            }
            a.files.push(f);
        }
    }
    let mut game_files = Vec::new();
    for ch in &character_folders {
        let dir = ch.rel_dir();
        for name in lua_files(&root.join(&ch.realm).join(&ch.name).join("SavedVariables")) {
            if name.eq_ignore_ascii_case(CORE_ADDON) {
                continue;
            }
            let key = entry(&mut addons, &name);
            if let Some(f) = file_info(&root, &format!("{dir}/SavedVariables/{name}.lua"), Some(ch))
            {
                let a = addons.get_mut(&key).expect("just made");
                a.characters.push(ch.clone());
                a.files.push(f);
            }
        }
        for name in GAME_FILES {
            game_files.extend(file_info(&root, &format!("{dir}/{name}"), Some(ch)));
        }
    }
    let mut every: BTreeSet<Character> = character_folders.iter().cloned().collect();
    let addons: Vec<AddonSettings> = addons
        .into_values()
        .map(|mut a| {
            let has_account =
                a.files.iter().any(|f| f.character.is_none()) || !a.account_variables.is_empty();
            let has_character =
                a.files.iter().any(|f| f.character.is_some()) || !a.character_variables.is_empty();
            a.scope = match (has_account, has_character) {
                (true, true) => Scope::Both,
                (false, true) => Scope::Character,
                _ => Scope::Account,
            };
            a.characters.sort();
            a.characters.dedup();
            every.extend(a.characters.iter().cloned());
            a.bytes = a.files.iter().map(|f| f.bytes).sum();
            a.modified = a.files.iter().filter_map(|f| f.modified).max();
            a
        })
        .collect();
    Ok(Inventory {
        install: install.path.to_string_lossy().into_owned(),
        flavor: install.flavor.clone(),
        account: account.to_string(),
        account_key: account_key(account),
        root,
        characters: every.into_iter().collect(),
        character_folders,
        addons,
        game_files,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::testing::{elvui_tree, install, put};

    fn ch(name: &str, realm: &str) -> Character {
        Character {
            name: name.into(),
            realm: realm.into(),
        }
    }

    #[test]
    fn characters_are_name_dash_realm() {
        assert_eq!(
            Character::parse("Aerin-Stormcall").unwrap(),
            ch("Aerin", "Stormcall")
        );
        assert_eq!(
            Character::parse(" Aerin-Argent Dawn-EU ").unwrap(),
            ch("Aerin", "Argent Dawn-EU")
        );
        assert!(Character::parse("Aerin").is_err());
        assert!(Character::parse("-Realm").is_err());
        let a = ch("Aerin", "Argent Dawn");
        assert_eq!(a.to_string(), "Aerin-Argent Dawn");
        assert_eq!(a.acedb_key(), "Aerin - Argent Dawn");
        assert_eq!(Character::from_acedb_key("Aerin - Argent Dawn"), Some(a));
        assert_eq!(Character::from_acedb_key("Default"), None);
    }

    #[test]
    fn toc_lines() {
        let t = parse_toc(
            "\u{feff}## Interface: 11507\r\n## Title: |cff1784d1Elv|r|cffffffffUI|r\r\n##SavedVariables: ElvDB, ElvPrivateDB ,\r\n## savedvariablespercharacter: ElvCharacterDB\r\n# ## SavedVariables: Commented\r\n",
        );
        assert_eq!(t.title.as_deref(), Some("ElvUI"));
        assert_eq!(t.account, ["ElvDB", "ElvPrivateDB"]);
        assert_eq!(t.character, ["ElvCharacterDB"]);
        assert_eq!(parse_toc(""), TocInfo::default());
    }

    #[test]
    fn suffixed_tocs_count_and_others_dont() {
        let dir = tempfile::tempdir().unwrap();
        let i = install(
            dir.path(),
            &[("Questie", "## SavedVariables: QuestieConfig\n")],
        );
        let q = i.path.join("Interface/AddOns/Questie");
        std::fs::write(
            q.join("Questie-Classic.toc"),
            "## SavedVariablesPerCharacter: QuestieChar\n",
        )
        .unwrap();
        std::fs::write(
            q.join("Questie_Vanilla.toc"),
            "## SavedVariables: QuestieConfig, QuestieExtra\n",
        )
        .unwrap();
        std::fs::write(q.join("Other.toc"), "## SavedVariables: NotOurs\n").unwrap();
        let found = installed_addons(&i);
        let t = &found["Questie"];
        assert_eq!(t.account, ["QuestieConfig", "QuestieExtra"]);
        assert_eq!(t.character, ["QuestieChar"]);
    }

    #[test]
    fn the_inventory_of_an_elvui_tree() {
        let dir = tempfile::tempdir().unwrap();
        let i = elvui_tree(dir.path());
        assert_eq!(accounts(&i), ["ACCOUNT1"]);
        let inv = inventory(&i, "ACCOUNT1").unwrap();
        assert_eq!(inv.account_key, account_key("ACCOUNT1"));
        let names: Vec<&str> = inv.addons.iter().map(|a| a.addon.as_str()).collect();
        // Ryna never; an addon that declares nothing isn't listed; a file left behind is.
        assert_eq!(names, ["Details", "ElvUI", "Gone", "Plain"]);

        let elv = inv.addon("elvui").unwrap();
        assert_eq!(elv.title.as_deref(), Some("ElvUI"));
        assert!(elv.installed && elv.acedb);
        assert_eq!(elv.scope, Scope::Both);
        assert_eq!(elv.account_variables, ["ElvDB", "ElvPrivateDB"]);
        assert_eq!(
            elv.files
                .iter()
                .map(|f| f.path.as_str())
                .collect::<Vec<_>>(),
            [
                "SavedVariables/ElvUI.lua",
                "Stormcall/Aerin/SavedVariables/ElvUI.lua"
            ]
        );
        assert_eq!(
            elv.characters,
            [ch("Aerin", "Stormcall"), ch("Bryn", "Doomhowl")]
        );
        assert_eq!(elv.bytes, elv.files.iter().map(|f| f.bytes).sum::<u64>());
        assert!(elv.modified.is_some());

        let details = inv.addon("Details").unwrap();
        assert_eq!(details.scope, Scope::Character);
        assert!(!details.acedb);
        assert_eq!(details.characters, [ch("Aerin", "Stormcall")]);

        let plain = inv.addon("Plain").unwrap();
        assert_eq!(plain.scope, Scope::Account);
        assert!(plain.characters.is_empty() && !plain.acedb);
        let gone = inv.addon("Gone").unwrap();
        assert!(!gone.installed && gone.title.is_none());

        assert_eq!(
            inv.character_folders,
            [
                ch("Bryn", "Doomhowl"),
                ch("Cael", "Doomhowl"),
                ch("Aerin", "Stormcall")
            ]
        );
        assert_eq!(inv.characters.len(), 3);
        let game: Vec<&str> = inv.game_files.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(
            game,
            [
                "Doomhowl/Bryn/config-cache.wtf",
                "Stormcall/Aerin/config-cache.wtf",
                "Stormcall/Aerin/macros-cache.txt",
                "Stormcall/Aerin/AddOns.txt"
            ]
        );
        // Nothing anywhere is Ryna.lua or a .bak.
        let every_path = inv
            .addons
            .iter()
            .flat_map(|a| a.files.iter())
            .chain(inv.game_files.iter())
            .map(|f| f.path.clone())
            .collect::<Vec<_>>();
        assert!(every_path
            .iter()
            .all(|p| !p.ends_with("Ryna.lua") && !p.ends_with(".bak")));
        assert!(inventory(&i, "NOPE").is_err());
    }

    #[test]
    fn an_acedb_only_character_is_listed() {
        let dir = tempfile::tempdir().unwrap();
        let i = install(
            dir.path(),
            &[("Bartender4", "## SavedVariables: Bartender4DB\n")],
        );
        put(
            &i,
            "SavedVariables/Bartender4.lua",
            "Bartender4DB = { [\"profileKeys\"] = { [\"Dara - Faraway\"] = \"Default\" } }\n",
        );
        let inv = inventory(&i, "ACCOUNT1").unwrap();
        assert_eq!(inv.characters, [ch("Dara", "Faraway")]);
        assert!(inv.character_folders.is_empty());
        assert!(inv.addon("Bartender4").unwrap().acedb);
    }
}
