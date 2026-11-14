//! The cores beside WoW (docs/ryna-agent-cores-mods.md 13): which ones the person is provisioned
//! for (the API's cores answer, asked at start, every hour and on Sync now, kept in the state
//! folder), fetching each one's package once it's allowed, and running it as a sidecar on its own
//! thread through the same `drive` as the WoW core. Every running core is driven at once.
//!
//! A core is in the window only when the answer names it and this build knows how to run it
//! ([`KNOWN`]); its name and whether it is allowed come from the answer, where it runs from here,
//! and everything else from the `core.json` inside its package.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::Duration;

use ryna_agent::auth::{Auth, KeyringStore};
use ryna_agent::cancel::CancelToken;
use ryna_agent::core::{
    runs_on, this_platform, Core, CoreContext, CoreControl, KeychainTokens, Phase,
};
use ryna_agent::http::HttpTransport;
use ryna_agent::package::{self, Source};
use ryna_agent::provisioned::{self, Asked, Cores, WOW_CORE_ID};
use ryna_agent::sidecar::{SidecarCore, SidecarEvent};
use ryna_agent::util::SystemClock;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::agent::{self, Agent};

/// `{ core, name, event }`: a sidecar core's `SidecarEvent`, for the window's activity log.
pub const CORE_EVENT: &str = "core-event";
/// `{ core, done, total, error }`: a package fetch's progress, or how it ended.
pub const CORE_FETCH: &str = "core-fetch";
/// How often the cores answer is asked again while the app is open.
const HOURLY: Duration = Duration::from_secs(60 * 60);
/// The updater's public key: a package is trusted exactly as far as an app update is.
const UPDATER_PUB: &str = include_str!("../../updater.pub");

/// A fetched core this build knows how to run: its id, and where it runs (its manifest's
/// `platforms`, known here so a page can say "not on this computer yet" before anything is
/// fetched). A provisioned core that isn't here is ignored, not shown.
pub struct Known {
    pub id: &'static str,
    pub platforms: &'static [&'static str],
}

pub const KNOWN: &[Known] = &[Known {
    id: "palworld",
    platforms: &["macos"],
}];

pub fn known(id: &str) -> Option<&'static Known> {
    KNOWN.iter().find(|k| k.id == id)
}

/// What a core's page shows about it (`{"is": "fetching", ...}`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "is", rename_all = "camelCase")]
pub enum CoreState {
    /// Running (for WoW: the watch loop is on).
    Running,
    /// Not running; `message` is its last words when it stopped by itself.
    #[serde(rename_all = "camelCase")]
    Stopped {
        message: Option<String>,
        fix: Option<String>,
    },
    /// Turned off by the person.
    Off,
    /// Provisioned, but the sign-in doesn't cover it yet: "Allow <name> on this computer".
    Allow,
    /// "Getting <name>…".
    Fetching { done: u64, total: Option<u64> },
    /// The fetch failed: the words, and Retry.
    FetchFailed { message: String },
    /// It doesn't run on this operating system: where it does.
    NotHere { platforms: Vec<String> },
    /// Signed out: nothing runs until the sign-in.
    SignedOut,
}

#[derive(Debug, Clone, Default)]
enum Fetch {
    #[default]
    Idle,
    Running {
        done: u64,
        total: Option<u64>,
    },
    Failed(String),
}

/// One fetched core's live state.
#[derive(Default)]
struct Side {
    control: Option<CoreControl>,
    worker: Option<JoinHandle<()>>,
    core: Option<Arc<SidecarCore>>,
    fetch: Fetch,
    /// Its last words when it stopped by itself (and what fixes them, when it said).
    stopped: Option<(String, Option<String>)>,
    last_error: Option<String>,
    phases: BTreeMap<String, Phase>,
    /// Other versions of its package are removed once, after its first good pass.
    pruned: bool,
}

impl Side {
    fn running(&self) -> bool {
        self.control.as_ref().is_some_and(|c| !c.is_stopped())
    }
}

/// The app's managed state for the cores beside WoW.
#[derive(Default)]
pub struct Sidecars {
    answer: Option<Cores>,
    sides: BTreeMap<String, Side>,
    hourly: Option<CancelToken>,
}

fn sidecars(app: &AppHandle) -> MutexGuard<'_, Sidecars> {
    app.state::<Mutex<Sidecars>>()
        .inner()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

