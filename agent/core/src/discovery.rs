//! Finding the game and the addon's files.
//!
//! Blizzard puts each product in its own `_<name>_` folder under one install root
//! (`/Applications/World of Warcraft/_forever_`, `_retail_`, ...), each holding a `.flavor.info`
//! whose second line names the product (`wowf` for Forever). The addon's file is
//! `<product>/WTF/Account/<account>/SavedVariables/Ryna.lua`, one per game account.

use std::path::{Path, PathBuf};

/// The file the client writes the addon's SavedVariable to.
pub const SAVED_VARIABLES_FILE: &str = "Ryna.lua";

/// One product folder (`_forever_`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Install {
    pub path: PathBuf,
    /// What `.flavor.info` says (`wowf`), if it was readable.
    pub flavor: Option<String>,
}

/// One account's `Ryna.lua`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountFile {
    /// The product folder it's under.
    pub install: PathBuf,
    pub flavor: Option<String>,
    /// The folder name under `WTF/Account` (hashed before it leaves the computer).
    pub account: String,
    pub path: PathBuf,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Discovery {
    pub installs: Vec<Install>,
    pub accounts: Vec<AccountFile>,
}

/// Where the game installs by default on this OS.
pub fn default_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if cfg!(target_os = "macos") {
        roots.push(PathBuf::from("/Applications/World of Warcraft"));
        if let Some(home) = std::env::var_os("HOME") {
            roots.push(PathBuf::from(home).join("Applications/World of Warcraft"));
        }
    } else if cfg!(windows) {
        for (var, fallback) in [
            ("ProgramFiles(x86)", r"C:\Program Files (x86)"),
            ("ProgramFiles", r"C:\Program Files"),
        ] {
            let base = std::env::var_os(var).map_or_else(|| PathBuf::from(fallback), PathBuf::from);
            let root = base.join("World of Warcraft");
            if !roots.contains(&root) {
                roots.push(root);
            }
        }
    }
    roots
}

/// The product named by a `.flavor.info`: `Product Flavor!STRING:0` then the flavor.
pub fn parse_flavor_info(text: &str) -> Option<String> {
    let lines: Vec<&str> = text
        .trim_start_matches('\u{feff}')
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect();
    match lines.as_slice() {
        [header, flavor, ..] if header.starts_with("Product Flavor") => Some((*flavor).to_string()),
        [only] if !only.contains('!') => Some((*only).to_string()),
        _ => None,
    }
}

pub fn read_flavor(product: &Path) -> Option<String> {
    std::fs::read_to_string(product.join(".flavor.info"))
        .ok()
        .and_then(|t| parse_flavor_info(&t))
}

fn is_product_folder(path: &Path) -> bool {
    path.join(".flavor.info").is_file() || path.join("WTF").is_dir()
}

fn sorted_dirs(path: &Path) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(path)
        .map(|rd| {
            rd.filter_map(Result::ok)
                .map(|e| e.path())
                .filter(|p| p.is_dir())
                .collect()
        })
        .unwrap_or_default();
    dirs.sort();
    dirs
}

/// The product folders under each root. A root that is itself a product folder (the person
/// pointed `--install` at `_forever_`) counts as one.
pub fn find_installs(roots: &[PathBuf]) -> Vec<Install> {
    let mut out: Vec<Install> = Vec::new();
    let mut add = |path: PathBuf| {
        if !out.iter().any(|i| i.path == path) {
            let flavor = read_flavor(&path);
            out.push(Install { path, flavor });
        }
    };
    for root in roots {
        if is_product_folder(root) {
            add(root.clone());
            continue;
        }
        for dir in sorted_dirs(root) {
            let name = dir.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if name.len() > 2
                && name.starts_with('_')
                && name.ends_with('_')
                && is_product_folder(&dir)
            {
                add(dir);
            }
        }
    }
    out
}

/// Every account's `Ryna.lua` in an install, by account folder name.
pub fn find_accounts(install: &Install) -> Vec<AccountFile> {
    sorted_dirs(&install.path.join("WTF").join("Account"))
        .into_iter()
        .filter_map(|dir| {
            let path = dir.join("SavedVariables").join(SAVED_VARIABLES_FILE);
            if !path.is_file() {
                return None;
            }
            Some(AccountFile {
                install: install.path.clone(),
                flavor: install.flavor.clone(),
                account: dir.file_name()?.to_string_lossy().into_owned(),
                path,
            })
        })
        .collect()
}

/// Installs and account files under `override_path` if given, else the OS's default roots.
pub fn discover(override_path: Option<&Path>) -> Discovery {
    let roots = match override_path {
        Some(p) => vec![p.to_path_buf()],
        None => default_roots(),
    };
    let installs = find_installs(&roots);
    let accounts = installs.iter().flat_map(find_accounts).collect();
    Discovery { installs, accounts }
}

/// The account folder a `Ryna.lua` path sits in (`.../Account/<name>/SavedVariables/Ryna.lua`).
pub fn account_folder_of(path: &Path) -> Option<String> {
    let saved = path.parent()?;
    if saved.file_name()? != "SavedVariables" {
        return None;
    }
    Some(saved.parent()?.file_name()?.to_string_lossy().into_owned())
}

