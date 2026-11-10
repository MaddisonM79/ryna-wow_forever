# World of Warcraft: Forever, the client API

The reference for writing addons against Forever's client. Read it before writing or changing any
`Ryna*` addon. Everything here comes from a source named next to it; where nothing settles a
question it says **unknown** and the probe that would settle it is in "Still owed". If a probe in
game contradicts this page, the probe wins: fix the page in the same change.

## Sources

Citations use these keys, with a path or line after. All were read on 2026-11-08.

| Key | What | Where |
|---|---|---|
| Disk | The installed client (read only) | `/Applications/World of Warcraft/_forever_` and its parent |
| ElvUI | ElvUI v16.01, Forever build (custom licence: read, never copy) | `Interface/AddOns/ElvUI*` |
| BenikUI | ElvUI_BenikUI 5.20 (custom licence) | `Interface/AddOns/ElvUI_BenikUI` |
| Questie | Questie 12.6.2 and QuestieDB 1.0.11 (licence in their `LICENSE.md`) | `Interface/AddOns/Questie*` |
| ATT | AllTheThings 5.4.7 (MIT) | `Interface/AddOns/AllTheThings` |
| Pawn | Pawn 2.14.1, VgerCore 1.0.23 (CC BY-NC-ND 3.0) | `Interface/AddOns/Pawn` |
| Doud | Doud's Dungeon Journal 1.1.0 (all rights reserved) | `Interface/AddOns/DoudJournal` |
| BVP | Better Vendor Price 2.1.3 (no licence file) | `Interface/AddOns/BetterVendorPrice` |
| AQ | AutoQuests 3.1.0 (no licence file) | `Interface/AddOns/AutoQuests` |
| Blz | Blizzard's UI source for Forever: Gethe/wow-ui-source, branch `forever`, commit `9dc84f0`, "1.60.1 (70862)" | https://github.com/Gethe/wow-ui-source/tree/forever |
| Docs | Blizzard's generated API docs in that branch | `Interface/AddOns/Blizzard_APIDocumentationGenerated/` |
| W160 | warcraft.wiki.gg, Patch 1.60.1/API changes (Forever against retail 12.1.5) | https://warcraft.wiki.gg/wiki/Patch_1.60.1/API_changes |
| W120 | warcraft.wiki.gg, Patch 12.0.0/API changes | https://warcraft.wiki.gg/wiki/Patch_12.0.0/API_changes |
| WSV | warcraft.wiki.gg, Secret Values | https://warcraft.wiki.gg/wiki/Secret_Values |
| WTOC | warcraft.wiki.gg, TOC format | https://warcraft.wiki.gg/wiki/TOC_format |
| Team | This repo's probes: commit messages, READMEs, code comments | as cited, 2026-11-07 and 08 |

The installed third-party addons are evidence of what the API does, nothing more. Their code is
never copied into ours.

## Which client this is

