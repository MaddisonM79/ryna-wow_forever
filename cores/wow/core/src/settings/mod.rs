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
use serde::Serialize;

/// The copy kept of each file a copy or restore replaced (`ElvUI.lua.ryna.bak`).
pub const BAK_SUFFIX: &str = ".ryna.bak";

/// What a write says when the game is running and it wasn't forced.
pub const GAME_RUNNING: &str =
    "the game is running, or Ryna can't tell that it isn't: close it first (it rewrites these files when you log out, so a change made now would be lost)";

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

/// A Windows device name (`CON`, `PRN`, `AUX`, `NUL`, `CONIN$`, `CONOUT$`, `CLOCK$`, `COM1`–`COM9`
/// and `LPT1`–`LPT9`, the superscript `¹²³` forms too), with or without an extension, in any case:
/// never a file there.
pub fn windows_device_name(part: &str) -> bool {
    let stem = part
        .split('.')
        .next()
        .unwrap_or("")
        .trim_end()
        .to_ascii_uppercase();
    matches!(
        stem.as_str(),
        "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$" | "CLOCK$"
    ) || stem
        .strip_prefix("COM")
        .or_else(|| stem.strip_prefix("LPT"))
        .is_some_and(|n| {
            matches!(
                n,
                "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
            )
        })
}

/// Whether `part` is one plain path component on every OS: not empty, `.` or `..`; no slash,
/// backslash, colon or NUL; no space at the start, no space or dot at the end (Windows drops
/// them); not a Windows device name.
pub fn plain_component(part: &str) -> bool {
    !part.is_empty()
        && part != "."
        && part != ".."
        && !part.contains(['/', '\\', ':', '\0'])
        && !part.starts_with(' ')
        && !part.ends_with([' ', '.'])
        && !windows_device_name(part)
}

/// What [`write_all`] did, by account-relative path.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Written {
    pub written: Vec<String>,
    /// Already these bytes: left alone, and their `.ryna.bak` with them.
    pub unchanged: Vec<String>,
    /// The `.ryna.bak` copies made.
    pub backups: Vec<String>,
}

struct Pending<'a> {
    rel: &'a str,
    path: PathBuf,
    data: &'a [u8],
    existed: bool,
}

/// Write `files` (account-relative path, bytes) under `root`, all or nothing as far as the disk
/// lets it:
///
/// - every path is plain components, and none of them (nor the file) is a link, else nothing is
///   written;
/// - a file that already holds the bytes is left alone (its `.ryna.bak` too);
/// - `game_running` is asked right before the first write, and refuses unless `force`;
/// - every replaced file is copied to `<file>.ryna.bak` first, then the files are written (each
///   through a temporary file and a rename);
/// - when a write fails, the files written before it are put back from their `.ryna.bak` (or
///   removed, when they were new), and the error says which.
pub fn write_all(
    root: &Path,
    files: &[(&str, &[u8])],
    game_running: &dyn Fn() -> bool,
    force: bool,
) -> Result<Written> {
    let mut done = Written::default();
    let mut todo: Vec<Pending> = Vec::new();
    for (rel, data) in files {
        let parts: Vec<&str> = rel.split('/').collect();
        if !parts.iter().all(|p| plain_component(p)) {
            bail!("{rel} isn't a path this mod writes: nothing was written");
        }
        let p = parts.iter().fold(root.to_path_buf(), |p, c| p.join(c));
        // No link anywhere on the way, the file included (a missing part ends the walk).
        let mut walk = root.to_path_buf();
        for c in &parts {
            walk.push(c);
            match std::fs::symlink_metadata(&walk) {
                Ok(m) if m.file_type().is_symlink() => bail!(
                    "{} is a link, and Ryna doesn't write through links: nothing was written",
                    walk.display()
                ),
                Ok(_) => {}
                Err(_) => break,
            }
        }
        // Nor the two files written beside it: the `.ryna.bak` copy and the temporary file.
        for beside in [bak_path(&p), crate::lua_write::tmp_path(&p)] {
            if std::fs::symlink_metadata(&beside).is_ok_and(|m| m.file_type().is_symlink()) {
                bail!(
                    "{} is a link, and Ryna doesn't write through links: nothing was written",
                    beside.display()
                );
            }
        }
        let existed = match std::fs::read(&p) {
            Ok(old) if old == *data => {
                done.unchanged.push(rel.to_string());
                continue;
            }
            Ok(_) => true,
            Err(e)
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::NotFound | std::io::ErrorKind::NotADirectory
                ) =>
            {
                false
            }
            Err(e) => bail!("reading {}: {e}: nothing was written", p.display()),
        };
        todo.push(Pending {
            rel,
            path: p,
            data,
            existed,
        });
    }
    if todo.is_empty() {
        return Ok(done);
    }
    refuse_if_running(game_running(), force)?;
    for t in todo.iter().filter(|t| t.existed) {
        let bak = bak_path(&t.path);
        std::fs::copy(&t.path, &bak)
            .with_context(|| format!("keeping {} (nothing else was changed)", bak.display()))?;
        done.backups.push(format!("{}{BAK_SUFFIX}", t.rel));
    }
    for (i, t) in todo.iter().enumerate() {
        let wrote = t
            .path
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|()| crate::lua_write::write_atomic(&t.path, t.data));
        if let Err(e) = wrote {
            let (back, stuck) = roll_back(&todo[..i]);
            let mut msg = format!("writing {} failed ({e})", t.rel);
            if back.is_empty() && stuck.is_empty() {
                msg.push_str(": nothing was changed");
            }
            if !back.is_empty() {
                msg.push_str(&format!("; put back as they were: {}", back.join(", ")));
            }
            if !stuck.is_empty() {
                msg.push_str(&format!(
                    "; couldn't be put back (each has its .ryna.bak beside it): {}",
                    stuck.join(", ")
                ));
            }
            bail!(msg);
        }
        done.written.push(t.rel.to_string());
    }
    Ok(done)
}

