-- Chinese (Simplified) (zhCN) strings for Ryna Chronicle. Loaded on every client, applied only when the game runs
-- in zhCN: GetLocale() is "zhCN".
--
-- How to translate:
--   - Each line is L["English"] = nil. Replace nil with your text in double quotes:
--       L["Run census now"] = "..."
--     A line left as nil shows the English, so a partial translation is fine.
--   - Keep every %s and %d, in the same order (WoW's string.format can't reorder them).
--   - Keep "||" as it is: it prints one "|" in chat. Inside a command (/ryna debug on||off||toggle,
--     /ryna census stop, /who, /reload) keep the words in English: they are what the player types.
--   - Plurals: a pair like "%d player" / "%d players" is used for 1 / everything else.
--     For a language with more forms, use the wording that reads best for most numbers.
--   - Keys are the English text and must stay exactly as they are; only change the values.
--   - Test: set the client's language (Battle.net -> Settings -> Game Settings -> Text Language,
--     or SET textLocale "zhCN" in WTF/Config.wtf, with that language installed), log in, then
--     /ryna, /ryna status and /ryna help.
-- This file is generated from enUS.lua by cores/wow/addon/tools/strings.mjs --templates, which keeps the
-- translations already here; run it after enUS.lua changes.

local _, ns = ...
if ns.off then return end
if ns.Locale() ~= "zhCN" then return end
local L = ns.L

-- Census.lua
L["%d player"] = nil
L["%d players"] = nil
-- %s here and in the census lines below: "%d player" / "%d players" (above).
L["Sweeping: %d of %d queries, %s so far"] = nil
-- %s: "Sweeping: %d of %d queries, %s so far" (above); %d: seconds until the next query may go.
L["%s; next query in %ds"] = nil
L["%s; ready for the next query"] = nil
L["No sweep yet."] = nil
L["Last sweep: %s, %s"] = nil
L["Last sweep: %s, %s (stopped early)"] = nil
L["%d of %d queries, %s so far"] = nil
L["census sweep: %s"] = nil
-- Why a census sweep stopped or didn't start, shown in "census sweep stopped (%s): ..." and
-- "census sweep: not now, %s.".
L["the census setting is off"] = nil
L["in combat"] = nil
L["in an instance"] = nil
L["the Who window is open"] = nil
L["this client has no /who"] = nil
L["the game allows /who only from a click"] = nil
L["the server didn't answer /who"] = nil
L["census sweep done: %s, %d new to this sweep's window, in %ds"] = nil
L["census sweep stopped (%s): %s, %d new to this sweep's window, in %ds"] = nil
L["census sweep: not ready yet."] = nil
L["census sweep: one is already running. %s"] = nil
-- The census modes: some clients answer /who only when it is sent from a click or a typed command.
-- Also the Census tab's status line on such a client.
L["the game doesn't let addons ask /who here; the census uses what you see instead"] = nil
L["census sweep: the game doesn't let addons ask /who here. "
  .. "The Census tab's Try again asks once more."] = nil
L["the game allows /who only from a click on this client, so the automatic census is off; "
  .. "the Census tab's button asks one query per click."] = nil
-- %d of %d: the next query's place in the plan and the plan's length.
L["the game allows /who only from a click on this client, so the census goes one "
  .. "query per click: press Next census query on the Census tab, or type /ryna census (%d of %d next)."] = nil
-- %s: "%d of %d queries, %s so far" (above).
L["census sweep: %s; the next query on your next click or /ryna census."] = nil
L["census sweep started: one query per click (Next census query, or /ryna census again)."] = nil
L["census sweep: waiting for the server's answer to the last query."] = nil
-- The same two, briefer, when the key binding is pressed (printed at most once per 4 seconds).
-- %s: "%d of %d queries, %s so far" (above).
L["census sweep: running, %s"] = nil
L["census sweep: waiting for the server."] = nil
-- A press of the button or the key before the server takes the next query.
L["census sweep: next query in %ds."] = nil
-- The server said "You must wait a moment longer before using /who again.": the first %d is the
-- seconds Ryna now leaves between queries, the second how long until the next one.
L["census sweep: the server wants about %ds between queries; next query in %ds."] = nil
L["census sweep: this client has no /who for addons."] = nil
L["census sweep: not now, %s."] = nil
-- %d: seconds between queries (5 to start with; more once the server asks for it).
L["census sweep started: asking the server who is online, one query every %ds."] = nil
L["stopped"] = nil
L["combat started"] = nil
L["entered an instance"] = nil

-- Status.lua
-- The status tab's census line. %s in "Census: %s" is the sweep's progress or why it can't run.
L["Census: off"] = nil
L["Census: %s"] = nil
L["Last census sweep: none yet"] = nil
-- %s: "%d player" / "%d players" (Census.lua), then how long ago ("20 minutes ago").
L["Last census sweep: %s, %s"] = nil
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
L["Run a realm census while I'm in a city"] = nil
L["Every 30 minutes while you're parked in a capital, Ryna asks the server who is online (/who), "
  .. "a few queries at a time, to count the realm's players by level, class and zone. "
  .. "Nothing about you is sent."] = nil
L["Run census now"] = nil
-- The same button where the game allows /who only from a click (%d of %d: the next query's place
-- in the plan and the plan's length), and where it doesn't allow it at all; then a link that
-- forgets that and asks again.
L["Next census query (%d of %d)"] = nil
L["Census: not allowed by the game"] = nil
L["Try again"] = nil
-- Under that button. "Next census query" is the key binding's name (Slash.lua), and
-- "Key Bindings > AddOns > Ryna" is where the game's Key Bindings settings list it: use the game's
-- own words for those in your language.
L["Bind a key to Next census query in Key Bindings > AddOns > Ryna to run a sweep "
  .. "with taps instead of clicks. /ryna census does the same from chat."] = nil
-- Replaces the city census switch's description where the game allows /who only from a click.
L["The game only allows /who from a click on this client."] = nil
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

-- Slash.lua: the key binding, in Key Bindings > AddOns: its header (the addon's name) and its name.
L["/ryna census: run a realm census now (/who, a few queries at a time, or one per "
  .. "/ryna census where the game allows /who only from a click); /ryna census stop ends it"] = nil
L["stopped by you"] = nil
L["no census sweep is running."] = nil
L["usage: /ryna census (runs a sweep now, or its next query) or /ryna census stop"] = nil
