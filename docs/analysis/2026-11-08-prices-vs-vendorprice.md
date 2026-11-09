# Prices (stage 1) vs VendorPrice: a read-only comparison

Written 2026-11-08 by Nyx, for Maddison. Analysis only: no code here was changed.

**Theirs:** VendorPrice by Ketho and Icesythe7, github.com/ketho-wow/VendorPrice, read at commit `862957a` (2026-01-29, tag line up to 2.0.0). **Licence: MIT** (copyright 2019 Ketho, Icesythe7). That would let us reuse code with the notice kept, but nothing was copied: the quotes below are a few words each. References to their files are `VendorPrice.lua:n` and `Compat.lua:n` at that commit. The download was deleted afterwards.

**Ours:** [Ryna_Prices](../../mods/wow/prices/addon/Ryna_Prices) 0.3.8, the core helpers it uses ([Core.lua](../../cores/wow/addon/Ryna/Core.lua), [Mods.lua](../../cores/wow/addon/Ryna/Mods.lua)) and [prices_test.lua](../../mods/wow/prices/test/prices_test.lua).

**One correction to the brief.** VendorPrice is not a retail addon. It is Classic only (Era, the Burning Crusade and Mists flavours, `VendorPrice.toc:1`) and switches itself off on retail: `VendorPrice.lua:4-7` and `Compat.lua:3-5` return early when `WOW_PROJECT_ID` is mainline, because retail's stock tooltip already shows the sell price everywhere. Forever is a retail engine whose stock tooltip shows the price only at a merchant (our finding, [design doc 7.3](../ryna-agent-cores-mods.md)), so Forever is the odd case neither world planned for. VendorPrice is still the right comparison: it is the best-known addon solving exactly "the price away from a vendor", and it is small enough to read whole (about 300 lines).

## At a glance

