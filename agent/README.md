# ryna-wow: the agent

A small program that runs on your computer, finds World of Warcraft: Forever, reads what the Ryna
addon saved (`WTF/Account/<account>/SavedVariables/Ryna.lua`), and sends a summary of it to the
backend as you. It never reads anything outside the game's folder, and it signs in with the OAuth
device grant, so it never sees your password.

- `core/` (crate `ryna-wow-core`): the library. A reader for the Lua the game writes (no Lua VM),
  the snapshot builder, install and account discovery, the sign-in and token refresh, the pusher,
  the per-file state, and the addon installer.
- `cli/` (crate `ryna-wow`, binary `ryna-wow`): a command line over it, **for developers and
  debugging**. People install the desktop app (`app/`), which runs the same core.
- `app/` (crate `ryna-wow-app` in `app/src-tauri`, product "Ryna"): the desktop app, Tauri 2
  with a Vue 3 window over the core. See "The app" below.
- `fixtures/`: scrubbed, invented SavedVariables files the tests read. Real captures never land
  here unscrubbed: `scripts/capture.sh` makes fixtures from them, and keeps the original in
  `captures/` (gitignored).

The wire format (the addon's table, the snapshot, the heartbeat) is a contract in words, kept with
the hosted side; the fixtures are what both sides' tests parse.

## Build

Rust 1.89 or newer (CI uses 1.98).

```bash
cargo build --release -p ryna-wow      # target/release/ryna-wow
cargo test                             # every crate
```

## How it runs

Nothing runs in the background. The agent syncs only while the companion app is open: the app
calls the core's watch loop in-process and stops it on quit. There are no launchd jobs, scheduled
tasks or run-at-login entries, and the CLI doesn't install any.

The loop, for the app (or any front end):

```rust
use ryna_wow_core::watch::{watch, CancelToken, WatchEvent, WatchOptions};

let cancel = CancelToken::new();          // clone it; cancel() on quit stops the loop at once
let opts = WatchOptions::new(None, std::time::Duration::from_secs(60)).unwrap();
let config = ryna_wow_core::config::Config::resolve(None, None);
std::thread::spawn(move || watch(&config, &opts, &cancel, &mut |event: WatchEvent| {
    // signedIn, installFound, accountFileChanged, pushing, pushed (with the API's counts),
    // error (with what fixes it: signIn, update, other), heartbeat, passFinished.
    // Every event is Serialize ({"event": "pushed", ...}): forward it to the webview as is.
}));
```

`watch_with` is the same over any transport, token store and discovery (the tests use it).
`runner::run_once` is one pass, for `ryna-wow run`.

After every pass, successful or not, the agent rewrites `Interface/AddOns/Ryna/AgentStatus.lua`
in each install that has the addon (`RynaAgentStatus`: version, signed in and as whom, last push,
that install's accounts, and the last error as a message), so the addon can show it in game.

## The app

`app/` is the product: the only thing a person installs (a `.dmg` on macOS, an NSIS installer on
Windows). It signs in with the device grant (it shows the code and opens the sign-in page), finds
the game, installs the bundled addon (`addon/Ryna`, carried as a resource), and runs the core's
watch loop on its own thread, showing each account's last save and push and a log of what the
loop did. It shows status, not charts: those live on the portal.

**Nothing runs when it's closed.** Closing the window hides it to the tray (menu bar on macOS),
where it keeps syncing; Quit, in the tray menu, stops the loop and exits. There is no service,
launchd job or scheduled task. "Open Ryna when I log in" is a setting, off by default; on macOS
it's a login item (through System Events, which asks once for permission), not a launch agent.

Run it in development (Node 24 and pnpm; the Rust toolchain above):

```bash
cd agent/app
pnpm install
pnpm tauri dev        # the window, with the frontend hot-reloading
pnpm test             # the frontend's tests; `cargo test -p ryna-wow-app` for the Rust side
pnpm tauri build      # the installer for this OS, in target/release/bundle/
```

A bare `cargo build` or `cargo test` at the root covers the core and the CLI only
(`default-members`): the app needs `pnpm build` first (it embeds `app/dist`) and is built by name.

The device client id: like the CLI, the app reads `RYNA_WOW_CLIENT_ID` from its environment. A
release build can also bake one in: set `RYNA_WOW_CLIENT_ID` when compiling and the app uses it
unless the environment overrides it. No id is issued yet, so today's builds show "can't sign in
yet" until one is set. API and sign-in provider come from the same variables as the CLI's
(`RYNA_WOW_API`, `RYNA_WOW_ISSUER`). The app keeps its own setting (the chosen game folder) in
`settings.json` in its config folder (`io.ryna.wow`); the sign-in and the push state are the
core's, shared with the CLI.

The icons in `app/src-tauri/icons` are placeholders: `pnpm icon` draws `app/app-icon.png` with a
small Node script and runs `tauri icon` on it. Replace the PNG and run `pnpm tauri icon app-icon.png`
for real artwork.

## Use (developers)

```bash
ryna-wow install-addon --from addon    # copies addon/Ryna into each Forever install's Interface/AddOns
ryna-wow login                         # prints a link and a code; open it, check the code, allow it
ryna-wow run                           # pushes every account whose Ryna.lua changed, then reports in
ryna-wow status                        # installs, accounts, last pushes, signed in or not
ryna-wow watch --interval 60           # `run` every minute until stopped
ryna-wow parse path/to/Ryna.lua        # the snapshot JSON for one file, sent nowhere (--raw: the Lua as JSON)
```

The game writes `Ryna.lua` when you log out or `/reload`, so that's when there is something new
to push. `run` exits 0 when everything went, 3 when you need to `login` again, 4 when the agent
and addon need updating (the file or the API is on a schema this agent doesn't know), and 1 for
anything else (offline, a file that doesn't parse). A file that failed stays unpushed and is
tried again next run.

The refresh token is kept in the OS keychain (service `ryna-wow`), never in a file. What was
pushed is kept in `state.json` in the OS data folder (`~/Library/Application Support/io.ryna.ryna-wow`
on macOS, `%APPDATA%\ryna\ryna-wow\data` on Windows).

## Settings (environment)

| Variable | Default | |
|---|---|---|
| `RYNA_WOW_CLIENT_ID` | a placeholder | The device client at the sign-in provider. **Not issued yet**: until it is, `login` refuses to start. |
| `RYNA_WOW_API` (`--api`) | `https://api.ryna.io` | The backend. Point it at a local API to test. |
| `RYNA_WOW_ISSUER` (`--issuer`) | `https://auth.nyratech.dev` | The sign-in provider. A sign-in to another issuer is kept under its own keychain name. |
| `RYNA_WOW_RESOURCE` | `https://api.ryna.io/wow/ingest` | The audience the tokens are for (scope `wow:ingest`). |
| `RYNA_WOW_INSTALL` (`--install`) | the OS default | The game folder (`World of Warcraft`) or one product folder (`_forever_`). Defaults: `/Applications/World of Warcraft` on macOS, `C:\Program Files (x86)\World of Warcraft` and `C:\Program Files\World of Warcraft` on Windows. |
| `RYNA_WOW_STATE_DIR` | the OS data folder | Where `state.json` and the run lock live. |
| `RYNA_WOW_HOST` | the host name | What the heartbeat calls this computer. |

## What it sends

For each account: a snapshot of `RynaDB` (characters, gear, bags and bank, quests, professions,
reputations, the guild roster and the addon's event journal), with the account folder's name
replaced by the first 16 hex digits of its sha256, so the backend never learns it. After each run
a heartbeat: the agent's version, the host name and OS, the install folders, and each account's
key, label and last push time.
