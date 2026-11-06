-- English (enUS): every string Ryna shows, keyed by the English text itself. `true` means "the
-- text is the key"; ns.L returns the key for anything no locale file translates, so English is
-- always the fallback. The other Locales/*.lua files are templates for translators, generated from
-- this one: after adding, changing or removing a string here, run (from the repository root)
--
--   node addon/tools/strings.mjs --templates
--
-- which rewrites them with the same keys, keeping every translation already in them. Without
-- --templates it only checks that this list matches what the code uses (L["..."] and N("...")).
-- Debug lines (ns.Debug) are English on purpose and are not listed.

local _, ns = ...
local L = ns.L

-- Core.lua
L["debug messages off."] = true
L["an error in %s was caught (reported once): %s"] = true

-- Init.lua
-- The login line, after "Ryna 0.2.0: ".
L["recording. /ryna for settings."] = true
L["the companion app hasn't synced recently."] = true

-- Zone.lua
L["combat logging on (instance)"] = true
L["combat logging on"] = true
L["combat logging off"] = true

-- Status.lua
L["In dungeons and raids only"] = true
L["Always"] = true
L["Never"] = true
L["just now"] = true
-- "Synced 12 minutes ago", "Last sweep: 412 players, 3 hours ago".
L["%d minute ago"] = true
L["%d minutes ago"] = true
L["%d hour ago"] = true
L["%d hours ago"] = true
L["%d day ago"] = true
L["%d days ago"] = true
-- %s: "12 minutes ago" (above), then the name the companion app is signed in as.
L["Synced %s as %s."] = true
L["Synced %s."] = true
L["Signed out now: sign in from the Ryna companion app."] = true
L["Not synced yet. Sign in from the Ryna companion app."] = true
L["Update available: Ryna addon %s (you have %s)."] = true
L["Update available: companion app %s (you have %s)."] = true
L["unavailable on this client"] = true
L["on (turned on by Ryna)"] = true
L["not turned on by Ryna"] = true

-- Panel.lua
-- %s: a combat-log mode ("Always").
L["%s (click to change)"] = true
L["Ryna records your characters, bags, quests and guild for the Ryna companion app, which "
  .. "uploads them when you sign in there. Signing in happens in the app, not in the game."] = true
-- The panel's tabs.
L["Status"] = true
L["Recording"] = true
L["Census"] = true
L["Messages"] = true
L["Advanced"] = true
L["Turn on the game's combat log:"] = true
L["The combat log is how Ryna sees fights. \"In dungeons and raids only\" keeps the "
  .. "file small. Logging you start yourself with /combatlog is left alone."] = true
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
L["Advanced combat logging"] = true
L["Sets the game's advanced combat logging on, so fights are logged with positions and health. "
  .. "Needed for Ryna's timelines and deaths."] = true
L["Show the minimap button"] = true
L["Drag it around the minimap to move it."] = true
L["Debug messages"] = true
L["Prints what Ryna is doing to the chat window: scans, journal events, sightings. "
  .. "For checking that it works."] = true
L["About your own characters, Ryna records their gear, bags and bank, quests, "
  .. "professions, reputations, gold and played time, your guild's roster, and a 30-day journal of what "
  .. "you do. Other players are the Census tab's."] = true
L["The census keeps who was seen, never what they said. "
  .. "Its data is never sold or used for advertising."] = true
L["From the companion app, newest first, as of your last login or /reload."] = true
L["Defaults"] = true
L["Every setting back to its default. The minimap button keeps its place."] = true
L["Ryna addon version: %s"] = true
-- %s: a date and time ("2026-11-06 14:02"), then how long ago ("12 minutes ago").
L["Companion app status file: as of %s (%s)"] = true
L["Companion app status file: nothing in it yet"] = true
-- The status tab's census line. %s in "Census: %s" is the sweep's progress or why it can't run.
L["Census: off"] = true
L["Census: %s"] = true
L["Last census sweep: none yet"] = true
-- %s: "%d player" / "%d players" (Census.lua), then how long ago ("20 minutes ago").
L["Last census sweep: %s, %s"] = true
L["settings back to their defaults; debug messages off."] = true
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
-- %s: "%d events this session" (below).
L["Journal: %s, %d kept in all (last 30 days)"] = true
L["%d event this session"] = true
L["%d events this session"] = true
L["Session: %s"] = true
L["not started"] = true
L["Combat log: %s right now. Mode: %s"] = true
L["Companion app: %s"] = true
L["Companion app version: %s"] = true
L["(The app's status is as of your last login or /reload.)"] = true
L["%d error caught; /ryna status shows the last."] = true
L["%d errors caught; /ryna status shows the last."] = true
L["No messages"] = true
-- A companion-app message's time before its text: "12 minutes ago: ...".
L["%s: "] = true
L["the settings open after combat."] = true
L["no settings panel on this client; /ryna status prints the status."] = true

-- Minimap.lua
L["Update available"] = true
L["Click: open Ryna. Drag: move this button."] = true

-- Census.lua
L["%d player"] = true
L["%d players"] = true
-- %s here and in the census lines below: "%d player" / "%d players" (above).
L["Sweeping: %d of %d queries, %s so far"] = true
L["The game blocked /who from Ryna this session."] = true
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
L["the game refused /who from Ryna"] = true
L["the server didn't answer /who"] = true
L["census sweep done: %s, %d new to this sweep's window, in %ds"] = true
L["census sweep stopped (%s): %s, %d new to this sweep's window, in %ds"] = true
L["census sweep: not ready yet."] = true
L["census sweep: one is already running. %s"] = true
L["census sweep: the game blocked /who from Ryna earlier this session; "
  .. "/reload to try again."] = true
L["census sweep: this client has no /who for addons."] = true
L["census sweep: not now, %s."] = true
L["census sweep started: asking the server who is online, one query every 4s."] = true
L["stopped"] = true
L["the game blocked /who from Ryna"] = true
L["combat started"] = true
L["entered an instance"] = true

-- Slash.lua
-- Setting states in /ryna status and /ryna debug ("debug messages are on").
L["on"] = true
L["off"] = true
-- No date: "bank as of never".
L["never"] = true
-- /ryna help. Command words and "||" (one "|" in chat) stay as they are.
L["v%s. Commands:"] = true
L["/ryna: open the Ryna settings panel"] = true
L["/ryna status: what is recorded, the journal's size, combat logging"] = true
L["/ryna combatlog instances||always||never: when the addon turns combat logging on"] = true
L["/ryna debug on||off||toggle: print what Ryna is doing to chat "
  .. "(/ryna debug alone: which it is)"] = true
L["/ryna census: run a realm census now (/who, a few queries at a time); "
  .. "/ryna census stop ends it"] = true
L["stopped by you"] = true
L["no census sweep is running."] = true
L["usage: /ryna census (runs a sweep now) or /ryna census stop"] = true
L["debug messages are %s. /ryna debug on||off||toggle changes it."] = true
L["usage: /ryna debug on||off||toggle (now %s)"] = true
L["debug messages are already %s."] = true
L["not initialised yet"] = true
-- /ryna status.
L["v%s, schema %d, flavor %s, client %s (interface %s)"] = true
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
-- %s: "%d events" (below), then a date, then the session id.
L["journal: %s since %s; session %s, %d this session"] = true
L["%d event"] = true
L["%d events"] = true
-- %s: the mode's command word (instances, always, never), then on, off or unavailable.
L["combat log: setting %s, currently %s"] = true
L["unavailable"] = true
L["debug messages: %s"] = true
L["companion app: %s"] = true
-- "2 caught errors; last: <the error>".
L["%s; last: %s"] = true
L["%d caught error"] = true
L["%d caught errors"] = true
L["combat logging: %s"] = true
L["usage: /ryna combatlog instances||always||never (now %s)"] = true
