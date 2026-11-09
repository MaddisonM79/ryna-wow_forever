-- English (enUS): every string Ryna Prices shows, keyed by the English text itself. `true` means "the
-- text is the key"; ns.L returns the key for anything no locale file translates, so English is always
-- the fallback. The other Locales/*.lua files are templates for translators, generated from this one:
-- after adding, changing or removing a string here, run (from the repository root)
--
--   node cores/wow/addon/tools/strings.mjs --templates
--
-- which rewrites them with the same keys, keeping every translation already in them. Without
-- --templates it only checks that this list matches what the code uses (L["..."] and N("...")).
-- Debug lines (ns.Debug) are English on purpose and are not listed. A key the core addon's
-- Locales/enUS.lua already has ("on", "off", ...) isn't repeated here: Prices' L falls back to it.
-- The setting names /ryna prices takes (sellLine, stackTotal, awayFromMerchant, auctionLine,
-- auctionStackTotal, auctionSnapshot) are commands and stay English.

local _, ns = ...
if ns.off then return end
local L = ns.L

-- Tooltip.lua
-- The line on item tooltips where the client has no SELL_PRICE label of its own (the client's own,
-- "Sell Price: ...", is used where it has one). %s: money, "13s 50c" or coin icons.
L["Sells for %s"] = true
-- After it, for a stack (and in the auction line). %s: the stack's money; %d: how many.
L["(%s for %d)"] = true
-- The auction line's price. %s: money, one item's.
L["%s each"] = true
-- The auction line, from a snapshot. %s: the price ("4g 20s each", the stack's total after it);
-- %d: how many listings; %s: how long ago, the core's "2 hours ago".
L["AH: %s · %d listed · %s"] = true
-- The auction line where only a browse search saw the item: its lowest price and how many are up.
L["AH: from %s · %d for sale · %s"] = true
-- Everyone's vendor line (Pooled.lua's table). %s: the vendor and its zone ("Kiro, Durotar"); %s: the price.
L["Sold by %s: %s"] = true
-- The vendor's price for a bundle. %s: money; %d: how many the bundle holds.
L["%s for %d"] = true
-- A vendor whose name the table lacks.
L["a vendor"] = true
-- After the vendor line when more vendors sell it. %d: how many more.
L["(+%d more)"] = true
-- After the vendor or everyone's auction line when one person alone saw the price.
L["unconfirmed"] = true
-- After the vendor line when the item also costs something besides copper.
L["+ other costs"] = true
-- Everyone's auction line. %s: the price ("1s 10c each", the stack's total after it); %d: listings
-- (the mean per snapshot); %s: how long ago the newest snapshot was, "2 hours ago".
L["AH (everyone): %s · %d listed · as of %s"] = true
-- Everyone's auction line where only older days this week have data. %s: money; %d: snapshots.
L["AH (everyone): %s each this week · %d snapshots"] = true

-- Settings.lua
-- The checkboxes (also /ryna prices' lines).
L["Show what a vendor pays on item tooltips"] = true
L["Add what the whole stack sells for"] = true
L["Not while a merchant window is open"] = true
-- The status line's piece, "Prices: sell line on (with the stack's total, everywhere)".
L["sell line off"] = true
L["sell line on (with the stack's total, away from merchants)"] = true
L["sell line on (with the stack's total, everywhere)"] = true
L["sell line on (no stack total, away from merchants)"] = true
L["sell line on (no stack total, everywhere)"] = true
L["Show auction house prices on item tooltips"] = true
L["Add what the whole stack goes for"] = true
L["Take a snapshot of the auction house when you open it"] = true
L["Show which vendor sells it, from everyone's visits"] = true
L["Show everyone's auction house prices on item tooltips"] = true
-- The status line's auction piece, "Prices: auction line on, snapshots on; ...".
L["auction line on, snapshots on"] = true
L["auction line on, snapshots off"] = true
L["auction line off, snapshots on"] = true
L["auction line off, snapshots off"] = true

-- Tab.lua
L["Prices"] = true
L["Prices puts what a vendor pays, which vendor sells an item, and what the auction house asks, on item "
  .. "tooltips. It reads the game's item data, the auction house you open, and the price tables the companion "
  .. "app brings down (read at login or /reload): it does nothing for you and says nothing in chat."] = true
-- The checkboxes' help, when hovered.
L["Adds the sell price to every item tooltip: in your bags and bank, in chat links, in loot and at "
  .. "merchants. From the game's own item data, so it works away from vendors. Items vendors won't buy get "
  .. "no line, and where the game shows its own price, this one stays away."] = true
L["After the price, what the stack sells for, wherever the game says how many there are: bags, bank, loot, "
  .. "mail, trade, quest rewards and merchants. A chat link shows one item's price."] = true
L["The line already stays away where the game shows its own price. This hides it for every item while a "
  .. "merchant window is open."] = true
L["Adds what the item goes for at the auction house: the median price each from your last snapshot, how "
  .. "many were listed and how long ago. From your own visits only. Not shown while the auction house window "
  .. "shows the item's price itself."] = true
L["After the auction price, what the stack would go for, wherever the game says how many there are."] = true
L["When you open the auction house, at most once every 15 minutes, reads the whole house over a few seconds "
  .. "and keeps each item's prices for the line above. The prices per item (never who posted them) go to your "
  .. "Ryna journal, so the companion app can pool them. Off: no snapshot and nothing sent; searches still "
  .. "update the line."] = true
L["Adds which vendor sells the item and for how much, from everyone's merchant visits pooled by the companion "
  .. "app: the cheapest known seller and how many more there are. The app brings the table down with each sync; "
  .. "the game reads it at login or /reload."] = true
L["Adds everyone's auction house numbers for the item, in its own colour under your own line: today's median "
  .. "price, how many were listed and how old the newest snapshot is. From the snapshots everyone's Prices mod "
  .. "took, pooled by the companion app and brought down with each sync; the game reads it at login or /reload."] = true

-- Status.lua
-- The Status tab's and /ryna status' line. %s: the piece above.
L["Prices: %s"] = true
-- /ryna prices and the status line: the last snapshot. %s: how long ago; %d: rows read; %d: items.
L["last auction snapshot %s: %d rows, %d items"] = true
L["no auction snapshot yet"] = true
-- After the auction piece when the core addon predates trimming auction events from its journal.
L["nothing sent: the Ryna addon is too old for it"] = true
-- Pooled.lua: the status line's third piece, everyone's tables.
L["no pooled prices: the Ryna addon is too old for them"] = true
L["pooled prices from another version: update the companion app and the addon"] = true
L["no pooled prices yet: sync the companion app, then /reload"] = true
-- %s: how long ago the tables were written; %d: items with a vendor; %d: items with auction prices.
L["pooled prices as of %s: %d items with a vendor, %d with auction prices"] = true
-- /ryna prices. %s: on or off, then a setting's name (sellLine).
L["%s (but %s is off)"] = true
L["/ryna prices <name> on||off changes one."] = true
-- %s: what was typed.
L["no price setting called %s. /ryna prices lists them."] = true
-- %s: the setting's name, on or off, the name again.
L["%s is %s. /ryna prices %s on||off changes it."] = true
-- %s: the setting's name; %s: on or off.
L["%s: %s"] = true
L["/ryna prices: the price lines on item tooltips (/ryna prices <name> on||off changes one)"] = true