/// The cores to show: the kept answer when signed in (WoW alone before any), WoW alone when
/// signed out (as before stage 4), and only the ones this build knows how to run.
pub fn shown(answer: Option<&Cores>, signed_in: bool) -> Cores {
    let mut cores = match (signed_in, answer) {
        (true, Some(a)) => a.clone(),
        _ => Cores::wow_alone(),
    };
    cores
        .cores
        .retain(|c| c.id == WOW_CORE_ID || known(&c.id).is_some());
    cores
}

fn answer(app: &AppHandle) -> Cores {
    let signed_in = agent::signed_in(&app.state::<Agent>().config);
    let mut s = sidecars(app);
    if s.answer.is_none() {
        s.answer = Some(provisioned::current(
            ryna_agent::state_dir::state_dir().as_deref(),
        ));
    }
    shown(s.answer.as_ref(), signed_in)
}

/// What a sign-in (the header's, or a core's Allow) asks for: the answer's `signIn`, else the
/// config's resource with every provisioned core's scope (WoW's alone before any answer).
pub fn sign_in_request(app: &AppHandle) -> (String, Vec<String>) {
    let config = app.state::<Agent>().config.clone();
    let kept = {
        let mut s = sidecars(app);
        if s.answer.is_none() {
            s.answer = Some(provisioned::current(
                ryna_agent::state_dir::state_dir().as_deref(),
            ));
        }
        s.answer.clone().unwrap_or_else(Cores::wow_alone)
    };
    kept.sign_in(&config)
}

/// Whether the WoW core is one the person has (always, until an answer leaves it out).
pub fn wow_provisioned(app: &AppHandle) -> bool {
    answer(app).get(WOW_CORE_ID).is_some()
}

/// At start: ask now, then every hour until quit.
pub fn begin(app: &AppHandle) {
    let token = CancelToken::new();
    sidecars(app).hourly = Some(token.clone());
    let app = app.clone();
    std::thread::spawn(move || loop {
        check_now(&app);
        if token.wait(HOURLY) {
            return;
        }
    });
}

/// Ask the API again, off the caller's thread (Sync now, a sign-in that finished).
pub fn check(app: &AppHandle) {
    let app = app.clone();
    std::thread::spawn(move || check_now(&app));
}

fn check_now(app: &AppHandle) {
    let config = app.state::<Agent>().config.clone();
    let state = ryna_agent::state_dir::state_dir();
    let asked = match HttpTransport::new() {
        Err(e) => Asked::NoAnswer(e),
        Ok(http) => {
            let store = KeyringStore::for_issuer(&config.issuer);
            match Auth::new(&config, &http, &store, &SystemClock).access_token() {
                Ok(Some(token)) => {
                    provisioned::ask(&config, &http, &token, ryna_agent::util::now_secs())
                }
                Ok(None) => Asked::NoAnswer("not signed in".into()),
                Err(e) if e.needs_login() => Asked::SignInAgain(e.to_string()),
                Err(e) => Asked::NoAnswer(e.to_string()),
            }
        }
    };
    let (cores, failed) = provisioned::update(state.as_deref(), asked);
    if let Some(Asked::SignInAgain(why)) = &failed {
        eprintln!("ryna: the cores answer refused the sign-in: {why}");
        agent::note_error(app, ryna_agent::fix::Fix::SignIn.message(why));
    }
    {
        let mut s = sidecars(app);
        s.answer = Some(cores);
        // The hourly check tries a failed fetch again.
        for side in s.sides.values_mut() {
            if matches!(side.fetch, Fetch::Failed(_)) {
                side.fetch = Fetch::Idle;
            }
        }
    }
    reconcile(app);
    let _ = app.emit(CORE_FETCH, serde_json::json!({ "core": null }));
}

