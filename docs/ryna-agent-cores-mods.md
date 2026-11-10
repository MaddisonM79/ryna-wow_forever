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
// mods/wow/prices/mod.json
{
  "id": "prices",
  "core": "wow",
  "name": "Prices",
  "version": "0.3.8",
  "summary": "Puts what a vendor pays for an item, and what the auction house asks, on every item tooltip, away from vendors too. Takes one snapshot of the auction house per visit (never anything about a seller). Nothing in chat, nothing done for you.",
  "kind": "addon",                                   // wow: "addon" (an addon folder) or "agent" (no folder: the agent's own work, the Settings mod); other cores: their kinds
  "actsInGame": false,                               // true for anything that does something for the player
  "source": { "kind": "bundled" },                   // or { "kind": "git", "repo": "https://github.com/…", "tag": "v0.1.0", "sha256": "…" }
  "requires": { "core": ">=0.3.8" },
  "defaultOn": false,
  "addons": ["Ryna_Prices"]                          // an addon mod's folders, installed beside Ryna
}
```

- **Seen in game.** Whatever a mod records, pools or does, it shows in the game, whole; the portal
  is the richer form of the same thing (section 7.5, the rule for every mod, old and new).
- **Off by default.** A mod that acts in the game for the player (`actsInGame: true`: automation,
  anything that presses a button) says so, and the toggle's text says so, every time.
- **Bundled or fetched.** A bundled mod ships inside the agent and versions with it. A fetched mod is
  pulled by the agent from a git source on an allow-list, by pinned tag, and its files are checked
  against the manifest's hash before anything is written. The person sees the source and the version.
- **The WoW kind.** A WoW mod is its own addon folder (`Ryna_Prices`, `## Dependencies: Ryna`),
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
startup and on `/reload`, and the agent writes files. The first such file was `AgentStatus.lua`
(the agent's status for the panel). The data layer generalises it: a versioned set of files the core
writes under `Interface/AddOns/Ryna/Data/`, one table each, each with a version, the addon reading
what's there and showing its age. The first two are the Prices mod's `Data/vendors.lua` and
`Data/auctions.lua` (7.3, "The way back down as built", 2026-11-09): one global `RynaData_<name>`
per file, `{ version, writtenAt, flavor, items }`, read by the core's `Ryna.Data.Get(name, version)`
and written by the agent's `prices_data` only when the rows changed.

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
- **What stays signed-in.** Everything about a person's own characters, which for the census means
  every character Ryna has recorded on this computer (every account's snapshot, and every character
  folder the game keeps under `WTF/Account`, in every install): a sighting of any of them, by GUID
  or by full name (case, spaces and a realm suffix aside), is left out of the uploads, and nothing
  is sent while an account's `Ryna.lua` can't be read. The census is the one thing
  the agent does before, or without, signing in; the Census tab says "contributes without an account"
  in plain words, with the same switches to stop any of it.

Stage (a) built (2026-11-07, agent side, the API's docs/wow.md 13.1, 13.3 and 13.5): the device key in the keychain (`ryna-agent` `identity`), the three signed headers on every API POST, `submissionId` in the heartbeat, the ID under Census in the WoW Settings and in `ryna status`, and `ryna census push [--dry-run]` / `ryna census id` (`census.rs`); the app sends no census uploads yet.
- **Stages.** (a) The device key and the signed census route, with sightings attributed to both account
  and ID for signed-in people, so the reputation data accrues before anyone relies on it. (b) Quarantine
  and weighting, with the Admin view of IDs. (c) The agent's "census only" mode: the Census tab and
  the core run with no sign-in.

### 7.2 Settings: backup and copy (asked 2026-11-08)

Maddison's ask, in her words: "settings is literal settings. i.e. backup my elv settings. i make a
new character on horde i want you to copy my settings from alliance to that character. i want no
chat messages." So: a **Settings mod** for the WoW core that backs up addon settings to ryna.io and
copies them between characters, with nothing said in game.

**What it works on.** Addon settings are the SavedVariables files the game writes: account-wide ones
in `WTF/Account/<account>/SavedVariables/<Addon>.lua` and per-character ones in
`WTF/Account/<account>/<Realm>/<Character>/SavedVariables/<Addon>.lua`. Which addon owns which file
comes from the addon's own TOC (`## SavedVariables:` and `## SavedVariablesPerCharacter:` lines), so
the mod detects every installed addon's settings without knowing the addon. The game's own
per-character files (`config-cache.wtf` CVars, `bindings-cache.wtf`, `macros-cache.txt`,
`chat-cache.txt`, `layout-local.txt`, `AddOns.txt`) are offered too, each by name, with one check
still owed in game: on the Modern engine bindings and macros are normally server-owned and those
files only caches, and whether Forever's server keeps them decides whether copying them does anything.

**Where it runs.** On the agent, not in the addon (`actsInGame: false`): the agent already finds every
install and account folder and parses SavedVariables Lua (`discovery.rs`, `lua.rs`); the new parts are
a Lua writer, the TOC scan, the realm/character folder walk and the AceDB adapter below. There is no
in-game side at all. Nothing is printed to chat, by this mod or (from 2026-11-08) by the core unless
the person turns the core's chat notices on.

**The one rule: the game must be closed.** WoW reads SavedVariables at login and rewrites them at
logout and `/reload`, so a file written under a logged-in character is lost at logout. The app
detects the game process and greys out every write ("close the game first"); reads (backups, the
diff view) are always safe. Ryna's own settings (`RynaDB`, account-wide, so already shared by every
character on one account) can instead ride the file the agent already writes into the addon folder
(`AgentStatus.lua`, read at login), which works while the game runs; other addons get no such path.

**Copy, two tiers.**

1. *File level.* A per-character file copies as it is to another character, another account or
   another install. Always works, and because the Lua is parsed the app can show which characters
   differ for an addon before copying.
2. *Addon aware.* The big addons (ElvUI, Details, WeakAuras, Questie, Plater, Bartender) use AceDB:
   one account-wide file where per-character state sits under `"Name - Realm"` keys in `profileKeys`
   and `char` (ElvUI's private settings in `ElvPrivateDB` the same way). Copying the file does nothing
   there; the agent edits the table and duplicates those keys for the target character. One generic
   AceDB adapter covers them, plus hand-written adapters for addons with their own layout; an addon
   the mod doesn't know falls back to tier 1 and says so.

Faction plays no part: "copy to my new Horde character" is a copy to another character. The few
addons that key data by faction (auction history) are left out of a copy on purpose.

**Backup.** Needs a sign-in (it is a person's own data, not census-style anonymous). The agent zips
the chosen files, uploads them to R2 under the person's `sub` through the API (`POST
/v1/wow/settings/backups`, a route section for `docs/wow.md`), keeps the last N, and restores through
the app with the same closed-game rule. The person's backups are theirs alone: other addons' files hold
other players' names and chat, so they never feed the census or any pooled table, and the app's copy
says so. A size cap (50 MB a backup, keep 10; Details and WeakAuras run to tens of megabytes).

**Encryption (decided 2026-11-08, Maddison: "option b"; the WoW app is open to the internet, not
friends and family, so this has to hold for strangers).** Server-side envelope encryption per person,
tied to the account:

- One master sealing key, `WOW_BACKUP_KEY`, in the `api (lmam)` 1Password Environment, on the API's
  never-regenerate list beside `FF_CRED_KEY` and the others (a new one makes every backup unreadable).
- Each backup gets its own random 256-bit data key. The API wraps it (AES-256-GCM) under a key derived
  with HKDF-SHA256 from the master and the person's `sub`, and stores the wrapped key, its nonce and a
  key version beside the object, so a rotation re-wraps data keys without touching the objects.
- The bulk work is the agent's, not the Worker's: the agent asks the API for a fresh data key (it
  comes back plain over TLS with its wrapped form), encrypts the zip with AES-256-GCM locally, and
  uploads ciphertext plus the wrapped key. Restore is the reverse: the signed-in agent asks the API to
  unwrap, downloads the ciphertext and decrypts on the machine. The Worker never holds a plaintext
  backup, and a 50 MB zip never passes through its memory.
- Backups are opt-in, and one plain sentence in the app's copy covers the limit ("stored encrypted
  for you on ryna.io"): ryna.io holds the master key, so it is not end to end. Maddison: "its opt in,
  im not worried about it." A passphrase mode (the person's own key, unrecoverable if lost) can be a
  further opt-in later if anyone asks.
- Not the device key: the Ed25519 identity key is for signing, and is per device, so it couldn't
  restore on a new machine anyway.

**UI.** In the agent, under the WoW core: a Settings section listing addons with their scope
(account, per character), size and last-written time; per addon the characters that have settings for
it; "Copy from <character>" on a character, "Back up now", "Restore", a history. Each copy and
restore writes a `.bak` of what it replaced beside the file, kept until the next one.

Settled 2026-11-08, Maddison: **backup and copy are one mod**, encrypted as above. Still open: copies
between accounts in v1 or only between characters; cross-machine sync automatic ("restore here" on a
new machine, or on every sync) or manual only.

**Built (2026-11-08, agent side, branch `claude/settings-mod-b9bec9`).** v1 defaults: copies between
characters on one game account only, restore on a click, never automatic.

- The mod: `mods/wow/settings/mod.json`, `kind: "agent"` (no addon folder; install and remove do
  nothing; the toggle shows the app's *Addon settings* tab and allows backups), off by default.
  `core.json`'s `data.reads`/`writes` list the SavedVariables and the game's files it touches.
- `cores/wow/core/src/settings/`: `inventory` (TOC scan with suffixed TOCs, account and
  realm/character walk, sizes and times, the characters each addon has settings for, the game's
  six per-character files as their own group, `Ryna.lua` never), `acedb` (the layout, copying one
  character's `profileKeys`/`char`/own profile and namespaces to another; ElvUI's `ElvPrivateDB` by
  the same code; `realm`/`faction`/`class` sections left alone), `copy` (plan = dry run, apply with a
  `.ryna.bak` per replaced file, the diff by content), `backup` (zip with `manifest.json`,
  AES-256-GCM under the API's data key with the id as AAD, parts of `partBytes.max`, signed PUTs,
  complete; restore checks size, sha256, decryption, every file's sha256 and every path before it
  writes anything). `lua.rs` gained a lossless tree (`parse_bytes`) and `lua_write.rs` the writer.
  `game_process.rs` (moved from the app) is the closed-game check; the CLI's `--force` is the only
  way past it.
- One deviation from docs/wow.md 15, told to the API side: manifest paths are WTF-relative with the
  account folder as its key (`Account/<16 hex>/…`), not its name, so the name never leaves the
  computer (the same rule as the snapshot's `account.key`).
- `ryna settings list|diff|copy|backup|backups|restore|delete`; the app's *Addon settings* tab.
- After review (same day): the game check fails closed (can't tell counts as running) and is asked
  again right before the first write; every write goes through one path (`settings::write_all`:
  plain path components only, no links, equal files left alone, every `.ryna.bak` first, rolled back
  on a failed write); a restore goes only into the backup's own account (the CLI's `--account …
  --force` is the one way past); `Name-Realm` parts are plain folder names and a copy's target must
  be a character the account has; AceDB copies touch only databases that hold the source, and make
  the target's entries what the source's are (removed where the source has none); a lost
  `complete` answer is checked with `GET /backups/:id`, and an abandoned upload is deleted; the data
  key is zeroized after use. Process names confirmed: macOS only (`_forever_/World of Warcraft.app`);
  the Windows executable names and the Wine command-line match are unconfirmed for Forever, and
  Windows also asks the install's own `.exe` files whether they're in use.
- Tested against fakes only: a fake game tree in a temp folder, a stateful fake of the section-15
  API (as a `Transport`, and behind a local HTTP server through the real transport). Left for the
  main session: the end-to-end run against the deployed API (create, parts, complete, list, open,
  content, delete), a real ElvUI copy in game, and the in-game check above on whether Forever's
  server keeps bindings and macros (which decides whether copying those files does anything).

### 7.3 Prices: vendor and auction house (asked 2026-11-08)

Maddison's ask: "a vendor prices mod on item hover? where would we get that data?", then "second
step is AH pricing too", and "i'd like them as one addon honestly". So **one mod, Prices**
(`mods/wow/prices`, addon `Ryna_Prices`; `actsInGame: false`, off by default like every mod), built in
steps because the data comes from three different places: the vendor line first, the auction line
added to the same addon later, each line its own toggle on the mod's tab.

**Step 1: the vendor line.** Two halves:

- *What a vendor pays you* is already on the machine: `C_Item.GetItemInfo(link)` returns `sellPrice`
  (its 11th value) from the item tables shipped with the client, away from any vendor (checked in game
  2026-11-08: the `GetItemInfo` global is nil on Forever, an unloaded item answers nothing at all rather
  than nil and a bare ID doesn't load it, so the mod asks with `C_Item.RequestLoadItemDataByID` and
  refreshes on `ITEM_DATA_LOAD_RESULT`; `C_Item.GetItemInfoInstant` works but has no price); the stock tooltip
  only shows it while a merchant window is open. The mod adds one line to every item tooltip through
  the core's tooltip hook (the `TooltipDataProcessor` path Chronicle's lines use): "Sells for 13s 50c"
  and, for a stack, "(2g 70s for 20)". Nothing to fetch. An item the client hasn't loaded is asked
  for and the tooltip refreshed once it arrives (as built, below). Settings: the line on
  every tooltip or only outside merchant windows (default: every tooltip), the stack total (default
  on). One probe still owed in game: `/dump select(11, C_Item.GetItemInfo(C_Container.GetContainerItemLink(0, 1)))`
  should print the first bag item's sell price; if a loaded item answers 0 or nothing, the served table
  below is the source instead.
- *What vendors charge, and which vendor* is not in `GetItemInfo`. Chronicle records the merchant
  when the player opens one: a `merchant` journal event (`npc` GUID and name, `zone`, `subzone`, and
  `items: [ { id, price, currency?, stack, limited } ]`, from `GetMerchantNumItems` /
  `GetMerchantItemInfo` / `GetMerchantItemLink`). The API pools it per item (`wow_merchant_offers`,
  newest price per NPC and item; a contract section in docs/wow.md to come) and the data layer
  (section 7) brings `Data/vendors.lua` back down: "Sold by Innkeeper Gryshka, Orgrimmar, 1g 20s". The
  client's own `ItemSparse` table (id, name, quality, stack, list buy and sell price) is extracted by the
  gamedata scripts too, as the listed price and the sell price for items the client hasn't seen; the
  observed price wins where they differ, since Forever tunes prices against the retail tables (it is Blizzard's own game: docs/forever-client-api.md). Privacy: merchants are
  NPCs, items are items; nothing here is about a person, so merchant recording is on under Chronicle
  (Maddison's call to confirm).

**Step 2: the auction line**, in the same mod. The same pooling shape with a price
model on top: the house itself: checked in game 2026-11-08 (docs/forever-client-api.md, Auction house),
`C_AuctionHouse.ReplicateItems()` hands an addon the whole house in one pull (192,300 rows on
Stormwind that day, each with item id, count, buyout, bid, quality, level and **no owner fields**), so the
mod takes one snapshot per auction-house visit (never more than once per 15 minutes, the throttle ATT
honours), walks the rows over several frames, aggregates per item in the addon (listings, quantity, min
unit buyout, a few price buckets for a median) and sends that aggregate up as one `auction` journal event;
per-listing rows never leave the client; the API keeps per-item daily aggregates (min,
median, market value as the weighted mean of the cheapest 30%, number of listings and sellers) and the
data layer brings `Data/auctions.lua` down for the tooltip: "AH: 4g 20s each (median this week), 31
listed". Rules, since this is the one place item data touches people: **never record a seller's name**
(the event has no field for it; counts of distinct sellers are computed in the addon and sent as a
number), no per-character history of who posted what, and reading market data needs the app grant like
every other read. A full scan (the "Scan" button of the classic addons) is the player's own click,
never automatic, with the same hardware-event rule the client enforces for `/who`. TSM and Undermine do
this for retail; nobody does it for Forever, which is why it's worth the pooling.

Stages: (1) the Prices mod with the sell line, Lua only, plus Chronicle's `merchant` event (the API
stores an unknown event type as sent, so recording starts at once), one release; (2) the API pool and
`Data/vendors.lua`; (3) the auction line, after the in-game check of what the AH API gives. Open: the two defaults above; whether the AH line shows on every tooltip or only in
bags and at the AH.

**Stage 1 as built (2026-11-08, not yet run in game).** `mods/wow/prices` (`mod.json`: `kind: "addon"`,
`actsInGame: false`, bundled, `defaultOn: false`, `requires.core >=0.3.8`) and its addon `Ryna_Prices`,
carried by the app and the release zip like Automation's. The line is "Sells for 13s 50c (2g 70s for
20)" in the core's `MoneyText`, on every item tooltip (TooltipDataProcessor's item post call, else
`OnTooltipSetItem` on GameTooltip and ItemRefTooltip). The stack's count is the tooltip's own: the
getter that filled it (`GetBagItem`, `GetInventoryItem` for the bank, `GetLootItem`) or the slot
button's `count` (never for a link, `GetHyperlink`, and clamped to the item's stack size); with
neither (a chat link, a merchant's item) it is a full stack of a stackable item, and a slot holding
one shows the single price. The item is the post call's own data first (`data.id`,
`data.hyperlink`), since a tooltip that appends a second item still answers `GetItem` with the
first; a line added by hand is never added twice. A sell price of 0, or none, adds nothing; an
uncached item adds nothing and, when `ITEM_DATA_LOAD_RESULT` or `GET_ITEM_INFO_RECEIVED` names it
while the same tooltip still shows it, the tooltip is refreshed (`RefreshData`, else the line added
by hand). What the game said (Maddison's client, 2026-11-08), which the code follows: the global
`GetItemInfo` is nil on Forever, so only `C_Item.GetItemInfo` is asked (the documented 18 returns:
sellPrice 11th, itemStackCount 8th); an item the client hasn't loaded answers **nothing** (no values,
not a nil) and asking again doesn't load it, so an empty answer counts as "not loaded" and the mod
calls `C_Item.RequestLoadItemDataByID`; `C_Item.GetItemInfoInstant` has no price; and links start
`|cnIQ2:` rather than `|cffxxxxxx`, so ids are read from `Hitem:(%d+)`. Whether a loaded item's answer
carries a sell price is still being checked; an answer without one adds no line. Its Prices tab
(after Automation) has `priceSellLine` (on), `priceStackTotal` (on) and `priceOnlyAwayFromMerchant`
(off: the line shows at merchants too; on, it stays away while the merchant window is open, by the
merchant events or `MerchantFrame`); `/ryna prices` lists them and `/ryna prices <name> on|off`
changes one. It prints nothing unasked (an error caught inside its hook is the one exception: the
core's `ReportError` line, once) and writes nothing but its settings. The probe is still owed:
if `/dump select(11, C_Item.GetItemInfo(2589))` (Linen Cloth loaded first) answers 0 or nil away
from a vendor, the line simply never shows (every item reads as selling for nothing), and the sell price has to come from the served
table instead. (Maddison ran it in game the same day: the line works.)

**After a comparison with Ketho's VendorPrice (2026-11-08; Maddison's calls).** This supersedes the line's wording and counts above. The line reads like the game's
own: the client's `SELL_PRICE` label ("Sell Price: ...", so every client language reads it) and its coin
icons (`C_CurrencyInfo.GetCoinTextureString`, else the `GetCoinTextureString` global), falling back to
"Sells for" and `MoneyText`. The count comes from every getter that names one: bag, bank and loot
slots, `GetLootRollItem`, `GetQuestItem`, `GetQuestLogItem` (reward and choice), `GetInboxItem` (an
attachment), `GetSendMailItem`, both sides of a trade, `GetMerchantItem` (`C_MerchantFrame`, else
`GetMerchantItemInfo`) and `GetBuybackItem`; else the slot button's `count`. A chat link, or anything
else with no count, shows one item's price: the full-stack guess is gone. Where the game already drew
its own price on the tooltip (a money line, `shownMoneyFrames`), ours stays away, so a merchant window
never shows two; `priceOnlyAwayFromMerchant` (still off) now means no line at all at a merchant.
Comparison tooltips keep the line. The load events are registered only while a tooltip waits, on a
frame of Prices' own. Still owed in game: that Forever's merchant tooltip draws its price as a money
line (else the two prices stay), the getter names above (a missing one falls back to the button's count
or one item's price), and whether a recipe tooltip gets two lines (the recipe's and the product's).

**Step 2 as built (2026-11-08, not yet run in game).** `mods/wow/prices/addon/Ryna_Prices/Auction.lua`,
in the same addon, against the API's contract (docs/wow.md section 17, built in parallel and not yet
deployed; the agent forwards the unknown `auction` type as sent). The snapshot (`priceAuctionSnapshot`,
on): at `AUCTION_HOUSE_SHOW`, when this character's last request is 15 minutes old or more
(`RynaDB.prices.auctionAt[<player GUID>]`, so two characters on one account each have their own),
one `C_AuctionHouse.ReplicateItems()`; at `REPLICATE_ITEM_LIST_UPDATE`, `GetReplicateItemInfo(i)` for
`i` from 0 (the probe's index) to `GetNumReplicateItems() - 1`, 2,000 rows a frame through the core's
`After(0)`. Only the 3rd, 10th, 17th and 18th values are read (count, buyoutPrice, itemID,
hasAllInfo, by Blizzard's `AuctionHouseDocumentation.lua` on the `forever` branch); the bidder and
owner values (12th to 15th) are never put in a variable. Rows with no buyout are skipped and
counted; rows with `hasAllInfo` false (the item's name and link not cached yet) are counted and kept,
since their numbers are there and a cold cache would otherwise make a partial snapshot that the API
pools as the real one (review, 2026-11-08). Each listing is one number, `unit * 65536 + count` (the unit price is
`buyoutPrice / count` rounded to copper, at least 1), so sorting an item's numbers sorts by price and
192k listings are about 5 MB live in Lua 5.1 (measured with a fake house of 192,300 rows over 15,000
items: 193 frames, under half a second of Lua in all under an emulated 5.1, the heaviest frame the
last one, which writes the store and the events). Once all rows are read, the items are summarised
a slice a frame and each item's list dropped: `n`, `q`, `min`, and `q25` and `med` as the unit price
at which a quarter and a half of the quantity is reached (so `min <= q25 <= med`). The house
closing while rows are read, or a logout at any point, stops the walk and keeps nothing; the house
closing also forgets the throttle time, so the next visit asks again (a logout keeps it). Closing it
once the rows are all read doesn't stop it (the numbers are in hand). Then, in one frame: the store
(`RynaDB.prices.auction[itemId] = { min, q25, med, n, q, at }`, replaced per item; pruned to 14 days
at login, at most 20,000 items, the most recently seen kept), `RynaDB.prices.auctionLast` (`at`,
`rows`, `items` and the skipped counts, for `/ryna prices`), and the `auction` events: `{ at, rows,
part, parts, items }` with items `{ id, n, q, min, q25, med }` in id order, 2,000 an event; an item
outside the API's limits (an id, count or price it would refuse) stays in the store and is left out
of the event, so a snapshot is never refused whole; past 20 parts the most listed items are sent. A
browse search (`AUCTION_HOUSE_BROWSE_RESULTS_UPDATED`, and `_ADDED`) folds `minPrice` and
`totalQuantity` in per item id for results that are the item itself (no random suffix; gear only at
its own item level, by `C_Item.GetItemInfoInstant`'s equip slot and `GetItemInfo`'s level; battle
pets skipped): a new entry `{ min, q, at }`, or, over a snapshot entry, only the new `min` and `at`
(its median and counts stay; nothing within 15 minutes of the snapshot). It is never sent. The
tooltip line (`priceAuctionLine`, on; `priceAuctionStackTotal`, on) is in Tooltip.lua's hook beside
the sell line, with the same guards and the same count: "AH: 4g 20s each (84g for 20) · 31 listed ·
2 hours ago" (the core's `Ago`), "AH: from 10c each · 13754 for sale · just now" for a browse-only
entry, nothing without an entry, and nothing while `AuctionHouseFrame` is shown and either the
tooltip belongs to one of its own lists or its browse tab (`BrowseResultsFrame`) lists the item.
`/ryna prices` adds a line: the auction setting summary and the last snapshot's age, rows and items.
Nothing is printed unasked. Tests: `mods/wow/prices/test/auction_test.lua`. Owed in game: that
`REPLICATE_ITEM_LIST_UPDATE` fires (on a client that refuses the registration the rows are read a
minute after asking, which is a guess at when they are ready), the frame cost on 192k rows, that
`minPrice` is per unit for stackable goods, that a search fires `AUCTION_HOUSE_BROWSE_RESULTS_UPDATED`,
and whether a second `ReplicateItems()` inside 15 minutes is refused (the throttle is ours either way).
One thing for Maddison: the store is one table per account, so a Horde and an Alliance character
(separate houses on a Classic ruleset) overwrite each other's numbers, newest wins. The journal's
size, settled with the API's pool (a snapshot is pooled once, from the first upload that carries it,
every part in that upload; the API keeps every event and takes at most 16 MB an upload; it checks
`n <= q` too) and a review the same day: an `auction` item is about 140 bytes as the client writes
SavedVariables, so before adding a snapshot `A.MakeRoom` keeps at most 3 earlier snapshots and 6,000
auction items in all, oldest dropped first (a bigger house sends its 6,000 most listed items), so the
journal stays well inside the upload limit however many alts visit between logins. At login
`Journal.Prune` drops `auction` events once the companion app has pushed them for this account, and
after 3 days in any case (`J.SHORT_LIVED`). "This account" is `RynaDB.meta.accountId`, 16 random hex
digits the core mints once per RynaDB; the agent reads it from the file it pushes (never sending it),
keeps it with `journalThrough` (the newest event `at` that push carried) in its state, and writes both
into `AgentStatus.lua`'s account entries, so one account's push never drops another's unpushed
events; an agent or core without the id drops nothing early. A core without the trimming
(`Ryna.Journal.ShortLived` absent) gets no `auction` events at all, and `/ryna prices` says so. All of
a snapshot's parts are added in one frame, so they are always in the same save. Tests:
`mods/wow/prices/test/auction_test.lua`, `journal_prune_test.lua`, and the agent's
`snapshot`/`runner`/`agent_status` tests.

**The way back down as built (2026-11-09, not yet run in game).** The API gives the agent one read,
`GET /v1/wow/agent/prices?items=<≤200 ids>&flavor=` (api docs/wow.md 17.2; the agent token's one
pooled read, under its existing `wow:ingest`, so nobody signs in again): per item the best three
sellers (`npcId, name, zone, price, stack, currency, observers, lastSeen`) and the auction pool's
`today` and `week`. After each run, for every install that has the core addon, the agent
(`cores/wow/core/src/prices_data.rs`) takes the item ids of the account files it parsed and pushed
(`snapshot::item_ids`: what the characters hold, bags, bank and keyring, then what they have looted,
then what the Prices mod's own auction store has seen; each id once, most useful first; at most
4,000 an install), asks the API a page of 200 at a time, and writes `Interface/AddOns/Ryna/Data/vendors.lua`
and `Data/auctions.lua`: one plain global each (`RynaData_vendors`, `RynaData_auctions`; the
SavedVariables form the agent's own reader reads back), `{ version = 1, writtenAt, flavor, items }`,
vendors as up to three rows `{ npcId, name, zone, price, stack, observers, currency }` per item
(the API's order: confirmed first, then copper-only, then cheapest per piece) and auctions as one row `{ min, med, listings,
snapshots, observers, asOf, weekMed, weekDays, weekSnapshots }` per item (0 for none). Names are
escaped and stripped of `|` and control characters whatever arrived. Each file is budgeted at 256 KB:
over it, the vendor file first drops every item's third seller, then its second, then items from the
end of the id order (the auction store's ids are dropped first, then loot, then what's held); the
auction file drops items from the end. Written through a temp file and a rename, and only when the rows
changed (`writtenAt` alone doesn't count), so a quiet sync writes nothing; a reinstall of the addon
carries `Data/` over the release's stubs. When no account file changed, the tables are refreshed
anyway every 6 hours, since the pools move as others play; either way every account under the
install counts (the ones not pushed this run are read for their ids), and an account file that can't
be read skips the install rather than rewriting its tables without that account's items. A failed
read, call or write is reported in the run's `prices_data`, never fails the run, and puts the
install off for 30 minutes; none is attempted while the sign-in is gone or the API is unreachable.
The tables' `writtenAt` is when their rows last changed, which is what `/ryna prices` shows. The core addon loads the two
files from its TOC (shipped as stubs, the global `nil`) and hands them out through `Ryna.Data.Get(name,
version)` (`Data.lua`): the table only when it carries the version asked for, else nil and "absent"
or "version". `Ryna_Prices` reads both at login (`Pooled.lua`) and adds two lines in `Tooltip.lua`:
`priceVendorLine` (on), "Sold by Kiro, Durotar: 20c (+2 more)", the best known seller (confirmed first,
then copper-only, then cheapest, as the API orders them) and how many more, "20c for 5" for a bundle, "· unconfirmed" when one person alone saw the price, "· + other
costs" when it also costs something besides copper; and `pricePooledAuctionLine` (on), "AH (everyone):
1s 10c each (22s for 20) · 3 listed · as of 2 hours ago" (today's median, else the agreed min; the
mean listings per snapshot; the age of that day's newest snapshot, which is the data's time, not the
file's; "· unconfirmed" for one observer) or "AH (everyone): 1s 10c each this week · 5 snapshots" where
only older days this week have data, in its own light-blue colour under this account's own white "AH:"
line so the two never read as one, and away wherever the own line is (the auction house showing the
item). A table from another version is ignored whole and `/ryna prices` says so ("pooled prices from
another version: update the companion app and the addon"); absent tables read "no pooled prices yet:
sync the companion app, then /reload"; a core without `Ryna.Data` reads "too old". What shows is as
fresh as the last login or `/reload`, which the mod's summary in the app says. Tests:
`mods/wow/prices/test/pooled_test.lua` (over the core's real `Data.lua`), and the agent's
`prices_data`, `runner`, `snapshot` and `addon_install` tests. Owed in game: that the TOC loads
`Data\vendors.lua` from a subfolder as it does `Locales\`, the look of the light-blue line, and whether
"Sold by" should stay off merchant tooltips (the vendor in front of you is the answer there).

Chronicle's `merchant` event, as built (`Merchants.lua`, setting `merchants`, on, on the Recording
tab):

```jsonc
{ "type": "merchant", "npc": "Creature-0-…", "name": "Innkeeper Gryshka",
  "zone": "Orgrimmar", "subzone": "Valley of Strength", "filter": 1,
  "items": [
    { "id": 4540, "price": 25, "stack": 1, "limited": false },
    { "currencyId": 1901, "price": 0, "stack": 50, "limited": false },
    { "id": 18839, "price": 900, "stack": 1, "limited": false,
      "currency": [ { "amount": 75, "name": "Honor Points", "currencyId": 1901 },
                    { "amount": 2, "name": "Alterac Valley Mark of Honor", "itemId": 20560 } ] } ] }
```

`price` is copper for `stack` of the item (the bundle sold at once); `limited` is `numAvailable >= 0`;
`currency` is there only when `GetMerchantItemCostInfo` says the item costs something besides copper;
a slot that sells a currency (a currency link, or C_MerchantFrame's `currencyID`) is `{ currencyId,
price, stack, limited }`. `filter` is `GetMerchantFilter()`: the game opens the window filtered to the
player's class (`ResetSetMerchantFilter`), so the list is the filtered view, and 1
(`LE_LOOT_FILTER_ALL`) is the whole one. A list counts once it's complete (every slot has an item or
currency link). It is written at `MERCHANT_SHOW` when it is, else when `GET_ITEM_INFO_RECEIVED` (what
the game's own merchant frame waits on) or `MERCHANT_UPDATE` completes it; a visit that never
completes writes its last reading at `MERCHANT_CLOSED` with `"partial": true`, rather than losing it.
One event per visit and filter, at most 200 items; the NPC is done for the login only when its
complete, unfiltered list was written, so a partial or filtered list leaves it to the next visit. So
the journal doesn't take a list per visit forever, `RynaDB.chronicle.merchants` keeps each complete
list's fingerprint (`{ hash, at }`, a string hash over the sorted ids, prices and stacks and the
filter; keyed by GUID, or `GUID#filter` for a filtered list): an unchanged list written in the last 7
days isn't written again, a changed price or list is, and entries older than 30 days are pruned at
login. The reads prefer `C_MerchantFrame.GetItemInfo`'s named fields (a table on the Modern engine;
which of the two the client has is asked at run time, not assumed) and fall back to the 3rd to 5th
values of `GetMerchantItemInfo` (price, stack, available), which classic and modern clients agree on,
since Forever's return orders have differed from the wiki's before. The agent passes it through as it
does every unknown event type; docs/wow.md's row comes with the pool.

### 7.4 Chronicle's journal: what exists, against what you've done (asked 2026-11-09)

Maddison, after looking at what she runs beside Ryna (AllTheThings, 71 MB; Doud's Dungeon Journal,
35 MB; Questie and its database, 211 MB): "Journal and ATT, there is a HUGE amount of overlap there.
Those are both heavier lifts but one tool for it", then the point that decides the design: "ATT and
Questie both have something we don't then: what isn't done." And: "an enhanced UI in game so as more
items are added we have one window to review things." Where it is heading, in her words: "like
an ATT but our style and portaled."

**The gap.** Three kinds of knowledge make a journal, and Ryna has two:

1. *What this character has done*: Chronicle's record (kills, loot and where it came from, completed
   quests, known recipes, reputations, bags and bank).
2. *What anyone has seen*: the pools (drop rates, kill counts, merchants, auctions).
3. *What exists*: the whole set, which "missing" is measured against. This is what ATT and Questie
   ship as hand-built Lua, and what Ryna doesn't have.

Chronicle's journal is the third, joined to the first two. It isn't hand-built: each set comes from the
client's own tables where they hold it, and from play, pooled, where they don't.

**What exists, by source** (checked 2026-11-09 against build 1.60.1.70790 through the same
wago.tools route `scripts/wow-gamedata` uses, and against Blizzard's `forever` UI branch):

| Set | Source | Whole? | The record today |
|---|---|---|---|
| Recipes | `SkillLineAbility`, 2,775 rows in the profession lines (Leatherworking 616, Blacksmithing 527, Tailoring 486, Engineering 295, Enchanting 273, Alchemy 212, Cooking 141, First Aid 40, …) | Yes | Chronicle: `professions[].recipes` (ids from `C_TradeSkillUI.GetAllRecipeIDs`; check they are the table's `Spell` before joining) |
| Reputations | `Faction`: 75 of its 253 rows have a reputation | Yes | Chronicle: `reputations` (what the pane lists) |
| Mounts | `Mount`, 144 rows, with the spell each teaches; the source columns are empty | The list, not where from | Nothing yet (below) |
| Companion pets | `BattlePetSpecies`, 121 rows; sources empty | The list, not where from | Nothing yet (below) |
| Flight points | `TaxiNodes`, 101 rows | Yes | Nothing; readable only with the flight map open |
| Dungeons and bosses | `DungeonEncounter`, 342 encounters on 42 maps (served already as `encounters`); Forever has **no** `JournalEncounter`, `JournalInstance`, `JournalEncounterItem`, `JournalEncounterSection` or `JournalEncounterCreature` (wago.tools serves all five for retail and none for this build) | The boss list only: no loot, no abilities, no boss-to-creature link | Chronicle: `kills` by creature id; the agent's fights by encounter id |
| Boss loot | Nothing in the client | No: pooled from play | Chronicle: `loot[*].from["c:<id>"]`, pooled as `/chronicle/creatures/:npcId` |
| Boss abilities | Nothing in the client; cast info of other units is secret to addons (docs/forever-client-api.md) | No: pooled from play | The agent's combat-log digests already count casts per actor and spell |
| Quests | `QuestV2`: 6,616 ids and nothing else (no title, zone or objective); `QuestSort` 39 category names; `QuestLine` 3 lines over 22 quests | The ids only | Chronicle: `quests.completed` |
| Achievements | `Achievement`: 440 rows, 14 with points, most of them statistics; Blizzard's achievement window is not allowed on this game type | Not a set here | Out of scope |

Three consequences:

- **Collections are whole on day one.** Recipes and reputations are a subtraction against what
  Chronicle already records. Where a missing recipe comes from is not in the table, but two pools
  already answer most of it: the merchant pool (recipes vendors sell) and Chronicle's loot (recipes
  that drop); `AcquireMethod` marks the ones that come with the skill itself.
- **Mounts and pets have journals** (checked in game 2026-11-09, docs/forever-client-api.md),
  though Blizzard's Collections window is not allowed to load on Forever. Mounts: the journal lists
  136 and says which are collected (`C_MountJournal.GetMountIDs`, `GetMountInfoByID`'s 11th value),
  so both the set and the record come from the game. Pets: `C_PetJournal.GetNumPets()` answers
  listed and owned, and lists owned pets only by default, so the record is the journal and the set
  is the `BattlePetSpecies` table (121). Chronicle records both; no guessing from bag items.
- **Dungeons and quests are grown, not shipped.** Doud's Journal carries 35 MB because the client has
  no journal data to read. Ryna's comes from everyone's play: loot per boss from Chronicle's tallies,
  abilities and the boss's creature ids from the agent's fights (the digest names the actors inside
  an `ENCOUNTER_START`/`END`), and quest titles, zones and objectives from the client's
  `questcache.wdb` (section 7's first source, stage 3). The quest count is honest from the start:
  6,616 ids exist, and the journal can say how many of them nobody has named yet. This side is thin
  while few people play and gets better without a release.

What Ryna will not have: Questie's map pins and objective locations, and ATT's hand-curated
"how to get it" notes. The journal says where something came from when a pool has seen it, and
"not seen yet" otherwise. Questie stays installed for the map.

**One mod, under the Chronicle name** (Maddison, 2026-11-09: "I feel like one mod, using the
chronicle name"; a mod of its own was the recommendation, and she chose this). There is no `journal`
mod. Chronicle grows a journal side:

- *Recording* is Chronicle as it is, on by default, touching nothing. It gains two records: mounts
  and pets owned.
- *The journal* is three sections of Chronicle's window, **Collections**, **Dungeons** and
  **Quests**, each a Chronicle setting (`journalCollections`, `journalDungeons`, `journalQuests`)
  that starts **off**. Off, a section isn't built and its data isn't loaded; on, it reads the record
  Chronicle already keeps, so a boss killed is marked the moment it is counted.
- So installing Chronicle still means what it meant: it records. The journal is something a person
  turns on, section by section, in the Recording tab or from the notice below.

**The first-dungeon notice** (Maddison: off, "but can we alert them at the first dungeon like hey,
this is here"). The first time a character enters a dungeon or raid with the Dungeons section off,
Chronicle shows one small notice frame, never a chat line: "Chronicle can show this dungeon's
bosses, what they drop and what you've already seen. Turn it on?" with **Turn on** and **Not now**.
Rules: once per account, whichever button is pressed (`journalNoticeSeen`); never in combat (it
waits for the first moment out of it); nothing if the section is already on; and it ships with the
Dungeons section, not before, so it never points at an empty page. It is the one thing Chronicle
says unasked, and the section's own setting is where to find it afterwards.

**Quests: tracking in, the map out** (Maddison: "tracking it should go in"). The Quests section is
quests by zone, done or not, and how many in a zone remain, measured against the pooled quest
table. Where to go for an objective, and pins on the map, stay Questie's.

**In game it is whole; the portal is the primary path** (Maddison, 2026-11-09: "the portal is the
primary path but no one is going to use an addon that they cannot visualize in game"). Two rules
follow, and the rest of this section is held to them:

- *The window is a complete journal, not a pointer to a website.* Every set is browsable in game in
  full: every dungeon and boss, every recipe, mount, pet and faction, every zone's quests, with
  icons, the game's quality colours, the item's own tooltip on hover, a progress bar per section
  and per group ("Deadmines 3 of 6", "Tailoring 41 of 486"), and search. Nothing in it says "see
  ryna.io". The data budget above is sized for the whole sets for this reason.
- *All of a person's characters, in game too.* The saved file already holds every character
  Chronicle has recorded on the account, so the window can show another character's progress, or
  "any of mine has it", without the portal.

What the portal adds is what a game window can't do well: every set served by the API, history and
charts over time, the same view across accounts and for the people a profile is shared with, and
room to browse. It is where most looking will happen; the addon is why anyone installs.

**One window.** Chronicle had the window (`RynaChronicleWindow`: Session, Kills, Loot, Levels, a
search box, and lists that fill only the rows on screen, so 2,000 creatures cost what 15 do). It
moved into the core as `Ryna.Window`, beside `Ryna.Panel` (built 2026-11-09):

- One frame. A rail of sections down the left, one header (title, character picker, search box),
  Escape closes.
- A mod registers a section: `Ryna.Window.AddSection({ key, group, label, order, characters, columns,
  rows = function(ctx) … end, fill, tiles, tooltip, click, footer, version })`, where `ctx` carries the
  chosen character (or all) and the search text. The core owns the frame, the virtual list, sorting,
  the picker, the search box and the row tooltip; a mod supplies rows. As built (2026-11-09): a section
  is one list (no tabs inside a section); the spec is at the top of `cores/wow/addon/Ryna/Window.lua`
  and in `cores/wow/addon/README.md`, "Ryna's window".
- Chronicle registers its four tabs as one section, unchanged, and its journal sections
  ("Collections", "Dungeons", later "Quests") when their settings are on. Prices can add "Auctions"
  over its own snapshot. The panel (`/ryna`) stays for settings; the window is for data.
- `/chronicle` and `/ryna chronicle` keep working and open the window on Chronicle's section. A
  section's rows are built when it is first shown and rebuilt only when its data's version changes,
  as the Chronicle views are today.

The layout, from the mockup Maddison approved on 2026-11-09 ("Your mock is good. in game will be
different but lets see it"; the mockup fixed layout and content, the game's own look decides
colours, fonts and icons):

- *The rail*, in three groups: **Me** (Session, Characters, Items, History), **World** (Census,
  Dungeons, Collections, Quests), **Combat and gold** (Fights, Auctions, Chores). Only sections
  that exist are listed; nothing greyed out or "coming soon".
- *The header*, the same on every section: the section's title, a character picker ("All
  characters" and each character on the account), and the search box.
- *The body*: summary tiles where a section has headline numbers, then rows; a row may carry an
  icon, a name in the game's quality colour, a progress bar and a right-aligned number, and shows
  the game's own tooltip on hover.
- *The footer*, on every section: where the numbers came from and how old they are ("From your
  saved file", "Global contributions as of 2 hours ago").

First build: the window in the core, Chronicle's Session, Kills and Loot and its levels (as
History) moved onto it, and the two sections that need only the saved file, **Characters** and
**Items** (with the tooltip line that says which character holds an item). The Chronicle window's
tests move with it.

**Data files and their cost.** AllTheThings and Questie cost what they do because the client parses
their tables at every login. The journal's tables are small because they hold ids and numbers, not
prose, and they are loaded late:

- The agent writes them as a load-on-demand addon folder beside the others (`Ryna_ChronicleData`:
  a TOC with `## LoadOnDemand: 1` and one Lua file per table), and Chronicle loads it the first
  time a journal section is shown (`C_AddOns.LoadAddOn`). Nothing is parsed at login. Checked in game
  2026-11-09: a load-on-demand addon folder loads on that call (`ElvUI_Options`).
- Budget: 2 MB for everything, with the quest table (the largest, perhaps 800 KB at 6,616 quests)
  optional. Recipes are about 110 KB, the boss list and pooled loot (at most 40 items a boss) under
  300 KB, mounts, pets and factions a few KB.
- Each table carries its version and written-at time, like every data-layer file, and the window
  shows "as of". The agent refreshes by version on sync.

**The API's side**, to be written into docs/wow.md before it's built: new served game tables from
`scripts/wow-gamedata` (`recipes`, `factions`, `mounts`, `pets`, the item-to-spell links, and
`DungeonEncounter` as it is); two new pools, both aggregates about no one (creatures and spell casts
per encounter, from the fights the agent already uploads; loot per encounter, from Chronicle's
tallies joined through those creatures); the quest pool with stage 3; and one agent read that hands
the agent each table by version, the shape the Prices mod's agent read sets. The portal gets the
same lists and "missing" views from the same routes, which is the web half of this.

**Privacy and manners.** Every table here is about the game: items, spells, NPCs, bosses, quests.
The pools are counts. Nothing about a person is read, pooled or shown, the journal sends nothing new,
prints nothing in chat, and does nothing for the player.

**Stages.**

- **J0. The window into the core.** `Ryna.Window`, Chronicle's tabs moved onto it. One release.
- **J1. Collections.** The game tables, the agent read and the data folder; Chronicle's mount and
  pet records and its Collections section: recipes by profession (known, missing, and where a pool has seen the
  missing ones), reputations, then mounts and pets once the probe says how they are owned.
- **J2. Dungeons.** The two pools; the first-dungeon notice; a Dungeons section: instances, bosses (killed or not), each
  boss's loot with its pooled rate and whether this character has looted it, and its abilities.
- **J3. Quests.** With data layer v1 (section 11, stage 3): quests by zone, done or not, and the
  count nobody has named yet.

**Answered in game 2026-11-09** (Maddison; details in docs/forever-client-api.md): the mount and
pet journals exist and tell collected from not; load on demand works; and the game's own encounter
journal is loadable but empty (`EJ_GetNumTiers()` is 0), so boss loot and abilities are pooled from
play, as planned above.

**Still owed in game:** in a dungeon, after a boss dies, does Chronicle's kills tab count the
boss's creature id (it does for open-world kills), and does the app show the fight under its
encounter name?

**Settled 2026-11-09, Maddison:** one mod under the Chronicle name; the journal sections off by
default, with the first-dungeon notice; quest tracking in. **Open, hers:** whether flight points
are worth a Chronicle record; whether the notice's wording above is the one she wants.

### 7.5 Seen in game, richer on the portal (Maddison, 2026-11-09)

The rule, for everything Ryna has built and everything it builds: **the game has to visualize what
we are doing; the portal adds richness.** Her words: "the portal is the primary path but no one is
going to use an addon that they cannot visualize in game", and, on what went wrong: "in game there
is no census information to view. Journal was go look at the web, why?"

What it means in practice:

- **Every feature that records, pools or does something has a place in the game where a person
  sees it**: a section or tab of the window (7.4's `Ryna.Window`), a tooltip line, or the strip.
  A settings checkbox is not a view.
- **In game is whole, not a teaser.** The view answers the question the feature exists for, on its
  own. The addon never says "see ryna.io".
- **The portal is the more robust form of the same thing**: history and charts over time, every
  account and shared profile, bigger tables, links. It never holds a feature the game can't show.
- **The test for "done"**: a feature is finished when its in-game view exists. API and portal work
  without it is half a feature. New designs in this document name their in-game view first.
- **What reaches the game is still bound by the privacy rules.** Pooled data arrives as aggregates
  through the data layer; nothing about another player is looked up or listed in game (the census's
  per-character lookup stays on the API, where every lookup is audited).

Honest limits, which are exceptions and not excuses: the game loads the agent's files only at login
and `/reload`, so pooled numbers in game are "as of" a time, shown; the Settings mod's copy runs
with the game closed, so its in-game part is a status line; Admin is the portal's alone.

**Where each feature stands** (audited 2026-11-09 against the addon on main and the portal's WoW
pages):

| Feature | In game today | Portal today | Owed in game |
|---|---|---|---|
| Session numbers | Strip; Session section | none | Nothing |
| Kills and loot, this character | Kills and Loot sections, any character on the account by the picker; tooltip lines | Character, Chronicle | Nothing (an account-wide total is a later choice) |
| Pooled drop rates and kill counts | Nothing | Chronicle, Items | Rates on the Loot and Kills rows and on tooltips, from the data layer |
| The character record: level, gold, played, gear, professions, reputations | **Characters** section (built 2026-11-09): every character at a glance, each one's detail | Characters, Character | Nothing |
| Bags and bank | **Items** section (built 2026-11-09) across every character, and the tooltip line ("Maddiepally: 12 in the bank") | Bags, Items search | Nothing |
| Quests | Nothing | Quests | 7.4's Quests section; the completed count per character meanwhile |
| History: levels, deaths, zones, sessions | **History** section, levels only | History, Trends | A **History** tab: deaths, zones, sessions by day, with the week's totals |
| Census | Four checkboxes | Population | A **Census** section in two halves, Maddison's words for them: **Your contributions** (what this account has seen and sent: counts by faction, class and level band, never names) and **Global contributions** (the realm's pooled picture, from a data file) |
| Guild roster | Nothing (the game's own guild pane) | Guilds | With the guild stage |
| Fights | Nothing | Logs, Fight | A **Fights** section in the manner of Details (below) |
| Prices | Tooltip lines; `/ryna prices` | **Nothing** | An **Auctions** section over the snapshot (search, low and median, listed, age) and who sells what; and the portal owes Prices a page |
| Automation | A status line (the chat report is off) | none | What it did this session (sold, repaired, accepted, trained), as a list in the window |
| Settings backups | Nothing | none (the agent's alone) | A status line: the last backup and its age |
| Agent and sync state | Status tab | Agent | Nothing |
| Collections, dungeons, quests | Not built | Not built | 7.4 |

Two things the audit shows. The window today is one character's kills, loot and levels, while the
saved file already holds every character on the account: the Characters and Items sections need no
API work at all, only the window. And two features have no portal page either (Prices, and the
journal sets), so the rule cuts both ways.

**One suite** (Maddison, 2026-11-09: "we are going to unify MULTIPLE tools into one suite"). The
addons people stack today each do one of these jobs with their own window, settings and saved
file. Ryna does them as mods of one core, with one window, one settings panel, one saved file and
one agent behind it:

| The job | What people install for it | In Ryna |
|---|---|---|
| Quest and vendor chores | AutoQuests and its kind | Automation |
| Vendor and auction prices | BetterVendorPrice, Auctionator's price lines | Prices |
| Collections and what's missing | AllTheThings | Chronicle's journal (7.4) |
| Dungeon journal | Doud's Dungeon Journal | Chronicle's journal (7.4) |
| Alts, their bags and bank | Altoholic, BagSync | Chronicle: Characters and Items |
| Who is playing | CensusPlus | Chronicle: Census |
| Damage and healing meters | Details | Fights |
| Logs and their analysis | Warcraft Logs and its uploader | The agent, and the portal's Logs |
| Settings backup and copy | Hand-copying WTF folders | The Settings mod |

Left alone on purpose: the UI replacement (ElvUI), the quest map (Questie), gear scoring (Pawn).

**Fights, in the manner of Details** ("we are reading combat logs... so don't be surprised when I
ask to see something like Details from us... imagine one tool for logs and dps"). The combat log
event is closed to addons on Forever, but the game's own meter is not: `C_DamageMeter` is
documented for every environment on the `forever` branch (`DamageMeterDocumentation.lua`) and
`Blizzard_DamageMeter` is allowed on Forever's game type. It gives the sessions the game kept
(`GetAvailableCombatSessions`: id, name, length), and for each one every source's total and
per-second amount (`GetCombatSessionFromID`) and a source's spells (`GetCombatSessionSourceFromID`),
for damage, healing, absorbs, interrupts, dispels, damage taken and deaths, with
`DAMAGE_METER_COMBAT_SESSION_UPDATED` when one changes. The session reads are marked
`SecretWhenInCombat`: during a fight the numbers are secret values (they can be shown, not compared
or summed), and after it they are plain.

So the split is: *during* the fight the live bars are the game's own meter, which Ryna doesn't
redraw; *the moment it ends*, the Fights section has it: the fight list for the session, each
fight's players ranked with bars, a player's spells, damage against healing against deaths, this
pull beside the last one on the same boss. That is most of what Details is used for, with no
combat log in the addon. The agent's parse of the log file is the same fight in depth (timeline,
casts, the 5-second series), and the portal's Logs page is its robust form; the addon's fight and
the agent's are matched by encounter and start time so a row in game is the page on the portal.
Unverified in game (docs/forever-client-api.md, "Still owed"): that the calls answer for an addon
on Forever and what a finished session returns.

**Order** (approved by Maddison, 2026-11-09, after Prices is finished). (1) The window into the core, with its mockup (7.4, J0). (2) Characters and Items, from
the saved file alone. (3) Census, with a pooled summary file. (4) History, and Automation's session
list. (5) Fights, after the probes. Then 7.4's Collections, Dungeons and Quests, and the Auctions
section, each landing with its portal page.

### 7.6 Chronicle by zone (asked 2026-11-10)

The ask came from the guild Discord, after a preview of Kills & Loot: "Do you have it set up so it keeps track
in each zone you go to?" Today the answer is "partly".

#### What exists

- **Zones are logged.** `Zone.lua` writes a `zone` journal event on every `ZONE_CHANGED_NEW_AREA` (zone,
  subzone, instance, difficulty, `mapId`). Quest accepts and turn-ins, levels and deaths carry the zone too.
- **The tallies are lifetime totals per creature, item and object** (`Chronicle.lua`, wow.md 12.1):
  - `kills[npcId]` keeps `zone`, but that is only the latest kill's.
  - `objects[id]` is the same.
  - `loot[itemId]` has no zone at all; it is split by source (`from`), not by place.
- **The window shows the same.** The Kills section shows that latest zone as a column, and Loot shows none.
- **The API mirrors it.** `wow_kills.last_zone` and `wow_objects.last_zone`. The pooled
  `/chronicle/creatures/:npcId` lists "zones" as the 5 most common `last_zone`s across characters: one vote per
  character, not a count of kills.
- **Measured on 2026-11-10:**
  - Maddison's `Ryna.lua` is 5.8 MB, almost all of it journal and price data.
  - Its two characters hold 94 kill entries, 206 loot entries and 17 objects.
  - In prod, all 168 creatures killed so far were killed in one zone each (6 characters).
  - The pool is young, so "most creatures live in one zone" is likely but not yet proven.

#### Recommendation

**1. Record shape: a small count map on each tally, not a tally per zone.**

```lua
kills["6"]    = { name = "Kobold Vermin", n = 43, own = 40, xp = 1720, looted = 38, last = …, zone = "Elwynn Forest",
                  zones = { ["Elwynn Forest"] = 31 } }        -- kills per zone, counted from now on
loot["2589"]  = { …, n = 30, qty = 45, from = { … },
                  zones = { ["Elwynn Forest"] = { n = 22, qty = 31 }, ["Westfall"] = { n = 8, qty = 14 } } }  -- drops and pieces per zone
objects["1731"] = { n = 5, last = …, zone = "Elwynn Forest", zones = { ["Elwynn Forest"] = 5 } }
```

- `zones[z]` counts the same unit as the entry's `n`: kills, or objects opened. On loot it is `{ n, qty }`: loot
  windows with the item and pieces, both per zone (Maddison, 2026-11-10).
- `zone` stays as it is (the latest), so nothing reading it breaks.
- At most 8 zones per entry, keeping the biggest, with the same rule as `from` (`MAX_FROM`).
- Kills don't get per-zone `xp` or `own` counts; each would roughly double the map for little gain. Loot's `qty`
  per zone was chosen (Maddison, 2026-11-10), so a loot zone costs about twice a kill zone in the file.
- **Size.** About 27 bytes for the map plus about 30 per zone in the saved file:
  - roughly +20 KB on today's 5.8 MB file;
  - about +600 KB per character only if every tally sits at its cap (2,000 kills, 3,000 loot, 2,000 objects)
    with two zones each.
- **Logout time** grows in step with that, which won't be noticeable next to the journal.

**2. The zone key is the name the record already holds** (`ns.rec.zone`, from `GetRealZoneText`), not a map id:

- It is the zone, not the subzone. Inside a dungeon it is the dungeon (The Deadmines), which is what a player
  means by "where".
- It is what `last_zone`, the quest and death events and the census already use. The API's `areas` fold (wow.md
  8.x) and any later locale fix then apply to all of them at once.
- The alternative is `C_Map.GetBestMapForUnit("player")`, which `Zone.lua` already calls for the journal:
  - **For:** it is locale-proof.
  - **Against:** it returns the most specific map (a cave, a city floor), so it would need folding up to the zone
    through `C_Map.GetMapInfo` parents.
  - **Against:** none of those calls are in `docs/forever-client-api.md`. They would need checking in game first,
    and the rule is never to code Forever's API from memory.
  - It is worth doing later, for every zone field at once, not just for this one.

**3. When it counts.**
- **Kills:** `addKill` adds 1 to `zones[ns.rec.zone]`, beside `e.zone`. A kill that waited for its name (the
  pending ring) is counted in the zone it happened in, so the zone is captured when the XP line arrives.
- **Loot:** the zone is captured when the loot window opens (`rememberSlot` stores it in the slot), and
  `OnSlotCleared` adds 1 to that zone.
- **Objects:** `countSource` adds the same way.
- With the `chronicle` setting off, nothing is counted, as today.

**4. Totals from before the change: placed from the combat logs where they exist, never guessed.**

The API keeps no history of the tallies (each sync replaces `wow_kills`), so the only record of where an earlier
kill happened is the player's own combat logs, still on their Mac in `_forever_/Logs`. A log marks every kill
(`PARTY_KILL`: who landed the blow, the creature's GUID and so its npc id) and every zone change (`ZONE_CHANGE`).
The agent's parser (`cores/wow/core/src/combat/parse.rs`) already reads all of it: `PARTY_KILL`, `ZONE_CHANGE`,
the affiliation flags, and line times with their offset.

Measured on Maddison's Mac on 2026-11-10:
- 26 log files, from Nov 6 to Nov 10.
- **Player-6933-01A402F3:** the logs place 413 of 690 counted kills (60%), across 3 zones.
- **Player-6933-016102AC:** the logs place 96 of 108 (89%), in 1 zone.
- 359 kills in the logs were credited to party members. Chronicle counts every kill that gave the player
  experience, while the log credits only the killing blow, so counting party kills while grouped should close most
  of the first character's gap.

**The backfill:**
- **When.** It is a one-time job per character, run by the agent with the game closed. It writes the save file the
  same way 7.2's settings copy between characters does, so the counts show in game straight away.
- **Which kills.** The addon stamps `zonesSince` on the character the first time it counts a zone. Only log lines
  before that time are used, so nothing is counted twice.
- **Matching.** A kill is the character's when the blow's source is the character, or a party or raid member
  (`AFFILIATION_PARTY | AFFILIATION_RAID`) while the character was logging. Each kill is put in the zone of the
  `ZONE_CHANGE` before it.
- **How much is placed.** Per creature, the logs' zone split is applied to the earlier count `n − sum(zones)`,
  never placing more than that.
- **Where it is kept.** The result goes in its own map, `logZones = { [zone] = n }`, beside `zones`. The record can
  then always tell what was counted live from what was placed from logs, and the window can say so in a tooltip
  ("12 here, 4 of them placed from your combat logs").
- **Record of the run.** `RynaDB.characters[guid].zonesBackfill = { at, files, placed }`. A character that has one
  is never backfilled again.
- **Kills only.** The logs don't record loot or objects.
- **Not guessed from the pool.** The pool could guess that every creature ever seen in one zone was killed there.
  That is probably right, but it is a guess, and the logs are records. So the pool isn't used for this.

**What still can't be placed** (kills from before logging was on, logs the player deleted, or the dungeon-only
default of `combatLog = "instances"` missing world kills, which will be most players) stays on one line:
- **"Earlier travels"**, with a tooltip "counted before Ryna kept zones" (Maddison, 2026-11-10).
- Once an entry hits the 8-zone cap, the line takes in the dropped zones too, and the tooltip says "or in other
  zones".

**Picking a zone** shows `zones[z] + logZones[z]`, with the log-placed part named in the tooltip.

**Check first.** `ZONE_CHANGE`'s name is assumed to be the same text as `GetRealZoneText`. Check this on a real
log before building; if it's ever the subzone, fold it up through the same `areas` lookup the census uses.
- **Checked on 2026-11-10** (26 logs, Nov 6 to 10). It names the zone ("Elwynn Forest", "Westfall", "Duskwood",
  "Stormwind City", "Redridge Mountains", "Zephras Isle"; "The Deadmines" inside the dungeon, instance id 36), each
  with a `MAP_CHANGE` of the same name. But it also fires on walking into a building ("Lion's Pride Inn",
  "Sentinel Tower", "Lakeshire Inn", "Lakeshire Town Hall"), with no `MAP_CHANGE`, and on a loading screen
  ("UNKNOWN AREA", with the continent's `MAP_CHANGE`).
- So the agent (`cores/wow/core/src/zones_backfill.rs`) takes a `ZONE_CHANGE` name as a zone only when the logs
  also show it as a `MAP_CHANGE` or an instance (nonzero instance id), or the record already has it as a zone.
  Any other name leaves the kill in the zone it was in, which is the building's own. No `areas` lookup is needed.
- Measured with that rule: the logs place 781 kills (own and party) for Player-6933-01A402F3 in Elwynn Forest,
  Westfall and The Deadmines, and 315 for Player-6933-016102AC in Zephras Isle, none in a building. Each
  creature's share is still capped at its earlier count.

**5. In game first (7.5).** Decided (Maddison, 2026-11-10): the picker, the tooltip line and a Zones section.
- **Kills and Loot sections:** a zone picker beside the search ("All zones", "This zone", then the zones the
  character has counts in). With a zone picked, the numbers are that zone's and the totals tiles follow.
- **Tooltips:** one line, only when the creature or item has been counted in more than one zone or the current zone
  differs from the only one: "12 here (Westfall) · 43 in all".
- **A Zones section** in Ryna's window: one row per zone the character has been in, with kills, creatures, drops,
  pieces, quests done, deaths and time spent. Clicking a row opens Kills on that zone.
  - Kills, creatures, drops and pieces are sums over the tallies' `zones` (and `logZones`).
  - The rest needs a small lifetime map on the character, because the addon's journal keeps only 30 days
    (`cores/wow/addon/Ryna/Journal.lua` `Prune`): `places[zone] = { seconds, quests, deaths, first, last }`.
  - `seconds` grows on each zone change and at logout, from the time the zone was entered. `quests` grows on a quest
    turned in and `deaths` on `PLAYER_DEAD`, both in the character's zone at the time.
  - On first run it is seeded from what the journal still holds (up to 30 days of `zone`, `questDone` and `death`
    events). It is marked `placesSince`, so the window can say "time since <date>" until it covers everything.
  - The portal can show the full history, because the API keeps every journal event (`wow_journal`).
- Still silent: nothing in chat.

**6. Agent.** Nothing to change. The agent never parses the tallies; it passes the record through as JSON.

**7. API** (api repo; one migration, no new tables):
- **Migration** `00NN_wow_chronicle_zones.sql` adds `zones TEXT` (JSON `{ zone: n }`) to `wow_kills`, the loot table
  and `wow_objects`.
- **Ingest:**
  - `killEntry` and its siblings in `wow-service/src/schema.ts` gain an optional `zones`. Today's schema is a
    `looseObject`, so an addon that sends `zones` before the API knows it is accepted and ignored.
  - Checks: at most 8 keys; names follow the merchant rules (≤ 200 UTF-16 units, no `|`, no control or format
    characters); counts are whole and non-negative, each at most `n`.
  - A bad map is dropped with the entry kept, never a 400.
- **Reads:**
  - `GET /characters/:guid/kills` and the loot read gain `zones: [{ zone, n }]` per row and an optional `?zone=`
    filter (`json_each` over at most a few thousand rows of one character).
  - `GET /characters/:guid/zones` lists the character's zones for the portal's picker and its Zones view: kills,
    creatures, drops and pieces from the tallies, plus quests, deaths and time spent. Time comes from `places` once
    synced; before that, and for anything older than the addon's 30 days, it is computed from `wow_journal`'s `zone`
    events, with a session's end as its logout.
  - The record's `places` map is stored like the tallies: one row per character and zone, replaced on each snapshot.
  - The usual character access applies.
- **Pooled** `/chronicle/creatures/:npcId`: `zones` become real kill counts, summed over `json_each`, with
  `last_zone` as a fallback for rows not yet carrying `zones`. It stays an aggregate with no person in it, so the
  census privacy rules (8.1) aren't touched.
- **Connector:** the MCP's `wow` chronicle tool (`apps/mcp/src/domains/wow/tools/chronicle.ts`) passes `zone`
  through. It's a read, so there's no `scopes.ts` entry.

**8. Portal.** The character's Kills & Loot tab (`ChronicleSection.vue`) gets the same zone picker as the game,
plus the "before zones were tracked" line. The pure filter and remainder helpers get tests.

**9. Tests.**
- **Addon** (`fake_client`):
  - a kill in A, then B, then A gives `{A=2, B=1}`;
  - a pending-name kill counts in the zone it happened in, even after the player moves;
  - loot uses the zone at open;
  - the 8-zone cap keeps the biggest;
  - with `chronicle` off, nothing counts.
- **API:**
  - ingest keeps a good map and drops a bad one with the entry kept;
  - `?zone=` filters;
  - `/zones` lists;
  - pooled sums with the `last_zone` fallback.
- **Portal:** the remainder helper.

**10. Order.**
1. The next Chronicle release (Maddison, 2026-11-10): counting, `places`, the picker, the tooltip line and the Zones
   section. It is safe first, because the API
   ignores `zones` until step 2.
2. API migration, ingest and reads, deployed before the portal.
3. Portal picker.
4. The combat-log backfill in the agent (and the app's button for it), after the addon has stamped `zonesSince`
   on a character. The API takes `logZones` like `zones` (one more JSON column, the same checks).

The next agent sync after step 2 fills the columns from the addon's saved counts, so nothing is lost between
steps.

#### Maddison's calls (decided 2026-11-10)

1. **In game:** the zone picker, the tooltip line **and a Zones section** (one row per zone: kills, creatures, drops,
   pieces, quests done, deaths, time spent).
2. **Loot by zone:** drops **and pieces** (`zones[z] = { n, qty }`).
3. **The remainder's wording:** **"Earlier travels"**, with the tooltip "counted before Ryna kept zones".
4. **Release:** with the **next Chronicle release**.
5. **The backfill's trigger:** the **agent runs it by itself** the first time it sees `zonesSince`, with the game
   closed. Nothing to click; it only ever adds.

#### Not decided here

- **Locale-proof zone keys** for every zone field (map ids folded to zones), once the `C_Map` calls are verified on
  Forever.
- **Per-session "this zone"** numbers on the session strip.

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

### 9.1 Supported versions, channels and notices (designed 2026-11-10)

Asked by Maddison on 2026-11-09 ("minimum versions supported (think RFM from CrowdStrike), ideal
versions, and current version ... version channeling and notifications when someone is in RFM or out
in the cold") and on 2026-11-10 ("min version support, preferred and perfect"). This is Nyx's design;
the calls that are hers are listed at the end, with her answers once given.

**How versions travel today** (checked 2026-11-10, both repositories' main):

- The app's own version is in every body it sends: the snapshot's `agent`, the heartbeat's `agent`,
  the census upload's `agent` (`ryna-agent/0.3.9`). It is reliable: Cargo, `package.json` and the tag
  are held equal by `check-versions`.
- Each install's core addon version is read from `Interface/AddOns/Ryna/Ryna.toc` (`## Version:`) by
  the app and sent as the heartbeat's `installs[].addon`, by every released app since v0.2.0. Reliable.
- The snapshot's `addon` is `RynaDB.addon`, which the addon wrote from its own Lua. **Every released
  addon through 0.3.9 says `0.3.2` there** (the core and Chronicle each hard-coded it until 662e1b3,
  2026-11-10, unreleased). So the API must never judge an addon by that field, and the addon must never
  judge itself by its `ns.VERSION` while old copies are about.
- `AgentStatus.lua` carries `latest = { addon = nil, agent = nil }` back to the addon; nothing fills it,
  so the Status tab's "Update available" lines never show.
- Mods carry `requires.core` in `mod.json` and their own TOC version; all release together today.
- The only judgment that exists is the snapshot's `schema` (`SCHEMA_UNSUPPORTED`, accepted `[1]`).

**What is versioned, and what carries the marks.** One set of marks, for **a Ryna release**
(`0.3.9`), applied to two things that can each lag: **the app** (its own version, from every body it
sends) and **each install's core addon** (its TOC, as the app reads it). They release together and
the app carries the addon, but the addon can lag the app (the Installs list offers the update, it
doesn't force it). The rest follows: a **bundled mod** versions with the release and is covered by the
addon's mark (its `requires.core` stays the install-time check it is); the **saved-file schema** and
the **upload format** keep their own gate, `schema` and `SCHEMA_UNSUPPORTED`, which is about format,
not age. A mod fetched from git (stage 6) would get its own marks then; the policy's shape leaves room
(below) and nothing uses it now. A person's band is the lower of the app's and their addons'.

**The marks, one sentence each.**

- **Perfect**: the newest release on the person's channel, the one the updater offers; at it there is
  nothing to do.
- **Preferred**: the oldest release that still gets everything; between preferred and perfect, everything
  works and the only notice is the existing "Ryna x.y.z is available".
- **Minimum**: the oldest release the API still talks to at all; between minimum and preferred is
  **reduced mode**, below minimum is **unsupported** ("out in the cold").

So a person is in one of four states: **current** (= perfect), **behind** (preferred ≤ v < perfect),
**reduced** (minimum ≤ v < preferred), **unsupported** (v < minimum). A version the API can't read
(`dev`, `unknown`, missing) counts as current: it is never a reason to refuse.

**Reduced mode: what it keeps and drops.** The point of RFM is to keep a person working while
keeping an old client from doing harm to others. An old client's harm is to the pools, which everyone
reads: a parse or validation bug fixed in a later release (the 0.3.2 version field was one) would keep
feeding wrong rows into the census or the price pools. Your own data hurts no one but you, and you
can see it.

| In reduced mode | Kept or dropped | Why |
|---|---|---|
| Your snapshots, journal, characters, bags, quests | **kept** | Yours, shown back to you; an older format is still parsed or refused by `schema` as today |
| Your fights (encounters, digests, raw logs) | **kept** | Yours alone |
| Settings backups, all of them | **kept** | Yours, and a backup is what you'd want before an update |
| Heartbeat, the Agent page | **kept** | How you learn you're in reduced mode |
| Contributing census sightings (from a snapshot, and `POST /census`) | **dropped** | Pooled: everyone reads it |
| Contributing merchant offers and auction snapshots | **dropped** | Pooled: everyone's tooltips read it |
| Receiving the pooled price files (`GET /agent/prices`) | **kept** | Reading the pool harms no one; taking it away is only pressure, and the in-game notice already does that |

Dropping is the API's to do, not the client's, so it holds for an old app that knows nothing of
this: at ingest, the snapshot is stored and the pooling step is skipped, and the answer says so
(`pooled: false`, with the band). `POST /census` from a reduced app is a refusal with its own code.

**Unsupported: what the API refuses and what still answers.** Refusals are envelopes, 200 `ok:false`,
with a new code **`AGENT_UNSUPPORTED`** and `detail { version, minimum, preferred, perfect, channel,
update }` (`update` the guide's update steps, `https://ryna.io/wow/guide#upkeep`). Its `message` is written
for the person, because released apps (through 0.3.9) show an unknown refusal's message as the error
line in the app and write it into `AgentStatus.lua`'s `messages`, which every released addon shows on
its Status tab: so a 0.3.x app below minimum still tells its person in both places, with no change.

- **Refused**: `POST /agent/ingest`, `POST /agent/encounters` and its two PUTs, `POST /census`,
  `GET /agent/prices` (the files' format may have moved on), and the backup writes (create, parts,
  complete, delete).
- **Still answer**: `POST /agent/heartbeat` (always; it records the version and answers with the
  band, so the app can say why and where the update is), the backup reads (list, open, content: no
  one loses their settings for having been away), and the policy itself (below).
- **The portal and the connector are not versioned**: your characters stay readable on ryna.io
  whatever your app's version, with the notice above them.

**Where the policy lives.** Data the API serves, not numbers built into a client, so a mark moves
without a release:

- A D1 table, one row per channel: `minimum`, `preferred`, `perfect`, a `note` shown to people (one
  line, optional: "0.3.4 mis-reads auction prices"), `updated_at`, `updated_by`. Perfect is set by
  hand alongside a release; the Admin page links the newest release beside it (reading the updater
  manifest automatically is a later step, if forgetting proves real).
- **No public read of the policy** (as built, 2026-11-10): api.ryna.io stays closed to the public
  (the API's grant gate keeps its exemptions to the census upload and the portal's own summary), so
  instead every answer that judges carries `support: { band, version, addon, minimum, preferred,
  perfect, channel, note, update }`: the heartbeat's (always answered, so every signed-in app learns
  its band), the ingest's, and the census upload's (so a signed-out app learns it too).
- **What the app sends to be judged**: its version in the User-Agent every release already sends
  (`ryna-agent/0.3.9`), the addon's TOC as the heartbeat's `installs[].addon` (since 0.2.0) and, from
  the next release, as the snapshot's `addonToc`; its channel as the heartbeat's `channel` and the
  `X-Ryna-Channel` header (absent is stable).
- **Who edits it**: Admin only (`wow:admin`), `PUT /v1/wow/admin/versions`, portal-only and audited
  (`wow.admin.versions.put`, the old and new marks in `details`) like the other `/admin` routes; a
  `portalOnly` entry in `scopes.ts`. The editor refuses marks out of order (minimum ≤ preferred ≤
  perfect) and shows, before saving, how many people the change would put in reduced and unsupported.
- **Failing open.** The app treats a policy it can't fetch as "current" and keeps the last one it
  fetched only to show, never to refuse. On the API, a missing or unreadable policy row is "current"
  for everyone. Unlike authorization (which fails closed), this is about helping people update, and
  ryna.io being unreachable must never lock anyone out.
- **The first policy** is open: no minimum, no preferred, perfect 0.3.9. Every released version is
  current or behind; nobody is reduced or refused until Maddison sets a mark.

**Channels.** A channel is an updater manifest and a row of marks. Today there is one, **stable**:
`releases/latest/download/latest.json`, which a pre-release tag (`v0.4.0-rc.1`) never reaches. A
second, **beta**, would be a manifest at a fixed release (`channel-beta/latest.json`) that
`release.yml` rewrites for pre-release tags, a choice in the app's Settings (Stable or Beta; switching
back to stable waits on the version rather than downgrading), and `channel` in the heartbeat. Beta's
perfect is the newest pre-release; its minimum and preferred are stable's unless set. The first build
ships the policy keyed by channel and the heartbeat's `channel` (always `stable`), so adding beta later
is the manifest, the setting and one row, with no change to the judgment.

**Notices, one view per place** (7.5: whole in the game, richer on the portal).

- **The app**: the update banner it already has, in a colour per state, with **Update and restart**:
  - behind: unchanged, "Ryna 0.3.10 is available."
  - reduced: "This version of Ryna is getting old. Your characters still sync, but what you add to
    everyone's census and prices is paused until you update."
  - unsupported: "This version of Ryna is no longer supported, so nothing syncs to ryna.io. Update to
    carry on." Not dismissable by "Later"; the heartbeat keeps running.
  - the policy's `note`, when there is one, as a second line.
- **The game**: the app writes `support = { band, version, addon, perfect, note, update }` into
  `AgentStatus.lua` (as built, 2026-11-10); the
  addon shows it as the first line of the `/ryna` Status tab (amber for reduced, red for unsupported,
  the same words as the app, "Open the Ryna app to update"), and for unsupported a **notice frame** at
  login, once per login, closed with a button. Never chat. Addons through 0.3.9 don't know `support`,
  so the app also writes the same sentence as a `messages` line (level `warn` or `error`), which those
  addons already show. `latest.agent` and `latest.addon` are filled from the policy at last, `addon`
  only when the TOC on disk is older (an old addon compares against its frozen `0.3.2`, so filling it
  when they're equal would claim an update that isn't there).
- **The portal's Agent page**: each computer's card shows its band beside the app and addon versions:
  "Up to date", "Ryna 0.3.10 is available", "Reduced: update to keep contributing to the census and
  prices", "Not supported: nothing from this computer syncs until you update", with the note and a
  link to the guide's update steps.
- **Admin**: a **Versions** tab on `/wow/admin`: the marks per channel and their editor (with the
  preview of who moves), and every computer by band (person, host, app version, addon versions,
  channel, last seen), counts at the top. Reading it is audited (`wow.admin.versions`).

**Stages**, API first, each shippable alone:

1. **API and portal** (private repo; contract in its `docs/wow.md` section 18): the table with the
   open policy, the band on the heartbeat, ingest, encounters, census, prices and backup routes (`AGENT_UNSUPPORTED`,
   and reduced's `pooled: false` and census refusal), `support` on `GET /agent`, the Admin Versions
   tab and its audited `PUT`, the Agent page's notices, `docs/wow.md` section 18. Live at once; with
   the open policy nothing changes for anyone.
2. **The app** (ryna_agent): reads `support` from the heartbeat's answer (and the census upload's
   when signed out), the banner's states, `channel` in the heartbeat, `addonToc` in the snapshot,
   `support` and the `messages` line in `AgentStatus.lua`, `latest` filled. Ships with the next release.
   As built (2026-11-10, PR `claude/version-support`): the last judgment is kept in the state
   folder's `support.json` with when it came, shown only for the app version it judged (an update
   never shows the old version's banner), and kept through a failed heartbeat to show. The app itself
   sends no census upload yet (only `ryna census push` does, and keeps that answer's judgment), so a
   signed-out app has none and shows current. `X-Ryna-Channel` goes on every request the agent makes.
3. **The addon** (ryna_agent): the Status tab line and the notice frame, from `support`. Same release.
   As built: the line leads `/ryna status` too, and the Messages tab leaves out the app's copy of the
   sentence while the Status tab says it.
4. **Channels**: the beta manifest in `release.yml`, the app's channel setting. Its own release.

**Settled 2026-11-10, Maddison:**

- **Perfect** is the newest release on the channel, the one the updater offers. She sets it on the
  Admin page alongside each release (the page shows the updater manifest's newest beside it).
- **Reduced mode stops contributing only**: census sightings and merchant and auction contributions
  pause; your own snapshots, fights and backups keep syncing, and the pooled price files keep coming
  down.
- **The grace rule counts releases**: preferred trails perfect by **2** releases, minimum by **4**
  (so with perfect at 0.3.9, preferred may be 0.3.7 and minimum 0.3.5). It is the rule she follows
  when moving the marks; nothing moves them on a timer. A data-corruption or security fix may move
  minimum sooner, by her word.
- **Channels later, shape now**: the first build keys the policy by channel and sends
  `channel: "stable"`; beta is stage 4, its own release.
- **The marks are a D1 table she can change quickly** (Maddison, 2026-11-10, later the same day: "i
  would like to make the [versions] a DB table for me to be able to control that quickly if needed"):
  `wow_version_policy` in the `api` D1, one row per versioned thing and channel (`ryna`/`stable` today),
  with who changed it and when. Nothing caches it on the server, so an edit holds from the next call;
  a running app sees it at its next heartbeat (about a minute). Two ways to change it: the Admin page
  (audited), and for an emergency a plain SQL `UPDATE`, written out in the api repo's `docs/wow.md`
  section 18. Every read checks the row, and a malformed one counts as no policy (everyone supported),
  never a lockout. The seed supports every released version; she sets the first real marks.
- **The wording is the plain one above**, the same sentence in the app, the game and the portal:
  reduced, "This version of Ryna is getting old. Your characters still sync, but what you add to
  everyone's census and prices is paused until you update."; unsupported, "This version of Ryna is no
  longer supported, so nothing syncs to ryna.io. Update to carry on."

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
   The automation mod exists (`mods/wow/automation`, v1 and v2, 2026-11-07): quests (accept, turn in,
   quest-only gossip, leaving low-level quests, an optional pick of the best-selling reward), vendor
   (sell greys and marked junk, repair, the guild bank first optionally), trainers (all under a gold
   cap), mail (never cash on delivery), resurrections, friend and guild invites, bind/roll/delete
   confirmations, cinematics, quest tracking, release in battlegrounds, auto-loot and loot rolls
   (never Need); off by default, every chore its own toggle that starts off, Shift pauses it.
   Then the Settings mod (7.2): backup of addon settings and copies between characters, agent side.
   Then the Prices mod (7.3): the sell line first, merchant pooling second, the auction line third.
   Then Chronicle's journal (7.4): the Chronicle window moves into the core as `Ryna.Window` (J0),
   then collections (J1) and dungeons (J2) as sections of Chronicle; quests (J3) wait for stage 3.
3. **Data layer v1.** The plumbing exists since 2026-11-09 (`Data/`, `Ryna.Data.Get`, the agent's
   `prices_data`), carrying the Prices mod's two tables. Next: parse the three caches, upload them,
   pool them, write `Data/names.lua` and `Data/quests.lua` back down; the kills table gains names for
   creatures never targeted; the Chronicle window (0.2.x plan) reads community numbers from the same
   files, and the quest pool is what Chronicle's Quests section (7.4, J3) is measured against.
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
- Settled 2026-11-09 (7.5), Maddison: **the game visualizes what we do; the portal adds richness.**
  Every feature, old and new, has a whole in-game view; 7.5 holds the audit and the order.
- Settled 2026-11-09 (7.4): the journal is part of Chronicle, not a mod of its own; its sections
  start off, with one notice at the first dungeon; quest tracking is in, the map stays Questie's.
  Open: flight points, and the notice's wording.
- Settled 2026-11-10 (9.1): supported versions. Perfect is the newest release; reduced mode stops
  contributing to the pools and nothing else; the marks trail perfect by 2 and 4 releases; a beta
  channel comes later; the notices' wording.
- Settled 2026-11-07: the repository (A, `ryna_agent`); the Palworld agent goes MIT barring anything
  sensitive; Palworld mods are undefined for now; Chronicle is the first WoW mod.