**Forever is Blizzard's own game, not a private server.** The wiki: released 4 November 2026, a
standalone branch "running concurrently with both retail and Classic", developed as "Camelot"
(https://warcraft.wiki.gg/wiki/World_of_Warcraft:_Forever). The install is a Battle.net product:
`.build.info` names product `wowf` and Blizzard's CDN hosts (Disk `../.build.info`).

| Fact | Value | Source |
|---|---|---|
| Version and build | 1.60.1.70862 | Disk `../.build.info`, `_forever_/World of Warcraft.app/Contents/Info.plist` (`CFBundleVersion`) |
| Product, flavour | `wowf` | Disk `.flavor.info`, `../.product.db`, `WTF/Config.wtf` (`agentUID "wowf"`) |
| Engine | Retail (Midnight) 12.1.5 with Forever's changes; it "shares Mainline WoW's UI architecture, including the vast majority of APIs available in 12.1.5" | W160 intro and its "12.1.5 (69952)" to "1.60.1 (70205)" diff |
| Ruleset | Classic | QuestieDB `src/config.lua:66` (`rules = "Classic"`); Pawn treats it with Classic stats and scales (`Pawn/ScaleTemplates.lua:14`) |
| Interface number | **16001** | Every Forever addon's TOC (Disk `Interface/AddOns/*/*.toc`); WTOC's table ("Forever 16001"); https://warcraft.wiki.gg/wiki/GetBuildInfo (row 1.60.1, build 70790, 16001) |
| `WOW_PROJECT_ID` | 18, `WOW_PROJECT_CAMELOT` (also 18) | In game: `/dump WOW_PROJECT_ID, WOW_PROJECT_CAMELOT` gave `18, 18` (Team, 2026-11-08); combat log header `PROJECT_ID,18` (Disk `Logs/WoWCombatLog-110826_113736.txt`, line 1); https://warcraft.wiki.gg/wiki/WOW_PROJECT_ID; W160 ("Added `WOW_PROJECT_CAMELOT` (18) constant", build 70170) |
| `GetBuildInfo()` | `"1.60.1", "70862", "Nov  6 2026", 16001, "", " "`: six values, the retail shape (no Classic seventh `currentVersion`) | In game (Team, 2026-11-08); shape per https://warcraft.wiki.gg/wiki/GetBuildInfo |
| TOC game type | `camelot`, with `forever` as an alias since build 70291; family `mainline` | WTOC; W160 |

Our TOCs say `## Interface: 16001` (Disk `Ryna/Ryna.toc`), which is verified.

### Declaring the interface and loading per flavour

- A TOC may list several numbers. The Forever addons do (`## Interface: 120105, 16001` in
  `AutoQuests.toc`; ATT lists nine). Retail is 120105, its test realm 120107 (WTOC).
- Flavour-specific TOCs: Questie ships `Questie_Forever.toc` and `Questie_Camelot.toc` (16001) and
  a fallback `Questie.toc` with `## Interface: 00000` titled "game client not supported" (Questie
  `Questie.toc:1-6`). The client picks a game-suffixed TOC first, then a family one (`_Mainline`)
  (WTOC).
- Per-file conditions work: `Game\load_forever.xml [AllowLoadGameType camelot][ExcludeLoadGameType standard, classic]`
  (ElvUI `ElvUI.toc:40`, the exact pattern WTOC recommends). Blizzard's own TOCs use the same
  directives (Blz `Blizzard_TrainerUI/Blizzard_TrainerUI.toc`: `[Family]\Blizzard_TrainerUI_Camelot.lua [AllowLoadGameType camelot]`).
- An unrecognised game type in `AllowLoadGameType` makes the condition pass (WTOC), and retail
  doesn't know `forever` (W160): use `camelot` with `ExcludeLoadGameType`.

### Detecting Forever at runtime

What the Forever addons do:

| Addon | Test | Source |
|---|---|---|
| Pawn | `select(4, GetBuildInfo())` > 16000 and < 20000 | Pawn `VgerCore/VgerCore.lua:25` |
| ATT | the same, `>= 16001 and < 20000` | ATT `src/WoW API Wrappers.lua:3` |
| ElvUI's LibDualSpec | the same range | ElvUI `ElvUI_Libraries/Game/Shared/LibDualSpec-1.0/LibDualSpec-1.0.lua:72-75` |
| ElvUI | `WOW_PROJECT_ID == WOW_PROJECT_CAMELOT`, then `E.Modern = E.Retail or E.Forever` | ElvUI `Game/Shared/General/Initialize.lua:111,114` |

W160's own example is the interface range. **Use the interface range** (16000 to 19999) as the
test: it needs no constant that an older or newer build might lack.

What not to key on:

- **`E.Modern` or anything like it** to mean "retail gameplay". Forever has the retail engine and
  Classic rules: hunter pet happiness, loyalty and diet (ElvUI `Game/Shared/Tags/Shared.lua:1267`),
  a ranged slot (Pawn `VgerCore.lua:50`), three talent trees with dual spec. ElvUI uses `E.Modern`
  only for engine questions (which frame exists, whether CLEU may be registered).
- **`WOW_PROJECT_ID` alone, or `== WOW_PROJECT_MAINLINE` as "this is retail"**: before build 70170
  Forever reported retail's id (forum thread cited in W160). It reports 18 now, but don't depend on it.
- **`ClassicExpansionAtLeast(...)`**: "ClassicExpansionAtLeast is always true in Forever" (ElvUI
  `LibDualSpec-1.0.lua:72`).
- **`WOW_PROJECT_ID == WOW_PROJECT_CLASSIC`**: false on Forever; "Classic Era; does not include
  Forever" (Pawn `VgerCore.lua:24`).

## The rules of the engine

Each entry: what we expected, what Forever does, how we know, what to do.

### Combat log and `COMBAT_LOG_EVENT_UNFILTERED`

- **Expected**: Classic addons read CLEU.
- **Forever**: registering it is refused; Ryna 0.2.2 got the "blocked" dialog at login (Team
  `mods/wow/chronicle/addon/Ryna_Chronicle/Chronicle.lua:126`). Retail 12.0 made it an error (W120),
  the event carries `HasRestrictions = true` (Docs `CombatLogDocumentation.lua:132-133`), and ElvUI
  registers it only `not E.Modern` (ElvUI `Game/Shared/Modules/DataTexts/DPS.lua:67`).
- **Do instead**: the game still writes `Logs/WoWCombatLog-*.txt` (header
  `COMBAT_LOG_VERSION,22,ADVANCED_LOG_ENABLED,1,BUILD_VERSION,1.60.1,PROJECT_ID,18`, Disk). The
  agent reads the file; in game, use chat lines, unit events and `UNIT_HEALTH` as Chronicle does.
- The `CombatLogGetCurrentEventInfo` globals are deprecation shims that load only when the
  `loadDeprecationFallbacks` CVar is on (Blz `Blizzard_DeprecatedCombatLog/Deprecated_CombatLog.lua:4-17`).
- Forever adds a swing timer API instead: `C_SwingTimer`, `PLAYER_SWING` (W160).

### Secret values

- **Expected**: Classic has none.
- **Forever**: Midnight's "addon disarmament changes ... including secrets" are active (blue post
  2026-09-15, quoted in W160). Tainted code that compares, does arithmetic on, takes `#` of,
  indexes or uses a secret as a key gets "an immediate Lua error"; storing, passing and
  concatenating are allowed (WSV). `UnitName` of a non-player unit is secret in combat; cast info
  of other units is secret (WSV). Forever adds `GetFrameStrata` returning a secret (W160).
- **Seen in Forever addons**: comparing a secret throws (Doud `Core.lua:13-15`); enemy casts are
  often secret (Doud `Core.lua:163`); Questie skips an NPC whose GUID is secret
  (`Modules/Auto/AutoQuesting.lua:225-227`) and aura reads in combat and instances
  (`Database/QuestXP/QuestieXP.lua:42-45`); BVP avoids money frames in instance combat (BVP
  `BetterVendorPrice.lua:157`); ChatThrottleLib drops secret text (ElvUI
  `ElvUI_Libraries/Game/Shared/Ace3/AceComm-3.0/ChatThrottleLib.lua:282`).
- **Functions**: `issecretvalue`, `issecrettable`, `canaccessvalue`, `canaccesstable`,
  `canaccessallvalues`, `canaccesssecrets`, `hasanysecretvalues`, `scrubsecretvalues`,
  `secretwrap`, `dropsecretaccess` (Docs `FrameScriptDocumentation.lua`), and `C_Secrets.*`
  predicates such as `HasSecretRestrictions()` (retail docs, `SecretPredicateAPIDocumentation.lua`
  on `live`), which exists and returns true in game (Team, 2026-11-08).
