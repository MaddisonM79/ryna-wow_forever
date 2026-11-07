-- Chinese (Traditional) (zhTW) strings for Ryna Automation. Loaded on every client, applied only when the game runs
-- in zhTW: GetLocale() is "zhTW".
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
--     or SET textLocale "zhTW" in WTF/Config.wtf, with that language installed), log in, then
--     /ryna, /ryna status and /ryna help.
-- This file is generated from enUS.lua by cores/wow/addon/tools/strings.mjs --templates, which keeps the
-- translations already here; run it after enUS.lua changes.

local _, ns = ...
if ns.off then return end
if ns.Locale() ~= "zhTW" then return end
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
L["marked junk"] = nil
L["repair"] = nil
L["guild bank first"] = nil
L["train"] = nil
L["mail"] = nil
L["resurrections"] = nil
L["invites"] = nil
L["dialogs"] = nil
L["cinematics"] = nil
L["track quests"] = nil
L["release in battlegrounds"] = nil
L["auto-loot"] = nil
L["loot rolls"] = nil
L["chat lines"] = nil
L["quests (%s)"] = nil
L["vendor (%s)"] = nil
L["trainers and mail (%s)"] = nil
L["world (%s)"] = nil

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
L["Sold %d marked junk item for %s."] = nil
L["Sold %d marked junk items for %s."] = nil
-- %d: how many greys, then how many marked junk items; %s: the money they made.
L["Sold %d grey and %d marked junk items for %s."] = nil

-- Trainer.lua
-- %s: what was learnt ("Fireball (Rank 2), First Aid"), then what it cost.
L["Learnt %s for %s."] = nil
-- %d: how many services were left; %s: the cap ("10g").
L["%d more is over your %s cap or your gold."] = nil
L["%d more are over your %s cap or your gold."] = nil

-- Mail.lua
-- The pieces of "Took 3 items and 12g 5s from the mail." (%s: the pieces, joined with "%s and %s").
L["%d item"] = nil
L["%d items"] = nil
L["%s and %s"] = nil
L["Took %s from the mail."] = nil
L["Left %d cash-on-delivery mail for you."] = nil
L["Left %d cash-on-delivery mails for you."] = nil
L["Your bags are full, so the rest of the mail is still there."] = nil

-- Social.lua
-- %s: who cast it.
L["Accepted %s's resurrection."] = nil
L["Accepted a resurrection."] = nil
-- In place of the inviter's name when the client gives none.
L["someone"] = nil
-- %s: who invited.
L["Joined %s's group (your guild)."] = nil
L["Joined %s's group (a friend)."] = nil

-- Dialogs.lua
L["Skipped a cinematic. Hold Shift when one starts to watch it."] = nil

-- Loot.lua
-- %s: the item's link.
L["Greed on %s."] = nil
L["Passed on %s (bind on pickup, and you can't need it)."] = nil
L["Passed on %s (no greed on it)."] = nil
L["The game's auto-loot is on."] = nil
L["The game's auto-loot is off."] = nil

-- World.lua
L["The quest tracker is full, so new quests aren't tracked until you untrack one."] = nil
L["Released your spirit (battleground)."] = nil

-- Tab.lua
L["Automation"] = nil
L["Automation does only what you tick here. Nothing is ticked until you tick it."] = nil
L["Hold Shift (at an NPC, or when something starts) and nothing happens by itself. Nor in combat."] = nil
L["At quest givers"] = nil
L["At vendors"] = nil
L["At trainers and mailboxes"] = nil
L["Around the world"] = nil
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
L["Sell the items you marked as junk"] = nil
L["Sells what you marked below along with the greys, counted apart. Never a quest item, never "
  .. "anything a vendor won't pay for."] = nil
L["Mark or unmark the item on the cursor as junk"] = nil
L["Nothing is marked as junk. Pick an item up and press the button (or /ryna auto junk)."] = nil
-- Beside each marked item on the tab.
L["remove"] = nil
L["Repair"] = nil
L["Repairs everything at a vendor who can, with your own gold, and says what it cost."] = nil
L["Use the guild bank first"] = nil
L["Pays from the guild bank when you're allowed and it covers the cost, else from your own gold."] = nil
L["Learn everything a trainer offers, within a cap"] = nil
L["Buys every available skill or recipe in the trainer's order, a few a second, while the total "
  .. "stays within the cap below and your gold, and says what it learnt."] = nil
-- %s: the cap ("10g").
L["Cap: %s. Change it with /ryna auto traincap <gold>; 0 trains nothing."] = nil
L["Take the gold and items from your mail"] = nil
L["Empties every mail at a mailbox, one thing at a time, until your bags are full. Never a "
  .. "cash-on-delivery mail (it says how many it left), and it never deletes or returns mail."] = nil
L["Accept resurrections"] = nil
L["Takes a resurrection someone casts on you. Not in combat: then the dialog stays for you."] = nil
L["Accept group invites from friends and your guild"] = nil
L["Joins a group when a friend, a Battle.net friend or someone in your guild invites you. Anyone "
  .. "else's invite stays for you to answer."] = nil
L["Answer the bind and roll confirmations"] = nil
L["Says yes to looting, equipping or using an item that binds to you and to a roll on one. When you "
  .. "delete a good item it types the word for you; the click is still yours."] = nil
L["Skip cinematics"] = nil
L["Skips in-game cinematics and movies. Hold Shift when one starts to watch it."] = nil
L["Track new quests"] = nil
L["Adds a quest to the tracker when you accept it, while the tracker has room."] = nil
L["Release your spirit in battlegrounds"] = nil
L["Releases as soon as you die in a battleground, unless a soulstone or another way back is on "
  .. "offer. Never in a dungeon, raid, arena or the open world."] = nil
L["Keep the game's auto-loot on"] = nil
L["Turns on the game's own Auto Loot option at every login. Unticking it turns the option off once; "
  .. "after that it's the game's setting."] = nil
L["Roll on group loot (never Need)"] = nil
L["Greed on every roll that offers it, pass otherwise and on anything that binds to you that you "
  .. "can't need. Your group sees it: every roll is your word to them."] = nil
L["Say what it did in chat"] = nil
L["One line for what was sold, repaired, picked, learnt, taken or rolled, and for what it left to you."] = nil

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
-- /ryna auto's extra lines. %s: the cap ("10g"); %d: how many items are marked.
L["  traincap: %s (/ryna auto traincap <gold> changes it; 0 trains nothing)"] = nil
L["  junk: %d item marked (/ryna auto junk marks or unmarks the item on the cursor)"] = nil
L["  junk: %d items marked (/ryna auto junk marks or unmarks the item on the cursor)"] = nil
-- /ryna auto traincap. %s: the cap.
L["The trainer's cap is %s. /ryna auto traincap <gold> changes it (0 trains nothing)."] = nil
L["The trainer's cap is now %s."] = nil
-- /ryna auto junk. %s: the item's link.
L["%s is a quest item, so it can't be junk."] = nil
L["%s has no vendor price, so it can't be sold as junk."] = nil
L["Pick up an item first (or give its item ID): /ryna auto junk marks or unmarks it."] = nil
-- In place of an item's name the client doesn't know yet. %d: its item ID.
L["item %d"] = nil
L["%s is marked as junk: it's sold with the greys when selling junk is on."] = nil
L["%s is no longer junk."] = nil
