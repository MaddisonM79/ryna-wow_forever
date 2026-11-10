//! The Settings mod in the window (`ryna_wow_core::settings`): the addon inventory, copying between
//! characters, and backups to ryna.io. Only while the mod is on; every write refuses while the game
//! is running (the window greys them out first); nothing is ever said in the game.

use std::sync::Arc;

use ryna_agent::auth::{Auth, KeyringStore};
use ryna_agent::http::HttpTransport;
use ryna_agent::identity::DeviceKey;
use ryna_agent::util::{hostname, now_secs, SystemClock};
use ryna_wow_core::discovery::Install;
use ryna_wow_core::push::Api;
use ryna_wow_core::settings::backup::{
    self, BackupDone, BackupSummary, Include, Progress, RestoreOptions, RestoreReport,
};
use ryna_wow_core::settings::copy::{self, Applied, CopyPlan, DiffRow};
use ryna_wow_core::settings::inventory::{self, Inventory};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::agent::{self, Agent};
use ryna_wow_core::game_process;

/// The mod's id (`mods/wow/settings/mod.json`).
pub const MOD_ID: &str = "settings";
/// A backup's or restore's `Progress`, while it runs.
pub const PROGRESS_EVENT: &str = "settings-progress";

/// Whether the game is running, for the window: running, closed, or can't tell (`unknown` says
/// why, and counts as running: every write stays greyed out).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameState {
    pub running: bool,
    pub unknown: Option<String>,
}

/// The game's state, with the install's own executables asked too where the OS can (Windows).
pub fn game_state(install: Option<&std::path::Path>) -> GameState {
    if install.is_some_and(game_process::exe_in_use) {
        return GameState {
            running: true,
            unknown: None,
        };
    }
    match game_process::game_state() {
        Ok(running) => GameState {
            running,
            unknown: None,
        },
        Err(why) => GameState {
            running: true,
            unknown: Some(why),
        },
    }
}

/// Every account's inventory in every install the mod works in.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsView {
    pub on: bool,
    pub inventories: Vec<Inventory>,
    pub game: GameState,
    /// Problems reading an account (each its own words), shown beside the rest.
    pub problems: Vec<String>,
}

fn require_on(app: &AppHandle) -> Result<(), String> {
    if agent::mod_on(app, MOD_ID) {
        Ok(())
    } else {
        Err("Turn the Settings mod on first (Mods).".into())
    }
}

fn install_at(app: &AppHandle, path: &str) -> Result<Install, String> {
    agent::settings_installs(app)
        .into_iter()
        .find(|i| i.path.to_string_lossy() == path)
        .ok_or_else(|| format!("{path} isn't an install Ryna works in."))
}

/// `account` as the window sent it, only when it is exactly one of the account folders the install
/// lists (`inventory::accounts`): the webview never names a folder of its own.
fn listed_account(i: &Install, account: &str) -> Result<(), String> {
    if inventory::accounts(i).iter().any(|a| a == account) {
        Ok(())
    } else {
        Err(format!(
            "{account:?} isn't an account folder in {}.",
            i.path.display()
        ))
    }
}

fn inventory_of(app: &AppHandle, install: &str, account: &str) -> Result<Inventory, String> {
    let i = install_at(app, install)?;
    listed_account(&i, account)?;
    inventory::inventory(&i, account).map_err(|e| format!("{e:#}"))
}

pub fn view(app: &AppHandle) -> SettingsView {
    let on = agent::mod_on(app, MOD_ID);
    let mut inventories = Vec::new();
    let mut problems = Vec::new();
    let installs = agent::settings_installs(app);
    if on {
        for i in &installs {
            for account in inventory::accounts(i) {
                match inventory::inventory(i, &account) {
                    Ok(inv) => inventories.push(inv),
                    Err(e) => problems.push(format!("{account}: {e:#}")),
                }
            }
        }
    }
    SettingsView {
        on,
        inventories,
        game: game_state(installs.first().map(|i| i.path.as_path())),
        problems,
    }
}

pub fn diff(
    app: &AppHandle,
    install: &str,
    account: &str,
    a: &str,
    b: &str,
) -> Result<Vec<DiffRow>, String> {
    require_on(app)?;
    let inv = inventory_of(app, install, account)?;
    let (a, b) = (
        inv.character(a).map_err(|e| e.to_string())?,
        inv.character(b).map_err(|e| e.to_string())?,
    );
    Ok(copy::diff(&inv, &a, &b))
}

/// What a copy did, or (a preview) would do.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CopyResult {
    pub plan: CopyPlan,
    /// None for a preview.
    pub applied: Option<Applied>,
    pub game_running: bool,
}

