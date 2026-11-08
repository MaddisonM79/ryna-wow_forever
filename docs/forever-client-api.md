# World of Warcraft: Forever, the client API

The reference for writing addons against Forever's client. Read it before writing or changing any
`Ryna*` addon. Everything here comes from a source named next to it; where nothing settles a
question it says **unknown** and the probe that would settle it is in "Still owed". If a probe in
game contradicts this page, the probe wins: fix the page in the same change.

## Sources

Citations below use these keys, with a path or line after the key.

| Key | What | Where | Read |
|---|---|---|---|
| Disk | The installed client (read only) | `/Applications/World of Warcraft/_forever_` and its parent | 2026-11-08 |
| ElvUI | ElvUI v16.01, Forever build (custom licence: read, never copy) | `Interface/AddOns/ElvUI*` | 2026-11-08 |
| BenikUI | ElvUI_BenikUI 5.20 (custom licence) | `Interface/AddOns/ElvUI_BenikUI` | 2026-11-08 |
| Questie | Questie 12.6.2 and QuestieDB 1.0.11 (licence in their `LICENSE.md`) | `Interface/AddOns/Questie*` | 2026-11-08 |
| ATT | AllTheThings 5.4.7 (MIT) | `Interface/AddOns/AllTheThings` | 2026-11-08 |
| Pawn | Pawn 2.14.1, VgerCore 1.0.23 (CC BY-NC-ND 3.0) | `Interface/AddOns/Pawn` | 2026-11-08 |
| Doud | Doud's Dungeon Journal 1.1.0 (all rights reserved) | `Interface/AddOns/DoudJournal` | 2026-11-08 |
| BVP | Better Vendor Price 2.1.3 (no licence file) | `Interface/AddOns/BetterVendorPrice` | 2026-11-08 |
| AQ | AutoQuests 3.1.0 (no licence file) | `Interface/AddOns/AutoQuests` | 2026-11-08 |
| Blz | Blizzard's UI source for Forever: Gethe/wow-ui-source, branch `forever`, commit `9dc84f0`, "1.60.1 (70862)" | https://github.com/Gethe/wow-ui-source/tree/forever | 2026-11-08 |
| Docs | Blizzard's generated API docs in that branch | `Interface/AddOns/Blizzard_APIDocumentationGenerated/` | 2026-11-08 |
| W160 | warcraft.wiki.gg, Patch 1.60.1/API changes (Forever against retail 12.1.5) | https://warcraft.wiki.gg/wiki/Patch_1.60.1/API_changes | 2026-11-08 |
| W120 | warcraft.wiki.gg, Patch 12.0.0/API changes | https://warcraft.wiki.gg/wiki/Patch_12.0.0/API_changes | 2026-11-08 |
| WSV | warcraft.wiki.gg, Secret Values | https://warcraft.wiki.gg/wiki/Secret_Values | 2026-11-08 |
| WTOC | warcraft.wiki.gg, TOC format | https://warcraft.wiki.gg/wiki/TOC_format | 2026-11-08 |
| Team | This repo's probes: commit messages, READMEs, code comments | as cited | 2026-11-07 and 08 |

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
| `WOW_PROJECT_ID` | 18, `WOW_PROJECT_CAMELOT` | Combat log header `PROJECT_ID,18` (Disk `Logs/WoWCombatLog-110826_113736.txt`, line 1); https://warcraft.wiki.gg/wiki/WOW_PROJECT_ID; W160 ("Added `WOW_PROJECT_CAMELOT` (18) constant", build 70170) |
| TOC game type | `camelot`, with `forever` as an alias since build 70291; family `mainline` | WTOC; W160 |

Our TOCs already say `## Interface: 16001` (Disk `Ryna/Ryna.toc`). That number is now verified;
`cores/wow/addon/README.md` still calls it "not verified" and should be updated separately.

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
- WTOC warns that an unrecognised game type in `AllowLoadGameType` makes the condition pass, and
  that `forever` is "not yet recognized on standard 12.1.5" (W160). Use `camelot` and pair it with
  `ExcludeLoadGameType`.

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
- **`WOW_PROJECT_ID` alone, or `== WOW_PROJECT_MAINLINE` as "this is retail"**. W160 cites a forum
  thread titled "WOW_PROJECT_ID for Forever seems to be the same as Retail" from before the
  constant was added in build 70170. The current build reports 18 (combat log header above), but
  the history says not to depend on it.
