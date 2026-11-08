//! `ryna settings …`: the Settings mod by hand (docs/ryna-agent-cores-mods.md 7.2). Listing,
//! comparing and copying are local and never touch the network; backups go to ryna.io as the
//! signed-in person, encrypted on this machine. Every write refuses while the game is running
//! unless `--force`.

use std::path::Path;
use std::process::ExitCode;

use anyhow::{anyhow, bail, Result};
use clap::{Args, Subcommand};
use ryna_agent::auth::{Auth, KeyringStore};
use ryna_agent::config::Config;
use ryna_agent::http::HttpTransport;
use ryna_agent::identity;
use ryna_agent::util::{hostname, iso_utc, now_secs, SystemClock};
use ryna_wow_core::discovery::{self, Install};
use ryna_wow_core::game_process::game_running_at;
use ryna_wow_core::push::{Api, PushError};
use ryna_wow_core::settings::backup::{self, Include, Progress, RestoreOptions};
use ryna_wow_core::settings::copy::{self, Difference, WriteKind};
use ryna_wow_core::settings::inventory::{self, Inventory, Scope};

#[derive(Args)]
pub struct SettingsArgs {
    /// The WTF account folder to work in, when the install has more than one.
    #[arg(long, global = true, value_name = "FOLDER")]
    account: Option<String>,
    #[command(subcommand)]
    command: SettingsCommand,
}

#[derive(Subcommand)]
enum SettingsCommand {
    /// Each addon's settings files (scope, size, last written, the characters they're for) and
    /// the game's own per-character files.
    List,
    /// Which addons' settings differ between two characters.
    Diff {
        #[arg(long, value_name = "NAME-REALM")]
        a: String,
        #[arg(long, value_name = "NAME-REALM")]
        b: String,
    },
    /// Copy one character's settings to another on the same account (the game closed). Each file
    /// replaced is kept as <file>.ryna.bak.
    Copy {
        #[arg(long, value_name = "NAME-REALM")]
        from: String,
        #[arg(long, value_name = "NAME-REALM")]
        to: String,
        /// Only this addon (repeat it for more), or one of the game's files by name
        /// (config-cache.wtf, bindings-cache.wtf, macros-cache.txt, chat-cache.txt,
        /// layout-local.txt, AddOns.txt). Default: every addon.
        #[arg(long = "addon", value_name = "ADDON")]
        addons: Vec<String>,
        /// Say what would be written, writing nothing.
        #[arg(long)]
        dry_run: bool,
        /// Write even though the game seems to be running, or Ryna can't tell (it will overwrite
        /// these at logout); and let --to be a character this account doesn't have yet (its folder
        /// is made).
        #[arg(long)]
        force: bool,
    },
    /// Back the account's settings up to ryna.io, encrypted on this machine (sign in first).
    Backup {
        /// addons (the default), game (the game's own per-character files), or all.
        #[arg(long, default_value = "addons")]
        include: String,
        /// The name the backup is listed under (default: this computer's).
        #[arg(long, value_name = "NAME")]
        machine: Option<String>,
    },
    /// Your backups on ryna.io, newest first.
    Backups,
    /// Put a backup's files back (the game closed). Each file replaced is kept as <file>.ryna.bak.
    Restore {
        id: String,
        #[arg(long)]
        dry_run: bool,
        /// Write even though the game seems to be running, or Ryna can't tell; and with --account,
        /// restore into an account other than the one the backup was made from.
        #[arg(long)]
        force: bool,
    },
    /// Delete a backup from ryna.io.
    Delete { id: String },
}

/// The install to work in: the Forever one, else the only one found.
fn pick_install(install: Option<&Path>) -> Result<Install> {
    let found = discovery::discover(install).installs;
    if let Some(i) = found
        .iter()
        .find(|i| i.flavor.as_deref() == Some("wowf"))
        .or(if found.len() == 1 {
            found.first()
        } else {
            None
        })
    {
        return Ok(i.clone());
    }
    if found.is_empty() {
        bail!("no World of Warcraft install found: point at it with --install <folder>");
    }
    bail!(
        "more than one install and none is Forever: point at one with --install ({})",
        found
            .iter()
            .map(|i| i.path.display().to_string())
            .collect::<Vec<_>>()
            .join(", ")
    )
}

