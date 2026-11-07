# Ryna: the agent

A small program that runs on your computer, finds World of Warcraft: Forever, reads what the Ryna
addon saved (`WTF/Account/<account>/SavedVariables/Ryna.lua`), and sends a summary of it to the
backend as you. It never reads anything outside the game's folder, and it signs in with the OAuth
device grant, so it never sees your password.

- `host/` (crate `ryna-agent`): the agent's own library, shared by every core: the sign-in and
  token refresh (`auth`), `config`, the HTTP client (`http`), the state folder and run lock
  (`state_dir`), the failure words (`fix`), the cancel handle (`cancel`), the clock (`util`), and
  `core`: the interface between the agent and a core (`CoreManifest`, read from a core's
  `core.json`, and the `Core` trait; see "Cores" below).
- `../cores/wow/core` (crate `ryna-wow-core`): the WoW core. A reader for the Lua the game writes
  (no Lua VM), the snapshot builder, install and account discovery, the pusher, the per-file state,
  the combat-log tail, the addon installer, and the status every front end shows; `WowCore` is its
  implementation of the interface. It re-exports the host's modules under their old names
  (`ryna_wow_core::auth` is `ryna_agent::auth`).
- `cli/` (crate `ryna`, binary `ryna`): a command line over it, **for developers and
  debugging**. People install the desktop app (`app/`), which runs the same core.
- `app/` (crate `ryna-wow-app` in `app/src-tauri`, product "Ryna"): the desktop app, Tauri 2
  with a Vue 3 window over the core. See "The app" below.
- `../cores/wow/fixtures/`: scrubbed, invented SavedVariables files and combat logs the tests read.
  Real captures never land there unscrubbed: `scripts/capture.sh` makes fixtures from them, and keeps the original in
  `captures/` (gitignored).

The wire format (the addon's table, the snapshot, the heartbeat) is a contract in words, kept with
the hosted side; the fixtures are what both sides' tests parse.

## Cores

Between the agent and a core there is one interface (`agent/host/src/core.rs`,
docs/ryna-agent-cores-mods.md section 4):

```rust
pub trait Core: Send + Sync {
    type Event: Serialize + Send + 'static;   // forwarded to the window as serialised
    type Error: std::error::Error + Send + Sync + 'static;
    fn manifest(&self) -> &CoreManifest;
    fn start(&self, ctx: &CoreContext, control: &CoreControl,
             events: &mut dyn FnMut(Self::Event)) -> Result<(), Self::Error>;  // until stopped
    fn activity(&self, event: &Self::Event) -> Activity;   // a row's phase, an error to show
    fn failed(&self, message: String, fix: Fix) -> Self::Event;
    fn status_rows(&self, ctx: &CoreContext) -> Vec<StatusRow>;   // label, value, chip
    fn heartbeat(&self, ctx: &CoreContext) -> CoreHeartbeat;
}
```

A core is started with a `CoreContext` (its state folder, a `TokenProvider`, and the game folder
the person chose) on a thread the agent gives it; `CoreControl::sync()` asks for a pass now and
`stop()` ends it, from any thread. The app's `get_status` and the CLI's `watch` drive the WoW core
this way; the rest of the CLI's developer commands (`status`, `run`, `parse`, `combat`,
`install-addon`) and the app's addon installer and full status still call the core directly in
stage 1, each marked `// stage 1: direct`. Sidecar cores (stage 4) are only a `runtime` kind in
the manifest so far.

## Build

Rust 1.89 or newer (CI uses 1.98).

```bash
cargo build --release -p ryna      # target/release/ryna
cargo test                             # the host, the WoW core and the CLI (the app: -p ryna-wow-app)
```

## How it runs

Nothing runs in the background. The agent syncs only while the companion app is open: the app
calls the core's watch loop in-process and stops it on quit. There are no launchd jobs, scheduled
tasks or run-at-login entries, and the CLI doesn't install any.

The core's API for a front end (the app uses all of it; the CLI is a thin printer over it):