- **`ClassicExpansionAtLeast(...)`**: "ClassicExpansionAtLeast is always true in Forever" (ElvUI
  `LibDualSpec-1.0.lua:72`).
- **`WOW_PROJECT_ID == WOW_PROJECT_CLASSIC`**: false on Forever; "Classic Era; does not include
  Forever" (Pawn `VgerCore.lua:24`).

## The rules of the engine

Each entry: what we expected, what Forever does, how we know, what to do.

### Combat log and `COMBAT_LOG_EVENT_UNFILTERED`

- **Expected**: Classic addons read CLEU.
- **Forever**: registering it is refused. Ryna 0.2.2 raised the "blocked from an action only
  available to the Blizzard UI" dialog at login (Team `mods/wow/chronicle/addon/Ryna_Chronicle/Chronicle.lua:126`).
  Retail 12.0 made CLEU "error when trying to register" (W120), the event carries
  `HasRestrictions = true` (Docs `CombatLogDocumentation.lua:132-133`), and ElvUI registers it only
  `not E.Modern` (ElvUI `Game/Shared/Modules/DataTexts/DPS.lua:67`, `HPS.lua:61`).
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
- **Seen in Forever addons**: "Touching a secret (even comparing it) throws" (Doud `Core.lua:13-15`);
  "Enemy casts are often secret on Forever" (Doud `Core.lua:163`); Questie leaves an NPC manual
  when `issecretvalue(UnitGUID(unit))` (Questie `Modules/Auto/AutoQuesting.lua:225-227`) and
  skips aura reads in Forever combat and instances (`Database/QuestXP/QuestieXP.lua:42-45`); BVP
  avoids `SetTooltipMoney` because "in instance combat the money frame gets secret sizes" (BVP
  `BetterVendorPrice.lua:157`); ChatThrottleLib drops a message whose text is secret (ElvUI
  `ElvUI_Libraries/Game/Shared/Ace3/AceComm-3.0/ChatThrottleLib.lua:282`).
- **Functions**: `issecretvalue`, `issecrettable`, `canaccessvalue`, `canaccesstable`,
  `canaccessallvalues`, `canaccesssecrets`, `hasanysecretvalues`, `scrubsecretvalues`,
  `secretwrap`, `dropsecretaccess` (Docs `FrameScriptDocumentation.lua`), and `C_Secrets.*`
  predicates such as `HasSecretRestrictions()` (retail docs, `SecretPredicateAPIDocumentation.lua`
  on `live`; not checked on `forever`).
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
- **Do instead**: `C_Item.RequestLoadItemDataByID`, then `ITEM_DATA_LOAD_RESULT` or
  `GET_ITEM_INFO_RECEIVED`; or `Item:CreateFromItemID(id):ContinueOnItemLoad` (ATT
  `src/UI/Windows/Auctions.lua:57-59`). `C_Item.GetItemInfoInstant` works but has no price (Team).
  Item links start `|cnIQ2:`; read ids with `Hitem:(%d+)` (Team,
  `mods/wow/prices/addon/Ryna_Prices/Tooltip.lua:26`).
- Forever re-rolled some item qualities ("Smelting Pants green -> blue", Doud `UI.lua:28`) and has
  items no public database knows (Doud `Core.lua:383`).

### Tooltips

- **Forever**: `TooltipDataProcessor.AddTooltipPostCall(Enum.TooltipDataType.Item, ...)` is what
  BVP (`BetterVendorPrice.lua:198`) and Questie (`Modules/Tooltips/Tooltip.lua:572`) use; ATT
  enables it for `app.IsForever` (ATT `src/Modules/Tooltip.lua:909`). `C_TooltipInfo.GetItemByID`
  returns data whose `lines` carry `Enum.TooltipDataLineType.SellPrice` (BVP
  `BetterVendorPrice.lua:140-153`), which BVP uses to tell items vendors refuse. `OnTooltipSetItem`
  is not used by any of them.
- **Do instead**: the data processor, never `HookScript("OnTooltipSetItem")`. Add text with
  `AddLine`, not money frames, in anything that can run in instance combat (BVP, above).

### Containers and the bank

