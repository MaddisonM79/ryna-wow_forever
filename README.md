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
| Linux (x86_64: Arch, Omarchy, Ubuntu, …) | `Ryna_<version>_amd64.AppImage` |

On a Mac, open the disk image and drag Ryna into Applications. The app is signed by Nyra Security
LLC and notarized by Apple, so it opens like any other app. On Windows the installer isn't signed
yet, so SmartScreen will say "Windows protected your PC": click *More info*, then *Run anyway*. The
`SHA256SUMS` file on the release lets you check what you downloaded.

**On Linux** the AppImage is the whole app: make it executable and run it, from wherever you keep
such things.

```bash
chmod +x Ryna_<version>_amd64.AppImage && ./Ryna_<version>_amd64.AppImage
```

Linux builds aren't signed; check the download against `SHA256SUMS`. Ryna lives in the tray while
you play, so your bar needs a tray that speaks StatusNotifierItem (Omarchy's bar has one; on another
Hyprland or Sway setup, Waybar's `tray` module) and the system needs `libayatana-appindicator`
(the Arch package of that name; Ubuntu's `libayatana-appindicator3-1`). Without it Ryna still runs,
but has no tray icon, so closing the window quits it. Your sign-in is kept in the system's Secret
Service (gnome-keyring, or KeePassXC with its Secret Service integration on); without one running,
Ryna keeps it in the kernel keyring instead and tells you the sign-in lasts until you restart. On
Hyprland, if you'd rather Ryna float than tile, its window class is `ryna-wow-app`:

```
windowrulev2 = float, class:^(ryna-wow-app)$
```

`agent/README.md` ("Linux") has the rest: where the game is found under Wine and Proton, opening
Ryna at login under Hyprland, and what to do about a blank window.

**2. Install the addon.** Open Ryna. The *Game* section finds World of Warcraft: Forever in its usual
place (`/Applications/World of Warcraft/_forever_` on a Mac, `C:\Program Files (x86)\World of
Warcraft\_forever_` on Windows, and on Linux the same `Program Files` inside the usual Wine and
Proton prefixes: Lutris's `~/Games`, Bottles, Steam's `compatdata`, `~/.wine`); if yours is
elsewhere, *Choose folder…* and pick the `World of Warcraft` folder (on Linux, the one under the
prefix's `drive_c`). Click **Install addon**.
It copies the addon that came inside the app into the game's `Interface/AddOns`: Ryna itself and
the mods that are on. Each mod is listed under the install with a switch; there are four so far:

- **Chronicle**: what Ryna records about your characters, and what the merchants you visit sell
  (for vendor prices; merchants are NPCs). On by default.
- **Automation**: does the chores you tick in game (`/ryna`, its Automation tab, or `/ryna auto`).
  At quest givers: accepting and turning in quests, opening the one quest an NPC who offers nothing
  else has, leaving low-level quests alone, taking the reward that sells for the most (otherwise a
  choice of rewards is always yours). At vendors: selling grey items and the items you marked as
  junk, and repairing. At trainers, learning everything within a gold cap (10 gold until you change
  it); at a mailbox, taking the gold and items from every mail that isn't cash on delivery (it never
  deletes or returns mail). Around the world: accepting resurrections out of combat, group invites
  from friends and your guild only, the bind and roll confirmations (and typing DELETE for you,
  though the click stays yours), skipping cinematics, tracking new quests, releasing in
  battlegrounds only, keeping the game's auto-loot on, and loot rolls (Greed or Pass, never Need;
  your group sees them). It's off by default, and turning it on in the app still does nothing until
  you tick a chore: every one starts unticked. Hold Shift (at an NPC, or as a cinematic starts) and
  nothing happens by itself; nothing happens in combat either.
- **Prices**: what a vendor pays for an item, and for its stack, on every item tooltip ("Sell
  Price: 13s 50c (2g 70s for 20)", in the game's own words and coins), in your bags and bank, in chat
  links and loot, away from vendors too, and never twice where the game shows its own. Its Prices tab
  (or `/ryna prices`) turns the stack total off or keeps the line away while a merchant window is open. It only reads the game's item data. Off by default.
- **Settings**: your addons' settings (ElvUI and every other addon's, found from their own files),
  copied from one character to another and backed up to ryna.io, stored encrypted for you. It
  works in the app with the game closed and puts nothing in the game, so there's no addon folder for
  it. Off by default; turning it on adds an *Addon settings* tab beside Mods. "Copy from another
  character" shows every file it will write before it writes any, and keeps a `.ryna.bak` of each
  file it replaces. Backups are yours alone and need you signed in.

If the game is already running, restart it so it sees the new addon.

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

The census is the other players your characters see: nearby (nameplates, target, mouseover), in
chat (who spoke, never what), in your group and guild, and in the answers to a `/who` you type
yourself. Ryna never sends `/who` on its own. Your own characters never go to it: that means every
character Ryna has recorded on this computer, on any of your game accounts and installs (every
character that has logged in there, by its folder in the game's `WTF`), so one of your characters
seen by another of yours is left out like any other of yours. The census is about
everyone and names no one in its numbers. Looking up a character by name on ryna.io shows what `/who` would: name, level, class,
guild, and the zone (never the subzone), with when it was last seen only as "today", "this week",
"this month" or "older", never a time of day. There are no per-character views of hours or days;
those exist only as totals across all players. Every lookup is logged for the operator, so someone
searching one name over and over is visible, and lookups need a ryna.io account with the WoW app.
We can't verify that a removal request comes from the account holder, so, like Warcraft Logs, we
don't offer one; these limits are what protect a player instead.

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
  installer); the in-game addon `cores/wow/addon/Ryna`, tagged for Forever, the plumbing its mods
  build on (`cores/wow/addon/README.md`: what is recorded, settings, your own `/who`, translations,
  the mod API); and the scrubbed test fixtures in `cores/wow/fixtures`. Its mods are in `mods/wow`:
  Chronicle (`mods/wow/chronicle`), everything recorded about a character, Automation
  (`mods/wow/automation`), the chores (quests, vendors, trainers, mail, rolls, invites and the like),
  each off until ticked, Prices (`mods/wow/prices`), vendor prices on item tooltips, and Settings (`mods/wow/settings`, `kind: "agent"`: no addon folder; the
  core's `settings` module), addon settings copied between characters and backed up encrypted.
- `mods/`: each core's mods (`mods/wow`), from stage 2 of the design.
- `scripts/`: version checks, the release manifest, the CLI smoke test, and `capture.sh`, which
  turns a real `Ryna.lua` into a scrubbed test fixture.

Releases are built by `.github/workflows/release.yml` from a `vX.Y.Z` tag: macOS (both chips),
Windows and Linux (an AppImage), the addon zip, `SHA256SUMS`, and the updater's signed artifacts
and `latest.json`.

## Thanks

Ryna is made by MaddisonM79, with Nyx (Claude). The people who tested it and made it better are in
`CREDITS.md`, which the app's About tab and the addon's panel also show: first among them Rencas Copperbeard.

## Licence

MIT. See `LICENSE`.
