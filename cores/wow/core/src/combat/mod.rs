//! The combat log (docs/wow.md section 9): the client writes `<install>/Logs/WoWCombatLog-*.txt`
//! while `/combatlog` is on; the agent reads it as it grows, cuts it into encounters and uploads
//! each one's summary and digest (and, when asked, its raw lines).
//!
//! - [`parse`]: one line into an event, with Forever's field layouts.
//! - [`segment`]: lines into encounters (boss, trash, world).
//! - [`summary`]: an encounter's numbers, from its digest events.
//! - [`digest`]: the digest's NDJSON and gzip.
//! - [`tail`]: following every recent log in every install, with a state file per log.
//! - [`queue`]: the uploads, kept on disk until the API has them.

pub mod digest;
pub mod parse;
pub mod queue;
pub mod segment;
pub mod summary;
pub mod tail;

use segment::Fight;
use summary::{fight_key, SourceInfo, Summary};

/// What the agent does with the combat log (the app's settings).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CombatOptions {
    /// Read the logs at all.
    pub enabled: bool,
    /// Upload each fight's digest.
    pub digests: bool,
    /// Upload each fight's raw lines (kept 90 days on ryna.io).
    pub raw: bool,
}

impl Default for CombatOptions {
    fn default() -> Self {
        CombatOptions {
            enabled: true,
            digests: true,
            raw: false,
        }
    }
}

/// The summary the API takes for one fight from one log.
pub fn summarize(
    fight: &Fight,
    flavor: &str,
    account_key: &str,
    file: &str,
    opts: &CombatOptions,
) -> Summary {
    let totals = fight.totals();
    Summary {
        key: fight_key(flavor, fight.realm_id, account_key, file, fight.started_at),
        account: (!account_key.is_empty()).then(|| account_key.to_string()),
        flavor: flavor.to_string(),
        realm_id: fight.realm_id,
        kind: fight.kind.as_str().to_string(),
        encounter_id: fight.encounter_id,
        name: fight.name.clone(),
        zone: fight.zone.clone(),
        instance: fight.instance.clone(),
        difficulty: fight.difficulty,
        ui_map_id: fight.ui_map_id,
        started_at: fight.started_at,
        ended_at: fight.ended_at,
        duration_ms: fight.duration_ms(),
        success: fight.success,
        participants: totals.participants,
        actors: totals.actors,
        abilities: totals.abilities,
        series: totals.series,
        deaths: totals.deaths,
        source: SourceInfo {
            parser: crate::AGENT_VERSION.to_string(),
            advanced: fight.advanced,
            events: fight.events,
            file: file.to_string(),
            digest: opts.digests,
            raw: opts.raw,
        },
    }
}

/// "Zephras Isle, 3 min, 2 players": how the app's activity log names a fight.
pub fn describe(
    zone: Option<&str>,
    name: Option<&str>,
    duration_ms: i64,
    players: usize,
) -> String {
    let place = name.or(zone).unwrap_or("Somewhere");
    let secs = (duration_ms / 1000).max(0);
    let length = if secs < 90 {
        format!("{secs} s")
    } else {
        format!("{} min", (secs + 30) / 60)
    };
    let who = if players == 1 {
        "1 player".to_string()
    } else {
        format!("{players} players")
    };
    format!("{place}, {length}, {who}")
}

#[cfg(test)]
mod tests;
