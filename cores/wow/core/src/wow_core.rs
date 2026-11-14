//! The WoW core behind the agent's interface (`ryna_agent::core::Core`): the watch loop, the
//! status card's rows and the heartbeat, with the manifest `cores/wow/core.json` built in.
//!
//! Driving it through the trait does exactly what `watch::watch` does: the same loop
//! (`watch::watch_with_store`), the same events, the same state folder and keychain entries.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Duration;

use ryna_agent::core::{
    Activity, Chip, Core, CoreContext, CoreControl, CoreHeartbeat, CoreManifest, Phase, StatusRow,
};

use crate::addon_install::{self, AddonState};
use crate::combat::{self, CombatOptions};
use crate::discovery::{self, Discovery};
use crate::fix::Fix;
use crate::mods::{self, ModStatus};
use crate::runner::{heartbeat_body, RunError};
use crate::state::State;
use crate::status::{install_statuses, CombatStatus, InstallStatus};
use crate::watch::{self, WatchEvent, WatchOptions};

/// `cores/wow/core.json`, as built into this crate.
pub const MANIFEST_JSON: &str = include_str!("../../core.json");

/// The manifest, parsed once. A test holds `core.json` to what the code does.
pub fn manifest() -> &'static CoreManifest {
    static MANIFEST: OnceLock<CoreManifest> = OnceLock::new();
    MANIFEST.get_or_init(|| CoreManifest::parse(MANIFEST_JSON).expect("cores/wow/core.json"))
}

/// The WoW core, with what a front end chooses for it.
#[derive(Debug, Clone)]
pub struct WowCore {
    /// Between snapshot passes.
    pub interval: Duration,
    /// What to do with the combat logs.
    pub combat: CombatOptions,
    /// The addon this front end would install (the app's resources, the CLI's `--from`), for the
    /// status card's comparison.
    pub bundled_addon: Option<PathBuf>,
    /// The mods the person turned on or off, by id (the rest are their `defaultOn`).
    pub mods: BTreeMap<String, bool>,
}

impl WowCore {
    pub fn new(interval: Duration) -> Self {
        WowCore {
            interval,
            combat: CombatOptions::default(),
            bundled_addon: None,
            mods: BTreeMap::new(),
        }
    }

    pub fn with_mods(mut self, mods: BTreeMap<String, bool>) -> Self {
        self.mods = mods;
        self
    }

    /// Every bundled mod's state in the installs found (for the app's Game tab).
    pub fn mod_statuses(&self, ctx: &CoreContext) -> Vec<ModStatus> {
        let found = discovery::discover(ctx.game_folder.as_deref());
        let source = self
            .bundled_addon
            .as_deref()
            .and_then(|d| addon_install::resolve_source(d).ok());
        mods::statuses(&found.installs, source.as_deref(), &self.mods)
    }

    pub fn with_combat(mut self, combat: CombatOptions) -> Self {
        self.combat = combat;
        self
    }

    pub fn with_bundled_addon(mut self, dir: Option<PathBuf>) -> Self {
        self.bundled_addon = dir;
        self
    }

    fn options(&self, ctx: &CoreContext) -> WatchOptions {
        WatchOptions {
            install: ctx.game_folder.clone(),
            state_dir: ctx.state_dir.clone(),
            interval: self.interval,
            combat: self.combat,
        }
    }
}

impl Core for WowCore {
    type Event = WatchEvent;
    type Error = RunError;

    fn manifest(&self) -> &CoreManifest {
        manifest()
    }

    fn start(
        &self,
        ctx: &CoreContext,
        control: &CoreControl,
        events: &mut dyn FnMut(WatchEvent),
    ) -> Result<(), RunError> {
        let store = ctx.tokens.store();
        watch::watch_with_store(
            ctx.tokens.config(),
            store.as_ref(),
            ctx.device_key.as_deref(),
            &self.options(ctx),
            control.token(),
            events,
        )
    }

    fn activity(&self, event: &WatchEvent) -> Activity {
        activity(event)
    }

    fn failed(&self, message: String, fix: Fix) -> WatchEvent {
        WatchEvent::error(None, None, message, fix)
    }

