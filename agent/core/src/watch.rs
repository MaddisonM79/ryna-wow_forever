//! The watch loop as a library call: what `ryna-wow watch` runs, and what the desktop app runs
//! in-process while it is open (nothing syncs when it's closed; there is no background service).
//!
//! ```no_run
//! use std::time::Duration;
//! use ryna_wow_core::{config::Config, watch::{watch, CancelToken, WatchOptions}};
//!
//! let cancel = CancelToken::new();
//! let stop = cancel.clone();
//! let (tx, rx) = std::sync::mpsc::channel();
//! let worker = std::thread::spawn(move || {
//!     let config = Config::resolve(None, None);
//!     let opts = WatchOptions::new(None, Duration::from_secs(60)).expect("a state folder");
//!     watch(&config, &opts, &stop, &mut |event| { let _ = tx.send(event); })
//! });
//! for event in rx.iter().take(3) {
//!     println!("{event:?}"); // a tray app updates its status from these
//! }
//! cancel.cancel(); // on quit: the loop stops within the current run, or at once while waiting
//! worker.join().unwrap().unwrap();
//! ```
//!
//! An owner that wants a pass now, without restarting the loop, runs it with a [`WatchControl`]
//! (`watch_with_control`) and calls `request_pass()`: the wait between passes ends at once, or,
//! when a pass is running, the next one starts as soon as it finishes.
//!
//! Every event is `Serialize` (`{"event": "pushed", ...}`), so an app can forward it to its
//! webview unchanged.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde::Serialize;

use crate::auth::{Auth, KeyringStore};
use crate::cancel::Waited;
use crate::combat::tail::{self, Tailer};
use crate::combat::CombatOptions;
use crate::config::Config;
use crate::discovery::{self, Discovery};
use crate::fix::Fix;
use crate::http::{HttpTransport, Transport};
use crate::push::Api;
use crate::runner::{run_once_observed, RunError, RunOptions};
use crate::state;
use crate::util::{Clock, SystemClock};

/// What the loop is doing, for a status display.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "event", rename_all = "camelCase")]
pub enum WatchEvent {
    /// At the start of each pass: whether a sign-in is kept on this computer.
    #[serde(rename_all = "camelCase")]
    SignedIn {
        signed_in: bool,
    },
    /// An install seen for the first time since the loop started.
    #[serde(rename_all = "camelCase")]
    InstallFound {
        path: String,
        flavor: Option<String>,
    },
    /// An account's `Ryna.lua` was written since its last push.
    #[serde(rename_all = "camelCase")]
    AccountFileChanged {
        label: String,
        path: String,
        saved_at: i64,
    },
    #[serde(rename_all = "camelCase")]
    Pushing {
        label: String,
        path: String,
    },
    /// The API took it; `result` is its answer's `data` (snapshotId, account, journal counts, sightings).
    #[serde(rename_all = "camelCase")]
    Pushed {
        label: String,
        path: String,
        characters: usize,
        result: serde_json::Value,
    },
    /// An account (or, with no label, the whole pass) failed. `message` is the error itself;
    /// `fix` says what fixes it and `advice` is the words to show for it (`Fix::message`).
    #[serde(rename_all = "camelCase")]
    Error {
        label: Option<String>,
        path: Option<String>,
        message: String,
        fix: Fix,
        advice: String,
    },
    Heartbeat {
        ok: bool,
        error: Option<String>,
    },
    /// A pass ended; the next starts after the interval.
    #[serde(rename_all = "camelCase")]
    PassFinished {
        ok: bool,
        next_in_secs: u64,
    },
    /// A combat log seen for the first time since the loop started.
    #[serde(rename_all = "camelCase")]
    CombatLogFound {
        file: String,
        path: String,
    },
    /// A fight ended and is queued to upload. `kind` is boss, trash or world; `players` the
    /// players in it.
    #[serde(rename_all = "camelCase")]
    FightQueued {
        key: String,
        kind: String,
        zone: Option<String>,
        name: Option<String>,
        duration_ms: i64,
        players: usize,
    },
    /// A fight is up (its summary and, when on, its digest and raw lines). `id` is the API's
    /// encounter; `merged` says it joined another logger's copy of the same fight.
    #[serde(rename_all = "camelCase")]
    FightUploaded {
        key: String,
        kind: String,
        zone: Option<String>,
        name: Option<String>,
        duration_ms: i64,
        players: usize,
        id: serde_json::Value,
        merged: bool,
    },
    /// A fight's upload failed (it stays queued, unless it was refused too often). `fight` names
    /// it ("Zephras Isle, 3 min, 2 players", or "Fights" for the queue as a whole).
    #[serde(rename_all = "camelCase")]
    FightFailed {
        key: String,
        fight: String,
        message: String,
        fix: Fix,
        advice: String,
    },
}

