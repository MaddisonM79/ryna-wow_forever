//! One run (docs/wow.md section 7): for each account file whose mtime differs from the one last
//! pushed, parse it, build the snapshot and post it; then one heartbeat. A failed push keeps the
//! file unpushed, so the next run tries it again. `ryna run` is one of these, and the watch
//! loop (`ryna watch`, the desktop app) a loop of them.

use std::path::{Path, PathBuf};

use crate::addon_install;
use crate::agent_status;
use crate::discovery::{AccountFile, Discovery};
use crate::push::{Api, CombatLog, Heartbeat, HeartbeatAccount, HeartbeatInstall, PushError};
use crate::snapshot::{self, account_key, default_label};
use crate::state::{self, AccountState, LastRun, State};
use crate::util::{self, Clock};
use crate::watch::WatchEvent;

#[derive(Debug, Clone, Default)]
pub struct RunOptions {
    /// Push every file, changed or not.
    pub force: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Outcome {
    Unchanged,
    Pushed {
        characters: usize,
        data: serde_json::Value,
    },
    Failed(String),
    /// Not tried: an earlier account showed the sign-in is gone.
    Skipped,
}

#[derive(Debug, Clone)]
pub struct AccountReport {
    pub path: PathBuf,
    pub label: String,
    pub outcome: Outcome,
    pub warnings: Vec<String>,
}

pub use crate::fix::Fix;

/// The old name for [`Fix`]: why a run failed, most serious first.
pub type Failure = Fix;

#[derive(Debug, Clone)]
pub struct RunReport {
    pub accounts: Vec<AccountReport>,
    pub heartbeat: Result<(), String>,
    /// The most serious failure's fix.
    pub failure: Option<Fix>,
    pub first_error: Option<String>,
}

impl RunReport {
    pub fn ok(&self) -> bool {
        self.failure.is_none()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum RunError {
    /// The run lock is held; the detail says by whom, as far as its file says.
    #[error("another ryna run is in progress ({0})")]
    Busy(state::Busy),
    #[error("no place to keep the agent's state: set RYNA_STATE_DIR")]
    NoStateDir,
    #[error("the agent's state folder: {0}")]
    Io(#[from] std::io::Error),
}

fn classify(e: &PushError) -> Fix {
    e.fix()
}

fn path_key(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

/// One pass over the discovered accounts, then a heartbeat. Holds the run lock in `state_dir`.
pub fn run_once(
    discovery: &Discovery,
    state_dir: &Path,
    api: &Api<'_>,
    clock: &dyn Clock,
    opts: &RunOptions,
) -> Result<RunReport, RunError> {
    run_once_observed(discovery, state_dir, api, clock, opts, &mut |_| {})
}

/// `run_once`, telling `on_event` as each account is looked at, pushed or fails, and how the
/// heartbeat went (the watch loop and the desktop app show these).
pub fn run_once_observed(
    discovery: &Discovery,
    state_dir: &Path,
    api: &Api<'_>,
    clock: &dyn Clock,
    opts: &RunOptions,
    on_event: &mut dyn FnMut(WatchEvent),
) -> Result<RunReport, RunError> {
    let _lock = state::try_lock(state_dir)?.map_err(RunError::Busy)?;
    let mut st = State::load(state_dir);
    let mut reports = Vec::new();
    let mut failure: Option<Fix> = None;
    let mut first_error: Option<String> = None;
    fn note(f: Fix, msg: &str, failure: &mut Option<Fix>, first_error: &mut Option<String>) {
        *failure = Some(failure.map_or(f, |old| old.min(f)));
        first_error.get_or_insert_with(|| msg.to_string());
    }
    let mut signed_out = false;

    for acct in &discovery.accounts {
        let key = path_key(&acct.path);
        let entry = st.accounts.entry(key).or_insert_with(|| AccountState {
            key: account_key(&acct.account),
            label: default_label(&acct.account),
            ..Default::default()
        });
        let (outcome, warnings, failed) = if signed_out {
            (Outcome::Skipped, Vec::new(), None)
        } else {
            push_account(acct, entry, api, clock, opts, on_event)
        };
        let label = entry.label.clone();
        let path = acct.path.to_string_lossy().into_owned();
        match (&outcome, failed) {
            (Outcome::Failed(msg), Some(f)) => {
                note(f, msg, &mut failure, &mut first_error);
                signed_out |= f == Fix::SignIn;
                on_event(WatchEvent::error(
                    Some(label.clone()),
                    Some(path),
                    msg.clone(),
                    f,
                ));
            }
            (Outcome::Pushed { characters, data }, _) => on_event(WatchEvent::Pushed {
                label: label.clone(),
                path,
                characters: *characters,
                result: data.clone(),
            }),
            _ => {}
        }
        reports.push(AccountReport {
            path: acct.path.clone(),
            label: entry.label.clone(),
            outcome,
            warnings,
        });
        // Keep progress even if a later account crashes the run.
        st.save(state_dir)?;
    }

    let heartbeat = if signed_out {
        Err("skipped: not signed in".to_string())
    } else {
        let mut hb = heartbeat_body(discovery, &st, first_error.clone());
        hb.submission_id = api.submission_id();
        hb.combat_log = crate::combat::tail::heartbeat(state_dir, clock.now());
        api.heartbeat(&hb).map(|_| ()).map_err(|e| {
            note(classify(&e), &e.to_string(), &mut failure, &mut first_error);
            e.to_string()
        })
    };
    on_event(WatchEvent::Heartbeat {
        ok: heartbeat.is_ok(),
        error: heartbeat.as_ref().err().cloned(),
    });

    st.last_run = Some(LastRun {
        at: clock.now(),
        ok: failure.is_none(),
        error: first_error.clone(),
        fix: failure,
    });
    st.save(state_dir)?;

    // Tell the addon how it went, success or not.
    let auth = api.auth();
    let (_, unwritable) = agent_status::write_all(
        discovery,
        &st,
        auth.signed_in().unwrap_or(false),
        auth.identity(),
        failure.zip(first_error.as_deref()),
        &agent_status::Combat::from_state(state_dir, clock.now()),
        clock.now(),
    );
    for (path, e) in unwritable {
        eprintln!("couldn't write {}: {e}", path.display());
    }

    Ok(RunReport {
        accounts: reports,
        heartbeat,
        failure,
        first_error,
    })
}

fn push_account(
    acct: &AccountFile,
    entry: &mut AccountState,
    api: &Api<'_>,
    clock: &dyn Clock,
    opts: &RunOptions,
    on_event: &mut dyn FnMut(WatchEvent),
) -> (Outcome, Vec<String>, Option<Fix>) {
    let mtime = match util::mtime(&acct.path) {
        Ok(m) => m,
        Err(e) => {
            let msg = format!("{}: {e}", acct.path.display());
            entry.last_error = Some(msg.clone());
            return (Outcome::Failed(msg), Vec::new(), Some(Fix::Other));
        }
    };
    entry.saved_at = Some(mtime.secs);
    if !opts.force && entry.pushed_mtime_ns == Some(mtime.nanos) {
        return (Outcome::Unchanged, Vec::new(), None);
    }
    let label = entry.label.clone();
    let path = acct.path.to_string_lossy().into_owned();
    on_event(WatchEvent::AccountFileChanged {
        label: label.clone(),
        path: path.clone(),
        saved_at: mtime.secs,
    });
    let built = match snapshot::read_file(
        &acct.path,
        &acct.account,
        Some(&entry.label),
        acct.flavor.as_deref(),
    ) {
        Ok(b) => b,
        Err(e) => {
            let failure = if e.is_schema_unsupported() {
                Fix::Update
            } else {
                Fix::Other
            };
            let msg = e.to_string();
            entry.last_error = Some(msg.clone());
            return (Outcome::Failed(msg), Vec::new(), Some(failure));
        }
    };
    on_event(WatchEvent::Pushing { label, path });
    match api.ingest(&built.snapshot) {
        Ok(data) => {
            let characters = built.snapshot.characters.len();
            entry.pushed_mtime_ns = Some(mtime.nanos);
            entry.saved_at = Some(built.snapshot.saved_at);
            entry.pushed_at = Some(clock.now());
            entry.characters = Some(characters);
            entry.snapshot_id = data.get("snapshotId").cloned();
            entry.account_id = built.account_id.clone();
            entry.journal_through = built.journal_through;
            entry.last_error = None;
            (Outcome::Pushed { characters, data }, built.warnings, None)
        }
        Err(e) => {
            let msg = e.to_string();
            entry.last_error = Some(msg.clone());
            (Outcome::Failed(msg), built.warnings, Some(classify(&e)))
        }
    }
}

/// What the heartbeat says: every install, and every discovered account with its last push.
pub fn heartbeat_body(discovery: &Discovery, st: &State, last_error: Option<String>) -> Heartbeat {
    Heartbeat {
        agent: crate::AGENT_VERSION.to_string(),
        host: util::hostname(),
        os: util::os_name().to_string(),
        installs: discovery
            .installs
            .iter()
            .map(|i| HeartbeatInstall {
                path: i.path.to_string_lossy().into_owned(),
                flavor: i.flavor.clone(),
                addon: addon_install::installed_addon_version(i),
            })
            .collect(),
        accounts: discovery
            .accounts
            .iter()
            .map(|a| {
                let s = st.accounts.get(&path_key(&a.path));
                HeartbeatAccount {
                    key: s.map_or_else(|| account_key(&a.account), |s| s.key.clone()),
                    label: s.map_or_else(|| default_label(&a.account), |s| s.label.clone()),
                    saved_at: s.and_then(|s| s.saved_at),
                    pushed_at: s.and_then(|s| s.pushed_at),
                    characters: s.and_then(|s| s.characters),
                }
            })
            .collect(),
        combat_log: CombatLog::default(),
        // Kept well inside the heartbeat's 64 KiB.
        last_error: last_error.map(|e| e.chars().take(1000).collect()),
        // The caller's device key's, set where the Api is in hand.
        submission_id: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::testing::MemoryStore;
    use crate::auth::Auth;
    use crate::config::test_config;
    use crate::discovery::{discover, testing::product};
    use crate::http::testing::ScriptedTransport;
    use crate::identity::testing::test_key;
    use crate::identity::DeviceKey;
    use crate::util::testing::FakeClock;
    use serde_json::json;
    use std::time::{Duration, SystemTime};

    const FIXTURE: &str = include_str!("../../fixtures/Ryna.lua");
    const NOW: i64 = 1_800_000_000;

    fn ingested() -> serde_json::Value {
        json!({ "ok": true, "data": { "snapshotId": 7, "characters": 2, "journal": { "stored": 3, "repeated": 0 }, "sightings": 1 } })
    }
    fn ok() -> serde_json::Value {
        json!({ "ok": true, "data": {} })
    }

    struct Env {
        _dir: tempfile::TempDir,
        root: PathBuf,
        state: PathBuf,
        store: MemoryStore,
        clock: FakeClock,
    }

    fn env(accounts: &[(&str, &str)]) -> Env {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("World of Warcraft");
        product(&root, "_forever_", "wowf", accounts);
        Env {
            state: dir.path().join("state"),
            root,
            _dir: dir,
            store: MemoryStore::with_access("tok", NOW + 3600),
            clock: FakeClock::at(NOW),
        }
    }

    fn run(e: &Env, http: &ScriptedTransport, force: bool) -> RunReport {
        let config = test_config();
        let auth = Auth::new(&config, http, &e.store, &e.clock);
        let key = test_key();
        let api = Api::new(&config.api, http, &auth, Some(&key), &e.clock);
        run_once(
            &discover(Some(&e.root)),
            &e.state,
            &api,
            &e.clock,
            &RunOptions { force },
        )
        .expect("run_once (a Busy names the lock's holder)")
    }

    fn touch(path: &Path, secs_later: u64) {
        let f = std::fs::File::options().write(true).open(path).unwrap();
        f.set_modified(SystemTime::now() + Duration::from_secs(secs_later))
            .unwrap();
    }

    #[test]
    fn pushes_changed_files_only_then_heartbeats() {
        let e = env(&[("FAKEACCOUNT1", FIXTURE), ("FAKEACCOUNT2", FIXTURE)]);
        let http = ScriptedTransport::new();
        http.json(200, ingested())
            .json(200, ingested())
            .json(200, ok());
        let r = run(&e, &http, false);
        assert!(r.ok(), "{r:?}");
        assert!(matches!(
            r.accounts[0].outcome,
            Outcome::Pushed { characters: 2, .. }
        ));
        let urls = http.urls();
        assert_eq!(urls[0], "https://api.test/v1/wow/agent/ingest");
        assert_eq!(urls[2], "https://api.test/v1/wow/agent/heartbeat");
        let body: serde_json::Value =
            serde_json::from_slice(&http.requests.borrow()[0].body).unwrap();
        assert_eq!(body["account"]["key"], json!(account_key("FAKEACCOUNT1")));
        assert_eq!(body["account"]["label"], json!("UNT1"));
        let hb: serde_json::Value =
            serde_json::from_slice(&http.requests.borrow()[2].body).unwrap();
        assert_eq!(hb["accounts"][0]["pushedAt"], json!(NOW));
        assert_eq!(hb["accounts"][0]["characters"], json!(2));
        assert_eq!(hb["installs"][0]["flavor"], json!("wowf"));
        assert_eq!(hb["installs"][0]["addon"], json!(null));
        assert_eq!(hb["lastError"], json!(null));
        assert_eq!(hb["submissionId"], json!(test_key().submission_id()));
        // The snapshot and the heartbeat are both signed, over the bytes sent.
        for i in [0, 2] {
            let req = http.requests.borrow()[i].clone();
            let sig = http.header(i, "X-Ryna-Signature").unwrap();
            let pk = http.header(i, "X-Ryna-Key").unwrap();
            assert_eq!(
                http.header(i, "X-Ryna-Submission"),
                Some(test_key().submission_id())
            );
            assert!(crate::identity::verify(&pk, &req.body, &sig));
        }

        // Nothing changed: only the heartbeat.
        let http = ScriptedTransport::new();
        http.json(200, ok());
        let r = run(&e, &http, false);
        assert!(r.accounts.iter().all(|a| a.outcome == Outcome::Unchanged));
        assert_eq!(http.urls(), vec!["https://api.test/v1/wow/agent/heartbeat"]);

        // One file written again: that one.
        touch(
            &e.root
                .join("_forever_/WTF/Account/FAKEACCOUNT2/SavedVariables/Ryna.lua"),
            5,
        );
        let http = ScriptedTransport::new();
        http.json(200, ingested()).json(200, ok());
        let r = run(&e, &http, false);
        assert_eq!(r.accounts[0].outcome, Outcome::Unchanged);
        assert!(matches!(r.accounts[1].outcome, Outcome::Pushed { .. }));

        // --force: both.
        let http = ScriptedTransport::new();
        http.json(200, ingested())
            .json(200, ingested())
            .json(200, ok());
        assert!(run(&e, &http, true).ok());
    }

    #[test]
    fn offline_keeps_the_file_unpushed() {
        let e = env(&[("FAKEACCOUNT1", FIXTURE)]);
        let http = ScriptedTransport::new();
        http.fail("offline")
            .fail("offline")
            .fail("offline")
            .fail("offline")
            .fail("offline")
            .fail("offline");
        let r = run(&e, &http, false);
        assert_eq!(r.failure, Some(Fix::Network));
        assert!(r.heartbeat.is_err());
        let st = State::load(&e.state);
        let a = st.accounts.values().next().unwrap();
        assert_eq!(a.pushed_mtime_ns, None);
        assert!(a.last_error.as_deref().unwrap().contains("offline"));
        let last = st.last_run.unwrap();
        assert!(!last.ok);
        assert_eq!(last.fix, Some(Fix::Network));

        // Back online: it is pushed.
        let http = ScriptedTransport::new();
        http.json(200, ingested()).json(200, ok());
        let r = run(&e, &http, false);
        assert!(r.ok());
        assert!(State::load(&e.state)
            .accounts
            .values()
            .next()
            .unwrap()
            .last_error
            .is_none());
    }

    #[test]
    fn a_push_keeps_the_account_id_and_journal_reach_and_status_has_them() {
        let with_id = FIXTURE.replacen(
            "RynaDB = {",
            "RynaDB = {\n\tmeta = { accountId = \"a1b2c3d4e5f60718\" },",
            1,
        );
        let e = env(&[("FAKEACCOUNT1", &with_id), ("FAKEACCOUNT2", FIXTURE)]);
        let http = ScriptedTransport::new();
        http.json(200, ingested())
            .json(200, ingested())
            .json(200, ok());
        assert!(run(&e, &http, false).ok());
        // The id is never sent.
        let body = String::from_utf8(http.requests.borrow()[0].body.clone()).unwrap();
        assert!(!body.contains("a1b2c3d4e5f60718"));
        let st = State::load(&e.state);
        let by_folder = |folder: &str| {
            st.accounts
                .iter()
                .find(|(path, _)| path.contains(folder))
                .map(|(_, a)| a.clone())
                .unwrap()
        };
        let one = by_folder("FAKEACCOUNT1");
        let newest = snapshot::build(
            &crate::lua::parse(&with_id).unwrap(),
            &snapshot::Source {
                account_folder: "FAKEACCOUNT1",
                label: None,
                saved_at: 0,
                install_flavor: Some("wowf"),
            },
        )
        .unwrap()
        .snapshot
        .journal
        .iter()
        .map(|j| j.at)
        .max();
        assert_eq!(one.account_id.as_deref(), Some("a1b2c3d4e5f60718"));
        assert_eq!(one.journal_through, newest);
        assert!(newest.is_some());
        // A core without the id: none, so the addon drops nothing early.
        let two = by_folder("FAKEACCOUNT2");
        assert_eq!(two.account_id, None);
        assert_eq!(two.journal_through, newest);
    }

    #[test]
    fn signed_out_stops_early_with_exit_3() {
        let mut e = env(&[("FAKEACCOUNT1", FIXTURE), ("FAKEACCOUNT2", FIXTURE)]);
        e.store = MemoryStore::default();
        let http = ScriptedTransport::new();
        let r = run(&e, &http, false);
        assert_eq!(r.failure.map(Fix::exit_code), Some(3));
        assert_eq!(r.accounts[1].outcome, Outcome::Skipped);
        assert!(http.requests.borrow().is_empty());
    }

    #[test]
    fn a_bad_file_fails_alone_and_the_heartbeat_says_so() {
        let e = env(&[("FAKEACCOUNT1", "RynaDB = {"), ("FAKEACCOUNT2", FIXTURE)]);
        let http = ScriptedTransport::new();
        http.json(200, ingested()).json(200, ok());
        let r = run(&e, &http, false);
        assert_eq!(r.failure, Some(Fix::Other));
        assert!(
            matches!(&r.accounts[0].outcome, Outcome::Failed(m) if m.contains("unfinished table"))
        );
        assert!(matches!(r.accounts[1].outcome, Outcome::Pushed { .. }));
        let hb: serde_json::Value =
            serde_json::from_slice(&http.requests.borrow()[1].body).unwrap();
        assert!(hb["lastError"]
            .as_str()
            .unwrap()
            .contains("unfinished table"));
    }

    #[test]
    fn schema_refusals_exit_4() {
        let e = env(&[("FAKEACCOUNT1", "RynaDB = { schema = 2 }")]);
        let http = ScriptedTransport::new();
        http.json(200, ok());
        let r = run(&e, &http, false);
        assert_eq!(r.failure.map(Fix::exit_code), Some(4));

        let e = env(&[("FAKEACCOUNT1", FIXTURE)]);
        let http = ScriptedTransport::new();
        http.json(200, json!({ "ok": false, "error": { "code": "SCHEMA_UNSUPPORTED", "message": "x", "detail": { "accepted": [2] } } }))
            .json(200, ok());
        let r = run(&e, &http, false);
        assert_eq!(r.failure.map(Fix::exit_code), Some(4));
        assert_eq!(
            State::load(&e.state)
                .accounts
                .values()
                .next()
                .unwrap()
                .pushed_mtime_ns,
            None
        );
    }

    #[test]
    fn no_accounts_still_heartbeats() {
        let e = env(&[]);
        let http = ScriptedTransport::new();
        http.json(200, ok());
        assert!(run(&e, &http, false).ok());
        assert_eq!(http.urls().len(), 1);
    }

    #[test]
    fn a_second_run_waits_for_the_first() {
        let e = env(&[]);
        let _held = state::try_lock(&e.state).unwrap().unwrap();
        let config = test_config();
        let http = ScriptedTransport::new();
        let auth = Auth::new(&config, &http, &e.store, &e.clock);
        let key = test_key();
        let api = Api::new(&config.api, &http, &auth, Some(&key), &e.clock);
        let r = run_once(
            &discover(Some(&e.root)),
            &e.state,
            &api,
            &e.clock,
            &RunOptions::default(),
        );
        assert!(matches!(r, Err(RunError::Busy(_))));
    }
}
