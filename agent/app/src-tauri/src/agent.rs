//! The app's side of the agent: one core run at a time on its own thread (the WoW core, driven
//! through `ryna_agent::core::Core`), the sign-in, and the status the window shows. Nothing here outlives the app: quitting cancels the loop and any
//! sign-in, and there is no service, scheduled task or launchd job behind it.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::Duration;

use ryna_agent::auth::{self, AuthError, DeviceLogin, KeyringStore, TokenStore};
use ryna_agent::cancel::CancelToken;
use ryna_agent::config::Config;
pub use ryna_agent::core::Phase;
use ryna_agent::core::{Activity, Core, CoreContext, CoreControl, KeychainTokens, StatusRow};
use ryna_agent::fix::Fix;
use ryna_agent::identity::{self, DeviceKey};
use ryna_wow_core::addon_install;
use ryna_wow_core::discovery::{self, Install};
use ryna_wow_core::mods::{self as core_mods, ModChange, ModStatus};
use ryna_wow_core::status::{
    self as core_status, AccountStatus, CombatStatus, InstallStatus, LastRunStatus,
};
use ryna_wow_core::wow_core::WowCore;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::settings::Settings;

/// How often the loop looks for new saves while the app is open.
pub const INTERVAL: Duration = Duration::from_secs(60);
/// Every core event, as the core serialises it (the WoW core's `WatchEvent`: `{"event": "pushed", ...}`).
pub const AGENT_EVENT: &str = "agent-event";
/// `true` or `false`: whether a watch loop is running.
pub const WATCH_STATE: &str = "watch-state";
/// A `LoginFinished`, once the device grant ends either way (approved, refused, expired or
/// cancelled).
pub const LOGIN_FINISHED: &str = "login-finished";

/// The device client id: `RYNA_CLIENT_ID` when it is set where the app runs, else one baked
/// in at compile time (`RYNA_CLIENT_ID` in the build's environment), else the core's
/// placeholder, which `login` refuses. API and issuer come from the core (env overrides).
pub fn config() -> Config {
    let mut config = Config::resolve(None, None);
    let baked = option_env!("RYNA_CLIENT_ID")
        .map(str::trim)
        .filter(|id| !id.is_empty());
    if let (false, Some(id)) = (config.client_id_issued(), baked) {
        config.client_id = id.to_string();
    }
    config
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
    /// The running core's control; None (or stopped) when nothing is watching.
    control: Option<CoreControl>,
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
        self.control.as_ref().is_some_and(|c| !c.is_stopped())
    }

    fn login_running(&self) -> bool {
        self.login_cancel
            .as_ref()
            .is_some_and(|t| !t.is_cancelled())
    }

    /// One event's effect on the rows and the last error (what the core's `activity` says of it).
    pub fn observe(&mut self, activity: &Activity) {
        if let Some((key, phase)) = &activity.row {
            self.phases.insert(key.clone(), *phase);
        }
        if let Some(error) = &activity.error {
            self.last_error = Some(error.clone());
        }
        if activity.clears_error {
            self.last_error = None;
        }
    }
}

