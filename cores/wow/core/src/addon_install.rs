//! Installing the addon: copy an `addon/Ryna` folder into each install's `Interface/AddOns/Ryna`.
//!
//! The release puts the addon beside the binary (`<dir>/addon/Ryna` or `<dir>/Ryna`); `--from`
//! names another. The copy is made next to the old one and swapped in, so a failed copy leaves
//! the installed addon as it was. Symbolic links in the source are skipped.
//!
//! `installed_addon_version` reads the installed copy's `## Version:` from its `Ryna.toc`, for the
//! app's install row and the heartbeat; `bundled_addon_version` reads the same line from the copy
//! a front end would install (the app's resources, the CLI's `--from`), and `addon_state` says
//! how the two compare (`compare_versions`: dotted integers).
//!
//! A WoW mod is its own addon folder beside `Ryna` (`Ryna_Chronicle`): `install_named`,
//! `remove_named` and `installed_folder_version` do the same for any folder name; `install` is
//! `install_named` with `Ryna`, unchanged.

use std::cmp::Ordering;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};

use crate::discovery::Install;

pub const ADDON_NAME: &str = "Ryna";

/// `<install>/Interface/AddOns/Ryna`, whether or not it's there.
pub fn addon_dir(install: &Install) -> PathBuf {
    install
        .path
        .join("Interface")
        .join("AddOns")
        .join(ADDON_NAME)
}

/// Whether the addon's folder is in this install.
pub fn has_addon(install: &Install) -> bool {
    addon_dir(install).is_dir()
}

/// The `## Version:` line of a `.toc`, trimmed; None when there's none or it's empty.
pub fn parse_toc_version(toc: &str) -> Option<String> {
    toc.trim_start_matches('\u{feff}').lines().find_map(|line| {
        let rest = line.trim().strip_prefix("##")?;
        let (key, value) = rest.split_once(':')?;
        if !key.trim().eq_ignore_ascii_case("version") {
            return None;
        }
        let value = value.trim();
        (!value.is_empty()).then(|| value.to_string())
    })
}

/// The `## Version:` of the `Ryna.toc` in the addon folder `dir`; None when it can't be read or
/// names no version.
fn folder_version(dir: &Path) -> Option<String> {
    folder_version_named(dir, ADDON_NAME)
}

/// The `## Version:` of `<name>.toc` in the addon folder `dir`.
pub fn folder_version_named(dir: &Path, name: &str) -> Option<String> {
    let bytes = std::fs::read(dir.join(format!("{name}.toc"))).ok()?;
    parse_toc_version(&String::from_utf8_lossy(&bytes))
}

/// `<install>/Interface/AddOns/<name>`, whether or not it's there.
pub fn addon_dir_named(install: &Install, name: &str) -> PathBuf {
    install.path.join("Interface").join("AddOns").join(name)
}

/// The installed version of the addon folder `name` (its `<name>.toc`), when it's there and says.
pub fn installed_folder_version(install: &Install, name: &str) -> Option<String> {
    folder_version_named(&addon_dir_named(install, name), name)
}

/// The installed addon's version, from `Interface/AddOns/Ryna/Ryna.toc`; None when the addon
/// isn't installed, the file can't be read, or it names no version.
pub fn installed_addon_version(install: &Install) -> Option<String> {
    folder_version(&addon_dir(install))
}

/// The version of the addon `from` names (resolved as `resolve_source` does: the folder itself,
/// `Ryna/` or `addon/Ryna/` inside it); None when there's no addon there or its `.toc` names no
/// version.
pub fn bundled_addon_version(from: &Path) -> Option<String> {
    folder_version(&resolve_source(from).ok()?)
}

/// Two versions as dotted integers: `0.10.0` is after `0.9.1`, and `0.1` equals `0.1.0`. A part
/// that isn't a number counts its leading digits (`2-beta` as 2, `x` as 0).
pub fn compare_versions(a: &str, b: &str) -> Ordering {
    fn parts(v: &str) -> Vec<u64> {
        let mut parts: Vec<u64> = v
            .trim()
            .trim_start_matches(['v', 'V'])
            .split('.')
            .map(|p| {
                let digits: String = p.trim().chars().take_while(char::is_ascii_digit).collect();
                digits.parse().unwrap_or(0)
            })
            .collect();
        while parts.last() == Some(&0) {
            parts.pop();
        }
        parts
    }
    parts(a).cmp(&parts(b))
}

