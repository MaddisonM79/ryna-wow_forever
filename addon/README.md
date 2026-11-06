# Ryna (addon)

A World of Warcraft: Forever addon that records what your characters have and do into one
account-wide SavedVariable, `RynaDB`, for the Ryna companion app (the agent) to read and upload.
It has a small settings panel (in tabs), a minimap button, and a line in chat when you log in.

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

A **journal** of events, kept for 30 days and at most 100,000 events (when it goes past that, the
oldest are dropped). The cap is high on purpose: the addon may be installed days before the companion
app first uploads, and the journal has to hold several days of heavy play until then. It records
logins and logouts, zone changes, quests accepted and turned in, level-ups, deaths, gold changes of a
gold or more, group changes, and **sightings** of other players (see the census settings below).

A sighting records only *who* and *where*: the other player's GUID, name, and level, class, race,
guild and zone where the client gives them. Sightings come from your group, from your guild roster
(online members), from players near you, from chat, and from the census sweep (`/who`, below).

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
isn't counted again by a sweep in the same 10 minutes, and the other way round.

### The census sweep (`/who`)

A sweep asks the server who is online, the way `/who` does, to count the realm's players by level,
class and zone. **The queries are generic: level brackets, class names and zone names, never a
person's name, and nothing about you is sent.**

- **The plan**: one query per level bracket, `1-9`, `10-19`, `20-29`, `30-39`, `40-49`, `50-59` and
  `60`. The server answers at most 50 players per query, so a bracket that comes back with 50 (or
  says there are more) is asked again once per class (`20-29 c-"Warrior"`, and so on for the nine
  classes), and a bracket-and-class that is still capped is asked once per zone its answer named
  (`60 c-"Mage" z-"Orgrimmar"`, at most 10 zones). A sweep sends at most 80 queries.
- **The pace**: the server throttles `/who`, so one query goes out every 4 seconds, and the next
  waits 10 seconds after an empty, failed or unanswered one (8 seconds without an answer counts as
  unanswered). An uncapped sweep is 7 queries, about half a minute. If the first two queries get no
  answer, or the game refuses the call, the sweep stops, and after a refusal no sweep runs again
  until `/reload`.
- **What it records**: per answer, one `who` journal event `{ query, count, batch }` (`count` is how
  many players the server returned); per player in it, a `seen` event (source `who`, with the same
  `batch`, which names the sweep): name, level, class (the English token), race, guild and zone, no
  GUID (`/who` gives none). A name that appears in two queries of one sweep is one sighting, and the
  10-minute window applies by name.
- **The Who window**: while a sweep runs the answers go to Ryna, not the Who window; the friends
  frame stops listening for who results and gets them back, with the game's who-to-window setting,
  as soon as the sweep ends or is stopped, so your own `/who` works as before.
- **When it runs**: `/ryna census` or the Census tab's **Run census now** button runs one now; with the
  setting "Run a realm census while I'm in a city" on, one runs every 30 minutes while you're in a
  city for at least a minute. A capital is recognised by its map id
  (`C_Map.GetBestMapForUnit("player")`, the same in every client language; a sub-map of a capital
  counts too), using the Classic world's ids: Stormwind City 1453, Orgrimmar 1454, Ironforge 1455,
  Thunder Bluff 1456, Darnassus 1457, Undercity 1458. The neutral towns (Booty Bay, Gadgetzan,
  Everlook, Ratchet) have no map of their own, so outside a capital they are matched by the subzone's
  name, which is in the client's language: on a non-English client they don't start a sweep. If the
  client can't give a map id at all, no automatic sweep runs. Never in an instance, never in combat (combat
  stops a running sweep, and it isn't resumed), never while you have the Who window open (opening
  it stops one before its next query), and never with the census setting off. `/ryna census stop` stops one.