/// Bring every fetched core in line with the answer, the sign-in and the person's switches:
/// fetch what's allowed and missing, start what's ready and on, stop the rest.
pub fn reconcile(app: &AppHandle) {
    let agent = app.state::<Agent>();
    let signed_in = agent::signed_in(&agent.config);
    let settings = agent::settings(app);
    let cores = answer(app);
    if cores.get(WOW_CORE_ID).is_none() {
        // An answer that leaves WoW out removes it too.
        agent::cancel(app);
    }
    let state = ryna_agent::state_dir::state_dir();
    let mut to_start = Vec::new();
    let mut to_fetch = Vec::new();
    {
        let mut s = sidecars(app);
        let wanted: Vec<String> = cores
            .cores
            .iter()
            .filter(|c| c.id != WOW_CORE_ID)
            .map(|c| c.id.clone())
            .collect();
        for (id, side) in s.sides.iter_mut() {
            if !wanted.contains(id) {
                if let Some(c) = &side.control {
                    c.stop();
                }
            }
        }
        s.sides
            .retain(|id, side| wanted.contains(id) || side.running());
        for c in cores.cores.iter().filter(|c| c.id != WOW_CORE_ID) {
            let Some(k) = known(&c.id) else { continue };
            let side = s.sides.entry(c.id.clone()).or_default();
            let platforms: Vec<String> = k.platforms.iter().map(|p| p.to_string()).collect();
            let runnable = signed_in
                && c.allowed
                && runs_on(Some(&platforms), this_platform())
                && settings.core_on(&c.id);
            if !runnable {
                if let Some(ctl) = &side.control {
                    ctl.stop();
                }
            }
            // A package is fetched only for a core that is provisioned, allowed, runs here and is on.
            if !runnable {
                continue;
            }
            let ready = state
                .as_deref()
                .and_then(|d| package::installed(d, &c.id, env!("CARGO_PKG_VERSION")));
            match ready {
                None if matches!(side.fetch, Fetch::Idle) => {
                    side.fetch = Fetch::Running {
                        done: 0,
                        total: None,
                    };
                    to_fetch.push(c.id.clone());
                }
                None => {}
                Some(_) if !side.running() => to_start.push(c.id.clone()),
                Some(_) => {}
            }
        }
    }
    for id in to_fetch {
        fetch(app, &id);
    }
    for id in to_start {
        start(app, &id);
    }
}

fn fetch(app: &AppHandle, id: &str) {
    let app = app.clone();
    let id = id.to_string();
    std::thread::spawn(move || {
        let version = env!("CARGO_PKG_VERSION");
        let result = ryna_agent::state_dir::state_dir()
            .ok_or_else(|| "there's no folder to keep the agent's state in".to_string())
            .and_then(|state| {
                let source = Source::for_version(version).map_err(|e| e.to_string())?;
                let key = package::public_key(UPDATER_PUB);
                let mut last = 0u64;
                package::fetch(&state, &id, version, &source, &key, &mut |done, total| {
                    // A line for the window every 256 KB.
                    if done - last >= 256 * 1024 || Some(done) == total {
                        last = done;
                        if let Some(side) = sidecars(&app).sides.get_mut(&id) {
                            side.fetch = Fetch::Running { done, total };
                        }
                        let _ = app.emit(
                            CORE_FETCH,
                            serde_json::json!({ "core": id, "done": done, "total": total }),
                        );
                    }
                })
                .map_err(|e| e.to_string())
            });
        let error = result.as_ref().err().cloned();
        if let Some(side) = sidecars(&app).sides.get_mut(&id) {
            side.fetch = match &error {
                Some(e) => Fetch::Failed(e.clone()),
                None => Fetch::Idle,
            };
        }
        let _ = app.emit(
            CORE_FETCH,
            serde_json::json!({ "core": id, "finished": true, "error": error }),
        );
        if error.is_none() {
            reconcile(&app);
        }
    });
}

/// `Retry` on a failed fetch.
pub fn retry_fetch(app: &AppHandle, id: &str) {
    if let Some(side) = sidecars(app).sides.get_mut(id) {
        if matches!(side.fetch, Fetch::Failed(_)) {
            side.fetch = Fetch::Idle;
        }
    }
    reconcile(app);
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct CoreEvent<'a> {
    core: &'a str,
    name: &'a str,
    event: &'a SidecarEvent,
}

fn start(app: &AppHandle, id: &str) {
    let config = app.state::<Agent>().config.clone();
    let Some(state) = ryna_agent::state_dir::state_dir() else {
        return;
    };
    let version = env!("CARGO_PKG_VERSION");
    let Some((dir, manifest)) = package::installed(&state, id, version) else {
        return;
    };
    let name = answer(app)
        .get(id)
        .map(|c| c.name.clone())
        .unwrap_or_else(|| manifest.name.clone());
    let core = Arc::new(SidecarCore::new(manifest, dir, &config.api));
    let control = CoreControl::new();
    let previous = {
        let mut s = sidecars(app);
        let side = s.sides.entry(id.to_string()).or_default();
        if side.running() {
            return;
        }
        side.control = Some(control.clone());
        side.core = Some(core.clone());
        side.stopped = None;
        side.worker.take()
    };
    let ctx = CoreContext {
        state_dir: package::core_dir(&state, id),
        tokens: Arc::new(KeychainTokens::new(config)),
        device_key: None,
        game_folder: None,
    };
    let (handle, core_id) = (app.clone(), id.to_string());
    let worker = std::thread::spawn(move || {
        let (app, id) = (handle, core_id);
        if let Some(previous) = previous {
            let _ = previous.join();
        }
        agent::drive(core.as_ref(), Some(&ctx), &control, &mut |event| {
            let activity = core.activity(event);
            let prune = {
                let mut s = sidecars(&app);
                let side = s.sides.entry(id.clone()).or_default();
                agent::apply(&mut side.phases, &mut side.last_error, &activity);
                if let SidecarEvent::Stopped { message, fix } = event {
                    side.stopped = Some((message.clone(), fix.clone()));
                }
                let first_good =
                    matches!(event, SidecarEvent::PassFinished { ok: true }) && !side.pruned;
                side.pruned |= first_good;
                first_good
            };
            if prune {
                if let Some(state) = ryna_agent::state_dir::state_dir() {
                    if let Err(e) = package::prune_other_versions(&state, &id, version) {
                        eprintln!("ryna: couldn't remove {id}'s older packages: {e}");
                    }
                }
            }
            let _ = app.emit(
                CORE_EVENT,
                CoreEvent {
                    core: &id,
                    name: &name,
                    event,
                },
            );
        });
        let _ = app.emit(CORE_FETCH, serde_json::json!({ "core": id }));
    });
    if let Some(side) = sidecars(app).sides.get_mut(id) {
        side.worker = Some(worker);
    }
}

