//! The World of Warcraft: Forever core: everything that knows about the game (its folders,
//! `Ryna.lua`, the combat logs and the addon), driven by the agent (`ryna-agent`) through
//! [`wow_core::WowCore`], its implementation of `ryna_agent::core::Core`.
//!
//! - [`wow_core`]: the core behind the agent's interface, and its manifest (`cores/wow/core.json`).
//! - [`discovery`]: the game's install folders and each account's `Ryna.lua`.
//! - [`lua`]: a reader for the Lua the game writes to SavedVariables.
//! - [`snapshot`]: the JSON snapshot the API takes (docs/wow.md section 2), built from `RynaDB`.
//! - [`push`]: posting snapshots and heartbeats.
//! - [`state`] and [`runner`]: what was pushed, and one run over every account.
//! - [`watch`]: the loop the desktop app runs while it is open, with status events and a cancel handle.
//! - [`agent_status`]: `AgentStatus.lua`, what the agent tells the addon after every run.
//! - [`addon_install`]: copying the addon into the game, and reading the installed one's version.
//! - [`status`]: what a front end shows (installs, accounts, the sign-in, the last run).
//! - [`combat`]: the combat log: parsing, encounters, digests, the tailer and its upload queue.
//!
//! The agent's own modules (`auth`, `cancel`, `config`, `fix`, `http`, `util`) moved to
//! `ryna-agent` in stage 1; they're re-exported here under their old names, so `crate::auth` and
//! `ryna_wow_core::auth` still read as they did.

pub mod addon_install;
pub mod agent_status;
pub mod combat;
pub mod discovery;
pub mod lua;
pub mod push;
pub mod runner;
pub mod snapshot;
pub mod state;
pub mod status;
pub mod watch;
pub mod wow_core;

pub use ryna_agent::{auth, cancel, config, fix, http, util};

pub use ryna_agent::{user_agent, AGENT_VERSION};