- **Do instead**: pass every value from the game through a guard like the core's `IsSecret`
  before comparing it, and keep combat-time reads to a minimum.

### Items: `GetItemInfo` and loading

- **Expected**: the global `GetItemInfo`, returning nil for an uncached item.
- **Forever**: the global is nil (Team, probe 2026-11-08, `mods/wow/README.md`). It was namespaced
  in 10.2.6 (https://warcraft.wiki.gg/wiki/API_C_Item.GetItemInfo), and no Blizzard code on the
  `forever` branch calls the global (Blz, grep). `C_Item.GetItemInfo` has 18 returns ending in
  `itemDescription` (Docs `ItemDocumentation.lua`; added in 12.0.0 per the wiki page), and is
  flagged `MayReturnNothing = true`: an unloaded item gives **zero values**, not nil, and a bare id
  does not load it (Team probe; Docs).
- **Verified in game (Team, 2026-11-08)**: for loaded Linen Cloth,
  `/run print(select("#", C_Item.GetItemInfo(2589)), select(11, C_Item.GetItemInfo(2589)))` printed
  `18 13 7 5 0 0 nil true`: 18 returns, `sellPrice` 13 copper, then `classID`, `subclassID`,
  `bindType`, `expansionID`, `setID`, `isCraftingReagent`.
- **Do instead**: `C_Item.RequestLoadItemDataByID`, then `ITEM_DATA_LOAD_RESULT` or
  `GET_ITEM_INFO_RECEIVED`; or `Item:CreateFromItemID(id):ContinueOnItemLoad` (ATT
  `src/UI/Windows/Auctions.lua:57-59`). `C_Item.GetItemInfoInstant` works but has no price (Team).
  Item links start `|cnIQ2:`; read ids with `Hitem:(%d+)` (Team,
  `mods/wow/prices/addon/Ryna_Prices/Tooltip.lua:26`).
- Forever re-rolled some item qualities (Doud `UI.lua:28`) and has items no public database knows
  (Doud `Core.lua:383`).

### Tooltips

- **Forever**: BVP (`BetterVendorPrice.lua:198`), Questie (`Modules/Tooltips/Tooltip.lua:572`) and
  ATT (`src/Modules/Tooltip.lua:909`) use `TooltipDataProcessor.AddTooltipPostCall(Enum.TooltipDataType.Item, ...)`,
  never `OnTooltipSetItem`. `C_TooltipInfo.GetItemByID` lines carry
  `Enum.TooltipDataLineType.SellPrice` (BVP `BetterVendorPrice.lua:140-153`).
- **Do instead**: the data processor, never `HookScript("OnTooltipSetItem")`. Add text with
  `AddLine`, not money frames, in anything that can run in instance combat (BVP, above).

### Containers and the bank

- **Forever**: `C_Container` works (Team; BVP `Restack.lua:12-118`); `GetContainerItemInfo` is
  `MayReturnNothing` (Docs `ContainerDocumentation.lua`).
  There is a **keyring** (`GetKeyRingSize`, `showKeyring` CVar, W160). "Forever bank tabs are bag
  slots" (ElvUI `ElvUI_Options/Game/Shared/Bags.lua:134`), with no `bankTabPool` (ElvUI
  `Game/Shared/Skins/Bags.lua:546`).
- **Do instead**: `C_Container.*` only; walk bank bags as bags; count the keyring separately.

### Merchants

- **Forever**: Blizzard's Forever merchant frame is the mainline one
  (Blz `Blizzard_UIPanels_Game/Blizzard_UIPanels_Game.toc:110`, `[Family]\MerchantFrame.lua [AllowLoadGameType mainline]`)
  and reads `C_MerchantFrame.GetItemInfo(index)`, a table (Blz
  `Blizzard_UIPanels_Game/Mainline/MerchantFrame.lua:290`) with `name, texture, price, stackCount,
  numAvailable, isPurchasable, isUsable, hasExtendedCost, currencyID, spellID, isQuestStartItem`
  (Docs `MerchantFrameDocumentation.lua`). Retail 12.0 **removed**
  `GetMerchantItemInfo` (W120). `C_MerchantFrame.SellAllJunkItems` exists (ElvUI
  `Game/Shared/Modules/Bags/Bags.lua:61`).
- **Verified in game (Team, 2026-11-08, a weapons merchant)**: `GetMerchantItemInfo` is **nil**
  (Chronicle's fallback to it is for classic clients only). `C_MerchantFrame.GetItemInfo(1)` gave
  `{ name = "Cutlass", price = 2023, stackCount = 1, numAvailable = -1, isPurchasable = false, ... }`
  with `currencyID` and `spellID` nil. The window showed 20s 23c, so `price` is copper for
  `stackCount` pieces; -1 is unlimited stock. `isPurchasable` was false with 3s 14c on hand, so it
  may mean "can afford now"; **unverified**.

### Trainers

- **Forever**: `GetTrainerServiceInfo(i)`, `GetTrainerServiceCost(i)` and `BuyTrainerService(i)`
  work by index (Team, commits `5228639`, `5f98041`, 2026-11-07). The order is `serviceName,
  serviceType, texture, reqLevel, serviceSubText, category` (Blz
  `Blizzard_TrainerUI/Mainline/Blizzard_TrainerUI.lua:469`). `C_Trainer` has only
  `GetCategorizeTrainerUI`, `SetCategorizeTrainerUI`, `GetTrainerType` (Team `/dump C_Trainer`;
  W160 lists exactly those three as Forever additions; retail's `C_Trainer` has no functions,
  `TrainerDocumentation.lua` on `live`).
- **Resolved in game (Team, 2026-11-08, Brother Joshua, priest trainer, window open)**:
  `/dump GetNumTrainerServices(), GetTrainerServiceInfo(1)` gave `157, "Lesser Heal", "unavailable",
  135929, 10, "Rank 3", ""`, matching Blizzard's own loop (Blz `Blizzard_TrainerUI.lua:460-468`).
  An addon's 0 on 2026-11-07 was probably read at `TRAINER_SHOW`, before the list arrived (a
  guess). Automation's walk by index stays as a safety net.
