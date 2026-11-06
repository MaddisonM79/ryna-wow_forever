//! Following the combat logs (docs/wow.md section 9.8): every `Logs/WoWCombatLog-*.txt` written
//! in the last 7 days in every install, newest first, read from where the last read stopped. A
//! partial last line waits for the rest. Each file's state (where to resume, the last fight it
//! queued, and what the log had said by then) is in `<state>/combat/state.json`, so the first run
//! reads the logs already on disk whole and later runs only what's new.
//!
//! A pull ends 8 s after its last event (by the wall clock, plus a grace for the client's
//! buffered writes); a world encounter when 5 minutes pass without another pull in its zone; a
//! boss at `ENCOUNTER_END`. Then its summary and digest are queued ([`super::queue`]).

use std::collections::{BTreeMap, HashMap};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};

use super::digest::gzip;
use super::queue::{Queue, Round, Uploaded};
use super::segment::{Context, Fight, Segmenter};
use super::{describe, summarize, CombatOptions};
use crate::discovery::Discovery;
use crate::fix::Fix;
use crate::push::{Api, CombatLog};
use crate::snapshot::account_key;
use crate::watch::WatchEvent;

pub const STATE_FILE: &str = "state.json";
pub const LOCK_FILE: &str = "tail.lock";
/// Logs older than this are left alone.
pub const MAX_AGE: Duration = Duration::from_secs(7 * 24 * 3600);
/// At most this much of one file is read per poll, so a first run over a big log stays responsive.
pub const READ_CHUNK: u64 = 16 * 1024 * 1024;
/// A log written within this long counts as active.
pub const ACTIVE_WINDOW_SECS: i64 = 10 * 60;
/// "Tonight": uploads this recent.
pub const TONIGHT_SECS: i64 = 12 * 3600;
/// After an outage, the queue waits this long before trying again.
pub const RETRY_AFTER_SECS: i64 = 60;
const UPLOADS_KEPT: usize = 200;

/// Where one log's read stands.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileState {
    /// Where the next read starts: the first line of the oldest encounter still open, or the end
    /// of what was read.
    pub offset: u64,
    /// The context at `offset`.
    #[serde(default)]
    pub context: Option<Context>,
    /// The last fight this log queued: a re-read skips it and everything before it.
    #[serde(default)]
    pub last_key: Option<String>,
    #[serde(default)]
    pub last_started_at: Option<i64>,
}

/// A fight that went up (for "fights tonight" and the last fight's line).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UploadedFight {
    /// Unix seconds.
    pub at: i64,
    pub key: String,
    pub kind: String,
    pub name: Option<String>,
    pub zone: Option<String>,
    pub duration_ms: i64,
    pub players: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CombatState {
    #[serde(default)]
    pub files: BTreeMap<String, FileState>,
    /// The newest log's file name.
    #[serde(default)]
    pub file: Option<String>,
    /// The newest event read (unix seconds).
    #[serde(default)]
    pub last_event_at: Option<i64>,
    #[serde(default)]
    pub advanced: Option<bool>,
    /// Encounters read but still open.
    #[serde(default)]
    pub open: usize,
    #[serde(default)]
    pub uploads: Vec<UploadedFight>,
    #[serde(default)]
    pub last_error: Option<String>,
}

impl CombatState {
    pub fn load(state_dir: &Path) -> CombatState {
        std::fs::read(state_dir.join("combat").join(STATE_FILE))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, state_dir: &Path) -> std::io::Result<()> {
        let dir = state_dir.join("combat");
        std::fs::create_dir_all(&dir)?;
        let tmp = dir.join(format!(".{STATE_FILE}.{}.tmp", std::process::id()));
        let body = serde_json::to_vec_pretty(self).map_err(std::io::Error::other)?;
        let result =
            std::fs::write(&tmp, body).and_then(|()| std::fs::rename(&tmp, dir.join(STATE_FILE)));
        if result.is_err() {
            let _ = std::fs::remove_file(&tmp);
        }
        result
    }

    pub fn fights_tonight(&self, now: i64) -> usize {
        self.uploads
            .iter()
            .filter(|u| now - u.at < TONIGHT_SECS)
            .count()
    }

