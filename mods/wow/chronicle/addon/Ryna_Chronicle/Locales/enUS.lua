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
-- Under either, for a creature or item counted in more than one zone (or not where you are now):
-- "12 here (Westfall) · 43 in all": the count in the zone you're in, that zone, the count in all zones.
L["%d here (%s) · %d in all"] = true
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
L["Record the merchants I visit"] = true
L["When you open a merchant, records what that NPC sells and for how much, once per NPC each time you "
  .. "log in, for vendor prices on ryna.io. Merchants are NPCs: nothing here is about a player."] = true

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
L["Click: Ryna's window. Right-click: lock or unlock."] = true
L["Shift-drag to move."] = true
-- Printed in chat ("Ryna: the session strip is locked. ...") when the strip is right-clicked.
L["the session strip is locked. Right-click it to unlock."] = true
L["the session strip is unlocked: Shift-drag to move it."] = true
-- The Recording tab: a heading over the strip and window settings, then those settings.
L["In game"] = true
L["Show the session strip"] = true
L["One line on screen with this session's kills, experience and money per hour, quests and time. "
  .. "Hover it for more; click it for Ryna's window."] = true
L["Lock the strip"] = true
L["Unlocked, Shift-drag moves it. Right-clicking the strip locks or unlocks it too."] = true
-- A button: the strip back to its default place (top of the screen).
L["Reset position"] = true
-- The Recording tab's button that opens Ryna's window (the core's one window, where Chronicle's
-- sections are), and the line under it.
L["Open Ryna's window"] = true
L["Also /chronicle. Your session, your characters, everything in their bags and banks, "
  .. "your kills with their drops, your loot and your levels."] = true
-- The Recording tab's setting for the tooltip line naming your other characters that hold an item.
-- The names in the example are made up.
L["Show which of my characters hold an item"] = true
L["One more line on item tooltips naming your other characters that hold it, and how many, in their "
  .. "bags or the bank, from your saved file: \"Alyra: 120 in the bank · Brennic: 34\"."] = true
-- Chronicle's sections in Ryna's window (its rail, under Me).
L["Session"] = true
L["Characters"] = true
L["Items"] = true
L["Loot"] = true
L["History"] = true
-- The Session section: the column heading over its numbers, "Since" (when the session started), and
-- the heading over this session's kills.
L["This session"] = true
L["Since"] = true
L["Kills this session"] = true
-- The Kills section's tiles (how many kinds of creature) and columns: creature, kills, experience,
-- the zone of the last kill, when ("2 hours ago").
L["Creatures"] = true
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
-- The Loot section's columns (and tiles): item, times looted, how many in all, where from, when.
L["Item"] = true
L["Looted"] = true
L["Count"] = true
L["From"] = true
-- Where loot came from, besides a creature's name and fishing: a chest, vein, herb or pool, or a
-- container item (a bag or box opened).
L["object"] = true
L["container"] = true
L["From: %s"] = true
-- The History section's columns (and tiles): the level, the date it was reached, the time played at it, and the total
-- played time when it was reached. "before the journal": reached more than 30 days ago (no date).
L["Level"] = true
L["Reached"] = true
L["Time at the level"] = true
L["Played when reached"] = true
L["before the journal"] = true
-- History's tiles: the time played in all, "pending" (core) until /played answers.
L["Played"] = true
-- The footers: where the numbers came from and how old they are. %s: a character's name, then
-- "2 hours ago".
L["Counted in game since you logged in; a /reload keeps it."] = true
L["Counted in game for %s, the character you're on."] = true
L["From your saved file: %s as of %s, its last logout."] = true
L["an unknown time"] = true
L["From the journal's level-ups (it keeps 30 days) and /played."] = true
-- /chronicle with a core too old for the window (printed only when typed).
L["the Ryna addon here is too old for its window. Update it from the Ryna companion app."] = true
-- /ryna help's line for the window's command. Keep /ryna chronicle, /chronicle and the section words
-- (session, characters, items, kills, loot, zones, history) in English: they are what the player types.
L["/ryna chronicle (or /chronicle): Ryna's window; /chronicle session, characters, items, "
  .. "kills, loot, zones or history opens that section"] = true