/// Undo `written`: each replaced file from its `.ryna.bak`, each new one removed. Returns what was
/// put back and what couldn't be.
fn roll_back(written: &[Pending]) -> (Vec<String>, Vec<String>) {
    let mut back = Vec::new();
    let mut stuck = Vec::new();
    for t in written {
        let ok = if t.existed {
            std::fs::read(bak_path(&t.path))
                .and_then(|old| crate::lua_write::write_atomic(&t.path, &old))
                .is_ok()
        } else {
            std::fs::remove_file(&t.path).is_ok()
        };
        if ok {
            back.push(t.rel.to_string());
        } else {
            stuck.push(t.rel.to_string());
        }
    }
    (back, stuck)
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

    fn no() -> bool {
        false
    }

    fn yes() -> bool {
        true
    }

    #[test]
    fn writing_keeps_one_backup_and_leaves_equal_files_alone() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let p = root.join("a/b/ElvUI.lua");
        let w = write_all(root, &[("a/b/ElvUI.lua", b"one")], &no, false).unwrap();
        assert_eq!(w.written, ["a/b/ElvUI.lua"]);
        assert!(w.backups.is_empty());
        let w = write_all(root, &[("a/b/ElvUI.lua", b"two")], &no, false).unwrap();
        assert_eq!(w.backups, ["a/b/ElvUI.lua.ryna.bak"]);
        assert_eq!(std::fs::read(bak_path(&p)).unwrap(), b"one");
        // The same bytes again: nothing written, and the one backup is still of "one".
        let w = write_all(root, &[("a/b/ElvUI.lua", b"two")], &no, false).unwrap();
        assert_eq!(w.unchanged, ["a/b/ElvUI.lua"]);
        assert!(w.written.is_empty() && w.backups.is_empty());
        assert_eq!(std::fs::read(bak_path(&p)).unwrap(), b"one");
        write_all(root, &[("a/b/ElvUI.lua", b"three")], &no, false).unwrap();
        assert_eq!(std::fs::read(bak_path(&p)).unwrap(), b"two");
        assert_eq!(std::fs::read(&p).unwrap(), b"three");
    }

    #[test]
    fn the_game_is_asked_right_before_the_first_write() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::write(root.join("x.lua"), "same").unwrap();
        // Nothing to write: not asked, nothing refused.
        let asked = std::cell::Cell::new(0);
        let check = || {
            asked.set(asked.get() + 1);
            true
        };
        assert!(write_all(root, &[("x.lua", b"same")], &check, false).is_ok());
        assert_eq!(asked.get(), 0);
        let e = write_all(root, &[("x.lua", b"new")], &check, false).unwrap_err();
        assert_eq!(asked.get(), 1);
        assert!(e.to_string().contains("close it first"));
        assert_eq!(std::fs::read(root.join("x.lua")).unwrap(), b"same");
        assert!(!bak_path(&root.join("x.lua")).exists());
        write_all(root, &[("x.lua", b"new")], &yes, true).unwrap();
        assert_eq!(std::fs::read(root.join("x.lua")).unwrap(), b"new");
    }

    #[test]
    fn a_failed_write_puts_back_what_went_before_it() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join("a")).unwrap();
        std::fs::write(root.join("a/old.lua"), "old").unwrap();
        // "b" is a file, so b/y.lua can't be made.
        std::fs::write(root.join("b"), "in the way").unwrap();
        let e = write_all(
            root,
            &[
                ("a/old.lua", b"new"),
                ("a/fresh.lua", b"fresh"),
                ("b/y.lua", b"y"),
            ],
            &no,
            false,
        )
        .unwrap_err()
        .to_string();
        assert!(e.contains("writing b/y.lua failed"), "{e}");
        assert!(
            e.contains("put back as they were: a/old.lua, a/fresh.lua"),
            "{e}"
        );
        assert_eq!(std::fs::read(root.join("a/old.lua")).unwrap(), b"old");
        assert!(!root.join("a/fresh.lua").exists());
    }

    #[test]
    fn odd_paths_and_links_write_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        for bad in [
            "../x.lua",
            "a//b.lua",
            "a/./b.lua",
            "NUL",
            "a/con.txt",
            "a/b.",
            "c:/x",
        ] {
            let e = write_all(root, &[("ok.lua", b"1"), (bad, b"2")], &no, false).unwrap_err();
            assert!(e.to_string().contains("nothing was written"), "{bad}: {e}");
            assert!(!root.join("ok.lua").exists(), "{bad}");
        }
        #[cfg(unix)]
        {
            let elsewhere = tempfile::tempdir().unwrap();
            std::os::unix::fs::symlink(elsewhere.path(), root.join("Realm")).unwrap();
            let e = write_all(root, &[("Realm/Name/config-cache.wtf", b"x")], &no, false)
                .unwrap_err()
                .to_string();
            assert!(e.contains("link"), "{e}");
            assert!(std::fs::read_dir(elsewhere.path())
                .unwrap()
                .next()
                .is_none());
            // A planted link where the .ryna.bak or the temporary file goes: nothing written.
            let target = elsewhere.path().join("caught");
            for beside in ["x.lua.ryna.bak", "x.lua.ryna-tmp"] {
                std::fs::write(root.join("x.lua"), "old").unwrap();
                std::os::unix::fs::symlink(&target, root.join(beside)).unwrap();
                let e = write_all(root, &[("x.lua", b"new")], &no, false)
                    .unwrap_err()
                    .to_string();
                assert!(e.contains("link"), "{beside}: {e}");
                assert!(!target.exists(), "{beside}");
                assert_eq!(std::fs::read(root.join("x.lua")).unwrap(), b"old");
                std::fs::remove_file(root.join(beside)).unwrap();
            }
        }
    }

    #[test]
    fn plain_components() {
        for ok in [
            "ElvUI.lua",
            "Argent Dawn",
            "Aerin",
            "config-cache.wtf",
            "COM0",
            "CONSOLE",
            "nul_x",
        ] {
            assert!(plain_component(ok), "{ok}");
        }
        for bad in [
            "",
            ".",
            "..",
            "a/b",
            "a\\b",
            "c:",
            " x",
            "x ",
            "x.",
            "CON",
            "nul.txt",
            "Com1",
            "LPT9.lua",
            "aux ",
            "COM¹",
            "lpt³.txt",
            "CONIN$",
            "conout$.lua",
            "CLOCK$",
        ] {
            assert!(!plain_component(bad), "{bad}");
        }
    }

    #[test]
    fn a_running_game_refuses_unless_forced() {
        assert!(refuse_if_running(false, false).is_ok());
        assert!(refuse_if_running(true, true).is_ok());
        let e = refuse_if_running(true, false).unwrap_err().to_string();
        assert!(e.contains("close it first"), "{e}");
    }
}