/// Sync now: every running fetched core gets a pass now, and the API is asked again.
pub fn sync_now(app: &AppHandle) {
    for side in sidecars(app).sides.values() {
        if let Some(c) = side.control.as_ref().filter(|c| !c.is_stopped()) {
            c.sync();
        }
    }
    check(app);
}

/// Turn a fetched core on or off (kept in the settings).
pub fn set_on(app: &AppHandle, id: &str, on: bool) -> Result<(), String> {
    agent::update_settings(app, |s| {
        s.cores.insert(id.to_string(), on);
    })?;
    if on {
        // Its last stop is forgotten: turning it on starts it again.
        if let Some(side) = sidecars(app).sides.get_mut(id) {
            side.stopped = None;
        }
    }
    reconcile(app);
    Ok(())
}

/// Stop every fetched core (a sign-out); quitting stops the hourly check too.
pub fn stop_all(app: &AppHandle) {
    for side in sidecars(app).sides.values() {
        if let Some(c) = &side.control {
            c.stop();
        }
    }
}

pub fn shutdown(app: &AppHandle) {
    if let Some(t) = sidecars(app).hourly.take() {
        t.cancel();
    }
    stop_all(app);
}

/// What the window shows of one fetched core.
pub struct SideView {
    pub id: String,
    pub name: String,
    pub version: String,
    pub platforms: Vec<String>,
    pub state: CoreState,
    pub on: bool,
    pub last_error: Option<String>,
    pub rows: Vec<ryna_agent::core::StatusRow>,
}

/// What decides a fetched core's page.
pub struct Facts<'a> {
    pub signed_in: bool,
    pub allowed: bool,
    pub runs_here: bool,
    pub platforms: &'a [String],
    pub on: bool,
    pub fetching: Option<(u64, Option<u64>)>,
    pub fetch_failed: Option<&'a str>,
    pub running: bool,
    pub stopped: Option<&'a (String, Option<String>)>,
}

impl Facts<'_> {
    /// The state its page shows, most pressing first.
    pub fn state(&self) -> CoreState {
        if !self.runs_here {
            return CoreState::NotHere {
                platforms: self.platforms.to_vec(),
            };
        }
        if !self.signed_in {
            return CoreState::SignedOut;
        }
        if !self.allowed {
            return CoreState::Allow;
        }
        if !self.on {
            return CoreState::Off;
        }
        if let Some(message) = self.fetch_failed {
            return CoreState::FetchFailed {
                message: message.to_string(),
            };
        }
        if let Some((done, total)) = self.fetching {
            return CoreState::Fetching { done, total };
        }
        if self.running {
            return CoreState::Running;
        }
        CoreState::Stopped {
            message: self.stopped.map(|s| s.0.clone()),
            fix: self.stopped.and_then(|s| s.1.clone()),
        }
    }
}