/// How an install's addon compares with the one a front end would install.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum AddonState {
    /// `Interface/AddOns/Ryna` isn't there.
    NotInstalled,
    /// The same version as the bundled one.
    Current,
    /// Older than the bundled one: an update is available.
    Older,
    /// Newer than the bundled one (installed from elsewhere).
    Newer,
    /// Installed, but one side names no version (or nothing is bundled), so they can't be
    /// compared.
    Unknown,
}

/// `installed` (`None` with `has_addon` false: not there) against `bundled`.
pub fn addon_state(has_addon: bool, installed: Option<&str>, bundled: Option<&str>) -> AddonState {
    if !has_addon {
        return AddonState::NotInstalled;
    }
    match (installed, bundled) {
        (Some(i), Some(b)) => match compare_versions(i, b) {
            Ordering::Equal => AddonState::Current,
            Ordering::Less => AddonState::Older,
            Ordering::Greater => AddonState::Newer,
        },
        _ => AddonState::Unknown,
    }
}

fn has_toc(dir: &Path) -> bool {
    std::fs::read_dir(dir)
        .map(|rd| {
            rd.filter_map(Result::ok).any(|e| {
                e.path()
                    .extension()
                    .is_some_and(|x| x.eq_ignore_ascii_case("toc"))
            })
        })
        .unwrap_or(false)
}

/// The addon folder `from` names: `from` itself when it holds a `.toc`, else `from/Ryna`, else
/// `from/addon/Ryna`.
pub fn resolve_source(from: &Path) -> Result<PathBuf> {
    for candidate in [
        from.to_path_buf(),
        from.join(ADDON_NAME),
        from.join("addon").join(ADDON_NAME),
    ] {
        if has_toc(&candidate) {
            return Ok(candidate);
        }
    }
    bail!(
        "no addon in {}: expected a folder holding Ryna.toc (or Ryna/ or addon/Ryna/ inside it)",
        from.display()
    )
}

/// The addon beside the running binary, if the release layout put it there.
pub fn beside_binary() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    resolve_source(exe.parent()?).ok()
}

fn copy_dir(src: &Path, dst: &Path) -> Result<()> {
    std::fs::create_dir_all(dst).with_context(|| format!("creating {}", dst.display()))?;
    for entry in std::fs::read_dir(src).with_context(|| format!("reading {}", src.display()))? {
        let entry = entry?;
        let kind = entry.file_type()?;
        let to = dst.join(entry.file_name());
        if kind.is_dir() {
            copy_dir(&entry.path(), &to)?;
        } else if kind.is_file() {
            std::fs::copy(entry.path(), &to)
                .with_context(|| format!("copying to {}", to.display()))?;
        }
    }
    Ok(())
}

/// Copy `source` (the `Ryna` folder) into `install`'s `Interface/AddOns/Ryna`, replacing what's
/// there. Returns the installed folder.
pub fn install(source: &Path, install: &Install) -> Result<PathBuf> {
    install_named(source, install, ADDON_NAME)
}

/// Copy `source` into `install`'s `Interface/AddOns/<name>`, replacing what's there, the same way
/// (staged beside it and swapped in). Returns the installed folder.
pub fn install_named(source: &Path, install: &Install, name: &str) -> Result<PathBuf> {
    let addons = install.path.join("Interface").join("AddOns");
    std::fs::create_dir_all(&addons).with_context(|| format!("creating {}", addons.display()))?;
    let target = addons.join(name);
    let staging = addons.join(format!(".{name}.installing"));
    let old = addons.join(format!(".{name}.old"));
    for leftover in [&staging, &old] {
        if leftover.exists() {
            std::fs::remove_dir_all(leftover)
                .with_context(|| format!("removing {}", leftover.display()))?;
        }
    }
    if let Err(e) = copy_dir(source, &staging) {
        let _ = std::fs::remove_dir_all(&staging);
        return Err(e);
    }
    if target.exists() {
        std::fs::rename(&target, &old)
            .with_context(|| format!("moving the old addon aside in {}", addons.display()))?;
    }
    if let Err(e) = std::fs::rename(&staging, &target) {
        if old.exists() {
            let _ = std::fs::rename(&old, &target);
        }
        return Err(e).with_context(|| format!("putting the addon in {}", target.display()));
    }
    if old.exists() {
        // The core's Data/ folder is the agent's, not the release's (prices_data): carry it over,
        // so a reinstall doesn't lose the pooled prices until the next sync. The stubs the release
        // ships under Data/ go; the agent's files take their place.
        if name == ADDON_NAME {
            let kept = old.join(crate::prices_data::DIR);
            if kept.is_dir() {
                let into = target.join(crate::prices_data::DIR);
                let _ = std::fs::remove_dir_all(&into);
                if let Err(e) = std::fs::rename(&kept, &into) {
                    // Lost until the next sync rewrites it; the stubs are gone with the old copy.
                    eprintln!("couldn't keep {}: {e}", into.display());
                }
            }
        }
        std::fs::remove_dir_all(&old).with_context(|| format!("removing {}", old.display()))?;
    }
    Ok(target)
}