/// The account folder to work in: the one named, else the only one.
fn pick_account(install: &Install, chosen: Option<&str>) -> Result<String> {
    let all = inventory::accounts(install);
    if let Some(c) = chosen {
        return all
            .into_iter()
            .find(|a| a.eq_ignore_ascii_case(c))
            .ok_or_else(|| {
                anyhow!(
                    "there's no account folder {c} in {}",
                    install.path.display()
                )
            });
    }
    match all.as_slice() {
        [one] => Ok(one.clone()),
        [] => bail!(
            "{} has no account folders yet: log in to the game once",
            install.path.display()
        ),
        many => bail!(
            "this install has more than one account: choose one with --account ({})",
            many.join(", ")
        ),
    }
}

fn size(bytes: u64) -> String {
    match bytes {
        b if b < 1024 => format!("{b} B"),
        b if b < 1024 * 1024 => format!("{:.1} KB", b as f64 / 1024.0),
        b => format!("{:.1} MB", b as f64 / (1024.0 * 1024.0)),
    }
}

fn scope_words(s: Scope) -> &'static str {
    match s {
        Scope::Account => "account",
        Scope::Character => "per character",
        Scope::Both => "account + per character",
    }
}

/// A PushError's fix, as the rest of the CLI words it.
fn api_error(e: anyhow::Error) -> anyhow::Error {
    match e.downcast_ref::<PushError>() {
        Some(p) => anyhow!(crate::with_hint(&format!("{e:#}"), p.fix())),
        None => e,
    }
}

pub fn run(config: &Config, install: Option<&Path>, args: SettingsArgs) -> Result<ExitCode> {
    let inst = pick_install(install)?;
    let account = args.account.as_deref();
    match args.command {
        SettingsCommand::List => {
            let inv = inventory::inventory(&inst, &pick_account(&inst, account)?)?;
            list(&inv);
            Ok(ExitCode::SUCCESS)
        }
        SettingsCommand::Diff { a, b } => {
            let inv = inventory::inventory(&inst, &pick_account(&inst, account)?)?;
            let (a, b) = (inv.character(&a)?, inv.character(&b)?);
            let rows = copy::diff(&inv, &a, &b);
            if rows.is_empty() {
                println!("Neither {a} nor {b} has settings.");
            }
            for r in rows {
                let how = match r.difference {
                    Difference::Same => "same".to_string(),
                    Difference::Differ => "differ".to_string(),
                    Difference::OnlyA => format!("only {a}"),
                    Difference::OnlyB => format!("only {b}"),
                };
                let what = if r.game_file { " (game file)" } else { "" };
                println!("  {:<28} {how}{what}", r.item);
            }
            Ok(ExitCode::SUCCESS)
        }
        SettingsCommand::Copy {
            from,
            to,
            addons,
            dry_run,
            force,
        } => {
            let inv = inventory::inventory(&inst, &pick_account(&inst, account)?)?;
            let from = inv.character(&from)?;
            let to = if force {
                inv.new_character(&to)?
            } else {
                inv.character(&to)?
            };
            let items = (!addons.is_empty()).then_some(addons.as_slice());
            let plan = copy::plan(&inv, &from, &to, items)?;
            let verb = if dry_run { "Would write" } else { "Writing" };
            if plan.new_folder {
                println!("{to} has no folder yet: it's made, and the game uses it at {to}'s first login.");
            }
            for w in &plan.writes {
                let how = match &w.kind {
                    WriteKind::File { from } => format!("copied from {from}"),
                    WriteKind::AceDb { variables } => {
                        format!(
                            "{to}'s entries made from {from}'s in {}",
                            variables.join(", ")
                        )
                    }
                };
                let bak = if w.replaces {
                    "; the file there is kept as .ryna.bak"
                } else {
                    ""
                };
                println!("{verb} {} ({how}{bak})", w.path);
            }
            for s in &plan.skipped {
                println!("Left {}: {}", s.item, s.reason);
            }
            if plan.writes.is_empty() {
                println!("Nothing to write.");
                return Ok(ExitCode::SUCCESS);
            }
            if dry_run {
                println!("Dry run: nothing was written.");
                return Ok(ExitCode::SUCCESS);
            }
            let done = copy::apply(&plan, &|| game_running_at(&inst.path), force)?;
            println!(
                "Copied {from} to {to}: {} file(s) written, {} kept as .ryna.bak.",
                done.written.len(),
                done.backups.len()
            );
            Ok(ExitCode::SUCCESS)
        }
        SettingsCommand::Backup { include, machine } => {
            let include = Include::parse(&include)?;
            let inv = inventory::inventory(&inst, &pick_account(&inst, account)?)?;
            let machine = machine.unwrap_or_else(hostname);
            let done = with_api(config, |api| {
                backup::backup(api, &inv, include, &machine, now_secs(), &mut progress)
            })?;
            println!(
                "Backed up {} file(s) from {} as {} ({}, {} part(s)). Stored encrypted for you on ryna.io.",
                done.files,
                inv.account,
                done.id,
                size(done.bytes),
                done.parts
            );
            Ok(ExitCode::SUCCESS)
        }
        SettingsCommand::Backups => {
            let list = with_api(config, backup::list)?;
            if list.is_empty() {
                println!("No backups on ryna.io yet.");
            }
            for b in list {
                let when = b
                    .created_at
                    .as_i64()
                    .map(iso_utc)
                    .unwrap_or_else(|| b.created_at.to_string());
                println!(
                    "{}  {when}  {}  {} file(s)  {}",
                    b.id,
                    size(b.bytes),
                    b.files,
                    b.machine.unwrap_or_default()
                );
            }
            Ok(ExitCode::SUCCESS)
        }
        SettingsCommand::Restore { id, dry_run, force } => {
            let report = with_api(config, |api| {
                let opts = RestoreOptions {
                    account,
                    other_account: force && account.is_some(),
                    dry_run,
                    force,
                };
                backup::restore(
                    api,
                    &id,
                    &inst,
                    opts,
                    &|| game_running_at(&inst.path),
                    &mut progress,
                )
            })?;
            let verb = if report.written {
                "Wrote"
            } else {
                "Would write"
            };
            for f in &report.files {
                let bak = if f.replaces {
                    " (the file there kept as .ryna.bak)"
                } else {
                    ""
                };
                println!("{verb} {} ({}){bak}", f.path, size(f.bytes));
            }
            if report.written {
                println!(
                    "Restored {} file(s) into {}; {} kept as .ryna.bak.",
                    report.files.len(),
                    report.account,
                    report.backups.len()
                );
            } else {
                println!("Dry run: nothing was written.");
            }
            Ok(ExitCode::SUCCESS)
        }
        SettingsCommand::Delete { id } => {
            with_api(config, |api| backup::delete(api, &id))?;
            println!("Deleted backup {id}.");
            Ok(ExitCode::SUCCESS)
        }
    }
}

