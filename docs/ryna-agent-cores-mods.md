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

### 7.4 Journal: what exists, against what you've done (asked 2026-11-09)

Maddison, after looking at what she runs beside Ryna (AllTheThings, 71 MB; Doud's Dungeon Journal,
35 MB; Questie and its database, 211 MB): "Journal and ATT, there is a HUGE amount of overlap there.
Those are both heavier lifts but one tool for it", then the point that decides the design: "ATT and
Questie both have something we don't then: what isn't done." And: "an enhanced UI in game so as more
items are added we have one window to review things."

**The gap.** Three kinds of knowledge make a journal, and Ryna has two:

1. *What this character has done*: Chronicle's record (kills, loot and where it came from, completed
   quests, known recipes, reputations, bags and bank).
2. *What anyone has seen*: the pools (drop rates, kill counts, merchants, auctions).
3. *What exists*: the whole set, which "missing" is measured against. This is what ATT and Questie
   ship as hand-built Lua, and what Ryna doesn't have.

The Journal is the third, joined to the first two. It isn't hand-built: each set comes from the
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
- **Mounts and pets need one answer first.** Blizzard's Collections window is not allowed to load on
  Forever (`Blizzard_Collections.toc`: `AllowLoadGameType: wrath, cata, mists, mainline`), so they are
  probably items in bags, as in Classic, and "owned" is then the item in Chronicle's bags and bank,
  joined to the list through `ItemEffect` (item to spell) and `Mount.SourceSpellID` /
  `BattlePetSpecies.SummonSpellID`. If `C_MountJournal` and `C_PetJournal` answer in game instead,
  Chronicle reads those. Probe below.
- **Dungeons and quests are grown, not shipped.** Doud's Journal carries 35 MB because the client has
  no journal data to read. Ryna's comes from everyone's play: loot per boss from Chronicle's tallies,
  abilities and the boss's creature ids from the agent's fights (the digest names the actors inside
  an `ENCOUNTER_START`/`END`), and quest titles, zones and objectives from the client's
  `questcache.wdb` (section 7's first source, stage 3). The quest count is honest from the start:
  6,616 ids exist, and the Journal can say how many of them nobody has named yet. This side is thin
  while few people play and gets better without a release.

What Ryna will not have: Questie's map pins and objective locations, and ATT's hand-curated
"how to get it" notes. The Journal says where something came from when a pool has seen it, and
"not seen yet" otherwise. Questie stays installed for the map.

**Chronicle or its own mod.** Recommended, Maddison's to confirm (she offered the other reading too:
"part of Chronicle and then have settings to enable or disable certain things"): its own mod,
`journal` (`Ryna_Journal`, `kind: "addon"`, `actsInGame: false`, off by default), which requires the
core and nothing else.

- *Recording stays Chronicle's.* Anything new the Journal needs about a character (mounts and pets
  owned, flight points) is a Chronicle record, by the line in section 12.
- *The Journal reads, never records.* It reads this character's record through the core
  (`Ryna.DB().characters[guid]`, the saved file the core owns and Chronicle fills) and the reference
  tables from the data layer. Neither addon calls the other.
- *Without Chronicle* the reference side still works (every list, every pooled rate) and the "done"
  marks are absent, with one line saying Chronicle fills them in. With it, they are live: a boss
  killed is marked the moment Chronicle counts it, no sync or reload in between.
- *Why not inside Chronicle:* Chronicle is on by default and earns that by recording and touching
  nothing. A browsing window and its data files, on for everyone, change what installing it means.
  The cost of separating is one more toggle.

**One window.** Chronicle already has the window (`RynaChronicleWindow`: Session, Kills, Loot,
Levels, a search box, and lists that fill only the rows on screen, so 2,000 creatures cost what 15
do). It moves into the core as `Ryna.Window`, beside `Ryna.Panel`:

- One frame. A rail of sections down the left, tabs inside a section, one search box, Escape closes.
- A mod registers a section: `Ryna.Window.AddSection({ key, label, order, tabs = { { key, label,
  columns, rows = function(filter) … end, tooltip = function(row) … end } } })`. The core owns the
  frame, the virtual list, sorting, the search box and the row tooltip; a mod supplies rows.
- Chronicle registers "Chronicle" with its four tabs, unchanged. The Journal adds "Collections" and
  "Dungeons", later "Quests". Prices can add "Auctions" over its own snapshot. The panel
  (`/ryna`) stays for settings; the window is for data.
