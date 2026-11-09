-- Italian (itIT) strings for Ryna Prices. Loaded on every client, applied only when the game runs
-- in itIT: GetLocale() is "itIT".
--
-- How to translate:
--   - Each line is L["English"] = nil. Replace nil with your text in double quotes:
--       L["Take part in the census"] = "..."
--     A line left as nil shows the English, so a partial translation is fine.
--   - Keep every %s and %d, in the same order (WoW's string.format can't reorder them).
--   - Keep "||" as it is: it prints one "|" in chat. Inside a command (/ryna debug on||off||toggle,
--     /who, /reload) keep the words in English: they are what the player types.
--   - Plurals: a pair like "%d player" / "%d players" is used for 1 / everything else.
--     For a language with more forms, use the wording that reads best for most numbers.
--   - Keys are the English text and must stay exactly as they are; only change the values.
--   - Test: set the client's language (Battle.net -> Settings -> Game Settings -> Text Language,
--     or SET textLocale "itIT" in WTF/Config.wtf, with that language installed), log in, then
--     /ryna, /ryna status and /ryna help.
-- This file is generated from enUS.lua by cores/wow/addon/tools/strings.mjs --templates, which keeps the
-- translations already here; run it after enUS.lua changes.

local _, ns = ...
if ns.off then return end
if ns.Locale() ~= "itIT" then return end
local L = ns.L

-- Tooltip.lua
-- The line on item tooltips where the client has no SELL_PRICE label of its own (the client's own,
-- "Sell Price: ...", is used where it has one). %s: money, "13s 50c" or coin icons.
L["Sells for %s"] = nil
-- After it, for a stack (and in the auction line). %s: the stack's money; %d: how many.
L["(%s for %d)"] = nil
-- The auction line's price. %s: money, one item's.
L["%s each"] = nil
-- The auction line, from a snapshot. %s: the price ("4g 20s each", the stack's total after it);
-- %d: how many listings; %s: how long ago, the core's "2 hours ago".
L["AH: %s · %d listed · %s"] = nil
-- The auction line where only a browse search saw the item: its lowest price and how many are up.
L["AH: from %s · %d for sale · %s"] = nil
-- Everyone's vendor line (Pooled.lua's table). %s: the vendor and its zone ("Kiro, Durotar"); %s: the price.
L["Sold by %s: %s"] = nil
-- The vendor's price for a bundle. %s: money; %d: how many the bundle holds.
L["%s for %d"] = nil
-- A vendor whose name the table lacks.
L["a vendor"] = nil
-- After the vendor line when more vendors sell it. %d: how many more.
L["(+%d more)"] = nil
-- After the vendor or everyone's auction line when one person alone saw the price.
L["unconfirmed"] = nil
-- After the vendor line when the item also costs something besides copper.
L["+ other costs"] = nil
-- Everyone's auction line. %s: the price ("1s 10c each", the stack's total after it); %d: listings
-- (the mean per snapshot); %s: how long ago the newest snapshot was, "2 hours ago".
L["AH (everyone): %s · %d listed · as of %s"] = nil
-- Everyone's auction line where only older days this week have data. %s: money; %d: snapshots.
L["AH (everyone): %s each this week · %d snapshots"] = nil

-- Settings.lua
-- The checkboxes (also /ryna prices' lines).
L["Show what a vendor pays on item tooltips"] = nil
L["Add what the whole stack sells for"] = nil
L["Not while a merchant window is open"] = nil
-- The status line's piece, "Prices: sell line on (with the stack's total, everywhere)".
L["sell line off"] = nil
L["sell line on (with the stack's total, away from merchants)"] = nil
L["sell line on (with the stack's total, everywhere)"] = nil
L["sell line on (no stack total, away from merchants)"] = nil
L["sell line on (no stack total, everywhere)"] = nil
L["Show auction house prices on item tooltips"] = nil
L["Add what the whole stack goes for"] = nil
L["Take a snapshot of the auction house when you open it"] = nil
L["Show which vendor sells it, from everyone's visits"] = nil
L["Show everyone's auction house prices on item tooltips"] = nil
-- The status line's auction piece, "Prices: auction line on, snapshots on; ...".
L["auction line on, snapshots on"] = nil
L["auction line on, snapshots off"] = nil
L["auction line off, snapshots on"] = nil
L["auction line off, snapshots off"] = nil

