-- Italian (itIT) strings for Ryna. Loaded on every client, applied only when the game runs
-- in itIT: GetLocale() is "itIT".
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
--     or SET textLocale "itIT" in WTF/Config.wtf, with that language installed), log in, then
--     /ryna, /ryna status and /ryna help.
-- This file is generated from enUS.lua by addon/tools/strings.mjs --templates, which keeps the
-- translations already here; run it after enUS.lua changes.

local _, ns = ...
if ns.Locale() ~= "itIT" then return end
local L = ns.L

-- Core.lua
L["debug messages off."] = nil
L["an error in %s was caught (reported once): %s"] = nil

-- Init.lua
-- The login line, after "Ryna 0.2.0: ".
L["recording. /ryna for settings."] = nil
L["the companion app hasn't synced recently."] = nil

-- Zone.lua
L["combat logging on (instance)"] = nil
L["combat logging on"] = nil
L["combat logging off"] = nil

-- Status.lua
L["In dungeons and raids only"] = nil
L["Always"] = nil
L["Never"] = nil
L["just now"] = nil
-- "Synced 12 minutes ago", "Last sweep: 412 players, 3 hours ago".
L["%d minute ago"] = nil
L["%d minutes ago"] = nil
L["%d hour ago"] = nil
L["%d hours ago"] = nil
L["%d day ago"] = nil
L["%d days ago"] = nil
-- %s: "12 minutes ago" (above), then the name the companion app is signed in as.
L["Synced %s as %s."] = nil
L["Synced %s."] = nil
L["Signed out now: sign in from the Ryna companion app."] = nil
L["Not synced yet. Sign in from the Ryna companion app."] = nil
L["Update available: Ryna addon %s (you have %s)."] = nil
L["Update available: companion app %s (you have %s)."] = nil
L["unavailable on this client"] = nil
L["on (turned on by Ryna)"] = nil
L["not turned on by Ryna"] = nil

-- Panel.lua
-- %s: a combat-log mode ("Always").
L["%s (click to change)"] = nil
L["Ryna records your characters, bags, quests and guild for the Ryna companion app, which "
  .. "uploads them when you sign in there. Signing in happens in the app, not in the game."] = nil
-- The panel's tabs.
L["Status"] = nil
L["Recording"] = nil
L["Census"] = nil
L["Messages"] = nil
L["Advanced"] = nil
L["Turn on the game's combat log:"] = nil
L["The combat log is how Ryna sees fights. \"In dungeons and raids only\" keeps the "
  .. "file small. Logging you start yourself with /combatlog is left alone."] = nil
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
-- Replaces the city census switch's description where the game allows /who only from a click.
L["The game only allows /who from a click on this client."] = nil
L["Advanced combat logging"] = nil
L["Sets the game's advanced combat logging on, so fights are logged with positions and health. "
  .. "Needed for Ryna's timelines and deaths."] = nil
L["Show the minimap button"] = nil
L["Drag it around the minimap to move it."] = nil
L["Debug messages"] = nil
L["Prints what Ryna is doing to the chat window: scans, journal events, sightings. "
  .. "For checking that it works."] = nil
L["About your own characters, Ryna records their gear, bags and bank, quests, "
  .. "professions, reputations, gold and played time, your guild's roster, and a 30-day journal of what "
  .. "you do. Other players are the Census tab's."] = nil
L["The census keeps who was seen, never what they said. "
  .. "Its data is never sold or used for advertising."] = nil
