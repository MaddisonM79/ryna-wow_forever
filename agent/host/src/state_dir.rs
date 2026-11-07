//! The agent's state folder and the run lock. Stage 1 has one folder for the agent, shared by its
//! one core (a per-core folder under it is stage 2's).

use std::fs::{File, OpenOptions, TryLockError};
use std::io::{Read, Seek, Write};
use std::path::{Path, PathBuf};

/// The run lock's file name in the state folder.
pub const LOCK_FILE: &str = "run.lock";
/// The variable that points the state folder elsewhere.
pub const STATE_DIR_ENV: &str = "RYNA_STATE_DIR";
/// `directories::ProjectDirs` qualifier, organisation and application: the app's identifier,
/// `io.ryna.app`, so on macOS the state sits beside the app's own settings.
pub const PROJECT_DIRS: (&str, &str, &str) = ("io", "ryna", "app");

/// `RYNA_STATE_DIR`, else the OS data directory ([`os_state_dir`]).
pub fn state_dir() -> Option<PathBuf> {
    if let Some(d) = std::env::var_os(STATE_DIR_ENV).filter(|d| !d.is_empty()) {
        return Some(PathBuf::from(d));
    }
    os_state_dir()
}

/// The OS data directory for the app: `~/Library/Application Support/io.ryna.app`,
/// `%APPDATA%\ryna\app\data`, `~/.local/share/io.ryna.app`. Linux takes the application
/// name alone, so there it gets the whole identifier (beside the app's own
/// `~/.config/io.ryna.app`), not a folder called `app`.
pub fn os_state_dir() -> Option<PathBuf> {
    let (q, o, a) = PROJECT_DIRS;
    let dirs = if cfg!(target_os = "linux") {
        directories::ProjectDirs::from("", "", &format!("{q}.{o}.{a}"))
    } else {
        directories::ProjectDirs::from(q, o, a)
    };
    dirs.map(|p| p.data_dir().to_path_buf())
}

/// Held for the length of a run. The OS drops the lock when the process ends, however it ends.
pub struct RunLock {
    _file: File,
}

/// The lock was held: by whom, as the holder wrote it into the lock file (`<pid> <unix time>
/// <thread>`), when the file could be read (on Windows a held lock can't be, so it's None there).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Busy {
    pub holder: Option<String>,
}

impl std::fmt::Display for Busy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Some(line) = self.holder.as_deref() else {
            return write!(f, "held by an unknown holder");
        };
        let mut parts = line.splitn(3, ' ');
        match (parts.next(), parts.next(), parts.next()) {
            (Some(pid), Some(at), thread) if !pid.is_empty() && !at.is_empty() => {
                write!(f, "held by pid {pid} since {at}")?;
                match thread {
                    Some(t) if !t.is_empty() => write!(f, " on thread {t}"),
                    _ => Ok(()),
                }
            }
            _ => write!(f, "held by {line:?}"),
        }
    }
}

/// Take the run lock, or `Err(Busy)` when another run holds it.
pub fn try_lock(dir: &Path) -> std::io::Result<Result<RunLock, Busy>> {
    try_lock_named(dir, LOCK_FILE)
}

/// What a holder writes into its lock file: `<pid> <unix time> <thread name or id>`.
fn holder_line() -> String {
    let at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let current = std::thread::current();
    let thread = current
        .name()
        .map_or_else(|| format!("{:?}", current.id()), str::to_string);
    format!("{} {at} {thread}", std::process::id())
}

/// The holder line in a held lock file, if it can be read and says anything.
fn read_holder(path: &Path) -> Option<String> {
    let mut text = String::new();
    File::open(path).ok()?.read_to_string(&mut text).ok()?;
    let text = text.trim();
    (!text.is_empty()).then(|| text.to_string())
}

/// Take the lock in `dir/name` (the combat tailer's is `combat/tail.lock`), or `Err(Busy)` when
/// it's held. The holder writes who it is into the file (truncated first), so a refusal can say.
pub fn try_lock_named(dir: &Path, name: &str) -> std::io::Result<Result<RunLock, Busy>> {
    std::fs::create_dir_all(dir)?;
    let path = dir.join(name);
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&path)?;
    match file.try_lock() {
        Ok(()) => {
            // Who holds it, for the next refusal's words; the lock itself doesn't depend on it.
            file.set_len(0)?;
            file.rewind()?;
            file.write_all(holder_line().as_bytes())?;
            file.flush()?;
            Ok(Ok(RunLock { _file: file }))
        }
        Err(TryLockError::WouldBlock) => Ok(Err(Busy {
            holder: read_holder(&path),
        })),
        Err(TryLockError::Error(e)) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_state_folder_is_named_for_the_app() {
        let dir = os_state_dir().unwrap();
        if cfg!(windows) {
            assert!(dir.ends_with(r"ryna\app\data"), "{}", dir.display());
        } else {
            assert!(dir.ends_with("io.ryna.app"), "{}", dir.display());
        }
    }

    #[test]
    fn one_lock_at_a_time() {
        let dir = tempfile::tempdir().unwrap();
        let held = try_lock(dir.path()).unwrap().expect("first lock");
        assert!(try_lock(dir.path()).unwrap().is_err());
        drop(held);
        assert!(try_lock(dir.path()).unwrap().is_ok());
    }

    #[test]
    fn the_holder_writes_who_it_is_and_a_refusal_reads_it() {
        let dir = tempfile::tempdir().unwrap();
        // A stale line from an earlier holder is replaced, not appended to.
        std::fs::write(dir.path().join(LOCK_FILE), "stale stale stale stale stale").unwrap();
        let held = try_lock(dir.path()).unwrap().expect("first lock");
        let busy = try_lock(dir.path()).unwrap().err().expect("held");
        // Windows refuses reads of a locked file (so there the holder is unknown, and the file is
        // read only once the lock is gone); Unix reads it while held.
        drop(held);
        let written = std::fs::read_to_string(dir.path().join(LOCK_FILE)).unwrap();
        let parts: Vec<&str> = written.splitn(3, ' ').collect();
        assert_eq!(parts[0], std::process::id().to_string(), "{written}");
        assert!(
            parts[1].parse::<u64>().unwrap() > 1_700_000_000,
            "{written}"
        );
        assert!(
            !parts[2].is_empty() && !written.contains("stale"),
            "{written}"
        );
        if cfg!(unix) {
            assert_eq!(busy.holder.as_deref(), Some(written.as_str()));
            let words = busy.to_string();
            assert!(
                words.starts_with(&format!("held by pid {} since ", std::process::id())),
                "{words}"
            );
        } else {
            assert!(busy.holder.is_none());
        }
        assert_eq!(
            Busy {
                holder: Some("1234 1762464000 main".into())
            }
            .to_string(),
            "held by pid 1234 since 1762464000 on thread main"
        );
        assert_eq!(
            Busy { holder: None }.to_string(),
            "held by an unknown holder"
        );
    }
}
