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
L["marked junk"] = true
L["repair"] = true
L["guild bank first"] = true
L["train"] = true
L["mail"] = true
L["resurrections"] = true
L["invites"] = true
L["dialogs"] = true
L["cinematics"] = true
L["track quests"] = true
L["release in battlegrounds"] = true
L["auto-loot"] = true
L["loot rolls"] = true
L["chat lines"] = true
L["quests (%s)"] = true
L["vendor (%s)"] = true
L["trainers and mail (%s)"] = true
L["world (%s)"] = true

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
L["Sold %d marked junk item for %s."] = true
L["Sold %d marked junk items for %s."] = true
-- %d: how many greys, then how many marked junk items; %s: the money they made.
L["Sold %d grey and %d marked junk items for %s."] = true

-- Trainer.lua
-- %s: what was learnt ("Fireball (Rank 2), First Aid"), then what it cost.
L["Learnt %s for %s."] = true
-- %d: how many services were left; %s: the cap ("10g").
L["%d more is over your %s cap or your gold."] = true
L["%d more are over your %s cap or your gold."] = true

-- Mail.lua
-- The pieces of "Took 3 items and 12g 5s from the mail." (%s: the pieces, joined with "%s and %s").
L["%d item"] = true
L["%d items"] = true
L["%s and %s"] = true
L["Took %s from the mail."] = true
L["Left %d cash-on-delivery mail for you."] = true
L["Left %d cash-on-delivery mails for you."] = true
L["Your bags are full, so the rest of the mail is still there."] = true

-- Social.lua
-- %s: who cast it.
L["Accepted %s's resurrection."] = true
L["Accepted a resurrection."] = true
-- In place of the inviter's name when the client gives none.
L["someone"] = true
-- %s: who invited.
L["Joined %s's group (your guild)."] = true
L["Joined %s's group (a friend)."] = true

-- Dialogs.lua
L["Skipped a cinematic. Hold Shift when one starts to watch it."] = true

-- Loot.lua
-- %s: the item's link.
L["Greed on %s."] = true
L["Passed on %s (bind on pickup, and you can't need it)."] = true
L["Passed on %s (no greed on it)."] = true
L["The game's auto-loot is on."] = true
L["The game's auto-loot is off."] = true

-- World.lua
L["The quest tracker is full, so new quests aren't tracked until you untrack one."] = true
L["Released your spirit (battleground)."] = true

-- Tab.lua
L["Automation"] = true
L["Automation does only what you tick here. Nothing is ticked until you tick it."] = true
L["Hold Shift (at an NPC, or when something starts) and nothing happens by itself. Nor in combat."] = true
L["At quest givers"] = true
L["At vendors"] = true
L["At trainers and mailboxes"] = true
L["Around the world"] = true
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
L["Sell the items you marked as junk"] = true
L["Sells what you marked below along with the greys, counted apart. Never a quest item, never "
  .. "anything a vendor won't pay for."] = true
L["Mark or unmark the item on the cursor as junk"] = true
L["Nothing is marked as junk. Pick an item up and press the button (or /ryna auto junk)."] = true
-- Beside each marked item on the tab.
L["remove"] = true
L["Repair"] = true
L["Repairs everything at a vendor who can, with your own gold, and says what it cost."] = true
L["Use the guild bank first"] = true
L["Pays from the guild bank when you're allowed and it covers the cost, else from your own gold."] = true
L["Learn everything a trainer offers, within a cap"] = true
L["Buys every available skill or recipe in the trainer's order, a few a second, while the total "
  .. "stays within the cap below and your gold, and says what it learnt."] = true
-- %s: the cap ("10g").
L["Cap: %s. Change it with /ryna auto traincap <gold>; 0 trains nothing."] = true
L["Take the gold and items from your mail"] = true
L["Empties every mail at a mailbox, one thing at a time, until your bags are full. Never a "
  .. "cash-on-delivery mail (it says how many it left), and it never deletes or returns mail."] = true
L["Accept resurrections"] = true
L["Takes a resurrection someone casts on you. Not in combat: then the dialog stays for you."] = true
L["Accept group invites from friends and your guild"] = true
L["Joins a group when a friend, a Battle.net friend or someone in your guild invites you. Anyone "
  .. "else's invite stays for you to answer."] = true
L["Answer the bind and roll confirmations"] = true
L["Says yes to looting, equipping or using an item that binds to you and to a roll on one. When you "
  .. "delete a good item it types the word for you; the click is still yours."] = true
L["Skip cinematics"] = true
L["Skips in-game cinematics and movies. Hold Shift when one starts to watch it."] = true
L["Track new quests"] = true
L["Adds a quest to the tracker when you accept it, while the tracker has room."] = true
L["Release your spirit in battlegrounds"] = true
L["Releases as soon as you die in a battleground, unless a soulstone or another way back is on "
  .. "offer. Never in a dungeon, raid, arena or the open world."] = true
L["Keep the game's auto-loot on"] = true
L["Turns on the game's own Auto Loot option at every login. Unticking it turns the option off once; "
  .. "after that it's the game's setting."] = true
L["Roll on group loot (never Need)"] = true
L["Greed on every roll that offers it, pass otherwise and on anything that binds to you that you "
  .. "can't need. Your group sees it: every roll is your word to them."] = true
L["Say what it did in chat"] = true
L["One line for what was sold, repaired, picked, learnt, taken or rolled, and for what it left to you."] = true

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
-- /ryna auto's extra lines. %s: the cap ("10g"); %d: how many items are marked.
L["  traincap: %s (/ryna auto traincap <gold> changes it; 0 trains nothing)"] = true
L["  junk: %d item marked (/ryna auto junk marks or unmarks the item on the cursor)"] = true
L["  junk: %d items marked (/ryna auto junk marks or unmarks the item on the cursor)"] = true
-- /ryna auto traincap. %s: the cap.
L["The trainer's cap is %s. /ryna auto traincap <gold> changes it (0 trains nothing)."] = true
L["The trainer's cap is now %s."] = true
-- /ryna auto junk. %s: the item's link.
L["%s is a quest item, so it can't be junk."] = true
L["%s has no vendor price, so it can't be sold as junk."] = true
L["Pick up an item first (or give its item ID): /ryna auto junk marks or unmarks it."] = true
-- In place of an item's name the client doesn't know yet. %d: its item ID.
L["item %d"] = true
L["%s is marked as junk: it's sold with the greys when selling junk is on."] = true
L["%s is no longer junk."] = true
