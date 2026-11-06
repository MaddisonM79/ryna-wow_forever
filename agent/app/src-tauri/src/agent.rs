//! The app's side of the core: one watch loop at a time on its own thread, the sign-in, and the
//! status the window shows. Nothing here outlives the app: quitting cancels the loop and any
//! sign-in, and there is no service, scheduled task or launchd job behind it.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::Duration;

use ryna_wow_core::addon_install;
use ryna_wow_core::auth::{self, AuthError, DeviceLogin, KeyringStore, TokenStore};
use ryna_wow_core::cancel::CancelToken;
use ryna_wow_core::config::Config;
use ryna_wow_core::discovery::{self, Install};
use ryna_wow_core::fix::Fix;
use ryna_wow_core::status::{self as core_status, AccountStatus, InstallStatus, LastRunStatus};
use ryna_wow_core::watch::{self, WatchControl, WatchEvent, WatchOptions};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::settings::Settings;

/// How often the loop looks for new saves while the app is open.
pub const INTERVAL: Duration = Duration::from_secs(60);
/// Every `WatchEvent`, as the core serialises it (`{"event": "pushed", ...}`).
pub const AGENT_EVENT: &str = "agent-event";
/// `true` or `false`: whether a watch loop is running.
pub const WATCH_STATE: &str = "watch-state";
/// A `LoginFinished`, once the device grant ends either way (approved, refused, expired or
/// cancelled).
pub const LOGIN_FINISHED: &str = "login-finished";

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

/// The code to show the person, and where to check it: the core's `DeviceLogin` (less the
/// device code, which stays in Rust), plus the link to open.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginCode {
    pub user_code: String,
    pub verification_uri: String,
    pub verification_uri_complete: Option<String>,
    /// The link to open: the one with the code in it when the provider gives one.
    pub link: String,
    /// When the code stops working (unix seconds).
    pub expires_at: i64,
    /// Seconds between polls.
    pub interval: u64,
}

