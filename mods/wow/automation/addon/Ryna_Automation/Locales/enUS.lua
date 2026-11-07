-- English (enUS): every string Ryna Automation shows, keyed by the English text itself. `true` means
-- "the text is the key"; ns.L returns the key for anything no locale file translates, so English is
-- always the fallback. The other Locales/*.lua files are templates for translators, generated from
-- this one: after adding, changing or removing a string here, run (from the repository root)
--
--   node cores/wow/addon/tools/strings.mjs --templates
--
-- which rewrites them with the same keys, keeping every translation already in them. Without
-- --templates it only checks that this list matches what the code uses (L["..."] and N("...")).
-- Debug lines (ns.Debug) are English on purpose and are not listed. A key the core addon's
-- Locales/enUS.lua already has ("on", "off", ...) isn't repeated here: Automation's L falls back to it.
-- The feature names /ryna auto takes (acceptQuests, sellGreys, ...) are commands and stay English.

local _, ns = ...
if ns.off then return end
local L = ns.L

-- Settings.lua
-- The pieces of the status line "Automation: quests (accept, turn in), vendor (greys)": each
-- feature's short word, and the two groups (%s: the words that are on, joined with ", ").
L["accept"] = true
L["not low-level ones"] = true
L["turn in"] = true
L["pick the reward"] = true
L["skip quest-only gossip"] = true
L["greys"] = true
L["repair"] = true
L["guild bank first"] = true
L["chat lines"] = true
L["quests (%s)"] = true
L["vendor (%s)"] = true

-- Quests.lua
-- In place of a quest's title when the client gives none.
L["this quest"] = true
-- %s: the quest's title.
L["%s looks like an escort, so it's yours to accept."] = true
-- %s: the quest's title, then an amount of money ("1g 20s").
L["%s costs %s to hand in, so that's yours to do."] = true
L["The rewards' prices aren't known yet, so this choice is yours."] = true
L["None of the rewards sells for anything, so this choice is yours."] = true
-- %s: the item's link, then its vendor price.
L["Took %s, the reward that sells for the most (%s)."] = true

-- Vendor.lua
-- %s: an amount of money ("3g 41s 7c").
L["Repaired for %s from the guild bank."] = true
L["Not enough gold to repair (%s)."] = true
L["Repaired for %s."] = true
-- %d: how many items; %s: the money they made.
L["Sold %d grey item for %s."] = true
L["Sold %d grey items for %s."] = true

-- Tab.lua
L["Automation"] = true
L["Automation does only what you tick here. Nothing is ticked until you tick it."] = true
L["Hold Shift while you talk to an NPC and nothing happens by itself. Nor in combat."] = true
L["At quest givers"] = true
L["At vendors"] = true
L["Chat"] = true
L["Accept quests"] = true
L["Accepts a quest an NPC, an item or a place offers you. Not quests a player shares, nor ones that "
  .. "read like an escort: those are yours."] = true
L["Not low-level quests"] = true
L["Leaves grey (trivial) quests for you to accept by hand, if you want them."] = true
L["Turn in quests"] = true
L["Hands in a finished quest and takes its reward when there's only one. With a choice of rewards it "
  .. "stops there for you, and a quest that costs money to hand in is left to you too."] = true
L["Pick the reward that sells for the most"] = true
L["With a choice of rewards, takes the one with the highest vendor price and says which in chat."] = true
L["Skip quest-only gossip"] = true
L["When an NPC offers nothing but quests, opens one: a finished one first, then a new one. Never a "
  .. "vendor, trainer, flight master, banker, innkeeper, battlemaster or stable master."] = true
L["Sell grey items"] = true
L["Sells every grey (poor-quality) item in your bags at a vendor, a few a second, and says what they "
  .. "made. Nothing but grey."] = true
L["Repair"] = true
L["Repairs everything at a vendor who can, with your own gold, and says what it cost."] = true
L["Use the guild bank first"] = true
L["Pays from the guild bank when you're allowed and it covers the cost, else from your own gold."] = true
L["Say what it did in chat"] = true
L["One line for what was sold, repaired or picked, and for what it left to you."] = true

-- Status.lua
-- The Status tab's and /ryna status's line. %s: the pieces from Settings.lua above.
L["Automation: %s"] = true
L["Automation: nothing on"] = true
-- /ryna auto: "acceptQuests: on (but turnInQuests is off)". %s: on or off, then a feature's name.
L["%s (but %s is off)"] = true
L["/ryna auto <name> on||off changes one. Hold Shift at an NPC to pause it all."] = true
-- %s: what the player typed.
L["no automation called %s. /ryna auto lists them."] = true
-- %s: a feature's name, on or off, the name again.
L["%s is %s. /ryna auto %s on||off changes it."] = true
-- A feature's name and its new state.
L["%s: %s"] = true
L["/ryna auto: what Automation does for you (/ryna auto <name> on||off changes one)"] = true
