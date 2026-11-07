//! `ryna`: the World of Warcraft: Forever companion agent's command line, for developers and
//! for debugging. People install the desktop app, which runs the same core while it is open.
//!
//! Finds the game's install(s), reads each account's `Ryna.lua` (the addon's SavedVariables),
//! and pushes a snapshot of it to the API as the person who signed in with `ryna login`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
// The agent's own parts (sign-in, config, HTTP, the state folder, the core interface).
use ryna_agent::auth::{self, Auth, AuthError, KeyringStore, LoginStatus, TokenStore};
use ryna_agent::cancel::CancelToken;
use ryna_agent::config::Config;
use ryna_agent::core::{Core, CoreContext, CoreControl, KeychainTokens};
use ryna_agent::fix::Fix;
use ryna_agent::http::HttpTransport;
use ryna_agent::identity::{self, DeviceKey, Ed25519Key};
use ryna_agent::state_dir;
use ryna_agent::util::{iso_utc, now_secs, SystemClock};
use ryna_agent::AGENT_VERSION;
// The WoW core: `watch` drives it through the interface; the developer commands below call it
// directly (each marked).
use ryna_wow_core::addon_install::AddonState;
use ryna_wow_core::discovery::{self, Discovery};
use ryna_wow_core::push::Api;
use ryna_wow_core::runner::{self, Outcome, RunError, RunOptions, RunReport};
use ryna_wow_core::watch::WatchEvent;
use ryna_wow_core::wow_core::WowCore;
use ryna_wow_core::{addon_install, lua, mods, snapshot};

#[derive(Parser)]
#[command(
    name = "ryna",
    version,
    about = "The World of Warcraft: Forever companion agent (developer command line; the desktop app is the way to install it)"
)]
struct Cli {
    /// The game folder (`World of Warcraft`) or one product folder in it (`_forever_`), in place
    /// of the default install locations.
    #[arg(long, global = true, env = "RYNA_INSTALL", value_name = "PATH")]
    install: Option<PathBuf>,
    /// The API origin (default https://api.ryna.io).
    #[arg(long, global = true, env = "RYNA_API", value_name = "ORIGIN")]
    api: Option<String>,
    /// The sign-in provider (default https://auth.nyratech.dev).
    #[arg(long, global = true, env = "RYNA_ISSUER", value_name = "URL")]
    issuer: Option<String>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Sign this agent in as you (opens nothing: prints a link and a code to check).
    Login,
    /// Forget the sign-in on this computer.
    Logout,
    /// Installs and accounts found, their files, the last pushes, and whether you're signed in.
    Status {
        /// The addon to compare each install's with (as for install-addon). Default: beside this
        /// binary.
        #[arg(long, value_name = "DIR")]
        from: Option<PathBuf>,
    },
    /// Push every account whose file changed since its last push, then report in. Exit codes:
    /// 0 fine, 1 failed, 3 sign in again, 4 update the agent and addon.
    Run {
        /// Push every account, changed or not.
        #[arg(long)]
        force: bool,
    },
    /// `run` every so often, until stopped (the loop the desktop app runs while it is open).
    Watch {
        /// Seconds between runs.
        #[arg(long, default_value_t = 60, value_parser = clap::value_parser!(u64).range(5..))]
        interval: u64,
        /// Leave the combat logs alone.
        #[arg(long)]
        no_combat: bool,
        /// Upload fights' summaries only, not their digests.
        #[arg(long)]
        no_digests: bool,
        /// Also upload each fight's raw log lines (kept 90 days on ryna.io).
        #[arg(long)]
        raw_logs: bool,
    },
    /// Copy the addon into each Forever install's Interface/AddOns.
    InstallAddon {
        /// The addon folder (or a folder holding Ryna/ or addon/Ryna/). Default: beside this binary.
        #[arg(long, value_name = "DIR")]
        from: Option<PathBuf>,
    },
    /// Print the fights found in one combat log as JSON, without sending anything (a debug view of
    /// what the agent would upload).
    Combat {
        file: PathBuf,
        /// Write each fight's digest NDJSON (uncompressed) into this folder, as `<key>.ndjson`.
        #[arg(long, value_name = "DIR")]
        digest: Option<PathBuf>,
        /// The flavor for each fight's key.
        #[arg(long, default_value = "wowf")]
        flavor: String,
        /// The logger's account key for each fight's key (what the agent finds from the addon's
        /// files).
        #[arg(long, default_value = "")]
        account_key: String,
        /// Print every fight's whole summary, not just what it is.
        #[arg(long)]
        full: bool,
    },
    /// The census without an account: this computer's submission ID, and sending each account's
    /// sightings to the census by hand (stage (a); the app doesn't send them yet).
    Census {
        #[command(subcommand)]
        command: CensusCommand,
    },
    /// Print the snapshot JSON for one SavedVariables file, without sending anything.
    Parse {
        file: PathBuf,
        /// The parsed Lua as JSON instead of the snapshot.
        #[arg(long)]
        raw: bool,
        /// The WTF account folder name to hash for account.key (default: read from the path).
        #[arg(long, value_name = "NAME")]
        account: Option<String>,
    },
}

