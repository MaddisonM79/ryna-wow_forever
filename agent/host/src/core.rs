//! The one interface between the agent and a core (docs/ryna-agent-cores-mods.md section 4).
//!
//! A core declares itself in a manifest (`cores/<id>/core.json`, [`CoreManifest`]) and implements
//! [`Core`]: it is started with a [`CoreContext`] (its state folder, a [`TokenProvider`] and the
//! game folder the person chose, if any) and runs on a thread the agent gives it until it's told to
//! stop. While it runs the agent can ask it for a pass now (`sync`) or to stop (`stop`), both
//! through the [`CoreControl`] it was started with, from any thread. It reports what it does as
//! events: each is `Serialize` and goes to the window unchanged (which words it for the activity
//! log), and [`Core::activity`] says what the agent's own state takes from it (a row's phase, an
//! error to show). It also answers, at any time, the rows for its status card
//! ([`Core::status_rows`]) and what its heartbeat says ([`Core::heartbeat`]).
//!
//! The trait is generic over its event type rather than object-safe: the app drives each core on a
//! thread of its own through the same generic glue, the WoW core in-process (`runtime.kind =
//! "builtin"`) and every sidecar core through [`crate::sidecar::SidecarCore`], which speaks this
//! interface to a separate program as line-delimited JSON (docs/ryna-agent-cores-mods.md 13.4).

use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::auth::{KeyringStore, TokenStore};
use crate::cancel::CancelToken;
use crate::config::Config;
use crate::fix::Fix;
use crate::identity::DeviceKey;

/// `cores/<id>/core.json`: what a core is, what it needs and what it touches.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoreManifest {
    /// Short, lowercase: `wow`, `palworld`.
    pub id: String,
    pub name: String,
    /// The core's version (`x.y.z`); while cores are bundled it is the agent's.
    pub version: String,
    pub runtime: Runtime,
    /// The resource (RFC 8707) its tokens are for.
    pub resource: String,
    /// The scopes it needs on that resource.
    pub scopes: Vec<String>,
    #[serde(default)]
    pub detect: Detect,
    #[serde(default)]
    pub data: DataFiles,
    /// Where its mods' manifests live, from the repository root (`mods/wow`).
    #[serde(default)]
    pub mods: Option<String>,
    /// The operating systems it runs on (`macos`, `windows`, `linux`); absent means all.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub platforms: Option<Vec<String>>,
}

/// How the agent runs a core.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Runtime {
    /// In the agent's own process (Rust).
    Builtin,
    /// A separate program the agent starts and stops (stage 4).
    Sidecar {
        bin: String,
        #[serde(default)]
        args: Vec<String>,
    },
}

/// Where the game usually is.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Detect {
    #[serde(default)]
    pub folders: Vec<String>,
}

/// What the core reads in the game folder and writes there (globs, from the game's product folder).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DataFiles {
    #[serde(default)]
    pub reads: Vec<String>,
    #[serde(default)]
    pub writes: Vec<String>,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ManifestError {
    #[error("the core manifest isn't valid JSON for one: {0}")]
    Json(String),
    #[error("the core manifest's {0}")]
    Invalid(String),
}

impl CoreManifest {
    /// Parse and check a `core.json`.
    pub fn parse(json: &str) -> Result<CoreManifest, ManifestError> {
        let m: CoreManifest =
            serde_json::from_str(json).map_err(|e| ManifestError::Json(e.to_string()))?;
        m.check()?;
        Ok(m)
    }

    fn check(&self) -> Result<(), ManifestError> {
        let bad = |why: &str| Err(ManifestError::Invalid(why.to_string()));
        if self.id.is_empty()
            || !self
                .id
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        {
            return bad("id must be lowercase letters, digits and dashes");
        }
        if self.name.trim().is_empty() {
            return bad("name is empty");
        }
        let parts: Vec<&str> = self
            .version
            .split('-')
            .next()
            .unwrap_or("")
            .split('.')
            .collect();
        if parts.len() != 3 || parts.iter().any(|p| p.parse::<u64>().is_err()) {
            return bad("version isn't x.y.z");
        }
        if !self.resource.starts_with("https://") && !self.resource.starts_with("http://") {
            return bad("resource isn't a URL");
        }
        if self.scopes.is_empty() || self.scopes.iter().any(|s| s.trim().is_empty()) {
            return bad("scopes are empty");
        }
        if let Some(p) = &self.platforms {
            if p.iter()
                .any(|p| !matches!(p.as_str(), "macos" | "windows" | "linux"))
            {
                return bad("platforms names one that isn't macos, windows or linux");
            }
        }
        if let Runtime::Sidecar { bin, .. } = &self.runtime {
            if bin.trim().is_empty() {
                return bad("sidecar names no bin");
            }
        }
        Ok(())
    }

    /// Whether it runs on this computer's operating system.
    pub fn runs_here(&self) -> bool {
        runs_on(self.platforms.as_deref(), this_platform())
    }
}