fn progress(p: Progress) {
    if p.stage == "uploading" || p.stage == "writing" {
        eprintln!("  {} {} of {}", p.stage, p.done + 1, p.total);
    } else {
        eprintln!("  {}", p.stage);
    }
}

/// Run `f` with the API as the signed-in person, signed by the device key.
fn with_api<T>(config: &Config, f: impl FnOnce(&Api) -> Result<T>) -> Result<T> {
    let key = identity::device_key(config)?;
    let http = HttpTransport::new().map_err(anyhow::Error::msg)?;
    let store = KeyringStore::for_issuer(&config.issuer);
    let clock = SystemClock;
    let auth = Auth::new(config, &http, &store, &clock);
    let api = Api::new(&config.api, &http, &auth, Some(&key), &clock);
    f(&api).map_err(api_error)
}

fn list(inv: &Inventory) {
    println!(
        "{} in {} (known to ryna.io as {})",
        inv.account, inv.install, inv.account_key
    );
    let chars: Vec<String> = inv.characters.iter().map(ToString::to_string).collect();
    println!(
        "Characters: {}",
        if chars.is_empty() {
            "none yet".to_string()
        } else {
            chars.join(", ")
        }
    );
    println!("Addons:");
    if inv.addons.is_empty() {
        println!("  none with settings");
    }
    for a in &inv.addons {
        let mut notes = vec![scope_words(a.scope).to_string()];
        if a.acedb {
            notes.push("AceDB".into());
        }
        if !a.installed {
            notes.push("not installed".into());
        }
        let when = a
            .modified
            .map(iso_utc)
            .unwrap_or_else(|| "never written".into());
        let who: Vec<String> = a.characters.iter().map(ToString::to_string).collect();
        println!(
            "  {:<24} {:<34} {:>9}  {when}  {}",
            a.addon,
            notes.join(", "),
            size(a.bytes),
            who.join(", ")
        );
    }
    println!("Game files:");
    if inv.game_files.is_empty() {
        println!("  none");
    }
    for c in &inv.character_folders {
        let names: Vec<&str> = inv
            .game_files
            .iter()
            .filter(|f| f.character.as_ref() == Some(c))
            .map(|f| f.path.rsplit('/').next().unwrap_or(""))
            .collect();
        if !names.is_empty() {
            println!("  {c}: {}", names.join(", "));
        }
    }
}
