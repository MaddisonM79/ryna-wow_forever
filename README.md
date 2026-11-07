# Ryna for World of Warcraft: Forever

Ryna is a companion for World of Warcraft: Forever: a game addon plus a small desktop app for the
computer you play on. The addon records what your characters have and do; the app reads that, and
the game's own combat log, while it's open, and sends it to a backend where you can look at it. For
the people on [ryna.io](https://ryna.io/wow) that's characters, bags, bank and gold, quests, played
time per level, every fight with its meters and timelines, a guild roster, and a pooled census of
who's out there.

This repository is the part that lives on your machine, published so it can be read, audited and
reused. The hosted half (the API, the web portal and the assistant connector) is private and runs
at ryna.io for its own people. Anyone can run a backend that speaks the same format; that's allowed
and not a goal.

## Install

You need an account on the backend first. For ryna.io that means Maddison has turned the WoW app on
for you; if you can open [ryna.io/wow](https://ryna.io/wow) and see a page, you're set.

**1. Get the app** from the [latest release](https://github.com/MaddisonM79/ryna_agent/releases/latest).
Pick one file:

| You have | Download |
|---|---|
| A Mac with Apple silicon (M1 or later) | `Ryna_<version>_aarch64.dmg` |
| A Mac with an Intel chip | `Ryna_<version>_x64.dmg` |
| Windows | `Ryna_<version>_x64-setup.exe` |

On a Mac, open the disk image and drag Ryna into Applications. The app is signed by Nyra Security
LLC and notarized by Apple, so it opens like any other app. On Windows the installer isn't signed
yet, so SmartScreen will say "Windows protected your PC": click *More info*, then *Run anyway*. The
`SHA256SUMS` file on the release lets you check what you downloaded.

**2. Install the addon.** Open Ryna. The *Game* section finds World of Warcraft: Forever in its usual
place (`/Applications/World of Warcraft/_forever_` on a Mac, `C:\Program Files (x86)\World of
Warcraft\_forever_` on Windows); if yours is elsewhere, *Choose folder…*. Click **Install addon**.
It copies the addon that came inside the app into the game's `Interface/AddOns`. If the game is
already running, restart it so it sees the new addon.

**3. Sign in.** Click **Sign in**. Ryna shows a short code and opens a sign-in page in your browser;
check the page shows the same code, then allow it. Ryna never sees your password: it gets a token
for itself, kept in your keychain (or Windows credential store), and that's what it signs in with
from then on.

**4. Play.** Leave Ryna open while you play; it syncs only while it's open, and quitting it stops
everything. The addon writes its file when you log out or type `/reload`, so your first character
appears on ryna.io after your first logout. Every game account under the same install is picked up,
so two accounts open at once both sync.

In game, `/ryna` opens the addon's panel: what it's recording, the census settings, messages from
the app, and a debug toggle. There's a minimap button too.

**Updates.** Ryna checks for a newer release when it opens and every six hours, and offers to
update and restart. Each update is checked against a signing key before it installs. The addon
travels inside the app, so when the app updates, the *Game* section offers the matching addon.

## What leaves your computer

Only what you can see in the addon's SavedVariables file (`WTF/Account/<account>/SavedVariables/Ryna.lua`)
and in the per-fight summaries the app computes: your characters, their inventory, quests,
professions and reputations; the guild roster; and for each fight, who was in it (character names
and in-game ids), what they did, and when. Raw combat-log text is uploaded only if you turn that on
in Ryna's settings. Chat is never recorded: the addon keeps who it saw and where, never what anyone
said. Nothing is read from anywhere but the game's own folder, and the app signs in as you, to the
backend you point it at. Your game account's folder name is hashed before it leaves the computer;
only its last four characters go along as a label, so you can tell two accounts apart.

The data is never sold and never used for advertising. That includes the pooled census of players
seen in the world, which exists so the people who play can see how many of them there are and
when. It is for the community that plays, and for nothing else.

## What is in here

- `docs/ryna-agent-cores-mods.md`: the design for where this goes: Ryna is the agent, games are cores,
  mods belong to cores, and a data layer carries game data into the game without an addon release.

Ryna is **the agent**; each game is a **core** behind it; **mods** belong to a core. One Rust
workspace (`Cargo.toml` at the root) holds the agent and the cores' Rust halves.

- `agent/`: the agent. `agent/host` (crate `ryna-agent`) is what's the agent's and not any
  game's: the sign-in (OAuth device grant) and token store, the HTTP client, the state folder, the
  failure words, and `ryna_agent::core`, the one interface a core implements (its manifest and the
  `Core` trait). `agent/app` is the Tauri desktop app people install; `agent/cli` is a command line
  for developers. `agent/README.md` covers building, how it runs, releasing and signing.
- `cores/`: the games (`cores/README.md`). `cores/wow` is World of Warcraft: Forever: its manifest
  `core.json`; the Rust core `cores/wow/core` (crate `ryna-wow-core`: the SavedVariables reader,
  install discovery, the combat-log tailer and fight parser, the pusher, the watch loop and the addon
  installer); the in-game addon `cores/wow/addon/Ryna`, tagged for Forever
  (`cores/wow/addon/README.md`: what it records, settings, the census sweep, translations); and the
  scrubbed test fixtures in `cores/wow/fixtures`.
- `mods/`: each core's mods (`mods/wow`), from stage 2 of the design.
- `scripts/`: version checks, the release manifest, the CLI smoke test, and `capture.sh`, which
  turns a real `Ryna.lua` into a scrubbed test fixture.

Releases are built by `.github/workflows/release.yml` from a `vX.Y.Z` tag: macOS (both chips)
and Windows, the addon zip, `SHA256SUMS`, and the updater's signed artifacts and `latest.json`.

## Licence

MIT. See `LICENSE`.
