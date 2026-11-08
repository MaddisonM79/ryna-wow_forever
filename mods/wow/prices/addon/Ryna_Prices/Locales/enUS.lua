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
-- The setting names /ryna prices takes (sellLine, stackTotal, awayFromMerchant) are commands and stay
-- English.

local _, ns = ...
if ns.off then return end
local L = ns.L

-- Tooltip.lua
-- The line on item tooltips where the client has no SELL_PRICE label of its own (the client's own,
-- "Sell Price: ...", is used where it has one). %s: money, "13s 50c" or coin icons.
L["Sells for %s"] = true
-- After it, for a stack. %s: the stack's money; %d: how many.
L["(%s for %d)"] = true

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

-- Tab.lua
L["Prices"] = true
L["Prices puts what a vendor pays on item tooltips. It only reads the game's item data: it does nothing "
  .. "for you and says nothing in chat."] = true
-- The checkboxes' help, when hovered.
L["Adds the sell price to every item tooltip: in your bags and bank, in chat links, in loot and at "
  .. "merchants. From the game's own item data, so it works away from vendors. Items vendors won't buy get "
  .. "no line, and where the game shows its own price, this one stays away."] = true
L["After the price, what the stack sells for, wherever the game says how many there are: bags, bank, loot, "
  .. "mail, trade, quest rewards and merchants. A chat link shows one item's price."] = true
L["The line already stays away where the game shows its own price. This hides it for every item while a "
  .. "merchant window is open."] = true

-- Status.lua
-- The Status tab's and /ryna status' line. %s: the piece above.
L["Prices: %s"] = true
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
