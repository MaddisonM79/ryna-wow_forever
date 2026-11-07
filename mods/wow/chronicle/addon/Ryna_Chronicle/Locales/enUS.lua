-- English (enUS): every string Ryna Chronicle shows, keyed by the English text itself. `true` means "the
-- text is the key"; ns.L returns the key for anything no locale file translates, so English is
-- always the fallback. The other Locales/*.lua files are templates for translators, generated from
-- this one: after adding, changing or removing a string here, run (from the repository root)
--
--   node cores/wow/addon/tools/strings.mjs --templates
--
-- which rewrites them with the same keys, keeping every translation already in them. Without
-- --templates it only checks that this list matches what the code uses (L["..."] and N("...")).
-- Debug lines (ns.Debug) are English on purpose and are not listed. A key the core addon's
-- Locales/enUS.lua already has ("on", "never", ...) isn't repeated here: Chronicle's L falls back to it.

local _, ns = ...
if ns.off then return end
local L = ns.L

-- Status.lua
-- The status tab's census line: whether the census setting is on.
L["Census: on"] = true
L["Census: off"] = true
L["Character: %s, level %s"] = true
L["Character: not loaded yet"] = true
-- The panel's status: "Recorded: 212 items in bags and bank, 17 equipped, 12 quests in the log,
-- 340 completed, 48 guild members". The %s slots are the pairs below, in that order.
L["Recorded: %s, %d equipped, %s, %d completed, %s"] = true
L["%d item in bags and bank"] = true
L["%d items in bags and bank"] = true
L["%d quest in the log"] = true
L["%d quests in the log"] = true
L["%d guild member"] = true
L["%d guild members"] = true
-- The status tab's Chronicle line: "Chronicle: 41 creatures, 112 items" (kinds of creature killed and
-- kinds of item looted, lifetime, this character). %s: the pairs below.
L["Chronicle: %s, %s"] = true
L["Chronicle: off"] = true
L["%d creature"] = true
L["%d creatures"] = true
L["%d item"] = true
L["%d items"] = true
-- Name, level, class token, realm, realm id.
L["%s, level %s %s, realm %s (%s)"] = true
L["recorded: %d gear, %s (bank as of %s), %d active / %d completed quests"] = true
L["%d container slot"] = true
L["%d container slots"] = true
-- "2 professions, 9 reputations, played 41h, guild <name>".
L["%s, %s, played %s, guild %s"] = true
L["%d profession"] = true
L["%d professions"] = true
L["%d reputation"] = true
L["%d reputations"] = true
-- Played time in hours, and "played pending" before /played answers; "guild none".
L["%dh"] = true
L["pending"] = true
L["none"] = true
-- "3 characters, 1 guild on this account".
L["%s, %s on this account"] = true
L["%d character"] = true
L["%d characters"] = true
L["%d guild"] = true
L["%d guilds"] = true

-- Tooltip.lua: lines added to creature and item tooltips, the player's own numbers.
-- "Ryna: killed 43 (40 yours), last Elwynn Forest 2 hours ago": %d kills, %d my killing blows,
-- then the zone and how long ago ("%d hours ago", Status.lua). The second has no zone.
L["Ryna: killed %d (%d yours), last %s %s"] = true
L["Ryna: killed %d (%d yours), last %s"] = true
-- "Linen Cloth: 22 of 38 corpses (58%)": an item, how many corpses dropped it, corpses looted, percent.
L["%s: %d of %d corpses (%d%%)"] = true
-- "Ryna: looted 30 times (45), mostly from Kobold Vermin (22)": "%d times" (below), pieces in all,
-- the biggest source and how many times it dropped from there.
L["Ryna: looted %s (%d), mostly from %s (%d)"] = true
L["Ryna: looted %s (%d)"] = true
L["%d time"] = true
L["%d times"] = true
-- Loot sources without a name of their own (%s: the game's id for it).
L["fishing"] = true
L["a container"] = true
L["creature %s"] = true
L["object %s"] = true
L["item %s"] = true

-- Tabs.lua
L["Recording"] = true
L["Census"] = true
-- The Census tab's first line.
L["Ryna never sends /who itself. When you type /who, the results are recorded as "
  .. "sightings, like players you see on nameplates or in chat."] = true
L["Record who I see in chat"] = true
L["Only who spoke, never what they said or in which channel."] = true
L["Take part in the census"] = true
L["Records other players you see (nearby, in chat, your group and your guild). "
  .. "Off: Ryna records only your own characters."] = true
L["Record other players in my group"] = true
L["Who was in your groups."] = true
L["Record players I see nearby"] = true
L["Players whose nameplates, target or mouseover you see: their level, class and guild, and where."] = true
-- Under "Record players I see nearby"; Shift+V is the game's default key for friendly nameplates.
L["The game shows nameplates for enemies only unless friendly nameplates are on "
  .. "(Shift+V). Ryna only sees nameplates the game draws."] = true
-- A button that turns the game's friendly nameplates on, and its text once they are on.
L["Turn friendly nameplates on"] = true
L["Friendly nameplates are on"] = true
L["About your own characters, Ryna records their gear, bags and bank, quests, "
  .. "professions, reputations, gold and played time, your guild's roster, and a 30-day journal of what "
  .. "you do. Other players are the Census tab's."] = true
L["The census keeps who was seen, never what they said. "
  .. "Its data is never sold or used for advertising."] = true
L["Record kills and loot"] = true
L["Counts the creatures you and your group kill, the experience they give and what you loot from them, "
  .. "as running totals on the character (not journal events). Off: nothing is counted, and the totals so "
  .. "far are kept."] = true
L["Show my numbers on tooltips"] = true
L["Adds your own kills and drops to creature and item tooltips: how many you killed, what they dropped "
  .. "for you and how often."] = true

-- Session.lua, Strip.lua, Window.lua: the session strip and the Chronicle window.
-- Durations: "under a minute", "2d 3h" (days, hours), "1h 12m" (hours, minutes), "12m" (minutes).
L["under a minute"] = true
L["%dd %dh"] = true
L["%dh %dm"] = true
L["%dm"] = true
-- The Status tab's session line: "Session: 42 kills, 3,120 XP/h" (experience per hour), or not started.
L["Session: not started"] = true
L["Session: %s, %s XP/h"] = true
L["%d kill"] = true
L["%d kills"] = true
-- The session strip, one line joined by " · ": "Kills 42 · 3,120 XP/h · 1g 20s/h · 2 quests · 1h 12m".
-- "%s/h" is an amount of money per hour ("1g 20s/h"); keep it short, the strip is one line.
L["Kills %s"] = true
L["%s XP/h"] = true
L["%s/h"] = true
L["%d quest"] = true
L["%d quests"] = true
-- The strip's tooltip: its title, then "Since 14:02 (1h 12m)" (the clock time the session started,
-- how long ago), then label / value rows ("Kills: 42 (35/h)").
L["Chronicle: this session"] = true
L["Since %s (%s)"] = true
L["Kills"] = true
L["%s (%s/h)"] = true
L["Experience"] = true
L["Levels gained"] = true
-- Money: the net change this session; "Gained / spent": "3g 10s / 1g 50s".
L["Money"] = true
L["Gained / spent"] = true
L["%s / %s"] = true
L["Quests turned in"] = true
L["Items looted"] = true
L["Deaths"] = true
-- The tooltip's list of creatures killed most this session: "Kobold Vermin   12 (480 XP)".
L["Top kills this session"] = true
L["%s (%s XP)"] = true
L["Click: the Chronicle window. Right-click: lock or unlock."] = true
L["Shift-drag to move."] = true
-- Printed in chat ("Ryna: the session strip is locked. ...") when the strip is right-clicked.
L["the session strip is locked. Right-click it to unlock."] = true
L["the session strip is unlocked: Shift-drag to move it."] = true
-- The Recording tab: a heading over the strip and window settings, then those settings.
L["In game"] = true
L["Show the session strip"] = true
L["One line on screen with this session's kills, experience and money per hour, quests and time. "
  .. "Hover it for more; click it for the Chronicle window."] = true
L["Lock the strip"] = true
L["Unlocked, Shift-drag moves it. Right-clicking the strip locks or unlocks it too."] = true
-- A button: the strip back to its default place (top of the screen).
L["Reset position"] = true
-- The Recording tab's button that opens the Chronicle window, and the line under it.
L["Open the Chronicle window"] = true
L["Also /chronicle. Your session, your kills with their drops, your loot and your "
  .. "levels, in its own window."] = true
-- The Chronicle window: its title (the mod's name) and its four tabs.
L["Chronicle"] = true
L["Session"] = true
L["Loot"] = true
L["Levels"] = true
-- The Session tab: the column heading over its numbers, "Since" (when the session started), and the
-- heading over this session's kills.
L["This session"] = true
L["Since"] = true
L["Kills this session"] = true
-- The Kills tab's columns: creature, kills, experience, the zone of the last kill, when ("2 hours ago").
L["Creature"] = true
L["Last zone"] = true
L["Last"] = true
-- A creature row's tooltip: "Killed: 42 (30 yours)", "Last: Westfall, 2 hours ago", "Corpses looted: 40",
-- then "Drops" and per item "12 of 40 (30%)" (how many corpses dropped it; %% prints one %).
L["Killed"] = true
L["%s (%s yours)"] = true
L["%s, %s"] = true
L["Corpses looted"] = true
L["Drops"] = true
L["%d of %d (%d%%)"] = true
-- The Loot tab's columns: item, times looted, how many in all, where from, when.
L["Item"] = true
L["Looted"] = true
L["Count"] = true
L["From"] = true
-- Where loot came from, besides a creature's name and fishing: a chest, vein, herb or pool, or a
-- container item (a bag or box opened).
L["object"] = true
L["container"] = true
L["From: %s"] = true
-- The Levels tab's columns: the level, the date it was reached, the time played at it, and the total
-- played time when it was reached. "before the journal": reached more than 30 days ago (no date).
L["Level"] = true
L["Reached"] = true
L["Time at the level"] = true
L["Played when reached"] = true
L["before the journal"] = true
-- A list with nothing in it: the search matched nothing, or nothing is recorded yet.
L["Nothing matches."] = true
L["Nothing recorded yet."] = true
L["Your own numbers for this character. Hover a row for more."] = true
-- /ryna help's line for the window's command. Keep /ryna chronicle, /chronicle and the tab words
-- (kills, loot, levels, session) in English: they are what the player types.
L["/ryna chronicle (or /chronicle): the Chronicle window; "
  .. "/chronicle kills, loot, levels or session opens that tab"] = true
