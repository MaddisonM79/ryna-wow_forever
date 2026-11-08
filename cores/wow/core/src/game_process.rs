//! Whether the game is running: so the window can say how to load a freshly installed addon, and so
//! the Settings mod refuses to write SavedVariables under a logged-in character (the game rewrites
//! them at logout, [`crate::settings`]). A best guess from the OS's own process list (`pgrep` on
//! macOS and Linux, `tasklist` on Windows): false when it can't tell.

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

#[cfg(windows)]
pub fn game_running() -> bool {
    use std::os::windows::process::CommandExt as _;
    /// No console window flashes up.
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    Command::new("tasklist")
        .args(["/FO", "CSV", "/NH"])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .is_ok_and(|o| {
            o.status.success() && tasklist_lists_the_game(&String::from_utf8_lossy(&o.stdout))
        })
}

#[cfg(not(windows))]
fn pgrep(args: &[&str]) -> bool {
    Command::new("pgrep")
        .args(args)
        .output()
        .is_ok_and(|o| o.status.success())
}

#[cfg(target_os = "macos")]
pub fn game_running() -> bool {
    // The process by name, or any executable inside a "World of Warcraft…" app bundle.
    pgrep(&["-x", "World of Warcraft"])
        || pgrep(&["-x", "World of Warcraft Classic"])
        || pgrep(&["-f", r"World of Warcraft[^/]*\.app/Contents/MacOS/"])
}

/// `wow.exe` as a pattern for `pgrep -x` that matches it in any case (`Wow.exe`, `WoW.exe`);
/// procps's pgrep has no case-insensitive flag.
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

/// Under Wine or Proton the game is its Windows executable, and that's the process name.
#[cfg(target_os = "linux")]
pub fn game_running() -> bool {
    WINDOWS_EXES
        .iter()
        .any(|exe| pgrep(&["-x", &any_case(exe)]))
}

#[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
pub fn game_running() -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_wine_process_pattern_matches_any_case() {
        assert_eq!(any_case("wow.exe"), r"[Ww][Oo][Ww]\.[Ee][Xx][Ee]");
        assert_eq!(any_case("wowt.exe"), r"[Ww][Oo][Ww][Tt]\.[Ee][Xx][Ee]");
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
    fn asking_never_panics() {
        let _ = game_running();
    }
}