- **Forever**: `C_Container` works: `GetContainerItemLink` (Team), `GetContainerItemInfo`
  (`MayReturnNothing`, Docs `ContainerDocumentation.lua`), `GetContainerNumSlots`,
  `GetContainerNumFreeSlots`, `PickupContainerItem`, `SplitContainerItem` (BVP `Restack.lua:12-118`).
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
- **Unknown**: whether the global `GetMerchantItemInfo` exists on Forever. Chronicle falls back to
  it; on Forever that path is probably dead.

### Trainers

- **Forever**: `GetNumTrainerServices()` answered 0 to an addon while the window listed spells;
  `GetTrainerServiceInfo(i)`, `GetTrainerServiceCost(i)` and `BuyTrainerService(i)` work by index;
  the return order is `(name, kind, texture, requiredLevel, subText)` (Team, commits `5228639`,
  `5f98041`, 2026-11-07). Blizzard's Forever trainer code confirms the order and adds a sixth
  value: `serviceName, serviceType, texture, reqLevel, serviceSubText, category` (Blz
  `Blizzard_TrainerUI/Mainline/Blizzard_TrainerUI.lua:469`). `C_Trainer` has only
  `GetCategorizeTrainerUI`, `SetCategorizeTrainerUI`, `GetTrainerType` (Team `/dump C_Trainer`;
  W160 lists exactly those three as Forever additions; retail's `C_Trainer` has no functions,
  `TrainerDocumentation.lua` on `live`).
- **Contradiction, unresolved**: Blizzard's own list loop is `for index = 1, GetNumTrainerServices()`
  (Blz `Blizzard_TrainerUI.lua:460-468`), so the count is not 0 for Blizzard's code. Either it is
  0 only for tainted callers, or our read came before `TRAINER_UPDATE`. Keep walking by index
  until the first empty slot, as Automation does; the probe is owed.
- The window groups services into categories (`categorizeTrainerUI` CVar) except at pet trainers
  (Blz `Blizzard_TrainerUI/Mainline/Blizzard_TrainerUI_Camelot.lua:9-11`).

### Quests and gossip

- **Forever**: the quest frame is the mainline one and accepts with `AcceptQuest()` on
  `QUEST_DETAIL` (Blz `Blizzard_UIPanels_Game/Mainline/QuestFrame.lua:646`, after
  `AcknowledgeAutoAcceptQuest` for auto-accept quests). AQ, built for 16001, drives the whole flow
  with `C_GossipInfo.GetActiveQuests`, `GetAvailableQuests`, `GetOptions`, `SelectOption`,
  `SelectAvailableQuest`, `SelectActiveQuest` on `GOSSIP_SHOW`, the global
  `GetNumAvailableQuests`/`SelectAvailableQuest` on `QUEST_GREETING`, `AcceptQuest`,
  `CompleteQuest`, `GetNumQuestChoices`, `GetQuestReward` (AQ `main.lua:63-163`). Forever adds
  `C_QuestLog.GetQuestTimers`, `GetTrivialRange`, `IsEliteQuest` (W160). Forever's own quests have
  ids around 86000 and up (Doud `Core.lua:724`).
- **Watch**: the NPC's GUID can be secret, so a quest-giver check by NPC id must fail safe
  (Questie, above). Native quest ids stay readable (Questie `Modules/QuestieCompat.lua:200-202`).

### `/who` and `C_FriendList`

- **Forever**: `/who` only from a key press or click (Team, commit `3e50f74`, 2026-11-07).
  `C_FriendList.SendWho` is `HasRestrictions = true` (retail docs, `FriendListDocumentation.lua`)
  and Forever's Who list passes a third argument, filters: `C_FriendList.SendWho(text, Enum.SocialWhoOrigin.Social, LFGWhoListFilterUtil.GetWhoFilters())`
  (Blz `Blizzard_GroupFinder_VanillaStyle/Mainline/WhoList.lua:178`). Forever adds
  `C_FriendList.GetWhoRaceFilters` and moves the Who list into the group finder (W160; ElvUI
  `Game/Shared/Skins/LFG.lua:963`). Results: `C_FriendList.GetNumWhoResults`, `GetWhoInfo(i)` (a
  table) (Blz `WhoList.lua:268`).
- **Do instead**: read `WHO_LIST_UPDATE` after the player's own `/who`; never send.

### Guild roster

