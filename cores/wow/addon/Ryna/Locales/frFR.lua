-- French (frFR) strings for Ryna. Loaded on every client, applied only when the game runs
-- in frFR: GetLocale() is "frFR".
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
--     or SET textLocale "frFR" in WTF/Config.wtf, with that language installed), log in, then
--     /ryna, /ryna status and /ryna help.
-- This file is generated from enUS.lua by cores/wow/addon/tools/strings.mjs --templates, which keeps the
-- translations already here; run it after enUS.lua changes.

local _, ns = ...
if ns.Locale() ~= "frFR" then return end
local L = ns.L

-- Core.lua
L["debug messages off."] = nil
L["an error in %s was caught (reported once): %s"] = nil

-- Status.lua
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

-- CombatLog.lua
L["combat logging on (instance)"] = nil
L["combat logging on"] = nil
L["combat logging off"] = nil
L["In dungeons and raids only"] = nil
L["Always"] = nil
L["Never"] = nil
L["unavailable on this client"] = nil
L["on (turned on by Ryna)"] = nil
L["not turned on by Ryna"] = nil

-- Panel.lua
L["Ryna records your characters, bags, quests and guild for the Ryna companion app, which "
  .. "uploads them when you sign in there. Signing in happens in the app, not in the game."] = nil
-- The panel's tabs.
L["Status"] = nil
L["Messages"] = nil
L["Advanced"] = nil
L["Show the minimap button"] = nil
L["Drag it around the minimap to move it."] = nil
L["Debug messages"] = nil
L["Prints what Ryna is doing to the chat window: scans, journal events, sightings. "
  .. "For checking that it works."] = nil
L["From the companion app, newest first, as of your last login or /reload."] = nil
L["Defaults"] = nil
L["Every setting back to its default. The minimap button keeps its place."] = nil
L["Ryna addon version: %s"] = nil
-- %s: a date and time ("2026-11-06 14:02"), then how long ago ("12 minutes ago").
L["Companion app status file: as of %s (%s)"] = nil
L["Companion app status file: nothing in it yet"] = nil
L["settings back to their defaults; debug messages off."] = nil
-- %s: "%d events this session" (below).
L["Journal: %s, %d kept in all (last 30 days)"] = nil
L["%d event this session"] = nil
L["%d events this session"] = nil
L["Session: %s"] = nil
L["not started"] = nil
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
-- The Mods tab: its name, what it says, and each mod's state under its box (%s: the mod's version).
L["Mods"] = nil
L["Mods are optional addons beside Ryna that the companion app installs. Turning one "
  .. "off here turns it off in the game's AddOns list from the next reload; the app's Game tab removes it."] = nil
L["No mods are installed. The companion app's Game tab installs them."] = nil
L["Version %s."] = nil
L["On."] = nil
L["Off."] = nil
L["On until the next reload, then off."] = nil
L["Off until the next reload, then on."] = nil
-- The reload offer after a mod is turned on or off, and its buttons where the game has none.
L["Reload the interface now, so the change to Ryna's mods takes effect?"] = nil
L["Reload"] = nil
L["Later"] = nil
L["type /reload to apply the change."] = nil
L["the game didn't let Ryna change its addon list; use the AddOns button on the character screen."] = nil
L["Combat log: %s right now. Mode: %s"] = nil
-- %s: a combat-log mode ("Always").
L["%s (click to change)"] = nil
L["Turn on the game's combat log:"] = nil
L["The combat log is how Ryna sees fights. \"In dungeons and raids only\" keeps the "
  .. "file small. Logging you start yourself with /combatlog is left alone."] = nil
L["Advanced combat logging"] = nil
L["Sets the game's advanced combat logging on, so fights are logged with positions and health. "
  .. "Needed for Ryna's timelines and deaths."] = nil
-- The Advanced tab's two groups: the combat log, then everything else.
L["Combat log"] = nil
L["Other settings"] = nil

-- Minimap.lua
L["Update available"] = nil
L["Click: open Ryna. Drag: move this button."] = nil

-- Slash.lua: the key binding, in Key Bindings > AddOns: its header (the addon's name) and its name.
-- Setting states in /ryna status and /ryna debug ("debug messages are on").
L["on"] = nil
L["off"] = nil
-- No date: "bank as of never".
L["never"] = nil
-- /ryna help. Command words and "||" (one "|" in chat) stay as they are.
L["v%s. Commands:"] = nil
L["/ryna: open the Ryna settings panel"] = nil
L["/ryna status: what is recorded, the journal's size, combat logging"] = nil
L["/ryna debug on||off||toggle: print what Ryna is doing to chat "
  .. "(/ryna debug alone: which it is)"] = nil
L["debug messages are %s. /ryna debug on||off||toggle changes it."] = nil
L["usage: /ryna debug on||off||toggle (now %s)"] = nil
L["debug messages are already %s."] = nil
L["not initialised yet"] = nil
-- /ryna status.
L["v%s, schema %d, flavor %s, client %s (interface %s)"] = nil
-- %s: "%d events" (below), then a date, then the session id.
L["journal: %s since %s; session %s, %d this session"] = nil
L["%d event"] = nil
L["%d events"] = nil
L["debug messages: %s"] = nil
L["companion app: %s"] = nil
-- "2 caught errors; last: <the error>".
L["%s; last: %s"] = nil
L["%d caught error"] = nil
L["%d caught errors"] = nil
L["Ryna"] = nil
L["Next census query"] = nil
-- %s: the mode's command word (instances, always, never), then on, off or unavailable.
L["combat log: setting %s, currently %s"] = nil
L["unavailable"] = nil
L["/ryna combatlog instances||always||never: when the addon turns combat logging on"] = nil
L["combat logging: %s"] = nil
L["usage: /ryna combatlog instances||always||never (now %s)"] = nil

-- Mods.lua
-- A key binding pressed while the mod behind it (Chronicle, for Next census query) is off.
L["that key binding belongs to a mod that isn't loaded."] = nil

-- Init.lua
-- The login line, after "Ryna 0.2.1: ".
L["recording. /ryna for settings."] = nil
L["the companion app hasn't synced recently."] = nil