- The window groups services into categories (`categorizeTrainerUI` CVar) except at pet trainers
  (Blz `Blizzard_TrainerUI/Mainline/Blizzard_TrainerUI_Camelot.lua:9-11`).

### Quests and gossip

- **Forever**: the quest frame is the mainline one and accepts with `AcceptQuest()` on
  `QUEST_DETAIL` (Blz `Blizzard_UIPanels_Game/Mainline/QuestFrame.lua:646`). AQ, built for 16001,
  drives the whole flow with `C_GossipInfo.*` on `GOSSIP_SHOW`, the global
  `GetNumAvailableQuests`/`SelectAvailableQuest` on `QUEST_GREETING`, then `AcceptQuest`,
  `CompleteQuest` and `GetQuestReward` (AQ `main.lua:63-163`). Forever adds
  `C_QuestLog.GetQuestTimers`, `GetTrivialRange`, `IsEliteQuest` (W160). Forever's own quests have
  ids around 86000 and up (Doud `Core.lua:724`).
- **Watch**: the NPC's GUID can be secret, so a quest-giver check by NPC id must fail safe
  (Questie, above). Native quest ids stay readable (Questie `Modules/QuestieCompat.lua:200-202`).

### `/who` and `C_FriendList`

- **Forever**: `/who` only from a key press or click (Team, commit `3e50f74`, 2026-11-07);
  `C_FriendList.SendWho` is `HasRestrictions = true` (retail docs) and takes a third argument,
  filters (Blz `Blizzard_GroupFinder_VanillaStyle/Mainline/WhoList.lua:178`). The Who list lives in
  the group finder (W160). Results: `C_FriendList.GetNumWhoResults`, `GetWhoInfo(i)`, a table
  (Blz `WhoList.lua:268`).
- **Do instead**: read `WHO_LIST_UPDATE` after the player's own `/who`; never send.

### Guild roster

- **Forever**: no roster API change is listed against 12.1.5 (W160), so `C_GuildInfo.GuildRoster`
  and `GetGuildRosterInfo` should behave as on retail; Forever adds preferred-play-settings
  functions (W160). Chronicle's guild scan has run on Forever (Team, `cores/wow/addon/README.md`
  "refreshed at login and every 10 minutes"). Return order of `GetGuildRosterInfo`: **unknown on
  Forever**, read by position with care.

### Group, units and names