    pub fn last_fight(&self) -> Option<&UploadedFight> {
        self.uploads.last()
    }

    pub fn active(&self, now: i64) -> bool {
        self.last_event_at
            .is_some_and(|t| now - t < ACTIVE_WINDOW_SECS)
    }
}

/// The heartbeat's `combatLog`, from the state on disk and the queue.
pub fn heartbeat(state_dir: &Path, now: i64) -> CombatLog {
    let st = CombatState::load(state_dir);
    CombatLog {
        active: st.active(now),
        advanced: st.advanced,
        file: st.file.clone(),
        last_event_at: st.last_event_at,
        pending: Queue::new(state_dir).len() + st.open,
    }
}

/// Every `WoWCombatLog-*.txt` in an install's `Logs`, written within `MAX_AGE` of `now`, newest
/// first.
pub fn logs(install: &Path, now: SystemTime) -> Vec<(PathBuf, SystemTime)> {
    let mut out: Vec<(PathBuf, SystemTime)> = std::fs::read_dir(install.join("Logs"))
        .map(|rd| {
            rd.filter_map(Result::ok)
                .filter(|e| {
                    let n = e.file_name();
                    let n = n.to_string_lossy();
                    n.starts_with("WoWCombatLog") && n.ends_with(".txt")
                })
                .filter_map(|e| {
                    let m = e.metadata().ok()?;
                    let t = m.modified().ok()?;
                    (m.is_file() && now.duration_since(t).map_or(true, |age| age <= MAX_AGE))
                        .then(|| (e.path(), t))
                })
                .collect()
        })
        .unwrap_or_default();
    out.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    out
}

struct Open {
    seg: Segmenter,
    /// The next byte to read.
    offset: u64,
    install: PathBuf,
    flavor: String,
    name: String,
    last_key: Option<String>,
    last_started_at: Option<i64>,
}

/// Per install: the account files' mtimes when read, each character's account key, and the newest
/// account's key.
type InstallAccounts = (
    Vec<(PathBuf, i128)>,
    HashMap<String, String>,
    Option<String>,
);

/// The account (its key) each character GUID belongs to, per install, from the addon's files.
#[derive(Default)]
struct Accounts {
    by_install: HashMap<PathBuf, InstallAccounts>,
}

impl Accounts {
    /// The logger's account key: the account whose `Ryna.lua` lists one of the fight's players
    /// (the logger first), else the install's most recently saved account, else none.
    fn key_for(
        &mut self,
        discovery: &Discovery,
        install: &Path,
        players: &[&str],
    ) -> Option<String> {
        let files: Vec<(PathBuf, i128, String)> = discovery
            .accounts
            .iter()
            .filter(|a| a.install == install)
            .map(|a| {
                let m = crate::util::mtime(&a.path).map_or(0, |m| m.nanos);
                (a.path.clone(), m, a.account.clone())
            })
            .collect();
        let stamp: Vec<(PathBuf, i128)> = files.iter().map(|(p, m, _)| (p.clone(), *m)).collect();
        let fresh = self
            .by_install
            .get(install)
            .is_some_and(|(s, _, _)| *s == stamp);
        if !fresh {
            let mut map = HashMap::new();
            for (path, _, folder) in &files {
                if let Ok(built) = crate::snapshot::read_file(path, folder, None, None) {
                    for c in built.snapshot.characters {
                        map.insert(c.guid, account_key(folder));
                    }
                }
            }
            let newest = files
                .iter()
                .max_by_key(|(_, m, _)| *m)
                .map(|(_, _, folder)| account_key(folder));
            self.by_install
                .insert(install.to_path_buf(), (stamp, map, newest));
        }
        let (_, map, newest) = &self.by_install[install];
        players
            .iter()
            .find_map(|g| map.get(*g).cloned())
            .or_else(|| newest.clone())
    }
}

/// What one poll did.
#[derive(Debug, Default)]
pub struct Poll {
    pub queued: usize,
    pub uploaded: usize,
    pub failed: usize,
}

