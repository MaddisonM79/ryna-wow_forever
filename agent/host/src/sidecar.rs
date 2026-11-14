//! A core that is a separate program (docs/ryna-agent-cores-mods.md 13.4), driven through the
//! same [`Core`] interface as a built-in one.
//!
//! The agent starts the program when the core starts and stops it when the core stops; it is
//! never left running. They speak line-delimited JSON, one object a line, UTF-8:
//!
//! - to its stdin: `hello` (once), `sync` (with the access token, for that pass only), `stop`;
//! - from its stdout: `ready` (once, naming the protocol), `event`, `status`, `done` (the pass a
//!   `sync` asked for has ended), `error` (it can't run at all).
//!
//! The agent paces: a `sync` on its timer (five minutes), and at once on Sync now
//! ([`CoreControl::sync`]); one asked for during a pass is sent when that pass ends. The token is
//! fetched fresh for each `sync` and never written anywhere else: not to the debug log (every
//! line logged has each token sent so far replaced), not in a debug print.
//!
//! Stopping: `stop`, then stdin closed (which the program also takes as `stop`); one that hasn't
//! exited five seconds later is killed. One that exits by itself is started again, at most once a
//! minute; three exits in a row without a good pass between stop the core with its last words. A
//! `ready` naming a protocol this agent doesn't know stops it with "this core needs a newer Ryna".
//! Anything on stdout that isn't a message, and all of stderr, goes to the debug log, never the
//! window.

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use serde::Serialize;
use serde_json::{json, Value};

use crate::cancel::Waited;
use crate::core::{
    Activity, Chip, Core, CoreContext, CoreControl, CoreHeartbeat, CoreManifest, Phase, Runtime,
    StatusRow,
};
use crate::fix::Fix;

/// The protocol this agent speaks.
pub const PROTOCOL: u64 = 1;
/// The words for a core whose program speaks a protocol this agent doesn't know.
pub const NEEDS_NEWER: &str = "this core needs a newer Ryna";
/// The debug log's file in the core's state folder.
pub const DEBUG_LOG: &str = "sidecar.log";
const DEBUG_LOG_MAX: u64 = 1024 * 1024;
const REDACTED: &str = "<redacted>";
/// How often the loop looks at the program, the control and the timer.
const TICK: Duration = Duration::from_millis(50);

/// The sidecar's clocks: the spec's, or shorter ones for tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Timing {
    /// Between `sync`s on the timer.
    pub sync_every: Duration,
    /// From `stop` to a kill.
    pub stop_grace: Duration,
    /// At most one start in this long.
    pub restart_gap: Duration,
    /// How long a started program has to say `ready`.
    pub ready_wait: Duration,
    /// Exits in a row (with no good pass between) that stop the core.
    pub max_failures: u32,
}

impl Default for Timing {
    fn default() -> Self {
        Timing {
            sync_every: Duration::from_secs(5 * 60),
            stop_grace: Duration::from_secs(5),
            restart_gap: Duration::from_secs(60),
            ready_wait: Duration::from_secs(30),
            max_failures: 3,
        }
    }
}

/// What a sidecar core reports, as the window receives it (`{"event": "pass", ...}`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "event", rename_all = "camelCase")]
pub enum SidecarEvent {
    /// The program said `ready`.
    Started { version: String },
    /// One of its `event`s: `kind` is `waiting`, `pushing`, `pushed`, `error` or `info`; `row` is
    /// its key for the thing it is about (a world).
    Event {
        kind: String,
        row: Option<String>,
        message: String,
    },
    /// Its status card's rows changed.
    Status { rows: Vec<StatusRow> },
    /// The pass a `sync` asked for has ended.
    PassFinished { ok: bool },
    /// A pass couldn't be asked for (no sign-in, the token couldn't be had).
    #[serde(rename_all = "camelCase")]
    Failed {
        message: String,
        fix: Fix,
        advice: String,
    },
    /// The program exited by itself; it is started again within a minute.
    #[serde(rename_all = "camelCase")]
    Exited { message: String },
    /// The core has stopped for good (until it is started again): its last words, and what fixes
    /// them when the program said.
    #[serde(rename_all = "camelCase")]
    Stopped {
        message: String,
        fix: Option<String>,
    },
}

