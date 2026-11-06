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
//! Every event is `Serialize` (`{"event": "pushed", ...}`), so an app can forward it to its
//! webview unchanged.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use serde::Serialize;

use crate::auth::{Auth, KeyringStore};
use crate::config::Config;
use crate::discovery::{self, Discovery};
use crate::http::{HttpTransport, Transport};
use crate::push::Api;
use crate::runner::{run_once_observed, Failure, RunError, RunOptions};
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
    /// An account (or, with no label, the whole pass) failed; `failure` says what fixes it.
    #[serde(rename_all = "camelCase")]
    Error {
        label: Option<String>,
        path: Option<String>,
        message: String,
        failure: Failure,
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
}

/// Stops a watch loop from another thread. Cloning shares it.
#[derive(Debug, Clone, Default)]
pub struct CancelToken(Arc<(Mutex<bool>, Condvar)>);

impl CancelToken {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        let (flag, cv) = &*self.0;
        *flag.lock().unwrap_or_else(|e| e.into_inner()) = true;
        cv.notify_all();
    }

    pub fn is_cancelled(&self) -> bool {
        *self.0 .0.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Wait up to `d`; true if cancelled meanwhile (returns at once when it is).
    pub fn wait(&self, d: Duration) -> bool {
        let (flag, cv) = &*self.0;
        let guard = flag.lock().unwrap_or_else(|e| e.into_inner());
        let (guard, _) = cv
            .wait_timeout_while(guard, d, |cancelled| !*cancelled)
            .unwrap_or_else(|e| e.into_inner());
        *guard
    }
}

#[derive(Debug, Clone)]
pub struct WatchOptions {
    /// `--install`: the game folder or a product folder, else the OS defaults.
    pub install: Option<PathBuf>,
    pub state_dir: PathBuf,
    pub interval: Duration,
}

impl WatchOptions {
    /// With the OS state folder (`state::state_dir`); None when there isn't one.
    pub fn new(install: Option<PathBuf>, interval: Duration) -> Option<Self> {
        Some(WatchOptions {
            install,
            state_dir: state::state_dir()?,
            interval,
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

/// `watch` over any transport, token store and discovery (tests; another front end).
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
    while !cancel.is_cancelled() {
        on_event(WatchEvent::SignedIn {
            signed_in: auth.signed_in().unwrap_or(false),
        });
        // Rediscovered every pass, so a new install or account folder is picked up.
        let found = discover(opts.install.as_deref());
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
                on_event(WatchEvent::Error {
                    label: None,
                    path: None,
                    message: RunError::Busy.to_string(),
                    failure: Failure::Other,
                });
                false
            }
            Err(e) => return Err(e),
        };
        on_event(WatchEvent::PassFinished {
            ok,
            next_in_secs: opts.interval.as_secs(),
        });
        if cancel.wait(opts.interval) {
            break;
        }
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
    fn cancel_wakes_a_wait_at_once() {
        let c = CancelToken::new();
        let other = c.clone();
        let t = std::thread::spawn(move || {
            let start = Instant::now();
            let cancelled = other.wait(Duration::from_secs(30));
            (cancelled, start.elapsed())
        });
        std::thread::sleep(Duration::from_millis(50));
        c.cancel();
        let (cancelled, took) = t.join().unwrap();
        assert!(cancelled && took < Duration::from_secs(5));
        assert!(!CancelToken::new().wait(Duration::from_millis(1)));
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
        assert_eq!(v["failure"], json!("signIn"));
        assert!(v["message"].as_str().unwrap().contains("ryna-wow login"));
    }
}
