# Ryna: the agent, its cores, and their mods

*Design note, 2026-11-07. Decisions are Maddison's; the shape is Nyx's proposal for them. This is
written before any plumbing code, the way `docs/wow.md` in the api repo was written before the API.*

## 1. The words

- **Ryna** is **the agent**: the one desktop app a person installs. It signs in once, keeps itself
  updated, manages what's installed, and carries data between each game and ryna.io. It runs only
  while it is open; quitting it stops everything. There is one Ryna for every game.
- A **core** is a game: World of Warcraft: Forever today, Palworld next, others later. A core knows how
  to find its game, read what the game leaves on disk, push it to ryna.io, and bring data back down.
  A core runs fine with no mods.
- A **mod** belongs to a core: an optional piece around the core's data, off until the person turns
  it on. WoW's mods are in-game addon pieces (quest automation, vendor prices, whatever earns a
  toggle). A mod can't exist without its core.

**Chronicle is the first mod** (Maddison, 2026-11-07): the WoW core lays the foundation, the files
and what the game requires, and Chronicle is the first thing built on it, then an automation mod
(quests first), then things like selling greys. The **data layer** (section 7) is the agent's, shared
by every core, and is not a mod either. The core is the plumbing only, the essential support for any mod;
everything recorded about a character is Chronicle (section 12).

Rules that don't move, from the README: the data is never sold or used for advertising; only what the
person can see in their own files leaves their computer; the agent signs in as them and never sees a
password; nothing runs in the background.

## 2. Today, and what has to change

Today the agent and the WoW core are one program. `agent/core` (Rust) is the WoW core: install
discovery, the SavedVariables reader, the snapshot pusher, the combat-log tail, the addon installer.
`agent/app` (Tauri) is the agent: sign-in, updater, tray, the window. The addon `addon/Ryna` is the
WoW core's in-game half, with Chronicle inside it. The Palworld agent is a separate Node program in
the private monorepo (`apps/palworld-agent`), run by launchd in the background on one Mac, with its
own device client at the identity provider.

What changes:

1. The agent becomes a host with a defined seam, and the WoW core becomes the first thing behind it.
   Behaviour stays identical; this is a refactor (stage 1).
2. Mods get plumbing: a manifest, a source, a toggle, verification (stage 2).
3. The data layer gets its first real tables (stage 3).
4. The Palworld core joins as a sidecar of the existing Node program, the launchd job retires, and
   one sign-in yields a token per core (stage 4).

## 3. Repository and licence

Options:

- **A. One repository, renamed `ryna_agent`.** `agent/`, `cores/wow/`, `cores/palworld/`, `mods/wow/<name>/`,
  `docs/`. One release pipeline, one place to read. GitHub redirects the old name, and the updater
  URL (`releases/latest/download/latest.json`) follows the redirect, so installed apps keep updating;
  the next release after the rename should still be checked end to end.
- **B. One repository per core and per mod,** the agent's repository holding only the host. Makes
  "pull from git" real for the agent's own cores, and lets someone contribute a mod without touching
  the host. More pipelines, more tags, more places to read.

