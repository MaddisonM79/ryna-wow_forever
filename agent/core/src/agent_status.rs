//! `AgentStatus.lua`: what the agent tells the addon. After every run, the agent rewrites
//! `<install>/Interface/AddOns/Ryna/AgentStatus.lua` in each install that has the addon (and leaves
//! installs without it alone), as one Lua assignment the addon loads at login:
//!
//! ```lua
//! RynaAgentStatus = { agent = "0.1.0", signedIn = true, as = "aerin", lastPush = 1762464060,
//!   accounts = { { key = "…", label = "UNT1", savedAt = 1762464000, pushedAt = 1762464060, characters = 2 } },
//!   latest = { addon = nil, agent = nil },
//!   messages = { { at = 1762464060, level = "error", text = "Couldn't reach the API" } } }
//! ```
//!
//! `accounts` lists that install's accounts only. `messages` carries the last run's error, if any;
//! `latest` (newest released versions) stays nil until a release feed exists. Written through a
//! temp file and a rename, so the game never loads half of one.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use crate::discovery::Discovery;
use crate::runner::Failure;
use crate::snapshot::{account_key, default_label};
use crate::state::State;

pub const FILE_NAME: &str = "AgentStatus.lua";
pub const VARIABLE: &str = "RynaAgentStatus";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusAccount {
    pub key: String,
    pub label: String,
    pub saved_at: Option<i64>,
    pub pushed_at: Option<i64>,
    pub characters: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Info,
    Warn,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    pub at: i64,
    pub level: Level,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentStatus {
    pub agent: String,
    pub signed_in: bool,
    /// Who it is signed in as (a display name, or an email's local part).
    pub signed_in_as: Option<String>,
    pub last_push: Option<i64>,
    pub accounts: Vec<StatusAccount>,
    pub messages: Vec<Message>,
}

/// A Lua string literal: quotes, backslashes, and every control or non-ASCII byte escaped as
/// `\ddd` (the client reads them back as the same UTF-8 bytes).
pub fn lua_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for b in s.bytes() {
        match b {
            b'"' => out.push_str("\\\""),
            b'\\' => out.push_str("\\\\"),
            b'\n' => out.push_str("\\n"),
            b'\r' => out.push_str("\\r"),
            b'\t' => out.push_str("\\t"),
            0x20..=0x7e => out.push(b as char),
            _ => {
                let _ = write!(out, "\\{b:03}");
            }
        }
    }
    out.push('"');
    out
}

fn opt_int(v: Option<i64>) -> String {
    v.map_or_else(|| "nil".to_string(), |n| n.to_string())
}

impl AgentStatus {
    pub fn to_lua(&self) -> String {
        let mut s = String::new();
        s.push_str("-- Written by the ryna-wow agent after every run. Edits are overwritten.\n");
        let _ = writeln!(s, "{VARIABLE} = {{");
        let _ = writeln!(s, "\tagent = {},", lua_string(&self.agent));
        let _ = writeln!(s, "\tsignedIn = {},", self.signed_in);
        let _ = writeln!(
            s,
            "\tas = {},",
            self.signed_in_as
                .as_deref()
                .map_or_else(|| "nil".to_string(), lua_string)
        );
        let _ = writeln!(s, "\tlastPush = {},", opt_int(self.last_push));
        s.push_str("\taccounts = {\n");
        for a in &self.accounts {
            let _ = writeln!(
                s,
                "\t\t{{ key = {}, label = {}, savedAt = {}, pushedAt = {}, characters = {} }},",
                lua_string(&a.key),
                lua_string(&a.label),
                opt_int(a.saved_at),
                opt_int(a.pushed_at),
                opt_int(a.characters.map(|n| n as i64)),
            );
        }
        s.push_str("\t},\n");
        s.push_str("\tlatest = { addon = nil, agent = nil },\n");
        s.push_str("\tmessages = {\n");
        for m in &self.messages {
            let level = match m.level {
                Level::Info => "info",
                Level::Warn => "warn",
                Level::Error => "error",
            };
            let _ = writeln!(
                s,
                "\t\t{{ at = {}, level = \"{level}\", text = {} }},",
                m.at,
                lua_string(&m.text)
            );
        }
        s.push_str("\t},\n}\n");
        s
    }
}

