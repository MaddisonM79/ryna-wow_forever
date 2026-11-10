//! One run (docs/wow.md section 7): for each account file whose mtime differs from the one last
//! pushed, parse it, build the snapshot and post it; then one heartbeat. A failed push keeps the
//! file unpushed, so the next run tries it again. `ryna run` is one of these, and the watch
//! loop (`ryna watch`, the desktop app) a loop of them.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::addon_install;
use crate::agent_status;
use crate::discovery::{AccountFile, Discovery, Install};
use crate::prices_data;
use crate::push::{Api, CombatLog, Heartbeat, HeartbeatAccount, HeartbeatInstall, PushError};
use crate::snapshot::{self, account_key, default_label};
use crate::state::{self, AccountState, LastRun, State};
use crate::support::{self, KnownSupport, Support};
use crate::util::{self, Clock};
use crate::watch::WatchEvent;
use crate::zones_backfill;

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
    /// The Prices mod's `Data/` tables, per install that has the addon: refreshed, or why not.
    /// Never part of `failure` (a stale price table doesn't fail a run).
    pub prices_data: Vec<Result<prices_data::Refreshed, (PathBuf, String)>>,
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
    // The item ids of each account file this run parsed and pushed, for the pooled prices.
    let mut ids_by_account: BTreeMap<PathBuf, Vec<i64>> = BTreeMap::new();
    // The API's newest judgment of this version this pass (an ingest's answer or refusal, then the
    // heartbeat's), and the ingests' `pooled` (the first account that wasn't pooled, by label).
    let mut judged: Option<Support> = None;
    let mut paused_label: Option<String> = None;
    let mut pooled_any = false;

    for acct in &discovery.accounts {
        // Chronicle by zone's backfill (zones_backfill): once per character, with the game closed,
        // before the push so the push carries it. Local only; a failure is the next run's to retry.
        backfill_zones(acct, clock);
        let key = path_key(&acct.path);
        let entry = st.accounts.entry(key).or_insert_with(|| AccountState {
            key: account_key(&acct.account),
            label: default_label(&acct.account),
            ..Default::default()
        });
        let (outcome, warnings, failed) = if signed_out {
            (Outcome::Skipped, Vec::new(), None)
        } else {
            let (outcome, warnings, failed, ids) =
                push_account(acct, entry, api, clock, opts, on_event, &mut judged);
            if let Some(ids) = ids {
                ids_by_account.insert(acct.path.clone(), ids);
            }
            (outcome, warnings, failed)
        };
        let label = entry.label.clone();
        let path = acct.path.to_string_lossy().into_owned();
        match (&outcome, failed) {
            (Outcome::Failed(msg), Some(f)) => {
                note(f, msg, &mut failure, &mut first_error);
                signed_out |= f == Fix::SignIn;
                on_event(WatchEvent::Error {
                    label: Some(label.clone()),
                    path: Some(path),
                    message: msg.clone(),
                    fix: f,
                    advice: support::advice(f, msg, judged.as_ref()),
                });
            }
            (Outcome::Pushed { characters, data }, _) => {
                match data.get("pooled").and_then(serde_json::Value::as_bool) {
                    Some(false) => {
                        paused_label.get_or_insert_with(|| label.clone());
                    }
                    Some(true) => pooled_any = true,
                    None => {}
                }
                on_event(WatchEvent::Pushed {
                    label: label.clone(),
                    path,
                    characters: *characters,
                    result: data.clone(),
                })
            }
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
        match api.heartbeat(&hb) {
            Ok(data) => {
                if let Some(s) = Support::in_answer(&data) {
                    judged = Some(s);
                }
                Ok(())
            }
            Err(e) => {
                note(classify(&e), &e.to_string(), &mut failure, &mut first_error);
                Err(e.to_string())
            }
        }
    };
    on_event(WatchEvent::Heartbeat {
        ok: heartbeat.is_ok(),
        error: heartbeat.as_ref().err().cloned(),
    });

    // What the API said about this version, kept to show (never to refuse: it fails open, so with
    // no answer this pass the last one stays, and with none ever it's `current`).
    let known = match judged.take() {
        Some(s) => {
            let (known, changed) = support::record(state_dir, s, clock.now());
            on_event(WatchEvent::Support {
                support: known.support.clone(),
                received_at: known.received_at,
                changed,
            });
            Some(known)
        }
        None => KnownSupport::load(state_dir),
    };
    // Reduced mode: the ingest stored your data and pooled none of it. Said once, not every pass.
    if let Some(label) = paused_label {
        if support::note_pooled(state_dir, false) {
            on_event(WatchEvent::ContributionsPaused { label });
        }
    } else if pooled_any {
        support::note_pooled(state_dir, true);
    }

    // The Prices mod's way back down (prices_data): the pooled prices for what these accounts
    // know, into the addon's Data/ files. Not while the sign-in is gone or the API isn't
    // answering (the run has already said so; asking again would only add retries).
    let prices = if signed_out || matches!(failure, Some(Fix::Network) | Some(Fix::Update)) {
        Vec::new()
    } else {
        prices_data::refresh(discovery, &mut st, api, &ids_by_account, clock.now())
    };
    for (path, e) in prices.iter().filter_map(|r| r.as_ref().err()) {
        eprintln!("prices for {}: {e}", path.display());
    }

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
        known.as_ref().map(|k| &k.support),
        &agent_status::Combat::from_state(state_dir, clock.now()),
        clock.now(),
    );
    for (path, e) in unwritable {
        eprintln!("couldn't write {}: {e}", path.display());
    }

    Ok(RunReport {
        accounts: reports,
        heartbeat,
        prices_data: prices,
        failure,
        first_error,
    })
}