/// The combat side of the watch loop: reads, segments, queues and uploads.
pub struct Tailer {
    state_dir: PathBuf,
    pub opts: CombatOptions,
    state: CombatState,
    open: BTreeMap<PathBuf, Open>,
    accounts: Accounts,
    queue: Queue,
    digests_off: bool,
    retry_at: i64,
    announced: Vec<PathBuf>,
}

fn file_name(p: &Path) -> String {
    p.file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned())
}

impl Tailer {
    pub fn new(state_dir: &Path, opts: CombatOptions) -> Tailer {
        Tailer {
            state_dir: state_dir.to_path_buf(),
            opts,
            state: CombatState::load(state_dir),
            open: BTreeMap::new(),
            accounts: Accounts::default(),
            queue: Queue::new(state_dir),
            digests_off: false,
            retry_at: 0,
            announced: Vec::new(),
        }
    }

    pub fn queue(&self) -> &Queue {
        &self.queue
    }

    /// Read what's new in every log, queue what ended, and upload when the queue has something.
    /// `now` is unix seconds; the wall clock in ms closes fights whose gap has passed.
    pub fn poll(
        &mut self,
        discovery: &Discovery,
        api: Option<&Api<'_>>,
        now: i64,
        on_event: &mut dyn FnMut(WatchEvent),
    ) -> Poll {
        let mut report = Poll::default();
        if !self.opts.enabled {
            return report;
        }
        let wall = SystemTime::now();
        let now_ms = now * 1000;
        let mut seen: Vec<PathBuf> = Vec::new();
        let mut newest: Option<(SystemTime, PathBuf)> = None;
        for install in &discovery.installs {
            for (path, mtime) in logs(&install.path, wall) {
                if newest.as_ref().is_none_or(|(t, _)| mtime > *t) {
                    newest = Some((mtime, path.clone()));
                }
                seen.push(path.clone());
                if !self.announced.contains(&path) {
                    self.announced.push(path.clone());
                    on_event(WatchEvent::CombatLogFound {
                        file: file_name(&path),
                        path: path.to_string_lossy().into_owned(),
                    });
                }
                if let Err(e) = self.read(
                    &path,
                    &install.path,
                    install.flavor.as_deref(),
                    discovery,
                    now_ms,
                    now,
                    &mut report,
                    on_event,
                ) {
                    self.state.last_error = Some(format!("{}: {e}", path.display()));
                }
            }
        }
        // Logs past their age: finish what they had open and forget them.
        let gone: Vec<PathBuf> = self
            .open
            .keys()
            .filter(|p| !seen.contains(p))
            .cloned()
            .collect();
        for path in gone {
            if let Some(mut o) = self.open.remove(&path) {
                o.seg.finish();
                self.queue_fights(&mut o, discovery, now, &mut report, on_event);
            }
            self.state
                .files
                .remove(&path.to_string_lossy().into_owned());
        }
        self.state.file = newest.as_ref().map(|(_, p)| file_name(p));
        if let Some((_, p)) = &newest {
            if let Some(o) = self.open.get(p) {
                self.state.advanced = o.seg.ctx.advanced.or(self.state.advanced);
            }
        }
        self.state.open = self.open.values().map(|o| o.seg.open()).sum();
        if let Some(api) = api {
            if !self.queue.is_empty() && now >= self.retry_at {
                let round = self.queue.upload(api, &self.opts, self.digests_off);
                self.after_round(round, now, &mut report, on_event);
            }
        }
        let _ = self.state.save(&self.state_dir);
        report
    }

