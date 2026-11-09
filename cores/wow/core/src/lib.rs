//! The World of Warcraft: Forever core: everything that knows about the game (its folders,
//! `Ryna.lua`, the combat logs and the addon), driven by the agent (`ryna-agent`) through
//! [`wow_core::WowCore`], its implementation of `ryna_agent::core::Core`.
//!
//! - [`wow_core`]: the core behind the agent's interface, and its manifest (`cores/wow/core.json`).
//! - [`discovery`]: the game's install folders and each account's `Ryna.lua`.
//! - [`lua`]: a reader for the Lua the game writes to SavedVariables, and [`lua_write`] a writer.
//! - [`snapshot`]: the JSON snapshot the API takes (docs/wow.md section 2), built from `RynaDB`.
//! - [`push`]: posting snapshots and heartbeats (and census uploads), signed by the device key.
//! - [`census`]: the census without an account: an account's `seen` events as signed uploads.
//! - [`state`] and [`runner`]: what was pushed, and one run over every account.
//! - [`watch`]: the loop the desktop app runs while it is open, with status events and a cancel handle.
//! - [`agent_status`]: `AgentStatus.lua`, what the agent tells the addon after every run.
//! - [`prices_data`]: `Data/vendors.lua` and `Data/auctions.lua`, the pooled prices brought down for the Prices mod.
//! - [`addon_install`]: copying the addon into the game, and reading the installed one's version.
//! - [`mods`]: the bundled mods (Chronicle first): what's on, installing and removing their folders.
//! - [`game_process`]: whether the game is running (a best guess from the process list).
//! - [`settings`]: the Settings mod: addon settings backed up (encrypted) and copied between characters.
//! - [`status`]: what a front end shows (installs, accounts, the sign-in, the last run).
//! - [`combat`]: the combat log: parsing, encounters, digests, the tailer and its upload queue.
//!
//! The agent's own modules (`auth`, `cancel`, `config`, `fix`, `http`, `util`) moved to
//! `ryna-agent` in stage 1; they're re-exported here under their old names, so `crate::auth` and
//! `ryna_wow_core::auth` still read as they did.

pub mod addon_install;
pub mod agent_status;
pub mod census;
pub mod combat;
pub mod discovery;
pub mod game_process;
pub mod lua;
pub mod lua_write;
pub mod mods;
pub mod prices_data;
pub mod push;
pub mod runner;
pub mod settings;
pub mod snapshot;
pub mod state;
pub mod status;
pub mod watch;
pub mod wow_core;

pub use ryna_agent::{auth, cancel, config, fix, http, identity, util};

pub use ryna_agent::{user_agent, AGENT_VERSION};