#[derive(Subcommand)]
enum CensusCommand {
    /// Print this computer's submission ID (minting the device key on first use).
    Id,
    /// Send every account's sightings (the `seen` events, less your own characters) to
    /// /v1/wow/census, signed by the device key, with no sign-in. Use the global --install to
    /// point at the game.
    Push {
        /// Build the uploads and say what they'd be (events, bytes, the ID), sending nothing.
        #[arg(long)]
        dry_run: bool,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match dispatch(cli) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("ryna: {e:#}");
            ExitCode::from(1)
        }
    }
}

fn dispatch(cli: Cli) -> Result<ExitCode> {
    let config = Config::resolve(cli.api.clone(), cli.issuer.clone());
    match cli.command {
        Command::Login => login(&config),
        Command::Logout => logout(&config),
        Command::Status { from } => status(&config, cli.install.as_deref(), from.as_deref()),
        Command::Run { force } => {
            let code = run(&config, cli.install.as_deref(), &RunOptions { force })?;
            Ok(ExitCode::from(code))
        }
        Command::Watch {
            interval,
            no_combat,
            no_digests,
            raw_logs,
        } => {
            let combat = ryna_wow_core::combat::CombatOptions {
                enabled: !no_combat,
                digests: !no_digests,
                raw: raw_logs,
            };
            watch(&config, cli.install.as_deref(), interval, combat)
        }
        Command::InstallAddon { from } => install_addon(cli.install.as_deref(), from.as_deref()),
        Command::Census { command } => match command {
            CensusCommand::Id => census_id(&config),
            CensusCommand::Push { dry_run } => {
                census_push(&config, cli.install.as_deref(), dry_run)
            }
        },
        Command::Parse { file, raw, account } => parse(&file, raw, account.as_deref()),
        Command::Combat {
            file,
            digest,
            flavor,
            account_key,
            full,
        } => combat(&file, digest.as_deref(), &flavor, &account_key, full),
    }
}

/// The command that fixes a failure: the core's errors name none, so the CLI adds its own.
fn hint(fix: Fix) -> Option<&'static str> {
    match fix {
        Fix::SignIn => Some("run `ryna login`"),
        Fix::Update => Some("update ryna, then run `ryna install-addon`"),
        Fix::Network | Fix::Other => None,
    }
}

/// `message`, with the command that fixes it when there is one.
fn with_hint(message: &str, fix: Fix) -> String {
    match hint(fix) {
        Some(h) => format!("{message}: {h}"),
        None => message.to_string(),
    }
}

fn auth_error(e: AuthError) -> anyhow::Error {
    anyhow::Error::msg(with_hint(&e.to_string(), e.fix()))
}

