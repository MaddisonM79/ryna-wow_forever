//! Chronicle by zone's backfill (docs/ryna-agent-cores-mods.md 7.6, part 4): the kills a character
//! counted before the addon kept zones, placed in zones from the player's own combat logs.
//!
//! The addon stamps `zonesSince` on a character the first time it counts a kill, loot or object in
//! a zone. What each creature counted before then (`n` less its `zones`) has no zone; the logs on
//! this computer may say where some of it happened. This runs once per character, by itself, the
//! first time a run sees `zonesSince` on a character with no `zonesBackfill`, and only with the
//! game closed (it writes `Ryna.lua` the way the Settings mod's copy writes SavedVariables:
//! [`crate::settings::write_all`], a `.ryna.bak` first, refused while the game runs). Nothing here
//! leaves the computer: the next push carries the result like any other change to the file.
//!
//! - **Which lines.** Only those before the character's `zonesSince`, so nothing is counted twice.
//! - **Whose kills.** A `PARTY_KILL` whose killer is in the logging character's group (its own
//!   blow, its pet's, or a party or raid member's: the `AFFILIATION_MINE | PARTY | RAID` flags),
//!   while that character was logging. The logging character is the player unit the log marks
//!   `AFFILIATION_MINE`.
//! - **Which zone.** The `ZONE_CHANGE` before the kill, when its name is a zone. On Forever (checked
//!   on 2026-11-10 against 26 logs from Nov 6 to Nov 10) `ZONE_CHANGE` names the zone as
//!   `GetRealZoneText` does ("Elwynn Forest", "Westfall", "The Deadmines" inside the dungeon), but it
//!   also fires on walking into a building ("Lion's Pride Inn", "Sentinel Tower", "Lakeshire Inn")
//!   and on a loading screen ("UNKNOWN AREA"), with no `MAP_CHANGE` for either. A name counts as a
//!   zone when the logs also show it as a `MAP_CHANGE` (the zone's map), as an instance's
//!   `ZONE_CHANGE` (a nonzero instance id), or the record already has it as a zone; anything else
//!   leaves the kill in the zone it was in, which is the building's own zone.
//! - **How much.** Per creature, what the logs saw per zone, at most the [`MAX_ZONES`] biggest, and
//!   never more than the earlier count `n − sum(zones)`: when the logs saw more (they credit a
//!   group's kills the experience line may not have), the split is scaled down to it.
//! - **Where it goes.** `kills[npcId].logZones = { [zone] = n }`, beside `zones`, and
//!   `characters[guid].zonesBackfill = { at, files, placed }`. A character with `zonesBackfill` is
//!   never backfilled again, even when the logs placed nothing.

use std::collections::{BTreeMap, BTreeSet};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::combat::parse::{self, flags, Kind};
use crate::discovery::AccountFile;
use crate::lua::{self, LuaKey, LuaValue};
use crate::lua_write;

/// At most this many zones per creature, as the addon keeps (`MAX_ZONES` in Chronicle.lua).
pub const MAX_ZONES: usize = 8;

/// What a loading screen's `ZONE_CHANGE` says.
const UNKNOWN_AREA: &str = "UNKNOWN AREA";

/// Kills read from the logs: character GUID -> npc id -> zone -> kills.
pub type Tally = BTreeMap<String, BTreeMap<String, BTreeMap<String, i64>>>;

/// What the logs gave: the kills by character, creature and zone, and how many files were read.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LogKills {
    pub kills: Tally,
    pub files: usize,
}

/// One thing a log says, in order.
enum Item {
    Zone(String),
    Kill { owner: String, npc: String },
}

/// The npc id of a `Creature-` or `Vehicle-` GUID (its sixth field), as the addon keys it.
fn npc_id(guid: &str) -> Option<String> {
    let mut p = guid.split('-');
    let kind = p.next()?;
    if kind != "Creature" && kind != "Vehicle" {
        return None;
    }
    let id: u64 = p.nth(4)?.parse().ok()?;
    Some(id.to_string())
}