/// Remove `install`'s `Interface/AddOns/<name>` (a mod's folder; never the core's own). Returns
/// whether there was one to remove.
pub fn remove_named(install: &Install, name: &str) -> Result<bool> {
    if name == ADDON_NAME || name.is_empty() || name.contains(['/', '\\']) || name.starts_with('.')
    {
        bail!("not a mod's addon folder: {name:?}");
    }
    let target = addon_dir_named(install, name);
    if !target.is_dir() {
        return Ok(false);
    }
    std::fs::remove_dir_all(&target).with_context(|| format!("removing {}", target.display()))?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn addon(dir: &Path) -> PathBuf {
        let a = dir.join("release/addon/Ryna");
        std::fs::create_dir_all(a.join("Libs")).unwrap();
        std::fs::write(a.join("Ryna.toc"), "## Interface: 11600\nCore.lua\n").unwrap();
        std::fs::write(a.join("Core.lua"), "-- core\n").unwrap();
        std::fs::write(a.join("Libs/Lib.lua"), "-- lib\n").unwrap();
        a
    }

    #[test]
    fn resolves_the_release_layout_and_the_folder_itself() {
        let dir = tempfile::tempdir().unwrap();
        let a = addon(dir.path());
        assert_eq!(resolve_source(&dir.path().join("release")).unwrap(), a);
        assert_eq!(
            resolve_source(&dir.path().join("release/addon")).unwrap(),
            a
        );
        assert_eq!(resolve_source(&a).unwrap(), a);
        assert!(resolve_source(dir.path()).is_err());
    }

    #[test]
    fn reads_the_installed_version() {
        assert_eq!(
            parse_toc_version("## Interface: 16001\n## Title: Ryna\n## Version: 0.1.0\n")
                .as_deref(),
            Some("0.1.0")
        );
        assert_eq!(
            parse_toc_version("\u{feff}##version:  1.2.3 \r\n").as_deref(),
            Some("1.2.3")
        );
        assert_eq!(parse_toc_version("## Version:\n## Title: x"), None);
        assert_eq!(parse_toc_version("Core.lua"), None);

        let dir = tempfile::tempdir().unwrap();
        let inst = Install {
            path: dir.path().join("_forever_"),
            flavor: Some("wowf".into()),
        };
        assert!(!has_addon(&inst));
        assert_eq!(installed_addon_version(&inst), None);
        let a = addon(dir.path());
        std::fs::write(
            a.join("Ryna.toc"),
            "## Interface: 16001\n## Version: 0.1.0\n",
        )
        .unwrap();
        install(&a, &inst).unwrap();
        assert!(has_addon(&inst));
        assert_eq!(installed_addon_version(&inst).as_deref(), Some("0.1.0"));
    }

    #[test]
    fn compares_versions_as_dotted_integers() {
        use std::cmp::Ordering::*;
        assert_eq!(compare_versions("0.1.0", "0.1.0"), Equal);
        assert_eq!(compare_versions("0.1", "0.1.0"), Equal);
        assert_eq!(compare_versions("v1.2.0", "1.2"), Equal);
        assert_eq!(compare_versions("0.1.0", "0.2.0"), Less);
        assert_eq!(
            compare_versions("0.10.0", "0.9.1"),
            Greater,
            "not as strings"
        );
        assert_eq!(compare_versions("1.0.0", "0.99.99"), Greater);
        assert_eq!(compare_versions("0.2-beta", "0.2"), Equal);
        assert_eq!(compare_versions("0.1.1", "0.1"), Greater);
    }

    #[test]
    fn says_how_the_installed_addon_compares() {
        use AddonState::*;
        assert_eq!(addon_state(false, None, Some("0.2.0")), NotInstalled);
        assert_eq!(addon_state(false, None, None), NotInstalled);
        assert_eq!(addon_state(true, Some("0.2.0"), Some("0.2.0")), Current);
        assert_eq!(addon_state(true, Some("0.1.0"), Some("0.2.0")), Older);
        assert_eq!(addon_state(true, Some("0.3.0"), Some("0.2.0")), Newer);
        assert_eq!(addon_state(true, None, Some("0.2.0")), Unknown);
        assert_eq!(addon_state(true, Some("0.2.0"), None), Unknown);
        assert_eq!(serde_json::to_value(NotInstalled).unwrap(), "notInstalled");
    }

    #[test]
    fn reads_the_bundled_version() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(bundled_addon_version(dir.path()), None, "no addon there");
        let a = addon(dir.path());
        assert_eq!(
            bundled_addon_version(&dir.path().join("release")),
            None,
            "a toc with no version"
        );
        std::fs::write(
            a.join("Ryna.toc"),
            "## Interface: 16001\n## Version: 0.2.0\nCore.lua\n",
        )
        .unwrap();
        for from in ["release", "release/addon", "release/addon/Ryna"] {
            assert_eq!(
                bundled_addon_version(&dir.path().join(from)).as_deref(),
                Some("0.2.0"),
                "{from}"
            );
        }
    }

    #[test]
    fn a_mod_folder_installs_beside_the_core_and_comes_out_again() {
        let dir = tempfile::tempdir().unwrap();
        let a = addon(dir.path());
        let m = dir.path().join("release/addon/Ryna_Chronicle");
        std::fs::create_dir_all(m.join("Locales")).unwrap();
        std::fs::write(m.join("Ryna_Chronicle.toc"), "## Version: 0.3.0\n").unwrap();
        std::fs::write(m.join("Locales/enUS.lua"), "-- strings\n").unwrap();
        let inst = Install {
            path: dir.path().join("_forever_"),
            flavor: Some("wowf".into()),
        };
        install(&a, &inst).unwrap();
        assert_eq!(installed_folder_version(&inst, "Ryna_Chronicle"), None);
        let to = install_named(&m, &inst, "Ryna_Chronicle").unwrap();
        assert_eq!(to, addon_dir_named(&inst, "Ryna_Chronicle"));
        assert!(to.join("Locales/enUS.lua").is_file());
        assert_eq!(
            installed_folder_version(&inst, "Ryna_Chronicle").as_deref(),
            Some("0.3.0")
        );
        assert!(remove_named(&inst, "Ryna_Chronicle").unwrap());
        assert!(!to.exists());
        assert!(has_addon(&inst), "the core's folder is left alone");
        assert!(
            !remove_named(&inst, "Ryna_Chronicle").unwrap(),
            "nothing left to remove"
        );
        for refused in ["Ryna", "", "../Ryna", ".Ryna.old"] {
            assert!(remove_named(&inst, refused).is_err(), "{refused}");
        }
        assert!(has_addon(&inst));
    }

    #[test]
    fn installs_and_replaces() {
        let dir = tempfile::tempdir().unwrap();
        let a = addon(dir.path());
        let product = dir.path().join("_forever_");
        std::fs::create_dir_all(product.join("Interface/AddOns/Ryna")).unwrap();
        std::fs::write(product.join("Interface/AddOns/Ryna/Stale.lua"), "old").unwrap();
        // The agent's price tables (prices_data) live under the core's Data/: kept across a reinstall.
        std::fs::create_dir_all(product.join("Interface/AddOns/Ryna/Data")).unwrap();
        std::fs::write(
            product.join("Interface/AddOns/Ryna/Data/vendors.lua"),
            "RynaData_vendors = { version = 1 }",
        )
        .unwrap();
        std::fs::create_dir_all(a.join("Data")).unwrap();
        std::fs::write(a.join("Data/vendors.lua"), "-- stub\n").unwrap();
        std::fs::create_dir_all(product.join("Interface/AddOns/Other")).unwrap();
        let inst = Install {
            path: product.clone(),
            flavor: Some("wowf".into()),
        };

        let target = install(&a, &inst).unwrap();
        assert_eq!(target, product.join("Interface/AddOns/Ryna"));
        assert_eq!(
            std::fs::read_to_string(target.join("Libs/Lib.lua")).unwrap(),
            "-- lib\n"
        );
        assert!(target.join("Ryna.toc").is_file());
        assert!(
            !target.join("Stale.lua").exists(),
            "the old copy is replaced, not merged"
        );
        assert_eq!(
            std::fs::read_to_string(target.join("Data/vendors.lua")).unwrap(),
            "RynaData_vendors = { version = 1 }",
            "the agent's Data/ tables survive a reinstall, over the release's stubs"
        );
        assert!(
            product.join("Interface/AddOns/Other").is_dir(),
            "other addons are left alone"
        );
        let names: Vec<String> = std::fs::read_dir(product.join("Interface/AddOns"))
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names.len(), 2, "{names:?}");

        // A fresh install with no Interface folder yet.
        let fresh = Install {
            path: dir.path().join("_forever_ptr_"),
            flavor: None,
        };
        assert!(install(&a, &fresh).unwrap().join("Core.lua").is_file());
    }
}