    fn status_rows(&self, ctx: &CoreContext) -> Vec<StatusRow> {
        let found = discovery::discover(ctx.game_folder.as_deref());
        let bundled = self
            .bundled_addon
            .as_deref()
            .and_then(addon_install::bundled_addon_version);
        let installs = install_statuses(&found, bundled.as_deref());
        let combat = CombatStatus::read(&ctx.state_dir, ryna_agent::util::now_secs());
        let source = self
            .bundled_addon
            .as_deref()
            .and_then(|d| addon_install::resolve_source(d).ok());
        let mods = mods::statuses(&found.installs, source.as_deref(), &self.mods);
        rows(&installs, bundled.as_deref(), &mods, &combat)
    }

    fn heartbeat(&self, ctx: &CoreContext) -> CoreHeartbeat {
        heartbeat(
            &discovery::discover(ctx.game_folder.as_deref()),
            &ctx.state_dir,
            ryna_agent::util::now_secs(),
        )
    }
}

/// What the agent's state takes from an event: the account rows and the failure to show (what
/// the app's window did with them before the seam).
pub fn activity(event: &WatchEvent) -> Activity {
    let row = |path: &str, phase| Activity {
        row: Some((path.to_string(), phase)),
        ..Activity::default()
    };
    match event {
        WatchEvent::AccountFileChanged { path, .. } => row(path, Phase::Waiting),
        WatchEvent::Pushing { path, .. } => row(path, Phase::Pushing),
        WatchEvent::Pushed { path, .. } => row(path, Phase::Pushed),
        WatchEvent::Error { path, advice, .. } => Activity {
            row: path.as_ref().map(|p| (p.clone(), Phase::Error)),
            error: Some(advice.clone()),
            clears_error: false,
        },
        WatchEvent::FightFailed {
            fix: Fix::SignIn | Fix::Network,
            advice,
            ..
        } => Activity {
            error: Some(advice.clone()),
            ..Activity::default()
        },
        WatchEvent::PassFinished { ok: true, .. } => Activity {
            clears_error: true,
            ..Activity::default()
        },
        _ => Activity::default(),
    }
}

/// The addon's words for an install (the window's `addonRow`).
fn addon_words(i: &InstallStatus, bundled: Option<&str>) -> (String, Chip) {
    let v = i.addon_version.as_deref().unwrap_or("");
    let b = bundled.unwrap_or("");
    match i.addon {
        AddonState::NotInstalled => ("Addon not installed".into(), Chip::Waiting),
        AddonState::Current => (format!("Addon {v} installed"), Chip::Pushed),
        AddonState::Older => (format!("Addon {v} installed, {b} available"), Chip::Update),
        AddonState::Newer => (
            format!("Addon {v} installed (this app bundles {b})"),
            Chip::Pushed,
        ),
        AddonState::Unknown if i.addon_version.is_some() => {
            (format!("Addon {v} installed"), Chip::Pushed)
        }
        AddonState::Unknown if bundled.is_some() => (
            format!("Addon installed (version unknown), {b} available"),
            Chip::Update,
        ),
        AddonState::Unknown => ("Addon installed".into(), Chip::Pushed),
    }
}

