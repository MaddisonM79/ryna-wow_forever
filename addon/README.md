# Ryna (addon)

A World of Warcraft: Forever addon that records what your characters have and do into one
account-wide SavedVariable, `RynaDB`, for the Ryna companion app (the agent) to read and upload.
It has a small settings panel, a minimap button, and a line in chat when you log in.

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

A sighting records only *who*: the other player's GUID, name, and level, class, race, guild and zone
where the client gives them. Sightings come from your group, from your guild roster (online
members), and from chat.

**Chat**: a chat message yields only a sighting of its sender (GUID and name, plus class and race
from the client's cache), at most one per player per 10 minutes. The addon never reads the message
text, the channel or the recipient into anything: it doesn't store them, buffer them, log them or
print them. The handler takes the sender and the GUID from the event and discards the rest.

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
Interface Options → AddOns → Ryna). It has three parts.

**Status**: the character you're on; what's been recorded (items in your bags and bank, equipped
gear, quests in your log and completed, guild members); how many journal events this session and in
all, and the session id; whether combat logging is on right now and the mode; and the companion app:
"Synced 12 minutes ago as <name>", or "Not synced yet. Sign in from the Ryna companion app.", its
version, and "Update available" when the app knows of a newer addon or app. Signing in happens in the
companion app, never in the game: an addon can't reach the network.

**Settings** (saved in `RynaDB.settings`; if the game loses them, the defaults come back):

| Setting | Default | What it does |
|---|---|---|
| Turn on the game's combat log | In dungeons and raids only | When the addon turns combat logging on (see below). Same as `/ryna combatlog`. |
| Record who I see in chat | on | A `seen` event for who speaks in chat (only who, never what). Off: chat isn't looked at. |
| Take part in the census | on | Records the other players you see. Off: **no** `seen` events at all, from chat, your group or your guild roster; the addon records only your own characters (and still your guild's roster, which is your guild's own list). |
| Record other players in my group | on | Part of the census. Off: no `seen` events for group members, and `group` events say only that you joined, changed or left a group, without the member list. |
| Show the minimap button | on | Hides or shows the button. `/ryna` still opens the panel. |

The settings are for the whole game account (`RynaDB` is account-wide), not per character.

**Messages**: up to 10 notices from the companion app, newest first ("No messages" when there are
none).

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

## Development

`luacheck addon/` from the repository root uses `addon/.luacheckrc`.
