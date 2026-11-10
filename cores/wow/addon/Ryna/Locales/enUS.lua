-- English (enUS): every string Ryna shows, keyed by the English text itself. `true` means "the
-- text is the key"; ns.L returns the key for anything no locale file translates, so English is
-- always the fallback. The other Locales/*.lua files are templates for translators, generated from
-- this one: after adding, changing or removing a string here, run (from the repository root)
--
--   node cores/wow/addon/tools/strings.mjs --templates
--
-- which rewrites them with the same keys, keeping every translation already in them. Without
-- --templates it only checks that this list matches what the code uses (L["..."] and N("...")).
-- Debug lines (ns.Debug) are English on purpose and are not listed.

local _, ns = ...
local L = ns.L

-- Core.lua
L["debug messages off."] = true
L["an error in %s was caught (reported once): %s"] = true

-- Status.lua
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
-- Supported versions: the API's words when this version is old (reduced) or too old (unsupported),
-- the same sentences as the companion app and ryna.io, then where to update.
L["This version of Ryna is getting old. Your characters still sync, but what you add to "
  .. "everyone's census and prices is paused until you update."] = true
L["This version of Ryna is no longer supported, so nothing syncs to ryna.io. Update to "
  .. "carry on."] = true
L["Open the Ryna app to update."] = true

-- CombatLog.lua
L["combat logging on (instance)"] = true
L["combat logging on"] = true
L["combat logging off"] = true
L["In dungeons and raids only"] = true
L["Always"] = true
L["Never"] = true
L["unavailable on this client"] = true
L["on (turned on by Ryna)"] = true
L["not turned on by Ryna"] = true

-- Panel.lua
L["Ryna records your characters, bags, quests and guild for the Ryna companion app, which "
  .. "uploads them when you sign in there. Signing in happens in the app, not in the game."] = true
-- The panel's tabs.
L["Status"] = true
L["Messages"] = true
L["Advanced"] = true
L["Show the minimap button"] = true
L["Drag it around the minimap to move it."] = true
L["Chat notices"] = true
L["A line in chat when you log in, when the companion app hasn't synced, and when Ryna turns "
  .. "combat logging on or off. Off, Ryna only answers /ryna."] = true
L["Debug messages"] = true
L["Prints what Ryna is doing to the chat window: scans, journal events, sightings. "
  .. "For checking that it works."] = true
L["From the companion app, newest first, as of your last login or /reload."] = true
L["Defaults"] = true
L["Every setting back to its default. The minimap button keeps its place."] = true
L["Ryna addon version: %s"] = true
-- The credits line at the foot of the Advanced tab: a name, then their line from credits.json.
L["Thanks to %s: %s"] = true
-- %s: a date and time ("2026-11-06 14:02"), then how long ago ("12 minutes ago").
L["Companion app status file: as of %s (%s)"] = true
L["Companion app status file: nothing in it yet"] = true
L["settings back to their defaults; debug messages off."] = true
-- %s: "%d events this session" (below).
L["Journal: %s, %d kept in all (last 30 days)"] = true
L["%d event this session"] = true
L["%d events this session"] = true
L["Session: %s"] = true
L["not started"] = true
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
-- The Mods tab: its name, what it says, and each mod's state under its box (%s: the mod's version).
L["Mods"] = true
L["Mods are optional addons beside Ryna that the companion app installs. Turning one "
  .. "off here turns it off in the game's AddOns list from the next reload; the app's Game tab removes it."] = true
L["No mods are installed. The companion app's Game tab installs them."] = true
L["Version %s."] = true
L["On."] = true
L["Off."] = true
L["On until the next reload, then off."] = true
L["Off until the next reload, then on."] = true
-- The reload offer after a mod is turned on or off, and its buttons where the game has none.
L["Reload the interface now, so the change to Ryna's mods takes effect?"] = true
L["Reload"] = true
L["Later"] = true
L["type /reload to apply the change."] = true
L["the game didn't let Ryna change its addon list; use the AddOns button on the character screen."] = true
L["Combat log: %s right now. Mode: %s"] = true
-- %s: a combat-log mode ("Always").
L["%s (click to change)"] = true
L["Turn on the game's combat log:"] = true
L["The combat log is how Ryna sees fights. \"In dungeons and raids only\" keeps the "
  .. "file small. Logging you start yourself with /combatlog is left alone."] = true
L["Advanced combat logging"] = true
L["Sets the game's advanced combat logging on, so fights are logged with positions and health. "
  .. "Needed for Ryna's timelines and deaths."] = true
-- The Advanced tab's two groups: the combat log, then everything else.
L["Combat log"] = true
L["Other settings"] = true

-- Minimap.lua
L["Update available"] = true
L["Click: open Ryna. Drag: move this button."] = true

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
L["/ryna debug on||off||toggle: print what Ryna is doing to chat "
  .. "(/ryna debug alone: which it is)"] = true
L["debug messages are %s. /ryna debug on||off||toggle changes it."] = true
L["usage: /ryna debug on||off||toggle (now %s)"] = true
L["debug messages are already %s."] = true
L["not initialised yet"] = true
-- /ryna status.
L["v%s, schema %d, flavor %s, client %s (interface %s)"] = true
-- %s: "%d events" (below), then a date, then the session id.
L["journal: %s since %s; session %s, %d this session"] = true
L["%d event"] = true
L["%d events"] = true
L["debug messages: %s"] = true
L["companion app: %s"] = true
-- "2 caught errors; last: <the error>".
L["%s; last: %s"] = true
L["%d caught error"] = true
L["%d caught errors"] = true
-- %s: the mode's command word (instances, always, never), then on, off or unavailable.
L["combat log: setting %s, currently %s"] = true
L["unavailable"] = true
L["/ryna combatlog instances||always||never: when the addon turns combat logging on"] = true
L["combat logging: %s"] = true
L["usage: /ryna combatlog instances||always||never (now %s)"] = true

-- Window.lua
-- Ryna's window: the three groups of its rail (what is about you, about the world, about fights and
-- money), the character picker's first choice, and a character whose name the file doesn't have.
L["Me"] = true
L["World"] = true
L["Combat and gold"] = true
L["All characters"] = true
L["Unknown character"] = true
-- A list with nothing in it: the search matched nothing, or nothing is recorded yet.
L["Nothing matches."] = true
L["Nothing recorded yet."] = true

-- Notice.lua
-- The notice at login when this version is no longer supported, and its button.
L["Ryna needs updating"] = true
L["Close"] = true

-- Init.lua
-- The login line, after "Ryna 0.2.1: ".
L["recording. /ryna for settings."] = true
L["the companion app hasn't synced recently."] = true
