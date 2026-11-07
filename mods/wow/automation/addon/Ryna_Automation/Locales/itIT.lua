-- Italian (itIT) strings for Ryna Automation. Loaded on every client, applied only when the game runs
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

-- Settings.lua
-- The pieces of the status line "Automation: quests (accept, turn in), vendor (greys)": each
-- feature's short word, and the two groups (%s: the words that are on, joined with ", ").
L["accept"] = nil
L["not low-level ones"] = nil
L["turn in"] = nil
L["pick the reward"] = nil
L["skip quest-only gossip"] = nil
L["greys"] = nil
L["repair"] = nil
L["guild bank first"] = nil
L["chat lines"] = nil
L["quests (%s)"] = nil
L["vendor (%s)"] = nil

-- Quests.lua
-- In place of a quest's title when the client gives none.
L["this quest"] = nil
-- %s: the quest's title.
L["%s looks like an escort, so it's yours to accept."] = nil
-- %s: the quest's title, then an amount of money ("1g 20s").
L["%s costs %s to hand in, so that's yours to do."] = nil
L["The rewards' prices aren't known yet, so this choice is yours."] = nil
L["None of the rewards sells for anything, so this choice is yours."] = nil
-- %s: the item's link, then its vendor price.
L["Took %s, the reward that sells for the most (%s)."] = nil

-- Vendor.lua
-- %s: an amount of money ("3g 41s 7c").
L["Repaired for %s from the guild bank."] = nil
L["Not enough gold to repair (%s)."] = nil
L["Repaired for %s."] = nil
-- %d: how many items; %s: the money they made.
L["Sold %d grey item for %s."] = nil
L["Sold %d grey items for %s."] = nil

-- Tab.lua
L["Automation"] = nil
L["Automation does only what you tick here. Nothing is ticked until you tick it."] = nil
L["Hold Shift while you talk to an NPC and nothing happens by itself. Nor in combat."] = nil
L["At quest givers"] = nil
L["At vendors"] = nil
L["Chat"] = nil
L["Accept quests"] = nil
L["Accepts a quest an NPC, an item or a place offers you. Not quests a player shares, nor ones that "
  .. "read like an escort: those are yours."] = nil
L["Not low-level quests"] = nil
L["Leaves grey (trivial) quests for you to accept by hand, if you want them."] = nil
L["Turn in quests"] = nil
L["Hands in a finished quest and takes its reward when there's only one. With a choice of rewards it "
  .. "stops there for you, and a quest that costs money to hand in is left to you too."] = nil
L["Pick the reward that sells for the most"] = nil
L["With a choice of rewards, takes the one with the highest vendor price and says which in chat."] = nil
L["Skip quest-only gossip"] = nil
L["When an NPC offers nothing but quests, opens one: a finished one first, then a new one. Never a "
  .. "vendor, trainer, flight master, banker, innkeeper, battlemaster or stable master."] = nil
L["Sell grey items"] = nil
L["Sells every grey (poor-quality) item in your bags at a vendor, a few a second, and says what they "
  .. "made. Nothing but grey."] = nil
L["Repair"] = nil
L["Repairs everything at a vendor who can, with your own gold, and says what it cost."] = nil
L["Use the guild bank first"] = nil
L["Pays from the guild bank when you're allowed and it covers the cost, else from your own gold."] = nil
L["Say what it did in chat"] = nil
L["One line for what was sold, repaired or picked, and for what it left to you."] = nil

-- Status.lua
-- The Status tab's and /ryna status's line. %s: the pieces from Settings.lua above.
L["Automation: %s"] = nil
L["Automation: nothing on"] = nil
-- /ryna auto: "acceptQuests: on (but turnInQuests is off)". %s: on or off, then a feature's name.
L["%s (but %s is off)"] = nil
L["/ryna auto <name> on||off changes one. Hold Shift at an NPC to pause it all."] = nil
-- %s: what the player typed.
L["no automation called %s. /ryna auto lists them."] = nil
-- %s: a feature's name, on or off, the name again.
L["%s is %s. /ryna auto %s on||off changes it."] = nil
-- A feature's name and its new state.
L["%s: %s"] = nil
L["/ryna auto: what Automation does for you (/ryna auto <name> on||off changes one)"] = nil
