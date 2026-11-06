//! `ryna-wow`: the World of Warcraft: Forever companion agent's command line, for developers and
//! for debugging. People install the desktop app, which runs the same core while it is open.
//!
//! Finds the game's install(s), reads each account's `Ryna.lua` (the addon's SavedVariables),
//! and pushes a snapshot of it to the API as the person who signed in with `ryna-wow login`.

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use ryna_wow_core::addon_install::AddonState;
use ryna_wow_core::auth::{self, Auth, AuthError, KeyringStore, LoginStatus};
use ryna_wow_core::config::Config;
use ryna_wow_core::discovery::{self, Discovery};
use ryna_wow_core::fix::Fix;
use ryna_wow_core::http::HttpTransport;
use ryna_wow_core::push::Api;
use ryna_wow_core::runner::{self, Outcome, RunError, RunOptions, RunReport};
use ryna_wow_core::state;
use ryna_wow_core::util::{iso_utc, now_secs, SystemClock};
use ryna_wow_core::watch::{CancelToken, WatchEvent, WatchOptions};
use ryna_wow_core::{addon_install, lua, snapshot, AGENT_VERSION};

#[derive(Parser)]
#[command(
    name = "ryna-wow",
    version,
    about = "The World of Warcraft: Forever companion agent (developer command line; the desktop app is the way to install it)"
)]
struct Cli {
    /// The game folder (`World of Warcraft`) or one product folder in it (`_forever_`), in place
    /// of the default install locations.
    #[arg(long, global = true, env = "RYNA_WOW_INSTALL", value_name = "PATH")]
    install: Option<PathBuf>,
    /// The API origin (default https://api.ryna.io).
    #[arg(long, global = true, env = "RYNA_WOW_API", value_name = "ORIGIN")]
    api: Option<String>,
    /// The sign-in provider (default https://auth.nyratech.dev).
    #[arg(long, global = true, env = "RYNA_WOW_ISSUER", value_name = "URL")]
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
    },
    /// Copy the addon into each Forever install's Interface/AddOns.
    InstallAddon {
        /// The addon folder (or a folder holding Ryna/ or addon/Ryna/). Default: beside this binary.
        #[arg(long, value_name = "DIR")]
        from: Option<PathBuf>,
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

fn main() -> ExitCode {
    let cli = Cli::parse();
    match dispatch(cli) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("ryna-wow: {e:#}");
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
        Command::Watch { interval } => watch(&config, cli.install.as_deref(), interval),
        Command::InstallAddon { from } => install_addon(cli.install.as_deref(), from.as_deref()),
        Command::Parse { file, raw, account } => parse(&file, raw, account.as_deref()),
    }
}

/// The command that fixes a failure: the core's errors name none, so the CLI adds its own.
fn hint(fix: Fix) -> Option<&'static str> {
    match fix {
        Fix::SignIn => Some("run `ryna-wow login`"),
        Fix::Update => Some("update ryna-wow, then run `ryna-wow install-addon`"),
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
        Some(who) => println!("Signed in as {who}. `ryna-wow run` now pushes your accounts."),
        None => println!("Signed in. `ryna-wow run` now pushes your accounts as you."),
    }
    Ok(ExitCode::SUCCESS)
}

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
    println!("ryna-wow {AGENT_VERSION}");
    println!("API {}  sign-in {}", config.api, config.issuer);
    if !config.client_id_issued() {
        println!("Device client: not issued yet (set RYNA_WOW_CLIENT_ID); `login` can't run until it is.");
    }
    let beside = addon_install::beside_binary();
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
        (None, false, _) => println!("Not signed in: run `ryna-wow login`."),
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
                        addon.push_str(&format!(" ({b} available: `ryna-wow install-addon`)"))
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
        println!("\nNo Ryna.lua yet: install the addon (`ryna-wow install-addon`), log in to the game, then /reload or log out.");
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
        state::state_dir().context("no place to keep the agent's state: set RYNA_WOW_STATE_DIR")?;
    let found: Discovery = discovery::discover(install);
    if found.installs.is_empty() {
        eprintln!("no installs found: point at the game with --install <folder>");
    }
    let http = HttpTransport::new().map_err(anyhow::Error::msg)?;
    let store = KeyringStore::for_issuer(&config.issuer);
    let clock = SystemClock;
    let auth = Auth::new(config, &http, &store, &clock);
    let api = Api::new(&config.api, &http, &auth, &clock);
    match runner::run_once(&found, &dir, &api, &clock, opts) {
        Ok(report) => {
            print_report(&report);
            if let Some(h) = report.failure.and_then(hint) {
                eprintln!("To fix it: {h}");
            }
            Ok(report.failure.map_or(0, |f| f.exit_code() as u8))
        }
        Err(RunError::Busy) => {
            eprintln!("another ryna-wow run is in progress; not starting a second");
            Ok(1)
        }
        Err(e) => Err(e.into()),
    }
}

/// The core's watch loop (the one the desktop app drives), printing its events.
fn watch(config: &Config, install: Option<&Path>, interval: u64) -> Result<ExitCode> {
    let opts = WatchOptions::new(
        install.map(Path::to_path_buf),
        Duration::from_secs(interval),
    )
    .context("no place to keep the agent's state: set RYNA_WOW_STATE_DIR")?;
    println!("Watching every {interval} s (Ctrl-C to stop).");
    let cancel = CancelToken::new();
    ryna_wow_core::watch::watch(config, &opts, &cancel, &mut |event| {
        let stamp = iso_utc(now_secs());
        match &event {
            WatchEvent::SignedIn { signed_in: false } => {
                println!("{stamp} not signed in: run `ryna-wow login`")
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
        }
    })?;
    Ok(ExitCode::SUCCESS)
}

fn install_addon(install: Option<&Path>, from: Option<&Path>) -> Result<ExitCode> {
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
    }
    println!("Restart the game (or /reload) to load it.");
    Ok(ExitCode::SUCCESS)
}

fn parse(file: &Path, raw: bool, account: Option<&str>) -> Result<ExitCode> {
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
