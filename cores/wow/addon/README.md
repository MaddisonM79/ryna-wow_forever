# Ryna (addon)

World of Warcraft: Forever addons that record what your characters have and do into one
account-wide SavedVariable, `RynaDB`, for the Ryna companion app (the agent) to read and upload.
There are two, installed side by side by the companion app:

- **Ryna** (`cores/wow/addon/Ryna`), the WoW core's in-game half and only the plumbing: `RynaDB`
  and its schema, the event dispatcher, the settings and their defaults, the journal (append,
  dedupe, prune), the settings panel and its tabs, the strings (`Locales/`), `/ryna`, error
  reporting, the minimap button, reading the app's `AgentStatus.lua`, and the
  combat log (`CombatLog.lua`: turning it on where the mode wants it, and Advanced combat logging),
  which is how the app gets fights whatever mods are on. On its own it records nothing about a
  character.
- **Ryna Chronicle** (`mods/wow/chronicle/addon/Ryna_Chronicle`, `## Dependencies: Ryna`), the first
  mod and on by default: everything recorded about a character (below), the tooltips, and the
  panel's Recording and Census tabs. It builds on the core only through the mod API (below) and
  writes exactly the `RynaDB` the single addon wrote before the split (docs/wow.md section 2).
- **Ryna Automation** (`mods/wow/automation/addon/Ryna_Automation`), off by default: chores at
  quest givers and vendors, each its own setting that starts off, with its own Automation tab and
  `/ryna auto` (`mods/wow/README.md`). Holding Shift at an NPC pauses it.

Together they have a small settings panel (in tabs), a minimap button, and a line in chat when you
log in.

The game writes SavedVariables at logout and `/reload`. Addons can't reach the network or the
filesystem, so that file is the only way the data leaves the game. The addon never depends on
reading its own file back: every session it rebuilds the character record from the live game, and
it only carries over the character table and the journal (which it appends to). If the client
fails to restore SavedVariables, nothing breaks; the backend is the system of record.

## What it records

Per character (keyed by its GUID):

- Identity: first and last name (Forever's `First-Last`), the full name exactly as the client
  returns it, realm and the GUID's realm id, class, race, faction, gender.
- Level, experience, gold, played time (`/played`, requested quietly at login and after each
  level-up), zone and subzone, when it was last seen online.
- Equipped gear (item ids, links, quality, enchant), a short talent summary, professions with skill
  ranks (and known recipes, read when you open a profession window), reputations.
- Bags and keyring. Bank contents only while the bank is open; the last bank reading is kept, with
  the time it was taken, until your next visit.
- The quest log (titles, levels, objectives, when accepted) and the ids of completed quests.
- Your guild's roster (names, levels, classes, ranks, zones, notes, online status; officer notes only
  if your rank can see them), refreshed at login and every 10 minutes.