fn login(config: &Config) -> Result<ExitCode> {
    let login = auth::start_device_login(config).map_err(auth_error)?;
    println!("To sign this agent in, open {}", login.link());
    println!(
        "and check the code there is {}, then allow it. Waiting…",
        login.user_code
    );
    let session = auth::poll_device_login(config, &login, &CancelToken::new(), &mut |s| {
        if let LoginStatus::SlowDown { interval } = s {
            println!("The sign-in provider asked for slower polling: every {interval} s.");
        }
    })
    .map_err(auth_error)?;
    match session.signed_in_as {
        Some(who) => println!("Signed in as {who}. `ryna run` now pushes your accounts."),
        None => println!("Signed in. `ryna run` now pushes your accounts as you."),
    }
    if KeyringStore::for_issuer(&config.issuer).until_reboot() {
        println!("{UNTIL_REBOOT}");
    }
    Ok(ExitCode::SUCCESS)
}

/// Linux with no Secret Service: the sign-in is in the kernel keyring only.
const UNTIL_REBOOT: &str = "This sign-in lasts until the computer restarts: no Secret Service is \
     running to keep it. Install gnome-keyring or KeePassXC (and let it start with your session), \
     then sign in again.";

fn logout(config: &Config) -> Result<ExitCode> {
    let http = HttpTransport::new().map_err(anyhow::Error::msg)?;
    let store = KeyringStore::for_issuer(&config.issuer);
    Auth::new(config, &http, &store, &SystemClock).logout()?;
    println!("Signed out on this computer.");
    Ok(ExitCode::SUCCESS)
}

fn when(secs: Option<i64>) -> String {
    secs.map_or_else(|| "never".to_string(), iso_utc)
}

fn status(config: &Config, install: Option<&Path>, from: Option<&Path>) -> Result<ExitCode> {
    println!("ryna {AGENT_VERSION}");
    println!("API {}  sign-in {}", config.api, config.issuer);
    if !config.client_id_issued() {
        println!(
            "Device client: not issued yet (set RYNA_CLIENT_ID); `login` can't run until it is."
        );
    }
    let beside = addon_install::beside_binary();
    // stage 1: direct; the core interface takes this in stage 2 (the full status, not just its rows).
    let s = ryna_wow_core::status::status(config, install, from.or(beside.as_deref()));
    match &s.addon_bundled {
        Some(v) => println!("Addon to install: {v}"),
        None if from.is_some() => println!("No versioned addon in the --from folder."),
        None => {}
    }
    match (&s.keychain_error, s.signed_in, &s.signed_in_as) {
        (Some(e), _, _) => println!("Couldn't read the keychain: {e}"),
        (None, true, Some(who)) => println!("Signed in as {who}."),
        (None, true, None) => println!("Signed in (a refresh token is in the keychain)."),
        (None, false, _) => println!("Not signed in: run `ryna login`."),
    }
    if s.signed_in && s.sign_in_until_reboot {
        println!("{UNTIL_REBOOT}");
    }
    // Read, never minted here: the first run, watch or census command makes it.
    match identity::stored_device_key(config) {
        Ok(Some(k)) => println!("Submission ID: {}", k.display_id()),
        Ok(None) => println!("Submission ID: none yet (the first run makes one)"),
        Err(e) => println!("Submission ID: none ({e})"),
    }

    if s.installs.is_empty() {
        println!("\nNo installs found. Point at the game with --install <folder>.");
    } else {
        println!("\nInstalls:");
        for i in &s.installs {
            let mut addon = match (&i.addon_version, i.has_addon) {
                (Some(v), _) => format!("addon {v}"),
                (None, true) => "addon installed (no version)".to_string(),
                (None, false) => "addon not installed".to_string(),
            };
            if let Some(b) = &s.addon_bundled {
                match i.addon {
                    AddonState::Older | AddonState::Unknown => {
                        addon.push_str(&format!(" ({b} available: `ryna install-addon`)"))
                    }
                    AddonState::Newer => addon.push_str(&format!(" (newer than {b})")),
                    AddonState::NotInstalled | AddonState::Current => {}
                }
            }
            println!(
                "  {}  flavor {}  {addon}",
                i.path,
                i.flavor.as_deref().unwrap_or("unknown")
            );
        }
    }
    if s.accounts.is_empty() {
        println!("\nNo Ryna.lua yet: install the addon (`ryna install-addon`), log in to the game, then /reload or log out.");
    } else {
        println!("\nAccounts:");
    }
    for a in &s.accounts {
        println!("  {}  ({})", a.label, a.path);
        println!(
            "    key {}  file written {}  last pushed {}",
            a.key,
            when(a.saved_at),
            when(a.pushed_at)
        );
        if let Some(n) = a.characters {
            println!("    characters {n}");
        }
        if let Some(e) = &a.last_error {
            println!("    last error: {e}");
        }
        // What the client said about itself: how the real Interface number and name split are learned.
        if let Ok(text) = std::fs::read(&a.path) {
            if let Ok(parsed) = lua::parse(&String::from_utf8_lossy(&text)) {
                if let Some(client) = snapshot::client_info(&parsed) {
                    println!("    client: {}", snapshot::client_summary(client));
                }
            }
        }
    }
    if let Some(r) = &s.last_run {
        let outcome = if r.ok {
            "ok".to_string()
        } else {
            let error = r.error.as_deref().unwrap_or("");
            format!("failed: {}", with_hint(error, r.fix.unwrap_or(Fix::Other)))
        };
        println!("\nLast run {}: {outcome}", iso_utc(r.at));
    }
    let c = &s.combat;
    match &c.file {
        Some(file) => {
            println!(
                "\nCombat log: {file}, {}{}, last event {}",
                if c.active { "active" } else { "quiet" },
                match c.advanced {
                    Some(true) => ", advanced logging on",
                    Some(false) => ", advanced logging off",
                    None => "",
                },
                when(c.last_event_at)
            );
            println!(
                "  fights uploaded in the last 12 h: {}  waiting: {}",
                c.fights_tonight, c.pending
            );
            if let Some(f) = &c.last_fight {
                println!(
                    "  last fight: {} ({}), uploaded {}",
                    ryna_wow_core::combat::describe(
                        f.zone.as_deref(),
                        f.name.as_deref(),
                        f.duration_ms,
                        f.players
                    ),
                    f.kind,
                    iso_utc(f.at)
                );
            }
            if let Some(e) = &c.last_error {
                println!("  last error: {e}");
            }
        }
        None => println!("\nCombat log: none read yet (type /combatlog in the game to start one)."),
    }
    if let Some(d) = &s.state_dir {
        println!("State: {d}");
    }
    Ok(ExitCode::SUCCESS)
}