/// Why a sidecar core couldn't go on.
#[derive(Debug, thiserror::Error)]
pub enum SidecarError {
    #[error("{0}")]
    Stopped(String),
}

/// What the debug log is handed: one line at a time, already free of tokens.
pub type DebugSink = Arc<dyn Fn(&str) + Send + Sync>;

/// A sidecar core: its manifest (from the unpacked package), the folder it runs in, and where the
/// API is.
pub struct SidecarCore {
    manifest: CoreManifest,
    package_dir: PathBuf,
    api: String,
    agent: String,
    timing: Timing,
    env: Vec<(String, String)>,
    debug: Option<DebugSink>,
    rows: Arc<Mutex<Vec<StatusRow>>>,
}

impl SidecarCore {
    /// `manifest` must be a sidecar's; `package_dir` is where its `bin` and `args` are.
    pub fn new(manifest: CoreManifest, package_dir: PathBuf, api: &str) -> Self {
        SidecarCore {
            manifest,
            package_dir,
            api: api.to_string(),
            agent: crate::user_agent(),
            timing: Timing::default(),
            env: Vec::new(),
            debug: None,
            rows: Arc::new(Mutex::new(Vec::new())),
        }
    }

    #[must_use]
    pub fn with_timing(mut self, timing: Timing) -> Self {
        self.timing = timing;
        self
    }

    /// Variables set for the program, on top of the agent's own environment.
    #[must_use]
    pub fn with_env(mut self, env: Vec<(String, String)>) -> Self {
        self.env = env;
        self
    }

    /// Where the debug log goes (by default `sidecar.log` in the core's state folder).
    #[must_use]
    pub fn with_debug_log(mut self, sink: DebugSink) -> Self {
        self.debug = Some(sink);
        self
    }

    pub fn package_dir(&self) -> &Path {
        &self.package_dir
    }

    /// The rows of its status card, as its last `status` laid them out.
    pub fn last_rows(&self) -> Vec<StatusRow> {
        self.rows().clone()
    }

    fn rows(&self) -> MutexGuard<'_, Vec<StatusRow>> {
        self.rows.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn sink(&self, ctx: &CoreContext) -> DebugSink {
        self.debug
            .clone()
            .unwrap_or_else(|| file_log(ctx.state_dir.join(DEBUG_LOG)))
    }

    fn command(&self) -> Result<Command, String> {
        let Runtime::Sidecar { bin, args } = &self.manifest.runtime else {
            return Err(format!("{} isn't a sidecar core", self.manifest.id));
        };
        let path = self.package_dir.join(bin);
        if !path.is_file() {
            return Err(format!("{} isn't in the core's package", path.display()));
        }
        let mut cmd = Command::new(path);
        cmd.args(args)
            .current_dir(&self.package_dir)
            .envs(self.env.iter().map(|(k, v)| (k, v)))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        Ok(cmd)
    }
}

/// A debug log file, kept under a megabyte (the one before it is kept as `.1`).
pub fn file_log(path: PathBuf) -> DebugSink {
    let lock = Arc::new(Mutex::new(()));
    Arc::new(move |line: &str| {
        let _held = lock.lock().unwrap_or_else(|e| e.into_inner());
        if std::fs::metadata(&path).is_ok_and(|m| m.len() > DEBUG_LOG_MAX) {
            let _ = std::fs::rename(&path, path.with_extension("log.1"));
        }
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
        {
            let _ = writeln!(f, "{} {line}", crate::util::now_secs());
        }
    })
}

/// The tokens sent so far this run, for taking out of every line the debug log is given.
#[derive(Clone, Default)]
struct Secrets(Arc<Mutex<Vec<String>>>);

impl Secrets {
    fn add(&self, token: &str) {
        let mut v = self.0.lock().unwrap_or_else(|e| e.into_inner());
        if !token.is_empty() && !v.iter().any(|t| t == token) {
            v.push(token.to_string());
            if v.len() > 8 {
                v.remove(0);
            }
        }
    }