```rust
use std::{path::Path, time::Duration};
use ryna_wow_core::{auth, cancel::CancelToken, config::Config, status};
use ryna_wow_core::watch::{watch_with_control, WatchControl, WatchEvent, WatchOptions};

let config = Config::resolve(None, None);

// Sign in, in two halves so it can be cancelled.
let login = auth::start_device_login(&config)?;      // DeviceLogin: user_code, verification_uri(_complete),
                                                      // expires_at, interval (device_code is never serialised)
let cancel = CancelToken::new();                      // cancel() from another thread: Err(Cancelled) at once
let session = auth::poll_device_login(&config, &login, &cancel, &mut |s| {
    // LoginStatus: pending, slowDown { interval }, approved
})?;                                                  // Session { signed_in_as }; tokens stored as `login` does

// What to show: installs (flavor, path, has_addon, addon_version, and `addon`: how it compares
// with the addon this front end would install), that addon's version (`addon_bundled`), accounts
// (label, key, savedAt, pushedAt, characters, last error), signed in and as whom, the last run.
let s = status::status(&config, None, Some(Path::new("cores/wow/addon")));

// The loop: one at a time, on its own thread.
let control = WatchControl::new();                    // keep a clone
let opts = WatchOptions::new(None, Duration::from_secs(60)).unwrap();
let handle = control.clone();
std::thread::spawn(move || watch_with_control(&config, &opts, &handle, &mut |event: WatchEvent| {
    // signedIn, installFound, accountFileChanged, pushing, pushed (with the API's counts),
    // error, heartbeat, passFinished. Every event is Serialize ({"event": "pushed", ...}).
}));
control.request_pass();                               // a pass now: wakes the wait, no restart
control.cancel();                                     // on quit: at once while waiting
```

`auth::Auth::login` is still the two halves in one call, and `watch::watch` takes a bare
`CancelToken`; `watch_with` is the loop over any transport, token store and discovery (the tests
use it). `runner::run_once` is one pass, for `ryna run`.