- Kills and loot (Chronicle's `Chronicle.lua`, below).

**Kills and loot** are running totals on the character record, not journal events: a session has
thousands of kills, and an event each would make the file large and logout slow. Nothing here reads
the combat log, which this client keeps from addons (asking for it raised the "blocked from an action
only available to the Blizzard UI" dialog, 0.2.2): a kill is the chat line the game prints for it,
"X dies, you gain N experience", and the creature's id comes from the units you target, mouse over or
see on nameplates, joined on the name. At the level cap there is no such line, so there is a second
signal: when your **target dies** (its health event, or targeting a dead unit), the kill is counted
for its creature id, once per creature (by its GUID), and only for a unit you had as a living target
and that another player hadn't tapped; it gives no experience. The two never count one kill twice:
each kill either signal counts leaves a mark under the creature's name, and the other signal for the
same name within 5 seconds takes that mark instead of counting again (an experience line that finds
the target's mark adds its experience to the kill already counted). `kills` counts each kind of creature (by creature id: how many,
how many of those were solo kills, the experience they gave you, how many of their corpses you opened,
and the last kill's time and zone); `loot` counts each item you looted (how often, how many in all, and from what:
up to 8 sources, a creature, a game object, a container item or fishing); `objects` counts the chests,
veins, herbs and pools you opened. A corpse reopened isn't counted twice. Money and currency aren't
counted (gold is the journal's). Caps keep the file small: 2000 creatures, 3000 items and 2000
objects per character, the least recent dropped first. **Record kills and loot** (Recording tab, on)
turns the counting off; the totals so far are kept. **Show my numbers on tooltips** (on) adds your
own numbers to tooltips in game: a creature's "Ryna: killed 43 (40 yours), last Elwynn Forest 2 hours
ago" and its top three drops ("Linen Cloth: 22 of 38 corpses (58%)"), and an item's "Ryna: looted 30
times (45), mostly from Kobold Vermin (22)". Only this character's numbers, never anyone else's.

A **journal** of events, kept for 30 days and at most 100,000 events (when it goes past that, the
oldest are dropped). The cap is high on purpose: the addon may be installed days before the companion
app first uploads, and the journal has to hold several days of heavy play until then. It records
logins and logouts, zone changes, quests accepted and turned in, level-ups, deaths, gold changes of a
gold or more, group changes, and **sightings** of other players (see the census settings below).

A sighting records only *who* and *where*: the other player's GUID, name, and level, class, race,
guild and zone where the client gives them. Sightings come from your group, from your guild roster
(online members), from players near you, from chat, and from your own `/who` (below).

**Nearby** (source `nameplate`, `target` or `mouseover`): a player whose nameplate appears, whom you
target, or whom you mouse over gives their level, class, race and guild as the client shows them,
with your own current zone, and their **faction**: a `faction` field on the `seen` event, `"Horde"`
or `"Alliance"` (the English token `UnitFactionGroup` gives), on every nearby sighting, your own
faction's included. Players of both factions are recorded, and the API prefers the event's faction
over the observer's. Chat, group, guild-roster and `/who` sightings carry no `faction`: they are your
own faction's players by construction.

Nameplates only appear for players the game already draws, and the addon asks the client nothing
extra. **The game shows nameplates for enemies only unless friendly nameplates are on** (Shift+V,
the game's `nameplateShowFriends` variable), so with them off the nearby source sees mostly the
other faction. The Census tab says so under "Record players I see nearby" and has a **Turn friendly
nameplates on** button, which sets `nameplateShowFriends` to 1 when you click it (it reads
"Friendly nameplates are on" once it is). The addon never changes it by itself.

**Chat**: a chat message yields only a sighting of its sender (GUID and name, plus class and race
from the client's cache) and, for chat that is only heard nearby or in your zone, where: say, yell,
emotes and the zone's General channel get your own current zone. Trade, LocalDefense and
LookingForGroup get no zone (that would need a list of cities, which the addon doesn't keep), and
neither do whispers, guild, party, raid or instance chat. Nothing from chat beyond who spoke and
that zone is kept: the addon never reads the message text or the recipient into anything, and reads
the channel's id and base name only to decide whether the zone applies; it doesn't store any of
them, buffer them, log them or print them.

**At most one sighting per player per 10 minutes**, shared by chat and nearby players, so someone
who talks in chat and walks past isn't counted twice. The one exception: a sighting that has a level
is recorded after an earlier one in the window that didn't (a chat line has no level; the nameplate
does). Group and guild-roster sightings are always recorded and count towards the window. The
window is kept by GUID and by name, since a `/who` row has no GUID: a player seen in chat or nearby
isn't counted again by your `/who` in the same 10 minutes, and the other way round.

### Your own `/who`

Ryna never sends /who itself. When you type /who, the results are recorded as sightings, like
players you see on nameplates or in chat. The client allows `/who` only from a key press or a
click, and an addon driving that is automation, so the census sweep Ryna once had (timed or
click-by-click `/who` queries, its key binding and `/ryna census`) is gone.

- **What it reads**: when the answer arrives (`WHO_LIST_UPDATE`, for a `/who` you typed or one you
  asked from the game's Who window), Chronicle's `Census.lua` reads the list the client already holds
  (`C_FriendList.GetNumWhoResults` and `GetWhoInfo`, or the old globals). It sends nothing, and it
  doesn't touch the game's who-to-window setting or the Who window. The words you asked come from a
  post-hook on `SendWho` (`hooksecurefunc`), which runs after the game sent your query and only notes
  them. An answer the game prints into chat instead of the Who window (a short one, with that
  setting off) may come without `WHO_LIST_UPDATE` on some clients, and then nothing is recorded. A
  `/who` another addon sends is answered the same way, so it is read too.
- **What it records**: per answer, one `who` journal event `{ query, count, batch }` (`count` is how
  many players the server returned, `batch` names the answer); per player in it, a `seen` event
  (source `who`, with the same `batch`): name, level, class (the English token), race, guild and
  zone, no GUID (`/who` gives none). You are left out, a name twice in one answer is one sighting,
  and a player already seen in the last 10 minutes isn't recorded again (see above). Sorting the Who
  window brings the same answer again; it isn't recorded twice.
- Only with the census setting on.

### The census's privacy rules

The census is about everyone and names no one in its numbers. On ryna.io:

- Looking up a character by name shows what `/who` would: name, level, class, guild, and the zone
  (never the subzone), with when it was last seen only as "today", "this week", "this month" or
  "older", never a time of day.
- There are no per-character views of hours or days; those exist only as totals across all players.
- Every lookup is logged for the operator, so someone searching one name over and over is visible.
- Lookups need a ryna.io account with the WoW app.
- We can't verify that a removal request comes from the account holder, so, like Warcraft Logs, we
  don't offer one; the limits above are what protect a player instead.

`RynaDB.client` holds the client's version and Interface number and how it returned your character's
name parts, overwritten each login, so the format can be checked against a real client.

The exact table shape is the contract with the agent, documented alongside the backend; the field
names match the JSON the agent uploads one for one.

## Where the file is

`WTF/Account/<ACCOUNT>/SavedVariables/Ryna.lua` under the Forever install:

- macOS: `/Applications/World of Warcraft/_forever_/WTF/Account/<ACCOUNT>/SavedVariables/Ryna.lua`
- Windows: `C:\Program Files (x86)\World of Warcraft\_forever_\WTF\Account\<ACCOUNT>\SavedVariables\Ryna.lua`

`<ACCOUNT>` is the game account's folder. `WTF` only exists after the first login.

## The panel

`/ryna`, or a click on the minimap button, opens **Settings → AddOns → Ryna** (on older clients,
Interface Options → AddOns → Ryna). Under its title is a row of tabs (Status, Recording, Census,
Automation, Messages, Mods, Advanced; Recording and Census are Chronicle's and Automation is
Automation's, and each goes when its mod is off); a click on one shows that
tab (the content scrolls when it's taller than the window), and the panel opens on the tab you last
had (`RynaDB.settings.panelTab`). The tabs use the game's own tab style where the client has it, and
plain buttons with the selected one lit where it doesn't.

**Status**: the character you're on; what's been recorded (items in your bags and bank, equipped
gear, quests in your log and completed, guild members); how many journal events this session and in
all, and the session id; the Chronicle's totals ("Chronicle: 41 creatures, 112 items", or "Chronicle:
off"); this session's kills and experience per hour ("Session: 42 kills, 3,120 XP/h", from the session
strip's numbers, below); whether combat logging is on right now and the mode; whether the census is on ("Census: on"
or "Census: off");
and the companion app: "Synced 12 minutes ago as <name>", or "Not synced yet. Sign in from the Ryna
companion app.", its version, and "Update available" when the app knows of a newer addon or app.
Signing in happens in the companion app, never in the game: an addon can't reach the network.

**Recording** (Chronicle's): one line on what is recorded about your own characters, Record kills
and loot, and Show my numbers on tooltips; then **In game**: Show the session strip, Lock the strip
(greyed while the strip is hidden), a **Reset position** button (the strip back to the top of the
screen) and an **Open the Chronicle window** button.

**Census**: first, "Ryna never sends /who itself. When you type /who, the results are recorded as
sightings, like players you see on nameplates or in chat." Then Take part in the census, with its
three switches under it (greyed while it's off): chat, your group and players nearby. Under players
nearby, a note that the game draws nameplates for enemies only unless friendly nameplates are on
(Shift+V), and the **Turn friendly nameplates on** button ("Friendly nameplates are on", greyed, once
they are). Last, the census's promise: who was seen, never what they said, and the data is never sold
or used for advertising.

**Messages**: up to 10 notices from the companion app, newest first ("No messages" when there are
none).

**Mods**: every `Ryna_*` addon in the game's AddOns list, each with a box: on means it loads, off
turns it off in the game's AddOns list (`C_AddOns.DisableAddOn`, or `EnableAddOn` to turn it back
on), which takes effect at the next reload, so the game's reload dialog follows. Under each, its
version and state ("On.", "Off until the next reload, then on.", ...). The companion app's Game tab
installs and removes the folders; a mod the app hasn't installed isn't in the game's list, so it
isn't here. The app reads the game's AddOns lists, so a mod turned off here shows as off in the game
there too.

**Advanced**: first a **Combat log** group (the core's, there whatever mods are on): the combat-log
mode ("Turn on the game's combat log") and Advanced combat logging; then **Other settings**: Debug
messages, Show the minimap button, a **Defaults** button (every setting back to
its default, as the Settings window's own Defaults does; the minimap button keeps its place and the
panel its tab), the addon's version, and how recent the companion app's `AgentStatus.lua` is (the
newest time in it: the last upload, an account's save or upload, or a message, since the file holds
no time of its own).

The settings (saved in `RynaDB.settings`; if the game loses them, the defaults come back):

| Setting | Tab | Default | What it does |
|---|---|---|---|
| Turn on the game's combat log | Advanced | In dungeons and raids only | When the addon turns combat logging on (see below). Same as `/ryna combatlog`. |
| Advanced combat logging | Advanced | on | Sets the game's `advancedCombatLogging`, so fights are logged with positions and health (see below). |
| Record kills and loot | Recording | on | Counts kills, the experience they give, and loot into the character record (see "Kills and loot"). Off: nothing is counted; the totals so far are kept. |
| Show my numbers on tooltips | Recording | on | Adds your own kill and loot numbers to creature and item tooltips. |
| Show the session strip | Recording | on | The session strip on screen (`sessionStrip`; see "The session strip"). Off: hidden; the session is still counted, and the window and the Status line still show it. |
| Lock the strip | Recording | off | Locked, the strip can't be dragged (`sessionStripLocked`). Right-clicking the strip switches it too. Its place is `sessionStripPos` (Reset position clears it); the panel's Defaults keeps it. |
| Take part in the census | Census | on | Records the other players you see. Off: **no** `seen` events at all, from chat, players nearby, your group, your guild roster or your own `/who`; the addon records only your own characters (and still your guild's roster, which is your guild's own list). |
| Record who I see in chat | Census | on | Part of the census. A `seen` event for who speaks in chat (only who, never what). Off: chat isn't looked at. |
| Record other players in my group | Census | on | Part of the census. Off: no `seen` events for group members, and `group` events say only that you joined, changed or left a group, without the member list. |
| Record players I see nearby | Census | on | Part of the census. Players whose nameplates, target or mouseover you see, of either faction: their level, class, guild and faction, and where. Off: those three sources are ignored. Friendly nameplates are the game's own setting (Shift+V); the button under this one turns them on. |
| Debug messages | Advanced | off | Prints what Ryna is doing to the chat window, one dimmed `Ryna debug:` line each: scans as they run (played time, gear, professions, reputations, bags and bank, quest log, group, guild roster), every journal event, every sighting and every sighting skipped (by the 10-minute window or a census setting; nearby players print at most one line per 5 seconds, a count with the names when there are 5 or fewer), each answer to your own `/who`, combat-log decisions and settings changes. For checking that it works. Same as `/ryna debug`. |
| Show the minimap button | Advanced | on | Hides or shows the button. `/ryna` still opens the panel. |

The settings are for the whole game account (`RynaDB` is account-wide), not per character.

`censusSweep`, the switch for the deleted `/who` sweep, is gone: the core removes it from
`RynaDB.settings` on load, and Chronicle removes the sweep's own state (`RynaDB.census`) at login.
Neither the companion app nor the API reads them.

## The session strip

Chronicle's in-game numbers for the session you're playing, on one line at the top of the screen
(`RynaChronicleStrip`):

    Kills 42 · 3,120 XP/h · 1g 20s/h · 2 quests · 1h 12m

Kills this session, experience per hour, money per hour (the net change: what you gained less what
you spent; whole silver once it's a gold or more), quests turned in and the time since you logged
in. It's the game's own small font on a translucent black backdrop and nothing more, so it sits as
well in the default UI as in a dark one like ElvUI's. Hovering it shows when the session started,
kills, experience, levels gained, money (net, and gained / spent), quests, items looted and deaths,
with the per-hour rates, and the session's top 8 creatures (kills and experience). A **click** opens
the Chronicle window; a **right-click** locks or unlocks it (said in chat); unlocked, **Shift-drag**
moves it, and the place is kept (`RynaDB.settings.sessionStripPos`). It's clamped to the screen.

What a session counts (Chronicle's `Session.lua`, kept in memory): kills by creature (count and
experience; the same signals as the lifetime kills, so with Record kills and loot off there are
none), experience gained (each `PLAYER_XP_UPDATE`'s change, a level-up adding what was left of the
old bar), money gained and spent (each `PLAYER_MONEY`'s change), quests turned in, deaths, items
looted (stack sizes counted) and the time since login. Rates are per hour, worked out when they're
shown; in the first minute they're taken over a minute, so one early kill doesn't read as 3,600 an
hour. **A `/reload` keeps the session**: at logout (a `/reload` is one) it's written to
`RynaDB.chronicle.session`, and the first `PLAYER_ENTERING_WORLD` after the reload (its
`isReloadingUi`) takes it back for the same character; a real login starts a new one. Nothing else is
saved, and the companion app doesn't read it.

It redraws when something it shows changes, at most every half second, and every 5 seconds for the
clock, and only while it's shown.

## The Chronicle window

`/chronicle` (or `/ryna chronicle`), a click on the strip, or the Recording tab's button opens a
window of its own (`RynaChronicleWindow`; not the settings panel). It moves by dragging and closes
with Escape or its close button. Its four tabs, for the character you're on:

- **Session**: the session's numbers (since, kills, experience, levels gained, money, gained /
  spent, quests, items looted, deaths) with the per-hour rates, then this session's kills by
  creature (name, kills, experience), most first.
- **Kills**: every creature you've killed (the record's `kills`), most first: name, kills,
  experience, the last kill's zone and when. A search box filters by name. Hovering a row shows that
  creature's numbers (kills and how many were yours, experience, last zone, corpses looted) and its
  drops: each item looted from its corpses, in its quality colour, as "12 of 40 (30%)" of the corpses
  you opened.
- **Loot**: every item you've looted (`loot`), newest first: the name in its quality colour (the
  game's `ITEM_QUALITY_COLORS`), how often, how many in all, where it came from (the creature's name,
  "object" for a chest, vein, herb or pool, "container", "fishing"; the top two) and when. A search
  box filters by name; hovering a row shows the item's own tooltip and all its sources.
- **Levels**: the journal's `level` events for this character, oldest first: the level, when you
  reached it, the time played at it (the difference of the played totals at two level-ups; for the
  current level, `/played`'s time at this level, counted on) and the played total when you reached it.
  The journal keeps 30 days, so a level reached before then shows as the current level only, "before
  the journal".

Each list is one fixed set of 15 rows over a sorted view of the data, scrolled with the mouse wheel
or the bar beside it: only the rows on screen are filled, so 2,000 creatures cost what 15 do. A view
is sorted again only when its data or its search changed. The window redraws when something it shows
changes (a kill or loot counted, experience, money, a quest, a death, a level), at most every half
second, and only while it's open.

## The minimap button

A round button on the edge of the minimap. Click it to open the panel; drag it to move it around
the minimap (its place is saved in `RynaDB.settings.minimap`); hover it for the sync status.

## AgentStatus.lua: how the companion app talks back

An addon can't read the network or any file other than its own, so the companion app writes one:
`Interface/AddOns/Ryna/AgentStatus.lua`, rewritten after every sync, which the game loads at login
and `/reload`. It sets one global, `RynaAgentStatus`:

```lua
RynaAgentStatus = {
  agent = "0.1.0",          -- the app's version
  signedIn = true,
  as = "Name",              -- who the app is signed in as
  lastPush = 1762464000,    -- unix seconds of the last upload
  accounts = { { key = "...", label = "...", savedAt = 0, pushedAt = 0, characters = 0 } },
  latest = { addon = "0.2.0", agent = "0.2.0" },   -- newest releases the app knows of
  messages = { { at = 1762464000, level = "info", text = "..." } },  -- info, warn or error
}
```

The addon ships the file with `RynaAgentStatus = nil` and reads every field defensively: the table
may be missing or have any subset of these. Versions compare as dotted integers. What the panel
shows is as fresh as your last login or `/reload`; the companion app itself is the live view. At
login, if the file says the last upload was more than a day ago (or never), chat gets one extra
line: "Ryna: the companion app hasn't synced recently."

## Combat logging

The core addon (`CombatLog.lua`, with or without any mod) can turn the game's combat log
(`Logs/WoWCombatLog-*.txt`) on for you:

- `/ryna combatlog instances` (the default): on inside dungeons and raids, off again when you leave.
- `/ryna combatlog always`: on everywhere.
- `/ryna combatlog never`: the addon never turns it on (it turns it off only if it turned it on).

It doesn't turn off logging you started yourself with `/combatlog` in the open world. The setting
lives in `RynaDB.settings`; if the client loses SavedVariables, the default applies again. For
positions and health in the log, turn on **Advanced Combat Logging** in the game's network settings.

## Commands

- `/ryna`: opens the panel.
- `/ryna status`: the same status in chat, plus the client's version and any errors the addon caught.
- `/ryna combatlog instances|always|never`
- `/ryna debug on|off|toggle`: turns the debug messages on or off; `/ryna debug` alone says which.
  Busy scans print once per scan, not per game event, and a quest-log, reputation or profession
  line that hasn't changed isn't repeated within five minutes. Group and guild-roster sightings
  print as one summary line per scan, and nearby players at most one line per 5 seconds.
  Your own `/who` prints one line per answer: its count, how many were recorded, and how many were
  skipped by the window or appeared twice.
- `/ryna chronicle`, or `/chronicle`: opens or closes the Chronicle window (Chronicle's);
  `/chronicle session|kills|loot|levels` opens it on that tab.
- `/ryna help`: the commands.

## Installing by hand

Copy the `cores/wow/addon/Ryna` folder and the `mods/wow/chronicle/addon/Ryna_Chronicle` folder
(the release's `Ryna-<version>.zip` holds both at its top level) into the Forever install's
`Interface/AddOns`, so you have `.../_forever_/Interface/AddOns/Ryna/Ryna.toc` and
`.../Interface/AddOns/Ryna_Chronicle/Ryna_Chronicle.toc`, then restart the game or `/reload`. Without
Ryna Chronicle nothing about your characters is recorded. The agent's `install-addon` command does
the same (it installs the core and every mod that is on by default). Ryna Chronicle needs this Ryna
or newer: beside an older Ryna (0.2.6 and before, which carried Chronicle inside itself) it says so
once and loads nothing, so nothing is ever recorded twice.

## The Interface number

`Ryna.toc` says `## Interface: 16001`, our best reading of Forever's 1.60.x client; it is **not
verified**. The game loads only addons whose Interface number it accepts for Forever. If the addon
shows as out of date, check the client's number with
`/run print((select(4, GetBuildInfo())))` and put that number in the TOC (or tick "Load out of date
AddOns" on the character screen). `/ryna status` prints it too once the addon is loaded.

## Robustness

Forever runs the modern client with a Classic ruleset, and Blizzard may have trimmed APIs for it.
Every call that might be missing is checked and wrapped, with the modern API tried first and the
classic one as the fallback (`C_Container` / `GetContainerItemInfo`, `C_QuestLog` /
`GetQuestLogTitle`, `C_QuestLog.GetAllCompletedQuestIDs` / `GetQuestsCompleted`, `C_Reputation` /
`GetFactionInfo`, `GetProfessions` / `GetSkillLineInfo`, and so on). Errors are caught per event,
reported once in chat, and never stop login. The panel and the minimap button are set up after
recording starts, each under its own error guard, with plainer fallbacks for every frame template
(the modern dropdown falls back to a button that cycles the modes), so a client missing part of the
UI API still records everything. Heavy scans run a few seconds after you enter the
world, and bag and quest updates are coalesced.

## Translating Ryna

Everything Ryna shows (the panel, the minimap tooltip, chat lines and `/ryna` output) comes from
`Ryna/Locales/` and, for Chronicle's own strings, `Ryna_Chronicle/Locales/` (a key the core's files
already have isn't repeated there: Chronicle's strings fall back to the core's). The keys are the English text itself, and **English is always the fallback**: a
string a language hasn't translated shows in English, never as a blank or an error. Debug lines
(`/ryna debug`) stay English on purpose, and so does everything Ryna saves for the companion app.

To translate it into your client's language:

1. Open that language's file in `Ryna/Locales/` (and in `Ryna_Chronicle/Locales/`): `deDE.lua`, `frFR.lua`, `esES.lua`, `esMX.lua`,
   `itIT.lua`, `ptBR.lua`, `ruRU.lua`, `koKR.lua`, `zhCN.lua` or `zhTW.lua` (the name is what
   `/run print(GetLocale())` prints). Each is a copy of `enUS.lua` with every value `nil`, and only
   loads when the game runs in that language.
2. Replace `nil` with your text: `L["Take part in the census"] = "Am Zensus teilnehmen"`. Leave the key as it
   is. Keep each `%s` and `%d` in the same order, keep `||` (one `|` in chat), and keep command words
   inside commands (`/ryna debug on||off||toggle`) in English, since they are what players type. A
   pair such as `"%d player"` / `"%d players"` is the form for 1 and for every other number.
3. Test it in the game: set the client's language (Battle.net → Settings → Game Settings → Text
   Language, or `SET textLocale "deDE"` in `WTF/Config.wtf` with that language installed), log in,
   and look at `/ryna`, `/ryna status`, `/ryna help` and the minimap tooltip. `/reload` picks up
   edits to the file.

When a string in the code changes, add or change it in that addon's `enUS.lua` (as `L["..."] = true`)
and run `node cores/wow/addon/tools/strings.mjs --templates` from the repository root (it checks both
addons, or the folders you name): it rewrites the other locale files with the new keys and keeps
every translation already in them. Without `--templates` it only
checks (CI runs it): every `L["..."]` and `N("...")` key the code uses is in `enUS.lua`, nothing in
`enUS.lua` is unused, and each locale file has exactly `enUS.lua`'s keys. Text that is stored or
printed to debug in English and translated only where it is shown is marked with `N("...")` and
looked up with `L[reason]` (no string needs it today).

## The mod API

A mod is its own addon folder (`Ryna_<Name>`, `## Dependencies: Ryna`) with a `mod.json` beside it
in `mods/wow/<id>/` (docs/ryna-agent-cores-mods.md section 6). It reaches the core only through the
global `Ryna` (`Mods.lua`, where every function is described), never the core's private namespace:

- `Ryna.API` (1), `Ryna.VERSION`; `Ryna.RegisterMod{ id, addon, name, version, onLogin, onLogout,
  onDefaults }` (onLogin runs at login once `RynaDB` is set up and the journal session has started)
  and `Ryna.Mods()`.
- `Ryna.On(event, fn)`, the dispatcher; `Ryna.DB()`, `RynaDB` (a mod writes under its own key or in
  the character record it keeps; Chronicle keeps the keys the companion app reads).
- `Ryna.Journal.Add(type, fields, quiet)`, `.Session()`, `.Seq()`, `.Count()`.
- `Ryna.Settings.Get`, `.Set`, `.All`, `.AddDefaults`, `.COMBAT_LOG_MODES`.
- `Ryna.Panel.AddTab{ key, label, order, build, layout }`, `.AddStatusLines(order, fn)`, `.Refresh()`,
  `.RefreshIfShown()`, `.Widgets`; `Ryna.Slash.Add{ word, run(arg, rest), help, order }` (`rest`: the
  whole line after the word, in cores after 0.3.4),
  `.AddStatusLines(order, fn)`; `Ryna.L`.
- The helpers: `Try`, `Fn`, `IsSecret`, `NonEmpty`, `Now`, `Locale`, `Print`, `PrintRaw`, `Debug`,
  `DebugChanged`, `Debugging`, `Plural`, `N`, `Run`, `After`, `Debounce`, `ReportError`,
  `CanonicalName`, `MoneyText`, `DurationText`, `ListText`, `Ago`, `Plain`.

## Development

`luacheck cores/wow/addon/ mods/wow/ --config cores/wow/addon/.luacheckrc` from the repository root,
and `node cores/wow/addon/tools/strings.mjs` checks the translatable strings of both addons (see
"Translating Ryna"). Each addon's version is in two places, `## Version` in its `.toc` and
`ns.VERSION` (Ryna's `Core.lua`, Chronicle's `Mod.lua`); change them together (Chronicle's also in
its `mod.json`; `scripts/check-versions.mjs` holds them all to the release's).

### Advanced combat logging

On by default: the addon sets the game's `advancedCombatLogging` variable at login and whenever the setting changes, so fights are logged with positions and health.