#[cfg(test)]
pub(crate) mod testing {
    use std::path::{Path, PathBuf};

    /// `<root>/<product>/` with a `.flavor.info` and the given accounts' `Ryna.lua` files.
    pub fn product(root: &Path, product: &str, flavor: &str, accounts: &[(&str, &str)]) -> PathBuf {
        let p = root.join(product);
        std::fs::create_dir_all(p.join("WTF/Account")).unwrap();
        std::fs::write(
            p.join(".flavor.info"),
            format!("Product Flavor!STRING:0\n{flavor}\n"),
        )
        .unwrap();
        for (account, body) in accounts {
            let sv = p.join("WTF/Account").join(account).join("SavedVariables");
            std::fs::create_dir_all(&sv).unwrap();
            std::fs::write(sv.join("Ryna.lua"), body).unwrap();
        }
        p
    }
}

#[cfg(test)]
mod tests {
    use super::testing::product;
    use super::*;

    #[test]
    fn flavor_info() {
        assert_eq!(
            parse_flavor_info("Product Flavor!STRING:0\nwowf\n"),
            Some("wowf".into())
        );
        assert_eq!(
            parse_flavor_info("Product Flavor!STRING:0\r\nwow_classic_era\r\n"),
            Some("wow_classic_era".into())
        );
        assert_eq!(
            parse_flavor_info("\u{feff}Product Flavor!STRING:0\n\n  wowf  \n"),
            Some("wowf".into())
        );
        assert_eq!(parse_flavor_info("wowf"), Some("wowf".into()));
        assert_eq!(parse_flavor_info("Product Flavor!STRING:0\n"), None);
        assert_eq!(parse_flavor_info(""), None);
    }

    #[test]
    fn finds_products_and_accounts() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("World of Warcraft");
        product(
            &root,
            "_forever_",
            "wowf",
            &[
                ("FAKEACCOUNT1", "RynaDB = {}"),
                ("FAKEACCOUNT2", "RynaDB = {}"),
            ],
        );
        product(&root, "_retail_", "wow", &[]);
        // An account without the addon's file, a stray file, a non-product folder.
        std::fs::create_dir_all(root.join("_forever_/WTF/Account/NOADDON/SavedVariables")).unwrap();
        std::fs::write(
            root.join("_forever_/WTF/Account/NOADDON/SavedVariables/Other.lua"),
            "",
        )
        .unwrap();
        std::fs::create_dir_all(root.join("Data")).unwrap();
        std::fs::create_dir_all(root.join("_empty_")).unwrap();

        let d = discover(Some(&root));
        let names: Vec<String> = d
            .installs
            .iter()
            .map(|i| i.path.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, vec!["_forever_", "_retail_"]);
        assert_eq!(d.installs[0].flavor.as_deref(), Some("wowf"));
        assert_eq!(d.installs[1].flavor.as_deref(), Some("wow"));
        let accounts: Vec<&str> = d.accounts.iter().map(|a| a.account.as_str()).collect();
        assert_eq!(accounts, vec!["FAKEACCOUNT1", "FAKEACCOUNT2"]);
        assert_eq!(d.accounts[0].flavor.as_deref(), Some("wowf"));
        assert!(d.accounts[0]
            .path
            .ends_with("_forever_/WTF/Account/FAKEACCOUNT1/SavedVariables/Ryna.lua"));
        assert_eq!(
            account_folder_of(&d.accounts[0].path).as_deref(),
            Some("FAKEACCOUNT1")
        );
    }

    #[test]
    fn override_can_name_the_product_folder() {
        let dir = tempfile::tempdir().unwrap();
        let p = product(dir.path(), "_forever_", "wowf", &[("FAKEACCOUNT1", "")]);
        let d = discover(Some(&p));
        assert_eq!(d.installs.len(), 1);
        assert_eq!(d.installs[0].path, p);
        assert_eq!(d.accounts.len(), 1);
    }

    #[test]
    fn a_product_without_flavor_info_still_counts_if_it_has_wtf() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("_beta_/WTF")).unwrap();
        let d = discover(Some(dir.path()));
        assert_eq!(
            d.installs,
            vec![Install {
                path: dir.path().join("_beta_"),
                flavor: None
            }]
        );
    }

    #[test]
    fn missing_root_finds_nothing() {
        let d = discover(Some(Path::new("/definitely/not/here")));
        assert_eq!(d, Discovery::default());
        assert_eq!(account_folder_of(Path::new("/x/Ryna.lua")), None);
    }

    #[test]
    fn default_roots_per_os() {
        let roots = default_roots();
        if cfg!(target_os = "macos") {
            assert_eq!(roots[0], PathBuf::from("/Applications/World of Warcraft"));
        } else if cfg!(windows) {
            assert!(roots.iter().any(|r| r.ends_with("World of Warcraft")));
        } else {
            assert!(roots.is_empty());
        }
    }
}