Errors name no command. Each carries a `Fix` (`signIn`, `update`, `network`, `other`) with neutral
words for it (`Fix::message`: "Sign in again", "Update the agent and addon", "Couldn't reach the
API", or for `other` the error itself); an `error` event has both, as `fix` and `advice`, beside the
raw `message`. The CLI appends its own hints (`run ryna login`); the app shows the words.
`addon_install::installed_addon_version` reads `## Version:` from the installed `Ryna.toc`; the
heartbeat sends it as each install's `addon`. `bundled_addon_version` reads the same line from the
copy a front end would install (the app's resources, the CLI's `--from`), and `addon_state`
compares the two as dotted integers (`notInstalled`, `current`, `older`, `newer`, `unknown`).

After every pass, successful or not, the agent rewrites `Interface/AddOns/Ryna/AgentStatus.lua`
in each install that has the addon (`RynaAgentStatus`: version, signed in and as whom, last push,
that install's accounts, and the last error as a message), so the addon can show it in game.

## Combat logs

While the loop runs it also follows the game's combat logs (`<install>/Logs/WoWCombatLog-*.txt`,
written while `/combatlog` is on), every 2 s between snapshot passes (`combat` in the core):

- **Reading**: every log written in the last 7 days, newest first, from where the last read
  stopped (`<state>/combat/state.json`, per log: the resume offset, the last fight queued, and the
  zone and owners known there). The first run reads what's on disk whole; a partial last line
  waits. Forever's layouts (checked against real logs): a 19-field advanced block (detected per
  line, not from the header's flag), damage `amount, baseAmount, overkill, school, resisted,
  blocked, absorbed, critical, glancing, crushing` plus a trailing `ST` on spells, heals `amount,
  baseAmount, overheal, absorbed, critical`.
- **Encounters**: a boss is `ENCOUNTER_START`…`END`; otherwise a pull opens on the first damage, heal
  or miss involving the logger's group (the affiliation flags: the logger, party, raid and their
  pets) and ends 8 s after the last one; in an instance it's `trash`, in the world pulls in one zone
  under 5 minutes apart are one `world` encounter. A stranger nearby only counts when they touch the
  group or a mob it engaged. Fights under 2 s or without the group doing or taking damage are
  dropped. A swing and its `SWING_DAMAGE_LANDED` twin count once; a lone one of either kind counts.
- **Uploads**: each fight's summary, gzipped digest (and, with raw logs on, its own lines) wait in
  `<state>/combat/queue/` until the API has them: `POST /v1/wow/agent/encounters` in batches, then
  `PUT …/:id/digest` and `…/raw`. Quitting ends what's open and queues it.

`ryna combat <file>` prints the fights a log holds (`--full` for each summary, `--digest <dir>`
for each digest's NDJSON). `ryna watch --no-digests --raw-logs --no-combat` set the same
choices the app's Settings do. Combat fixtures come from `scripts/capture.sh --combat <log>`.

## The app

`app/` is the product: the only thing a person installs (a `.dmg` on macOS, an NSIS installer on
Windows, an AppImage on Linux). It signs in with the device grant (it shows the code and opens the sign-in page), finds
the game, installs the bundled addon (`cores/wow/addon/Ryna`, carried as the resource `addon/Ryna`)
with every mod that's on (Chronicle, `mods/wow/chronicle/addon/Ryna_Chronicle`, carried beside it as
`addon/Ryna_Chronicle`; each mod's switch under the install, kept in the app's `settings.json` as
`mods`, installs or removes its folder), and runs the core's
watch loop on its own thread, showing each install's addon version, each account's last save and
push, and a log of what the loop did. It shows status, not charts: those live on the portal.

The window is a sidebar under the header (sign-in, Sync now) and the update banner: **Status**
(the default: the agent's card with the account, whether it's syncing, the last sync, the version
and Check for updates, then a card per core with the game and its addon, the accounts, the combat
log, fights in the last 12 hours and the last one, and a button to the core's page), then **one
entry per core** (today World of Warcraft: Forever), then the agent's **Settings** (open at login),
**Activity** (the session's log, which scrolls in its own panel, with Copy and Clear) and **About**
(versions, links, credits, licence and the third-party list). A core's page is a row of tabs:
**Game** (each install and its addon, with the install button), **Accounts**, **Combat** (the log's
state and file, what it saw last, fights waiting to upload, fights in the last 12 hours and the last
one), **Mods** (a toggle per mod and its state in each install) and the core's own **Settings**
(the game folder, fight digests, raw logs). Under 700 px wide the sidebar is icons with tooltips.
The chosen page and each core's tab are remembered in the webview's `localStorage`; the arrow
keys, Home and End move along the sidebar (up and down) and a core's tabs (left and right). The
window can open only ryna.io/wow and the sign-in page, so About's other links are text to copy.

It holds no logic of its own over the core's: `get_status` is the core's `status` plus the loop's
live state; `start_login` returns the core's `DeviceLogin` (and the link to open) and polls on a
thread with a cancel token it keeps, `cancel_login` fires it, and `login-finished` says how it
ended (`ok`, `cancelled`, `error`, `fix`); `sync_now` wakes the running loop
(`WatchControl::request_pass`) rather than restarting it. Errors show the core's words.

The window opens at 820×560 the first time and remembers the size and position it was left at
(`tauri-plugin-window-state`, in `window-state.json` in the app's config folder); it opens hidden
and shows once that frame has been checked against the monitors (`src-tauri/src/frame.rs`): a frame
whose centre is on no monitor, or that is larger than its monitor's work area, goes back to 820×560
centred on the primary monitor. Whether it was visible isn't remembered, since it's usually hidden
to the tray at quit.

**Nothing runs when it's closed.** Closing the window hides it to the tray (menu bar on macOS),
where it keeps syncing; Quit, in the tray menu, stops the loop and exits. There is no service,
launchd job or scheduled task. "Open Ryna when I log in" is a setting, off by default; on macOS
it's a login item (through System Events, which asks once for permission), not a launch agent; on
Linux it's a `.desktop` file in `~/.config/autostart`. When there is no tray to come back from
(Linux without an appindicator library, below), closing the window quits instead.

### Linux

The Linux build is `Ryna_<version>_amd64.AppImage`, x86_64 only and unsigned (`SHA256SUMS` on the
release is the check). It carries WebKitGTK and GTK, so it runs on any current distribution:
`chmod +x` it and run it. Arch and Omarchy are what it's tried for; Omarchy has no AUR by policy,
and the AppImage needs nothing from it. What the system provides:

- **The tray.** Tauri's tray is an appindicator, which the bar shows through StatusNotifierItem
  (SNI): Omarchy's bar has a tray, and so does Waybar's `tray` module and most desktops' panels.
  The library is loaded at run time: `libayatana-appindicator` (Arch), `libayatana-appindicator3-1`
  (Ubuntu). Without it the app starts without a tray and says so on stderr, and closing the
  window quits it (`try_build_tray` in `app/src-tauri/src/main.rs`).
- **The window under Hyprland.** The window class is the binary's name, `ryna-wow-app` (GTK's
  program name; the app sets no GTK application id, so it also matches the `StartupWMClass` in
  the AppImage's desktop file); `hyprctl clients` shows it. To float it:

  ```
  windowrulev2 = float, class:^(ryna-wow-app)$
  ```

  A blank white window is WebKitGTK's DMA-BUF renderer failing on that GPU (NVIDIA, mostly):
  run it with `WEBKIT_DISABLE_DMABUF_RENDERER=1`.
- **Opening at login.** "Open Ryna when I log in" writes `~/.config/autostart/Ryna.desktop`,
  which desktops read and Hyprland doesn't; there, `exec-once = <path to the AppImage>` in
  `hyprland.conf` does the same.
- **The game.** On Linux the game runs under Wine or Proton, so discovery (`default_roots` in
  `../cores/wow/core/src/discovery.rs`, mirrored in `../cores/wow/core.json`) looks in the usual
  prefixes' `drive_c/Program Files (x86)/World of Warcraft` and `Program Files/World of Warcraft`:
  `~/.wine`; Lutris, a prefix per game under `~/Games`; Bottles, the Flatpak
  (`~/.var/app/com.usebottles.bottles/data/bottles/bottles/*`) and a native install
  (`~/.local/share/bottles/bottles/*`); and Steam's Proton prefixes
  (`~/.steam/steam/steamapps/compatdata/*/pfx`, `~/.local/share/Steam/...`, and the Flatpak's).
  A prefix elsewhere: *Choose folder…* in the Game tab and pick the `World of Warcraft` folder
  under its `drive_c`. "The game is running" is `pgrep` for `Wow.exe` (any case).
- **The sign-in.** The token is kept in the Secret Service (`org.freedesktop.secrets`: gnome-keyring,
  KeePassXC with its Secret Service integration on, or KWallet), under the service `ryna`, with the
  kernel keyring (`keyutils`) in front of it as a cache. The client is `keyring`'s zbus one, pure
  Rust, so the build needs no D-Bus library. When nothing answers on the session bus (no provider
  installed, or it isn't running: Omarchy ships none), the kernel keyring is all there is and the
  sign-in lasts until a reboot; `ryna status` and the app's header say so. The choice is made once
  per process (`KeyringStore` in `host/src/auth.rs`), so after installing a provider, start Ryna
  again and sign in again. The state folder is `~/.local/share/io.ryna.app`, the app's settings
  `~/.config/io.ryna.app`.

Building it yourself needs Tauri's Ubuntu/Arch prerequisites (the webview and tray development
libraries): the `apt-get` line in `.github/workflows/agent.yml` is the Ubuntu list.

Run it in development (Node 24 and pnpm; the Rust toolchain above):

```bash
cd agent/app
pnpm install
pnpm tauri dev        # the window, with the frontend hot-reloading
pnpm test             # the frontend's tests; `cargo test -p ryna-wow-app` for the Rust side
pnpm tauri build      # the installer for this OS, in target/release/bundle/ (see "Updates" for what it adds)
```

A bare `cargo build` or `cargo test` at the root covers the host, the core and the CLI only
(`default-members`): the app needs `pnpm build` first (it embeds `app/dist`) and is built by name.

The device client id: like the CLI, the app reads `RYNA_CLIENT_ID` from its environment. A
release build can also bake one in: set `RYNA_CLIENT_ID` when compiling and the app uses it
unless the environment overrides it. No id is issued yet, so today's builds show "can't sign in
yet" until one is set. API and sign-in provider come from the same variables as the CLI's
(`RYNA_API`, `RYNA_ISSUER`). The app keeps its own setting (the chosen game folder) in
`settings.json` in its config folder (`io.ryna.app`); the sign-in and the push state are the
core's, shared with the CLI.

The icons in `app/src-tauri/icons` are placeholders: `pnpm icon` draws `app/app-icon.png` with a
small Node script and runs `tauri icon` on it. Replace the PNG and run `pnpm tauri icon app-icon.png`
for real artwork.

## Updates

The app checks this repository's GitHub Releases at launch and every 6 hours
(`plugins.updater.endpoints`: `releases/latest/download/latest.json`). When a newer release is
there, a banner says "Ryna x.y.z is available" with **Update and restart** (download, verify,
install, start the new version) and **Later** (hides it until a newer one, or a check from
Settings). Settings has **Check for updates**. Each finding goes to the activity log. The app
carries the addon, so an app update is also the addon update: the Installs list then offers it.

Every download is checked against the updater's public key, baked in at build time, and the
signature must name the version `latest.json` announces (`requireSignedVersion`, so an old signed
build can't be passed off as a new one). `pnpm tauri …` runs `app/scripts/tauri.mjs`, which merges
this build's additions into `tauri.conf.json` as `--config <json>` (`app/scripts/release-config.mjs`
decides them, and prints a `[ryna]` line for each):

| What exists | What the build gets |
|---|---|
| no `app/updater.pub` | an empty `pubkey`: the app never checks, and its button says "Updates aren't set up in this build" |
| `app/updater.pub` | the key, so the app can update itself |
| `app/updater.pub` and `TAURI_SIGNING_PRIVATE_KEY` | also `createUpdaterArtifacts` (and the `app` target on macOS): `Ryna.app.tar.gz` + `.sig`, the NSIS installer's `.sig`, or the AppImage's `.sig` |
| all six Azure values (below) | `bundle.windows.signCommand`: `artifact-signing-cli` signs the exe and the installer, before the updater signs the installer |

A plain `cargo build -p ryna-wow-app` reads `tauri.conf.json` alone, so it's always the build
without a key.

## Releasing

1. Bump the version everywhere: `version` in the root `Cargo.toml` (`[workspace.package]`, every
   crate takes it), `version` in `app/package.json`, `## Version:` in `cores/wow/addon/Ryna/Ryna.toc`, `version` in `cores/wow/core.json`,
   and each bundled mod's `version` in `mods/wow/<id>/mod.json` and `## Version:` in its addon's TOC
   (Chronicle: `mods/wow/chronicle/addon/Ryna_Chronicle/Ryna_Chronicle.toc`; also `ns.VERSION` in
   its `Mod.lua`, and the core addon's in `Core.lua`). A mod's `requires.core` should name the
   release that brought the mod API it needs.
   Run `cargo check` so `Cargo.lock` follows, then `node scripts/check-versions.mjs`, which fails
   and names each file when they disagree (CI runs it on every push too).
2. Commit, then tag and push the tag: `git tag -s vX.Y.Z && git push origin vX.Y.Z`.
3. `.github/workflows/release.yml` checks the tag against the files (a mismatch stops it before
   anything builds), then builds and attaches to one GitHub Release, with generated notes:
   - the CLI: `ryna-aarch64-apple-darwin`, `ryna-x86_64-apple-darwin`,
     `ryna-x86_64-pc-windows-msvc.exe`, `ryna-x86_64-unknown-linux-gnu` (each smoke-tested where
     the runner can run it);
   - the app: `Ryna_X.Y.Z_aarch64.dmg`, `Ryna_X.Y.Z_x64.dmg`, `Ryna_X.Y.Z_x64-setup.exe`,
     `Ryna_X.Y.Z_amd64.AppImage`;
   - the addon: `Ryna-X.Y.Z.zip` (`Ryna` and `Ryna_Chronicle` at its top level, as the app carries
     them, for a manual install or CurseForge);
   - with the updater key: `Ryna_X.Y.Z_<arch>.app.tar.gz` and their `.sig`s, the installer's and
     the AppImage's `.sig`, and `latest.json` (`scripts/latest-json.mjs`: `darwin-aarch64`,
     `darwin-x86_64`, `windows-x86_64`, `linux-x86_64`);
   - `SHA256SUMS` over all of it.
4. Every running app finds it within 6 hours (or at its next launch) and offers it. A tag with a
   `-` (`v0.3.0-rc.1`) is a pre-release, which is never "latest", so no app is offered it.

To try a release without publishing, run the `release` workflow by hand (Actions → release →
Run workflow): everything builds and lands as the `release-X.Y.Z` workflow artifact. From a
branch it is always a dry run; from a tag, untick `dry_run` to publish that tag.

### Secrets

All optional: without them the release is unsigned and can't update anyone, and still builds.
Maddison creates each one, keeps it in 1Password, and adds it as a repository secret in GitHub
(Settings → Secrets and variables → Actions). Each group is used only when all of it is set.

| Secret | What it does |
|---|---|
| `TAURI_SIGNING_PRIVATE_KEY` | The updater's private key (the file's contents). Signs `Ryna.app.tar.gz` and the installer so apps accept them as updates. Needs `app/updater.pub` committed. |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | That key's password (leave unset for a key made without one). |
| `APPLE_CERTIFICATE` | A Developer ID Application certificate, exported as `.p12` and base64-encoded. Signs the macOS app. |
| `APPLE_CERTIFICATE_PASSWORD` | The `.p12`'s export password. |
| `APPLE_SIGNING_IDENTITY` | The certificate's name, `Developer ID Application: <name> (<team id>)`. |
| `APPLE_ID` | The Apple account that notarizes (only with the three above). |
| `APPLE_PASSWORD` | An app-specific password for it (appleid.apple.com), not the account password. |
| `APPLE_TEAM_ID` | The developer team's id. |
| `AZURE_TRUSTED_SIGNING_ENDPOINT` | Azure Artifact Signing (formerly Trusted Signing): the account's region endpoint, e.g. `https://eus.codesigning.azure.net`. |
| `AZURE_TRUSTED_SIGNING_ACCOUNT` | The Artifact Signing account's name. |
| `AZURE_TRUSTED_SIGNING_PROFILE` | The certificate profile's name. |
| `AZURE_CLIENT_ID`, `AZURE_TENANT_ID`, `AZURE_CLIENT_SECRET` | An app registration with the "Artifact Signing Certificate Profile Signer" role on that account; `artifact-signing-cli` signs in as it. |

