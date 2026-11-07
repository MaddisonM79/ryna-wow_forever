//! The agent's memory between runs, in the OS data directory: for each account file, the mtime it
//! last pushed (so a run pushes only changed files) and what the heartbeat reports about it.
//! Written whole through a temp file and a rename, so a crash never leaves half of one. The run lock,
//! which keeps a `watch` and a scheduled `run` from racing on it, is the agent's (re-exported here).

use std::collections::BTreeMap;
use std::fs::File;
use std::io::Write;
use std::path::Path;

use serde::{Deserialize, Serialize};

pub const STATE_FILE: &str = "state.json";

/// The state folder and the run lock are the agent's (`ryna_agent::state_dir`); the names stay
/// here so `state::state_dir()` and `state::try_lock` read as they always have.
pub use ryna_agent::state_dir::{state_dir, try_lock, try_lock_named, Busy, RunLock, LOCK_FILE};

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
}