fn print_report(r: &RunReport) {
    let stamp = iso_utc(now_secs());
    for a in &r.accounts {
        for w in &a.warnings {
            println!("{stamp} {}: warning: {w}", a.label);
        }
        match &a.outcome {
            Outcome::Unchanged => println!("{stamp} {}: unchanged", a.label),
            Outcome::Pushed { characters, data } => {
                let journal = data
                    .get("journal")
                    .map(|j| format!(", journal {j}"))
                    .unwrap_or_default();
                println!(
                    "{stamp} {}: pushed {characters} character(s){journal}",
                    a.label
                );
            }
            Outcome::Failed(e) => println!("{stamp} {}: failed: {e}", a.label),
            Outcome::Skipped => println!("{stamp} {}: skipped", a.label),
        }
    }
    match &r.heartbeat {
        Ok(()) => println!("{stamp} heartbeat sent"),
        Err(e) => println!("{stamp} heartbeat: {e}"),
    }
}

/// One run; the exit code.
fn run(config: &Config, install: Option<&Path>, opts: &RunOptions) -> Result<u8> {
    let dir =
        state_dir::state_dir().context("no place to keep the agent's state: set RYNA_STATE_DIR")?;
    // stage 1: direct; the core interface takes this in stage 2 (one pass, then exit).
    let found: Discovery = discovery::discover(install);
    if found.installs.is_empty() {
        eprintln!("no installs found: point at the game with --install <folder>");
    }
    let http = HttpTransport::new().map_err(anyhow::Error::msg)?;
    let store = KeyringStore::for_issuer(&config.issuer);
    let clock = SystemClock;
    let auth = Auth::new(config, &http, &store, &clock);
    let key = device_key_or_warn(config);
    let api = Api::new(
        &config.api,
        &http,
        &auth,
        key.as_ref().map(|k| k as &dyn DeviceKey),
        &clock,
    );
    match runner::run_once(&found, &dir, &api, &clock, opts) {
        Ok(report) => {
            print_report(&report);
            if let Some(h) = report.failure.and_then(hint) {
                eprintln!("To fix it: {h}");
            }
            Ok(report.failure.map_or(0, |f| f.exit_code() as u8))
        }
        Err(RunError::Busy(busy)) => {
            eprintln!("another ryna run is in progress ({busy}); not starting a second");
            Ok(1)
        }
        Err(e) => Err(e.into()),
    }
}