/// The combat-log backfill for one account file ([`zones_backfill::run`]), said on stderr.
fn backfill_zones(acct: &AccountFile, clock: &dyn Clock) {
    let running = || crate::game_process::game_running_at(&acct.install);
    match zones_backfill::run(acct, clock.now(), &running) {
        Ok(zones_backfill::Outcome::Done { placed, files }) => {
            for p in placed {
                eprintln!(
                    "zones backfill: {} kills placed in zones for {} from {files} combat logs",
                    p.placed, p.guid
                );
            }
        }
        Ok(_) => {}
        Err(e) => eprintln!("zones backfill for {}: {e:#}", acct.path.display()),
    }
}

/// One account: pushed, unchanged, or failed; with the file's item ids when it was parsed and
/// pushed (for the pooled prices).
#[allow(clippy::type_complexity)]
fn push_account(
    acct: &AccountFile,
    entry: &mut AccountState,
    api: &Api<'_>,
    clock: &dyn Clock,
    opts: &RunOptions,
    on_event: &mut dyn FnMut(WatchEvent),
    judged: &mut Option<Support>,
) -> (Outcome, Vec<String>, Option<Fix>, Option<Vec<i64>>) {
    let mtime = match util::mtime(&acct.path) {
        Ok(m) => m,
        Err(e) => {
            let msg = format!("{}: {e}", acct.path.display());
            entry.last_error = Some(msg.clone());
            return (Outcome::Failed(msg), Vec::new(), Some(Fix::Other), None);
        }
    };
    entry.saved_at = Some(mtime.secs);
    if !opts.force && entry.pushed_mtime_ns == Some(mtime.nanos) {
        return (Outcome::Unchanged, Vec::new(), None, None);
    }
    let label = entry.label.clone();
    let path = acct.path.to_string_lossy().into_owned();
    on_event(WatchEvent::AccountFileChanged {
        label: label.clone(),
        path: path.clone(),
        saved_at: mtime.secs,
    });
    let mut built = match snapshot::read_file(
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
            return (Outcome::Failed(msg), Vec::new(), Some(failure), None);
        }
    };
    // The TOC of the addon in this file's install: what the API judges the addon by.
    built.snapshot.addon_toc = addon_install::installed_addon_version(&Install {
        path: acct.install.clone(),
        flavor: acct.flavor.clone(),
    });
    on_event(WatchEvent::Pushing { label, path });
    match api.ingest(&built.snapshot) {
        Ok(data) => {
            if let Some(s) = Support::in_answer(&data) {
                *judged = Some(s);
            }
            let characters = built.snapshot.characters.len();
            entry.pushed_mtime_ns = Some(mtime.nanos);
            entry.saved_at = Some(built.snapshot.saved_at);
            entry.pushed_at = Some(clock.now());
            entry.characters = Some(characters);
            entry.snapshot_id = data.get("snapshotId").cloned();
            entry.account_id = built.account_id.clone();
            entry.journal_through = built.journal_through;
            entry.last_error = None;
            (
                Outcome::Pushed { characters, data },
                built.warnings,
                None,
                Some(built.item_ids),
            )
        }
        Err(e) => {
            if let Some(s) = e.support() {
                *judged = Some(s.clone());
            }
            let msg = e.to_string();
            entry.last_error = Some(msg.clone());
            (
                Outcome::Failed(msg),
                built.warnings,
                Some(classify(&e)),
                None,
            )
        }
    }
}