- **Forever**: no roster API change is listed against 12.1.5 (W160), so `C_GuildInfo.GuildRoster`
  and `GetGuildRosterInfo` should behave as on retail; Forever adds preferred-play-settings
  functions (W160). Chronicle's guild scan has run on Forever (Team, `cores/wow/addon/README.md`
  "refreshed at login and every 10 minutes"). Return order of `GetGuildRosterInfo`: **unknown on
  Forever**, read by position with care.

### Group, units and names

- Names: a Forever character has a first and last name, and the client puts the last name where
  the realm goes: "forever 'First Last' otherwise 'First'" (ElvUI `Game/Shared/General/Core.lua:70`);
  "Forever: use the character's last name, unique across all rulesets/realms" (Pawn `Pawn.lua:588`).
  Ryna's identity code already uses `First-Last` (Team `cores/wow/addon/README.md`).
- GUIDs: "the server ID and spawn ID portions of creature, vehicle, pet and game
  object GUIDs have been replaced with obfuscated values" (blue post 2026-10-16 in W160). The NPC
  id field is not mentioned as changed. **Unknown**: whether the obfuscated spawn part is stable
  for one creature, which Chronicle's once-per-GUID kill count relies on.
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
- **Unknown**: everything about behaviour in game (throttles, whether `ReplicateItems` returns the
  whole house, whether prices carry copper, commodities vs items for Forever's goods). Probes first
  in the list below.

### Mail

- **Forever**: the inbox is still the global API: `GetInboxHeaderInfo` (Blz
  `Blizzard_MailFrame/MailFrame.lua:340`), with `C_Mail` holding only `CanCheckInbox`,
  `HasInboxMoney`, `IsCommandPending`, `SetOpeningAll`, `GetCraftingOrderMailInfo` (retail docs,
  `MailInfoDocumentation.lua` on `live`; `C_Mail.CanCheckInbox` at Blz `MailFrame.lua:242`).
  Forever's mail frame differs in layout (ElvUI `Game/Shared/Skins/Mail.lua:81-89`).

### Loot and rolls

- **Forever**: Classic loot methods are back: master looter frames, `GroupLootFrame` additions,
  `C_LootFrame.TryAutoLoot`, `C_AutoLoot.SetUseAutoLootToggle`, `UnitHasLootInteraction` (W160).
  `GetLootSourceInfo` is the per-source read (Team, Chronicle `Chronicle.lua:436`).
  `GetLootRollItemInfo`/`RollOnLoot` are used by Automation; **unverified** on Forever.

### Talents and specs

- **Forever**: three trees, dual spec. Blizzard draws them with the modern trait system: the
  Forever talent frame reads `C_Traits.GetGroupDisplayInfoByTreeID` and
  `C_Traits.GetGroupCurrencyInfo` (points per tree) and switches specs with
  `C_SpecializationInfo.SetActiveSpecGroup` (Blz `Blizzard_PlayerSpells/Camelot/ClassTalents/Blizzard_ClassTalentsFrame.lua:95,112`).
  ElvUI skins `PlayerSpellsFrame.TalentsFrame` with Forever-only tree headers and primary/secondary
  tabs (ElvUI `Game/Shared/Skins/PlayerSpells.lua:172-200`).
- **Unknown**: whether the classic `GetNumTalentTabs`/`GetTalentTabInfo` exist on Forever. No
  Blizzard code on the branch calls them (Blz, grep). Chronicle builds its "Assassination 5 /
  Combat 31 / Subtlety 0" string from them (`Ryna_Chronicle/Character.lua:184-209`); if they are
  absent the string must come from `C_Traits.GetGroupCurrencyInfo`.

### Professions

- **Forever**: skill lines moved into a namespace: `C_SkillInfo.GetNumSkillLines`,
  `GetSkillLineInfo`, `GetSkillLineInfoByID`, `ExpandSkillHeader`, `AbandonSkill` were added and the
  global `AbandonSkill` removed (W160). The professions book is a Forever frame (ElvUI
  `Game/Shared/Skins/Professions.lua:691`).
- **Unknown**: whether the global `GetSkillLineInfo` and `GetProfessions` still exist. Chronicle
  tries `GetProfessions` first and `GetSkillLineInfo` second; add `C_SkillInfo` as a candidate.

### CVars

- `C_CVar.GetCVar` and `SetCVar` exist; `SetCVar` requires a public, writable, non-secure CVar and
  refuses secret arguments (Docs `CVarDocumentation.lua`). Forever adds 138 CVars, among them
  `showKeyring`, `categorizeTrainerUI`, `showQuestLevel`, `instantQuestText`, `showSwingTimer`,
  `hasSelectedExperiencePreset`, and `C_CVar.SetTempCVar` (W160). Which 7 were removed is not
  readable from W160.
- `loadDeprecationFallbacks` decides whether Blizzard's deprecation globals exist at all (Blz,
  above). Never rely on a deprecated global.