/// The device key, or None with a warning: the posts then go unsigned (fine everywhere but the
/// census).
fn device_key_or_warn(config: &Config) -> Option<Ed25519Key> {
    match identity::device_key(config) {
        Ok(k) => Some(k),
        Err(e) => {
            eprintln!("warning: {e}; posting unsigned");
            None
        }
    }
}

fn census_id(config: &Config) -> Result<ExitCode> {
    let key = identity::device_key(config)?;
    println!("{}", key.display_id());
    Ok(ExitCode::SUCCESS)
}

/// `ryna census push`: each account's sightings as signed uploads, sent with no token.
fn census_push(config: &Config, install: Option<&Path>, dry_run: bool) -> Result<ExitCode> {
    use ryna_wow_core::census;
    let key = identity::device_key(config)?;
    // stage 1: direct; stage (c) gives the core a "contribute without an account" mode.
    let found = discovery::discover(install);
    if found.accounts.is_empty() {
        bail!("no Ryna.lua found: point at the game with --install <folder>");
    }
    println!("Submission ID: {}", key.display_id());
    let http = HttpTransport::new().map_err(anyhow::Error::msg)?;
    let store = KeyringStore::for_issuer(&config.issuer);
    let clock = SystemClock;
    let auth = Auth::new(config, &http, &store, &clock);
    let api = Api::new(&config.api, &http, &auth, Some(&key), &clock);
    let mut failed = false;
    for acct in &found.accounts {
        let built =
            match snapshot::read_file(&acct.path, &acct.account, None, acct.flavor.as_deref()) {
                Ok(b) => b,
                Err(e) => {
                    eprintln!("{}: {e}", acct.path.display());
                    failed = true;
                    continue;
                }
            };
        let label = built.snapshot.account.label.clone();
        let uploads = census::uploads(&built.snapshot, now_secs());
        let events: usize = uploads.iter().map(|u| u.events.len()).sum();
        let bytes: usize = uploads.iter().map(|u| u.body().len()).sum();
        println!(
            "{label}: {events} sighting(s) in {} upload(s), {bytes} bytes",
            uploads.len()
        );
        if dry_run {
            continue;
        }
        for (i, u) in uploads.iter().enumerate() {
            match api.post_census(u) {
                Ok(data) => println!(
                    "  upload {}: stored {}, repeated {}",
                    i + 1,
                    data.get("stored").unwrap_or(&serde_json::Value::Null),
                    data.get("repeated").unwrap_or(&serde_json::Value::Null)
                ),
                Err(e) => {
                    eprintln!("  upload {}: {e}", i + 1);
                    failed = true;
                }
            }
        }
    }
    if dry_run {
        println!("Dry run: nothing was sent.");
    }
    Ok(if failed {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    })
}