/// What the heartbeat says: every install, and every discovered account with its last push.
pub fn heartbeat_body(discovery: &Discovery, st: &State, last_error: Option<String>) -> Heartbeat {
    Heartbeat {
        agent: crate::AGENT_VERSION.to_string(),
        channel: support::CHANNEL.to_string(),
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
    fn a_push_brings_the_pooled_prices_down_for_installs_with_the_addon() {
        let mut e = env(&[("FAKEACCOUNT1", FIXTURE)]);
        // A token that outlives the six hours below, so no refresh call lands in the script.
        e.store = MemoryStore::with_access("tok", NOW + 100_000);
        let addon = e.root.join("_forever_/Interface/AddOns/Ryna");
        std::fs::create_dir_all(&addon).unwrap();
        let http = ScriptedTransport::new();
        http.json(200, ingested()).json(200, ok()).json(
            200,
            json!({ "ok": true, "data": { "flavor": "wowf", "items": [
                { "itemId": 6948, "vendors": [ { "npcId": 6929, "name": "Innkeeper Gryshka", "zone": "Orgrimmar", "price": 25, "stack": 1, "currency": false, "observers": 2, "lastSeen": 1 } ],
                  "today": { "min": 80, "med": 110, "listings": 3, "quantity": 10, "snapshots": 2, "observers": 2, "asOf": 1_762_464_060 }, "week": { "med": 110, "days": 1, "snapshots": 2 } } ] } }),
        );
        let r = run(&e, &http, false);
        assert!(r.ok(), "{r:?}");
        let urls = http.urls();
        assert_eq!(urls.len(), 3, "{urls:?}");
        assert!(urls[2].starts_with("https://api.test/v1/wow/agent/prices?items="));
        // The fixture's bag items are asked for, Hearthstone among them, and the flavor is the install's.
        assert!(urls[2].contains("6948"), "{}", urls[2]);
        assert!(urls[2].ends_with("&flavor=wowf"), "{}", urls[2]);
        assert_eq!(r.prices_data.len(), 1);
        let refreshed = r.prices_data[0].as_ref().unwrap();
        assert_eq!((refreshed.vendor_items, refreshed.auction_items), (1, 1));
        assert_eq!(refreshed.written.len(), 2);
        let vendors =
            crate::lua::parse(&std::fs::read_to_string(addon.join("Data/vendors.lua")).unwrap())
                .unwrap();
        assert_eq!(
            vendors["RynaData_vendors"]["items"]["6948"][0][1],
            json!("Innkeeper Gryshka")
        );
        assert_eq!(vendors["RynaData_vendors"]["writtenAt"], json!(NOW));
        let auctions =
            crate::lua::parse(&std::fs::read_to_string(addon.join("Data/auctions.lua")).unwrap())
                .unwrap();
        assert_eq!(
            auctions["RynaData_auctions"]["items"]["6948"][1],
            json!(110)
        );
        assert_eq!(
            State::load(&e.state)
                .prices_data_at
                .values()
                .copied()
                .collect::<Vec<_>>(),
            vec![NOW]
        );

        // Nothing changed, tables fresh: the heartbeat alone, no price call.
        let http = ScriptedTransport::new();
        http.json(200, ok());
        let r = run(&e, &http, false);
        assert!(r.ok());
        assert_eq!(http.urls().len(), 1);
        assert!(r.prices_data.is_empty());

        // Six hours on and still nothing changed: the account file is read for its ids and the
        // tables refreshed; the same rows aren't rewritten.
        e.clock.now.set(NOW + crate::prices_data::REFRESH_EVERY_S);
        let http = ScriptedTransport::new();
        http.json(200, ok()).json(200, json!({ "ok": true, "data": { "flavor": "wowf", "items": [
            { "itemId": 6948, "vendors": [ { "npcId": 6929, "name": "Innkeeper Gryshka", "zone": "Orgrimmar", "price": 25, "stack": 1, "currency": false, "observers": 2, "lastSeen": 1 } ],
              "today": { "min": 80, "med": 110, "listings": 3, "quantity": 10, "snapshots": 2, "observers": 2, "asOf": 1_762_464_060 }, "week": { "med": 110, "days": 1, "snapshots": 2 } } ] } }));
        let r = run(&e, &http, false);
        assert_eq!(http.urls().len(), 2);
        assert!(
            http.urls()[1].starts_with("https://api.test/v1/wow/agent/prices?"),
            "{:?}",
            http.urls()
        );
        assert!(
            r.prices_data[0].as_ref().unwrap().written.is_empty(),
            "same rows: not rewritten"
        );

        // The price read failing never fails the run.
        touch(
            &e.root
                .join("_forever_/WTF/Account/FAKEACCOUNT1/SavedVariables/Ryna.lua"),
            5,
        );
        let http = ScriptedTransport::new();
        http.json(200, ingested()).json(200, ok()).json(
            403,
            json!({ "ok": false, "error": { "code": "FORBIDDEN", "message": "no" } }),
        );
        let r = run(&e, &http, false);
        assert!(r.ok(), "{r:?}");
        assert!(r.prices_data[0].is_err());
    }

    #[test]
    fn the_pooled_prices_count_every_account_and_back_off_after_a_failure() {
        // Two accounts on one install; the second holds an item the first doesn't.
        let other = FIXTURE.replacen("[\"itemId\"] = 6948", "[\"itemId\"] = 424242", 1);
        let mut e = env(&[("FAKEACCOUNT1", FIXTURE), ("FAKEACCOUNT2", &other)]);
        e.store = MemoryStore::with_access("tok", NOW + 100_000);
        let addon = e.root.join("_forever_/Interface/AddOns/Ryna");
        std::fs::create_dir_all(&addon).unwrap();
        let empty = || json!({ "ok": true, "data": { "flavor": "wowf", "items": [] } });
        let http = ScriptedTransport::new();
        http.json(200, ingested())
            .json(200, ingested())
            .json(200, ok())
            .json(200, empty());
        assert!(run(&e, &http, false).ok());
        let prices = http.urls()[3].clone();
        assert!(
            prices.contains("6948") && prices.contains("424242"),
            "{prices}"
        );

        // Only the first account changes: the second's ids are still asked for (read from its file).
        touch(
            &e.root
                .join("_forever_/WTF/Account/FAKEACCOUNT1/SavedVariables/Ryna.lua"),
            5,
        );
        let http = ScriptedTransport::new();
        http.json(200, ingested())
            .json(200, ok())
            .json(200, empty());
        let r = run(&e, &http, false);
        assert!(r.ok(), "{r:?}");
        let prices = http.urls()[2].clone();
        assert!(
            prices.contains("424242"),
            "the unchanged account's items too: {prices}"
        );

        // A refused page: the run is fine, the install waits RETRY_AFTER_S, then tries again.
        touch(
            &e.root
                .join("_forever_/WTF/Account/FAKEACCOUNT1/SavedVariables/Ryna.lua"),
            10,
        );
        let http = ScriptedTransport::new();
        http.json(200, ingested()).json(200, ok()).json(
            400,
            json!({ "ok": false, "error": { "code": "BAD_REQUEST", "message": "no" } }),
        );
        let r = run(&e, &http, false);
        assert!(r.ok() && r.prices_data[0].is_err(), "{r:?}");
        let st = State::load(&e.state);
        assert_eq!(
            st.prices_data_retry_at
                .values()
                .copied()
                .collect::<Vec<_>>(),
            vec![NOW + crate::prices_data::RETRY_AFTER_S]
        );
        touch(
            &e.root
                .join("_forever_/WTF/Account/FAKEACCOUNT1/SavedVariables/Ryna.lua"),
            15,
        );
        let http = ScriptedTransport::new();
        http.json(200, ingested()).json(200, ok());
        let r = run(&e, &http, false);
        assert!(r.prices_data.is_empty(), "not tried again yet: {r:?}");
        // Once the wait is over, the next push (or the 6-hour mark) tries again.
        e.clock.now.set(NOW + crate::prices_data::RETRY_AFTER_S);
        touch(
            &e.root
                .join("_forever_/WTF/Account/FAKEACCOUNT1/SavedVariables/Ryna.lua"),
            20,
        );
        let http = ScriptedTransport::new();
        http.json(200, ingested())
            .json(200, ok())
            .json(200, empty());
        let r = run(&e, &http, false);
        assert!(r.prices_data[0].is_ok(), "{r:?}");
        assert!(State::load(&e.state).prices_data_retry_at.is_empty());

        // An account file that can't be read: nothing asked, nothing written, the install put off.
        let tables_before = std::fs::read_to_string(addon.join("Data/vendors.lua")).unwrap();
        e.clock
            .now
            .set(NOW + crate::prices_data::REFRESH_EVERY_S * 2);
        std::fs::write(
            e.root
                .join("_forever_/WTF/Account/FAKEACCOUNT2/SavedVariables/Ryna.lua"),
            "RynaDB = { broken",
        )
        .unwrap();
        let http = ScriptedTransport::new();
        http.json(200, ok());
        let r = run(&e, &http, false);
        // The broken file never reaches the API (a parse failure), and neither does a price page.
        assert_eq!(
            http.urls(),
            vec!["https://api.test/v1/wow/agent/heartbeat".to_string()]
        );
        assert!(r.prices_data[0].is_err(), "{r:?}");
        assert_eq!(
            std::fs::read_to_string(addon.join("Data/vendors.lua")).unwrap(),
            tables_before
        );
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

    /// `run`, with the events it says.
    fn run_seeing(e: &Env, http: &ScriptedTransport, force: bool) -> (RunReport, Vec<WatchEvent>) {
        let config = test_config();
        let auth = Auth::new(&config, http, &e.store, &e.clock);
        let key = test_key();
        let api = Api::new(&config.api, http, &auth, Some(&key), &e.clock);
        let mut events = Vec::new();
        let r = run_once_observed(
            &discover(Some(&e.root)),
            &e.state,
            &api,
            &e.clock,
            &RunOptions { force },
            &mut |ev| events.push(ev),
        )
        .unwrap();
        (r, events)
    }

    /// The addon at `toc` in the fixture's install; returns its folder.
    fn addon_at(e: &Env, toc: &str) -> PathBuf {
        let addon = e.root.join("_forever_/Interface/AddOns/Ryna");
        std::fs::create_dir_all(&addon).unwrap();
        std::fs::write(
            addon.join("Ryna.toc"),
            format!("## Title: Ryna\n## Version: {toc}\n"),
        )
        .unwrap();
        addon
    }

    fn judged(band: &str) -> serde_json::Value {
        json!({ "band": band, "version": crate::AGENT_VERSION, "addon": "0.3.4", "channel": "stable",
                "minimum": "0.3.1", "preferred": "999.0.0", "perfect": "999.0.0", "note": null,
                "update": "https://ryna.io/wow/guide#upkeep" })
    }

    fn agent_status(addon: &Path) -> serde_json::Value {
        let text = std::fs::read_to_string(addon.join("AgentStatus.lua")).unwrap();
        crate::lua::parse(&text).unwrap()["RynaAgentStatus"].clone()
    }

    #[test]
    fn reduced_mode_is_kept_shown_and_said_once() {
        let mut e = env(&[("FAKEACCOUNT1", FIXTURE)]);
        e.store = MemoryStore::with_access("tok", NOW + 100_000);
        let addon = addon_at(&e, "0.3.4");
        let ingest = json!({ "ok": true, "data": { "snapshotId": 7, "sightings": 0, "pooled": false, "support": judged("reduced") } });
        let beat = json!({ "ok": true, "data": { "host": "h", "seenAt": NOW, "support": judged("reduced") } });
        let prices = json!({ "ok": true, "data": { "flavor": "wowf", "items": [] } });
        let http = ScriptedTransport::new();
        http.json(200, ingest.clone())
            .json(200, beat.clone())
            .json(200, prices.clone());
        let (r, events) = run_seeing(&e, &http, false);
        assert!(r.ok(), "{r:?}");
        // The snapshot carries the install's TOC, the heartbeat the channel.
        let body: serde_json::Value =
            serde_json::from_slice(&http.requests.borrow()[0].body).unwrap();
        assert_eq!(body["addonToc"], json!("0.3.4"));
        assert_eq!(
            body["addon"],
            json!("0.1.0"),
            "RynaDB.addon, sent as it was"
        );
        let hb: serde_json::Value =
            serde_json::from_slice(&http.requests.borrow()[1].body).unwrap();
        assert_eq!(hb["channel"], json!("stable"));
        // A changed band and the pause, each said.
        let support = events.iter().find_map(|ev| match ev {
            WatchEvent::Support {
                support, changed, ..
            } => Some((support.band, *changed)),
            _ => None,
        });
        assert_eq!(support, Some((support::Band::Reduced, true)));
        assert!(events
            .iter()
            .any(|ev| matches!(ev, WatchEvent::ContributionsPaused { label } if label == "UNT1")));
        // Kept, with when, for the window.
        let known = KnownSupport::load(&e.state).unwrap();
        assert_eq!(
            (known.support.band, known.received_at, known.pooled),
            (support::Band::Reduced, NOW, Some(false))
        );
        // Written for the addon: support, the old addons' line, and both updates offered.
        let a = agent_status(&addon);
        assert_eq!(a["support"]["band"], json!("reduced"));
        assert_eq!(
            a["messages"],
            json!([{ "at": NOW, "level": "warn", "text": support::REDUCED_SENTENCE }])
        );
        assert_eq!(
            a["latest"],
            json!({ "addon": "999.0.0", "agent": "999.0.0" })
        );

        // The next pass, the same answers: nothing said again.
        let http = ScriptedTransport::new();
        http.json(200, ingest).json(200, beat).json(200, prices);
        let (r, events) = run_seeing(&e, &http, true);
        assert!(r.ok(), "{r:?}");
        assert!(events
            .iter()
            .any(|ev| matches!(ev, WatchEvent::Support { changed: false, .. })));
        assert!(!events
            .iter()
            .any(|ev| matches!(ev, WatchEvent::ContributionsPaused { .. })));
    }

    #[test]
    fn an_unsupported_version_fails_in_the_api_s_words_and_the_heartbeat_still_goes() {
        let mut e = env(&[("FAKEACCOUNT1", FIXTURE)]);
        e.store = MemoryStore::with_access("tok", NOW + 100_000);
        let addon = addon_at(&e, "0.3.1");
        let http = ScriptedTransport::new();
        http.json(200, json!({ "ok": false, "error": { "code": "AGENT_UNSUPPORTED",
                "message": support::UNSUPPORTED_SENTENCE, "detail": judged("unsupported") } }))
            .json(200, json!({ "ok": true, "data": { "host": "h", "seenAt": NOW, "support": judged("unsupported") } }));
        let (r, events) = run_seeing(&e, &http, false);
        assert_eq!(r.failure, Some(Fix::Update));
        assert!(
            matches!(&r.accounts[0].outcome, Outcome::Failed(m) if m == support::UNSUPPORTED_SENTENCE)
        );
        assert!(r.heartbeat.is_ok());
        // No price call: the API would refuse it too.
        assert_eq!(http.urls().len(), 2, "{:?}", http.urls());
        let advice = events.iter().find_map(|ev| match ev {
            WatchEvent::Error { advice, fix, .. } => Some((advice.clone(), *fix)),
            _ => None,
        });
        assert_eq!(
            advice,
            Some((support::UNSUPPORTED_SENTENCE.to_string(), Fix::Update))
        );
        // One line in game, the sentence, as an error; the file not pushed, so tried again.
        let a = agent_status(&addon);
        assert_eq!(a["support"]["band"], json!("unsupported"));
        assert_eq!(
            a["messages"],
            json!([{ "at": NOW, "level": "error", "text": support::UNSUPPORTED_SENTENCE }])
        );
        let st = State::load(&e.state);
        assert_eq!(st.accounts.values().next().unwrap().pushed_mtime_ns, None);
    }

    #[test]
    fn no_judgment_is_current_and_an_unreachable_api_keeps_the_last_one() {
        let mut e = env(&[("FAKEACCOUNT1", FIXTURE)]);
        e.store = MemoryStore::with_access("tok", NOW + 100_000);
        let addon = addon_at(&e, "0.3.9");
        // An API that says nothing about versions (before section 18): nothing kept, nothing said.
        let http = ScriptedTransport::new();
        http.json(200, ingested()).json(200, ok()).json(
            200,
            json!({ "ok": true, "data": { "flavor": "wowf", "items": [] } }),
        );
        let (r, events) = run_seeing(&e, &http, false);
        assert!(r.ok(), "{r:?}");
        assert!(!events
            .iter()
            .any(|ev| matches!(ev, WatchEvent::Support { .. })));
        assert_eq!(KnownSupport::load(&e.state), None);
        assert!(agent_status(&addon).get("support").is_none());

        // Judged behind, then the API is unreachable: the last judgment stays, to show.
        let http = ScriptedTransport::new();
        http.json(
            200,
            json!({ "ok": true, "data": { "support": judged("behind") } }),
        );
        run_seeing(&e, &http, false);
        let http = ScriptedTransport::new();
        http.fail("a").fail("b").fail("c");
        let (r, _) = run_seeing(&e, &http, false);
        assert_eq!(r.failure, Some(Fix::Network));
        assert_eq!(
            KnownSupport::load(&e.state).unwrap().support.band,
            support::Band::Behind
        );
        assert_eq!(agent_status(&addon)["support"]["band"], json!("behind"));
    }
}
