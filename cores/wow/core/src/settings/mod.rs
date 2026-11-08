//! The Settings mod (docs/ryna-agent-cores-mods.md 7.2; `mods/wow/settings`, `kind: "agent"`):
//! addon settings backed up to ryna.io, encrypted, and copied between characters. All of it runs
//! here, in the agent, with the game closed; nothing is in the game and nothing goes to chat.
//!
//! - [`inventory`]: which addon owns which SavedVariables file (from each addon's TOC), the realm
//!   and character folders, and the game's own per-character files.
//! - [`acedb`]: the AceDB layout most big addons use (one account-wide file, each character under
//!   `"Name - Realm"`), and duplicating one character's entries for another.
//! - [`copy`]: copying settings from one character to another (files as they are, AceDB tables
//!   edited), the diff between two characters, and the dry run.
//! - [`backup`]: the zip, its encryption under the data key the API mints, the upload in parts,
//!   and the restore (docs/wow.md section 15 in the API repository).
//!
//! The rule everything that writes keeps: the game must be closed ([`refuse_if_running`]). WoW
//! reads SavedVariables at login and writes them back at logout, so a file written under a
//! logged-in character is lost. Every file replaced first gets a copy beside it,
//! `<file>.ryna.bak` ([`BAK_SUFFIX`]), one kept, the last; the game's own `<file>.bak`, which it
//! writes at every save, is left alone.

pub mod acedb;
pub mod backup;
pub mod copy;
pub mod inventory;

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};

/// The copy kept of each file a copy or restore replaced (`ElvUI.lua.ryna.bak`).
pub const BAK_SUFFIX: &str = ".ryna.bak";

/// What a write says when the game is running and it wasn't forced.
pub const GAME_RUNNING: &str =
    "the game is running: close it first (it rewrites these files when you log out, so a change made now would be lost)";

/// Refuse to write while the game runs, unless forced (the command line's `--force` only).
pub fn refuse_if_running(game_running: bool, force: bool) -> Result<()> {
    if game_running && !force {
        bail!(GAME_RUNNING);
    }
    Ok(())
}

/// `path` with [`BAK_SUFFIX`] on its name.
pub fn bak_path(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(BAK_SUFFIX);
    path.with_file_name(name)
}

/// Put `bytes` at `path`: its folder made if it isn't there, the file it replaces (if any) kept as
/// `<path>.ryna.bak` (replacing an older one), then the new bytes through a temporary file and a
/// rename. Returns the backup's path when one was written.
pub fn replace_file(path: &Path, bytes: &[u8]) -> Result<Option<PathBuf>> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).with_context(|| format!("making {}", dir.display()))?;
    }
    let bak = if path.is_file() {
        let bak = bak_path(path);
        std::fs::copy(path, &bak).with_context(|| format!("keeping {}", bak.display()))?;
        Some(bak)
    } else {
        None
    };
    crate::lua_write::write_atomic(path, bytes)
        .with_context(|| format!("writing {}", path.display()))?;
    Ok(bak)
}

#[cfg(test)]
pub(crate) mod testing {
    use std::path::{Path, PathBuf};

    use crate::discovery::Install;

    /// A fake `_forever_` under `root`: an `Interface/AddOns` with the given addons' TOCs
    /// (`(folder, toc text)`), and an account folder `ACCOUNT1`. Returns the install.
    pub fn install(root: &Path, addons: &[(&str, &str)]) -> Install {
        let p = root.join("_forever_");
        std::fs::create_dir_all(p.join("WTF/Account/ACCOUNT1/SavedVariables")).unwrap();
        std::fs::write(p.join(".flavor.info"), "Product Flavor!STRING:0\nwowf\n").unwrap();
        for (folder, toc) in addons {
            let d = p.join("Interface/AddOns").join(folder);
            std::fs::create_dir_all(&d).unwrap();
            std::fs::write(d.join(format!("{folder}.toc")), toc).unwrap();
        }
        Install {
            path: p,
            flavor: Some("wowf".into()),
        }
    }

    pub fn account_dir(i: &Install) -> PathBuf {
        i.path.join("WTF/Account/ACCOUNT1")
    }