impl From<&DeviceLogin> for LoginCode {
    fn from(login: &DeviceLogin) -> Self {
        LoginCode {
            user_code: login.user_code.clone(),
            verification_uri: login.verification_uri.clone(),
            verification_uri_complete: login.verification_uri_complete.clone(),
            link: login.link().to_string(),
            expires_at: login.expires_at,
            interval: login.interval,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginFinished {
    pub ok: bool,
    /// Ended by `cancel_login` (no error to show).
    pub cancelled: bool,
    pub error: Option<String>,
    pub fix: Option<Fix>,
}

impl LoginFinished {
    fn from_result<T>(result: &Result<T, AuthError>) -> LoginFinished {
        match result {
            Ok(_) => LoginFinished {
                ok: true,
                cancelled: false,
                error: None,
                fix: None,
            },
            Err(AuthError::Cancelled) => LoginFinished {
                ok: false,
                cancelled: true,
                error: None,
                fix: None,
            },
            Err(e) => LoginFinished {
                ok: false,
                cancelled: false,
                error: Some(sentence(e)),
                fix: Some(e.fix()),
            },
        }
    }
}

/// An error as a sentence: a capital first letter and a full stop.
fn sentence(e: &AuthError) -> String {
    let text = e.to_string();
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => format!("{}{}.", first.to_uppercase(), chars.as_str()),
        None => String::new(),
    }
}

/// What changes while the app runs.
#[derive(Default)]
pub struct Live {
    /// The running loop's control; None (or cancelled) when nothing is watching.
    control: Option<WatchControl>,
    /// The last loop's thread, joined by the next loop before it starts so two never overlap.
    worker: Option<JoinHandle<()>>,
    /// Each account's row state, by its `Ryna.lua` path.
    phases: BTreeMap<String, Phase>,
    /// The last pass's whole-pass error (its neutral words), cleared by a pass that went through.
    last_error: Option<String>,
    /// The sign-in's token while one is starting or polling; `cancel_login` fires it.
    login_cancel: Option<CancelToken>,
    login: Option<LoginCode>,
    settings: Settings,
}

impl Live {
    fn watching(&self) -> bool {
        self.control.as_ref().is_some_and(|c| !c.is_cancelled())
    }

    fn login_running(&self) -> bool {
        self.login_cancel
            .as_ref()
            .is_some_and(|t| !t.is_cancelled())
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
                advice,
                ..
            } => {
                self.phases.insert(path.clone(), Phase::Error);
                self.last_error = Some(advice.clone());
            }
            WatchEvent::Error {
                path: None, advice, ..
            } => self.last_error = Some(advice.clone()),
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
        let control = WatchControl::new();
        live.control = Some(control.clone());
        let previous = live.worker.take();
        let install = live.settings.install_override.clone();
        let config = agent.config.clone();
        let handle = app.clone();
        live.worker = Some(std::thread::spawn(move || {
            if let Some(previous) = previous {
                let _ = previous.join();
            }
            run_loop(&handle, &config, install, &control);
        }));
    }
    let _ = app.emit(WATCH_STATE, true);
}

/// Stop the loop: at once while it waits, or at the end of the pass it's in.
pub fn stop_watch(app: &AppHandle) {
    cancel(app);
    let _ = app.emit(WATCH_STATE, false);
}

/// Stop the loop without telling the window.
pub fn cancel(app: &AppHandle) {
    let control = app.state::<Agent>().live().control.take();
    if let Some(control) = control {
        control.cancel();
    }
}

/// On quit: stop the loop and any sign-in that is polling.
pub fn shutdown(app: &AppHandle) {
    cancel(app);
    cancel_login(app);
}

/// A pass now: wake the running loop (its wait ends, or the pass in progress is followed by
/// another), or start one, whose first pass starts straight away.
pub fn sync_now(app: &AppHandle) {
    let running = app
        .state::<Agent>()
        .live()
        .control
        .clone()
        .filter(|c| !c.is_cancelled());
    match running {
        Some(control) => control.request_pass(),
        None => start_watch(app),
    }
}

/// Restart the loop if it's running (after a setting it reads at start changed).
fn restart_if_watching(app: &AppHandle) {
    if app.state::<Agent>().watching() {
        cancel(app);
        start_watch(app);
    }
}

fn emit_event(app: &AppHandle, event: &WatchEvent) {
    app.state::<Agent>().live().observe(event);
    let _ = app.emit(AGENT_EVENT, event);
}

fn run_loop(app: &AppHandle, config: &Config, install: Option<PathBuf>, control: &WatchControl) {
    let result = match WatchOptions::new(install, INTERVAL) {
        Some(opts) => {
            watch::watch_with_control(config, &opts, control, &mut |event: WatchEvent| {
                emit_event(app, &event)
            })
            .map_err(|e| e.to_string())
        }
        None => Err("there's no folder to keep the agent's state in".to_string()),
    };
    if let Err(message) = result {
        emit_event(app, &WatchEvent::error(None, None, message, Fix::Other));
    }
    // This loop has ended, whether asked to or not; a newer loop's control is left alone.
    control.cancel();
    let watching = app.state::<Agent>().watching();
    let _ = app.emit(WATCH_STATE, watching);
}

/// Forget the sign-in `token` belongs to, unless a newer one has replaced it.
fn clear_login(app: &AppHandle, token: &CancelToken) {
    let agent = app.state::<Agent>();
    let mut live = agent.live();
    if live.login_cancel.as_ref().is_some_and(|t| t.same(token)) {
        live.login_cancel = None;
        live.login = None;
    }
}

/// Begin the device grant and return the code to show (the core's `start_device_login`); the
/// polling (`poll_device_login`) carries on on its own thread with a cancel token kept here, so
/// `cancel_login` stops it. When it ends, `login-finished` is emitted, and on success a pass runs.
pub fn start_login(app: &AppHandle) -> Result<LoginCode, String> {
    let agent = app.state::<Agent>();
    let token = {
        let mut live = agent.live();
        if live.login_running() {
            return live
                .login
                .clone()
                .ok_or_else(|| "A sign-in is already starting.".to_string());
        }
        let token = CancelToken::new();
        live.login_cancel = Some(token.clone());
        live.login = None;
        token
    };
    let config = agent.config.clone();
    let login = match auth::start_device_login(&config) {
        Ok(login) => login,
        Err(e) => {
            clear_login(app, &token);
            return Err(sentence(&e));
        }
    };
    let code = LoginCode::from(&login);
    {
        let mut live = agent.live();
        let still_ours = live.login_cancel.as_ref().is_some_and(|t| t.same(&token));
        if !still_ours || token.is_cancelled() {
            return Err("The sign-in was cancelled.".to_string());
        }
        live.login = Some(code.clone());
    }
    let handle = app.clone();
    std::thread::spawn(move || {
        let result = auth::poll_device_login(&config, &login, &token, &mut |_| {});
        clear_login(&handle, &token);
        let finished = LoginFinished::from_result(&result);
        let _ = handle.emit(LOGIN_FINISHED, &finished);
        if finished.ok {
            sync_now(&handle);
        }
    });
    Ok(code)
}

/// Stop a sign-in that is starting or polling; its `login-finished` says `cancelled`.
pub fn cancel_login(app: &AppHandle) {
    let agent = app.state::<Agent>();
    let token = {
        let mut live = agent.live();
        live.login = None;
        live.login_cancel.take()
    };
    if let Some(token) = token {
        token.cancel();
    }
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

/// An account's row: the core's `AccountStatus` and where the loop has it.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountView {
    #[serde(flatten)]
    pub account: AccountStatus,
    pub phase: Phase,
}

/// The core's `Status`, with what only the app knows (the loop, the rows, the sign-in code).
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
    pub installs: Vec<InstallStatus>,
    /// The bundled addon's version (its `Ryna.toc`), when this build carries one that says.
    pub addon_bundled: Option<String>,
    pub accounts: Vec<AccountView>,
    /// What to show for the last failure: this session's, else the last run's.
    pub last_error: Option<String>,
    pub last_run: Option<LastRunStatus>,
    pub install_override: Option<String>,
    pub login: Option<LoginCode>,
    pub state_dir: Option<String>,
}