L["From the companion app, newest first, as of your last login or /reload."] = nil
L["Defaults"] = nil
L["Every setting back to its default. The minimap button keeps its place."] = nil
L["Ryna addon version: %s"] = nil
-- %s: a date and time ("2026-11-06 14:02"), then how long ago ("12 minutes ago").
L["Companion app status file: as of %s (%s)"] = nil
L["Companion app status file: nothing in it yet"] = nil
-- The status tab's census line. %s in "Census: %s" is the sweep's progress or why it can't run.
L["Census: off"] = nil
L["Census: %s"] = nil
L["Last census sweep: none yet"] = nil
-- %s: "%d player" / "%d players" (Census.lua), then how long ago ("20 minutes ago").
L["Last census sweep: %s, %s"] = nil
L["settings back to their defaults; debug messages off."] = nil
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
-- %s: "%d events this session" (below).
L["Journal: %s, %d kept in all (last 30 days)"] = nil
L["%d event this session"] = nil
L["%d events this session"] = nil
L["Session: %s"] = nil
L["not started"] = nil
L["Combat log: %s right now. Mode: %s"] = nil
L["Companion app: %s"] = nil
L["Companion app version: %s"] = nil
L["(The app's status is as of your last login or /reload.)"] = nil
L["%d error caught; /ryna status shows the last."] = nil
L["%d errors caught; /ryna status shows the last."] = nil
L["No messages"] = nil
-- A companion-app message's time before its text: "12 minutes ago: ...".
L["%s: "] = nil
L["the settings open after combat."] = nil
L["no settings panel on this client; /ryna status prints the status."] = nil

-- Minimap.lua
L["Update available"] = nil
L["Click: open Ryna. Drag: move this button."] = nil

-- Census.lua
L["%d player"] = nil
L["%d players"] = nil
-- %s here and in the census lines below: "%d player" / "%d players" (above).
L["Sweeping: %d of %d queries, %s so far"] = nil
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
L["census sweep: the server takes one query every few seconds; "
  .. "try again in %ds."] = nil
L["census sweep: this client has no /who for addons."] = nil
L["census sweep: not now, %s."] = nil
L["census sweep started: asking the server who is online, one query every 4s."] = nil
L["stopped"] = nil
L["combat started"] = nil
L["entered an instance"] = nil

-- Slash.lua
-- Setting states in /ryna status and /ryna debug ("debug messages are on").
L["on"] = nil
L["off"] = nil
-- No date: "bank as of never".
L["never"] = nil
-- /ryna help. Command words and "||" (one "|" in chat) stay as they are.
L["v%s. Commands:"] = nil
L["/ryna: open the Ryna settings panel"] = nil
L["/ryna status: what is recorded, the journal's size, combat logging"] = nil
L["/ryna combatlog instances||always||never: when the addon turns combat logging on"] = nil
L["/ryna debug on||off||toggle: print what Ryna is doing to chat "
  .. "(/ryna debug alone: which it is)"] = nil
L["/ryna census: run a realm census now (/who, a few queries at a time, or one per "
  .. "/ryna census where the game allows /who only from a click); /ryna census stop ends it"] = nil
L["stopped by you"] = nil
L["no census sweep is running."] = nil
L["usage: /ryna census (runs a sweep now, or its next query) or /ryna census stop"] = nil
L["debug messages are %s. /ryna debug on||off||toggle changes it."] = nil
L["usage: /ryna debug on||off||toggle (now %s)"] = nil
L["debug messages are already %s."] = nil
L["not initialised yet"] = nil
-- /ryna status.
L["v%s, schema %d, flavor %s, client %s (interface %s)"] = nil
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
-- %s: "%d events" (below), then a date, then the session id.
L["journal: %s since %s; session %s, %d this session"] = nil
L["%d event"] = nil
L["%d events"] = nil
-- %s: the mode's command word (instances, always, never), then on, off or unavailable.
L["combat log: setting %s, currently %s"] = nil
L["unavailable"] = nil
L["debug messages: %s"] = nil
L["companion app: %s"] = nil
-- "2 caught errors; last: <the error>".
L["%s; last: %s"] = nil
L["%d caught error"] = nil
L["%d caught errors"] = nil
L["combat logging: %s"] = nil
L["usage: /ryna combatlog instances||always||never (now %s)"] = nil