    fn scrub(&self, line: &str) -> String {
        let v = self.0.lock().unwrap_or_else(|e| e.into_inner());
        v.iter()
            .fold(line.to_string(), |l, t| l.replace(t.as_str(), REDACTED))
    }
}

/// A `sync` line's contents. Its `Debug` never shows the token.
struct SyncLine<'a> {
    token: &'a str,
    force: bool,
}

impl std::fmt::Debug for SyncLine<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SyncLine")
            .field("token", &REDACTED)
            .field("force", &self.force)
            .finish()
    }
}

impl SyncLine<'_> {
    fn json(&self) -> Value {
        json!({ "type": "sync", "token": self.token, "force": self.force })
    }
}

enum Line {
    Out(String),
    Closed,
}

/// How one run of the program ended.
enum Ended {
    /// The agent stopped it.
    Asked,
    /// It can't run at all: stop the core.
    Fatal(String, Option<String>),
    /// It exited (or never got going): start it again, unless that's too many.
    Exited(String),
}

/// One running program and the agent's side of the pipes.
struct Run<'a> {
    core: &'a SidecarCore,
    child: Child,
    stdin: Option<ChildStdin>,
    lines: Receiver<Line>,
    log: DebugSink,
    secrets: Secrets,
}

impl Run<'_> {
    fn debug(&self, what: &str, line: &str) {
        (self.log)(&self.secrets.scrub(&format!("{what}: {line}")));
    }

    fn send(&mut self, v: &Value) -> bool {
        let Some(stdin) = self.stdin.as_mut() else {
            return false;
        };
        let mut line = v.to_string();
        line.push('\n');
        stdin
            .write_all(line.as_bytes())
            .and_then(|()| stdin.flush())
            .is_ok()
    }

    /// `stop`, stdin closed, then a kill once the grace has passed.
    fn stop(&mut self) {
        self.send(&json!({ "type": "stop" }));
        self.stdin = None;
        let deadline = Instant::now() + self.core.timing.stop_grace;
        while Instant::now() < deadline {
            if let Ok(Some(_)) = self.child.try_wait() {
                return;
            }
            std::thread::sleep(TICK);
        }
        self.debug("agent", "the program didn't stop in time: killed");
        let _ = self.child.kill();
        let _ = self.child.wait();
    }

    /// The program closed its stdout: wait for it to exit (killing it after the grace).
    fn exit_words(&mut self) -> String {
        self.stdin = None;
        let deadline = Instant::now() + self.core.timing.stop_grace;
        loop {
            match self.child.try_wait() {
                Ok(Some(status)) => return format!("it exited ({status})"),
                Ok(None) if Instant::now() < deadline => std::thread::sleep(TICK),
                _ => {
                    let _ = self.child.kill();
                    let _ = self.child.wait();
                    return "it closed its output and was stopped".into();
                }
            }
        }
    }
}

/// A line from the program, if it is one of the protocol's messages.
fn message(line: &str) -> Option<(String, Value)> {
    let v: Value = serde_json::from_str(line.trim()).ok()?;
    let kind = v.get("type")?.as_str()?.to_string();
    Some((kind, v))
}

fn text(v: &Value, k: &str) -> Option<String> {
    v.get(k).and_then(Value::as_str).map(str::to_string)
}

fn chip(v: Option<&str>) -> Option<Chip> {
    match v? {
        "pushed" => Some(Chip::Pushed),
        "waiting" => Some(Chip::Waiting),
        "update" => Some(Chip::Update),
        "error" => Some(Chip::Error),
        _ => None,
    }
}