fn display(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

/// Everything the window shows: the core's `status` (keychain, disk, state) and the live state.
pub fn status(app: &AppHandle) -> Status {
    let agent = app.state::<Agent>();
    let (watching, install_override, phases, live_error, login) = {
        let live = agent.live();
        (
            live.watching(),
            live.settings.install_override.clone(),
            live.phases.clone(),
            live.last_error.clone(),
            live.login.clone(),
        )
    };
    let core_status::Status {
        agent_version,
        api,
        issuer,
        client_id_issued,
        signed_in,
        signed_in_as,
        keychain_error,
        installs,
        addon_bundled,
        accounts,
        last_error: _,
        last_run,
        state_dir,
    } = core_status::status(
        &agent.config,
        install_override.as_deref(),
        bundled_addon(app).ok().as_deref(),
    );
    let accounts = accounts
        .into_iter()
        .map(|account| AccountView {
            phase: phases.get(&account.path).copied().unwrap_or(Phase::Waiting),
            account,
        })
        .collect();
    let last_error = live_error.or_else(|| last_run.as_ref().and_then(|r| r.advice.clone()));
    Status {
        agent_version,
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        api,
        issuer,
        client_id_issued,
        signed_in,
        signed_in_as,
        keychain_error,
        watching,
        installs,
        addon_bundled,
        accounts,
        last_error,
        last_run,
        install_override: install_override.as_deref().map(display),
        login,
        state_dir,
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
    /// The install it went into (as `InstallStatus::path` has it).
    pub install: String,
    /// The addon's folder there.
    pub path: String,
}

/// What `install_addon` did.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallResult {
    /// The version installed (the bundled `Ryna.toc`'s), when it says.
    pub version: Option<String>,
    pub installed: Vec<Installed>,
    /// The game was running when it finished, so it needs a /reload (or a log out and in).
    pub game_running: bool,
}

/// The installs `install_addon` may write to: Forever installs, or every install found when a
/// folder was chosen; only the one at `only` (an `InstallStatus::path`) when it's given.
fn targets<'a>(
    installs: &'a [Install],
    chosen: bool,
    only: Option<&str>,
) -> Result<Vec<&'a Install>, String> {
    let allowed: Vec<&Install> = installs
        .iter()
        .filter(|i| chosen || i.flavor.as_deref() == Some(core_status::FOREVER_FLAVOR))
        .collect();
    let picked: Vec<&Install> = match only {
        Some(path) => allowed
            .into_iter()
            .filter(|i| display(&i.path) == path)
            .collect(),
        None => allowed,
    };
    if picked.is_empty() {
        return Err(match only {
            Some(path) => format!("{path} isn't a World of Warcraft: Forever install."),
            None => "No World of Warcraft: Forever install found. Choose the game's folder first."
                .to_string(),
        });
    }
    Ok(picked)
}

