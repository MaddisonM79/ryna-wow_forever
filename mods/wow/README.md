# WoW mods

Mods for the World of Warcraft: Forever core go here, one folder each with its `mod.json`
(`cores/wow/core.json` names this folder as its `mods`). Nothing is here yet: the mod plumbing
(manifest, bundled source, the toggles in the agent and the in-game panel, install and remove) is
stage 2, and Chronicle, everything the addon records about a character, becomes the first mod then.

A WoW mod is its own addon folder (`Ryna_<Name>`, `## Dependencies: Ryna`) installed beside the core's
`Ryna` addon, off until the person turns it on. See `docs/ryna-agent-cores-mods.md`, sections 6 and 11.