pub use crate::cancel::CancelToken;

impl WatchEvent {
    /// An `Error`, with `advice` from its fix.
    pub fn error(label: Option<String>, path: Option<String>, message: String, fix: Fix) -> Self {
        WatchEvent::Error {
            label,
            path,
            advice: fix.message(&message),
            message,
            fix,
        }
    }
}

/// The owner's handle on a running loop: ask for a pass now, or stop it. Cloning shares it.
#[derive(Debug, Clone, Default)]
pub struct WatchControl {
    token: CancelToken,
}

impl WatchControl {
    pub fn new() -> Self {
        Self::default()
    }

    /// Start a pass now: the loop's wait ends at once; a pass in progress is followed straight
    /// away by another. Requests before the next wait count as one.
    pub fn request_pass(&self) {
        self.token.wake();
    }

    /// Stop the loop: at once while it waits, or at the end of the pass it's in.
    pub fn cancel(&self) {
        self.token.cancel();
    }

    pub fn is_cancelled(&self) -> bool {
        self.token.is_cancelled()
    }

    /// The token under it (`watch` and `watch_with` take one).
    pub fn token(&self) -> &CancelToken {
        &self.token
    }
}

impl From<CancelToken> for WatchControl {
    fn from(token: CancelToken) -> Self {
        WatchControl { token }
    }
}

/// How often the combat logs are read between snapshot passes.
pub const TAIL_INTERVAL: Duration = Duration::from_secs(2);

#[derive(Debug, Clone)]
pub struct WatchOptions {
    /// `--install`: the game folder or a product folder, else the OS defaults.
    pub install: Option<PathBuf>,
    pub state_dir: PathBuf,
    pub interval: Duration,
    /// What to do with the combat logs (read them, send digests, send raw lines).
    pub combat: CombatOptions,
}

impl WatchOptions {
    /// With the OS state folder (`state::state_dir`) and the default combat options; None when
    /// there's no state folder.
    pub fn new(install: Option<PathBuf>, interval: Duration) -> Option<Self> {
        Some(WatchOptions {
            install,
            state_dir: state::state_dir()?,
            interval,
            combat: CombatOptions::default(),
        })
    }
}

/// Run passes every `opts.interval` until `cancel` fires, with the real network and keychain.
/// Returns only on cancel, or when the state folder can't be used.
pub fn watch(
    config: &Config,
    opts: &WatchOptions,
    cancel: &CancelToken,
    on_event: &mut dyn FnMut(WatchEvent),
) -> Result<(), RunError> {
    let http = HttpTransport::new().map_err(|e| RunError::Io(std::io::Error::other(e)))?;
    let store = KeyringStore::for_issuer(&config.issuer);
    watch_with(
        config,
        &http,
        &store,
        &SystemClock,
        opts,
        cancel,
        &discovery::discover,
        on_event,
    )
}

/// `watch`, driven by a `WatchControl` the owner keeps a clone of (for `request_pass`).
pub fn watch_with_control(
    config: &Config,
    opts: &WatchOptions,
    control: &WatchControl,
    on_event: &mut dyn FnMut(WatchEvent),
) -> Result<(), RunError> {
    watch(config, opts, control.token(), on_event)
}

