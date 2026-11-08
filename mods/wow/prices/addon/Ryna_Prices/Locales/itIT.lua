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
-- The line on item tooltips. %s: money, "13s 50c".
L["Sells for %s"] = nil
-- After it, for a stack. %s: the stack's money; %d: how many.
L["(%s for %d)"] = nil

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

-- Tab.lua
L["Prices"] = nil
L["Prices puts what a vendor pays on item tooltips. It only reads the game's item data: it does nothing "
  .. "for you and says nothing in chat."] = nil
-- The checkboxes' help, when hovered.
L["Adds \"Sells for ...\" to every item tooltip: in your bags and bank, in chat links, in loot and at "
  .. "merchants. From the game's own item data, so it works away from vendors. Items vendors won't buy get "
  .. "no line."] = nil
L["After the price, what the stack sells for: the stack in that bag slot, or a full stack of an item "
  .. "you're not holding."] = nil
L["At a merchant the game shows its own sell price, so the line can stay away while the merchant window "
  .. "is open."] = nil

-- Status.lua
-- The Status tab's and /ryna status' line. %s: the piece above.
L["Prices: %s"] = nil
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
