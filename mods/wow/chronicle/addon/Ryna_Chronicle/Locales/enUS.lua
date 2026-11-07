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

-- Census.lua
L["%d player"] = true
L["%d players"] = true
-- %s here and in the census lines below: "%d player" / "%d players" (above).
L["Sweeping: %d of %d queries, %s so far"] = true
-- %s: "Sweeping: %d of %d queries, %s so far" (above); %d: seconds until the next query may go.
L["%s; next query in %ds"] = true
L["%s; ready for the next query"] = true
L["No sweep yet."] = true
L["Last sweep: %s, %s"] = true
L["Last sweep: %s, %s (stopped early)"] = true
L["%d of %d queries, %s so far"] = true
L["census sweep: %s"] = true
-- Why a census sweep stopped or didn't start, shown in "census sweep stopped (%s): ..." and
-- "census sweep: not now, %s.".
L["the census setting is off"] = true
L["in combat"] = true
L["in an instance"] = true
L["the Who window is open"] = true
L["this client has no /who"] = true
L["the game allows /who only from a click"] = true
L["the server didn't answer /who"] = true
L["census sweep done: %s, %d new to this sweep's window, in %ds"] = true
L["census sweep stopped (%s): %s, %d new to this sweep's window, in %ds"] = true
L["census sweep: not ready yet."] = true
L["census sweep: one is already running. %s"] = true
-- The census modes: some clients answer /who only when it is sent from a click or a typed command.
-- Also the Census tab's status line on such a client.
L["the game doesn't let addons ask /who here; the census uses what you see instead"] = true
L["census sweep: the game doesn't let addons ask /who here. "
  .. "The Census tab's Try again asks once more."] = true
L["the game allows /who only from a click on this client, so the automatic census is off; "
  .. "the Census tab's button asks one query per click."] = true
-- %d of %d: the next query's place in the plan and the plan's length.
L["the game allows /who only from a click on this client, so the census goes one "
  .. "query per click: press Next census query on the Census tab, or type /ryna census (%d of %d next)."] = true
-- %s: "%d of %d queries, %s so far" (above).
L["census sweep: %s; the next query on your next click or /ryna census."] = true
L["census sweep started: one query per click (Next census query, or /ryna census again)."] = true
L["census sweep: waiting for the server's answer to the last query."] = true
-- The same two, briefer, when the key binding is pressed (printed at most once per 4 seconds).
-- %s: "%d of %d queries, %s so far" (above).
L["census sweep: running, %s"] = true
L["census sweep: waiting for the server."] = true
-- A press of the button or the key before the server takes the next query.
L["census sweep: next query in %ds."] = true
-- The server said "You must wait a moment longer before using /who again.": the first %d is the
-- seconds Ryna now leaves between queries, the second how long until the next one.
L["census sweep: the server wants about %ds between queries; next query in %ds."] = true
L["census sweep: this client has no /who for addons."] = true
L["census sweep: not now, %s."] = true
-- %d: seconds between queries (5 to start with; more once the server asks for it).
L["census sweep started: asking the server who is online, one query every %ds."] = true
L["stopped"] = true
L["combat started"] = true
L["entered an instance"] = true

-- Status.lua
-- The status tab's census line. %s in "Census: %s" is the sweep's progress or why it can't run.
L["Census: off"] = true
L["Census: %s"] = true
L["Last census sweep: none yet"] = true
-- %s: "%d player" / "%d players" (Census.lua), then how long ago ("20 minutes ago").
L["Last census sweep: %s, %s"] = true
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
L["Run a realm census while I'm in a city"] = true
L["Every 30 minutes while you're parked in a capital, Ryna asks the server who is online (/who), "
  .. "a few queries at a time, to count the realm's players by level, class and zone. "
  .. "Nothing about you is sent."] = true
L["Run census now"] = true
-- The same button where the game allows /who only from a click (%d of %d: the next query's place
-- in the plan and the plan's length), and where it doesn't allow it at all; then a link that
-- forgets that and asks again.
L["Next census query (%d of %d)"] = true
L["Census: not allowed by the game"] = true
L["Try again"] = true
-- Under that button. "Next census query" is the key binding's name (Slash.lua), and
-- "Key Bindings > AddOns > Ryna" is where the game's Key Bindings settings list it: use the game's
-- own words for those in your language.
L["Bind a key to Next census query in Key Bindings > AddOns > Ryna to run a sweep "
  .. "with taps instead of clicks. /ryna census does the same from chat."] = true
-- Replaces the city census switch's description where the game allows /who only from a click.
L["The game only allows /who from a click on this client."] = true
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

-- Slash.lua: the key binding, in Key Bindings > AddOns: its header (the addon's name) and its name.
L["/ryna census: run a realm census now (/who, a few queries at a time, or one per "
  .. "/ryna census where the game allows /who only from a click); /ryna census stop ends it"] = true
L["stopped by you"] = true
L["no census sweep is running."] = true
L["usage: /ryna census (runs a sweep now, or its next query) or /ryna census stop"] = true