/// The player unit the log marks as the one logging.
fn logging_player(u: &parse::Unit) -> Option<&str> {
    let mine = flags::AFFILIATION_MINE | flags::TYPE_PLAYER;
    (u.flags & mine == mine && u.guid.starts_with("Player-")).then_some(u.guid.as_str())
}

/// What one log file says before `until` (unix ms): its zone changes and kills in order, and the
/// names it shows as zones (maps, instances) into `zones`.
fn read_file(
    path: &Path,
    until: i64,
    owners: &BTreeSet<String>,
    zones: &mut BTreeSet<String>,
) -> std::io::Result<Vec<Item>> {
    let mut out = Vec::new();
    let mut owner: Option<String> = None;
    let mut reader = BufReader::new(std::fs::File::open(path)?);
    let mut buf = Vec::new();
    loop {
        buf.clear();
        if reader.read_until(b'\n', &mut buf)? == 0 {
            break;
        }
        let line = String::from_utf8_lossy(&buf);
        let Ok(ev) = parse::parse_line(&line) else {
            continue;
        };
        if ev.ts >= until {
            break;
        }
        match &ev.kind {
            Kind::Header { .. } => owner = None,
            Kind::ZoneChange {
                instance_id, name, ..
            } => {
                if *instance_id != 0 && name != UNKNOWN_AREA {
                    zones.insert(name.clone());
                }
                out.push(Item::Zone(name.clone()));
            }
            Kind::MapChange { name, .. } => {
                zones.insert(name.clone());
            }
            _ => {}
        }
        for u in [&ev.src, &ev.dst] {
            if let Some(g) = logging_player(u) {
                if owner.as_deref() != Some(g) {
                    owner = Some(g.to_string());
                }
            }
        }
        if matches!(ev.kind, Kind::PartyKill) && ev.src.flags & flags::AFFILIATION_GROUP != 0 {
            if let (Some(o), Some(npc)) = (&owner, npc_id(&ev.dst.guid)) {
                if owners.contains(o) {
                    out.push(Item::Kill {
                        owner: o.clone(),
                        npc,
                    });
                }
            }
        }
    }
    Ok(out)
}

/// Reads `logs` for the characters in `cutoffs` (GUID -> unix ms: only lines before it count for
/// that character). `known` are names already known to be zones (the record's own). A file that
/// can't be read is skipped.
pub fn read_logs(
    logs: &[PathBuf],
    cutoffs: &BTreeMap<String, i64>,
    known: &BTreeSet<String>,
) -> LogKills {
    let Some(&until) = cutoffs.values().max() else {
        return LogKills::default();
    };
    let owners: BTreeSet<String> = cutoffs.keys().cloned().collect();
    let mut zones = known.clone();
    let mut files = Vec::new();
    let mut read = 0;
    for path in logs {
        match read_file(path, until, &owners, &mut zones) {
            Ok(items) => {
                read += 1;
                files.push(items);
            }
            Err(e) => eprintln!("zones backfill: {}: {e}", path.display()),
        }
    }
    // The kills, each in the zone of the last ZONE_CHANGE before it that names a zone. Cutoffs per
    // character were applied as one (the latest) while reading; each kill's own is checked here.
    let mut kills: Tally = BTreeMap::new();
    for items in &files {
        let mut zone: Option<&str> = None;
        for item in items {
            match item {
                Item::Zone(name) => {
                    if name != UNKNOWN_AREA && zones.contains(name) {
                        zone = Some(name);
                    }
                }
                Item::Kill { owner, npc } => {
                    if let Some(z) = zone {
                        *kills
                            .entry(owner.clone())
                            .or_default()
                            .entry(npc.clone())
                            .or_default()
                            .entry(z.to_string())
                            .or_default() += 1;
                    }
                }
            }
        }
    }
    LogKills { kills, files: read }
}

