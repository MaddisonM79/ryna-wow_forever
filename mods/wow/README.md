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
  (GUID), name, zone, subzone, filter, partial?, items: [ { id | currencyId, price, stack, limited,
  currency? } ] }`. `price` is copper for `stack` of the item; `limited` is `numAvailable >= 0`;
  `currency` (`[ { amount, name, itemId | currencyId } ]`) is there only when the item costs
  something besides copper; a slot that sells a currency is `{ currencyId, price, stack, limited }`.
  `filter` is `GetMerchantFilter()`'s value: the window lists only what its filter shows (the game
  sets it to your class when it opens), and 1 (`LE_LOOT_FILTER_ALL`) is the whole list. A list is
  written once it's complete (every slot has an item or currency link): at `MERCHANT_SHOW`, or when
  `GET_ITEM_INFO_RECEIVED` or `MERCHANT_UPDATE` completes it; a visit that never completes writes
  what had loaded at `MERCHANT_CLOSED`, with `partial: true`. One event per visit and filter, at
  most 200 items, and the NPC is done for the login only once its complete, unfiltered list was
  written. Each complete list's fingerprint (a hash over item ids, prices, stacks and the filter)
  is kept in `RynaDB.chronicle.merchants` (`{ hash, at }` by GUID, `GUID#filter` for a filtered
  list): an unchanged list written in the last 7 days isn't written again, a changed one is, and
  fingerprints older than 30 days go at login. It reads `C_MerchantFrame.GetItemInfo` (a table)
  where the client has it, else the classic positional `GetMerchantItemInfo`.
  `test/chronicle_merchant_test.lua` covers it.
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
  docs/ryna-agent-cores-mods.md 7.3): prices on item tooltips. Stage 1 is the vendor line, "Sell
  Price: 13s 50c (2g 70s for 20)" (the client's own label and coin icons, never where the game
  already drew its own price), on every item tooltip (bags, bank, chat links, loot, merchants), from
  `C_Item.GetItemInfo`'s sell price (its 11th value; Forever has no global `GetItemInfo`), which the
  client has for every item it has loaded, away from vendors. The stack's total is for how many the tooltip shows where the client says (bag,
  bank or loot slot, loot roll, quest reward, mail, trade, merchant or buyback, else the slot
  button's count); a chat link, or no count at all, shows one item's price. Items vendors won't buy get no line. An item the client
  hasn't loaded answers nothing at all on Forever (no values, and asking again doesn't load it), so
  the mod asks for it (`C_Item.RequestLoadItemDataByID`) and refreshes the tooltip on
  `ITEM_DATA_LOAD_RESULT` or `GET_ITEM_INFO_RECEIVED` if it still shows that item; items you can
  hover are normally loaded already. Item ids are read from `Hitem:(%d+)`, since Forever's links
  start `|cnIQ2:`. Its settings, on its own Prices tab: `priceSellLine` (on), `priceStackTotal` (on)
  and `priceOnlyAwayFromMerchant` (off; on, the line stays away while a merchant window is open,
  where the game shows its own price). `/ryna prices` lists them, `/ryna prices <name> on|off`
  (`sellLine`, `stackTotal`, `awayFromMerchant`) changes one; nothing goes to chat otherwise, with
  one exception every Ryna addon shares: an error inside its tooltip hook is caught, and the core's
  `ReportError` prints one "an error in … was caught (reported once)" line for it, once per kind
  of error per session. A slot's count is clamped to the item's stack size, and a link never takes
  a button's count. The vendor line writes only its settings. **Owed in game**: `/dump select(11, C_Item.GetItemInfo(2589))` away
  from a vendor, once Linen Cloth is loaded (hover one first: a bare id query doesn't load it), should
  print 13 (its sell price in copper); the global `GetItemInfo(2589)` is nil on Forever. If the
  engine answers 0 or nil there, the line never shows (every item reads as selling for nothing;
  nothing breaks), and the sell price has to come from the served table instead (7.3's stage 2). A later stage adds what
  vendors charge (Chronicle's `merchant` events, pooled by the API).

  **The auction line** (7.3's step 2, `Auction.lua`; the API's contract is docs/wow.md section 17 in
  the api repo). When you open the auction house, and this character's last snapshot was asked for
  15 minutes ago or more (`RynaDB.prices.auctionAt[<player GUID>]`; ATT keeps the same throttle),
  the mod calls `C_AuctionHouse.ReplicateItems()` once; at `REPLICATE_ITEM_LIST_UPDATE` it reads the
  rows (`GetReplicateItemInfo(i)`, `i` from 0) 2,000 a frame, skipping and counting rows with no
  buyout or with `hasAllInfo` false, then summarises each item a slice a frame: `n` listings, `q`
  total quantity, and the unit buyout (`buyoutPrice / count`, rounded to copper) as `min`, `q25` and
  `med`, the 25th and 50th percentiles weighted by quantity. Each listing is held as one number
  until its item is summarised (about 5 MB for the 192,300 rows Stormwind had on 2026-11-08), then
  dropped. The house closing or a logout while rows are read stops the walk and nothing is kept.
  The result goes to `RynaDB.prices.auction[itemId] = { min, q25, med, n, q, at }` (copper, one table
  for the account, replaced per item by each snapshot; entries older than 14 days are dropped at
  login, and past 20,000 items the most recently seen are kept), and up as `auction` journal events
  (`{ at, rows, part, parts, items = { { id, n, q, min, q25, med } } }`, integers only, at most 2,000
  items an event; an item outside the API's limits is kept here and left out of the event). A
  search on the browse tab folds its results in as `{ min = minPrice, q = totalQuantity, at }` (no
  median), unless the item has a snapshot from the last 15 minutes; searches are never sent.
  **What is not stored or sent: anything about a seller or a bidder.** Forever's rows have no owner
  fields (they come back nil), the mod never reads those positions, and the event has no field for
  one; per-listing rows never leave the client. The tooltip line: "AH: 4g 20s each · 31 listed ·
  2 hours ago", with the stack's total when the tooltip says how many ("AH: 4g 20s each (84g for 20)
  · ..."), or "AH: from 10c each · 13754 for sale · just now" where only a search saw the item;
  nothing for an item never seen, and nothing while the auction house window shows that item's
  price itself (a tooltip from its own lists, or an item the browse tab lists). Settings, on the
  Prices tab: `priceAuctionLine` (on), `priceAuctionStackTotal` (on), `priceAuctionSnapshot` (on;
  off means no `ReplicateItems()` and no event, while searches still feed the line). `/ryna prices`
  shows them and the last snapshot's age, rows and items. **Owed in game**: that
  `REPLICATE_ITEM_LIST_UPDATE` fires after `ReplicateItems()` (without it the mod reads the rows a
  minute after asking), what the walk costs a frame on 192k rows, that `GetBrowseResults()`'
  `minPrice` is per unit for stackable goods, and that `AUCTION_HOUSE_BROWSE_RESULTS_UPDATED` is the
  event a search fires. `test/prices_test.lua` runs the vendor line against a fake client,
  `test/auction_test.lua` the auction side.
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
