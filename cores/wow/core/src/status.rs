//! What a front end shows, read fresh from the keychain, the disk and the agent's state: the
//! installs (with the addon version in each, and how it compares with the bundled one), the accounts (with their last save and push), the
//! sign-in and the last run. `ryna status` prints it; the app's window shows it.

use std::path::Path;

use serde::Serialize;

use crate::addon_install::{self, AddonState};
use crate::auth::{identity_from_jwt, KeyringStore, TokenStore};
use crate::config::Config;
use crate::discovery::{self, Discovery};
use crate::fix::Fix;
use crate::snapshot::{account_key, default_label};
use crate::state::{self, State};
use crate::util::mtime;

/// The flavor World of Warcraft: Forever's `.flavor.info` names.
pub const FOREVER_FLAVOR: &str = "wowf";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallStatus {
    pub path: String,
    pub flavor: Option<String>,
    /// Forever (`wowf`): the addon is installed only into these unless a folder was chosen.
    pub forever: bool,
    /// `Interface/AddOns/Ryna` is there.
    pub has_addon: bool,
    /// Its `Ryna.toc`'s `## Version:`, when it's there and says.
    pub addon_version: Option<String>,
    /// How that compares with `Status::addon_bundled`.
    pub addon: AddonState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountStatus {
    /// The account's `Ryna.lua`.
    pub path: String,
    /// The product folder it's under.
    pub install: String,
    /// What the API knows it by (the first 16 hex digits of the folder name's sha256).
    pub key: String,
    pub label: String,
    /// When the game last wrote the file (unix seconds): the file's own time, else the last run's.
    pub saved_at: Option<i64>,
    pub pushed_at: Option<i64>,
    pub characters: Option<usize>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LastRunStatus {
    pub at: i64,
    pub ok: bool,
    pub error: Option<String>,
    pub fix: Option<Fix>,
    /// The words to show for the failure (`Fix::message`), when it failed.
    pub advice: Option<String>,
}

/// The combat log's side: what the tailer last saw and what went up.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CombatStatus {
    /// A log was written in the last few minutes.
    pub active: bool,
    pub advanced: Option<bool>,
    /// The newest log's file name.
    pub file: Option<String>,
    /// Unix seconds.
    pub last_event_at: Option<i64>,
    /// Fights queued to upload, plus encounters still open.
    pub pending: usize,
    /// Fights uploaded in the last 12 hours.
    pub fights_tonight: usize,
    pub last_fight: Option<crate::combat::tail::UploadedFight>,
    pub last_error: Option<String>,
}

impl CombatStatus {
    pub fn read(state_dir: &Path, now: i64) -> CombatStatus {
        let st = crate::combat::tail::CombatState::load(state_dir);
        let hb = crate::combat::tail::heartbeat(state_dir, now);
        CombatStatus {
            active: hb.active,
            advanced: hb.advanced,
            file: hb.file,
            last_event_at: hb.last_event_at,
            pending: hb.pending,
            fights_tonight: st.fights_tonight(now),
            last_fight: st.last_fight().cloned(),
            last_error: st.last_error.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub agent_version: String,
    pub api: String,
    pub issuer: String,
    pub client_id_issued: bool,
    /// A refresh token is kept on this computer.
    pub signed_in: bool,
    /// Who as: from the kept access token's claims, for display only.
    pub signed_in_as: Option<String>,
    /// The keychain couldn't be read (so `signed_in` is unknown and false).
    pub keychain_error: Option<String>,
    pub installs: Vec<InstallStatus>,
    /// The version of the addon this front end would install (the app's bundled copy, the CLI's
    /// `--from`), when it has one and its `.toc` says.
    pub addon_bundled: Option<String>,
    pub accounts: Vec<AccountStatus>,
    /// The last run's error, if it failed.
    pub last_error: Option<String>,
    pub last_run: Option<LastRunStatus>,
    pub combat: CombatStatus,
    pub state_dir: Option<String>,
}

fn display(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

/// The status with the real keychain, discovery (`install` in place of the OS defaults) and
/// state folder. `addon_from` is the addon this front end would install (resolved as
/// `addon_install::resolve_source` does), for `addon_bundled` and each install's `addon`.
pub fn status(config: &Config, install: Option<&Path>, addon_from: Option<&Path>) -> Status {
    let store = KeyringStore::for_issuer(&config.issuer);
    let found = discovery::discover(install);
    let bundled = addon_from.and_then(addon_install::bundled_addon_version);
    status_with(
        config,
        &store,
        &found,
        state::state_dir().as_deref(),
        bundled,
    )
}

/// Each install found, with its addon and how that compares with `addon_bundled`.
pub fn install_statuses(found: &Discovery, addon_bundled: Option<&str>) -> Vec<InstallStatus> {
    found
        .installs
        .iter()
        .map(|i| {
            let has_addon = addon_install::has_addon(i);
            let addon_version = addon_install::installed_addon_version(i);
            InstallStatus {
                path: display(&i.path),
                flavor: i.flavor.clone(),
                forever: i.flavor.as_deref() == Some(FOREVER_FLAVOR),
                has_addon,
                addon: addon_install::addon_state(
                    has_addon,
                    addon_version.as_deref(),
                    addon_bundled,
                ),
                addon_version,
            }
        })
        .collect()
}

/// `status` over any token store, discovery, state folder and bundled addon version (tests;
/// another front end).
pub fn status_with(
    config: &Config,
    store: &dyn TokenStore,
    found: &Discovery,
    state_dir: Option<&Path>,
    addon_bundled: Option<String>,
) -> Status {
    let (signed_in, keychain_error) = match store.refresh_token() {
        Ok(t) => (t.is_some(), None),
        Err(e) => (false, Some(e)),
    };
    let signed_in_as = if signed_in {
        store
            .access_token()
            .and_then(|a| identity_from_jwt(&a.token))
    } else {
        None
    };
    let st = state_dir.map(State::load).unwrap_or_default();
    let installs = install_statuses(found, addon_bundled.as_deref());
    let accounts = found
        .accounts
        .iter()
        .map(|a| {
            let path = display(&a.path);
            let s = st.accounts.get(&path);
            AccountStatus {
                install: display(&a.install),
                key: s.map_or_else(|| account_key(&a.account), |s| s.key.clone()),
                label: s.map_or_else(|| default_label(&a.account), |s| s.label.clone()),
                saved_at: mtime(&a.path)
                    .ok()
                    .map(|m| m.secs)
                    .or_else(|| s.and_then(|s| s.saved_at)),
                pushed_at: s.and_then(|s| s.pushed_at),
                characters: s.and_then(|s| s.characters),
                last_error: s.and_then(|s| s.last_error.clone()),
                path,
            }
        })
        .collect();
    let last_run = st.last_run.map(|r| {
        let advice = match (&r.error, r.ok) {
            (Some(e), false) => Some(r.fix.unwrap_or(Fix::Other).message(e)),
            _ => None,
        };
        LastRunStatus {
            at: r.at,
            ok: r.ok,
            error: r.error,
            fix: r.fix,
            advice,
        }
    });
    Status {
        agent_version: crate::AGENT_VERSION.to_string(),
        api: config.api.clone(),
        issuer: config.issuer.clone(),
        client_id_issued: config.client_id_issued(),
        signed_in,
        signed_in_as,
        keychain_error,
        installs,
        addon_bundled,
        accounts,
        last_error: last_run
            .as_ref()
            .filter(|r| !r.ok)
            .and_then(|r| r.error.clone()),
        last_run,
        combat: state_dir
            .map(|d| CombatStatus::read(d, crate::util::now_secs()))
            .unwrap_or_default(),
        state_dir: state_dir.map(display),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::testing::MemoryStore;
    use crate::config::test_config;
    use crate::discovery::{discover, testing::product};
    use crate::state::{AccountState, LastRun};
    use serde_json::json;

    #[test]
    fn installs_accounts_sign_in_and_the_last_run() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("World of Warcraft");
        let forever = product(
            &root,
            "_forever_",
            "wowf",
            &[("FAKEACCOUNT1", "RynaDB = {}")],
        );
        product(&root, "_retail_", "wow", &[]);
        let addon = forever.join("Interface/AddOns/Ryna");
        std::fs::create_dir_all(&addon).unwrap();
        std::fs::write(
            addon.join("Ryna.toc"),
            "## Title: Ryna\n## Version: 0.1.0\n",
        )
        .unwrap();
        let found = discover(Some(&root));

        let state_dir = dir.path().join("state");
        let mut st = State::default();
        st.accounts.insert(
            display(&found.accounts[0].path),
            AccountState {
                key: "k1".into(),
                label: "Main".into(),
                pushed_at: Some(20),
                characters: Some(2),
                ..Default::default()
            },
        );
        st.last_run = Some(LastRun {
            at: 30,
            ok: false,
            error: Some("the API is unreachable or failing after 3 tries: x".into()),
            fix: Some(Fix::Network),
        });
        st.save(&state_dir).unwrap();

        // {"sub":"x","email":"aerin@example.test"}
        let store = MemoryStore::with_access(
            "h.eyJzdWIiOiJ4IiwiZW1haWwiOiJhZXJpbkBleGFtcGxlLnRlc3QifQ.s",
            i64::MAX,
        );
        let s = status_with(
            &test_config(),
            &store,
            &found,
            Some(&state_dir),
            Some("0.2.0".into()),
        );
        assert!(s.signed_in && s.client_id_issued);
        assert_eq!(s.signed_in_as.as_deref(), Some("aerin"));
        assert_eq!(s.installs.len(), 2);
        assert!(s.installs[0].forever && s.installs[0].has_addon);
        assert_eq!(s.installs[0].addon_version.as_deref(), Some("0.1.0"));
        assert_eq!(s.installs[0].addon, AddonState::Older);
        assert_eq!(s.addon_bundled.as_deref(), Some("0.2.0"));
        assert!(!s.installs[1].forever && !s.installs[1].has_addon);
        assert_eq!(s.installs[1].addon_version, None);
        assert_eq!(s.installs[1].addon, AddonState::NotInstalled);
        let a = &s.accounts[0];
        assert_eq!((a.key.as_str(), a.label.as_str()), ("k1", "Main"));
        assert_eq!((a.pushed_at, a.characters), (Some(20), Some(2)));
        assert!(a.saved_at.is_some());
        assert_eq!(
            s.last_error.as_deref(),
            Some("the API is unreachable or failing after 3 tries: x")
        );
        let v = serde_json::to_value(&s).unwrap();
        assert_eq!(v["installs"][0]["addonVersion"], json!("0.1.0"));
        assert_eq!(v["installs"][0]["addon"], json!("older"));
        assert_eq!(v["addonBundled"], json!("0.2.0"));
        assert_eq!(v["lastRun"]["fix"], json!("network"));
        assert_eq!(v["lastRun"]["advice"], json!("Couldn't reach the API"));
        assert_eq!(v["signedInAs"], json!("aerin"));

        // Signed out, nothing found, no state.
        let s = status_with(
            &test_config(),
            &MemoryStore::default(),
            &Discovery::default(),
            None,
            None,
        );
        assert!(!s.signed_in);
        assert_eq!(s.signed_in_as, None);
        assert!(s.installs.is_empty() && s.accounts.is_empty());
        assert_eq!(s.addon_bundled, None);
        assert_eq!((s.last_run, s.last_error, s.state_dir), (None, None, None));
    }
}