/// [`read_logs`] with each kill checked against its own character's cutoff: reads per character
/// when cutoffs differ (they rarely do: one account's characters each have their own).
pub fn read_logs_per_character(
    logs: &[PathBuf],
    cutoffs: &BTreeMap<String, i64>,
    known: &BTreeSet<String>,
) -> LogKills {
    let distinct: BTreeSet<i64> = cutoffs.values().copied().collect();
    if distinct.len() <= 1 {
        return read_logs(logs, cutoffs, known);
    }
    let mut out = LogKills::default();
    for (guid, cutoff) in cutoffs {
        let one = BTreeMap::from([(guid.clone(), *cutoff)]);
        let got = read_logs(logs, &one, known);
        out.files = out.files.max(got.files);
        out.kills.extend(got.kills);
    }
    out
}

/// The logs' kills per zone for one creature, placed against its earlier count `earlier`: the
/// [`MAX_ZONES`] biggest zones, all of them when they fit, else scaled down to `earlier` (largest
/// remainders first, then the bigger zone, then by name). Zero-count zones are left out.
pub fn place(earlier: i64, seen: &BTreeMap<String, i64>) -> BTreeMap<String, i64> {
    let mut top: Vec<(&String, i64)> = seen
        .iter()
        .filter(|(_, n)| **n > 0)
        .map(|(z, n)| (z, *n))
        .collect();
    top.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));
    top.truncate(MAX_ZONES);
    let total: i64 = top.iter().map(|(_, n)| n).sum();
    let earlier = earlier.max(0);
    if total <= earlier {
        return top.into_iter().map(|(z, n)| (z.clone(), n)).collect();
    }
    if earlier == 0 {
        return BTreeMap::new();
    }
    // Scaled: floor of each share, then the rest one at a time to the largest remainders.
    let mut shares: Vec<(&String, i64, i64)> = top
        .iter()
        .map(|(z, n)| {
            let exact = n * earlier;
            (*z, exact / total, exact % total)
        })
        .collect();
    let mut left = earlier - shares.iter().map(|s| s.1).sum::<i64>();
    let mut order: Vec<usize> = (0..shares.len()).collect();
    order.sort_by(|&a, &b| {
        shares[b]
            .2
            .cmp(&shares[a].2)
            .then_with(|| top[b].1.cmp(&top[a].1))
            .then_with(|| shares[a].0.cmp(shares[b].0))
    });
    for i in order {
        if left == 0 {
            break;
        }
        shares[i].1 += 1;
        left -= 1;
    }
    shares
        .into_iter()
        .filter(|s| s.1 > 0)
        .map(|(z, n, _)| (z.clone(), n))
        .collect()
}

fn number(v: Option<&LuaValue>) -> Option<f64> {
    match v? {
        LuaValue::Int(i) => Some(*i as f64),
        LuaValue::Float(f) => Some(*f),
        _ => None,
    }
}

fn int(v: Option<&LuaValue>) -> i64 {
    number(v).map_or(0, |f| f as i64)
}

fn key_text(k: &LuaKey) -> Option<String> {
    match k {
        LuaKey::Str(b) => Some(String::from_utf8_lossy(b).into_owned()),
        LuaKey::Int(i) => Some(i.to_string()),
        _ => None,
    }
}

/// The characters waiting for a backfill: GUID -> `zonesSince` (unix seconds).
pub fn pending(db: &LuaValue) -> BTreeMap<String, i64> {
    let mut out = BTreeMap::new();
    let Some(chars) = db.field("characters").and_then(LuaValue::as_table) else {
        return out;
    };
    for (k, c) in chars {
        let (Some(guid), Some(since)) = (key_text(k), number(c.field("zonesSince"))) else {
            continue;
        };
        if c.field("zonesBackfill").is_none() && since > 0.0 {
            out.insert(guid, since as i64);
        }
    }
    out
}

/// Every zone name the record already holds (kills', loot's and objects' `zone` and `zones`, the
/// character's `zone` and `places`): names the addon read from `GetRealZoneText`.
fn record_zones(db: &LuaValue) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let add_text = |v: Option<&LuaValue>, out: &mut BTreeSet<String>| {
        if let Some(t) = v.and_then(LuaValue::as_text) {
            if !t.is_empty() {
                out.insert(t);
            }
        }
    };
    let Some(chars) = db.field("characters").and_then(LuaValue::as_table) else {
        return out;
    };
    for (_, c) in chars {
        add_text(c.field("zone"), &mut out);
        if let Some(places) = c.field("places").and_then(LuaValue::as_table) {
            out.extend(places.iter().filter_map(|(k, _)| key_text(k)));
        }
        for map in ["kills", "loot", "objects"] {
            let Some(entries) = c.field(map).and_then(LuaValue::as_table) else {
                continue;
            };
            for (_, e) in entries {
                add_text(e.field("zone"), &mut out);
                if let Some(zones) = e.field("zones").and_then(LuaValue::as_table) {
                    out.extend(zones.iter().filter_map(|(k, _)| key_text(k)));
                }
            }
        }
    }
    out
}

