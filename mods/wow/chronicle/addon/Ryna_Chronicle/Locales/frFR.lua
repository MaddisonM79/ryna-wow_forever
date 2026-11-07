-- French (frFR) strings for Ryna Chronicle. Loaded on every client, applied only when the game runs
-- in frFR: GetLocale() is "frFR".
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
--     or SET textLocale "frFR" in WTF/Config.wtf, with that language installed), log in, then
--     /ryna, /ryna status and /ryna help.
-- This file is generated from enUS.lua by cores/wow/addon/tools/strings.mjs --templates, which keeps the
-- translations already here; run it after enUS.lua changes.

local _, ns = ...
if ns.off then return end
if ns.Locale() ~= "frFR" then return end
local L = ns.L

-- Status.lua
-- The status tab's census line: whether the census setting is on.
L["Census: on"] = nil
L["Census: off"] = nil
L["Character: %s, level %s"] = nil
L["Character: not loaded yet"] = nil
-- The panel's status: "Recorded: 212 items in bags and bank, 17 equipped, 12 quests in the log,
-- 340 completed, 48 guild members". The %s slots are the pairs below, in that order.
L["Recorded: %s, %d equipped, %s, %d completed, %s"] = nil
L["%d item in bags and bank"] = nil
L["%d items in bags and bank"] = nil
L["%d quest in the log"] = nil
L["%d quests in the log"] = nil
L["%d guild member"] = nil
L["%d guild members"] = nil
-- The status tab's Chronicle line: "Chronicle: 41 creatures, 112 items" (kinds of creature killed and
-- kinds of item looted, lifetime, this character). %s: the pairs below.
L["Chronicle: %s, %s"] = nil
L["Chronicle: off"] = nil
L["%d creature"] = nil
L["%d creatures"] = nil
L["%d item"] = nil
L["%d items"] = nil
-- Name, level, class token, realm, realm id.
L["%s, level %s %s, realm %s (%s)"] = nil
L["recorded: %d gear, %s (bank as of %s), %d active / %d completed quests"] = nil
L["%d container slot"] = nil
L["%d container slots"] = nil
-- "2 professions, 9 reputations, played 41h, guild <name>".
L["%s, %s, played %s, guild %s"] = nil
L["%d profession"] = nil
L["%d professions"] = nil
L["%d reputation"] = nil
L["%d reputations"] = nil
-- Played time in hours, and "played pending" before /played answers; "guild none".
L["%dh"] = nil
L["pending"] = nil
L["none"] = nil
-- "3 characters, 1 guild on this account".
L["%s, %s on this account"] = nil
L["%d character"] = nil
L["%d characters"] = nil
L["%d guild"] = nil
L["%d guilds"] = nil