/// `watch` over any transport, token store and discovery (tests; another front end). A
/// `cancel.wake()` (`WatchControl::request_pass`) ends the wait between passes early.
#[allow(clippy::too_many_arguments)]
pub fn watch_with(
    config: &Config,
    http: &dyn Transport,
    store: &dyn crate::auth::TokenStore,
    clock: &dyn Clock,
    opts: &WatchOptions,
    cancel: &CancelToken,
    discover: &dyn Fn(Option<&Path>) -> Discovery,
    on_event: &mut dyn FnMut(WatchEvent),
) -> Result<(), RunError> {
    let auth = Auth::new(config, http, store, clock);
    let api = Api::new(&config.api, http, &auth, clock);
    let mut known: Vec<PathBuf> = Vec::new();
    let mut tailer = Tailer::new(&opts.state_dir, opts.combat);
    // One tailer per state folder: a second loop (the CLI beside the app) leaves the logs alone.
    let mut tail_lock = None;
    let mut found = Discovery::default();
    while !cancel.is_cancelled() {
        on_event(WatchEvent::SignedIn {
            signed_in: auth.signed_in().unwrap_or(false),
        });
        // Rediscovered every pass, so a new install or account folder is picked up.
        found = discover(opts.install.as_deref());
        for i in &found.installs {
            if !known.contains(&i.path) {
                known.push(i.path.clone());
                on_event(WatchEvent::InstallFound {
                    path: i.path.to_string_lossy().into_owned(),
                    flavor: i.flavor.clone(),
                });
            }
        }
        let ok = match run_once_observed(
            &found,
            &opts.state_dir,
            &api,
            clock,
            &RunOptions::default(),
            on_event,
        ) {
            Ok(report) => report.ok(),
            Err(RunError::Busy) => {
                on_event(WatchEvent::error(
                    None,
                    None,
                    RunError::Busy.to_string(),
                    Fix::Other,
                ));
                false
            }
            Err(e) => return Err(e),
        };
        on_event(WatchEvent::PassFinished {
            ok,
            next_in_secs: opts.interval.as_secs(),
        });
        // The wait until the next pass, read in short ticks for the combat logs.
        let deadline = Instant::now() + opts.interval;
        let mut stop = false;
        loop {
            if opts.combat.enabled {
                if tail_lock.is_none() {
                    tail_lock =
                        state::try_lock_named(&opts.state_dir.join("combat"), tail::LOCK_FILE)
                            .ok()
                            .flatten();
                }
                if tail_lock.is_some() {
                    let signed_in = auth.signed_in().unwrap_or(false);
                    tailer.poll(&found, signed_in.then_some(&api), clock.now(), on_event);
                }
            }
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                break;
            }
            match cancel.wait_for(left.min(TAIL_INTERVAL)) {
                Waited::Cancelled => {
                    stop = true;
                    break;
                }
                Waited::Woken => break,
                Waited::TimedOut => {}
            }
        }
        if stop {
            break;
        }
    }
    // On quit: what's open is ended and queued, so the next run uploads it.
    if tail_lock.is_some() {
        tailer.flush(&found, clock.now(), on_event);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::testing::MemoryStore;
    use crate::config::test_config;
    use crate::discovery::testing::product;
    use crate::http::testing::ScriptedTransport;
    use crate::util::testing::FakeClock;
    use serde_json::json;
    use std::time::Instant;

    const NOW: i64 = 1_800_000_000;

    #[test]
    fn a_requested_pass_runs_without_waiting_out_the_interval() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("World of Warcraft");
        product(&root, "_forever_", "wowf", &[]);
        let http = ScriptedTransport::new();
        http.json(200, json!({ "ok": true, "data": {} }))
            .json(200, json!({ "ok": true, "data": {} }));
        let store = MemoryStore::with_access("tok", NOW + 3600);
        let clock = FakeClock::at(NOW);
        let opts = WatchOptions {
            install: Some(root),
            state_dir: dir.path().join("state"),
            // Far longer than the test may take: only a requested pass ends the first wait.
            interval: Duration::from_secs(600),
            combat: CombatOptions::default(),
        };
        let control = WatchControl::new();
        let mut passes = 0;
        let start = Instant::now();
        watch_with(
            &test_config(),
            &http,
            &store,
            &clock,
            &opts,
            control.token(),
            &discovery::discover,
            &mut |e| {
                if matches!(e, WatchEvent::PassFinished { .. }) {
                    passes += 1;
                    if passes == 1 {
                        control.request_pass();
                    } else {
                        control.cancel();
                    }
                }
            },
        )
        .unwrap();
        assert_eq!(passes, 2);
        assert!(start.elapsed() < Duration::from_secs(60));
        assert!(control.is_cancelled());
        assert_eq!(http.remaining(), 0);
    }

    #[test]
    fn a_pass_reports_what_it_does_and_stops_on_cancel() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("World of Warcraft");
        product(
            &root,
            "_forever_",
            "wowf",
            &[("FAKEACCOUNT1", include_str!("../../fixtures/Ryna.lua"))],
        );
        let http = ScriptedTransport::new();
        http.json(200, json!({ "ok": true, "data": { "snapshotId": 1, "journal": { "stored": 21, "repeated": 0 } } }))
            .json(200, json!({ "ok": true, "data": {} }))
            .json(200, json!({ "ok": true, "data": {} }));
        let store = MemoryStore::with_access("tok", NOW + 3600);
        let clock = FakeClock::at(NOW);
        let opts = WatchOptions {
            install: Some(root.clone()),
            state_dir: dir.path().join("state"),
            interval: Duration::from_millis(10),
            combat: CombatOptions::default(),
        };
        let cancel = CancelToken::new();
        let mut events = Vec::new();
        let mut passes = 0;
        watch_with(
            &test_config(),
            &http,
            &store,
            &clock,
            &opts,
            &cancel,
            &discovery::discover,
            &mut |e| {
                if matches!(e, WatchEvent::PassFinished { .. }) {
                    passes += 1;
                    if passes == 2 {
                        cancel.cancel();
                    }
                }
                events.push(e);
            },
        )
        .unwrap();

        let kinds: Vec<String> = events
            .iter()
            .map(|e| {
                serde_json::to_value(e).unwrap()["event"]
                    .as_str()
                    .unwrap()
                    .to_string()
            })
            .collect();
        assert_eq!(
            kinds,
            vec![
                "signedIn",
                "installFound",
                "accountFileChanged",
                "pushing",
                "pushed",
                "heartbeat",
                "passFinished",
                // The second pass: nothing changed, the install is already known.
                "signedIn",
                "heartbeat",
                "passFinished",
            ]
        );
        let pushed = serde_json::to_value(&events[4]).unwrap();
        assert_eq!(pushed["label"], json!("UNT1"));
        assert_eq!(pushed["characters"], json!(2));
        assert_eq!(pushed["result"]["journal"]["stored"], json!(21));
        assert_eq!(
            serde_json::to_value(&events[0]).unwrap(),
            json!({ "event": "signedIn", "signedIn": true })
        );
    }

    #[test]
    fn errors_carry_what_fixes_them() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("World of Warcraft");
        product(
            &root,
            "_forever_",
            "wowf",
            &[("FAKEACCOUNT1", include_str!("../../fixtures/Ryna.lua"))],
        );
        let http = ScriptedTransport::new();
        let store = MemoryStore::default();
        let clock = FakeClock::at(NOW);
        let opts = WatchOptions {
            install: Some(root),
            state_dir: dir.path().join("state"),
            interval: Duration::from_millis(1),
            combat: CombatOptions::default(),
        };
        let cancel = CancelToken::new();
        let mut events = Vec::new();
        watch_with(
            &test_config(),
            &http,
            &store,
            &clock,
            &opts,
            &cancel,
            &discovery::discover,
            &mut |e| {
                if matches!(e, WatchEvent::PassFinished { .. }) {
                    cancel.cancel();
                }
                events.push(e);
            },
        )
        .unwrap();
        assert_eq!(events[0], WatchEvent::SignedIn { signed_in: false });
        let err = events
            .iter()
            .find(|e| matches!(e, WatchEvent::Error { .. }))
            .unwrap();
        let v = serde_json::to_value(err).unwrap();
        assert_eq!(v["fix"], json!("signIn"));
        assert_eq!(v["advice"], json!("Sign in again"));
        assert!(!v["message"].as_str().unwrap().contains("ryna-wow"), "{v}");
    }
}