The macOS values are the Tauri bundler's own environment (the same names tauri-action passes):
it imports the certificate into a temporary keychain, signs, and notarizes and staples when the
`APPLE_ID` trio is there.

### The updater keypair

Made once, by Maddison, and never again: a new key means every installed app refuses every
later update, and the only way forward is a manual reinstall.

```bash
cargo tauri signer generate -w ~/.tauri/ryna.key     # asks for a password; writes ryna.key and ryna.key.pub
```

Store both halves (and the password) in 1Password. Put the private key's contents in the
`TAURI_SIGNING_PRIVATE_KEY` secret and the password in `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`. Copy
`ryna.key.pub` to `agent/app/updater.pub` and commit only that file. Then delete the local copies.

## Use (developers)

```bash
ryna install-addon --from cores/wow/addon    # copies Ryna and the on-by-default mods (Chronicle) into each Forever install's Interface/AddOns
ryna login                         # prints a link and a code; open it, check the code, allow it
ryna run                           # pushes every account whose Ryna.lua changed, then reports in
ryna status                        # installs, accounts, last pushes, signed in or not
                                       # (--from addon: say which installs need the addon updated)
ryna watch --interval 60           # `run` every minute until stopped
ryna parse path/to/Ryna.lua        # the snapshot JSON for one file, sent nowhere (--raw: the Lua as JSON)
ryna census id                     # this computer's submission ID
ryna census push --dry-run         # each account's sightings as census uploads (events, bytes, the ID), sent nowhere;
                                       # without --dry-run, signed and sent to /v1/wow/census with no sign-in
```