### Addon messaging and chat

- `C_ChatInfo.SendAddonMessage` refuses secret arguments (`SecretArguments = "NotAllowed"`);
  `RegisterAddonMessagePrefix` is `AllowedWhenUntainted` (Docs `ChatInfoDocumentation.lua`).
  "Trial accounts can no longer send addon messages" (blue post 2026-10-08 in W160). In dungeons
  and raids chat is in messaging lockdown and chat text can be secret (WSV;
  `C_ChatInfo.InChatMessagingLockdown`, retail docs).
- ATT and ElvUI prefer `C_ChatInfo.SendAddonMessage` and `SendChatMessage` (ATT
  `src/WoW API Wrappers.lua` "ChatInfo APIs").

### SavedVariables and `AddOns.txt`

- SavedVariables were broken in early beta and "Fixed SavedVariables not loading" in build 70009
  (W160). Files live at `WTF/Account/<ACCOUNT>/SavedVariables/` (Team).
- A TOC line naming a missing file logs `Error loading <path>` in `Logs/FrameXML.log` and
  `Logs/General.log` and the rest loads (Disk: the installed Ryna 0.3.8 lacks `Locales/`, logged
  2026-11-08 12:03; AQ's missing `options.lua` then raised `main.lua:213: attempt to call a nil
  value`).
- "An unknown event name aborts the whole file on Forever, so register safely" (Doud
  `Core.lua:845`): retail `RegisterEvent` errors on unknown events. Wrap optional registrations in
  `pcall`.
- **Unknown**: how Forever writes `AddOns.txt`, and whether bindings and macros are server-side
  (Team, `docs/ryna-agent-cores-mods.md`).

### Hardware events and protected actions

- `/who` (above). `SendChatMessage` with `"EMOTE"` from addon code outside instances "now requires
  a hardware event" (blue post 2026-10-08 in W160). Retail flags `CancelAuction`, `PlaceBid`,
  `PostItem`, `PostCommodity`, `StartCommoditiesPurchase` as `HasRestrictions` (Docs
  `AuctionHouseDocumentation.lua`): buying and posting will need a click.
- Ryna's rule stands: an addon may react to the player's action, never manufacture one.

### `/dump`

- `/dump f()` prints nothing at all when `f` returns zero values (Team, 2026-11-08). That is how
  `C_Item.GetItemInfo` answers for an unloaded item (`MayReturnNothing`, Docs).

## Verified facts

| Fact | Status | Source | Date |
|---|---|---|---|
| Build 1.60.1.70862, product `wowf` | on disk | Disk `.build.info`, `Info.plist` | 2026-11-08 |
| Interface 16001 | seen in Forever addons; documented | Disk TOCs; WTOC | 2026-11-08 |
| `WOW_PROJECT_ID` 18 = `WOW_PROJECT_CAMELOT` | on disk (combat log); documented | Disk; W160 | 2026-11-08 |
| TOC game type `camelot` (alias `forever`) | seen in Forever addons; documented | ElvUI `ElvUI.toc:40`; WTOC | 2026-11-08 |
| Engine is retail 12.1.5 plus Forever changes | documented | W160 | 2026-11-08 |
| CLEU refused to addons | verified in game | Team 0.2.2; W120; Docs | 2026-11-07 |
| Secret values active | seen in Forever addons; documented | Doud, Questie, BVP; W160 | 2026-11-08 |
| `GetItemInfo` global nil | verified in game | Team | 2026-11-08 |
| `C_Item.GetItemInfo`: 18 returns, nothing when unloaded | verified in game; documented | Team; Docs | 2026-11-08 |
| Links use `\|cnIQ2:` | verified in game | Team | 2026-11-08 |
| `C_Container.*` works | verified in game; seen in BVP | Team; BVP | 2026-11-08 |
| `TooltipDataProcessor` and `C_TooltipInfo` work | seen in Forever addons | BVP, Questie, ATT | 2026-11-08 |
| Merchant frame reads `C_MerchantFrame.GetItemInfo` | Blizzard source | Blz `MerchantFrame.lua:290` | 2026-11-08 |
| `GetMerchantItemInfo` exists | unknown (removed in retail 12.0) | W120 | 2026-11-08 |
| `GetNumTrainerServices()` is 0 for addons | verified in game; contradicted by Blizzard's loop | Team; Blz | 2026-11-07 |
| `GetTrainerServiceInfo` order (name, kind, texture, level, subText, category) | verified in game; Blizzard source | Team; Blz `Blizzard_TrainerUI.lua:469` | 2026-11-08 |
| `C_Trainer` has 3 functions | verified in game; documented | Team; W160 | 2026-11-08 |
| Quests: `AcceptQuest` on `QUEST_DETAIL`, `C_GossipInfo` | Blizzard source; seen in AQ | Blz `QuestFrame.lua:646`; AQ | 2026-11-08 |
| `/who` needs a hardware event | verified in game | Team `3e50f74` | 2026-11-07 |
| `SendWho` takes a filters argument | Blizzard source | Blz `WhoList.lua:178` | 2026-11-08 |
| Auction house is `C_AuctionHouse` | Blizzard source; seen in ElvUI | Blz AH TOC; ElvUI | 2026-11-08 |
| `QueryAuctionItems` absent | Blizzard source (no caller, doc says replaced) | Blz | 2026-11-08 |
| AH behaviour in game | unknown | | |
| Mail inbox via `GetInboxHeaderInfo` | Blizzard source | Blz `MailFrame.lua:340` | 2026-11-08 |
| Talents via `C_Traits` groups | Blizzard source | Blz Camelot `Blizzard_ClassTalentsFrame.lua:95` | 2026-11-08 |
| `GetTalentTabInfo` exists | unknown | | |
| Spec deprecation globals absent | Blizzard source | Blz `Blizzard_DeprecatedSpecialization.toc` | 2026-11-08 |
| `C_SkillInfo` added, global `AbandonSkill` removed | documented | W160 | 2026-11-08 |
| Creature GUID server and spawn parts obfuscated | documented | W160 (blue post 2026-10-16) | 2026-11-08 |
| Names are First plus Last-as-realm | seen in Forever addons | ElvUI `Core.lua:70`; Pawn `Pawn.lua:588` | 2026-11-08 |
| Unknown event name errors on register | seen in Forever addon | Doud `Core.lua:845` | 2026-11-08 |
| `/dump` prints nothing for zero values | verified in game | Team | 2026-11-08 |

## How to probe

- **One value**: `/dump expr`. Nothing printed means zero values; `nil` printed means one nil.
- **Nil or nothing, for sure**: `/run print(select("#", C_Item.GetItemInfo(2589)))` prints 0 for
  nothing and the number of returns otherwise.
- **Does a function exist**: `/dump GetTalentTabInfo ~= nil` (a bare `/dump GetTalentTabInfo`
  prints nothing when it is nil).
- **Enumerate a namespace**: `/dump C_Trainer` prints its keys (this is how the three `C_Trainer`
  functions were found); for a long one, `/run for k in pairs(C_AuctionHouse) do print(k) end`.
- **Secret or not**: `/dump issecretvalue(UnitGUID("target"))`, in and out of combat.
- **Blizzard's own call** is the cheapest evidence before a probe: clone the `forever` branch
  (`git clone --depth 1 --branch forever --filter=blob:none --sparse https://github.com/Gethe/wow-ui-source.git`)
  and grep it. Its head matches the installed build (1.60.1 70862 on 2026-11-07).
- After a probe, put the answer in this page's table with the date.

### Still owed

Auction house first; each is one line to paste.

1. `/dump C_AuctionHouse ~= nil, QueryAuctionItems ~= nil, GetAuctionItemInfo ~= nil` (anywhere)
2. `/dump C_AuctionHouse.SupportsCopperValues(), C_AuctionHouse.FavoritesAreAvailable()` (at the auctioneer)
3. `/dump C_AuctionHouse.GetBrowseResults()[1]` (after one search in the Browse tab)
4. `/run C_AuctionHouse.ReplicateItems()` then, a minute later, `/dump C_AuctionHouse.GetNumReplicateItems(), C_AuctionHouse.GetReplicateItemInfo(0)` (at the auctioneer; ATT allows this once per 15 minutes)
5. `/dump WOW_PROJECT_ID, WOW_PROJECT_CAMELOT, GetBuildInfo()`
6. `/run print(select("#", C_Item.GetItemInfo(2589)), select(11, C_Item.GetItemInfo(2589)))` (after hovering Linen Cloth, away from a vendor)
7. `/dump GetMerchantItemInfo ~= nil, C_MerchantFrame.GetItemInfo(1)` (at a vendor)
8. `/dump GetNumTrainerServices(), GetTrainerServiceInfo(1)` (trainer window open, a second after it opens)
9. `/dump GetNumTalentTabs ~= nil, GetTalentTabInfo ~= nil, C_ClassTalents.GetActiveConfigID()`
10. `/dump GetProfessions ~= nil, GetSkillLineInfo ~= nil, C_SkillInfo.GetNumSkillLines()`
11. `/dump GetInboxHeaderInfo(1)` (at a mailbox with mail)
12. `/dump GetLootRollItemInfo ~= nil, RollOnLoot ~= nil, GetLootSourceInfo ~= nil`
13. `/dump C_Secrets and C_Secrets.HasSecretRestrictions(), GetCVarBool("loadDeprecationFallbacks")`
14. `/dump UnitGUID("target")` on the same mob twice, a minute apart (is the obfuscated spawn part stable?)

## Links

- Blizzard UI source for Forever: https://github.com/Gethe/wow-ui-source/tree/forever ; trainer
  `Interface/AddOns/Blizzard_TrainerUI/Mainline/Blizzard_TrainerUI.lua`; auction TOC
  `Interface/AddOns/Blizzard_AuctionHouseUI/Blizzard_AuctionHouseUI.toc`; merchant
  `Interface/AddOns/Blizzard_UIPanels_Game/Mainline/MerchantFrame.lua`; mail
  `Interface/AddOns/Blizzard_MailFrame/MailFrame.lua`; who
  `Interface/AddOns/Blizzard_GroupFinder_VanillaStyle/Mainline/WhoList.lua`; talents
  `Interface/AddOns/Blizzard_PlayerSpells/Camelot/ClassTalents/Blizzard_ClassTalentsFrame.lua`;
  quests `Interface/AddOns/Blizzard_UIPanels_Game/Mainline/QuestFrame.lua`.
- API docs for Forever: `Interface/AddOns/Blizzard_APIDocumentationGenerated/` on that branch
  (`ItemDocumentation.lua`, `AuctionHouseDocumentation.lua`, `CombatLogDocumentation.lua`,
  `FrameScriptDocumentation.lua`, `ContainerDocumentation.lua`, `CVarDocumentation.lua`,
  `ChatInfoDocumentation.lua`, `SharedTraitsDocumentation.lua`).
- Retail (`live`, 12.1.5 70783 on 2026-11-04): https://github.com/Gethe/wow-ui-source/tree/live
- Forever's diff against retail: https://warcraft.wiki.gg/wiki/Patch_1.60.1/API_changes and its
  links (`compare/12.1.5..forever` on Gethe; Ketho's BlizzardInterfaceResources `12.1.5..1.60.1`).
- Retail 12.0: https://warcraft.wiki.gg/wiki/Patch_12.0.0/API_changes ,
  https://warcraft.wiki.gg/wiki/Secret_Values
- TOC rules: https://warcraft.wiki.gg/wiki/TOC_format ; project ids:
  https://warcraft.wiki.gg/wiki/WOW_PROJECT_ID ; build info: https://warcraft.wiki.gg/wiki/GetBuildInfo
- Items: https://warcraft.wiki.gg/wiki/API_C_Item.GetItemInfo
- The game: https://warcraft.wiki.gg/wiki/World_of_Warcraft:_Forever
- This repo's probes: `mods/wow/README.md`, `cores/wow/addon/README.md`,
  `docs/ryna-agent-cores-mods.md`, `mods/wow/automation/addon/Ryna_Automation/Trainer.lua`.