| Topic | Ours (Ryna_Prices) | Theirs (VendorPrice) |
|---|---|---|
| Hook | `TooltipDataProcessor.AddTooltipPostCall(Item)`, else `OnTooltipSetItem` on GameTooltip and ItemRefTooltip ([Tooltip.lua:237-264](../../mods/wow/prices/addon/Ryna_Prices/Tooltip.lua#L237)) | `hooksecurefunc` on 17 GameTooltip setters (`VendorPrice.lua:95-197`), `OnTooltipSetItem` for ItemRefTooltip and auction buttons (`:199-238`), plus chat and other-addon shims (`Compat.lua`) |
| Tooltips covered | Every tooltip that goes through the data processor, comparison tooltips included | Only the setters it lists; no SetHyperlink on GameTooltip, no comparison tooltips |
| Count | The getter's bag, inventory or loot slot, else the owner button's `count`, else a full stack; clamped to the stack size ([Tooltip.lua:106-125,188-190](../../mods/wow/prices/addon/Ryna_Prices/Tooltip.lua#L106)) | Each setter's own API: mail, trade, quest, loot roll, auction, craft, action bar (`VendorPrice.lua:96-192`) |
| Unloaded item | Requests the load, refreshes the same tooltip on `ITEM_DATA_LOAD_RESULT` or `GET_ITEM_INFO_RECEIVED` ([Tooltip.lua:179-186,206-230](../../mods/wow/prices/addon/Ryna_Prices/Tooltip.lua#L179)) | Nothing: no price that hover (`VendorPrice.lua:128`, "item information is not readily available") |
| At a merchant | Line shows by default; a toggle hides it | Hidden, except for character-pane and trade-skill items (`VendorPrice.lua:32-44`) |
| Line | "Sells for 13s 50c (2g 70s for 20)", white text | The stock "Sell Price:" label and coin icons via `SetTooltipMoney`; stack total by default, Shift for one |
| Settings | Three toggles, a panel tab, `/ryna prices` | None |
| Error handling | Every client call under `pcall`; hook errors reported once | No `pcall` anywhere |
| Taint | No Blizzard globals replaced | Replaces the global `SetTooltipMoney` (`VendorPrice.lua:55-63`) |
| Stored, sent | Three booleans in `RynaDB.settings`; no networking | Nothing stored, nothing sent |
| Libraries | None (needs the Ryna core) | None |
| Locales | 11 locale files, 0 strings translated beyond English | Uses the client's `SELL_PRICE` string: every locale for free |
| Tests | 64 checks on a fake client, all passing | None; CI only packages releases |
| On Forever | Built for it | Does nothing (mainline guard), and would fail to load without the guard |

## 1. Approach and structure

Ours makes one decision: hook the data layer once, and treat every item tooltip the same. That buys coverage for free (bags, bank, chat links, loot, merchant, comparison, anything a later patch adds) and keeps the per-context logic small. The cost is that the count has to be inferred after the fact from `processingInfo.getterName` and `getterArgs` ([Tooltip.lua:90-102](../../mods/wow/prices/addon/Ryna_Prices/Tooltip.lua#L90)), so anything other than a bag, inventory or loot slot falls back to the owner button and then to a full stack.

Theirs makes the opposite trade: hook each setter, and use each setter's own count API. Counts are exact everywhere it hooks (a quest reward of 5 potions says 5), but every context is code to write and maintain, and anything unlisted gets nothing. It also needs bespoke patches for double-fired hooks: recipes fire `OnTooltipSetItem` twice, handled by a per-tooltip flip-flop (`VendorPrice.lua:46-52`), and the commit log shows fixes for double prices at the bank and the keyring.

The unloaded-item case is ours clearly: a request plus a targeted refresh only when the same tooltip still shows that item. Theirs simply misses until the next hover. At a merchant, theirs hides its line where the stock UI already shows one, but keeps it for items the stock UI does not price there; ours shows by default, so a player at a Forever merchant probably sees two prices.

## 2. Efficiency

Both are cheap; neither has an OnUpdate, timer or cache, and neither needs one (the client caches item info).

Ours per hover: about ten `pcall`-wrapped calls, up to four `S.On` lookups each scanning three features, two `string.format`s and one or two small tables in `MoneyText` ([Core.lua:161-172](../../cores/wow/addon/Ryna/Core.lua#L161)). Microseconds. Four event registrations; `GET_ITEM_INFO_RECEIVED` fires for every item load in the session (hundreds when a list opens), and each costs a dispatch, a `pcall` and a nil check, because the handler stays registered even when nothing waits. The footprint includes the core, which Prices cannot run without.

Theirs per hover: one or two setter hooks, but also two `OnTooltipSetItem` hooks on GameTooltip that run for every item: one calls `GetMouseFoci` and loops the chat frames (`Compat.lua:55-64`), the other does `GetDebugName` and string searches (`VendorPrice.lua:228-238`). Its `SetTooltipMoney` replacement runs on every money line in the whole UI. Standalone, smaller than ours.

## 3. Robustness and safety

**Client differences.** Ours probes every API (`ns.Fn`, [Core.lua:17-25](../../cores/wow/addon/Ryna/Core.lua#L17)), never touches the missing `GetItemInfo` global, treats an empty answer as "not loaded", reads ids from `Hitem:` for Forever's `|cnIQ2:` links, and refuses 12.x secret values (`ns.Usable`, [Mod.lua:43-45](../../mods/wow/prices/addon/Ryna_Prices/Mod.lua#L43)). Theirs branches on `WOW_PROJECT_ID` and calls Classic APIs unguarded. On Forever it returns at line 5; with the guard removed, `hooksecurefunc` on a setter the retail engine lacks (such as `SetCraftItem`) would raise at load.

**Errors.** Ours runs the hook inside `ns.Run` ([Tooltip.lua:232-234](../../mods/wow/prices/addon/Ryna_Prices/Tooltip.lua#L232)): an error cannot break the tooltip or the next addon's hook, and is reported once. Theirs has no `pcall`; an error in a hook can interrupt the hooks chained after it, on every hover. There are live candidates: `Compat.lua:17` calls `:match` on a link that may be nil, and `Compat.lua:27` walks deep into Auctioneer's private tables.

**Taint.** Theirs replaces `SetTooltipMoney`, a Blizzard global, so every caller of it now runs addon code. Mostly harmless on tooltips, but it is exactly the pattern that causes "action blocked" reports when secure code passes through. Ours only adds post calls and script hooks and calls `AddLine`, which is the safe shape.

**Duplicates.** Ours checks for its own line on the by-hand path ([Tooltip.lua:156-167](../../mods/wow/prices/addon/Ryna_Prices/Tooltip.lua#L156)) and relies on the processor rebuilding the tooltip otherwise. Theirs patches each double case as found.

## 4. Security and privacy

Neither networks, sends addon messages or pulls in libraries. Theirs has no SavedVariables at all. Ours writes three booleans into the shared `RynaDB.settings`; Prices itself writes nothing else ([Mod.lua:6](../../mods/wow/prices/addon/Ryna_Prices/Mod.lua#L6)). Item links from chat are untrusted input on both sides: both pass them only to `C_Item.GetItemInfo` and anchored, linear patterns, so there is no meaningful attack surface. Ours also type-checks every field it reads from tooltip data.

## 5. User experience

Theirs looks native: the stock "Sell Price:" label and coin icons, in the place and style the player already knows, translated by the client. The default is the stack total, with Shift for one, a hidden control that takes effect after "a small delay" (its own comment).

Ours says both at once, "Sells for 13s 50c (2g 70s for 20)", so nobody needs a modifier key; that is better. But "13s 50c" uses English abbreviations, and "Sells for" is untranslated in all ten other locales (every non-English locale file is still a template of `nil`s). A chat link shows a full stack's total ("for 20"), which is useful but also shows on mail, quest and trade items whose real count we did not read. Ours has settings and a clear help text on each toggle; theirs has none.

## 6. Testing and maintenance

Ours has a fake-client suite covering the line, counts, clamping, append-mode data, the unloaded path, refresh guards, the merchant setting, toggles and both hook modes. I ran it: 64 checks, 0 failed (on Lua 5.5, not 5.1). It cannot prove Forever's APIs behave as assumed. Theirs has no tests; maintenance is TOC bumps and reactive fixes, roughly every patch since 2019 (77 commits), shipped by a packager workflow on tag. Ours will need far less per-patch work because it hooks one data path rather than seventeen setters, but it carries the in-game probes the design doc still lists.

## 7. Verdict

**Where ours is better:** coverage from one data-layer hook; the unloaded-item load and refresh; `pcall` around everything, so a bug cannot break tooltips; no taint; secret-value and API-presence guards built for Forever; unit price and stack total together, no Shift; a real test suite; settings with help text.

**Where theirs is better:** exact counts in every context it hooks (quest, mail, trade, loot roll, auction, craft); native look (coin icons, the stock label) and full localisation for free; hides the line at a merchant by default, with a sensible exception for items the stock UI does not price there; standalone and smaller.

**Worth changing in ours, ranked (ideas, not patches):**

1. **Read the real count in more contexts.** Add getter cases for quest rewards, quest log, mail (inbox and send), trade (both sides), loot roll and merchant, using each getter's arguments. Why: today those fall back to a full stack, so a 5-potion quest reward says "for 20". Effort: half a day with tests, plus an in-game check of Forever's getter names.
2. **Show coin icons instead of "g s c".** Use the client's coin-texture formatter where it exists (probed through `ns.Fn`), falling back to `MoneyText`. Why: locale-proof and native-looking. Effort: an hour or two with a test.
3. **Use the client's `SELL_PRICE` string for the label.** Why: the line becomes readable in all eleven locales today, without waiting for translators. Tradeoff: it then reads exactly like the stock line, which matters at merchants (see 4). Effort: under an hour.
4. **Revisit the merchant default.** Check in game whether Forever's stock tooltip shows its own price at a merchant, and for which items; if it does, hide ours there by default, keeping it for items the stock UI does not price. Why: two prices on one tooltip. Effort: an in-game check and a few lines.
5. **Check recipe and append tooltips in game.** If a recipe shows two "Sells for" lines (the recipe and the product), label or drop the second. Effort: small, after the check.
6. **Register the load events only while a tooltip waits.** Why: avoids hundreds of no-op dispatches when a list loads. Effort: under an hour; a small win, last on the list.

**Decided (Maddison, 2026-11-08):** comparison tooltips keep the line; a chat link shows the single price, with no stack total; the label matches the stock one (3); and the line hides at a merchant where the game shows its own price (4).

**Do not imitate:**

- Replacing a Blizzard global such as `SetTooltipMoney` (`VendorPrice.lua:55-63`): taint risk.
- Hooks with no `pcall`, and reaching into another addon's private tables (`Compat.lua:15-33`).
- Per-addon compatibility shims in general: they couple us to other people's internals.
- Disabling ourselves by `WOW_PROJECT_ID`: on Forever that would turn us off.
- The recipe flip-flop flag: state that desyncs if the hook ever fires once.
- A Shift-only control for the unit price.
- Copying code at all: MIT would allow it with the notice kept, but you have decided against it, and nothing here needs it.