The game writes `Ryna.lua` when you log out or `/reload`, so that's when there is something new
to push. `run` exits 0 when everything went, 3 when you need to `login` again, 4 when the agent
and addon need updating (the file or the API is on a schema this agent doesn't know), and 1 for
anything else (offline, a file that doesn't parse). A file that failed stays unpushed and is
tried again next run.

The refresh token is kept in the OS keychain (service `ryna`), never in a file. What was
pushed is kept in `state.json` in the OS data folder (`~/Library/Application Support/io.ryna.app`
on macOS, `%APPDATA%\ryna\app\data` on Windows, `~/.local/share/io.ryna.app` on Linux).

## Settings (environment)

| Variable | Default | |
|---|---|---|
| `RYNA_CLIENT_ID` | a placeholder | The device client at the sign-in provider. **Not issued yet**: until it is, `login` refuses to start. |
| `RYNA_API` (`--api`) | `https://api.ryna.io` | The backend. Point it at a local API to test. Another API keeps its own device key (keychain account `device-key <api>`), so a test never uses the real submission ID. |
| `RYNA_ISSUER` (`--issuer`) | `https://auth.nyratech.dev` | The sign-in provider. A sign-in to another issuer is kept under its own keychain name. |
| `RYNA_RESOURCE` | `https://api.ryna.io/wow/ingest` | The audience the tokens are for (scope `wow:ingest`). |
| `RYNA_INSTALL` (`--install`) | the OS default | The game folder (`World of Warcraft`) or one product folder (`_forever_`). Defaults: `/Applications/World of Warcraft` on macOS, `C:\Program Files (x86)\World of Warcraft` and `C:\Program Files\World of Warcraft` on Windows, and on Linux those two inside the usual Wine and Proton prefixes (`drive_c` under `~/.wine`, Lutris's `~/Games/*`, Bottles' and Steam's; "Linux" above). |
| `RYNA_STATE_DIR` | the OS data folder | Where `state.json` and the run lock live. |
| `RYNA_HOST` | the host name | What the heartbeat calls this computer. |

## What it sends

For each account: a snapshot of `RynaDB` (characters, gear, bags and bank, quests, professions,
reputations, the guild roster and the addon's event journal), with the account folder's name
replaced by the first 16 hex digits of its sha256, so the backend never learns it. After each run
a heartbeat: the agent's version, the host name and OS, the install folders, each account's
key, label and last push time, and the submission ID.

### Submission ID

Sightings you contribute are traced to this ID, not to you. While you're signed in they're also
attributed to your account. The ID is the first 32 hex digits of the SHA-256 of this computer's
device key (an Ed25519 key minted on first run, its private half kept in the OS keychain, service
`ryna`, account `device-key`, never in a file), shown in groups of four in the app's WoW Settings
under Census and by `ryna status` and `ryna census id`. Every POST to the API carries it with the
public key and a signature over the exact body (`X-Ryna-Submission`, `X-Ryna-Key`,
`X-Ryna-Signature`; the API's docs/wow.md section 13). One install is one ID; deleting the keychain
entry mints a new one, which starts again at zero. A key the keychain holds but can't be read is
reported (the app's header shows Problem) and never quietly replaced. In this release only
`ryna census push`, run by hand, sends a census upload; the app keeps sending snapshots as before.