- Names: a Forever character has a first and last name, and the last name sits where the realm
  goes (ElvUI `Game/Shared/General/Core.lua:70`; Pawn `Pawn.lua:588`, "unique across all
  rulesets/realms"). Ryna already uses `First-Last` (Team `cores/wow/addon/README.md`).
- GUIDs: "the server ID and spawn ID portions of creature, vehicle, pet and game
  object GUIDs have been replaced with obfuscated values" (blue post 2026-10-16 in W160).
  **Verified in game (Team, 2026-11-08)**: `UnitGUID("target")` on one Defias Cutpurse, three times
  over a minute out of combat, was `"Creature-0-6805-0-2-94-39FF3AAEEF"` each time. It splits on
  `-` as before, with the **NPC id (94)** in field 6. **Unknown**: whether it is secret in combat,
  and whether it survives a `/reload` or matches for another player.
- `UnitName("player")` returning "Unknown" before `PLAYER_LOGIN` was fixed in build 70170 (W160).
- Specs: the `GetSpecialization`-style deprecation globals do not load on Forever
  (`Blizzard_DeprecatedSpecialization.toc`: `AllowLoadGameType: classic, standard`, Blz); use
  `C_SpecializationInfo.*`, as ElvUI does (`Game/Shared/General/API.lua:71-72`).

### Auction house

- **Forever**: the modern auction house. Blizzard's `Blizzard_AuctionHouseUI` declares
  `AllowLoadGameType: mainline, cata, mists` (mainline includes camelot) and loads a Forever
  category file `[Game]\Blizzard_AuctionData.lua [AllowLoadGameType camelot]` (Blz
  `Blizzard_AuctionHouseUI/Blizzard_AuctionHouseUI.toc`). ElvUI skins `Blizzard_AuctionHouseUI` on
  Forever and has a Forever branch because "Forever shows the copper box" in bids (ElvUI
  `Game/Shared/Skins/AuctionHouse.lua:11-15,367`). So `C_AuctionHouse` is the API: `SendBrowseQuery`,
  `GetBrowseResults`, `ReplicateItems`, `GetNumReplicateItems`, `GetReplicateItemInfo`,
  `SupportsCopperValues` and the rest are in Docs `AuctionHouseDocumentation.lua`.
- **Not on Forever, as far as the source shows**: `QueryAuctionItems` and `GetAuctionItemInfo`.
  The only mention of `QueryAuctionItems` on the `forever` branch is the doc string saying
  `ReplicateItems` replaces it (Blz, grep).
- **Verified in game (Team, 2026-11-08, Stormwind auctioneer, 1.60.1.70862):** `C_AuctionHouse` is
  present, `QueryAuctionItems` and `GetAuctionItemInfo` are nil, `SupportsCopperValues()` and
  `FavoritesAreAvailable()` are true. After one search, `GetBrowseResults()[1]` was
  `itemKey { itemID = 1251, itemLevel = 1, ... }, totalQuantity = 13754, minPrice = 10,
  containsOwnerItem = false` (Linen Bandage at 10 copper); before any search it answers nothing.
  `ReplicateItems()`, then a minute later `GetNumReplicateItems()`, gave **192,300** rows: the whole
  house in one pull. `GetReplicateItemInfo(0)` (the index starts at 0) gave `"Simple Branch of
  Power", 135139, 1, 2, false, 10, "REQ_LEVEL_ABBR", 0, 0, 700, 0, nil, nil, nil, nil, 0, 15933,
  true`: the owner fields are **nil**, so the client hands addons no seller names. Prices are per
  listing; divide by `count` for a unit price.
- **Positions** (Docs `AuctionHouseDocumentation.lua`): 18 returns, `name, texture, count, qualityID,
  usable, level, levelType, minBid, minIncrement, buyoutPrice, bidAmount, highBidder,
  bidderFullName, owner, ownerFullName, saleStatus, itemID, hasAllInfo`, so the owner and bidder
  fields are the 12th to 15th. The Prices mod reads only the 3rd, 10th, 17th and 18th. Events:
  `REPLICATE_ITEM_LIST_UPDATE`, `AUCTION_HOUSE_BROWSE_RESULTS_UPDATED` (no payload),
  `AUCTION_HOUSE_BROWSE_RESULTS_ADDED`. The window is `AuctionHouseFrame` with `BrowseResultsFrame`,
  `ItemBuyFrame` and `CommoditiesBuyFrame` (Blz `Blizzard_AuctionHouseUI/Shared/Blizzard_AuctionHouseFrame.xml:4,82,99,107`);
  a browse row's tooltip is GameTooltip owned by the row (Blz `Blizzard_AuctionHouseTableBuilder.lua:79-82`).
- **Still unknown**: the replicate throttle (ATT allows itself one per 15 minutes); whether
  `hasAllInfo` is ever false; whether `minPrice` is per unit for stackable goods (10c reads like it);
  whether `REPLICATE_ITEM_LIST_UPDATE` and `AUCTION_HOUSE_BROWSE_RESULTS_UPDATED` fire on Forever.
  The Prices mod assumes the documented behaviour for each and says so (`mods/wow/README.md`).

### Mail

- **Forever**: the inbox is still the global API: `GetInboxHeaderInfo` (Blz
  `Blizzard_MailFrame/MailFrame.lua:340`), with `C_Mail` holding only `CanCheckInbox`,
  `HasInboxMoney`, `IsCommandPending`, `SetOpeningAll`, `GetCraftingOrderMailInfo` (retail docs,
  `MailInfoDocumentation.lua` on `live`; `C_Mail.CanCheckInbox` at Blz `MailFrame.lua:242`).
  Forever's mail frame differs in layout (ElvUI `Game/Shared/Skins/Mail.lua:81-89`).
- **Verified in game (Team, 2026-11-08)**: `GetInboxHeaderInfo(1)` for an auction-house mail gave
  15 values in Blizzard's order (Blz `MailFrame.lua:340`): icons, sender "Alliance Auction House",
  subject, money 0, CODAmount 0, daysLeft 29.9998 (a fraction), itemCount 1, wasRead false,
  `wasReturned` false, `textCreated` true (`MailFrame.lua:583`), canReply false, isGM false,
  firstItemQuantity 1, and a link with bonus fields after the id
  (`|cnIQ2:|Hitem:2569::::::8:1487:::::::|h[Linen Boots]|h|r`): parse with `Hitem:(%d+)` only.

### Loot and rolls

- **Forever**: Classic loot methods are back: master looter frames, `GroupLootFrame` additions,
  `C_LootFrame.TryAutoLoot`, `C_AutoLoot.SetUseAutoLootToggle`, `UnitHasLootInteraction` (W160).
  `GetLootSourceInfo` is the per-source read (Team, Chronicle `Chronicle.lua:436`).
  `GetLootRollItemInfo`, `RollOnLoot` and `GetLootSourceInfo` all exist (`/dump ... ~= nil` gave
  `true, true, true`, Team, 2026-11-08). Their return shapes on Forever are not yet checked.

### Talents and specs

- **Forever**: three trees, dual spec. Blizzard draws them with the modern trait system: the
  Forever talent frame reads `C_Traits.GetGroupDisplayInfoByTreeID` and
  `C_Traits.GetGroupCurrencyInfo` (points per tree) and switches specs with
  `C_SpecializationInfo.SetActiveSpecGroup` (Blz `Blizzard_PlayerSpells/Camelot/ClassTalents/Blizzard_ClassTalentsFrame.lua:95,112`).
  ElvUI skins `PlayerSpellsFrame.TalentsFrame` with Forever-only tree headers and primary/secondary
  tabs (ElvUI `Game/Shared/Skins/PlayerSpells.lua:172-200`).
- **Verified in game (Team, 2026-11-08)**: `GetNumTalentTabs` and `GetTalentTabInfo` are **nil**;
  `C_ClassTalents.GetActiveConfigID()` is `4954925`. Points per tree come from the trait API, as
  Blizzard's frame gets them: `C_Traits.GetConfigInfo(configID).treeIDs[1]` for the tree,
  `C_Traits.GetGroupDisplayInfoByTreeID(treeID)` for each tree's `groupID` and `displayName`, and
  `C_Traits.GetGroupCurrencyInfo(configID, { groupID })`, whose `currencyInfos[1].spent` is the
  tree's points (Docs `SharedTraitsDocumentation.lua`; the tree header shows exactly that, Blz
  `Blizzard_PlayerSpells/Camelot/ClassTalents/Blizzard_ClassTalentsFrame.lua:331-340`). Chronicle
  reads it this way, with the tab globals kept for classic clients (`Character.lua`,
  `C.ScanTalents`; `mods/wow/chronicle/test/chronicle_talents_test.lua`).