/// This computer's operating system as a manifest's `platforms` names it.
pub fn this_platform() -> &'static str {
    match std::env::consts::OS {
        "macos" => "macos",
        "windows" => "windows",
        _ => "linux",
    }
}

/// Whether a core with `platforms` (None: all) runs on `os`.
pub fn runs_on(platforms: Option<&[String]>, os: &str) -> bool {
    platforms.is_none_or(|p| p.iter().any(|p| p == os))
}

/// How a core gets its tokens. Stage 1: the sign-in settings and the store the agent keeps the
/// tokens in; the core refreshes through `auth::Auth` itself. Stage 4 (a token per core's resource,
/// from one sign-in) puts the refresh behind this too.
pub trait TokenProvider: Send + Sync {
    /// The IdP, the device client, the API and the resource.
    fn config(&self) -> &Config;
    /// Where the refresh and access tokens are kept.
    fn store(&self) -> Box<dyn TokenStore>;
    /// The one access token every core uses (refreshed when it's near its end), or None when
    /// signed out. A sidecar core is handed it with each `sync`.
    fn access_token(&self) -> Result<Option<String>, crate::auth::AuthError> {
        let http =
            crate::http::HttpTransport::new().map_err(crate::auth::AuthError::Unavailable)?;
        let store = self.store();
        crate::auth::Auth::new(
            self.config(),
            &http,
            store.as_ref(),
            &crate::util::SystemClock,
        )
        .access_token()
    }
}

/// The real one: the OS keychain, under the issuer's names (`KeyringStore::for_issuer`).
#[derive(Debug, Clone)]
pub struct KeychainTokens {
    config: Config,
}

impl KeychainTokens {
    pub fn new(config: Config) -> Self {
        KeychainTokens { config }
    }
}

impl TokenProvider for KeychainTokens {
    fn config(&self) -> &Config {
        &self.config
    }
    fn store(&self) -> Box<dyn TokenStore> {
        Box::new(KeyringStore::for_issuer(&self.config.issuer))
    }
}

/// What a core is started with.
#[derive(Clone)]
pub struct CoreContext {
    /// Its state folder. Stage 1: the agent's one folder (`state_dir::state_dir`), which the WoW
    /// core has always used.
    pub state_dir: PathBuf,
    pub tokens: Arc<dyn TokenProvider>,
    /// The device key that signs every post to the API (`identity`); None when the keychain
    /// couldn't give it, which the agent reports itself (the posts then go unsigned).
    pub device_key: Option<Arc<dyn DeviceKey>>,
    /// The game folder the person chose, in place of the manifest's `detect.folders`.
    pub game_folder: Option<PathBuf>,
}

/// The agent's handle on a running core: `sync` (a pass now) and `stop`. Cloning shares it; a
/// stopped control stays stopped, so each start gets a new one.
#[derive(Debug, Clone, Default)]
pub struct CoreControl {
    token: CancelToken,
}

impl CoreControl {
    pub fn new() -> Self {
        Self::default()
    }

    /// A pass now: the core's wait ends at once; a pass in progress is followed by another.
    pub fn sync(&self) {
        self.token.wake();
    }

    /// Stop: at once while the core waits, or at the end of the pass it's in.
    pub fn stop(&self) {
        self.token.cancel();
    }

    pub fn is_stopped(&self) -> bool {
        self.token.is_cancelled()
    }

    /// The token under it, for a core whose loop takes one.
    pub fn token(&self) -> &CancelToken {
        &self.token
    }
}

impl From<CancelToken> for CoreControl {
    fn from(token: CancelToken) -> Self {
        CoreControl { token }
    }
}

/// Where a row of the core's (an account, a save) is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Phase {
    Waiting,
    Pushing,
    Pushed,
    Error,
}

/// What the agent's own state takes from one event.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Activity {
    /// A row (by its key, the core's choice: WoW's is the account file's path) moved to a phase.
    pub row: Option<(String, Phase)>,
    /// The words to show for a failure.
    pub error: Option<String>,
    /// A pass went through: the failure shown is cleared.
    pub clears_error: bool,
}

/// A status chip's style (the window's `chip` classes).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Chip {
    Pushed,
    Waiting,
    Update,
    Error,
}

/// One row of a core's status card.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusRow {
    pub label: String,
    pub value: String,
    pub chip: Option<Chip>,
}

impl StatusRow {
    pub fn new(label: &str, value: impl Into<String>, chip: Option<Chip>) -> Self {
        StatusRow {
            label: label.to_string(),
            value: value.into(),
            chip,
        }
    }
}

/// What a core's heartbeat says: which core, its version, and its own fields.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CoreHeartbeat {
    pub core: String,
    pub version: String,
    pub fields: serde_json::Value,
}

/// A core, as the agent drives it.
pub trait Core: Send + Sync {
    /// What it reports while it runs; forwarded to the window as serialised.
    type Event: Serialize + Send + 'static;
    /// Why it couldn't run (not a failed pass: those are events).
    type Error: std::error::Error + Send + Sync + 'static;

    fn manifest(&self) -> &CoreManifest;