/// One character's result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Placed {
    pub guid: String,
    /// Kills placed in zones, over all creatures.
    pub placed: i64,
}

/// Applies `logs` to the characters in `which` in `db` (`RynaDB`): each creature's `logZones`
/// (only where something was placed, and never on a creature that has one already), then the
/// character's `zonesBackfill`. Characters that have one already are left alone.
pub fn apply(
    db: &mut LuaValue,
    which: &BTreeMap<String, i64>,
    logs: &LogKills,
    now_secs: i64,
) -> Vec<Placed> {
    let mut out = Vec::new();
    let Some(chars) = db.field_mut("characters") else {
        return out;
    };
    let Some(entries) = chars.as_table_mut() else {
        return out;
    };
    for (k, c) in entries.iter_mut() {
        let Some(guid) = key_text(k) else { continue };
        if !which.contains_key(&guid) || c.field("zonesBackfill").is_some() {
            continue;
        }
        let mut placed = 0;
        if let (Some(seen), Some(kills)) = (logs.kills.get(&guid), c.field_mut("kills")) {
            if let Some(kills) = kills.as_table_mut() {
                for (npc_key, e) in kills.iter_mut() {
                    let Some(npc) = key_text(npc_key) else {
                        continue;
                    };
                    let Some(by_zone) = seen.get(&npc) else {
                        continue;
                    };
                    if e.as_table().is_none() || e.field("logZones").is_some() {
                        continue;
                    }
                    let live: i64 = e
                        .field("zones")
                        .and_then(LuaValue::as_table)
                        .map_or(0, |z| z.iter().map(|(_, v)| int(Some(v))).sum());
                    let earlier = int(e.field("n")) - live;
                    let put = place(earlier, by_zone);
                    if put.is_empty() {
                        continue;
                    }
                    placed += put.values().sum::<i64>();
                    let table = put
                        .into_iter()
                        .map(|(z, n)| (LuaKey::str(&z), LuaValue::Int(n)))
                        .collect();
                    e.set(LuaKey::str("logZones"), LuaValue::Table(table));
                }
            }
        }
        c.set(
            LuaKey::str("zonesBackfill"),
            LuaValue::Table(vec![
                (LuaKey::str("at"), LuaValue::Int(now_secs)),
                (LuaKey::str("files"), LuaValue::Int(logs.files as i64)),
                (LuaKey::str("placed"), LuaValue::Int(placed)),
            ]),
        );
        out.push(Placed { guid, placed });
    }
    out
}

/// Every `WoWCombatLog*.txt` in an install's `Logs`, oldest name first (the names carry the time
/// logging started).
pub fn log_files(install: &Path) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = std::fs::read_dir(install.join("Logs"))
        .map(|rd| {
            rd.filter_map(Result::ok)
                .filter(|e| {
                    let n = e.file_name();
                    let n = n.to_string_lossy();
                    n.starts_with("WoWCombatLog") && n.ends_with(".txt")
                })
                .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
                .map(|e| e.path())
                .collect()
        })
        .unwrap_or_default();
    out.sort();
    out
}

/// What [`run`] did for one account file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// No character waits for a backfill.
    Nothing,
    /// Some do, but the game is running (or can't be told not to be): next run.
    GameRunning,
    /// Done: the characters, what each had placed, and the log files read.
    Done { placed: Vec<Placed>, files: usize },
}