- **Not a level effect**: the probe asked whether the functions exist, and the level-8 character
  only affects the values. Blizzard's Forever source defines `GetTalentTabInfo` only as shims in
  `Blizzard_DeprecatedSpecialization`, which declares `AllowLoadGameType: classic, standard` and so
  never loads on Forever (Blz, grep of `Interface`). Those shims wrap
  `C_SpecializationInfo.GetSpecializationInfo`, whose `pointsSpent` may be a second route; not yet
  checked. Expect zeros before the first talent point.

### Professions

- **Forever**: skill lines moved into a namespace: `C_SkillInfo.GetNumSkillLines`,
  `GetSkillLineInfo`, `GetSkillLineInfoByID`, `ExpandSkillHeader`, `AbandonSkill` were added and the
  global `AbandonSkill` removed (W160). The professions book is a Forever frame (ElvUI
  `Game/Shared/Skins/Professions.lua:691`).
- **Verified in game (Team, 2026-11-08)**: `GetProfessions` exists, the global `GetSkillLineInfo`
  is **nil**, and `C_SkillInfo.GetNumSkillLines()` is 21. Chronicle takes the `GetProfessions` path
  (its skill-line fallback is for classic clients). What `GetProfessions()` returns is not yet checked.

### The game's damage meter

- **Forever**: `Blizzard_DamageMeter` is allowed (`AllowLoadGameType: standard, camelot`), and
  `C_DamageMeter` is documented with `Environment = "All"` (Docs `DamageMeterDocumentation.lua`):
  `IsDamageMeterAvailable`, `GetAvailableCombatSessions` (id, name, duration),
  `GetCombatSessionFromID` / `FromType` (sources with `totalAmount`, `amountPerSecond`,
  `classFilename`, `isLocalPlayer`, deaths), `GetCombatSessionSourceFromID` / `FromType` (a source's
  spells), `GetSessionDurationSeconds`, `ResetAllCombatSessions`; events
  `DAMAGE_METER_COMBAT_SESSION_UPDATED`, `DAMAGE_METER_CURRENT_SESSION_UPDATED`, `DAMAGE_METER_RESET`.
  Meter types: damage done, dps, healing done, hps, absorbs, interrupts, dispels, damage taken,
  avoidable damage taken, deaths, enemy damage taken. Session types: overall 0, current 1, expired 2.
- **Watch**: the session reads are `SecretWhenInCombat`; a source's `name` and `sourceGUID` are
  conditionally secret and `specIconID` always is.
- **Unknown**: that an addon's calls answer on Forever, and what a finished session returns.
- **Do instead of CLEU**: read finished fights from here; leave live bars to the game's meter.

### Collections, the encounter journal and load on demand

- **Blizzard's UI**: `Blizzard_Collections` and `Blizzard_AchievementUI` are not allowed on Forever's
  game type (their TOCs on the `forever` branch list `wrath, cata, mists, mainline` and `mainline,
  tbc, wrath, cata, mists`); `Blizzard_EncounterJournal` lists `standard, classic`.
- **Verified in game (Team, 2026-11-09)**: the journals answer without that UI.
  `C_MountJournal.GetMountIDs()` lists 136 (the `Mount` table has 144 rows) and the 11th value of
  `GetMountInfoByID` is the collected flag, as on retail (2 collected on a level 17 character).
  `C_PetJournal.GetNumPets()` returns listed and owned, `5, 5`: under the journal's default filters
  it lists owned pets only, so the whole set is the `BattlePetSpecies` table (121 rows).
- **The encounter journal is empty.** `C_AddOns.IsAddOnLoadable("Blizzard_EncounterJournal")` is
  `true, ""` (Forever's game type is inside `classic` for TOC purposes), but `EJ_GetNumTiers()` is 0
  and `EJ_GetInstanceByIndex(1, false)` returns nothing, matching wago.tools serving no
  `JournalEncounter*` or `JournalInstance` table for build 1.60.1.70790. Boss loot and abilities are
  not in the client.
- **Load on demand works** for an addon's own folder: `C_AddOns.IsAddOnLoadOnDemand("ElvUI_Options")`
  is true and `C_AddOns.LoadAddOn("ElvUI_Options")` returned true (Team, 2026-11-09).

### CVars

- `C_CVar.SetCVar` requires a public, writable, non-secure CVar and refuses secret arguments (Docs
  `CVarDocumentation.lua`). Forever adds 138 CVars, among them `showKeyring`, `categorizeTrainerUI`,
  `showQuestLevel`, `showSwingTimer`, and `C_CVar.SetTempCVar` (W160).
- `loadDeprecationFallbacks` decides whether Blizzard's deprecation globals exist at all (Blz,
  above). On the probe character it is **true** (`GetCVarBool`, Team, 2026-11-08), so shims such
  as `CombatLogGetCurrentEventInfo` load there. It is a CVar a player can turn off, so never rely
  on a deprecated global.

### Addon messaging and chat

- `C_ChatInfo.SendAddonMessage` refuses secret arguments (`SecretArguments = "NotAllowed"`);
  `RegisterAddonMessagePrefix` is `AllowedWhenUntainted` (Docs `ChatInfoDocumentation.lua`).
  "Trial accounts can no longer send addon messages" (blue post 2026-10-08 in W160). In dungeons
  and raids chat is in messaging lockdown and chat text can be secret (WSV;
  `C_ChatInfo.InChatMessagingLockdown`, retail docs).

