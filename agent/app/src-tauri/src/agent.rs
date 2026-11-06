//! The app's side of the core: one watch loop at a time on its own thread, the sign-in, and the
//! status the window shows. Nothing here outlives the app: quitting cancels the loop, and there
//! is no service, scheduled task or launchd job behind it.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::{Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::Duration;

use ryna_wow_core::auth::{identity_from_jwt, Auth, DeviceCode, KeyringStore, TokenStore};
use ryna_wow_core::config::Config;
use ryna_wow_core::discovery::{self, Install};
use ryna_wow_core::http::HttpTransport;
use ryna_wow_core::runner::Failure;
use ryna_wow_core::state::{self, State};
use ryna_wow_core::util::{mtime, SystemClock};
use ryna_wow_core::watch::{self, CancelToken, WatchEvent, WatchOptions};
use ryna_wow_core::{addon_install, snapshot, AGENT_VERSION};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::settings::Settings;

/// How often the loop looks for new saves while the app is open.
pub const INTERVAL: Duration = Duration::from_secs(60);
/// Every `WatchEvent`, as the core serialises it (`{"event": "pushed", ...}`).
pub const AGENT_EVENT: &str = "agent-event";
/// `true` or `false`: whether a watch loop is running.
pub const WATCH_STATE: &str = "watch-state";
/// A `LoginFinished`, once the device grant ends either way.
pub const LOGIN_FINISHED: &str = "login-finished";
/// How long `start_login` waits for the sign-in provider to hand out a code.
const CODE_WAIT: Duration = Duration::from_secs(45);

/// The device client id: `RYNA_WOW_CLIENT_ID` when it is set where the app runs, else one baked
/// in at compile time (`RYNA_WOW_CLIENT_ID` in the build's environment), else the core's
/// placeholder, which `login` refuses. API and issuer come from the core (env overrides).
pub fn config() -> Config {
    let mut config = Config::resolve(None, None);
    let baked = option_env!("RYNA_WOW_CLIENT_ID")
        .map(str::trim)
        .filter(|id| !id.is_empty());
    if let (false, Some(id)) = (config.client_id_issued(), baked) {
        config.client_id = id.to_string();
    }
    config
}

/// What an account's row shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Phase {
    Waiting,
    Pushing,
    Pushed,
    Error,
}

/// The code to show the person, and where to check it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginCode {
    pub user_code: String,
    pub verification_uri: String,
    /// The link to open: the one with the code in it when the provider gives one.
    pub link: String,
    pub expires_in: i64,
}