/// The backfill for one account's `Ryna.lua`: the characters with `zonesSince` and no
/// `zonesBackfill`, from the install's logs, written back with the game closed. `game_running` is
/// asked only when there is something to do, and again right before the write.
pub fn run(acct: &AccountFile, now_secs: i64, game_running: &dyn Fn() -> bool) -> Result<Outcome> {
    let bytes =
        std::fs::read(&acct.path).with_context(|| format!("reading {}", acct.path.display()))?;
    // Cheap first: most runs have nothing to do.
    if !bytes
        .windows(b"zonesSince".len())
        .any(|w| w == b"zonesSince")
    {
        return Ok(Outcome::Nothing);
    }
    let mut vars =
        lua::parse_bytes(&bytes).with_context(|| format!("reading {}", acct.path.display()))?;
    let Some(db) = vars.iter().position(|(n, _)| n == "RynaDB") else {
        return Ok(Outcome::Nothing);
    };
    let which = pending(&vars[db].1);
    if which.is_empty() {
        return Ok(Outcome::Nothing);
    }
    if game_running() {
        return Ok(Outcome::GameRunning);
    }
    let cutoffs: BTreeMap<String, i64> = which
        .iter()
        .map(|(g, s)| (g.clone(), s.saturating_mul(1000)))
        .collect();
    let known = record_zones(&vars[db].1);
    let logs = read_logs_per_character(&log_files(&acct.install), &cutoffs, &known);
    let placed = apply(&mut vars[db].1, &which, &logs, now_secs);
    let out = lua_write::write_file(&vars);
    let root = acct
        .path
        .parent()
        .context("the account file has no folder")?;
    let name = acct
        .path
        .file_name()
        .and_then(|n| n.to_str())
        .context("the account file has no name")?;
    crate::settings::write_all(root, &[(name, out.as_slice())], game_running, false)
        .with_context(|| format!("writing {}", acct.path.display()))?;
    Ok(Outcome::Done {
        placed,
        files: logs.files,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const LOG: &str = include_str!("../../fixtures/combat-zones.txt");
    const ME: &str = "Player-6933-0F00A001";
    const ALT: &str = "Player-6933-0F00A009";

    fn unix_ms(stamp: &str) -> i64 {
        parse::parse_time(stamp).unwrap()
    }

    fn log_dir() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let install = dir.path().join("_forever_");
        std::fs::create_dir_all(install.join("Logs")).unwrap();
        std::fs::write(
            install.join("Logs").join("WoWCombatLog-111026_051242.txt"),
            LOG,
        )
        .unwrap();
        (dir, install)
    }

    fn zones(pairs: &[(&str, i64)]) -> BTreeMap<String, i64> {
        pairs.iter().map(|(z, n)| (z.to_string(), *n)).collect()
    }

    #[test]
    fn the_logs_zones_kills_and_whose_they_are() {
        let (_d, install) = log_dir();
        let cutoffs = BTreeMap::from([(ME.to_string(), unix_ms("11/10/2026 07:00:00.000-6"))]);
        let got = read_logs(&log_files(&install), &cutoffs, &BTreeSet::new());
        assert_eq!(got.files, 1);
        let mine = &got.kills[ME];
        // Kobold Vermin (6): two of mine in Elwynn (one inside the inn), one my party's in Westfall.
        assert_eq!(mine["6"], zones(&[("Elwynn Forest", 2), ("Westfall", 1)]));
        // Defias Thug (38): one in Westfall, one in the Deadmines (whose first ZONE_CHANGE says 0 and
        // whose map is the continent's).
        assert_eq!(mine["38"], zones(&[("The Deadmines", 1), ("Westfall", 1)]));
        // A stranger's blow (no group flag) and the kill after the cutoff aren't counted.
        assert!(!mine.contains_key("99"));
        assert!(!mine.contains_key("40"));
        // The alt logged later in the same file; not asked for, so not read.
        assert!(!got.kills.contains_key(ALT));
    }

    #[test]
    fn each_character_has_its_own_cutoff() {
        let (_d, install) = log_dir();
        let cutoffs = BTreeMap::from([
            (ME.to_string(), unix_ms("11/10/2026 06:00:00.000-6")),
            (ALT.to_string(), unix_ms("11/10/2026 09:00:00.000-6")),
        ]);
        let got = read_logs_per_character(&log_files(&install), &cutoffs, &BTreeSet::new());
        // Before 06:00 only the first Elwynn kill was mine.
        assert_eq!(got.kills[ME]["6"], zones(&[("Elwynn Forest", 1)]));
        // The alt's own kill, in Redridge, after it logged in.
        assert_eq!(got.kills[ALT]["40"], zones(&[("Redridge Mountains", 1)]));
    }

    #[test]
    fn a_building_isnt_a_zone_unless_the_record_says_so() {
        let (_d, install) = log_dir();
        let cutoffs = BTreeMap::from([(ME.to_string(), unix_ms("11/10/2026 07:00:00.000-6"))]);
        let known = BTreeSet::from(["Lion's Pride Inn".to_string()]);
        let got = read_logs(&log_files(&install), &cutoffs, &known);
        assert_eq!(
            got.kills[ME]["6"],
            zones(&[
                ("Elwynn Forest", 1),
                ("Lion's Pride Inn", 1),
                ("Westfall", 1)
            ])
        );
    }

    #[test]
    fn placing_never_more_than_the_earlier_count() {
        let seen = zones(&[("Elwynn Forest", 6), ("Westfall", 3), ("Duskwood", 1)]);
        assert_eq!(place(20, &seen), seen, "all of it fits");
        assert_eq!(place(10, &seen), seen, "exactly");
        let scaled = place(5, &seen);
        assert_eq!(
            scaled.values().sum::<i64>(),
            5,
            "scaled to the earlier count"
        );
        assert_eq!(scaled, zones(&[("Elwynn Forest", 3), ("Westfall", 2)]));
        assert!(place(0, &seen).is_empty());
        assert!(place(-3, &seen).is_empty());
        let many: BTreeMap<String, i64> = (1..=10).map(|i| (format!("Zone {i:02}"), i)).collect();
        let kept = place(1000, &many);
        assert_eq!(kept.len(), MAX_ZONES, "the eight biggest");
        assert!(!kept.contains_key("Zone 01") && !kept.contains_key("Zone 02"));
    }

    const SAVED: &str = r#"RynaDB = {
	["characters"] = {
		["Player-6933-0F00A001"] = {
			["zonesSince"] = 1793000000,
			["kills"] = {
				["6"] = {
					["n"] = 5,
					["name"] = "Kobold Vermin",
					["zones"] = {
						["Elwynn Forest"] = 3,
					},
				},
				["38"] = {
					["n"] = 1,
				},
				["40"] = {
					["n"] = 2,
				},
			},
		},
		["Player-6933-0F00A002"] = {
			["kills"] = {
			},
		},
	},
	["schema"] = 1,
}
RynaOther = {
	"kept", -- [1]
}
"#;

    fn account(dir: &Path, install: &Path) -> AccountFile {
        let sv = dir.join("SavedVariables");
        std::fs::create_dir_all(&sv).unwrap();
        let path = sv.join("Ryna.lua");
        std::fs::write(&path, SAVED).unwrap();
        AccountFile {
            install: install.to_path_buf(),
            flavor: Some("wowf".into()),
            account: "FAKEACCOUNT".into(),
            path,
        }
    }

    fn reread(acct: &AccountFile) -> serde_json::Value {
        lua::parse(&std::fs::read_to_string(&acct.path).unwrap()).unwrap()
    }

    #[test]
    fn run_places_once_with_the_game_closed() {
        let (dir, install) = log_dir();
        let acct = account(dir.path(), &install);
        // zonesSince in the file is 1793000000 (an older time): set it to 07:00 on the log's day.
        let since = unix_ms("11/10/2026 07:00:00.000-6") / 1000;
        let text = SAVED.replace("1793000000", &since.to_string());
        std::fs::write(&acct.path, &text).unwrap();

        assert_eq!(
            run(&acct, 1_800_000_000, &|| true).unwrap(),
            Outcome::GameRunning
        );
        assert_eq!(
            std::fs::read_to_string(&acct.path).unwrap(),
            text,
            "untouched"
        );

        let got = run(&acct, 1_800_000_000, &|| false).unwrap();
        let Outcome::Done { placed, files } = got else {
            panic!("not done: {got:?}")
        };
        assert_eq!(files, 1);
        // Kobold Vermin: 2 earlier (5 less the 3 live), the logs saw 3 (2 Elwynn, 1 Westfall):
        // scaled to 2. Defias Thug: 1 earlier, the logs saw 2: 1. Gnoll: the logs saw none of mine.
        assert_eq!(
            placed,
            vec![Placed {
                guid: ME.into(),
                placed: 3
            }]
        );
        let v = reread(&acct);
        let me = &v["RynaDB"]["characters"][ME];
        assert_eq!(
            me["kills"]["6"]["logZones"],
            serde_json::json!({ "Elwynn Forest": 1, "Westfall": 1 })
        );
        assert_eq!(
            me["kills"]["6"]["zones"],
            serde_json::json!({ "Elwynn Forest": 3 })
        );
        let thug = me["kills"]["38"]["logZones"].as_object().unwrap();
        assert_eq!(thug.values().filter_map(|n| n.as_i64()).sum::<i64>(), 1);
        assert!(me["kills"]["40"].get("logZones").is_none());
        assert_eq!(
            me["zonesBackfill"],
            serde_json::json!({ "at": 1_800_000_000i64, "files": 1, "placed": 3 })
        );
        // The rest of the file as it was, the other variable included, and a backup beside it.
        assert_eq!(v["RynaOther"], serde_json::json!(["kept"]));
        assert_eq!(v["RynaDB"]["schema"], 1);
        assert!(acct.path.with_file_name("Ryna.lua.ryna.bak").exists());

        // Never twice: the stamp is there now, so nothing to do, and nothing counted again.
        let before = std::fs::read(&acct.path).unwrap();
        assert_eq!(
            run(&acct, 1_900_000_000, &|| false).unwrap(),
            Outcome::Nothing
        );
        assert_eq!(std::fs::read(&acct.path).unwrap(), before);
    }

    #[test]
    fn apply_leaves_what_has_log_zones_or_a_stamp() {
        let mut vars = lua::parse_bytes(SAVED.as_bytes()).unwrap();
        let db = &mut vars[0].1;
        let which = pending(db);
        assert_eq!(which.keys().collect::<Vec<_>>(), vec![ME]);
        let mut logs = LogKills {
            kills: BTreeMap::new(),
            files: 2,
        };
        logs.kills.insert(
            ME.into(),
            BTreeMap::from([("6".to_string(), zones(&[("Westfall", 1)]))]),
        );
        let first = apply(db, &which, &logs, 10);
        assert_eq!(first[0].placed, 1);
        // Asked again (as if the stamp were missed): the stamp stops it, and logZones stays.
        let again = apply(db, &which, &logs, 20);
        assert!(again.is_empty());
        let me = db.field("characters").unwrap().field(ME).unwrap();
        assert_eq!(
            int(me.field("zonesBackfill").unwrap().field("at")),
            10,
            "the first run's stamp"
        );
        assert!(pending(db).is_empty());
    }

    #[test]
    fn nothing_to_do_reads_no_logs_and_writes_nothing() {
        let (dir, install) = log_dir();
        let acct = account(dir.path(), &install);
        std::fs::write(&acct.path, "RynaDB = {\n\t[\"characters\"] = {\n\t},\n}\n").unwrap();
        let asked = std::cell::Cell::new(false);
        let out = run(&acct, 1, &|| {
            asked.set(true);
            true
        })
        .unwrap();
        assert_eq!(out, Outcome::Nothing);
        assert!(!asked.get(), "the game isn't even asked about");
    }

    #[test]
    fn npc_ids_from_guids() {
        assert_eq!(
            npc_id("Creature-0-6805-0-148-550-0000730363").as_deref(),
            Some("550")
        );
        assert_eq!(npc_id("Vehicle-0-1-2-3-06-0000AAAA").as_deref(), Some("6"));
        assert_eq!(npc_id("Player-6933-0F00A001"), None);
        assert_eq!(npc_id("Pet-0-1-2-3-4-5"), None);
    }
}
