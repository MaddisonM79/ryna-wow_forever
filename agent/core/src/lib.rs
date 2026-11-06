//! The World of Warcraft: Forever companion agent's library: everything the command line (and,
//! in stage 3, the desktop app) does, without a terminal or a window.
//!
//! - [`discovery`]: the game's install folders and each account's `Ryna.lua`.
//! - [`lua`]: a reader for the Lua the game writes to SavedVariables.
//! - [`snapshot`]: the JSON snapshot the API takes (docs/wow.md section 2), built from `RynaDB`.
//! - [`auth`]: the device-grant sign-in and token refresh, with the refresh token in the keychain.
//! - [`push`]: posting snapshots and heartbeats.
//! - [`state`] and [`runner`]: what was pushed, and one run over every account.
//! - [`watch`]: the loop the desktop app runs while it is open, with status events and a cancel handle.
//! - [`agent_status`]: `AgentStatus.lua`, what the agent tells the addon after every run.
//! - [`addon_install`]: copying the addon into the game.

pub mod addon_install;
pub mod agent_status;
pub mod auth;
pub mod config;
pub mod discovery;
pub mod http;
pub mod lua;
pub mod push;
pub mod runner;
pub mod snapshot;
pub mod state;
pub mod util;
pub mod watch;

/// This agent's version, sent as `agent` in every snapshot and heartbeat.
pub const AGENT_VERSION: &str = env!("CARGO_PKG_VERSION");

/// `ryna-wow/<version>`.
pub fn user_agent() -> String {
    format!("ryna-wow/{AGENT_VERSION}")
}
