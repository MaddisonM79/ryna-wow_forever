//! Whether the game is running: so the window can say how to load a freshly installed addon, and so
//! the Settings mod refuses to write SavedVariables under a logged-in character (the game rewrites
//! them at logout, [`crate::settings`]).
//!
//! [`game_state`] reads the OS's process list (`pgrep` on macOS and Linux, `tasklist` on Windows)
//! and says when it couldn't. [`game_running`] is what writes go by: running, or can't tell (it
//! fails closed; only the command line's `--force` gets past it). Given the install, Windows also
//! asks the install's own executables whether they're in use ([`exe_in_use`]), so a Forever client
//! whose executable has a name this list doesn't know is still seen.
//!
//! What's matched:
//! - macOS: a process named `World of Warcraft` (or `World of Warcraft Classic`), or any
//!   executable inside a `World of Warcraft….app/Contents/MacOS/` bundle. Forever's client is
//!   `_forever_/World of Warcraft.app/Contents/MacOS/World of Warcraft` (seen 2026-11-08).
//! - Windows: `Wow.exe`, `WowClassic.exe`, `WowT.exe`, `WowB.exe` in `tasklist`, and any `.exe` in
//!   the install's folder that the OS says is in use.
//! - Linux (Wine, Proton): the same executable names as process names, and any process whose
//!   command line runs an `.exe` from a `_forever_` folder.

use std::path::Path;
use std::process::Command;

/// The game's executables on Windows (Forever, retail, Classic, the test realms), and so under
/// Wine or Proton on Linux.
#[cfg_attr(target_os = "macos", allow(dead_code))]
const WINDOWS_EXES: &[&str] = &["wow.exe", "wowclassic.exe", "wowt.exe", "wowb.exe"];

/// Whether `tasklist /FO CSV /NH` output lists one of the game's executables.
#[cfg_attr(not(windows), allow(dead_code))]
fn tasklist_lists_the_game(output: &str) -> bool {
    output.lines().any(|line| {
        let name = line
            .split(',')
            .next()
            .unwrap_or("")
            .trim()
            .trim_matches('"');
        WINDOWS_EXES
            .iter()
            .any(|exe| name.eq_ignore_ascii_case(exe))
    })
}

/// Whether the game is running: `Err` with why when the process list couldn't be read.
#[cfg(windows)]
pub fn game_state() -> Result<bool, String> {
    use std::os::windows::process::CommandExt as _;
    /// No console window flashes up.
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let out = Command::new("tasklist")
        .args(["/FO", "CSV", "/NH"])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("tasklist didn't run: {e}"))?;
    if !out.status.success() {
        return Err(format!("tasklist failed ({})", out.status));
    }
    Ok(tasklist_lists_the_game(&String::from_utf8_lossy(
        &out.stdout,
    )))
}

/// `pgrep` with `args`: found (exit 0), not found (exit 1), or couldn't tell (anything else).
#[cfg(not(windows))]
fn pgrep(args: &[&str]) -> Result<bool, String> {
    let out = Command::new("pgrep")
        .args(args)
        .output()
        .map_err(|e| format!("pgrep didn't run: {e}"))?;
    match out.status.code() {
        Some(0) => Ok(true),
        Some(1) => Ok(false),
        _ => Err(format!(
            "pgrep failed ({}): {}",
            out.status,
            String::from_utf8_lossy(&out.stderr).trim()
        )),
    }
}

/// Running when any check finds it; not running when every check says so; else can't tell.
#[cfg_attr(windows, allow(dead_code))]
fn any_of(checks: impl IntoIterator<Item = Result<bool, String>>) -> Result<bool, String> {
    let mut failed = None;
    for c in checks {
        match c {
            Ok(true) => return Ok(true),
            Ok(false) => {}
            Err(e) => failed = Some(e),
        }
    }
    failed.map_or(Ok(false), Err)
}

#[cfg(target_os = "macos")]
pub fn game_state() -> Result<bool, String> {
    // The process by name, or any executable inside a "World of Warcraft…" app bundle.
    any_of([
        pgrep(&["-x", "World of Warcraft"]),
        pgrep(&["-x", "World of Warcraft Classic"]),
        pgrep(&["-f", r"World of Warcraft[^/]*\.app/Contents/MacOS/"]),
    ])
}

/// `wow.exe` as a pattern for `pgrep -x` that matches it in any case (`Wow.exe`, `WoW.exe`);
/// procps's pgrep has no case-insensitive flag on every version.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn any_case(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            c if c.is_ascii_alphabetic() => {
                format!("[{}{}]", c.to_ascii_uppercase(), c.to_ascii_lowercase())
            }
            '.' => r"\.".to_string(),
            c => c.to_string(),
        })
        .collect()
}

/// An `.exe` run from a `_forever_` folder, as Wine shows the command line
/// (`C:\…\World of Warcraft\_forever_\Wow.exe`), for `pgrep -f` (POSIX ERE: a backslash in brackets
/// is itself).
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn forever_exe_pattern() -> String {
    format!(
        r"[\/]{}[\/][^\/]*{}",
        any_case("_forever_"),
        any_case(".exe")
    )
}