/// The core's watch loop (the one the desktop app drives), printing its events.
fn watch(
    config: &Config,
    install: Option<&Path>,
    interval: u64,
    combat: ryna_wow_core::combat::CombatOptions,
) -> Result<ExitCode> {
    let ctx = CoreContext {
        state_dir: state_dir::state_dir()
            .context("no place to keep the agent's state: set RYNA_STATE_DIR")?,
        tokens: Arc::new(KeychainTokens::new(config.clone())),
        device_key: device_key_or_warn(config).map(|k| Arc::new(k) as Arc<dyn DeviceKey>),
        game_folder: install.map(Path::to_path_buf),
    };
    let core = WowCore::new(Duration::from_secs(interval)).with_combat(combat);
    println!("Watching every {interval} s (Ctrl-C to stop).");
    core.start(&ctx, &CoreControl::new(), &mut |event| {
        let stamp = iso_utc(now_secs());
        match &event {
            WatchEvent::SignedIn { signed_in: false } => {
                println!("{stamp} not signed in: run `ryna login`")
            }
            WatchEvent::SignedIn { .. } => {}
            WatchEvent::InstallFound { path, flavor } => {
                println!(
                    "{stamp} install {path} (flavor {})",
                    flavor.as_deref().unwrap_or("unknown")
                )
            }
            WatchEvent::AccountFileChanged { label, .. } => {
                println!("{stamp} {label}: file changed")
            }
            WatchEvent::Pushing { label, .. } => println!("{stamp} {label}: pushing"),
            WatchEvent::Pushed {
                label,
                characters,
                result,
                ..
            } => {
                let journal = result
                    .get("journal")
                    .map(|j| format!(", journal {j}"))
                    .unwrap_or_default();
                println!("{stamp} {label}: pushed {characters} character(s){journal}");
            }
            WatchEvent::Error {
                label,
                message,
                fix,
                ..
            } => {
                println!(
                    "{stamp} {}error: {}",
                    label.as_ref().map(|l| format!("{l}: ")).unwrap_or_default(),
                    with_hint(message, *fix)
                )
            }
            WatchEvent::Heartbeat { ok: true, .. } => {}
            WatchEvent::Heartbeat { error, .. } => println!(
                "{stamp} heartbeat: {}",
                error.as_deref().unwrap_or("failed")
            ),
            WatchEvent::PassFinished { .. } => {}
            WatchEvent::CombatLogFound { file, .. } => println!("{stamp} combat log {file}"),
            WatchEvent::FightQueued {
                kind,
                zone,
                name,
                duration_ms,
                players,
                ..
            } => println!(
                "{stamp} fight ({kind}) {}: queued",
                ryna_wow_core::combat::describe(
                    zone.as_deref(),
                    name.as_deref(),
                    *duration_ms,
                    *players
                )
            ),
            WatchEvent::FightUploaded {
                zone,
                name,
                duration_ms,
                players,
                id,
                merged,
                ..
            } => println!(
                "{stamp} fight {}: uploaded (encounter {id}{})",
                ryna_wow_core::combat::describe(
                    zone.as_deref(),
                    name.as_deref(),
                    *duration_ms,
                    *players
                ),
                if *merged { ", merged" } else { "" }
            ),
            WatchEvent::FightFailed {
                fight,
                message,
                fix,
                ..
            } => println!("{stamp} fight {fight}: {}", with_hint(message, *fix)),
        }
    })?;
    Ok(ExitCode::SUCCESS)
}

fn install_addon(install: Option<&Path>, from: Option<&Path>) -> Result<ExitCode> {
    // The core's addon, then its mods as they are by default (Chronicle on): the command line keeps
    // no settings of its own, so a mod turned off in the app is the app's to keep off.
    let source = match from {
        Some(dir) => addon_install::resolve_source(dir)?,
        None => addon_install::beside_binary().context(
            "no addon beside this binary: pass --from <dir> (the folder holding Ryna.toc)",
        )?,
    };
    let found = discovery::discover(install);
    // Auto-discovered: Forever installs only (the addon is tagged for it). Named with --install:
    // whatever is there.
    let targets: Vec<_> = found
        .installs
        .iter()
        .filter(|i| install.is_some() || i.flavor.as_deref() == Some("wowf"))
        .collect();
    if targets.is_empty() {
        bail!("no Forever install found: point at the game with --install <folder>");
    }
    let version = addon_install::bundled_addon_version(&source);
    for i in targets {
        let to = addon_install::install(&source, i)?;
        match &version {
            Some(v) => println!("Installed addon {v} into {}", to.display()),
            None => println!("Installed the addon in {}", to.display()),
        }
        match mods::apply(i, &source, &BTreeMap::new(), None) {
            Ok(changes) => {
                for c in changes {
                    match c {
                        mods::ModChange::Installed {
                            name,
                            path,
                            version,
                            ..
                        } => println!(
                            "Installed the {name} mod{} into {path}",
                            version.map(|v| format!(" {v}")).unwrap_or_default()
                        ),
                        mods::ModChange::Removed { name, path, .. } => {
                            println!("Removed the {name} mod from {path}")
                        }
                    }
                }
            }
            Err(e) => eprintln!(
                "warning: {e:#}. Without it nothing about your characters is recorded: pass --from \
                 the folder holding both Ryna and Ryna_Chronicle (the release's addon zip)."
            ),
        }
    }
    println!("Restart the game (or /reload) to load it.");
    Ok(ExitCode::SUCCESS)
}