/// Copy the bundled addon into one install (`only`, an `InstallStatus::path`) or into each
/// Forever install (or, when a folder was chosen, every install found there), as
/// `ryna-wow install-addon` does.
pub fn install_addon(app: &AppHandle, only: Option<&str>) -> Result<InstallResult, String> {
    let source = bundled_addon(app)?;
    let version = addon_install::bundled_addon_version(&source);
    let chosen = install_override(app);
    let found = discovery::discover(chosen.as_deref());
    let installed = targets(&found.installs, chosen.is_some(), only)?
        .into_iter()
        .map(|i| {
            addon_install::install(&source, i)
                .map(|p| Installed {
                    install: display(&i.path),
                    path: display(&p),
                })
                .map_err(|e| format!("{e:#}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(InstallResult {
        version,
        installed,
        game_running: crate::game::game_running(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const A: &str = "/wow/_forever_/WTF/Account/A/SavedVariables/Ryna.lua";

    fn failed(path: Option<&str>) -> WatchEvent {
        WatchEvent::error(None, path.map(str::to_string), "offline".into(), Fix::Other)
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
        // The neutral words, not the error itself.
        live.observe(&WatchEvent::error(
            None,
            None,
            "the API is unreachable or failing after 3 tries: x".into(),
            Fix::Network,
        ));
        assert_eq!(live.last_error.as_deref(), Some("Couldn't reach the API"));
    }

    #[test]
    fn a_loop_counts_as_watching_until_its_control_is_cancelled() {
        let mut live = Live::default();
        assert!(!live.watching());
        let control = WatchControl::new();
        live.control = Some(control.clone());
        assert!(live.watching());
        control.request_pass();
        assert!(live.watching());
        control.cancel();
        assert!(!live.watching());
    }

    #[test]
    fn a_sign_in_runs_until_its_token_is_cancelled() {
        let mut live = Live::default();
        assert!(!live.login_running());
        let token = CancelToken::new();
        live.login_cancel = Some(token.clone());
        assert!(live.login_running());
        token.cancel();
        assert!(!live.login_running());
    }

    #[test]
    fn the_login_code_links_to_the_page_with_the_code_in_it() {
        let login = DeviceLogin {
            user_code: "ABCD-EFGH".into(),
            verification_uri: "https://idp.test/device".into(),
            verification_uri_complete: Some("https://idp.test/device?user_code=ABCD-EFGH".into()),
            expires_at: 1_800_000_600,
            interval: 5,
            device_code: "secret".into(),
        };
        let shown = LoginCode::from(&login);
        assert_eq!(shown.link, "https://idp.test/device?user_code=ABCD-EFGH");
        let v = serde_json::to_value(&shown).unwrap();
        assert_eq!(v["userCode"], json!("ABCD-EFGH"));
        assert_eq!(v["expiresAt"], json!(1_800_000_600));
        assert!(!v.to_string().contains("secret"));
    }

    #[test]
    fn installs_into_forever_unless_a_folder_was_chosen() {
        let forever = Install {
            path: PathBuf::from("/wow/_forever_"),
            flavor: Some("wowf".into()),
        };
        let retail = Install {
            path: PathBuf::from("/wow/_retail_"),
            flavor: Some("wow".into()),
        };
        let both = [forever.clone(), retail.clone()];
        let paths = |t: Vec<&Install>| t.iter().map(|i| display(&i.path)).collect::<Vec<_>>();
        assert_eq!(
            paths(targets(&both, false, None).unwrap()),
            ["/wow/_forever_"]
        );
        assert_eq!(
            paths(targets(&both, false, Some("/wow/_forever_")).unwrap()),
            ["/wow/_forever_"]
        );
        assert!(targets(&both, false, Some("/wow/_retail_")).is_err());
        assert_eq!(
            paths(targets(&both, true, Some("/wow/_retail_")).unwrap()),
            ["/wow/_retail_"]
        );
        assert_eq!(targets(&both, true, None).unwrap().len(), 2);
        assert!(targets(&[retail], false, None).is_err());
    }

    #[test]
    fn how_a_sign_in_ended() {
        let cancelled = LoginFinished::from_result::<()>(&Err(AuthError::Cancelled));
        assert!(cancelled.cancelled && !cancelled.ok && cancelled.error.is_none());
        let expired = LoginFinished::from_result::<()>(&Err(AuthError::Expired));
        assert_eq!(expired.error.as_deref(), Some("The sign-in code expired."));
        assert_eq!(expired.fix, Some(Fix::SignIn));
        assert!(LoginFinished::from_result(&Ok(())).ok);
    }
}