/// Under Wine or Proton the game is its Windows executable, and that's the process name.
#[cfg(target_os = "linux")]
pub fn game_state() -> Result<bool, String> {
    let mut checks: Vec<Result<bool, String>> = WINDOWS_EXES
        .iter()
        .map(|exe| pgrep(&["-x", &any_case(exe)]))
        .collect();
    checks.push(pgrep(&["-f", &forever_exe_pattern()]));
    any_of(checks)
}

#[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
pub fn game_state() -> Result<bool, String> {
    Err("this OS has no way to tell whether the game is running".into())
}

/// Whether one of the `.exe` files in `install` is in use (Windows: opening it to write is refused
/// with a sharing violation while it runs). Only that answer counts: a file that can't be opened
/// for another reason (no write access to Program Files) says nothing.
pub fn exe_in_use(install: &Path) -> bool {
    #[cfg(windows)]
    {
        /// ERROR_SHARING_VIOLATION.
        const SHARING_VIOLATION: i32 = 32;
        let Ok(entries) = std::fs::read_dir(install) else {
            return false;
        };
        entries.filter_map(Result::ok).any(|e| {
            let p = e.path();
            p.extension().is_some_and(|x| x.eq_ignore_ascii_case("exe"))
                && std::fs::OpenOptions::new()
                    .write(true)
                    .open(&p)
                    .err()
                    .and_then(|err| err.raw_os_error())
                    == Some(SHARING_VIOLATION)
        })
    }
    #[cfg(not(windows))]
    {
        let _ = install;
        false
    }
}

/// What a write goes by: the game is running, or it can't be told that it isn't.
pub fn game_running() -> bool {
    fails_closed(game_state())
}

/// A state as writes read it: can't tell is running.
fn fails_closed(state: Result<bool, String>) -> bool {
    state.unwrap_or(true)
}

/// [`game_running`], and (on Windows) the install's own executables in use.
pub fn game_running_at(install: &Path) -> bool {
    exe_in_use(install) || game_running()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_wine_process_pattern_matches_any_case() {
        assert_eq!(any_case("wow.exe"), r"[Ww][Oo][Ww]\.[Ee][Xx][Ee]");
        assert_eq!(any_case("wowt.exe"), r"[Ww][Oo][Ww][Tt]\.[Ee][Xx][Ee]");
        assert_eq!(
            forever_exe_pattern(),
            r"[\/]_[Ff][Oo][Rr][Ee][Vv][Ee][Rr]_[\/][^\/]*\.[Ee][Xx][Ee]"
        );
    }

    #[test]
    fn reads_the_game_out_of_tasklist() {
        let running = "\"System Idle Process\",\"0\",\"Services\",\"0\",\"8 K\"\r\n\
                       \"Wow.exe\",\"4242\",\"Console\",\"1\",\"2,104,332 K\"\r\n";
        assert!(tasklist_lists_the_game(running));
        assert!(tasklist_lists_the_game(
            "\"WowClassic.exe\",\"1\",\"Console\",\"1\",\"1 K\""
        ));
        assert!(!tasklist_lists_the_game(
            "\"Battle.net.exe\",\"7\",\"Console\",\"1\",\"1 K\"\r\n\"Wowhead.exe\",\"8\",\"Console\",\"1\",\"1 K\""
        ));
        assert!(!tasklist_lists_the_game(
            "INFO: No tasks are running which match the specified criteria."
        ));
    }

    #[test]
    fn cant_tell_counts_as_running() {
        assert_eq!(any_of([Ok(false), Ok(false)]), Ok(false));
        assert_eq!(any_of([Ok(false), Ok(true)]), Ok(true));
        assert_eq!(any_of([Err("x".into()), Ok(true)]), Ok(true));
        assert_eq!(
            any_of([Ok(false), Err("broken".into())]),
            Err("broken".into())
        );
        // What writes go by fails closed.
        assert!(fails_closed(Err("broken".into())));
        assert!(fails_closed(Ok(true)));
        assert!(!fails_closed(Ok(false)));
    }

    #[cfg(unix)]
    #[test]
    fn pgrep_tells_found_from_not_found_from_broken() {
        // A made-up name isn't running; a pattern pgrep can't read can't tell.
        assert_eq!(pgrep(&["-x", "no-such-process-ryna-xyz"]), Ok(false));
        assert!(
            pgrep(&["-f", "("]).is_err(),
            "an invalid pattern can't tell"
        );
    }

    /// A running executable can't be opened to write on Windows: this test's own is in use.
    #[cfg(windows)]
    #[test]
    fn a_running_exe_is_in_use() {
        let exe = std::env::current_exe().unwrap();
        assert!(exe_in_use(exe.parent().unwrap()));
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("Idle.exe"), b"MZ").unwrap();
        assert!(!exe_in_use(dir.path()));
    }

    #[cfg(not(windows))]
    #[test]
    fn exe_in_use_is_windows_only() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!exe_in_use(dir.path()));
    }

    #[test]
    fn asking_never_panics() {
        let _ = game_state();
        let _ = game_running();
    }
}