impl From<&DeviceCode> for LoginCode {
    fn from(code: &DeviceCode) -> Self {
        LoginCode {
            user_code: code.user_code.clone(),
            verification_uri: code.verification_uri.clone(),
            link: code.link().to_string(),
            expires_in: code.expires_in,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginFinished {
    pub ok: bool,
    pub error: Option<String>,
}

/// What changes while the app runs.
#[derive(Default)]
pub struct Live {
    /// The running loop's token; None (or cancelled) when nothing is watching.
    cancel: Option<CancelToken>,
    /// The last loop's thread, joined by the next loop before it starts so two never overlap.
    worker: Option<JoinHandle<()>>,
    /// Each account's row state, by its `Ryna.lua` path.
    phases: BTreeMap<String, Phase>,
    /// The last pass's whole-pass error, cleared by a pass that went through.
    last_error: Option<String>,
    login_running: bool,
    login: Option<LoginCode>,
    settings: Settings,
}

impl Live {
    fn watching(&self) -> bool {
        self.cancel.as_ref().is_some_and(|c| !c.is_cancelled())
    }

    /// One event's effect on the rows and the last error.
    pub fn observe(&mut self, event: &WatchEvent) {
        match event {
            WatchEvent::AccountFileChanged { path, .. } => {
                self.phases.insert(path.clone(), Phase::Waiting);
            }
            WatchEvent::Pushing { path, .. } => {
                self.phases.insert(path.clone(), Phase::Pushing);
            }
            WatchEvent::Pushed { path, .. } => {
                self.phases.insert(path.clone(), Phase::Pushed);
            }
            WatchEvent::Error {
                path: Some(path),
                message,
                ..
            } => {
                self.phases.insert(path.clone(), Phase::Error);
                self.last_error = Some(message.clone());
            }
            WatchEvent::Error {
                path: None,
                message,
                ..
            } => self.last_error = Some(message.clone()),
            WatchEvent::PassFinished { ok: true, .. } => self.last_error = None,
            _ => {}
        }
    }
}

/// The app's managed state.
pub struct Agent {
    pub config: Config,
    settings_dir: Option<PathBuf>,
    live: Mutex<Live>,
}

impl Agent {
    pub fn new(config: Config, settings_dir: Option<PathBuf>) -> Agent {
        let settings = settings_dir
            .as_deref()
            .map(Settings::load)
            .unwrap_or_default();
        Agent {
            config,
            settings_dir,
            live: Mutex::new(Live {
                settings,
                ..Live::default()
            }),
        }
    }

    fn live(&self) -> MutexGuard<'_, Live> {
        self.live.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn watching(&self) -> bool {
        self.live().watching()
    }
}

fn keychain(config: &Config) -> KeyringStore {
    KeyringStore::for_issuer(&config.issuer)
}

/// Whether a sign-in is kept on this computer (no network).
pub fn signed_in(config: &Config) -> bool {
    keychain(config).refresh_token().is_ok_and(|t| t.is_some())
}

/// Start the watch loop unless one is running. The new loop's thread first waits for the last
/// one to end, so a restart never runs two passes at once (the core's run lock would refuse it).
pub fn start_watch(app: &AppHandle) {
    let agent = app.state::<Agent>();
    {
        let mut live = agent.live();
        if live.watching() {
            return;
        }
        let cancel = CancelToken::new();
        live.cancel = Some(cancel.clone());
        let previous = live.worker.take();
        let install = live.settings.install_override.clone();
        let config = agent.config.clone();
        let handle = app.clone();
        live.worker = Some(std::thread::spawn(move || {
            if let Some(previous) = previous {
                let _ = previous.join();
            }
            run_loop(&handle, &config, install, &cancel);
        }));
    }
    let _ = app.emit(WATCH_STATE, true);
}

/// Stop the loop: at once while it waits, or at the end of the pass it's in.
pub fn stop_watch(app: &AppHandle) {
    cancel(app);
    let _ = app.emit(WATCH_STATE, false);
}

/// Stop the loop without telling the window (on quit).
pub fn cancel(app: &AppHandle) {
    let token = app.state::<Agent>().live().cancel.take();
    if let Some(token) = token {
        token.cancel();
    }
}

/// A pass now: restart the loop, whose first pass starts straight away.
pub fn sync_now(app: &AppHandle) {
    cancel(app);
    start_watch(app);
}

/// Restart the loop if it's running (after a setting it reads changed).
fn restart_if_watching(app: &AppHandle) {
    if app.state::<Agent>().watching() {
        sync_now(app);
    }
}

fn emit_event(app: &AppHandle, event: &WatchEvent) {
    app.state::<Agent>().live().observe(event);
    let _ = app.emit(AGENT_EVENT, event);
}

fn run_loop(app: &AppHandle, config: &Config, install: Option<PathBuf>, cancel: &CancelToken) {
    let result = match WatchOptions::new(install, INTERVAL) {
        Some(opts) => watch::watch(config, &opts, cancel, &mut |event: WatchEvent| {
            emit_event(app, &event)
        })
        .map_err(|e| e.to_string()),
        None => Err("there's no folder to keep the agent's state in".to_string()),
    };
    if let Err(message) = result {
        emit_event(
            app,
            &WatchEvent::Error {
                label: None,
                path: None,
                message,
                failure: Failure::Other,
            },
        );
    }
    // This loop has ended, whether asked to or not; a newer loop's token is left alone.
    cancel.cancel();
    let watching = app.state::<Agent>().watching();
    let _ = app.emit(WATCH_STATE, watching);
}

fn sign_in(config: &Config, show: &mut dyn FnMut(&DeviceCode)) -> Result<(), String> {
    let http = HttpTransport::new()?;
    let store = keychain(config);
    Auth::new(config, &http, &store, &SystemClock)
        .login(show)
        .map_err(|e| e.to_string())
}

/// Begin the device grant and return the code to show. The core's `login` is one call that
/// shows the code and then polls, so the polling carries on here on its own thread: when it
/// ends, `login-finished` is emitted, and on success the watch loop (re)starts.
pub fn start_login(app: &AppHandle) -> Result<LoginCode, String> {
    let agent = app.state::<Agent>();
    {
        let mut live = agent.live();
        if live.login_running {
            return live
                .login
                .clone()
                .ok_or_else(|| "A sign-in is already starting.".to_string());
        }
        live.login_running = true;
        live.login = None;
    }
    let config = agent.config.clone();
    let handle = app.clone();
    let (tx, rx) = mpsc::channel::<Result<LoginCode, String>>();
    std::thread::spawn(move || {
        let result = sign_in(&config, &mut |code: &DeviceCode| {
            let shown = LoginCode::from(code);
            handle.state::<Agent>().live().login = Some(shown.clone());
            let _ = tx.send(Ok(shown));
        });
        {
            let agent = handle.state::<Agent>();
            let mut live = agent.live();
            live.login_running = false;
            live.login = None;
        }
        let finished = LoginFinished {
            ok: result.is_ok(),
            error: result.err(),
        };
        if let Some(error) = &finished.error {
            // Reaches start_login only if no code was shown yet; otherwise nobody is listening.
            let _ = tx.send(Err(error.clone()));
        }
        let _ = handle.emit(LOGIN_FINISHED, &finished);
        if finished.ok {
            sync_now(&handle);
        }
    });
    rx.recv_timeout(CODE_WAIT).unwrap_or_else(|_| {
        Err("The sign-in provider didn't answer. Try again in a minute.".to_string())
    })
}

/// The pending sign-in's link, for the "Open sign-in page" button.
pub fn login_link(app: &AppHandle) -> Option<String> {
    app.state::<Agent>()
        .live()
        .login
        .as_ref()
        .map(|c| c.link.clone())
}

/// Forget the sign-in on this computer and stop syncing.
pub fn logout(app: &AppHandle) -> Result<(), String> {
    stop_watch(app);
    let agent = app.state::<Agent>();
    keychain(&agent.config).clear()?;
    let mut live = agent.live();
    live.phases.clear();
    live.last_error = None;
    Ok(())
}

pub fn install_override(app: &AppHandle) -> Option<PathBuf> {
    app.state::<Agent>()
        .live()
        .settings
        .install_override
        .clone()
}

/// Point the agent at a folder (or back at the OS defaults with None), keep it, and restart
/// the loop so it looks there.
pub fn set_install_override(app: &AppHandle, path: Option<PathBuf>) -> Result<(), String> {
    if let Some(p) = &path {
        if !p.is_dir() {
            return Err(format!("{} isn't a folder.", p.display()));
        }
    }
    let agent = app.state::<Agent>();
    let settings = {
        let mut live = agent.live();
        live.settings.install_override = path;
        live.phases.clear();
        live.settings.clone()
    };
    if let Some(dir) = &agent.settings_dir {
        settings
            .save(dir)
            .map_err(|e| format!("Couldn't save the setting: {e}"))?;
    }
    restart_if_watching(app);
    Ok(())
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallView {
    pub path: String,
    pub flavor: Option<String>,
    /// Forever (`wowf`): the addon is installed only into these unless a folder was chosen.
    pub forever: bool,
    /// `Interface/AddOns/Ryna` is there.
    pub has_addon: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountView {
    pub path: String,
    pub install: String,
    pub label: String,
    /// When the game last wrote the file (unix seconds).
    pub saved_at: Option<i64>,
    pub pushed_at: Option<i64>,
    pub characters: Option<usize>,
    pub last_error: Option<String>,
    pub phase: Phase,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LastRunView {
    pub at: i64,
    pub ok: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub agent_version: String,
    pub app_version: String,
    pub api: String,
    pub issuer: String,
    pub client_id_issued: bool,
    pub signed_in: bool,
    pub signed_in_as: Option<String>,
    /// The keychain couldn't be read (so signed-in is unknown).
    pub keychain_error: Option<String>,
    pub watching: bool,
    pub installs: Vec<InstallView>,
    pub accounts: Vec<AccountView>,
    pub last_error: Option<String>,
    pub last_run: Option<LastRunView>,
    pub install_override: Option<String>,
    pub login: Option<LoginCode>,
    pub state_dir: Option<String>,
}

fn has_addon(install: &Install) -> bool {
    install
        .path
        .join("Interface")
        .join("AddOns")
        .join(addon_install::ADDON_NAME)
        .is_dir()
}

fn display(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

/// Everything the window shows, read fresh from the keychain, the disk and the live state.
pub fn status(app: &AppHandle) -> Status {
    let agent = app.state::<Agent>();
    let config = &agent.config;
    let (watching, install_override, phases, last_error, login) = {
        let live = agent.live();
        (
            live.watching(),
            live.settings.install_override.clone(),
            live.phases.clone(),
            live.last_error.clone(),
            live.login.clone(),
        )
    };
    let store = keychain(config);
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

    let found = discovery::discover(install_override.as_deref());
    let dir = state::state_dir();
    let st = dir.as_deref().map(State::load).unwrap_or_default();
    let installs = found
        .installs
        .iter()
        .map(|i| InstallView {
            path: display(&i.path),
            flavor: i.flavor.clone(),
            forever: i.flavor.as_deref() == Some("wowf"),
            has_addon: has_addon(i),
        })
        .collect();
    let accounts = found
        .accounts
        .iter()
        .map(|a| {
            let key = display(&a.path);
            let s = st.accounts.get(&key);
            AccountView {
                install: display(&a.install),
                label: s.map_or_else(|| snapshot::default_label(&a.account), |s| s.label.clone()),
                saved_at: mtime(&a.path)
                    .ok()
                    .map(|m| m.secs)
                    .or_else(|| s.and_then(|s| s.saved_at)),
                pushed_at: s.and_then(|s| s.pushed_at),
                characters: s.and_then(|s| s.characters),
                last_error: s.and_then(|s| s.last_error.clone()),
                phase: phases.get(&key).copied().unwrap_or(Phase::Waiting),
                path: key,
            }
        })
        .collect();
    Status {
        agent_version: AGENT_VERSION.to_string(),
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        api: config.api.clone(),
        issuer: config.issuer.clone(),
        client_id_issued: config.client_id_issued(),
        signed_in,
        signed_in_as,
        keychain_error,
        watching,
        installs,
        accounts,
        last_error,
        last_run: st.last_run.map(|r| LastRunView {
            at: r.at,
            ok: r.ok,
            error: r.error,
        }),
        install_override: install_override.as_deref().map(display),
        login,
        state_dir: dir.as_deref().map(display),
    }
}

/// Where the bundled addon is: the app's resources (`addon/Ryna`, from the repository's
/// `addon/Ryna`), or in a debug build the repository itself.
fn bundled_addon(app: &AppHandle) -> Result<PathBuf, String> {
    let mut tried = Vec::new();
    if let Ok(dir) = app
        .path()
        .resolve("addon/Ryna", tauri::path::BaseDirectory::Resource)
    {
        tried.push(dir);
    }
    #[cfg(debug_assertions)]
    let checkout = Some(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../addon/Ryna"));
    #[cfg(not(debug_assertions))]
    let checkout: Option<PathBuf> = None;
    tried.extend(checkout);
    tried
        .iter()
        .find_map(|dir| addon_install::resolve_source(dir).ok())
        .ok_or_else(|| "This build doesn't carry the addon.".to_string())
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Installed {
    pub path: String,
}

/// Copy the bundled addon into each Forever install (or, when a folder was chosen, every
/// install found there), as `ryna-wow install-addon` does.
pub fn install_addon(app: &AppHandle) -> Result<Vec<Installed>, String> {
    let source = bundled_addon(app)?;
    let chosen = install_override(app);
    let found = discovery::discover(chosen.as_deref());
    let targets: Vec<&Install> = found
        .installs
        .iter()
        .filter(|i| chosen.is_some() || i.flavor.as_deref() == Some("wowf"))
        .collect();
    if targets.is_empty() {
        return Err(
            "No World of Warcraft: Forever install found. Choose the game's folder first."
                .to_string(),
        );
    }
    targets
        .into_iter()
        .map(|i| {
            addon_install::install(&source, i)
                .map(|p| Installed { path: display(&p) })
                .map_err(|e| format!("{e:#}"))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const A: &str = "/wow/_forever_/WTF/Account/A/SavedVariables/Ryna.lua";

    fn failed(path: Option<&str>) -> WatchEvent {
        WatchEvent::Error {
            label: None,
            path: path.map(str::to_string),
            message: "offline".into(),
            failure: Failure::Other,
        }
    }

    #[test]
    fn rows_follow_the_events_and_a_good_pass_clears_the_error() {
        let mut live = Live::default();
        live.observe(&WatchEvent::Pushing {
            label: "UNT1".into(),
            path: A.into(),
        });
        assert_eq!(live.phases[A], Phase::Pushing);
        live.observe(&failed(Some(A)));
        assert_eq!(live.phases[A], Phase::Error);
        assert_eq!(live.last_error.as_deref(), Some("offline"));
        live.observe(&WatchEvent::Pushed {
            label: "UNT1".into(),
            path: A.into(),
            characters: 2,
            result: json!({}),
        });
        assert_eq!(live.phases[A], Phase::Pushed);
        live.observe(&WatchEvent::PassFinished {
            ok: true,
            next_in_secs: 60,
        });
        assert_eq!(live.last_error, None);
        live.observe(&failed(None));
        assert_eq!(live.last_error.as_deref(), Some("offline"));
    }

    #[test]
    fn a_loop_counts_as_watching_until_its_token_is_cancelled() {
        let mut live = Live::default();
        assert!(!live.watching());
        let token = CancelToken::new();
        live.cancel = Some(token.clone());
        assert!(live.watching());
        token.cancel();
        assert!(!live.watching());
    }

    #[test]
    fn the_login_code_links_to_the_page_with_the_code_in_it() {
        let code = DeviceCode {
            user_code: "ABCD-EFGH".into(),
            verification_uri: "https://idp.test/device".into(),
            verification_uri_complete: Some("https://idp.test/device?user_code=ABCD-EFGH".into()),
            expires_in: 600,
        };
        let shown = LoginCode::from(&code);
        assert_eq!(shown.link, "https://idp.test/device?user_code=ABCD-EFGH");
        assert_eq!(
            serde_json::to_value(&shown).unwrap()["userCode"],
            json!("ABCD-EFGH")
        );
    }
}