-- Tab.lua
L["Prices"] = nil
L["Prices puts what a vendor pays, which vendor sells an item, and what the auction house asks, on item "
  .. "tooltips. It reads the game's item data, the auction house you open, and the price tables the companion "
  .. "app brings down (read at login or /reload): it does nothing for you and says nothing in chat."] = nil
-- The checkboxes' help, when hovered.
L["Adds the sell price to every item tooltip: in your bags and bank, in chat links, in loot and at "
  .. "merchants. From the game's own item data, so it works away from vendors. Items vendors won't buy get "
  .. "no line, and where the game shows its own price, this one stays away."] = nil
L["After the price, what the stack sells for, wherever the game says how many there are: bags, bank, loot, "
  .. "mail, trade, quest rewards and merchants. A chat link shows one item's price."] = nil
L["The line already stays away where the game shows its own price. This hides it for every item while a "
  .. "merchant window is open."] = nil
L["Adds what the item goes for at the auction house: the median price each from your last snapshot, how "
  .. "many were listed and how long ago. From your own visits only. Not shown while the auction house window "
  .. "shows the item's price itself."] = nil
L["After the auction price, what the stack would go for, wherever the game says how many there are."] = nil
L["When you open the auction house, at most once every 15 minutes, reads the whole house over a few seconds "
  .. "and keeps each item's prices for the line above. The prices per item (never who posted them) go to your "
  .. "Ryna journal, so the companion app can pool them. Off: no snapshot and nothing sent; searches still "
  .. "update the line."] = nil
L["Adds which vendor sells the item and for how much, from everyone's merchant visits pooled by the companion "
  .. "app: the best known seller (confirmed, then cheapest) and how many more there are. The app brings the table "
  .. "down with each sync; the game reads it at login or /reload."] = nil
L["Adds everyone's auction house numbers for the item, in its own colour under your own line: today's median "
  .. "price, how many were listed and how old the newest snapshot is. From the snapshots everyone's Prices mod "
  .. "took, pooled by the companion app and brought down with each sync; the game reads it at login or /reload."] = nil

-- Status.lua
-- The Status tab's and /ryna status' line. %s: the piece above.
L["Prices: %s"] = nil
-- /ryna prices and the status line: the last snapshot. %s: how long ago; %d: rows read; %d: items.
L["last auction snapshot %s: %d rows, %d items"] = nil
L["no auction snapshot yet"] = nil
-- After the auction piece when the core addon predates trimming auction events from its journal.
L["nothing sent: the Ryna addon is too old for it"] = nil
-- Pooled.lua: the status line's third piece, everyone's tables.
L["no pooled prices: the Ryna addon is too old for them"] = nil
L["pooled prices from another version: update the companion app and the addon"] = nil
L["no pooled prices yet: sync the companion app, then /reload"] = nil
-- %s: how long ago the tables' rows last changed; %d: items with a vendor; %d: items with auction prices.
L["pooled prices last changed %s: %d items with a vendor, %d with auction prices"] = nil
-- /ryna prices. %s: on or off, then a setting's name (sellLine).
L["%s (but %s is off)"] = nil
L["/ryna prices <name> on||off changes one."] = nil
-- %s: what was typed.
L["no price setting called %s. /ryna prices lists them."] = nil
-- %s: the setting's name, on or off, the name again.
L["%s is %s. /ryna prices %s on||off changes it."] = nil
-- %s: the setting's name; %s: on or off.
L["%s: %s"] = nil
L["/ryna prices: the price lines on item tooltips (/ryna prices <name> on||off changes one)"] = nil