**Decided 2026-11-07: A.** The repository is renamed `ryna_agent` (Maddison's name). B stays the path for mods that outgrow it. The agent and its cores version and
release together until a second core exists and stabilises; mods are the thing other people may write,
and the manifest (section 6) is designed so a mod's source can be another repository from day one.

Licence: this repository is MIT. **Decided: the Palworld agent goes MIT when it moves here, as long
as nothing in it is sensitive** (Maddison, 2026-11-07; she doesn't think it is). Its save parser is a
port of palworld-save-tools (MIT). Before the move, stage 4 starts with an audit of the Palworld agent
for anything that shouldn't be public: credentials, personal paths, server addresses, her world ids.

## 4. The agent

The agent owns, for every core:

- **Sign-in.** One device-grant sign-in. The identity provider issues a token per core's resource
  (section 8); the agent keeps the refresh token in the keychain or credential store as it does today.
- **Updates.** Its own, through the signed updater; each core's and each mod's, through the module
  manager (section 9).
- **The module manager.** Reads each core's manifest, detects the game, shows the core's tab, installs,
  enables and verifies mods, and keeps the per-core state directory.
- **The data channel.** Reads files a game or its mods wrote (SavedVariables, caches, logs, saves),
  pushes them through the core, and writes files back for the game to read at its next load
  (section 7).
- **The window.** Header (sign-in, Sync now), a tab per enabled core, then Settings, Activity, About.
  A core's tab is laid out by the core from a small set of parts the agent provides: status cards,
  rows, toggles, an install line, a mods list. Cores don't ship their own UI framework.
- **Activity and errors.** One log, each line tagged with its core.

A core declares itself with a manifest the agent reads at start:

```jsonc
// cores/wow/core.json
{
  "id": "wow",
  "name": "World of Warcraft: Forever",
  "version": "0.3.0",
  "runtime": { "kind": "builtin" },                 // or { "kind": "sidecar", "bin": "ryna-palworld", "args": [] }
  "resource": "https://api.ryna.io/wow/ingest",    // the token it needs
  "scopes": ["wow:ingest"],
  "detect": { "folders": ["/Applications/World of Warcraft/_forever_", "C:\\Program Files (x86)\\World of Warcraft\\_forever_"] },
  "data": { "reads": ["WTF/Account/*/SavedVariables/Ryna.lua", "Cache/WDB/*/creaturecache.wdb", "Logs/WoWCombatLog-*.txt"],
            "writes": ["Interface/AddOns/Ryna/Data/"] },
  "mods": "mods/wow"                                 // where its mods' manifests live
}
```

Between the agent and a core there is one interface: a core is started with its state directory, its
token provider and the game folder; it reports status rows and activity lines; it is told when to sync
and when to stop. The WoW core implements it in-process (Rust). A sidecar core implements it over
stdin/stdout as line-delimited JSON, started and stopped by the agent, never left running.

## 5. The cores

### WoW Forever

Exactly what `agent/core` does today, behind the interface: find the install, read every account's
`Ryna.lua`, build and push snapshots, heartbeat, tail the combat logs and upload fights, install and
update the addon. New under this design: it also reads the client's caches (section 7), writes the
data files the addon loads, and manages the WoW mods. The addon stays the core's in-game half; the
mods are separate addon folders beside it (section 6).

### Palworld

The existing Node agent, bundled as a sidecar (an esbuild bundle plus a Node binary inside the app,
the packaging Maddison already prefers for tools), started by the agent while Ryna is open, stopped
when it quits. It does what it does today: watch the save folder, parse saves, push snapshots and
heartbeats. The launchd job, `install.sh` and the Full Disk Access dance go away: the agent is the
process that reads the saves, and it is granted once. The device client it signs in with becomes the
agent's (section 8).

What a Palworld **mod** is was not decided. Two readings, and they are different products:

- *Optional agent features*: alerts, the steward's suggestions, Discord pushes, extra reports. Toggles;
  nothing touches the game. The plumbing in section 6 covers this directly.
- *Real game mods*: the agent installs and updates pak or UE4SS mods into the game folder from
  allow-listed sources, a mod manager. That is executable code on the person's computer, which the
  rest of this design keeps out (section 10), so it would need its own rules.

**Decided: nothing now.** Palworld mods will be defined in a later session; until then the plumbing
in section 6 must be extensible enough to take either reading (a mod's `kind` is per core, and a core
may refuse kinds it doesn't support), and nothing in the Palworld core assumes an answer.

## 6. Mods

A mod is a folder with a manifest, installed and toggled by the agent, belonging to one core:

```jsonc
// mods/wow/vendor-prices/mod.json
{
  "id": "vendor-prices",
  "core": "wow",
  "name": "Vendor prices on tooltips",
  "version": "0.1.0",
  "summary": "What a vendor pays for an item and its stack, on the tooltip.",
  "kind": "addon",                                   // wow: an addon folder; other cores: their kinds
  "actsInGame": false,                               // true for anything that does something for the player
  "source": { "kind": "bundled" },                   // or { "kind": "git", "repo": "https://github.com/…", "tag": "v0.1.0", "sha256": "…" }
  "requires": { "core": ">=0.3.0" },
  "defaultOn": false
}
```

- **Off by default.** A mod that acts in the game for the player (`actsInGame: true`: automation,
  anything that presses a button) says so, and the toggle's text says so, every time.
- **Bundled or fetched.** A bundled mod ships inside the agent and versions with it. A fetched mod is
  pulled by the agent from a git source on an allow-list, by pinned tag, and its files are checked
  against the manifest's hash before anything is written. The person sees the source and the version.
- **The WoW kind.** A WoW mod is its own addon folder (`Ryna_VendorPrices`, `## Dependencies: Ryna`),
  installed next to `Ryna`. The toggle in the agent installs or removes the folder; the toggle in the
  in-game panel enables or disables the addon through the client's addon list and asks for a reload.
  Both toggles show the same state. Mods may use the core's namespace for shared helpers and settings
  (a small, documented surface), never its data tables directly.
- **Libraries.** Mods and the core may use the standard libraries where they earn their place; Questie
  ships LibStub and Ace3 on this client, so the Classic set runs on Forever's Modern engine. Before
  adopting any library, read how the installed addons use it here (`docs/`'s client notes), since the
  engine restricts some APIs (the combat log above all).
- **Removal is clean.** Removing a mod removes its folder and its settings; the core's data is untouched.

## 7. The data layer

The thing only an agent can do: bring data *into* the game. Addons can't fetch, but they load files at
startup and on `/reload`, and the agent writes files. Today one such file exists (`AgentStatus.lua`,
the agent's status for the panel). The data layer generalises it: a versioned set of files the core
writes under `Interface/AddOns/Ryna/Data/`, one table each, each with a version, the addon reading
what's there and showing its age.

Three sources feed it:

1. **The client's own caches.** The game writes `Cache/WDB/<locale>/creaturecache.wdb` (every NPC's
   name, title and type by id), `gameobjectcache.wdb` (chests, veins, herbs, nodes by id) and
   `questcache.wdb` (quests the client has seen: title, text, objectives by id). They grow as the
   person plays. The core parses them, uploads what's new (`POST /v1/wow/agent/caches`, a format
   section in `docs/wow.md` to come), and the API pools them: names for everything, and the first
   complete database of Forever's own quests (ids around 86000 and up, which no public database has),
   grown by play rather than curated. The pooled tables come back down to every player.
2. **Served game tables.** What the API already serves from the client's static tables
   (`GET /v1/wow/gamedata`, versioned per table): zones, races, maps, spells, bosses.
3. **Community numbers.** Chronicle's pooled aggregates (`/v1/wow/chronicle/*`): drop rates, kill
   counts, the census. Aggregates only, about no one.

Rules: every file carries its table version and the time it was written, and the addon shows "as of";
the agent refreshes on sync, by version, so an unchanged table costs nothing; sizes are budgeted
(AllTheThings loads 64 MB of Lua at login and is famous for it; the data layer should stay in the low
megabytes, with the big tables optional per mod); the caches hold no personal data, and nothing in the
data layer is about a person. The same mechanism carries a core's data to a mod.

Nothing here needs a new addon release: a data refresh is an agent sync.

### 7.1 The census without an account (decided 2026-11-07)

Today a sighting reaches ryna.io only inside a snapshot posted with a signed-in person's agent token,
so every census row has an account behind it. Maddison's decision: **installing Ryna is the consent,
and the census may be contributed without an account**, traced by a **submission ID** instead of a
person. The design:

- **A device key.** On first run the agent mints a keypair (Ed25519) and keeps the private key in
  the keychain or credential store beside the tokens. The **submission ID** is the public key's
  fingerprint. Census uploads are signed with it, so nobody can send under someone else's ID, and an
  ID accumulates a history. The ID is shown on the Census tab and in the agent's Status, so a person
  can quote it if they ever need to.
- **A census route** of its own, `POST /v1/wow/census` (section to come in `docs/wow.md`): the
  `seen` events and `who` events as the addon writes them, the observer's realm and faction, the
  submission ID, a timestamp, the signature over the body. No token. The existing snapshot path keeps
  carrying sightings for signed-in people (their rows are attributed to the account *and* the ID), so
  nothing changes for them.
- **Reputation, not identity, keeps the numbers honest** (the API, aggregates only):
  - *Quarantine.* Rows from an ID with no history sit unweighted until other observers corroborate
    them: a sighting counts once a second ID has seen the same character within the same day, or the
    ID has a week of sightings that mostly overlap others'.
  - *Rate limits.* Per ID per hour, sized to what one client can see (nameplates and chat in a city
    at peak), with the excess dropped and noted.
  - *Weight.* An ID whose sightings nobody else ever sees loses weight over time; an ID that
    reports impossible things (a character in two zones at once, levels going down, more distinct
    players per hour than a client can render) is zeroed and its rows excluded; the operator sees why
    in Admin. Excluding an ID removes its rows from the aggregates; nothing is ever shown about the ID.
  - *Re-minting is cheap, so new IDs start at zero.* That is the point: a fresh ID can't move the
    numbers until it behaves like a real client for a while.
- **Privacy.** The observer is an install, not a person: the who-saw-whom log holds the ID, which
  links to no account unless the person also signs in. The "Census privacy" rules in `docs/wow.md`
  (coarse last-seen, zone not subzone, no per-character hours, every lookup logged, no removal path)
  are unchanged; reading the census still needs an account, since lookups are logged to someone.
- **What stays signed-in.** Everything about a person's own characters. The census is the one thing
  the agent does before, or without, signing in; the Census tab says "contributes without an account"
  in plain words, with the same switches to stop any of it.

Stage (a) built (2026-11-07, agent side, the API's docs/wow.md 13.1, 13.3 and 13.5): the device key in the keychain (`ryna-agent` `identity`), the three signed headers on every API POST, `submissionId` in the heartbeat, the ID under Census in the WoW Settings and in `ryna status`, and `ryna census push [--dry-run]` / `ryna census id` (`census.rs`); the app sends no census uploads yet.
- **Stages.** (a) The device key and the signed census route, with sightings attributed to both account
  and ID for signed-in people, so the reputation data accrues before anyone relies on it. (b) Quarantine
  and weighting, with the Admin view of IDs. (c) The agent's "census only" mode: the Census tab and
  the core run with no sign-in.

## 8. Identity

Today: one device client per game (`ryna-wow`, the Palworld agent's), each a public client at
auth.nyratech.dev bound to its own resource (`https://api.ryna.io/wow/ingest`,
`…/palworld/ingest`), each in the API's `AGENT_CLIENT_IDS`.

Target: one device client, `ryna`, for the agent; one sign-in; a token per core's resource, obtained
with the resource indicator (RFC 8707) the IdP already models, from the one refresh token. The API's
grant gate doesn't change: each core's routes still admit only an `agent` principal for that resource,
with that core's scope. The IdP work (the new client, allowing several resources per refresh token or a
refresh per core under one consent, and retiring the two old clients once nobody uses them) is
Maddison's; the agent can carry both old and new clients for a release or two.

## 9. Releases and updates

- **The agent**: as today, a `vX.Y.Z` tag, the release workflow, signed and notarized builds, the
  updater's `latest.json`.
- **Cores**: bundled in the agent and versioned with it while they're ours (option A). A core's
  manifest version is what the API's heartbeat reports.
- **Mods**: bundled ones version with the agent; fetched ones update when their pinned tag changes in
  the agent's own list (a release of the agent) or, later, from a mods index the agent reads. Either
  way: source, tag and hash, shown to the person before the first install.
- **Data**: per table, per sync, no release.
- **The addon**: still inside the agent; a new agent brings the matching addon, as today.

## 10. Security

- Sources are allow-listed in the agent (today: this repository and ryna.io, the same list the About
  tab's links use); a mod from elsewhere needs the person to add the source, with the source shown.
- Files are verified against the manifest's hash before they're written; a mismatch installs nothing.
- Nothing a mod installs executes on the computer. WoW addons run inside the game's sandbox; that is
  the line. Real Palworld game mods would cross it, which is why section 5 leaves them open.
- The agent writes only inside the game folder's addon and data directories and its own state
  directory; it reads the game's files and nothing else.
- What leaves the computer is unchanged: the README's "What leaves your computer" is the contract,
  and a core or mod that sends anything new updates that section first.

## 11. Stages

0. **This note**, and the open questions below answered.
1. **The seam.** Split `agent/core` into the agent host and the WoW core behind the interface in
   section 4, manifests included, behaviour identical, tests green, one release. Nothing visible changes.
2. **Mod plumbing, and Chronicle becomes the first mod.** Manifest, bundled source, toggles in the
   agent and the panel, install/remove, the `actsInGame` wording. Chronicle, which is everything the addon records about a
   character (the record, bags and bank, quests, guild, journal events, census, kills and loot) plus the
   tooltips and the coming in-game window, moves out of the core addon into the first mod, on by default
   since it is data and touches nothing; the core keeps only the plumbing. Then the automation mod, quests first, then selling greys and the like.
   The automation mod exists (`mods/wow/automation`, v1, 2026-11-07): quests (accept, turn in,
   quest-only gossip, leaving low-level quests, an optional pick of the best-selling reward) and
   vendor (sell greys, repair, the guild bank first optionally); off by default, every chore its own
   toggle that starts off, Shift pauses it.
3. **Data layer v1.** Parse the three caches, upload them, pool them, write `Data/names.lua` and
   `Data/quests.lua` back down; the kills table gains names for creatures never targeted; the
   Chronicle window (0.2.x plan) reads community numbers from the same files.
4. **The Palworld core.** The Node agent as a sidecar, the identity change, the launchd job retired.
5. **Palworld mods**, per the reading Maddison picks.
6. **Git-fetched mods and a mods index**, once someone outside wants to write one.

## 12. Open questions (Maddison's)

- Settled 2026-11-07, Maddison: **the core is the plumbing only**, "the essential support for any mod
  we make": the saved file and its schema, the event dispatcher, the journal mechanism (append, dedupe,
  prune), settings and their defaults, the panel frame and its tabs, locales, the agent-status and data
  files, the slash command, error reporting, and the agent side's install discovery, addon management
  and sign-in. Everything recorded *about* a character is Chronicle: the record (identity, level, gold,
  played, gear, professions, reputations), bags and bank, quests, the guild roster, zones and deaths,
  the census sightings, kills and loot, the tooltips and the coming window. The stage 2 split follows
  this line.
- **CurseForge: yes, for the future**, and to be discussed before it happens: what exactly is listed
  (the core addon, Chronicle, every mod?), how a CurseForge installer learns they need the agent, and
  whether the agent simply takes over managing the addon when it finds a CurseForge copy.
- Windows signing (unchanged: Azure Artifact Signing is the recommendation).
- Settled 2026-11-07: the repository (A, `ryna_agent`); the Palworld agent goes MIT barring anything
  sensitive; Palworld mods are undefined for now; Chronicle is the first WoW mod.