- A sweep you start prints its progress ("census sweep: 8 of 12 queries, 214 players so far", every
  fourth answer) and a summary ("census sweep done: 412 players, 380 new to this sweep's window, in
  52s": the players it found, and how many of them weren't already seen in the last 10 minutes). An
  automatic sweep prints only debug lines. The last sweep's summary is kept in `RynaDB.census` and
  shown beside the Census tab's button and on the Status tab ("Last sweep: 412 players, 20 minutes ago").

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
Interface Options → AddOns → Ryna). Under its title is a row of five tabs; a click on one shows that
tab (the content scrolls when it's taller than the window), and the panel opens on the tab you last
had (`RynaDB.settings.panelTab`). The tabs use the game's own tab style where the client has it, and
plain buttons with the selected one lit where it doesn't.

**Status**: the character you're on; what's been recorded (items in your bags and bank, equipped
gear, quests in your log and completed, guild members); how many journal events this session and in
all, and the session id; whether combat logging is on right now and the mode; the last census sweep
("Last census sweep: 412 players, 20 minutes ago", the running one's progress, or "Census: off");
and the companion app: "Synced 12 minutes ago as <name>", or "Not synced yet. Sign in from the Ryna
companion app.", its version, and "Update available" when the app knows of a newer addon or app.
Signing in happens in the companion app, never in the game: an addon can't reach the network.

**Recording**: one line on what the addon records about your own characters, the combat-log mode and
Advanced combat logging.

**Census**: Take part in the census, with its four switches under it (greyed while it's off): chat,
your group, players nearby and the city sweep. Under players nearby, a note that the game draws
nameplates for enemies only unless friendly nameplates are on (Shift+V), and the **Turn friendly
nameplates on** button ("Friendly nameplates are on", greyed, once they are). Then the **Run census
now** button (greyed while a
sweep runs) with the last sweep's summary beside it, and the census's promise: who was seen, never
what they said, and the data is never sold or used for advertising.

**Messages**: up to 10 notices from the companion app, newest first ("No messages" when there are
none).

**Advanced**: Debug messages, Show the minimap button, a **Defaults** button (every setting back to
its default, as the Settings window's own Defaults does; the minimap button keeps its place and the
panel its tab), the addon's version, and how recent the companion app's `AgentStatus.lua` is (the
newest time in it: the last upload, an account's save or upload, or a message, since the file holds
no time of its own).

The settings (saved in `RynaDB.settings`; if the game loses them, the defaults come back):

| Setting | Tab | Default | What it does |
|---|---|---|---|
| Turn on the game's combat log | Recording | In dungeons and raids only | When the addon turns combat logging on (see below). Same as `/ryna combatlog`. |
| Advanced combat logging | Recording | on | Sets the game's `advancedCombatLogging`, so fights are logged with positions and health (see below). |
| Take part in the census | Census | on | Records the other players you see. Off: **no** `seen` events at all, from chat, players nearby, your group or your guild roster; the addon records only your own characters (and still your guild's roster, which is your guild's own list). |
| Record who I see in chat | Census | on | Part of the census. A `seen` event for who speaks in chat (only who, never what). Off: chat isn't looked at. |
| Record other players in my group | Census | on | Part of the census. Off: no `seen` events for group members, and `group` events say only that you joined, changed or left a group, without the member list. |
| Record players I see nearby | Census | on | Part of the census. Players whose nameplates, target or mouseover you see, of either faction: their level, class, guild and faction, and where. Off: those three sources are ignored. Friendly nameplates are the game's own setting (Shift+V); the button under this one turns them on. |
| Run a realm census while I'm in a city | Census | on | Part of the census. Every 30 minutes while you're parked in a city, a `/who` sweep (see "The census sweep"). Off: sweeps run only when you start one. |
| Debug messages | Advanced | off | Prints what Ryna is doing to the chat window, one dimmed `Ryna debug:` line each: scans as they run (played time, gear, professions, reputations, bags and bank, quest log, group, guild roster), every journal event, every sighting and every sighting skipped, each census-sweep query and answer (by the 10-minute window or a census setting; nearby players print at most one line per 5 seconds, a count with the names when there are 5 or fewer), combat-log decisions and settings changes. For checking that it works. Same as `/ryna debug`. |
| Show the minimap button | Advanced | on | Hides or shows the button. `/ryna` still opens the panel. |

The settings are for the whole game account (`RynaDB` is account-wide), not per character.

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

The addon can turn the game's combat log (`Logs/WoWCombatLog-*.txt`) on for you:

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
  A census sweep prints each query it sends and each answer (its count, how many were recorded,
  skipped by the window or already in the sweep, and any split), and its summary.
- `/ryna census`: runs a census sweep now; `/ryna census stop` stops it.
- `/ryna help`: the commands.

## Installing by hand

Copy the `addon/Ryna` folder into the Forever install's `Interface/AddOns`, so you have
`.../_forever_/Interface/AddOns/Ryna/Ryna.toc`, then restart the game or `/reload`. The agent's
`install-addon` command does the same.

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
`Ryna/Locales/`. The keys are the English text itself, and **English is always the fallback**: a
string a language hasn't translated shows in English, never as a blank or an error. Debug lines
(`/ryna debug`) stay English on purpose, and so does everything Ryna saves for the companion app.

To translate it into your client's language:

1. Open that language's file in `Ryna/Locales/`: `deDE.lua`, `frFR.lua`, `esES.lua`, `esMX.lua`,
   `itIT.lua`, `ptBR.lua`, `ruRU.lua`, `koKR.lua`, `zhCN.lua` or `zhTW.lua` (the name is what
   `/run print(GetLocale())` prints). Each is a copy of `enUS.lua` with every value `nil`, and only
   loads when the game runs in that language.
2. Replace `nil` with your text: `L["Run census now"] = "Zensus jetzt starten"`. Leave the key as it
   is. Keep each `%s` and `%d` in the same order, keep `||` (one `|` in chat), and keep command words
   inside commands (`/ryna debug on||off||toggle`) in English, since they are what players type. A
   pair such as `"%d player"` / `"%d players"` is the form for 1 and for every other number.
3. Test it in the game: set the client's language (Battle.net → Settings → Game Settings → Text
   Language, or `SET textLocale "deDE"` in `WTF/Config.wtf` with that language installed), log in,
   and look at `/ryna`, `/ryna status`, `/ryna help` and the minimap tooltip. `/reload` picks up
   edits to the file.

When a string in the code changes, add or change it in `enUS.lua` (as `L["..."] = true`) and run
`node addon/tools/strings.mjs --templates` from the repository root: it rewrites the other locale
files with the new keys and keeps every translation already in them. Without `--templates` it only
checks (CI runs it): every `L["..."]` and `N("...")` key the code uses is in `enUS.lua`, nothing in
`enUS.lua` is unused, and each locale file has exactly `enUS.lua`'s keys. Text that is stored or
printed to debug in English and translated only where it is shown (the census sweep's stop reasons,
kept in `RynaDB.census`) is marked with `N("...")` and looked up with `L[reason]`.

## Development

`luacheck addon/` from the repository root uses `addon/.luacheckrc`, and
`node addon/tools/strings.mjs` checks the translatable strings (see "Translating Ryna"). The
version is in two places, `## Version` in `Ryna.toc` and `ns.VERSION` in `Core.lua` (0.2.0 now);
change both together.

### Advanced combat logging

On by default: the addon sets the game's `advancedCombatLogging` variable at login and whenever the setting changes, so fights are logged with positions and health.