    /// Run until `control.stop()`, on the calling thread; `events` hears everything it does.
    /// Returns when stopped, or when it can't run at all.
    fn start(
        &self,
        ctx: &CoreContext,
        control: &CoreControl,
        events: &mut dyn FnMut(Self::Event),
    ) -> Result<(), Self::Error>;

    /// What the agent's state takes from an event.
    fn activity(&self, event: &Self::Event) -> Activity;

    /// An event for a failure the agent saw around the core (it couldn't start, say).
    fn failed(&self, message: String, fix: Fix) -> Self::Event;

    /// The rows of its status card, read fresh from the disk.
    fn status_rows(&self, ctx: &CoreContext) -> Vec<StatusRow>;

    /// What its heartbeat would say now. Stage 1: the core still sends its own after each pass;
    /// this is the same body, for the agent to show or (later) send.
    fn heartbeat(&self, ctx: &CoreContext) -> CoreHeartbeat;
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn wow_like() -> serde_json::Value {
        json!({
            "id": "wow",
            "name": "World of Warcraft: Forever",
            "version": "0.2.5",
            "runtime": { "kind": "builtin" },
            "resource": "https://api.ryna.io/wow/ingest",
            "scopes": ["wow:ingest"],
            "detect": { "folders": ["/Applications/World of Warcraft"] },
            "data": { "reads": ["WTF/Account/*/SavedVariables/Ryna.lua"], "writes": ["Interface/AddOns/Ryna/AgentStatus.lua"] },
            "mods": "mods/wow"
        })
    }

    #[test]
    fn a_manifest_parses_with_every_field() {
        let m = CoreManifest::parse(&wow_like().to_string()).unwrap();
        assert_eq!(m.id, "wow");
        assert_eq!(m.runtime, Runtime::Builtin);
        assert_eq!(m.scopes, ["wow:ingest"]);
        assert_eq!(m.detect.folders.len(), 1);
        assert_eq!(m.mods.as_deref(), Some("mods/wow"));
    }

    #[test]
    fn a_sidecar_runtime_and_the_optional_parts() {
        let mut v = wow_like();
        v["runtime"] = json!({ "kind": "sidecar", "bin": "ryna-palworld" });
        for k in ["detect", "data", "mods"] {
            v.as_object_mut().unwrap().remove(k);
        }
        let m = CoreManifest::parse(&v.to_string()).unwrap();
        assert_eq!(
            m.runtime,
            Runtime::Sidecar {
                bin: "ryna-palworld".into(),
                args: vec![]
            }
        );
        assert_eq!(m.detect, Detect::default());
        assert_eq!(m.mods, None);
        assert_eq!(m.platforms, None);
        assert!(m.runs_here());
    }

    #[test]
    fn platforms_say_where_a_core_runs() {
        let mut v = wow_like();
        v["platforms"] = json!(["macos"]);
        let m = CoreManifest::parse(&v.to_string()).unwrap();
        assert!(runs_on(m.platforms.as_deref(), "macos"));
        assert!(!runs_on(m.platforms.as_deref(), "windows"));
        assert!(runs_on(None, "linux"));
        assert_eq!(m.runs_here(), cfg!(target_os = "macos"));
        v["platforms"] = json!(["amiga"]);
        assert!(CoreManifest::parse(&v.to_string())
            .unwrap_err()
            .to_string()
            .contains("platforms"));
    }

    #[test]
    fn a_bad_manifest_is_refused() {
        assert!(matches!(
            CoreManifest::parse("{"),
            Err(ManifestError::Json(_))
        ));
        let refused = |k: &str, val: serde_json::Value| {
            let mut v = wow_like();
            v[k] = val;
            CoreManifest::parse(&v.to_string()).unwrap_err()
        };
        assert!(refused("id", json!("WoW")).to_string().contains("id"));
        assert!(refused("version", json!("1.2"))
            .to_string()
            .contains("version"));
        assert!(refused("scopes", json!([])).to_string().contains("scopes"));
        assert!(refused("resource", json!("wow"))
            .to_string()
            .contains("resource"));
        assert!(refused("runtime", json!({ "kind": "sidecar", "bin": "" }))
            .to_string()
            .contains("bin"));
        assert!(matches!(
            refused("runtime", json!({ "kind": "wasm" })),
            ManifestError::Json(_)
        ));
    }

    #[test]
    fn a_control_syncs_until_it_stops() {
        let c = CoreControl::new();
        let shared = c.clone();
        assert!(!shared.is_stopped());
        c.sync();
        assert!(!shared.is_stopped());
        c.stop();
        assert!(shared.is_stopped());
    }

    #[test]
    fn rows_and_phases_serialise_as_the_window_reads_them() {
        let row = StatusRow::new("Addon", "Addon 0.2.5 installed", Some(Chip::Pushed));
        assert_eq!(
            serde_json::to_value(&row).unwrap(),
            json!({ "label": "Addon", "value": "Addon 0.2.5 installed", "chip": "pushed" })
        );
        assert_eq!(serde_json::to_value(Phase::Pushing).unwrap(), "pushing");
    }
}
