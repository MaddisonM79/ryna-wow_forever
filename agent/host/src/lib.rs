//! Ryna, the agent: what is the agent's and not any game's (docs/ryna-agent-cores-mods.md section 4),
//! shared by the desktop app, the command line and every core.
//!
//! - [`core`]: the interface between the agent and a core: the manifest (`core.json`), the
//!   [`core::Core`] trait, what a core is started with and how it's told to sync and stop.
//! - [`mods`]: a core's mods: the manifest (`mod.json`) and the core version range it names.
//! - [`auth`]: the device-grant sign-in and token refresh, with the refresh token in the keychain.
//! - [`config`]: the IdP, the device client, the API and the resource, with their overrides.
//! - [`http`]: the HTTP client (and the transport trait the tests script).
//! - [`identity`]: the device key and the submission ID, and the headers that sign every API post.
//! - [`state_dir`]: the agent's state folder and the run lock.
//! - [`fix`]: what fixes a failure, in words with no command in them.
//! - [`cancel`]: the cancel (and wake) handle the loops and the sign-in share.
//! - [`util`]: the clock, file times, and how to name this machine.

pub mod auth;
pub mod cancel;
pub mod config;
pub mod core;
pub mod fix;
pub mod http;
pub mod identity;
pub mod mods;
pub mod state_dir;
pub mod util;

/// The agent's version, sent as `agent` in every snapshot and heartbeat (every crate in the
/// workspace takes the one version).
pub const AGENT_VERSION: &str = env!("CARGO_PKG_VERSION");

/// The User-Agent every request carries: `ryna-agent/<version>`.
pub fn user_agent() -> String {
    format!("ryna-agent/{AGENT_VERSION}")
}
