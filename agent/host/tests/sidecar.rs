//! The sidecar runtime driven against a fake sidecar: this test binary itself, run again with
//! `--exact fake_sidecar` and `RYNA_FAKE_SIDECAR=<mode>`, so it needs no Node and runs wherever
//! the tests do. libtest's own "running 1 test" line on stdout is the noise the agent must send to
//! the debug log and nowhere else.

use std::io::{BufRead, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use ryna_agent::auth::{AccessToken, AuthError, TokenStore};
use ryna_agent::config::Config;
use ryna_agent::core::{Core, CoreContext, CoreControl, CoreManifest, TokenProvider};
use ryna_agent::sidecar::{SidecarCore, SidecarEvent, Timing, NEEDS_NEWER};
use serde_json::{json, Value};

const MODE: &str = "RYNA_FAKE_SIDECAR";

/// The fake: what a sidecar does, by mode. Not a test unless `RYNA_FAKE_SIDECAR` is set.
#[test]
fn fake_sidecar() {
    let Ok(mode) = std::env::var(MODE) else {
        return;
    };
    let out = |v: Value| {
        let mut o = std::io::stdout().lock();
        writeln!(o, "{v}").unwrap();
        o.flush().unwrap();
    };
    let protocol = if mode == "badproto" { 99 } else { 1 };
    let mut passes = 0;
    for line in std::io::stdin().lock().lines() {
        let Ok(line) = line else { break };
        let v: Value = serde_json::from_str(&line).unwrap_or(Value::Null);
        match v["type"].as_str() {
            Some("hello") => {
                println!("noise that isn't a message");
                out(
                    json!({ "type": "ready", "protocol": protocol, "core": "fake", "version": "9.9.9" }),
                );
                out(
                    json!({ "type": "event", "event": { "kind": "info", "message":
                    format!("hello api={} state={} agent={} protocol={}", v["api"], v["stateDir"], v["agent"], v["protocol"]) } }),
                );
                match mode.as_str() {
                    "crash" => std::process::exit(3),
                    "error" => out(
                        json!({ "type": "error", "message": "no saves", "fix": "open the game" }),
                    ),
                    _ => {}
                }
            }
            Some("sync") => {
                passes += 1;
                eprintln!("got token {}", v["token"].as_str().unwrap_or(""));
                out(
                    json!({ "type": "event", "event": { "kind": "pushing", "row": "w1", "message": format!("pass {passes} force={}", v["force"]) } }),
                );
                if mode == "slow" {
                    std::thread::sleep(Duration::from_millis(400));
                }
                out(
                    json!({ "type": "status", "rows": [{ "label": "World 1", "value": "pushed", "chip": "pushed" }] }),
                );
                out(
                    json!({ "type": "event", "event": { "kind": "pushed", "row": "w1", "message": "World 1 pushed" } }),
                );
                out(json!({ "type": "done", "ok": true }));
            }
            Some("stop") if mode != "stubborn" => break,
            _ => {}
        }
    }
    if mode == "stubborn" {
        std::thread::sleep(Duration::from_secs(60));
    }
    std::process::exit(0);
}

struct NoStore;
impl TokenStore for NoStore {
    fn refresh_token(&self) -> Result<Option<String>, String> {
        Ok(None)
    }
    fn set_refresh_token(&self, _: &str) -> Result<(), String> {
        Ok(())
    }
    fn access_token(&self) -> Option<AccessToken> {
        None
    }
    fn set_access_token(&self, _: &AccessToken) -> Result<(), String> {
        Ok(())
    }
    fn clear(&self) -> Result<(), String> {
        Ok(())
    }
}

/// Hands out a new token for each ask, so a test can see each `sync` fetched its own.
struct Tokens {
    config: Config,
    asked: Mutex<u32>,
}

impl TokenProvider for Tokens {
    fn config(&self) -> &Config {
        &self.config
    }
    fn store(&self) -> Box<dyn TokenStore> {
        Box::new(NoStore)
    }
    fn access_token(&self) -> Result<Option<String>, AuthError> {
        let mut n = self.asked.lock().unwrap();
        *n += 1;
        Ok(Some(format!("tok-SECRET-{n}")))
    }
}

struct Harness {
    events: Arc<Mutex<Vec<SidecarEvent>>>,
    log: Arc<Mutex<Vec<String>>>,
    tokens: Arc<Tokens>,
    control: CoreControl,
    thread: Option<std::thread::JoinHandle<Result<(), String>>>,
    _state: tempfile::TempDir,
}

fn timing() -> Timing {
    Timing {
        sync_every: Duration::from_secs(3600),
        stop_grace: Duration::from_millis(500),
        restart_gap: Duration::from_millis(100),
        ready_wait: Duration::from_secs(20),
        max_failures: 3,
    }
}

fn start(mode: &str, timing: Timing) -> Harness {
    let exe = std::env::current_exe().unwrap();
    let dir: PathBuf = exe.parent().unwrap().to_path_buf();
    let bin = exe.file_name().unwrap().to_string_lossy().into_owned();
    let manifest = CoreManifest::parse(
        &json!({ "id": "fake", "name": "Fake", "version": "9.9.9",
            "runtime": { "kind": "sidecar", "bin": bin, "args": ["--exact", "fake_sidecar", "--nocapture", "--test-threads=1"] },
            "resource": "https://api.test/agent", "scopes": ["fake:ingest"] })
        .to_string(),
    )
    .unwrap();
    let log = Arc::new(Mutex::new(Vec::new()));
    let sink = log.clone();
    let core = SidecarCore::new(manifest, dir, "https://api.test")
        .with_timing(timing)
        .with_env(vec![(MODE.into(), mode.into())])
        .with_debug_log(Arc::new(move |l: &str| {
            sink.lock().unwrap().push(l.to_string())
        }));
    let state = tempfile::tempdir().unwrap();
    let tokens = Arc::new(Tokens {
        config: Config {
            issuer: "https://idp.test".into(),
            api: "https://api.test".into(),
            client_id: "c".into(),
            resource: "https://api.test/agent".into(),
        },
        asked: Mutex::new(0),
    });
    let ctx = CoreContext {
        state_dir: state.path().to_path_buf(),
        tokens: tokens.clone(),
        device_key: None,
        game_folder: None,
    };
    let control = CoreControl::new();
    let events = Arc::new(Mutex::new(Vec::new()));
    let (c, e) = (control.clone(), events.clone());
    let thread = std::thread::spawn(move || {
        core.start(&ctx, &c, &mut |ev| e.lock().unwrap().push(ev))
            .map_err(|e| e.to_string())
    });
    Harness {
        events,
        log,
        tokens,
        control,
        thread: Some(thread),
        _state: state,
    }
}

impl Harness {
    fn wait_for(&self, what: &str, f: impl Fn(&[SidecarEvent]) -> bool) {
        let deadline = Instant::now() + Duration::from_secs(30);
        while Instant::now() < deadline {
            if f(&self.events.lock().unwrap()) {
                return;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        panic!("{what}: {:#?}", self.events.lock().unwrap());
    }

    fn count(&self, f: impl Fn(&SidecarEvent) -> bool) -> usize {
        self.events.lock().unwrap().iter().filter(|e| f(e)).count()
    }

    fn join(&mut self) -> Result<(), String> {
        self.thread.take().unwrap().join().unwrap()
    }
}

fn passes(e: &SidecarEvent) -> bool {
    matches!(e, SidecarEvent::PassFinished { ok: true })
}

#[test]
fn hello_ready_sync_and_stop() {
    let mut h = start("normal", timing());
    h.wait_for("a first pass", |e| e.iter().any(passes));
    {
        let events = h.events.lock().unwrap();
        assert_eq!(
            events[0],
            SidecarEvent::Started {
                version: "9.9.9".into()
            }
        );
        let SidecarEvent::Event { kind, message, .. } = &events[1] else {
            panic!("{events:#?}")
        };
        assert_eq!(kind, "info");
        assert!(message.contains("api=\"https://api.test\""), "{message}");
        assert!(message.contains("agent=\"ryna-agent/"), "{message}");
        assert!(message.contains("protocol=1"), "{message}");
        assert!(
            message.contains(&format!(
                "state={}",
                serde_json::to_string(h._state.path()).unwrap()
            )),
            "{message}"
        );
        assert!(events.iter().any(|e| matches!(e, SidecarEvent::Event { kind, row, message }
            if kind == "pushing" && row.as_deref() == Some("w1") && message == "pass 1 force=false")));
        assert!(events
            .iter()
            .any(|e| matches!(e, SidecarEvent::Status { rows } if rows[0].label == "World 1")));
        assert!(events
            .iter()
            .any(|e| matches!(e, SidecarEvent::Event { kind, .. } if kind == "pushed")));
    }
    // Sync now: a second pass, forced, with a token fetched for it.
    h.control.sync();
    h.wait_for("a second pass", |e| {
        e.iter().filter(|e| passes(e)).count() == 2
    });
    assert_eq!(*h.tokens.asked.lock().unwrap(), 2);
    assert!(
        h.count(
            |e| matches!(e, SidecarEvent::Event { message, .. } if message == "pass 2 force=true")
        ) == 1
    );
    let stopping = Instant::now();
    h.control.stop();
    assert_eq!(h.join(), Ok(()));
    assert!(stopping.elapsed() < Duration::from_secs(5));
    // Noise and stderr went to the debug log, and no token went anywhere but the program.
    let log = h.log.lock().unwrap().join("\n");
    // (libtest's "test fake_sidecar ... " has no newline, so the noise shares its line.)
    assert!(
        h.log
            .lock()
            .unwrap()
            .iter()
            .any(|l| l.starts_with("stdout: ") && l.ends_with("noise that isn't a message")),
        "{log}"
    );
    assert!(log.contains("stderr: got token <redacted>"), "{log}");
    assert!(!log.contains("tok-SECRET"), "{log}");
    let shown = serde_json::to_string(&*h.events.lock().unwrap()).unwrap();
    assert!(!shown.contains("tok-SECRET"), "{shown}");
}

#[test]
fn a_sync_asked_for_during_a_pass_is_sent_once_it_ends() {
    let mut h = start("slow", timing());
    h.wait_for("the first pass under way", |e| {
        e.iter()
            .any(|e| matches!(e, SidecarEvent::Event { kind, .. } if kind == "pushing"))
    });
    h.control.sync();
    h.control.sync();
    h.wait_for("the held pass", |e| {
        e.iter().filter(|e| passes(e)).count() == 2
    });
    std::thread::sleep(Duration::from_millis(700));
    // Two syncs asked for during one pass are one more pass, not two.
    assert_eq!(h.count(passes), 2);
    h.control.stop();
    assert_eq!(h.join(), Ok(()));
}

#[test]
fn an_unknown_protocol_stops_the_core() {
    let mut h = start("badproto", timing());
    let e = h.join().unwrap_err();
    assert_eq!(e, NEEDS_NEWER);
    assert_eq!(h.count(|e| matches!(e, SidecarEvent::Started { .. })), 0);
    assert_eq!(
        h.count(|e| matches!(e, SidecarEvent::Stopped { message, .. } if message == NEEDS_NEWER)),
        1
    );
    // It never got a token.
    assert_eq!(*h.tokens.asked.lock().unwrap(), 0);
}

#[test]
fn three_exits_in_a_row_stop_the_core_and_restarts_are_paced() {
    let started = Instant::now();
    let mut h = start("crash", timing());
    assert!(h.join().is_err());
    assert_eq!(h.count(|e| matches!(e, SidecarEvent::Started { .. })), 3);
    assert_eq!(h.count(|e| matches!(e, SidecarEvent::Exited { .. })), 2);
    assert_eq!(h.count(|e| matches!(e, SidecarEvent::Stopped { .. })), 1);
    // Two restarts, each at least the gap after the start before it.
    assert!(started.elapsed() >= Duration::from_millis(200));
}

#[test]
fn an_error_stops_the_core_with_its_words() {
    let mut h = start("error", timing());
    assert_eq!(h.join().unwrap_err(), "no saves");
    assert_eq!(
        h.count(|e| matches!(e, SidecarEvent::Stopped { message, fix }
            if message == "no saves" && fix.as_deref() == Some("open the game"))),
        1
    );
}

#[test]
fn one_that_wont_stop_is_killed_after_the_grace() {
    let mut h = start("stubborn", timing());
    h.wait_for("a first pass", |e| e.iter().any(passes));
    let stopping = Instant::now();
    h.control.stop();
    assert_eq!(h.join(), Ok(()));
    let took = stopping.elapsed();
    assert!(took >= Duration::from_millis(400), "{took:?}");
    assert!(took < Duration::from_secs(10), "{took:?}");
    assert!(h.log.lock().unwrap().iter().any(|l| l.contains("killed")));
}