/// Every provisioned fetched core this build knows, in the answer's order.
pub fn views(app: &AppHandle) -> Vec<SideView> {
    let signed_in = agent::signed_in(&app.state::<Agent>().config);
    let settings = agent::settings(app);
    let cores = answer(app);
    let s = sidecars(app);
    cores
        .cores
        .iter()
        .filter(|c| c.id != WOW_CORE_ID)
        .filter_map(|c| {
            let k = known(&c.id)?;
            let platforms: Vec<String> = k.platforms.iter().map(|p| p.to_string()).collect();
            let side = s.sides.get(&c.id);
            let (fetching, failed) = match side.map(|s| &s.fetch) {
                Some(Fetch::Running { done, total }) => (Some((*done, *total)), None),
                Some(Fetch::Failed(m)) => (None, Some(m.as_str())),
                _ => (None, None),
            };
            let on = settings.core_on(&c.id);
            let state = Facts {
                signed_in,
                allowed: c.allowed,
                runs_here: runs_on(Some(&platforms), this_platform()),
                platforms: &platforms,
                on,
                fetching,
                fetch_failed: failed,
                running: side.is_some_and(Side::running),
                stopped: side.and_then(|s| s.stopped.as_ref()),
            }
            .state();
            Some(SideView {
                id: c.id.clone(),
                name: c.name.clone(),
                version: env!("CARGO_PKG_VERSION").to_string(),
                platforms,
                state,
                on,
                last_error: side.and_then(|s| s.last_error.clone()),
                rows: side
                    .and_then(|s| s.core.as_ref())
                    .map(|c| c.last_rows())
                    .unwrap_or_default(),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ryna_agent::provisioned::ProvisionedCore;

    fn core(id: &str, allowed: bool) -> ProvisionedCore {
        ProvisionedCore {
            id: id.into(),
            name: id.into(),
            scope: format!("{id}:ingest"),
            allowed,
        }
    }

    #[test]
    fn only_provisioned_cores_this_build_knows_are_shown() {
        let answer = Cores {
            cores: vec![
                core("wow", true),
                core("palworld", false),
                core("satisfactory", true),
            ],
            sign_in: None,
            at: 1,
        };
        let ids = |c: Cores| c.cores.into_iter().map(|c| c.id).collect::<Vec<_>>();
        assert_eq!(ids(shown(Some(&answer), true)), ["wow", "palworld"]);
        // Signed out: WoW alone, whatever was kept.
        assert_eq!(ids(shown(Some(&answer), false)), ["wow"]);
        // Nothing kept: WoW alone.
        assert_eq!(ids(shown(None, true)), ["wow"]);
        // An answer without Palworld: no Palworld anywhere.
        let without = Cores {
            cores: vec![core("wow", true)],
            ..answer
        };
        assert_eq!(ids(shown(Some(&without), true)), ["wow"]);
    }

    #[test]
    fn a_pages_state_is_the_most_pressing_one() {
        let mac = vec!["macos".to_string()];
        let stopped = ("this core needs a newer Ryna".to_string(), None);
        let ok = Facts {
            signed_in: true,
            allowed: true,
            runs_here: true,
            platforms: &mac,
            on: true,
            fetching: None,
            fetch_failed: None,
            running: true,
            stopped: None,
        };
        assert_eq!(ok.state(), CoreState::Running);
        assert_eq!(
            Facts {
                runs_here: false,
                signed_in: false,
                allowed: false,
                ..ok
            }
            .state(),
            CoreState::NotHere {
                platforms: mac.clone()
            }
        );
        assert_eq!(
            Facts {
                signed_in: false,
                ..ok
            }
            .state(),
            CoreState::SignedOut
        );
        assert_eq!(
            Facts {
                allowed: false,
                on: false,
                ..ok
            }
            .state(),
            CoreState::Allow
        );
        assert_eq!(Facts { on: false, ..ok }.state(), CoreState::Off);
        assert_eq!(
            Facts {
                fetch_failed: Some("offline"),
                fetching: Some((1, None)),
                ..ok
            }
            .state(),
            CoreState::FetchFailed {
                message: "offline".into()
            }
        );
        assert_eq!(
            Facts {
                fetching: Some((10, Some(100))),
                running: false,
                ..ok
            }
            .state(),
            CoreState::Fetching {
                done: 10,
                total: Some(100)
            }
        );
        assert_eq!(
            Facts {
                running: false,
                stopped: Some(&stopped),
                ..ok
            }
            .state(),
            CoreState::Stopped {
                message: Some("this core needs a newer Ryna".into()),
                fix: None
            }
        );
        assert_eq!(
            serde_json::to_value(CoreState::Fetching {
                done: 1,
                total: None
            })
            .unwrap(),
            serde_json::json!({ "is": "fetching", "done": 1, "total": null })
        );
    }

    #[test]
    fn palworld_is_known_and_runs_on_macos_only() {
        let k = known("palworld").unwrap();
        assert_eq!(k.platforms, ["macos"]);
        assert!(known("satisfactory").is_none());
        assert!(known(WOW_CORE_ID).is_none());
    }
}