/// Plan a copy and, unless `preview`, write it. Planned again at the moment of writing, so what's
/// written is what's on disk then. Never forced: the window has no way past a running game.
pub fn copy_settings(
    app: &AppHandle,
    install: &str,
    account: &str,
    from: &str,
    to: &str,
    items: Vec<String>,
    preview: bool,
) -> Result<CopyResult, String> {
    require_on(app)?;
    let inv = inventory_of(app, install, account)?;
    let (from, to) = (
        inv.character(from).map_err(|e| e.to_string())?,
        inv.character(to).map_err(|e| e.to_string())?,
    );
    if items.is_empty() {
        return Err("Choose at least one addon to copy.".into());
    }
    let plan = copy::plan(&inv, &from, &to, Some(&items)).map_err(|e| format!("{e:#}"))?;
    let path = std::path::PathBuf::from(&inv.install);
    let running = || game_state(Some(&path)).running;
    let applied = if preview {
        None
    } else {
        Some(copy::apply(&plan, &running, false).map_err(|e| sentence(&format!("{e:#}")))?)
    };
    Ok(CopyResult {
        plan,
        applied,
        game_running: running(),
    })
}

/// The first letter up and a full stop, for an error the core words as a clause.
fn sentence(s: &str) -> String {
    let mut c = s.chars();
    let mut out: String = c
        .next()
        .map(|f| f.to_uppercase().collect())
        .unwrap_or_default();
    out.push_str(c.as_str());
    if !out.ends_with('.') {
        out.push('.');
    }
    out
}

/// Run `f` with the API as the signed-in person, signed by the device key.
fn with_api<T>(app: &AppHandle, f: impl FnOnce(&Api) -> anyhow::Result<T>) -> Result<T, String> {
    let agent = app.state::<Agent>();
    let config = agent.config.clone();
    let key: Arc<dyn DeviceKey> = agent.device_key()?;
    let http = HttpTransport::new()?;
    let store = KeyringStore::for_issuer(&config.issuer);
    let clock = SystemClock;
    let auth = Auth::new(&config, &http, &store, &clock);
    let api = Api::new(&config.api, &http, &auth, Some(key.as_ref()), &clock);
    f(&api).map_err(|e| sentence(&format!("{e:#}")))
}

fn emit(app: &AppHandle) -> impl FnMut(Progress) + '_ {
    move |p| {
        let _ = app.emit(PROGRESS_EVENT, p);
    }
}

pub fn backup_now(
    app: &AppHandle,
    install: &str,
    account: &str,
    addons: bool,
    game: bool,
) -> Result<BackupDone, String> {
    require_on(app)?;
    let include = match (addons, game) {
        (true, true) => Include::All,
        (true, false) => Include::Addons,
        (false, true) => Include::Game,
        (false, false) => return Err("Choose what to back up.".into()),
    };
    let inv = inventory_of(app, install, account)?;
    let machine = hostname();
    let mut progress = emit(app);
    with_api(app, |api| {
        backup::backup(api, &inv, include, &machine, now_secs(), &mut progress)
    })
}

pub fn backups(app: &AppHandle) -> Result<Vec<BackupSummary>, String> {
    require_on(app)?;
    with_api(app, backup::list)
}

/// Restore a backup into the account chosen in the window, and only if it's the account the
/// backup was made from: the window has no way to restore into another (the command line's
/// `--account … --force` does), nor past a running game.
pub fn restore(
    app: &AppHandle,
    install: &str,
    id: &str,
    account: &str,
    preview: bool,
) -> Result<RestoreReport, String> {
    require_on(app)?;
    let i = install_at(app, install)?;
    // `backup::account_for` matches it against the listed folders too; checked here first so the
    // window's string never reaches the core unless it is one of them.
    listed_account(&i, account)?;
    let mut progress = emit(app);
    let opts = RestoreOptions {
        account: Some(account),
        other_account: false,
        dry_run: preview,
        force: false,
    };
    let running = || game_state(Some(&i.path)).running;
    with_api(app, |api| {
        backup::restore(api, id, &i, opts, &running, &mut progress)
    })
}

pub fn delete(app: &AppHandle, id: &str) -> Result<(), String> {
    require_on(app)?;
    with_api(app, |api| backup::delete(api, id))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_listed_account_folder_is_taken_from_the_window() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("_forever_");
        std::fs::create_dir_all(p.join("WTF/Account/ACCOUNT1")).unwrap();
        std::fs::create_dir_all(p.join("WTF/Account/SavedVariables")).unwrap();
        std::fs::create_dir_all(dir.path().join("Elsewhere/Realm/Name")).unwrap();
        let i = Install {
            path: p,
            flavor: Some("wowf".into()),
        };
        assert!(listed_account(&i, "ACCOUNT1").is_ok());
        for bad in [
            "../../../Elsewhere",
            "ACCOUNT1/../ACCOUNT1",
            "account1",
            "SavedVariables",
            "",
            "NOPE",
        ] {
            assert!(listed_account(&i, bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn errors_read_as_sentences() {
        assert_eq!(
            sentence("the game is running: close it first"),
            "The game is running: close it first."
        );
        assert_eq!(sentence("Already one."), "Already one.");
        assert_eq!(sentence(""), ".");
    }
}