fn status_rows(v: &Value) -> Vec<StatusRow> {
    v.get("rows")
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .filter_map(|r| {
                    Some(StatusRow {
                        label: text(r, "label")?,
                        value: text(r, "value").unwrap_or_default(),
                        chip: chip(r.get("chip").and_then(Value::as_str)),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

const KINDS: [&str; 5] = ["waiting", "pushing", "pushed", "error", "info"];

impl SidecarCore {
    fn spawn(&self, log: &DebugSink, secrets: &Secrets) -> Result<Run<'_>, String> {
        let mut child = self
            .command()?
            .spawn()
            .map_err(|e| format!("couldn't start {}: {e}", self.manifest.id))?;
        let (tx, rx) = mpsc::channel();
        if let Some(out) = child.stdout.take() {
            std::thread::spawn(move || {
                for line in BufReader::new(out).lines() {
                    match line {
                        Ok(l) => {
                            if tx.send(Line::Out(l)).is_err() {
                                return;
                            }
                        }
                        Err(_) => break,
                    }
                }
                let _ = tx.send(Line::Closed);
            });
        }
        if let Some(err) = child.stderr.take() {
            let (log, secrets) = (log.clone(), secrets.clone());
            std::thread::spawn(move || {
                for line in BufReader::new(err).lines().map_while(Result::ok) {
                    log(&secrets.scrub(&format!("stderr: {line}")));
                }
            });
        }
        let stdin = child.stdin.take();
        Ok(Run {
            core: self,
            child,
            stdin,
            lines: rx,
            log: log.clone(),
            secrets: secrets.clone(),
        })
    }

    /// Start the program, say hello, wait for ready, then pace it until it ends or is stopped.
    fn run_once(
        &self,
        ctx: &CoreContext,
        control: &CoreControl,
        events: &mut dyn FnMut(SidecarEvent),
        good_pass: &mut bool,
        log: &DebugSink,
        secrets: &Secrets,
    ) -> Ended {
        let mut run = match self.spawn(log, secrets) {
            Ok(r) => r,
            Err(e) => return Ended::Exited(e),
        };
        let hello = json!({
            "type": "hello",
            "protocol": PROTOCOL,
            "api": self.api,
            "stateDir": ctx.state_dir,
            "agent": self.agent,
            "gameFolder": ctx.game_folder,
        });
        if !run.send(&hello) {
            return Ended::Exited(run.exit_words());
        }
        // Ready, or not.
        let deadline = Instant::now() + self.timing.ready_wait;
        loop {
            if control.is_stopped() {
                run.stop();
                return Ended::Asked;
            }
            if Instant::now() >= deadline {
                run.stop();
                return Ended::Exited("it didn't say it was ready".into());
            }
            match run.lines.recv_timeout(TICK) {
                Ok(Line::Out(l)) => match message(&l) {
                    Some((kind, v)) if kind == "ready" => {
                        let protocol = v.get("protocol").and_then(Value::as_u64);
                        if protocol != Some(PROTOCOL) {
                            run.debug("agent", &format!("ready with protocol {protocol:?}"));
                            run.stop();
                            return Ended::Fatal(NEEDS_NEWER.into(), None);
                        }
                        if text(&v, "core").as_deref() != Some(self.manifest.id.as_str()) {
                            run.debug("agent", &format!("ready names another core: {l}"));
                        }
                        events(SidecarEvent::Started {
                            version: text(&v, "version").unwrap_or_default(),
                        });
                        break;
                    }
                    Some((kind, v)) if kind == "error" => {
                        run.stop();
                        return Ended::Fatal(
                            text(&v, "message").unwrap_or_else(|| "it can't run".into()),
                            text(&v, "fix"),
                        );
                    }
                    _ => run.debug("stdout", &l),
                },
                Ok(Line::Closed) | Err(RecvTimeoutError::Disconnected) => {
                    return Ended::Exited(run.exit_words())
                }
                Err(RecvTimeoutError::Timeout) => {}
            }
        }
        // Pacing: a sync now, then on the timer, on Sync now, and one held while a pass runs.
        let mut in_pass = false;
        let mut held: Option<bool> = Some(false);
        let mut next = Instant::now() + self.timing.sync_every;
        loop {
            loop {
                match run.lines.try_recv() {
                    Ok(Line::Out(l)) => {
                        let Some((kind, v)) = message(&l) else {
                            run.debug("stdout", &l);
                            continue;
                        };
                        match kind.as_str() {
                            "event" => {
                                let e = v.get("event").cloned().unwrap_or(Value::Null);
                                let k = text(&e, "kind").unwrap_or_default();
                                if !KINDS.contains(&k.as_str()) {
                                    run.debug("stdout", &l);
                                    continue;
                                }
                                events(SidecarEvent::Event {
                                    kind: k,
                                    row: text(&e, "row"),
                                    message: text(&e, "message").unwrap_or_default(),
                                });
                            }
                            "status" => {
                                let rows = status_rows(&v);
                                rows.clone_into(&mut self.rows());
                                events(SidecarEvent::Status { rows });
                            }
                            "done" => {
                                let ok = v.get("ok").and_then(Value::as_bool).unwrap_or(false);
                                in_pass = false;
                                *good_pass |= ok;
                                events(SidecarEvent::PassFinished { ok });
                            }
                            "error" => {
                                run.stop();
                                return Ended::Fatal(
                                    text(&v, "message").unwrap_or_else(|| "it can't run".into()),
                                    text(&v, "fix"),
                                );
                            }
                            _ => run.debug("stdout", &l),
                        }
                    }
                    Ok(Line::Closed) | Err(mpsc::TryRecvError::Disconnected) => {
                        return Ended::Exited(run.exit_words());
                    }
                    Err(mpsc::TryRecvError::Empty) => break,
                }
            }
            if !in_pass {
                if let Some(force) = held.take() {
                    match ctx.tokens.access_token() {
                        Ok(Some(token)) => {
                            run.secrets.add(&token);
                            let sync = SyncLine {
                                token: &token,
                                force,
                            };
                            if !run.send(&sync.json()) {
                                return Ended::Exited(run.exit_words());
                            }
                            in_pass = true;
                        }
                        Ok(None) => events(SidecarEvent::Failed {
                            message: "not signed in".into(),
                            fix: Fix::SignIn,
                            advice: Fix::SignIn.message("not signed in"),
                        }),
                        Err(e) => {
                            let message = e.to_string();
                            events(SidecarEvent::Failed {
                                advice: e.fix().message(&message),
                                fix: e.fix(),
                                message,
                            });
                        }
                    }
                    next = Instant::now() + self.timing.sync_every;
                }
            }
            let wait = next.saturating_duration_since(Instant::now()).min(TICK);
            match control.token().wait_for(wait) {
                Waited::Cancelled => {
                    run.stop();
                    return Ended::Asked;
                }
                Waited::Woken => held = Some(true),
                Waited::TimedOut if Instant::now() >= next => {
                    held = Some(held.unwrap_or(false));
                    next = Instant::now() + self.timing.sync_every;
                }
                Waited::TimedOut => {}
            }
        }
    }
}

impl Core for SidecarCore {
    type Event = SidecarEvent;
    type Error = SidecarError;

    fn manifest(&self) -> &CoreManifest {
        &self.manifest
    }

    fn start(
        &self,
        ctx: &CoreContext,
        control: &CoreControl,
        events: &mut dyn FnMut(SidecarEvent),
    ) -> Result<(), SidecarError> {
        let log = self.sink(ctx);
        let secrets = Secrets::default();
        let mut failures = 0u32;
        let mut last_start: Option<Instant> = None;
        loop {
            if control.is_stopped() {
                return Ok(());
            }
            if let Some(at) = last_start {
                let wait = self.timing.restart_gap.saturating_sub(at.elapsed());
                if !wait.is_zero() && control.token().wait(wait) {
                    return Ok(());
                }
            }
            last_start = Some(Instant::now());
            let mut good = false;
            match self.run_once(ctx, control, events, &mut good, &log, &secrets) {
                Ended::Asked => return Ok(()),
                Ended::Fatal(message, fix) => {
                    events(SidecarEvent::Stopped {
                        message: message.clone(),
                        fix,
                    });
                    return Err(SidecarError::Stopped(message));
                }
                Ended::Exited(message) => {
                    log(&secrets.scrub(&format!("agent: {message}")));
                    failures = if good { 1 } else { failures + 1 };
                    if failures >= self.timing.max_failures {
                        events(SidecarEvent::Stopped {
                            message: message.clone(),
                            fix: None,
                        });
                        return Err(SidecarError::Stopped(message));
                    }
                    events(SidecarEvent::Exited { message });
                }
            }
        }
    }

    fn activity(&self, event: &SidecarEvent) -> Activity {
        activity(event)
    }

    fn failed(&self, message: String, fix: Fix) -> SidecarEvent {
        SidecarEvent::Failed {
            advice: fix.message(&message),
            message,
            fix,
        }
    }

    fn status_rows(&self, _ctx: &CoreContext) -> Vec<StatusRow> {
        self.rows().clone()
    }

    fn heartbeat(&self, _ctx: &CoreContext) -> CoreHeartbeat {
        // The program sends its own heartbeat after each pass.
        CoreHeartbeat {
            core: self.manifest.id.clone(),
            version: self.manifest.version.clone(),
            fields: json!({}),
        }
    }
}

/// What the agent's state takes from a sidecar's event: its `event` kinds become the rows'
/// phases, an `error` kind or a stop the failure to show, a good pass clears it.
pub fn activity(event: &SidecarEvent) -> Activity {
    match event {
        SidecarEvent::Event { kind, row, message } => {
            let phase = match kind.as_str() {
                "waiting" => Some(Phase::Waiting),
                "pushing" => Some(Phase::Pushing),
                "pushed" => Some(Phase::Pushed),
                "error" => Some(Phase::Error),
                _ => None,
            };
            Activity {
                row: row.clone().zip(phase),
                error: (kind == "error").then(|| message.clone()),
                clears_error: false,
            }
        }
        SidecarEvent::PassFinished { ok: true } => Activity {
            clears_error: true,
            ..Activity::default()
        },
        SidecarEvent::Failed { advice, .. } => Activity {
            error: Some(advice.clone()),
            ..Activity::default()
        },
        SidecarEvent::Stopped { message, .. } => Activity {
            error: Some(message.clone()),
            ..Activity::default()
        },
        _ => Activity::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_sync_line_carries_the_token_and_its_debug_never_does() {
        let s = SyncLine {
            token: "secret-token",
            force: true,
        };
        assert_eq!(
            s.json(),
            json!({ "type": "sync", "token": "secret-token", "force": true })
        );
        assert!(!format!("{s:?}").contains("secret"));
        let secrets = Secrets::default();
        secrets.add("secret-token");
        assert_eq!(
            secrets.scrub("stderr: got secret-token twice: secret-token"),
            "stderr: got <redacted> twice: <redacted>"
        );
    }

    #[test]
    fn events_become_rows_and_errors() {
        let e = |kind: &str| SidecarEvent::Event {
            kind: kind.into(),
            row: Some("w1".into()),
            message: "World 1 pushed".into(),
        };
        assert_eq!(
            activity(&e("pushed")).row,
            Some(("w1".into(), Phase::Pushed))
        );
        assert_eq!(activity(&e("info")).row, None);
        assert_eq!(
            activity(&e("error")).error.as_deref(),
            Some("World 1 pushed")
        );
        assert!(activity(&SidecarEvent::PassFinished { ok: true }).clears_error);
        assert!(!activity(&SidecarEvent::PassFinished { ok: false }).clears_error);
        assert_eq!(
            activity(&SidecarEvent::Stopped {
                message: NEEDS_NEWER.into(),
                fix: None
            })
            .error
            .as_deref(),
            Some(NEEDS_NEWER)
        );
        assert_eq!(
            serde_json::to_value(SidecarEvent::PassFinished { ok: true }).unwrap(),
            json!({ "event": "passFinished", "ok": true })
        );
    }

    #[test]
    fn status_rows_read_as_the_window_draws_them() {
        let v = json!({ "type": "status", "rows": [
            { "label": "World 1", "value": "saved 2 min ago, pushed", "chip": "pushed" },
            { "label": "World 2", "value": "odd", "chip": "sparkly" },
            { "value": "no label" }
        ] });
        let rows = status_rows(&v);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].chip, Some(Chip::Pushed));
        assert_eq!(rows[1].chip, None);
    }

    #[test]
    fn the_file_log_rolls_over() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(DEBUG_LOG);
        std::fs::write(&path, vec![b'x'; DEBUG_LOG_MAX as usize + 1]).unwrap();
        file_log(path.clone())("stderr: hi");
        assert!(std::fs::read_to_string(&path)
            .unwrap()
            .ends_with("stderr: hi\n"));
        assert!(dir.path().join("sidecar.log.1").is_file());
    }
}