/// The status card's rows: the game and its addon, a row per mod (by its name: on or off, what's
/// installed, what's bundled), then the combat log.
pub fn rows(
    installs: &[InstallStatus],
    bundled: Option<&str>,
    mods: &[ModStatus],
    combat: &CombatStatus,
) -> Vec<StatusRow> {
    let mut rows = Vec::new();
    match installs.iter().find(|i| i.forever).or(installs.first()) {
        None => rows.push(StatusRow::new(
            "Game",
            "Not found in the usual place",
            Some(Chip::Waiting),
        )),
        Some(main) => {
            rows.push(StatusRow::new("Folder", main.path.clone(), None));
            let (words, chip) = if main.forever || main.has_addon {
                addon_words(main, bundled)
            } else {
                ("No addon here".into(), Chip::Waiting)
            };
            rows.push(StatusRow::new("Addon", words, Some(chip)));
            if main.forever || main.has_addon {
                for m in mods {
                    let (words, chip) = mods::row_words(m, &main.path);
                    rows.push(StatusRow::new(&m.name, words, Some(chip)));
                }
            }
        }
    }
    let (log, chip) = match (&combat.file, &combat.last_error) {
        (None, _) => (
            "No combat log yet: type /combatlog in the game".to_string(),
            Chip::Waiting,
        ),
        (Some(_), Some(e)) => (e.clone(), Chip::Error),
        (Some(file), None) => {
            let mut parts = vec![
                if combat.active { "Reading" } else { "Idle" }.to_string(),
                file.clone(),
            ];
            if combat.pending > 0 {
                parts.push(format!("{} waiting", combat.pending));
            }
            (
                parts.join(" · "),
                if combat.active {
                    Chip::Pushed
                } else {
                    Chip::Waiting
                },
            )
        }
    };
    rows.push(StatusRow::new("Log", log, Some(chip)));
    rows.push(StatusRow::new(
        "Fights, last 12 h",
        combat.fights_tonight.to_string(),
        None,
    ));
    rows.push(StatusRow::new(
        "Last fight",
        combat.last_fight.as_ref().map_or_else(
            || "None yet".to_string(),
            |f| {
                combat::describe(
                    f.zone.as_deref(),
                    f.name.as_deref(),
                    f.duration_ms,
                    f.players,
                )
            },
        ),
        None,
    ));
    rows
}