### SavedVariables and `AddOns.txt`

- SavedVariables were broken in early beta and "Fixed SavedVariables not loading" in build 70009
  (W160). Files live at `WTF/Account/<ACCOUNT>/SavedVariables/` (Team).
- A TOC line naming a missing file logs `Error loading <path>` in `Logs/FrameXML.log` and the rest
  loads (Disk: the installed Ryna 0.3.8 lacks `Locales/`, logged 2026-11-08 12:03).
- "An unknown event name aborts the whole file on Forever, so register safely" (Doud
  `Core.lua:845`): retail `RegisterEvent` errors on unknown events. Wrap optional registrations in
  `pcall`.
- **Unknown**: how Forever writes `AddOns.txt`, and whether bindings and macros are server-side
  (Team, `docs/ryna-agent-cores-mods.md`).

### Hardware events and protected actions

- `/who` (above); `SendChatMessage` `"EMOTE"` outside instances (blue post 2026-10-08 in W160).
  Auction buying and posting are `HasRestrictions` (Docs `AuctionHouseDocumentation.lua`): a click.
- Ryna's rule stands: an addon may react to the player's action, never manufacture one.

### `/dump`

- `/dump f()` prints nothing at all when `f` returns zero values (Team, 2026-11-08). That is how
  `C_Item.GetItemInfo` answers for an unloaded item (`MayReturnNothing`, Docs).

## Verified facts

"Game" means checked in game by the team; "Blz" and "Docs" mean Blizzard's Forever source and API
docs; "Addons" means seen in the installed Forever addons. All dated 2026-11-08 unless marked.

| Fact | Status | Source |
|---|---|---|
| Build 1.60.1.70862, product `wowf`, interface 16001 | on disk; Game | Disk `.build.info`, TOCs; WTOC |
| `WOW_PROJECT_ID` 18 = `WOW_PROJECT_CAMELOT`; `GetBuildInfo()` gives six values ending 16001, "", " " | Game | Team; W160 |
| TOC game type `camelot` (alias `forever`) | Addons; documented | ElvUI `ElvUI.toc:40`; WTOC |
| Engine is retail 12.1.5 plus Forever changes | documented | W160 |
| CLEU refused to addons | Game (2026-11-07) | Team 0.2.2; W120; Docs |
| Secret values on; `C_Secrets.HasSecretRestrictions()` true | Game; Addons | Team; Doud, Questie, BVP; W160 |
| `GetItemInfo` global nil; `C_Item.GetItemInfo` 18 returns, nothing when unloaded; Linen Cloth sells for 13c | Game; Docs | Team |
| Links start `\|cnIQ2:`; parse ids with `Hitem:(%d+)` | Game | Team |
| `C_Container.*`, `TooltipDataProcessor`, `C_TooltipInfo` work | Game; Addons | Team; BVP, Questie, ATT |
| `GetMerchantItemInfo` nil; `C_MerchantFrame.GetItemInfo` table, `price` copper per `stackCount`, `numAvailable` -1 unlimited | Game; Blz | Team; Blz `MerchantFrame.lua:290`; W120 |
| `GetNumTrainerServices()` counts with the window open (157); `GetTrainerServiceInfo` order name, kind, texture, level, subText, category; `C_Trainer` has 3 functions | Game; Blz | Team; Blz `Blizzard_TrainerUI.lua:469`; W160 |
| Quests: `AcceptQuest` on `QUEST_DETAIL`, `C_GossipInfo` | Blz; Addons | Blz `QuestFrame.lua:646`; AQ |
| `/who` needs a hardware event; `SendWho` takes filters | Game (2026-11-07); Blz | Team `3e50f74`; Blz `WhoList.lua:178` |
| `C_AuctionHouse` present, `QueryAuctionItems` and `GetAuctionItemInfo` nil; browse results as above; `ReplicateItems()` gives the whole house (192,300 rows) with owner fields nil | Game; Docs | Team; Docs |
| Mail via `GetInboxHeaderInfo`, 15 values | Game; Blz | Team; Blz `MailFrame.lua:340` |
| Talent tab globals nil; talents via `C_Traits` groups; `C_ClassTalents.GetActiveConfigID()` works; spec deprecation globals absent | Game; Blz | Team; Blz Camelot `Blizzard_ClassTalentsFrame.lua:95`, `Blizzard_DeprecatedSpecialization.toc` |
| `GetProfessions` exists, global `GetSkillLineInfo` nil, `C_SkillInfo.GetNumSkillLines()` 21 | Game; documented | Team; W160 |
| `GetLootRollItemInfo`, `RollOnLoot`, `GetLootSourceInfo` exist | Game | Team |
| `loadDeprecationFallbacks` true on the probe character | Game | Team |
| Creature GUID: server and spawn parts obfuscated, readable out of combat, stable over a minute, NPC id in field 6 | Game; documented | Team; W160 (blue post 2026-10-16) |
| Names are First plus Last-as-realm | Addons | ElvUI `Core.lua:70`; Pawn `Pawn.lua:588` |
| Unknown event name errors on register | Addons | Doud `Core.lua:845` |
| `/dump` prints nothing for zero values | Game | Team |
| Combat log `ZONE_CHANGE` names the zone as `GetRealZoneText` does (the dungeon inside one), and also a building walked into ("Lion's Pride Inn") and "UNKNOWN AREA" on a loading screen, neither with a `MAP_CHANGE` | Disk (2026-11-10) | 26 logs in `_forever_/Logs`; docs/ryna-agent-cores-mods.md 7.6 |
| `C_MountJournal` lists 136, collected flag is `GetMountInfoByID`'s 11th value; `C_PetJournal.GetNumPets()` is listed, owned (owned only by default); the encounter journal is loadable and empty (`EJ_GetNumTiers()` 0); `C_AddOns.LoadAddOn` loads a load-on-demand addon | Game (2026-11-09); Blz | Team; Blz TOCs on `forever` |

