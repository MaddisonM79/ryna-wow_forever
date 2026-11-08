# WoW mods

Mods for the World of Warcraft: Forever core, one folder each with its `mod.json`
(`cores/wow/core.json` names this folder as its `mods`; the manifest is
`ryna_agent::mods::ModManifest`, docs/ryna-agent-cores-mods.md section 6). A WoW mod is
`kind: "addon"`: its own addon folders under `addon/` (`Ryna_<Name>`, `## Dependencies: Ryna`),
installed beside the core's `Ryna` addon and built on it only through the mod API, the `Ryna` global
(`cores/wow/addon/Ryna/Mods.lua`; the addon README's "The mod API"). Or it is `kind: "agent"`: no
addon folder at all, the work done by the agent (installing and removing it do nothing; its toggle
is the whole of it).

- `chronicle/`: **Chronicle**, the first mod, on by default (`defaultOn: true`, `actsInGame: false`):
  everything recorded about a character (the record, bags and bank, quests, the guild roster, zones
  and deaths, the players seen and the census, kills and loot), the tooltips, and the panel's
  Recording and Census tabs. It writes the same `RynaDB` the single addon wrote before stage 2.
  Among its journal events is `merchant` (`Merchants.lua`, the `merchants` setting, on, on the
  Recording tab; docs/ryna-agent-cores-mods.md 7.3): when the player opens a merchant, `{ npc
  (GUID), name, zone, subzone, items: [ { id, price, stack, limited, currency? } ] }`, where `price`
  is copper for `stack` of the item, `limited` is `numAvailable >= 0`, and `currency` (`[ { amount,
  name, itemId | currencyId } ]`) is there only when the item costs something besides copper. Once
  per NPC per login, at most 200 items; written at `MERCHANT_SHOW`, or at the visit's first
  `MERCHANT_UPDATE` when links were still loading. It reads `C_MerchantFrame.GetItemInfo` (a table)
  where the client has it, else the classic positional `GetMerchantItemInfo`. `test/chronicle_merchant_test.lua` covers it.
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
- `prices/`: **Prices**, off by default (`defaultOn: false`, `actsInGame: false`;
  docs/ryna-agent-cores-mods.md 7.3): prices on item tooltips. Stage 1 is the vendor line, "Sells
  for 13s 50c (2g 70s for 20)", on every item tooltip (bags, bank, chat links, loot, merchants), from
  `C_Item.GetItemInfo`'s sell price (its 11th value; Forever has no global `GetItemInfo`), which the
  client has for every item it has loaded, away from vendors. The stack's total is for the stack in that slot where the tooltip says (bag,
  bank or loot slot, or the slot button's count), else for a full stack of a stackable item; a slot
  holding one shows the single price. Items vendors won't buy get no line. An item the client
  hasn't loaded answers nothing at all on Forever (no values, and asking again doesn't load it), so
  the mod asks for it (`C_Item.RequestLoadItemDataByID`) and refreshes the tooltip on
  `ITEM_DATA_LOAD_RESULT` or `GET_ITEM_INFO_RECEIVED` if it still shows that item; items you can
  hover are normally loaded already. Item ids are read from `Hitem:(%d+)`, since Forever's links
  start `|cnIQ2:`. Its settings, on its own Prices tab: `priceSellLine` (on), `priceStackTotal` (on)
  and `priceOnlyAwayFromMerchant` (off; on, the line stays away while a merchant window is open,
  where the game shows its own price). `/ryna prices` lists them, `/ryna prices <name> on|off`
  (`sellLine`, `stackTotal`, `awayFromMerchant`) changes one; nothing goes to chat otherwise. It
  writes only its settings. **Owed in game**: `/dump select(11, C_Item.GetItemInfo(2589))` away
  from a vendor, once Linen Cloth is loaded (hover one first: a bare id query doesn't load it), should
  print 13 (its sell price in copper); the global `GetItemInfo(2589)` is nil on Forever. If the
  engine answers 0 or nil there, the line never shows (every item reads as selling for nothing;
  nothing breaks), and the sell price has to come from the served table instead (7.3's stage 2). Later stages add what
  vendors charge (Chronicle's `merchant` events, pooled by the API) and an auction-house line to the
  same mod. `test/prices_test.lua` runs it against a fake client.
- `settings/`: **Settings**, off by default (`kind: "agent"`, `defaultOn: false`, `actsInGame:
  false`; docs/ryna-agent-cores-mods.md 7.2): addon settings copied between characters and backed
  up to ryna.io, encrypted on this machine (the API's docs/wow.md section 15). All of it is the core's
  `settings` module (`cores/wow/core/src/settings/`): the inventory (each addon's SavedVariables from
  its TOC, the realm and character folders, the game's own per-character files), the copy (files as
  they are; AceDB databases, ElvUI's `ElvPrivateDB` among them, edited for the target), and the
  backups. Every write refuses while the game runs and keeps a `<file>.ryna.bak` of what it
  replaced. It has no addon and says nothing in the game. In the app it's the *Addon settings* tab;
  on the command line `ryna settings …` (agent/README.md).

Bundled mods are compiled into the WoW core (`cores/wow/core/src/mods.rs`, `BUNDLED_MOD_JSON`: add a
new one there; a test holds the list to this folder) and version with the agent
(`scripts/check-versions.mjs` checks each `mod.json` and TOC; an agent mod has only the `mod.json`).
The app carries each addon folder
beside `addon/Ryna` (`tauri.conf.json`'s resources), and the release's addon zip holds them beside
`Ryna`. The app's Mods tab turns each one on or off (installing or removing its folders); the core
addon's Mods tab turns it on or off in the game's own AddOns list. A mod that `actsInGame` says so
under its toggle, every time.