- `/chronicle` and `/ryna chronicle` keep working and open the window on Chronicle's section. A
  section's rows are built when it is first shown and rebuilt only when its data's version changes,
  as the Chronicle views are today.

This is a move, not a feature: one release with nothing new in it, the Chronicle window's tests
moved with it.

**Data files and their cost.** AllTheThings and Questie cost what they do because the client parses
their tables at every login. The Journal's tables are small because they hold ids and numbers, not
prose, and they are loaded late:

- The agent writes them as a load-on-demand addon folder beside the others (`Ryna_JournalData`:
  a TOC with `## LoadOnDemand: 1` and one Lua file per table), and the Journal loads it the first
  time its section is shown (`C_AddOns.LoadAddOn`). Nothing is parsed at login. To check in game
  before relying on it (probe below); the fallback is `Data/` files in the core's TOC, held to a
  smaller budget.
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
The pools are counts. Nothing about a person is read, pooled or shown, the Journal sends nothing,
prints nothing in chat, and does nothing for the player.

**Stages.**

- **J0. The window into the core.** `Ryna.Window`, Chronicle's tabs moved onto it. One release.
- **J1. Collections.** The game tables, the agent read and the data folder; the `journal` mod with
  its Collections section: recipes by profession (known, missing, and where a pool has seen the
  missing ones), reputations, then mounts and pets once the probe says how they are owned.
- **J2. Dungeons.** The two pools; a Dungeons section: instances, bosses (killed or not), each
  boss's loot with its pooled rate and whether this character has looted it, and its abilities.
- **J3. Quests.** With data layer v1 (section 11, stage 3): quests by zone, done or not, and the
  count nobody has named yet.

**Owed in game** (Maddison's, one line each; answers go into docs/forever-client-api.md):

1. `/dump C_MountJournal ~= nil, C_MountJournal and C_MountJournal.GetNumMounts()` and
   `/dump C_PetJournal ~= nil, C_PetJournal and C_PetJournal.GetNumPets()` (is there a journal, or
   are mounts and pets items?)
2. `/dump C_AddOns.IsAddOnLoadOnDemand("ElvUI_Options"), C_AddOns.LoadAddOn("ElvUI_Options")`
   (does load-on-demand work for an addon's own folder here?)
3. `/dump C_AddOns.IsAddOnLoadable("Blizzard_EncounterJournal")` (its TOC allows `standard, classic`;
   is Forever's game type inside `classic`? With no journal tables it would be empty either way.)
4. In a dungeon, after a boss dies: does Chronicle's kills tab count the boss's creature id (it does
   for open-world kills), and does the app show the fight under its encounter name?

**Open, Maddison's:** the name ("Journal" is a working title; the Chronicle name is taken by the
data product); its own mod or sections inside Chronicle (recommended: its own); on or off by
default (recommended: off until J2 makes it worth opening); whether quests belong here at all or
stay Questie's; whether flight points are worth a Chronicle record.

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
   The automation mod exists (`mods/wow/automation`, v1 and v2, 2026-11-07): quests (accept, turn in,
   quest-only gossip, leaving low-level quests, an optional pick of the best-selling reward), vendor
   (sell greys and marked junk, repair, the guild bank first optionally), trainers (all under a gold
   cap), mail (never cash on delivery), resurrections, friend and guild invites, bind/roll/delete
   confirmations, cinematics, quest tracking, release in battlegrounds, auto-loot and loot rolls
   (never Need); off by default, every chore its own toggle that starts off, Shift pauses it.
   Then the Settings mod (7.2): backup of addon settings and copies between characters, agent side.
   Then the Prices mod (7.3): the sell line first, merchant pooling second, the auction line third.
   Then the Journal (7.4): the Chronicle window moves into the core as `Ryna.Window` (J0), then
   collections (J1) and dungeons (J2) as a mod of their own; its quest side (J3) waits for stage 3.
3. **Data layer v1.** Parse the three caches, upload them, pool them, write `Data/names.lua` and
   `Data/quests.lua` back down; the kills table gains names for creatures never targeted; the
   Chronicle window (0.2.x plan) reads community numbers from the same files, and the quest pool
   is what the Journal's Quests section (7.4, J3) is measured against.
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
- **The Journal (7.4), asked 2026-11-09:** its name; its own mod or sections inside Chronicle; on or
  off by default; whether quests belong in it or stay Questie's; and the four in-game probes it lists.
- Settled 2026-11-07: the repository (A, `ryna_agent`); the Palworld agent goes MIT barring anything
  sensitive; Palworld mods are undefined for now; Chronicle is the first WoW mod.