-- Tooltip.lua: lines added to creature and item tooltips, the player's own numbers.
-- "Ryna: killed 43 (40 yours), last Elwynn Forest 2 hours ago": %d kills, %d my killing blows,
-- then the zone and how long ago ("%d hours ago", Status.lua). The second has no zone.
L["Ryna: killed %d (%d yours), last %s %s"] = nil
L["Ryna: killed %d (%d yours), last %s"] = nil
-- "Linen Cloth: 22 of 38 corpses (58%)": an item, how many corpses dropped it, corpses looted, percent.
L["%s: %d of %d corpses (%d%%)"] = nil
-- "Ryna: looted 30 times (45), mostly from Kobold Vermin (22)": "%d times" (below), pieces in all,
-- the biggest source and how many times it dropped from there.
L["Ryna: looted %s (%d), mostly from %s (%d)"] = nil
L["Ryna: looted %s (%d)"] = nil
L["%d time"] = nil
L["%d times"] = nil
-- Loot sources without a name of their own (%s: the game's id for it).
L["fishing"] = nil
L["a container"] = nil
L["creature %s"] = nil
L["object %s"] = nil
L["item %s"] = nil

-- Tabs.lua
L["Recording"] = nil
L["Census"] = nil
-- The Census tab's first line.
L["Ryna never sends /who itself. When you type /who, the results are recorded as "
  .. "sightings, like players you see on nameplates or in chat."] = nil
L["Record who I see in chat"] = nil
L["Only who spoke, never what they said or in which channel."] = nil
L["Take part in the census"] = nil
L["Records other players you see (nearby, in chat, your group and your guild). "
  .. "Off: Ryna records only your own characters."] = nil
L["Record other players in my group"] = nil
L["Who was in your groups."] = nil
L["Record players I see nearby"] = nil
L["Players whose nameplates, target or mouseover you see: their level, class and guild, and where."] = nil
-- Under "Record players I see nearby"; Shift+V is the game's default key for friendly nameplates.
L["The game shows nameplates for enemies only unless friendly nameplates are on "
  .. "(Shift+V). Ryna only sees nameplates the game draws."] = nil
-- A button that turns the game's friendly nameplates on, and its text once they are on.
L["Turn friendly nameplates on"] = nil
L["Friendly nameplates are on"] = nil
L["About your own characters, Ryna records their gear, bags and bank, quests, "
  .. "professions, reputations, gold and played time, your guild's roster, and a 30-day journal of what "
  .. "you do. Other players are the Census tab's."] = nil
L["The census keeps who was seen, never what they said. "
  .. "Its data is never sold or used for advertising."] = nil
L["Record kills and loot"] = nil
L["Counts the creatures you and your group kill, the experience they give and what you loot from them, "
  .. "as running totals on the character (not journal events). Off: nothing is counted, and the totals so "
  .. "far are kept."] = nil
L["Show my numbers on tooltips"] = nil
L["Adds your own kills and drops to creature and item tooltips: how many you killed, what they dropped "
  .. "for you and how often."] = nil

-- Session.lua, Strip.lua, Window.lua: the session strip and the Chronicle window.
-- Durations: "under a minute", "2d 3h" (days, hours), "1h 12m" (hours, minutes), "12m" (minutes).
L["under a minute"] = nil
L["%dd %dh"] = nil
L["%dh %dm"] = nil
L["%dm"] = nil
-- The Status tab's session line: "Session: 42 kills, 3,120 XP/h" (experience per hour), or not started.
L["Session: not started"] = nil
L["Session: %s, %s XP/h"] = nil
L["%d kill"] = nil
L["%d kills"] = nil
-- The session strip, one line joined by " · ": "Kills 42 · 3,120 XP/h · 1g 20s/h · 2 quests · 1h 12m".
-- "%s/h" is an amount of money per hour ("1g 20s/h"); keep it short, the strip is one line.
L["Kills %s"] = nil
L["%s XP/h"] = nil
L["%s/h"] = nil
L["%d quest"] = nil
L["%d quests"] = nil
-- The strip's tooltip: its title, then "Since 14:02 (1h 12m)" (the clock time the session started,
-- how long ago), then label / value rows ("Kills: 42 (35/h)").
L["Chronicle: this session"] = nil
L["Since %s (%s)"] = nil
L["Kills"] = nil
L["%s (%s/h)"] = nil
L["Experience"] = nil
L["Levels gained"] = nil
-- Money: the net change this session; "Gained / spent": "3g 10s / 1g 50s".
L["Money"] = nil
L["Gained / spent"] = nil
L["%s / %s"] = nil
L["Quests turned in"] = nil
L["Items looted"] = nil
L["Deaths"] = nil
-- The tooltip's list of creatures killed most this session: "Kobold Vermin   12 (480 XP)".
L["Top kills this session"] = nil
L["%s (%s XP)"] = nil
L["Click: the Chronicle window. Right-click: lock or unlock."] = nil
L["Shift-drag to move."] = nil
-- Printed in chat ("Ryna: the session strip is locked. ...") when the strip is right-clicked.
L["the session strip is locked. Right-click it to unlock."] = nil
L["the session strip is unlocked: Shift-drag to move it."] = nil
-- The Recording tab: a heading over the strip and window settings, then those settings.
L["In game"] = nil
L["Show the session strip"] = nil
L["One line on screen with this session's kills, experience and money per hour, quests and time. "
  .. "Hover it for more; click it for the Chronicle window."] = nil
L["Lock the strip"] = nil
L["Unlocked, Shift-drag moves it. Right-clicking the strip locks or unlocks it too."] = nil
-- A button: the strip back to its default place (top of the screen).
L["Reset position"] = nil
-- The Recording tab's button that opens the Chronicle window, and the line under it.
L["Open the Chronicle window"] = nil
L["Also /chronicle. Your session, your kills with their drops, your loot and your "
  .. "levels, in its own window."] = nil
-- The Chronicle window: its title (the mod's name) and its four tabs.
L["Chronicle"] = nil
L["Session"] = nil
L["Loot"] = nil
L["Levels"] = nil
-- The Session tab: the column heading over its numbers, "Since" (when the session started), and the
-- heading over this session's kills.
L["This session"] = nil
L["Since"] = nil
L["Kills this session"] = nil
-- The Kills tab's columns: creature, kills, experience, the zone of the last kill, when ("2 hours ago").
L["Creature"] = nil
L["Last zone"] = nil
L["Last"] = nil
-- A creature row's tooltip: "Killed: 42 (30 yours)", "Last: Westfall, 2 hours ago", "Corpses looted: 40",
-- then "Drops" and per item "12 of 40 (30%)" (how many corpses dropped it; %% prints one %).
L["Killed"] = nil
L["%s (%s yours)"] = nil
L["%s, %s"] = nil
L["Corpses looted"] = nil
L["Drops"] = nil
L["%d of %d (%d%%)"] = nil
-- The Loot tab's columns: item, times looted, how many in all, where from, when.
L["Item"] = nil
L["Looted"] = nil
L["Count"] = nil
L["From"] = nil
-- Where loot came from, besides a creature's name and fishing: a chest, vein, herb or pool, or a
-- container item (a bag or box opened).
L["object"] = nil
L["container"] = nil
L["From: %s"] = nil
-- The Levels tab's columns: the level, the date it was reached, the time played at it, and the total
-- played time when it was reached. "before the journal": reached more than 30 days ago (no date).
L["Level"] = nil
L["Reached"] = nil
L["Time at the level"] = nil
L["Played when reached"] = nil
L["before the journal"] = nil
-- A list with nothing in it: the search matched nothing, or nothing is recorded yet.
L["Nothing matches."] = nil
L["Nothing recorded yet."] = nil
L["Your own numbers for this character. Hover a row for more."] = nil
-- /ryna help's line for the window's command. Keep /ryna chronicle, /chronicle and the tab words
-- (kills, loot, levels, session) in English: they are what the player types.
L["/ryna chronicle (or /chronicle): the Chronicle window; "
  .. "/chronicle kills, loot, levels or session opens that tab"] = nil