/// The app's managed state.
pub struct Agent {
    pub config: Config,
    settings_dir: Option<PathBuf>,
    live: Mutex<Live>,
    /// The device key, once the keychain gave it (kept for the app's life; a failure is tried
    /// again next time it's needed).
    device_key: Mutex<Option<Arc<dyn DeviceKey>>>,
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
            device_key: Mutex::new(None),
        }
    }

    /// The device key: from the keychain (minted there on first use), then kept in memory. The
    /// error is the keychain's words; a kept key that doesn't parse is never replaced.
    pub fn device_key(&self) -> Result<Arc<dyn DeviceKey>, String> {
        let mut kept = self.device_key.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(k) = kept.as_ref() {
            return Ok(k.clone());
        }
        let key: Arc<dyn DeviceKey> =
            Arc::new(identity::device_key(&self.config).map_err(|e| e.to_string())?);
        *kept = Some(key.clone());
        Ok(key)
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
        let control = CoreControl::new();
        live.control = Some(control.clone());
        let previous = live.worker.take();
        let install = live.settings.install_override.clone();
        let combat = live.settings.combat();
        let config = agent.config.clone();
        let handle = app.clone();
        live.worker = Some(std::thread::spawn(move || {
            if let Some(previous) = previous {
                let _ = previous.join();
            }
            run_loop(&handle, &config, install, combat, &control);
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
        control.stop();
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
        .filter(|c| !c.is_stopped());
    match running {
        Some(control) => control.sync(),
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

/// What the WoW core is started with: the agent's state folder, the keychain's tokens, the
/// device key (None when the keychain couldn't give it: the window says so, and the posts go
/// unsigned) and the chosen game folder. None when there's no state folder.
fn context(
    config: &Config,
    device_key: Option<Arc<dyn DeviceKey>>,
    install: Option<PathBuf>,
) -> Option<CoreContext> {
    Some(CoreContext {
        state_dir: ryna_agent::state_dir::state_dir()?,
        tokens: Arc::new(KeychainTokens::new(config.clone())),
        device_key,
        game_folder: install,
    })
}

/// Run `core` until `control` stops it, telling `on_event` everything it does; a core that can't
/// run (or no state folder) becomes one failure event. Whatever ended it, `control` is stopped.
pub fn drive<C: Core>(
    core: &C,
    ctx: Option<&CoreContext>,
    control: &CoreControl,
    on_event: &mut dyn FnMut(&C::Event),
) {
    let result = match ctx {
        Some(ctx) => core
            .start(ctx, control, &mut |event| on_event(&event))
            .map_err(|e| e.to_string()),
        None => Err("there's no folder to keep the agent's state in".to_string()),
    };
    if let Err(message) = result {
        on_event(&core.failed(message, Fix::Other));
    }
    control.stop();
}

fn run_loop(
    app: &AppHandle,
    config: &Config,
    install: Option<PathBuf>,
    combat: ryna_wow_core::combat::CombatOptions,
    control: &CoreControl,
) {
    let core = WowCore::new(INTERVAL).with_combat(combat);
    let ctx = context(config, app.state::<Agent>().device_key().ok(), install);
    drive(&core, ctx.as_ref(), control, &mut |event| {
        app.state::<Agent>().live().observe(&core.activity(event));
        let _ = app.emit(AGENT_EVENT, event);
    });
    // This loop has ended, whether asked to or not; a newer loop's control is left alone.
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

/// Whether the mod `id` is on (the person's choice, else its manifest's default).
pub fn mod_on(app: &AppHandle, id: &str) -> bool {
    let chosen = app.state::<Agent>().live().settings.mods.clone();
    core_mods::bundled()
        .iter()
        .find(|m| m.id == id)
        .is_some_and(|m| core_mods::is_on(m, &chosen))
}

/// The installs the Settings mod works in: the ones the addon goes into (Forever installs, or every
/// install in a chosen folder).
pub fn settings_installs(app: &AppHandle) -> Vec<Install> {
    let chosen = install_override(app);
    let found = discovery::discover(chosen.as_deref());
    targets(&found.installs, chosen.is_some(), None)
        .map(|v| v.into_iter().cloned().collect())
        .unwrap_or_default()
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

/// The two combat settings: send each fight's digest, keep its raw lines on ryna.io. Kept, and
/// the loop restarted so it reads them.
pub fn set_combat_settings(app: &AppHandle, digests: bool, raw: bool) -> Result<(), String> {
    let agent = app.state::<Agent>();
    let settings = {
        let mut live = agent.live();
        live.settings.send_digests = digests;
        live.settings.keep_raw = raw;
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
    /// The sign-in lasts only until a restart (Linux with no Secret Service running).
    pub sign_in_until_reboot: bool,
    /// The device key couldn't be read or kept (the keychain's words); the window shows Problem.
    pub device_key_error: Option<String>,
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
    /// The combat log: the newest log, fights uploaded tonight, the last fight.
    pub combat: CombatStatus,
    /// Each core's status card, as the core lays it out (through the core interface; the window
    /// still draws its own cards from the fields above in stage 1).
    pub cores: Vec<CoreView>,
    /// The WoW core's mods: on or off, what's bundled, what's installed in each install.
    pub mods: Vec<ModStatus>,
    /// "Send fight digests".
    pub send_digests: bool,
    /// "Keep raw logs on ryna.io".
    pub keep_raw: bool,
}

/// A core's card: what it is, and its rows.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CoreView {
    pub id: String,
    pub name: String,
    pub version: String,
    pub rows: Vec<StatusRow>,
}

fn core_view<C: Core>(core: &C, ctx: Option<&CoreContext>) -> CoreView {
    let m = core.manifest();
    CoreView {
        id: m.id.clone(),
        name: m.name.clone(),
        version: m.version.clone(),
        rows: ctx.map(|ctx| core.status_rows(ctx)).unwrap_or_default(),
    }
}

fn display(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

/// Everything the window shows: the core's `status` (keychain, disk, state) and the live state.
pub fn status(app: &AppHandle) -> Status {
    let agent = app.state::<Agent>();
    let (watching, install_override, phases, live_error, login, settings) = {
        let live = agent.live();
        (
            live.watching(),
            live.settings.install_override.clone(),
            live.phases.clone(),
            live.last_error.clone(),
            live.login.clone(),
            live.settings.clone(),
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
        sign_in_until_reboot,
        installs,
        addon_bundled,
        accounts,
        last_error: _,
        last_run,
        combat,
        state_dir,
    } = {
        // stage 1: direct; the core interface takes this in stage 2 (the window's own cards read
        // the core's full status; `cores` below is the same core through the interface).
        core_status::status(
            &agent.config,
            install_override.as_deref(),
            bundled_addon(app).ok().as_deref(),
        )
    };
    let wow = WowCore::new(INTERVAL)
        .with_bundled_addon(bundled_addon(app).ok())
        .with_mods(settings.mods.clone());
    let device_key = agent.device_key();
    let device_key_error = device_key.as_ref().err().cloned();
    let ctx = context(&agent.config, device_key.ok(), install_override.clone());
    let cores = vec![core_view(&wow, ctx.as_ref())];
    let mods = ctx
        .as_ref()
        .map(|ctx| wow.mod_statuses(ctx))
        .unwrap_or_default();
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
        sign_in_until_reboot,
        device_key_error,
        watching,
        installs,
        addon_bundled,
        accounts,
        last_error,
        last_run,
        install_override: install_override.as_deref().map(display),
        login,
        state_dir,
        combat,
        cores,
        mods,
        send_digests: settings.send_digests,
        keep_raw: settings.keep_raw,
    }
}

/// Where the bundled addon is: the app's resources (`addon/Ryna`, from the repository's
/// `cores/wow/addon/Ryna`), or in a debug build the repository itself.
fn bundled_addon(app: &AppHandle) -> Result<PathBuf, String> {
    let mut tried = Vec::new();
    if let Ok(dir) = app
        .path()
        .resolve("addon/Ryna", tauri::path::BaseDirectory::Resource)
    {
        tried.push(dir);
    }
    #[cfg(debug_assertions)]
    let checkout =
        Some(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../cores/wow/addon/Ryna"));
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
    /// Each mod folder installed (an on mod) or removed (an off one) with it.
    pub mods: Vec<ModChange>,
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
/// `ryna install-addon` does.
pub fn install_addon(app: &AppHandle, only: Option<&str>) -> Result<InstallResult, String> {
    // stage 1: direct; the core interface takes this in stage 2 (installing the core's addon).
    let source = bundled_addon(app)?;
    let version = addon_install::bundled_addon_version(&source);
    let chosen = install_override(app);
    let found = discovery::discover(chosen.as_deref());
    let chosen_mods = app.state::<Agent>().live().settings.mods.clone();
    let mut installed = Vec::new();
    let mut changes = Vec::new();
    for i in targets(&found.installs, chosen.is_some(), only)? {
        let p = addon_install::install(&source, i).map_err(|e| format!("{e:#}"))?;
        installed.push(Installed {
            install: display(&i.path),
            path: display(&p),
        });
        // The core's addon brings its mods: each on one in, each off one out.
        changes.extend(
            core_mods::apply(i, &source, &chosen_mods, None).map_err(|e| format!("{e:#}"))?,
        );
    }
    Ok(InstallResult {
        version,
        installed,
        mods: changes,
        game_running: crate::game::game_running(),
    })
}

/// What `set_mod_enabled` did.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModToggleResult {
    pub id: String,
    pub name: String,
    pub on: bool,
    /// The folders installed or removed.
    pub changes: Vec<ModChange>,
    /// Installs it couldn't go into yet because the addon isn't there (it goes in with the addon).
    pub waiting_for_addon: Vec<String>,
    pub game_running: bool,
}

/// Turn a mod on or off: kept in the settings, then each install it belongs in (as `install_addon`
/// picks them) gets its folders put in or taken out.
pub fn set_mod_enabled(app: &AppHandle, id: &str, on: bool) -> Result<ModToggleResult, String> {
    let m = core_mods::bundled()
        .iter()
        .find(|m| m.id == id)
        .ok_or_else(|| format!("There's no mod called {id}."))?;
    let agent = app.state::<Agent>();
    let settings = {
        let mut live = agent.live();
        live.settings.mods.insert(id.to_string(), on);
        live.settings.clone()
    };
    if let Some(dir) = &agent.settings_dir {
        settings
            .save(dir)
            .map_err(|e| format!("Couldn't save the setting: {e}"))?;
    }
    // An agent mod (Settings) has nothing in the game: the toggle is the whole of it.
    if core_mods::is_agent_mod(m) {
        return Ok(ModToggleResult {
            id: m.id.clone(),
            name: m.name.clone(),
            on,
            changes: Vec::new(),
            waiting_for_addon: Vec::new(),
            game_running: false,
        });
    }
    let found = discovery::discover(settings.install_override.as_deref());
    let picked =
        targets(&found.installs, settings.install_override.is_some(), None).unwrap_or_default();
    let mut changes = Vec::new();
    let mut waiting_for_addon = Vec::new();
    if !picked.is_empty() {
        let source = if on {
            Some(bundled_addon(app)?)
        } else {
            bundled_addon(app).ok()
        };
        for i in picked {
            if on && !addon_install::has_addon(i) {
                waiting_for_addon.push(display(&i.path));
                continue;
            }
            // Removing needs no source; a build without the addon still takes a mod out.
            let from = source.clone().unwrap_or_default();
            changes.extend(
                core_mods::apply(i, &from, &settings.mods, Some(id))
                    .map_err(|e| format!("{e:#}"))?,
            );
        }
    }
    Ok(ModToggleResult {
        id: m.id.clone(),
        name: m.name.clone(),
        on,
        changes,
        waiting_for_addon,
        game_running: crate::game::game_running(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ryna_agent::core::{CoreHeartbeat, CoreManifest};
    use ryna_wow_core::watch::WatchEvent;
    use ryna_wow_core::wow_core::activity;
    use serde_json::json;

    /// The app carries the core's addon and every bundled mod's addon folders beside it
    /// (`addon/<folder>`, where `mods::addon_source` looks), so turning a mod on can install it.
    /// Each is a directory key, never a glob: Tauri walks a directory and keeps its structure
    /// (`Locales/enUS.lua` lands at `addon/<folder>/Locales/enUS.lua`), while a glob in the map
    /// form puts every match straight under the target by file name, and `<folder>/*` skips the
    /// subfolders altogether (the 0.3.8 app shipped without `Locales/`).
    #[test]
    fn the_bundle_carries_every_bundled_mods_folders_whole() {
        let conf: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        let resources = conf["bundle"]["resources"].as_object().unwrap();
        for key in resources.keys() {
            assert!(
                !key.contains('*'),
                "{key}: a glob loses the addon's subfolders"
            );
        }
        let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
        let mut folders = vec![(
            "../../../cores/wow/addon/Ryna".to_string(),
            "Ryna".to_string(),
        )];
        for m in core_mods::bundled() {
            for folder in &m.addons {
                folders.push((
                    format!("../../../mods/wow/{}/addon/{folder}", m.id),
                    folder.clone(),
                ));
            }
        }
        for (from, folder) in folders {
            assert_eq!(
                resources.get(&from),
                Some(&json!(format!("addon/{folder}/"))),
                "tauri.conf.json's bundle.resources lacks {from}"
            );
            // Every file its TOC loads is under that directory, Locales\ included.
            let dir = manifest.join(&from);
            let toc = std::fs::read_to_string(dir.join(format!("{folder}.toc"))).unwrap();
            let files: Vec<_> = toc
                .lines()
                .map(str::trim)
                .filter(|l| !l.is_empty() && !l.starts_with('#'))
                .collect();
            assert!(
                files.iter().any(|f| f.starts_with("Locales")),
                "{folder}.toc loads its Locales"
            );
            for file in files {
                let rel = file.replace('\\', "/");
                assert!(
                    dir.join(&rel).is_file(),
                    "{folder}: the TOC loads {rel}, which isn't there"
                );
            }
        }
    }

    const A: &str = "/wow/_forever_/WTF/Account/A/SavedVariables/Ryna.lua";

    impl Live {
        fn observe_event(&mut self, event: &WatchEvent) {
            self.observe(&activity(event));
        }
    }

    fn failed(path: Option<&str>) -> WatchEvent {
        WatchEvent::error(None, path.map(str::to_string), "offline".into(), Fix::Other)
    }

    #[test]
    fn rows_follow_the_events_and_a_good_pass_clears_the_error() {
        let mut live = Live::default();
        live.observe_event(&WatchEvent::Pushing {
            label: "UNT1".into(),
            path: A.into(),
        });
        assert_eq!(live.phases[A], Phase::Pushing);
        live.observe_event(&failed(Some(A)));
        assert_eq!(live.phases[A], Phase::Error);
        assert_eq!(live.last_error.as_deref(), Some("offline"));
        live.observe_event(&WatchEvent::Pushed {
            label: "UNT1".into(),
            path: A.into(),
            characters: 2,
            result: json!({}),
        });
        assert_eq!(live.phases[A], Phase::Pushed);
        live.observe_event(&WatchEvent::PassFinished {
            ok: true,
            next_in_secs: 60,
        });
        assert_eq!(live.last_error, None);
        // The neutral words, not the error itself.
        live.observe_event(&WatchEvent::error(
            None,
            None,
            "the API is unreachable or failing after 3 tries: x".into(),
            Fix::Network,
        ));
        assert_eq!(live.last_error.as_deref(), Some("Couldn't reach the API"));
        // A fight that couldn't go up for the same reason says so too; a refusal doesn't.
        live.last_error = None;
        live.observe_event(&WatchEvent::FightFailed {
            key: "k".into(),
            fight: "Fights".into(),
            message: "refused".into(),
            fix: Fix::Other,
            advice: "refused".into(),
        });
        assert_eq!(live.last_error, None);
        live.observe_event(&WatchEvent::FightFailed {
            key: "k".into(),
            fight: "Fights".into(),
            message: "offline".into(),
            fix: Fix::Network,
            advice: "Couldn't reach the API".into(),
        });
        assert_eq!(live.last_error.as_deref(), Some("Couldn't reach the API"));
    }

    #[test]
    fn a_loop_counts_as_watching_until_its_control_is_cancelled() {
        let mut live = Live::default();
        assert!(!live.watching());
        let control = CoreControl::new();
        live.control = Some(control.clone());
        assert!(live.watching());
        control.sync();
        assert!(live.watching());
        control.stop();
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

    /// A core that runs one pass of events, or can't start at all.
    struct FakeCore {
        refuse: bool,
    }

    impl Core for FakeCore {
        type Event = String;
        type Error = std::io::Error;
        fn manifest(&self) -> &CoreManifest {
            ryna_wow_core::wow_core::manifest()
        }
        fn start(
            &self,
            _ctx: &CoreContext,
            control: &CoreControl,
            events: &mut dyn FnMut(String),
        ) -> Result<(), std::io::Error> {
            if self.refuse {
                return Err(std::io::Error::other("no disk"));
            }
            events("pushing".into());
            control.sync();
            events("passFinished".into());
            Ok(())
        }
        fn activity(&self, event: &String) -> Activity {
            Activity {
                clears_error: event == "passFinished",
                ..Activity::default()
            }
        }
        fn failed(&self, message: String, _fix: Fix) -> String {
            format!("failed: {message}")
        }
        fn status_rows(&self, _ctx: &CoreContext) -> Vec<StatusRow> {
            vec![StatusRow::new("Game", "fake", None)]
        }
        fn heartbeat(&self, _ctx: &CoreContext) -> CoreHeartbeat {
            CoreHeartbeat {
                core: "fake".into(),
                version: "0.0.0".into(),
                fields: json!({}),
            }
        }
    }

    fn fake_context() -> CoreContext {
        CoreContext {
            state_dir: PathBuf::from("/nowhere"),
            tokens: Arc::new(KeychainTokens::new(Config::resolve(
                Some("https://api.test".into()),
                Some("https://idp.test".into()),
            ))),
            device_key: None,
            game_folder: None,
        }
    }

    #[test]
    fn the_glue_drives_any_core_through_the_interface() {
        let ctx = fake_context();
        let control = CoreControl::new();
        let mut seen = Vec::new();
        drive(
            &FakeCore { refuse: false },
            Some(&ctx),
            &control,
            &mut |e| seen.push(e.clone()),
        );
        assert_eq!(seen, ["pushing", "passFinished"]);
        // Whatever ended the run, its control is stopped, so the app stops counting it as watching.
        assert!(control.is_stopped());

        let control = CoreControl::new();
        let mut seen = Vec::new();
        drive(&FakeCore { refuse: true }, Some(&ctx), &control, &mut |e| {
            seen.push(e.clone())
        });
        assert_eq!(seen, ["failed: no disk"]);
        assert!(control.is_stopped());

        let mut seen = Vec::new();
        drive(
            &FakeCore { refuse: false },
            None,
            &CoreControl::new(),
            &mut |e| seen.push(e.clone()),
        );
        assert_eq!(
            seen,
            ["failed: there's no folder to keep the agent's state in"]
        );

        let view = core_view(&FakeCore { refuse: false }, Some(&ctx));
        assert_eq!((view.id.as_str(), view.rows.len()), ("wow", 1));
        assert!(core_view(&FakeCore { refuse: false }, None).rows.is_empty());
    }
}
