//! Installing the addon: copy an `addon/Ryna` folder into each install's `Interface/AddOns/Ryna`.
//!
//! The release puts the addon beside the binary (`<dir>/addon/Ryna` or `<dir>/Ryna`); `--from`
//! names another. The copy is made next to the old one and swapped in, so a failed copy leaves
//! the installed addon as it was. Symbolic links in the source are skipped.

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};

use crate::discovery::Install;

pub const ADDON_NAME: &str = "Ryna";

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
    let addons = install.path.join("Interface").join("AddOns");
    std::fs::create_dir_all(&addons).with_context(|| format!("creating {}", addons.display()))?;
    let target = addons.join(ADDON_NAME);
    let staging = addons.join(format!(".{ADDON_NAME}.installing"));
    let old = addons.join(format!(".{ADDON_NAME}.old"));
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
        std::fs::remove_dir_all(&old).with_context(|| format!("removing {}", old.display()))?;
    }
    Ok(target)
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
    fn installs_and_replaces() {
        let dir = tempfile::tempdir().unwrap();
        let a = addon(dir.path());
        let product = dir.path().join("_forever_");
        std::fs::create_dir_all(product.join("Interface/AddOns/Ryna")).unwrap();
        std::fs::write(product.join("Interface/AddOns/Ryna/Stale.lua"), "old").unwrap();
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