/// The message the addon shows for a run's failure.
pub fn message_for(failure: Failure, error: &str) -> String {
    match failure {
        Failure::SignIn => "Sign in again: the agent's sign-in has lapsed".to_string(),
        Failure::Update => {
            "Update the agent: the server no longer accepts this snapshot format".to_string()
        }
        Failure::Other if error.contains("unreachable") || error.contains("failing after") => {
            "Couldn't reach the API".to_string()
        }
        Failure::Other => error.chars().take(200).collect(),
    }
}

/// The status for one install: its accounts as `state` knows them.
pub fn for_install(
    install: &Path,
    discovery: &Discovery,
    state: &State,
    signed_in: bool,
    signed_in_as: Option<String>,
    failure: Option<(Failure, &str)>,
    now: i64,
) -> AgentStatus {
    let accounts: Vec<StatusAccount> = discovery
        .accounts
        .iter()
        .filter(|a| a.install == install)
        .map(|a| {
            let s = state.accounts.get(&a.path.to_string_lossy().into_owned());
            StatusAccount {
                key: s.map_or_else(|| account_key(&a.account), |s| s.key.clone()),
                label: s.map_or_else(|| default_label(&a.account), |s| s.label.clone()),
                saved_at: s.and_then(|s| s.saved_at),
                pushed_at: s.and_then(|s| s.pushed_at),
                characters: s.and_then(|s| s.characters),
            }
        })
        .collect();
    AgentStatus {
        agent: crate::AGENT_VERSION.to_string(),
        signed_in,
        signed_in_as,
        last_push: accounts.iter().filter_map(|a| a.pushed_at).max(),
        accounts,
        messages: failure
            .map(|(f, e)| Message {
                at: now,
                level: Level::Error,
                text: message_for(f, e),
            })
            .into_iter()
            .collect(),
    }
}

