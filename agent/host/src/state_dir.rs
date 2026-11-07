//! The agent's state folder and the run lock. Stage 1 has one folder for the agent, shared by its
//! one core (a per-core folder under it is stage 2's).

use std::fs::{File, OpenOptions, TryLockError};
use std::path::{Path, PathBuf};

/// The run lock's file name in the state folder.
pub const LOCK_FILE: &str = "run.lock";
/// The variable that points the state folder elsewhere.
pub const STATE_DIR_ENV: &str = "RYNA_STATE_DIR";
/// `directories::ProjectDirs` qualifier, organisation and application: the app's identifier,
/// `io.ryna.app`, so on macOS the state sits beside the app's own settings.
pub const PROJECT_DIRS: (&str, &str, &str) = ("io", "ryna", "app");

/// `RYNA_STATE_DIR`, else the OS data directory (`~/Library/Application Support/io.ryna.app`,
/// `%APPDATA%\ryna\app\data`, `~/.local/share/app`).
pub fn state_dir() -> Option<PathBuf> {
    if let Some(d) = std::env::var_os(STATE_DIR_ENV).filter(|d| !d.is_empty()) {
        return Some(PathBuf::from(d));
    }
    let (q, o, a) = PROJECT_DIRS;
    directories::ProjectDirs::from(q, o, a).map(|p| p.data_dir().to_path_buf())
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
    fn one_lock_at_a_time() {
        let dir = tempfile::tempdir().unwrap();
        let held = try_lock(dir.path()).unwrap().expect("first lock");
        assert!(try_lock(dir.path()).unwrap().is_none());
        drop(held);
        assert!(try_lock(dir.path()).unwrap().is_some());
    }
}