## How to probe

- **One value**: `/dump expr`. Nothing printed means zero values; `nil` printed means one nil.
- **Nil or nothing, for sure**: `/run print(select("#", C_Item.GetItemInfo(2589)))` prints 0 for
  nothing and the number of returns otherwise.
- **Does a function exist**: `/dump GetTalentTabInfo ~= nil` (a bare `/dump GetTalentTabInfo`
  prints nothing when it is nil).
- **Enumerate a namespace**: `/dump C_Trainer`; for a long one,
  `/run for k in pairs(C_AuctionHouse) do print(k) end`.
- **Secret or not**: `/dump issecretvalue(UnitGUID("target"))`, in and out of combat.
- **Blizzard's own call** is the cheapest evidence before a probe: clone the `forever` branch
  (`git clone --depth 1 --branch forever --filter=blob:none --sparse https://github.com/Gethe/wow-ui-source.git`)
  and grep it. Its head matches the installed build (1.60.1 70862 on 2026-11-07).
- After a probe, put the answer in this page's table with the date.

### Still owed

The first fourteen were answered on 2026-11-08 and the journal probes on 2026-11-09, all folded in above. Next, one line each:

1. Talent points per tree (what Chronicle now reads; zeros before the first point), then `/dump C_SpecializationInfo.GetSpecializationInfo(1)`:
   `/run local c=C_ClassTalents.GetActiveConfigID() local t=C_Traits.GetConfigInfo(c).treeIDs[1] for _,d in ipairs(C_Traits.GetGroupDisplayInfoByTreeID(t)) do local g=C_Traits.GetGroupCurrencyInfo(c,{d.groupID})[1] local i=g and g.currencyInfos and g.currencyInfos[1] print(d.displayName, i and i.spent, i and i.spentInTree) end`
2. `/dump issecretvalue(UnitGUID("target")), issecretvalue(UnitName("target"))` (as a macro, in combat with a mob)
3. `/dump UnitGUID("target")` on the same mob after a `/reload` (does the obfuscated spawn part survive a reload?)
4. `/dump GetProfessions()` then `/dump C_SkillInfo.GetSkillLineInfo(1)`
5. `/run C_AuctionHouse.ReplicateItems()` again within 15 minutes of the last one, then `/dump C_AuctionHouse.GetNumReplicateItems()` (is there a throttle?)
6. `/dump GetGuildRosterInfo(1)` (needs a character in a guild)
7. `/dump C_MerchantFrame.GetItemInfo(1).isPurchasable` at a vendor with enough money for item 1 (does it mean "can afford"?)
8. With Prices on and `/ryna debug on`, open the auction house: a "prices: auction snapshot: ..." line within a minute means `REPLICATE_ITEM_LIST_UPDATE` fired; note any stutter
9. `/run local f=CreateFrame("Frame") f:RegisterEvent("AUCTION_HOUSE_BROWSE_RESULTS_UPDATED") f:SetScript("OnEvent",function(_,e) local r=C_AuctionHouse.GetBrowseResults() print(e,#r,r[1] and r[1].minPrice,r[1] and r[1].totalQuantity) end)`, then search for a stackable item whose cheapest listing you can see (is `minPrice` per unit?)
10. `/dump C_DamageMeter ~= nil, C_DamageMeter and C_DamageMeter.IsDamageMeterAvailable()`
11. After a fight, out of combat: `/dump C_DamageMeter.GetAvailableCombatSessions()`
12. Then: `/run local s=C_DamageMeter.GetCombatSessionFromType(1,0) print(#s.combatSources, s.totalAmount, s.durationSeconds) local c=s.combatSources[1] print(c and c.name, c and c.totalAmount, c and c.amountPerSecond)` (the current session's damage done: readable out of combat?)
13. Ryna's window (2026-11-09) uses these, each with a fallback, none yet seen on Forever:
    `/dump C_Item.GetItemIconByID ~= nil, C_CurrencyInfo and C_CurrencyInfo.GetCoinTextureString ~= nil, GetItemIcon ~= nil`;
    `/dump C_XMLUtil.GetTemplateInfo("WowStyle1DropdownTemplate") ~= nil, C_XMLUtil.GetTemplateInfo("SearchBoxTemplate") ~= nil`
    (the character picker falls back to a list of its own, the search box to a plain one);
    `/dump RAID_CLASS_COLORS.MAGE, ITEM_QUALITY_COLORS[2], FACTION_BAR_COLORS and FACTION_BAR_COLORS[5], FACTION_STANDING_LABEL5, LOCALIZED_CLASS_NAMES_MALE and LOCALIZED_CLASS_NAMES_MALE.MAGE, HEADSLOT, RANGEDSLOT`

## Links

The Sources table has the main ones. Beyond it:

- Retail UI source (`live`, 12.1.5 70783 on 2026-11-04): https://github.com/Gethe/wow-ui-source/tree/live ;
  Forever's diff against it is linked from W160 (`compare/12.1.5..forever` on Gethe).
- Wiki pages cited inline: https://warcraft.wiki.gg/wiki/World_of_Warcraft:_Forever ,
  https://warcraft.wiki.gg/wiki/WOW_PROJECT_ID , https://warcraft.wiki.gg/wiki/GetBuildInfo ,
  https://warcraft.wiki.gg/wiki/API_C_Item.GetItemInfo
- This repo's probes: `mods/wow/README.md`, `cores/wow/addon/README.md`,
  `docs/ryna-agent-cores-mods.md`, `mods/wow/automation/addon/Ryna_Automation/Trainer.lua`.