-- Chronicle by zone. The zone picker on Kills and Loot: every zone, the zone you're in ("This zone
-- (Westfall)"; plain "This zone" when the game hasn't said), each zone, and "Earlier travels": what
-- was counted before Ryna kept zones (a lifetime of places you went before).
L["All zones"] = true
L["This zone (%s)"] = true
L["This zone"] = true
L["Earlier travels"] = true
-- Earlier travels explained (a tooltip). The second once something was counted in more than the eight
-- zones Ryna keeps per creature or item, whose smallest zones then join the earlier ones.
L["counted before Ryna kept zones"] = true
L["counted before Ryna kept zones, or in other zones"] = true
-- Kills and Loot with a zone picked: the footer's first sentence (%s: a date and time, or "not yet"),
-- or Earlier travels' own footer.
L["Counted by zone since %s."] = true
L["not yet"] = true
L["Earlier travels: counted before Ryna kept zones, or in zones past the eight it keeps."] = true
-- Kills with a zone picked: the column with the creature's kills in all zones; a tile with the kills
-- placed in this zone from the player's own combat logs (by the Ryna companion app).
L["In all"] = true
L["From your combat logs"] = true
-- A creature's or item's tooltip: "By zone", then per zone "12 (4 placed from your combat logs)", or
-- for loot "22 (31 pieces)": times looted, then how many in all.
L["By zone"] = true
L["%s (%s placed from your combat logs)"] = true
L["%s (%s pieces)"] = true
-- The Zones section: its columns (the zone, kills, kinds of creature killed, items looted, pieces
-- looted, quests turned in, deaths, time spent there), its tiles, a row's tooltip ("First here: <date>",
-- "Last here: 2 hours ago") and its click, and its footer (%s: dates, or "not yet").
L["Zones"] = true
L["Pieces"] = true
L["Quests"] = true
L["Time"] = true
L["Time spent"] = true
L["Quests done"] = true
L["First here"] = true
L["Last here"] = true
L["Click to see its kills."] = true
L["Time, quests and deaths since %s; kills and loot by zone since %s."] = true

-- Characters.lua
-- The Characters section: its columns over every character, its tiles, "now" (the character you're
-- on, as "last seen"), and "Mining 75" (a profession and its rank).
L["Character"] = true
L["Gold"] = true
L["Professions"] = true
L["Last seen"] = true
L["now"] = true
L["%s %s"] = true
-- One character's detail: the headings of its groups and the rows under "Character".
L["Class"] = true
L["Guild"] = true
L["Zone"] = true
L["Talents"] = true
L["Gear"] = true
L["Reputations"] = true
L["None recorded yet."] = true
-- A tile: the time played at the current level ("3h 12m at this level"); a tooltip's "Level 17 Paladin".
L["%s at this level"] = true
L["Level %s %s"] = true
L["Click for this character's gear, professions and reputations."] = true
L["From your saved file: each character as of its last logout; the one you're on is live."] = true
-- Equipment slots, where the client has no name of its own for one.
L["Ammo"] = true
L["Head"] = true
L["Neck"] = true
L["Shoulder"] = true
L["Shirt"] = true
L["Chest"] = true
L["Waist"] = true
L["Legs"] = true
L["Feet"] = true
L["Wrist"] = true
L["Hands"] = true
L["Finger"] = true
L["Trinket"] = true
L["Back"] = true
L["Main Hand"] = true
L["Off Hand"] = true
L["Ranged"] = true
L["Tabard"] = true

-- Items.lua
-- The tooltip line naming your other characters that hold an item: "Alyra: 120 in the bank · Brennic:
-- 34 · 2 more". Each part is "<name>: <how many>"; "154 (120 in the bank)" when some are in bags too.
L["%s: %s"] = true
L["%s (%s in the bank)"] = true
L["%s in the bank"] = true
L["%d more"] = true
-- The Items section: columns, tiles, the tooltip's "34 in bags · 120 in the bank (as of 2 days ago)".
L["Total"] = true
L["In bags"] = true
L["In the bank"] = true
L["%s in bags"] = true
L["%s in the bank (as of %s)"] = true
L["Kinds of item"] = true
L["Bank read"] = true
L["Nothing in the bags or bank yet. Each character's are recorded at login."] = true
-- Its footers. %s: how long ago ("2 days ago"), a character's name, or "never: open the bank once".
L["From your saved file: bags as of each character's last login, the bank as of its last visit."] = true
L["never: open the bank once"] = true
L["Bags as they are now; the bank as of your last visit (%s)."] = true
L["From your saved file: %s's bags as of %s, the bank as of %s."] = true