fn parse(file: &Path, raw: bool, account: Option<&str>) -> Result<ExitCode> {
    // stage 1: direct; the core interface takes this in stage 2 (a debug view of the core's reader).
    let text = std::fs::read(file).with_context(|| format!("reading {}", file.display()))?;
    let parsed =
        lua::parse(&String::from_utf8_lossy(&text)).with_context(|| file.display().to_string())?;
    if raw {
        println!("{}", serde_json::to_string_pretty(&parsed)?);
        if let Some(client) = snapshot::client_info(&parsed) {
            eprintln!("client: {}", snapshot::client_summary(client));
        }
        return Ok(ExitCode::SUCCESS);
    }
    let folder = account
        .map(str::to_string)
        .or_else(|| discovery::account_folder_of(file))
        .unwrap_or_else(|| "unknown".to_string());
    let built = snapshot::read_file(file, &folder, None, None)?;
    for w in &built.warnings {
        eprintln!("warning: {w}");
    }
    if let Some(client) = snapshot::client_info(&parsed) {
        eprintln!("client: {}", snapshot::client_summary(client));
    }
    println!("{}", serde_json::to_string_pretty(&built.snapshot)?);
    Ok(ExitCode::SUCCESS)
}

/// `ryna combat <file>`: the fights in a log, as the agent would cut and summarise them.
fn combat(
    file: &Path,
    digest: Option<&Path>,
    flavor: &str,
    account_key: &str,
    full: bool,
) -> Result<ExitCode> {
    // stage 1: direct; the core interface takes this in stage 2 (a debug view of the core's parser).
    use ryna_wow_core::combat::{self, segment, CombatOptions};
    let bytes = std::fs::read(file).with_context(|| format!("reading {}", file.display()))?;
    let text = String::from_utf8_lossy(&bytes);
    let (fights, seg) = segment::segment_text(&text, false);
    let name = file
        .file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
    let opts = CombatOptions::default();
    if let Some(dir) = digest {
        std::fs::create_dir_all(dir).with_context(|| dir.display().to_string())?;
    }
    let mut out = Vec::new();
    for f in &fights {
        let s = combat::summarize(f, flavor, account_key, &name, &opts);
        if let Some(dir) = digest {
            let path = dir.join(format!("{}.ndjson", s.key));
            std::fs::write(&path, f.digest().to_ndjson())
                .with_context(|| path.display().to_string())?;
            eprintln!("digest: {}", path.display());
        }
        let mut v = if full {
            serde_json::to_value(&s)?
        } else {
            serde_json::json!({
                "key": s.key, "kind": s.kind, "name": s.name, "zone": s.zone,
                "instance": s.instance, "startedAt": s.started_at, "endedAt": s.ended_at,
                "durationMs": s.duration_ms, "success": s.success, "realmId": s.realm_id,
                "players": s.participants.len(), "actors": s.actors.len(),
                "abilities": s.abilities.len(), "deaths": s.deaths.len(),
                "events": s.source.events, "advanced": s.source.advanced,
                "digestEvents": f.digest.len(),
            })
        };
        v["pulls"] = serde_json::json!(f.pulls);
        out.push(v);
    }
    let report = serde_json::json!({
        "file": name,
        "fights": out,
        "dropped": seg.dropped,
        "subevents": seg.subevents,
        "unreadable": seg.unreadable,
        "logger": seg.ctx.logger.is_some(),
        "realmId": seg.ctx.realm_id(),
    });
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(ExitCode::SUCCESS)
}
