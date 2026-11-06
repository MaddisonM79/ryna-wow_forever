//! The agent's memory between runs, in the OS data directory: for each account file, the mtime it
//! last pushed (so a run pushes only changed files) and what the heartbeat reports about it.
//! Written whole through a temp file and a rename, so a crash never leaves half of one. Plus the
//! run lock, which keeps a `watch` and a scheduled `run` from racing on it.

use std::collections::BTreeMap;
use std::fs::{File, OpenOptions, TryLockError};
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub const STATE_FILE: &str = "state.json";
pub const LOCK_FILE: &str = "run.lock";

/// `RYNA_WOW_STATE_DIR`, else the OS data directory (`~/Library/Application Support/io.ryna.ryna-wow`,
/// `%APPDATA%\ryna\ryna-wow\data`, `~/.local/share/ryna-wow`).
pub fn state_dir() -> Option<PathBuf> {
    if let Some(d) = std::env::var_os("RYNA_WOW_STATE_DIR").filter(|d| !d.is_empty()) {
        return Some(PathBuf::from(d));
    }
    directories::ProjectDirs::from("io", "ryna", "ryna-wow").map(|p| p.data_dir().to_path_buf())
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct State {
    /// By the account file's path.
    #[serde(default)]
    pub accounts: BTreeMap<String, AccountState>,
    #[serde(default)]
    pub last_run: Option<LastRun>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountState {
    pub key: String,
    pub label: String,
    /// The file's mtime (ns since the epoch) at the last accepted push. Nanoseconds, so two saves
    /// in the same second are still two.
    #[serde(default)]
    pub pushed_mtime_ns: Option<i128>,
    /// The file's mtime (unix seconds) the last time a run looked.
    #[serde(default)]
    pub saved_at: Option<i64>,
    #[serde(default)]
    pub pushed_at: Option<i64>,
    #[serde(default)]
    pub characters: Option<usize>,
    #[serde(default)]
    pub snapshot_id: Option<serde_json::Value>,
    #[serde(default)]
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LastRun {
    pub at: i64,
    pub ok: bool,
    #[serde(default)]
    pub error: Option<String>,
    /// What fixes `error` (absent in state written before it was kept).
    #[serde(default)]
    pub fix: Option<crate::fix::Fix>,
}

impl State {
    /// The state at `dir`, or empty if there is none or it can't be read (the next run then
    /// pushes every file once, which the API takes without harm).
    pub fn load(dir: &Path) -> State {
        std::fs::read(dir.join(STATE_FILE))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, dir: &Path) -> std::io::Result<()> {
        std::fs::create_dir_all(dir)?;
        let tmp = dir.join(format!(".{STATE_FILE}.{}.tmp", std::process::id()));
        let result = (|| {
            let mut f = File::create(&tmp)?;
            f.write_all(&serde_json::to_vec_pretty(self).map_err(std::io::Error::other)?)?;
            f.sync_all()?;
            drop(f);
            std::fs::rename(&tmp, dir.join(STATE_FILE))
        })();
        if result.is_err() {
            let _ = std::fs::remove_file(&tmp);
        }
        result
    }
}

/// Held for the length of a run. The OS drops the lock when the process ends, however it ends.
pub struct RunLock {
    _file: File,
}

/// Take the run lock, or None when another run holds it.
pub fn try_lock(dir: &Path) -> std::io::Result<Option<RunLock>> {
    try_lock_named(dir, LOCK_FILE)
}

/// Take the lock in `dir/name` (the combat tailer's is `combat/tail.lock`), or None when it's held.
pub fn try_lock_named(dir: &Path, name: &str) -> std::io::Result<Option<RunLock>> {
    std::fs::create_dir_all(dir)?;
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(dir.join(name))?;
    match file.try_lock() {
        Ok(()) => Ok(Some(RunLock { _file: file })),
        Err(TryLockError::WouldBlock) => Ok(None),
        Err(TryLockError::Error(e)) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_and_missing_or_corrupt_is_empty() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(State::load(dir.path()), State::default());
        let mut s = State::default();
        s.accounts.insert(
            "/x/Ryna.lua".into(),
            AccountState {
                key: "k".into(),
                label: "l".into(),
                pushed_mtime_ns: Some(1_762_464_000_123_456_789),
                ..Default::default()
            },
        );
        s.save(dir.path()).unwrap();
        assert_eq!(State::load(dir.path()), s);
        std::fs::write(dir.path().join(STATE_FILE), "{ not json").unwrap();
        assert_eq!(State::load(dir.path()), State::default());
        // No temp files left behind.
        let names: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names.len(), 1);
    }

    #[test]
    fn one_lock_at_a_time() {
        let dir = tempfile::tempdir().unwrap();
        let held = try_lock(dir.path()).unwrap().expect("first lock");
        assert!(try_lock(dir.path()).unwrap().is_none());
        drop(held);
        assert!(try_lock(dir.path()).unwrap().is_some());
    }
}