fn write_atomic(path: &Path, text: &str) -> std::io::Result<()> {
    let dir = path.parent().unwrap_or(Path::new("."));
    let tmp = dir.join(format!(".{FILE_NAME}.{}.tmp", std::process::id()));
    let result = std::fs::write(&tmp, text).and_then(|()| std::fs::rename(&tmp, path));
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

/// Rewrite `AgentStatus.lua` in every install that has the addon. Returns the files written and
/// the ones that couldn't be (never fails the run).
pub fn write_all(
    discovery: &Discovery,
    state: &State,
    signed_in: bool,
    signed_in_as: Option<String>,
    failure: Option<(Failure, &str)>,
    now: i64,
) -> (Vec<PathBuf>, Vec<(PathBuf, std::io::Error)>) {
    let mut written = Vec::new();
    let mut failed = Vec::new();
    for i in &discovery.installs {
        let addon = i
            .path
            .join("Interface")
            .join("AddOns")
            .join(crate::addon_install::ADDON_NAME);
        if !addon.is_dir() {
            continue;
        }
        let path = addon.join(FILE_NAME);
        let status = for_install(
            &i.path,
            discovery,
            state,
            signed_in,
            signed_in_as.clone(),
            failure,
            now,
        );
        match write_atomic(&path, &status.to_lua()) {
            Ok(()) => written.push(path),
            Err(e) => failed.push((path, e)),
        }
    }
    (written, failed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::discovery::{discover, testing::product};
    use crate::lua;
    use crate::state::AccountState;
    use serde_json::json;

    fn status() -> AgentStatus {
        AgentStatus {
            agent: "0.1.0".into(),
            signed_in: true,
            signed_in_as: Some("aerin \"the\" \\ é\n".into()),
            last_push: Some(1_762_464_060),
            accounts: vec![
                StatusAccount {
                    key: "ba7816bf8f01cfea".into(),
                    label: "UNT1".into(),
                    saved_at: Some(1_762_464_000),
                    pushed_at: Some(1_762_464_060),
                    characters: Some(2),
                },
                StatusAccount {
                    key: "0123456789abcdef".into(),
                    label: "Alt's".into(),
                    saved_at: Some(5),
                    pushed_at: None,
                    characters: None,
                },
            ],
            messages: vec![Message {
                at: 9,
                level: Level::Error,
                text: "Couldn't reach the API".into(),
            }],
        }
    }

    #[test]
    fn round_trips_through_the_lua_reader() {
        let v = lua::parse(&status().to_lua()).unwrap();
        assert_eq!(
            v,
            json!({ "RynaAgentStatus": {
                "agent": "0.1.0", "signedIn": true, "as": "aerin \"the\" \\ é\n", "lastPush": 1_762_464_060,
                "accounts": [
                    { "key": "ba7816bf8f01cfea", "label": "UNT1", "savedAt": 1_762_464_000, "pushedAt": 1_762_464_060, "characters": 2 },
                    { "key": "0123456789abcdef", "label": "Alt's", "savedAt": 5 },
                ],
                "latest": [],
                "messages": [ { "at": 9, "level": "error", "text": "Couldn't reach the API" } ],
            } })
        );
        let mut empty = status();
        empty.signed_in = false;
        empty.signed_in_as = None;
        empty.last_push = None;
        empty.accounts.clear();
        empty.messages.clear();
        let v = lua::parse(&empty.to_lua()).unwrap();
        assert_eq!(v["RynaAgentStatus"]["signedIn"], json!(false));
        assert!(v["RynaAgentStatus"].get("as").is_none());
        assert_eq!(v["RynaAgentStatus"]["accounts"], json!([]));
    }

    #[test]
    fn escapes_every_awkward_byte() {
        assert_eq!(
            lua_string("a\"b\\c\nd\te\r\u{1}é"),
            "\"a\\\"b\\\\c\\nd\\te\\r\\001\\195\\169\""
        );
        assert_eq!(
            lua::parse(&format!("X = {}", lua_string("]] -- [[ ✓"))).unwrap()["X"],
            json!("]] -- [[ ✓")
        );
    }

    #[test]
    fn messages() {
        assert_eq!(
            message_for(
                Failure::Other,
                "the API is unreachable or failing after 3 tries: x"
            ),
            "Couldn't reach the API"
        );
        assert!(message_for(Failure::Update, "").starts_with("Update the agent"));
        assert_eq!(
            message_for(Failure::Other, "line 3, column 1: bad"),
            "line 3, column 1: bad"
        );
    }

    #[test]
    fn writes_only_where_the_addon_is_and_only_that_install_s_accounts() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("World of Warcraft");
        let forever = product(&root, "_forever_", "wowf", &[("FAKEACCOUNT1", "")]);
        product(&root, "_retail_", "wow", &[("FAKEACCOUNT2", "")]);
        std::fs::create_dir_all(forever.join("Interface/AddOns/Ryna")).unwrap();
        let d = discover(Some(&root));
        let mut st = State::default();
        st.accounts.insert(
            d.accounts[0].path.to_string_lossy().into_owned(),
            AccountState {
                key: "k1".into(),
                label: "Main".into(),
                saved_at: Some(10),
                pushed_at: Some(20),
                characters: Some(2),
                ..Default::default()
            },
        );
        let (written, failed) = write_all(
            &d,
            &st,
            true,
            Some("aerin".into()),
            Some((Failure::Other, "unreachable")),
            30,
        );
        assert!(failed.is_empty());
        assert_eq!(
            written,
            vec![forever.join("Interface/AddOns/Ryna/AgentStatus.lua")]
        );
        let v = lua::parse(&std::fs::read_to_string(&written[0]).unwrap()).unwrap();
        let s = &v["RynaAgentStatus"];
        assert_eq!(
            s["accounts"],
            json!([{ "key": "k1", "label": "Main", "savedAt": 10, "pushedAt": 20, "characters": 2 }])
        );
        assert_eq!(s["lastPush"], json!(20));
        assert_eq!(s["messages"][0]["text"], json!("Couldn't reach the API"));
        assert!(!root.join("_retail_/Interface").exists());
        // Rewritten whole the next time, with no temp file left.
        write_all(&d, &st, false, None, None, 40);
        let v = lua::parse(&std::fs::read_to_string(&written[0]).unwrap()).unwrap();
        assert_eq!(v["RynaAgentStatus"]["messages"], json!([]));
        assert_eq!(
            std::fs::read_dir(forever.join("Interface/AddOns/Ryna"))
                .unwrap()
                .count(),
            1
        );
    }
}