    #[allow(clippy::too_many_arguments)]
    fn read(
        &mut self,
        path: &Path,
        install: &Path,
        flavor: Option<&str>,
        discovery: &Discovery,
        now_ms: i64,
        now: i64,
        report: &mut Poll,
        on_event: &mut dyn FnMut(WatchEvent),
    ) -> std::io::Result<()> {
        let key = path.to_string_lossy().into_owned();
        let size = std::fs::metadata(path)?.len();
        if !self.open.contains_key(path) {
            let fs = self.state.files.get(&key).cloned().unwrap_or_default();
            // A file that shrank was replaced: read it from the start.
            let (offset, ctx, last_key, last_started_at) = if fs.offset > size {
                (0, None, None, None)
            } else {
                (fs.offset, fs.context, fs.last_key, fs.last_started_at)
            };
            let seg = match ctx {
                Some(c) => Segmenter::with_context(c, self.opts.raw),
                None => Segmenter::new(self.opts.raw),
            };
            self.open.insert(
                path.to_path_buf(),
                Open {
                    seg,
                    offset,
                    install: install.to_path_buf(),
                    flavor: flavor.unwrap_or("").to_string(),
                    name: file_name(path),
                    last_key,
                    last_started_at,
                },
            );
        }
        let mut o = self.open.remove(path).expect("inserted above");
        if size < o.offset {
            o = Open {
                seg: Segmenter::new(self.opts.raw),
                offset: 0,
                last_key: None,
                last_started_at: None,
                ..o
            };
        }
        let result = (|| -> std::io::Result<()> {
            if size > o.offset {
                let mut f = std::fs::File::open(path)?;
                f.seek(SeekFrom::Start(o.offset))?;
                let mut buf = Vec::new();
                f.take(READ_CHUNK).read_to_end(&mut buf)?;
                // Only whole lines: a partial last one waits for the rest.
                let whole = buf.iter().rposition(|&b| b == b'\n').map_or(0, |i| i + 1);
                let mut at = o.offset;
                for line in buf[..whole].split_inclusive(|&b| b == b'\n') {
                    o.seg.feed(&String::from_utf8_lossy(line), at);
                    at += line.len() as u64;
                }
                o.offset = at;
            }
            Ok(())
        })();
        o.seg.tick(now_ms);
        self.queue_fights(&mut o, discovery, now, report, on_event);
        if let Some(t) = o.seg.last_event_at {
            let secs = t.div_euclid(1000);
            self.state.last_event_at = Some(self.state.last_event_at.map_or(secs, |s| s.max(secs)));
        }
        let (offset, context) = match o.seg.resume_point() {
            Some((off, ctx)) => (off, ctx),
            None => (o.offset, o.seg.ctx.clone()),
        };
        self.state.files.insert(
            key,
            FileState {
                offset,
                context: Some(context),
                last_key: o.last_key.clone(),
                last_started_at: o.last_started_at,
            },
        );
        self.open.insert(path.to_path_buf(), o);
        result
    }

    fn queue_fights(
        &mut self,
        o: &mut Open,
        discovery: &Discovery,
        now: i64,
        report: &mut Poll,
        on_event: &mut dyn FnMut(WatchEvent),
    ) {
        for fight in o.seg.take() {
            if o.last_started_at.is_some_and(|t| fight.started_at <= t) {
                continue; // queued before a restart
            }
            let participants = fight.totals().participants;
            let mut players: Vec<&str> = o.seg.ctx.logger.as_deref().into_iter().collect();
            players.extend(participants.iter().map(String::as_str));
            let account = self.accounts.key_for(discovery, &o.install, &players);
            match self.enqueue(&fight, &o.flavor, account.as_deref(), &o.name, now) {
                Ok(key) => {
                    o.last_started_at = Some(fight.started_at);
                    o.last_key = Some(key.clone());
                    report.queued += 1;
                    on_event(WatchEvent::FightQueued {
                        key,
                        kind: fight.kind.as_str().to_string(),
                        zone: fight.zone.clone(),
                        name: fight.name.clone(),
                        duration_ms: fight.duration_ms(),
                        players: fight.players(),
                    });
                }
                Err(e) => {
                    let message = format!("couldn't keep a fight to upload: {e}");
                    self.state.last_error = Some(message.clone());
                    on_event(WatchEvent::error(
                        Some("Fights".into()),
                        None,
                        message,
                        Fix::Other,
                    ));
                }
            }
        }
    }