    /// Write `text` at `<account>/<rel>`, folders made.
    pub fn put(i: &Install, rel: &str, text: &str) -> PathBuf {
        let p = account_dir(i).join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, text).unwrap();
        p
    }

    /// An ElvUI-shaped tree: ElvUI (AceDB, account-wide `ElvDB` and `ElvPrivateDB`, plus a
    /// per-character `ElvCharacterDB`), Details (per-character only), a plain account-wide
    /// addon, and three characters on two realms with the game's files.
    pub fn elvui_tree(root: &Path) -> Install {
        let i = install(
            root,
            &[
                (
                    "ElvUI",
                    "## Title: |cff1784d1ElvUI|r\n## SavedVariables: ElvDB, ElvPrivateDB\n## SavedVariablesPerCharacter: ElvCharacterDB\n",
                ),
                ("Details", "## SavedVariablesPerCharacter: DetailsCharDB\n"),
                ("Plain", "## SavedVariables: PlainDB\n"),
                ("Ryna", "## SavedVariables: RynaDB\n"),
                ("NoSettings", "## Title: Nothing saved\n"),
            ],
        );
        put(
            &i,
            "SavedVariables/ElvUI.lua",
            r#"
ElvDB = {
	["profileKeys"] = {
		["Aerin - Stormcall"] = "Default",
		["Bryn - Doomhowl"] = "Bryn - Doomhowl",
	},
	["profiles"] = {
		["Default"] = { ["general"] = { ["fontSize"] = 12 } },
		["Bryn - Doomhowl"] = { ["general"] = { ["fontSize"] = 14 } },
	},
	["char"] = {
		["Aerin - Stormcall"] = { ["installed"] = true },
	},
	["namespaces"] = {
		["LibDualSpec-1.0"] = {
			["char"] = { ["Aerin - Stormcall"] = { ["enabled"] = true } },
		},
	},
	["global"] = { ["v"] = 1 },
}
ElvPrivateDB = {
	["profileKeys"] = {
		["Aerin - Stormcall"] = "Aerin - Stormcall",
	},
	["profiles"] = {
		["Aerin - Stormcall"] = { ["bags"] = { ["enable"] = false } },
	},
}
"#,
        );
        put(
            &i,
            "SavedVariables/Plain.lua",
            "PlainDB = { [\"x\"] = 1 }\n",
        );
        put(&i, "SavedVariables/Ryna.lua", "RynaDB = {}\n");
        put(&i, "SavedVariables/Gone.lua", "GoneDB = {}\n");
        put(&i, "SavedVariables/ElvUI.lua.bak", "ElvDB = {}\n");
        put(
            &i,
            "Stormcall/Aerin/SavedVariables/ElvUI.lua",
            "ElvCharacterDB = { [\"aerin\"] = true }\n",
        );
        put(
            &i,
            "Stormcall/Aerin/SavedVariables/Details.lua",
            "DetailsCharDB = { [\"window\"] = 1 }\n",
        );
        put(
            &i,
            "Stormcall/Aerin/config-cache.wtf",
            "SET uiScale \"0.7\"\n",
        );
        put(&i, "Stormcall/Aerin/AddOns.txt", "ElvUI: enabled\n");
        put(&i, "Stormcall/Aerin/macros-cache.txt", "MACRO 1 \"x\"\n");
        put(&i, "Doomhowl/Bryn/config-cache.wtf", "SET uiScale \"1\"\n");
        put(&i, "Doomhowl/Bryn/SavedVariables/Ryna.lua", "RynaDB = {}\n");
        std::fs::create_dir_all(account_dir(&i).join("Doomhowl/Cael")).unwrap();
        i
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replacing_keeps_one_backup_of_what_was_there() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("a/b/ElvUI.lua");
        assert_eq!(replace_file(&p, b"one").unwrap(), None);
        assert_eq!(std::fs::read(&p).unwrap(), b"one");
        let bak = replace_file(&p, b"two").unwrap().unwrap();
        assert_eq!(bak, dir.path().join("a/b/ElvUI.lua.ryna.bak"));
        assert_eq!(std::fs::read(&bak).unwrap(), b"one");
        replace_file(&p, b"three").unwrap();
        assert_eq!(std::fs::read(&bak).unwrap(), b"two");
        assert_eq!(std::fs::read(&p).unwrap(), b"three");
    }

    #[test]
    fn a_running_game_refuses_unless_forced() {
        assert!(refuse_if_running(false, false).is_ok());
        assert!(refuse_if_running(true, true).is_ok());
        let e = refuse_if_running(true, false).unwrap_err().to_string();
        assert!(e.contains("close it first"), "{e}");
    }
}
