# WoW mods

Mods for the World of Warcraft: Forever core, one folder each with its `mod.json`
(`cores/wow/core.json` names this folder as its `mods`; the manifest is
`ryna_agent::mods::ModManifest`, docs/ryna-agent-cores-mods.md section 6). A WoW mod is
`kind: "addon"`: its own addon folders under `addon/` (`Ryna_<Name>`, `## Dependencies: Ryna`),
installed beside the core's `Ryna` addon and built on it only through the mod API, the `Ryna` global
(`cores/wow/addon/Ryna/Mods.lua`; the addon README's "The mod API").

- `chronicle/`: **Chronicle**, the first mod, on by default (`defaultOn: true`, `actsInGame: false`):
  everything recorded about a character (the record, bags and bank, quests, the guild roster, zones
  and deaths, the players seen and the census, kills and loot), the tooltips, and the panel's
  Recording and Census tabs. It writes the same `RynaDB` the single addon wrote before stage 2.
- `automation/`: **Automation**, off by default (`defaultOn: false`, `actsInGame: true`): chores at
  quest givers (accept, turn in, quest-only gossip, leave low-level quests, optionally take the
  best-selling reward), vendors (sell greys and marked junk, repair, optionally from the guild bank
  first), trainers (learn everything within `autoTrainCap`, 10 gold by default, set with
  `/ryna auto traincap <gold>`) and the mailbox (take gold and items, never cash on delivery, never
  delete or return), and around the world (resurrections out of combat, invites from friends and
  the guild, bind/roll/delete confirmations, cinematics, quest tracking, release in battlegrounds,
  the game's auto-loot, loot rolls that never Need). Every chore is its own setting
  (`RynaDB.settings.auto*`), off until ticked on its Automation tab or with `/ryna auto <name> on`;
  Shift held, or combat, pauses all of it. Besides its settings it keeps only the items marked as
  junk (`RynaDB.automation.junk`, `/ryna auto junk` with the item on the cursor).
  `test/automation_test.lua` runs its logic against a fake client (`lua5.1`, in the addon workflow).

Bundled mods are compiled into the WoW core (`cores/wow/core/src/mods.rs`, `BUNDLED_MOD_JSON`: add a
new one there; a test holds the list to this folder) and version with the agent
(`scripts/check-versions.mjs` checks each `mod.json` and TOC). The app carries each addon folder
beside `addon/Ryna` (`tauri.conf.json`'s resources), and the release's addon zip holds them beside
`Ryna`. The app's Game tab turns each one on or off (installing or removing its folders); the core
addon's Mods tab turns it on or off in the game's own AddOns list. A mod that `actsInGame` says so
under its toggle, every time.