    fn enqueue(
        &self,
        fight: &Fight,
        flavor: &str,
        account: Option<&str>,
        file: &str,
        now: i64,
    ) -> std::io::Result<String> {
        let mut summary = summarize(fight, flavor, account.unwrap_or(""), file, &self.opts);
        summary.account = account.map(str::to_string);
        let key = summary.key.clone();
        if self.queue.contains(&key) {
            return Ok(key);
        }
        let players = summary.participants.len();
        let digest = self
            .opts
            .digests
            .then(|| gzip(fight.digest().to_ndjson().as_bytes()));
        let raw = (self.opts.raw && !fight.raw.is_empty()).then(|| {
            let mut text = fight.raw.join("");
            if !text.ends_with('\n') {
                text.push('\n');
            }
            gzip(text.as_bytes())
        });
        self.queue.push(summary, players, digest, raw, now)?;
        Ok(key)
    }

    fn after_round(
        &mut self,
        round: Round,
        now: i64,
        report: &mut Poll,
        on_event: &mut dyn FnMut(WatchEvent),
    ) {
        self.digests_off |= round.digests_off;
        for u in round.uploaded {
            match u {
                Uploaded::Done { item } => {
                    report.uploaded += 1;
                    let s = &item.summary;
                    self.state.uploads.push(UploadedFight {
                        at: now,
                        key: item.key.clone(),
                        kind: s.kind.clone(),
                        name: s.name.clone(),
                        zone: s.zone.clone(),
                        duration_ms: s.duration_ms,
                        players: item.players,
                    });
                    on_event(WatchEvent::FightUploaded {
                        key: item.key.clone(),
                        kind: s.kind.clone(),
                        zone: s.zone.clone(),
                        name: s.name.clone(),
                        duration_ms: s.duration_ms,
                        players: item.players,
                        id: item.id.clone().unwrap_or_default(),
                        merged: item.merged.unwrap_or(false),
                    });
                }
                Uploaded::Taken { .. } => report.uploaded += 1,
                Uploaded::GaveUp { item, message } => {
                    report.failed += 1;
                    let s = &item.summary;
                    let what = describe(
                        s.zone.as_deref(),
                        s.name.as_deref(),
                        s.duration_ms,
                        item.players,
                    );
                    on_event(WatchEvent::FightFailed {
                        key: item.key.clone(),
                        fight: what,
                        advice: Fix::Other.message(&message),
                        message,
                        fix: Fix::Other,
                    });
                }
            }
        }
        let excess = self.state.uploads.len().saturating_sub(UPLOADS_KEPT);
        self.state.uploads.drain(..excess);
        match round.error {
            Some((e, item)) => {
                let fix = e.fix();
                if matches!(fix, Fix::SignIn | Fix::Network) {
                    self.retry_at = now + RETRY_AFTER_SECS;
                }
                let message = e.to_string();
                self.state.last_error = Some(message.clone());
                let fight = item.map_or_else(
                    || "Fights".to_string(),
                    |i| {
                        describe(
                            i.summary.zone.as_deref(),
                            i.summary.name.as_deref(),
                            i.summary.duration_ms,
                            i.players,
                        )
                    },
                );
                report.failed += 1;
                on_event(WatchEvent::FightFailed {
                    key: String::new(),
                    fight,
                    advice: fix.message(&message),
                    message,
                    fix,
                });
            }
            None => self.state.last_error = None,
        }
    }

    /// On quit: end every open encounter and queue it, so nothing read is lost. Nothing uploads
    /// here; the next run sends it.
    pub fn flush(&mut self, discovery: &Discovery, now: i64, on_event: &mut dyn FnMut(WatchEvent)) {
        let mut report = Poll::default();
        let paths: Vec<PathBuf> = self.open.keys().cloned().collect();
        for path in paths {
            let Some(mut o) = self.open.remove(&path) else {
                continue;
            };
            o.seg.finish();
            self.queue_fights(&mut o, discovery, now, &mut report, on_event);
            let key = path.to_string_lossy().into_owned();
            self.state.files.insert(
                key,
                FileState {
                    offset: o.offset,
                    context: Some(o.seg.ctx.clone()),
                    last_key: o.last_key.clone(),
                    last_started_at: o.last_started_at,
                },
            );
        }
        self.state.open = 0;
        let _ = self.state.save(&self.state_dir);
    }
}