/// The heartbeat a pass would send now (`runner::heartbeat_body` with the combat log's state),
/// as the core's fields.
pub fn heartbeat(found: &Discovery, state_dir: &Path, now: i64) -> CoreHeartbeat {
    let st = State::load(state_dir);
    let last_error = st
        .last_run
        .as_ref()
        .filter(|r| !r.ok)
        .and_then(|r| r.error.clone());
    let mut hb = heartbeat_body(found, &st, last_error);
    hb.combat_log = combat::tail::heartbeat(state_dir, now);
    CoreHeartbeat {
        core: manifest().id.clone(),
        version: manifest().version.clone(),
        fields: serde_json::to_value(&hb).unwrap_or_default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::testing::MemoryStore;
    use crate::config::{test_config, INGEST_SCOPE, LEGACY_RESOURCE};
    use crate::discovery::testing::product;
    use ryna_agent::auth::TokenStore;
    use ryna_agent::config::Config;
    use ryna_agent::core::{Runtime, TokenProvider};
    use serde_json::json;
    use std::sync::Arc;

    struct Tokens(Config);
    impl TokenProvider for Tokens {
        fn config(&self) -> &Config {
            &self.0
        }
        fn store(&self) -> Box<dyn TokenStore> {
            Box::new(MemoryStore::default())
        }
    }

    #[test]
    fn the_manifest_says_what_the_code_does() {
        let m = manifest();
        assert_eq!(m.id, "wow");
        assert_eq!(m.runtime, Runtime::Builtin);
        assert_eq!(m.version, crate::AGENT_VERSION);
        // What the core needs, as documentation: the host signs in from the cores answer.
        assert_eq!(m.resource, LEGACY_RESOURCE);
        assert_eq!(m.scopes, [INGEST_SCOPE]);
        assert_eq!(m.mods.as_deref(), Some("mods/wow"));
        // Every default root discovery looks in on this OS is listed (with ~ for the home folder).
        let home = std::env::var("HOME").unwrap_or_default();
        for root in discovery::default_roots() {
            let root = root.to_string_lossy().into_owned();
            let listed = m
                .detect
                .folders
                .iter()
                .any(|f| *f == root || (!home.is_empty() && f.replacen('~', &home, 1) == root));
            assert!(listed, "{root} isn't in core.json's detect.folders");
        }
        for read in [
            "WTF/Account/*/SavedVariables/Ryna.lua",
            "Logs/WoWCombatLog-*.txt",
        ] {
            assert!(m.data.reads.iter().any(|r| r == read), "{read}");
        }
        assert_eq!(
            m.data.writes[0],
            format!("Interface/AddOns/{}/", addon_install::ADDON_NAME)
        );
        // Every bundled mod's folders are among what it writes (`Interface/AddOns/Ryna_*/`), and the
        // game's addon lists among what it reads.
        for folder in mods::bundled().iter().flat_map(|m| &m.addons) {
            let listed = m.data.writes.iter().any(|w| {
                w.strip_prefix("Interface/AddOns/")
                    .and_then(|w| w.strip_suffix("*/"))
                    .is_some_and(|prefix| folder.starts_with(prefix))
            });
            assert!(listed, "{folder} isn't in core.json's data.writes");
        }
        assert!(m.data.reads.iter().any(|r| r.ends_with("AddOns.txt")));
    }

    #[test]
    fn events_become_rows_and_errors() {
        let path = "/wow/_forever_/WTF/Account/A/SavedVariables/Ryna.lua";
        let a = activity(&WatchEvent::Pushing {
            label: "UNT1".into(),
            path: path.into(),
        });
        assert_eq!(a.row, Some((path.into(), Phase::Pushing)));
        let e = activity(&WatchEvent::error(
            None,
            Some(path.into()),
            "x".into(),
            Fix::Network,
        ));
        assert_eq!(e.row, Some((path.into(), Phase::Error)));
        assert_eq!(e.error.as_deref(), Some("Couldn't reach the API"));
        assert!(
            activity(&WatchEvent::PassFinished {
                ok: true,
                next_in_secs: 60
            })
            .clears_error
        );
        assert_eq!(
            activity(&WatchEvent::PassFinished {
                ok: false,
                next_in_secs: 60
            }),
            Activity::default()
        );
    }

    #[test]
    fn driven_through_the_trait_it_runs_a_pass_and_stops() {
        let root = tempfile::tempdir().unwrap();
        product(root.path(), "_forever_", "wowf", &[]);
        let state = tempfile::tempdir().unwrap();
        let core = WowCore::new(Duration::from_secs(3600)).with_combat(CombatOptions {
            enabled: false,
            ..CombatOptions::default()
        });
        let ctx = CoreContext {
            state_dir: state.path().to_path_buf(),
            tokens: Arc::new(Tokens(test_config())),
            device_key: Some(Arc::new(ryna_agent::identity::testing::test_key())),
            game_folder: Some(root.path().to_path_buf()),
        };
        let control = CoreControl::new();
        let stopper = control.clone();
        let mut seen = Vec::new();
        core.start(&ctx, &control, &mut |e| {
            if matches!(e, WatchEvent::PassFinished { .. }) {
                stopper.stop();
            }
            seen.push(serde_json::to_value(&e).unwrap()["event"].clone());
        })
        .unwrap();
        // Not signed in (an empty store): the pass says so, finds the install and ends.
        assert_eq!(seen.first(), Some(&json!("signedIn")));
        assert!(seen.contains(&json!("installFound")));
        assert_eq!(seen.last(), Some(&json!("passFinished")));
        assert!(control.is_stopped());

        let rows = core.status_rows(&ctx);
        let labels: Vec<_> = rows.iter().map(|r| r.label.as_str()).collect();
        assert_eq!(
            labels,
            [
                "Folder",
                "Addon",
                "Chronicle",
                "Automation",
                "Prices",
                "Settings",
                "Log",
                "Fights, last 12 h",
                "Last fight"
            ]
        );
        assert_eq!(rows[1].value, "Addon not installed");
        assert_eq!(rows[2].value, "On · not installed yet: install the addon");
        assert_eq!(rows[3].value, "Off", "Automation is off by default");
        assert_eq!(rows[4].value, "Off", "Prices is off by default");

        let hb = core.heartbeat(&ctx);
        assert_eq!(
            (hb.core.as_str(), hb.version.as_str()),
            ("wow", crate::AGENT_VERSION)
        );
        assert_eq!(hb.fields["installs"][0]["flavor"], json!("wowf"));
        assert_eq!(hb.fields["combatLog"]["pending"], json!(0));
    }

    #[test]
    fn rows_when_nothing_is_found() {
        let rows = rows(&[], None, &[], &CombatStatus::default());
        assert_eq!(rows[0].value, "Not found in the usual place");
        assert_eq!(rows[0].chip, Some(Chip::Waiting));
        assert_eq!(
            rows[1].value,
            "No combat log yet: type /combatlog in the game"
        );
    }
}
