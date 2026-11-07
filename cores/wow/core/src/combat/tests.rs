//! The combat module against `cores/wow/fixtures/combat-world.txt`: a real Forever log (open-world
//! questing, 2026-11-06), scrubbed by `scripts/capture.sh --combat` (names and GUID hexes are
//! fakes; NPCs, spells, times and structure are real).

use std::collections::BTreeMap;

use serde_json::json;

use super::digest::{bits, gunzip, Digest};
use super::parse::{parse_line, split_fields, split_time, Kind};
use super::queue::{Queue, Uploaded};
use super::segment::{segment_text, FightKind, Segmenter};
use super::summary::{totals, Extras};
use super::tail::{CombatState, Tailer};
use super::*;
use crate::auth::testing::MemoryStore;
use crate::auth::Auth;
use crate::config::test_config;
use crate::discovery::{discover, testing::product};
use crate::http::testing::ScriptedTransport;
use crate::http::Method;
use crate::push::Api;
use crate::util::testing::FakeClock;

const WORLD: &str = include_str!("../../../fixtures/combat-world.txt");
/// The fixture's logger (the `0x511` player).
const LOGGER: &str = "Player-6933-0F000004";
const NOW: i64 = 1_800_000_000;

fn world() -> super::segment::Fight {
    let (fights, _) = segment_text(WORLD, true);
    assert_eq!(fights.len(), 1, "one world encounter");
    fights.into_iter().next().unwrap()
}

#[test]
fn every_line_reads_with_the_layouts_of_section_9_1() {
    let mut lengths: BTreeMap<String, BTreeMap<usize, usize>> = BTreeMap::new();
    for line in WORLD.lines() {
        let e = parse_line(line).expect(line);
        let n = split_fields(split_time(line).unwrap().1).len();
        *lengths.entry(e.subevent).or_default().entry(n).or_default() += 1;
    }
    let only = |sub: &str| -> Vec<usize> { lengths[sub].keys().copied().collect() };
    assert_eq!(only("SPELL_DAMAGE"), vec![42]);
    assert_eq!(only("SWING_DAMAGE"), vec![38]);
    assert_eq!(only("SWING_DAMAGE_LANDED"), vec![38]);
    assert_eq!(only("SPELL_CAST_SUCCESS"), vec![31]);
    assert_eq!(only("UNIT_DIED"), vec![10]);
    assert_eq!(only("SPELL_AURA_APPLIED"), vec![13]);
    assert!(only("SWING_MISSED").iter().all(|n| *n == 11 || *n == 12));
    // Every damage line's advanced block was found (the header says ADVANCED_LOG_ENABLED,0).
    for line in WORLD.lines() {
        let e = parse_line(line).unwrap();
        if matches!(e.kind, Kind::Damage { .. } | Kind::Heal { .. }) {
            assert!(e.advanced.is_some(), "{line}");
        }
    }
}

#[test]
fn the_questing_is_one_world_encounter_of_six_pulls() {
    let f = world();
    assert_eq!(f.kind, FightKind::World);
    assert_eq!(f.zone.as_deref(), Some("Zephras Isle"));
    assert_eq!(f.ui_map_id, Some(2521));
    assert_eq!(f.instance, None);
    assert_eq!(f.realm_id, Some(6933));
    assert_eq!(f.pulls, 6);
    assert!(f.advanced);
    let t = f.totals();
    assert!(t.participants.contains(&LOGGER.to_string()));
    // Strangers only count where they touched the group's fight: the log names many more.
    let players_in_log: std::collections::BTreeSet<&str> = WORLD
        .split(|c: char| c == ',' || c.is_whitespace())
        .filter(|w| w.starts_with("Player-"))
        .collect();
    assert!(
        t.participants.len() < players_in_log.len() / 4,
        "{} of {}",
        t.participants.len(),
        players_in_log.len()
    );
    let me = t.actors.iter().find(|a| a.guid == LOGGER).unwrap();
    assert_eq!(me.name.as_deref(), Some("Brannoc-Ashvale"));
    assert!(me.damage_done > 0 && me.kills > 0);
    // Gaps between pulls are zeros in the series.
    let series = &t.series.actors[LOGGER];
    assert!(series.damage.iter().filter(|d| **d == 0).count() > 5);
    assert_eq!(series.damage.iter().sum::<i64>(), me.damage_done);
}

/// What the fixture's swings add up to, read straight from its lines inside the fight's window,
/// for the actors the fight counts.
fn melee_by_kind(f: &super::segment::Fight) -> (i64, i64) {
    let (mut landed, mut swing) = (0, 0);
    for line in WORLD.lines() {
        let e = parse_line(line).unwrap();
        if e.ts < f.started_at || e.ts > f.ended_at {
            continue;
        }
        if e.src.guid != LOGGER {
            continue;
        }
        if let Kind::Damage {
            amount,
            overkill,
            landed: l,
            swing: s,
            ..
        } = e.kind
        {
            if l {
                landed += amount - overkill;
            } else if s {
                swing += amount - overkill;
            }
        }
    }
    (landed, swing)
}

#[test]
fn a_swing_and_its_landed_twin_count_once() {
    let f = world();
    // No landed twin and its swing are both in the digest.
    let melee: Vec<_> = f
        .digest
        .iter()
        .filter(|e| e.e == "D" && e.sp == Some(1))
        .collect();
    for (i, a) in melee.iter().enumerate() {
        for b in &melee[i + 1..] {
            if b.t - a.t > 50 {
                break;
            }
            let pair = (a.flags() & bits::SWING_LANDED) != (b.flags() & bits::SWING_LANDED);
            assert!(
                !(pair && a.s == b.s && a.d == b.d && a.a == b.a),
                "{a:?} {b:?}"
            );
        }
    }
    let t = f.totals();
    let mine = t
        .abilities
        .iter()
        .find(|a| a.actor == LOGGER && a.spell_id == 1 && a.kind == "damage")
        .map_or(0, |a| a.total);
    let (landed, swing) = melee_by_kind(&f);
    assert!(landed > 0);
    // Counted once: at least what landed, never both kinds summed.
    assert!(
        mine >= landed && mine < landed + swing,
        "{mine} {landed} {swing}"
    );
}

#[test]
fn when_every_swing_has_its_twin_melee_is_the_landed_sum() {
    let lines = [
        "11/6/2026 15:00:00.000-6  SWING_DAMAGE,Player-1-0F00A001,\"Aerin-Stormcall-Normal-US\",0x511,0x80000000,Creature-0-1-2-3-4-0F00B001,\"Ogre\",0xa48,0x80000000,Player-1-0F00A001,0000000000000000,60,60,37,0,60,0,0,0,1,66,1000,0,1.0,1.0,2521,5.1,0,5,5,-1,1,0,0,0,nil,nil,nil",
        "11/6/2026 15:00:00.001-6  SWING_DAMAGE_LANDED,Player-1-0F00A001,\"Aerin-Stormcall-Normal-US\",0x511,0x80000000,Creature-0-1-2-3-4-0F00B001,\"Ogre\",0xa48,0x80000000,Creature-0-1-2-3-4-0F00B001,0000000000000000,50,60,37,0,60,0,0,0,1,66,1000,0,1.0,1.0,2521,5.1,3,5,5,-1,1,0,0,0,nil,nil,nil",
        "11/6/2026 15:00:03.000-6  SWING_DAMAGE_LANDED,Player-1-0F00A001,\"Aerin-Stormcall-Normal-US\",0x511,0x80000000,Creature-0-1-2-3-4-0F00B001,\"Ogre\",0xa48,0x80000000,Creature-0-1-2-3-4-0F00B001,0000000000000000,43,60,37,0,60,0,0,0,1,66,1000,0,1.0,1.0,2521,5.1,3,7,7,-1,1,0,0,0,nil,nil,nil",
        "11/6/2026 15:00:03.001-6  SWING_DAMAGE,Player-1-0F00A001,\"Aerin-Stormcall-Normal-US\",0x511,0x80000000,Creature-0-1-2-3-4-0F00B001,\"Ogre\",0xa48,0x80000000,Player-1-0F00A001,0000000000000000,60,60,37,0,60,0,0,0,1,66,1000,0,1.0,1.0,2521,5.1,0,7,7,-1,1,0,0,0,nil,nil,nil",
    ];
    let (fights, _) = segment_text(&lines.join("\n"), false);
    let t = fights[0].totals();
    let me = t
        .actors
        .iter()
        .find(|a| a.guid == "Player-1-0F00A001")
        .unwrap();
    assert_eq!(me.damage_done, 12);
    assert_eq!(fights[0].digest.len(), 2);
}

const DEATH: &str = "\
11/6/2026 15:00:00.000-6  ZONE_CHANGE,2991,\"Zephras Isle\",0
11/6/2026 15:00:00.000-6  SWING_DAMAGE_LANDED,Creature-0-1-2-3-4-0F00B001,\"Ogre\",0xa48,0x80000000,Player-1-0F00A001,\"Aerin-Stormcall-Normal-US\",0x511,0x80000000,Player-1-0F00A001,0000000000000000,90,100,1,0,1,0,0,0,0,1,1,0,1.0,1.0,2521,1.0,0,10,10,-1,1,0,0,0,nil,nil,nil
11/6/2026 15:00:02.000-6  SWING_DAMAGE_LANDED,Creature-0-1-2-3-4-0F00B001,\"Ogre\",0xa48,0x80000000,Player-1-0F00A001,\"Aerin-Stormcall-Normal-US\",0x511,0x80000000,Player-1-0F00A001,0000000000000000,70,100,1,0,1,0,0,0,0,1,1,0,1.0,1.0,2521,1.0,0,20,20,-1,1,0,0,0,nil,nil,nil
11/6/2026 15:00:03.000-6  SPELL_HEAL,Player-1-0F00A002,\"Thorbrand-Ironhew-Normal-US\",0x512,0x80000000,Player-1-0F00A001,\"Aerin-Stormcall-Normal-US\",0x511,0x80000000,331,\"Healing Wave\",0x8,Player-1-0F00A001,0000000000000000,80,100,1,0,1,0,0,0,0,1,1,0,1.0,1.0,2521,1.0,0,10,10,0,0,nil
11/6/2026 15:00:04.000-6  SPELL_DAMAGE,Creature-0-1-2-3-4-0F00B001,\"Ogre\",0xa48,0x80000000,Player-1-0F00A001,\"Aerin-Stormcall-Normal-US\",0x511,0x80000000,9053,\"Fireball\",0x4,Player-1-0F00A001,0000000000000000,0,100,1,0,1,0,0,0,0,1,1,0,1.0,1.0,2521,1.0,0,95,95,15,4,0,0,0,1,nil,nil,ST
11/6/2026 15:00:04.010-6  UNIT_DIED,0000000000000000,nil,0x80000000,0x80000000,Player-1-0F00A001,\"Aerin-Stormcall-Normal-US\",0x511,0x80000000,0
11/6/2026 15:00:05.000-6  SPELL_DAMAGE,Player-1-0F00A002,\"Thorbrand-Ironhew-Normal-US\",0x512,0x80000000,Creature-0-1-2-3-4-0F00B001,\"Ogre\",0xa48,0x80000000,403,\"Lightning Bolt\",0x8,Creature-0-1-2-3-4-0F00B001,0000000000000000,0,100,1,0,1,0,0,0,0,1,1,0,1.0,1.0,2521,1.0,3,30,30,-1,8,0,0,0,nil,nil,nil,ST
11/6/2026 15:00:05.100-6  SPELL_DAMAGE,Player-1-0F00A099,\"Tavi-Emberleaf-Normal-US\",0x518,0x80000000,Creature-0-1-2-3-4-0F00B002,\"Far Away Boar\",0xa48,0x80000000,403,\"Lightning Bolt\",0x8,Creature-0-1-2-3-4-0F00B002,0000000000000000,0,100,1,0,1,0,0,0,0,1,1,0,1.0,1.0,2521,1.0,3,30,30,-1,8,0,0,0,nil,nil,nil,ST
";

#[test]
fn a_death_keeps_its_last_hits_and_its_killer() {
    let (fights, _) = segment_text(DEATH, false);
    let f = &fights[0];
    let t = f.totals();
    assert_eq!(t.deaths.len(), 1);
    let d = &t.deaths[0];
    assert_eq!(d.actor, "Player-1-0F00A001");
    assert_eq!(d.at_ms, 4010);
    assert_eq!(d.killer.as_deref(), Some("Creature-0-1-2-3-4-0F00B001"));
    assert_eq!(d.killer_name.as_deref(), Some("Ogre"));
    let hits: Vec<(i64, i64, &str)> = d
        .last_hits
        .iter()
        .map(|h| (h.at_ms, h.amount, h.name.as_str()))
        .collect();
    assert_eq!(
        hits,
        vec![
            (0, 10, "Melee"),
            (2000, 20, "Melee"),
            (4000, 95, "Fireball")
        ]
    );
    // The party healer is in it; the stranger hitting another boar isn't.
    assert_eq!(
        t.participants,
        vec!["Player-1-0F00A001", "Player-1-0F00A002"]
    );
    let healer = t
        .actors
        .iter()
        .find(|a| a.guid == "Player-1-0F00A002")
        .unwrap();
    assert_eq!(healer.healing_done, 10);
    let me = t
        .actors
        .iter()
        .find(|a| a.guid == "Player-1-0F00A001")
        .unwrap();
    // 95 less 15 overkill.
    assert_eq!(me.damage_taken, 10 + 20 + 80);
    assert_eq!(me.deaths, 1);
}

#[test]
fn the_digest_round_trips_to_the_summary() {
    let f = world();
    let opts = CombatOptions::default();
    let s = summarize(
        &f,
        "wowf",
        "abcdef0123456789",
        "WoWCombatLog-110626_154815.txt",
        &opts,
    );
    let gz = super::digest::gzip(f.digest().to_ndjson().as_bytes());
    let text = String::from_utf8(gunzip(&gz).unwrap()).unwrap();
    let first: serde_json::Value = serde_json::from_str(text.lines().next().unwrap()).unwrap();
    assert!(first["actors"][LOGGER]["name"].is_string());
    let back = Digest::from_ndjson(&text).unwrap();
    let again = totals(
        &back.header,
        &back.events,
        s.duration_ms,
        &Extras::default(),
    );
    assert_eq!(again.participants, s.participants);
    assert_eq!(again.series, s.series);
    assert_eq!(again.deaths, s.deaths);
    // Casts, interrupts and dispels come from events the digest doesn't carry.
    let strip = |mut a: Vec<summary::ActorRow>| {
        for r in &mut a {
            r.interrupts = 0;
            r.dispels = 0;
        }
        a
    };
    assert_eq!(strip(again.actors), strip(s.actors.clone()));
    let no_casts = |a: &[summary::AbilityRow]| {
        a.iter()
            .map(|r| summary::AbilityRow {
                casts: 0,
                ..r.clone()
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(no_casts(&again.abilities), no_casts(&s.abilities));
    // The summary's shape (section 9.3).
    let v = serde_json::to_value(&s).unwrap();
    for k in [
        "key",
        "account",
        "flavor",
        "realmId",
        "kind",
        "encounterId",
        "name",
        "zone",
        "instance",
        "difficulty",
        "uiMapId",
        "startedAt",
        "endedAt",
        "durationMs",
        "success",
        "participants",
        "actors",
        "abilities",
        "series",
        "deaths",
        "source",
    ] {
        assert!(v.get(k).is_some(), "{k}");
    }
    assert_eq!(v["source"]["file"], json!("WoWCombatLog-110626_154815.txt"));
    assert_eq!(v["series"]["stepMs"], json!(5000));
    assert_eq!(s.key.len(), 16);
}

#[test]
fn casts_count_from_cast_successes() {
    let f = world();
    let t = f.totals();
    let bolt = t
        .abilities
        .iter()
        .find(|a| a.actor == LOGGER && a.spell_id == 403)
        .expect("Lightning Bolt");
    // The pre-roll counts the cast that pulled, which lands after the pull opened.
    assert!(bolt.casts >= bolt.hits, "{bolt:?}");
}

#[test]
fn bosses_and_trash() {
    let l = |t: &str, body: &str| format!("11/6/2026 20:{t}-6  {body}\n");
    let hit = |t: &str| {
        l(t, "SPELL_DAMAGE,Player-1-0F00A001,\"Aerin-Stormcall-Normal-US\",0x511,0x80000000,Creature-0-1-2-3-4-0F00B001,\"Onyxia\",0xa48,0x80000000,403,\"Lightning Bolt\",0x8,Creature-0-1-2-3-4-0F00B001,0000000000000000,0,100,1,0,1,0,0,0,0,1,1,0,1.0,1.0,248,1.0,63,30,30,-1,8,0,0,0,nil,nil,nil,ST")
    };
    let mut text = l("00:00.000", "ZONE_CHANGE,249,\"Onyxia's Lair\",9");
    text += &hit("00:01.000");
    text += &hit("00:04.000");
    // Trash ends 8 s later; then the boss, with a 20 s lull that doesn't split it.
    text += &l("00:30.000", "ENCOUNTER_START,1084,\"Onyxia\",9,40,249");
    text += &hit("00:31.000");
    text += &hit("00:51.000");
    text += &l("00:52.000", "ENCOUNTER_END,1084,\"Onyxia\",9,40,1");
    text += &hit("00:53.000");
    let (fights, seg) = segment_text(&text, false);
    let kinds: Vec<_> = fights.iter().map(|f| f.kind).collect();
    assert_eq!(kinds, vec![FightKind::Trash, FightKind::Boss]);
    let boss = &fights[1];
    assert_eq!(boss.name.as_deref(), Some("Onyxia"));
    assert_eq!(boss.encounter_id, Some(1084));
    assert_eq!(boss.success, Some(true));
    assert_eq!(boss.instance.as_deref(), Some("Onyxia's Lair"));
    assert_eq!(boss.duration_ms(), 22_000);
    // The lone hit after the kill is under 2 s: dropped.
    assert_eq!(seg.dropped, 1);
}

#[test]
fn a_partial_line_waits_and_unknown_events_are_counted() {
    let mut seg = Segmenter::new(false);
    seg.feed("11/6/2026 20:00:00.000-6  SOMETHING_NEW,1,2\n", 0);
    seg.feed("garbage\n", 10);
    assert_eq!(seg.subevents["SOMETHING_NEW"], 1);
    assert_eq!(seg.unreadable, 1);
}

fn install_with_log(dir: &std::path::Path, text: &str) -> std::path::PathBuf {
    let root = dir.join("World of Warcraft");
    let p = product(
        &root,
        "_forever_",
        "wowf",
        &[("FAKEACCOUNT1", include_str!("../../../fixtures/Ryna.lua"))],
    );
    std::fs::create_dir_all(p.join("Logs")).unwrap();
    std::fs::write(p.join("Logs/WoWCombatLog-110626_154815.txt"), text).unwrap();
    root
}

#[test]
fn the_tailer_reads_in_pieces_queues_once_and_resumes() {
    let dir = tempfile::tempdir().unwrap();
    // Half the file, cut mid-line.
    let cut = WORLD.len() / 2 + 7;
    let root = install_with_log(dir.path(), &WORLD[..cut]);
    let found = discover(Some(&root));
    let state = dir.path().join("state");
    let mut events = Vec::new();
    let mut tailer = Tailer::new(&state, CombatOptions::default());
    // A wall clock right after the half: the pull still open stays open.
    let half_end = WORLD[..cut]
        .lines()
        .filter_map(|l| split_time(l).map(|(t, _)| t))
        .max()
        .unwrap();
    tailer.poll(&found, None, half_end / 1000 + 1, &mut |e| events.push(e));
    assert_eq!(tailer.queue().len(), 0);
    let st = CombatState::load(&state);
    assert_eq!(st.file.as_deref(), Some("WoWCombatLog-110626_154815.txt"));
    assert_eq!(st.open, 1);
    // The rest arrives; much later the world encounter is over.
    std::fs::write(
        root.join("_forever_/Logs/WoWCombatLog-110626_154815.txt"),
        WORLD,
    )
    .unwrap();
    tailer.poll(&found, None, NOW, &mut |e| events.push(e));
    assert_eq!(tailer.queue().len(), 1);
    let item = &tailer.queue().items()[0];
    assert_eq!(item.summary.kind, "world");
    // The logger is on the fixture account's character list? No: so the install's newest account.
    assert_eq!(
        item.summary.account.as_deref(),
        Some(crate::snapshot::account_key("FAKEACCOUNT1").as_str())
    );
    assert!(item.has_digest && !item.has_raw);
    let queued: Vec<_> = events
        .iter()
        .filter(|e| matches!(e, crate::watch::WatchEvent::FightQueued { .. }))
        .collect();
    assert_eq!(queued.len(), 1);
    // A new tailer (a restart) re-reads nothing it queued.
    let mut again = Tailer::new(&state, CombatOptions::default());
    again.poll(&found, None, NOW, &mut |_| {});
    assert_eq!(again.queue().len(), 1);
    // Heartbeat and AgentStatus read the state.
    let hb = tail::heartbeat(&state, NOW);
    assert_eq!(hb.pending, 1);
    assert_eq!(
        hb.advanced,
        Some(false),
        "the header's flag; the lines carry the block anyway"
    );
    assert!(!hb.active);
}

fn api_run(
    http: &ScriptedTransport,
    state: &std::path::Path,
    opts: CombatOptions,
) -> super::queue::Round {
    let config = test_config();
    let store = MemoryStore::with_access("tok", NOW + 3600);
    let clock = FakeClock::at(NOW);
    let auth = Auth::new(&config, http, &store, &clock);
    let key = crate::identity::testing::test_key();
    let api = Api::new(&config.api, http, &auth, Some(&key), &clock);
    Queue::new(state).upload(&api, &opts, false)
}

fn queued(state: &std::path::Path, n: usize, raw: bool) -> Queue {
    let q = Queue::new(state);
    let f = world();
    for i in 0..n {
        let mut s = summarize(
            &f,
            "wowf",
            "k",
            &format!("f{i}.txt"),
            &CombatOptions::default(),
        );
        s.key = format!("{i:016}");
        q.push(s, 3, Some(vec![1, 2, 3]), raw.then(|| vec![4, 5]), NOW)
            .unwrap();
    }
    q
}

#[test]
fn uploads_post_the_summaries_then_put_the_bodies() {
    let dir = tempfile::tempdir().unwrap();
    let q = queued(dir.path(), 2, true);
    let http = ScriptedTransport::new();
    http.json(200, json!({ "ok": true, "data": { "encounters": [
            { "key": "0000000000000000", "id": 11, "merged": false, "sources": 1 },
            { "key": "0000000000000001", "id": 12, "merged": true, "sources": 2 } ] } }))
        .json(200, json!({ "ok": true, "data": { "id": 11, "key": "0000000000000000", "bytes": 3, "merging": false } }))
        .json(200, json!({ "ok": true, "data": {} }))
        .json(200, json!({ "ok": false, "error": { "code": "DIGESTS_OFF", "message": "no bucket" } }))
        .json(200, json!({ "ok": true, "data": {} }));
    let round = api_run(
        &http,
        dir.path(),
        CombatOptions {
            raw: true,
            ..CombatOptions::default()
        },
    );
    assert!(round.error.is_none(), "{:?}", round.error);
    assert_eq!(round.uploaded.len(), 2);
    assert!(round.digests_off);
    assert!(q.is_empty());
    let reqs = http.requests.borrow();
    assert_eq!(reqs[0].url, "https://api.test/v1/wow/agent/encounters");
    let body: serde_json::Value = serde_json::from_slice(&reqs[0].body).unwrap();
    assert_eq!(body["encounters"].as_array().unwrap().len(), 2);
    assert_eq!(body["encounters"][0]["account"], json!("k"));
    // The POST is signed over its body; the PUTs carry no signature headers.
    let header = |i: usize, name: &str| {
        reqs[i]
            .headers
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.clone())
    };
    assert!(crate::identity::verify(
        &header(0, "X-Ryna-Key").unwrap(),
        &reqs[0].body,
        &header(0, "X-Ryna-Signature").unwrap()
    ));
    assert!(reqs[1].headers.is_empty());
    assert_eq!(reqs[1].method, Method::Put);
    assert_eq!(
        reqs[1].url,
        "https://api.test/v1/wow/agent/encounters/11/digest?key=0000000000000000"
    );
    assert_eq!(reqs[1].content_type, "application/octet-stream");
    assert_eq!(reqs[1].content_encoding, Some("gzip"));
    assert_eq!(reqs[1].body, vec![1, 2, 3]);
    assert_eq!(
        reqs[2].url,
        "https://api.test/v1/wow/agent/encounters/11/raw?key=0000000000000000"
    );
    assert_eq!(
        reqs[3].url,
        "https://api.test/v1/wow/agent/encounters/12/digest?key=0000000000000001"
    );
    assert_eq!(
        reqs[4].url,
        "https://api.test/v1/wow/agent/encounters/12/raw?key=0000000000000001"
    );
    assert_eq!(http.remaining(), 0);
}

#[test]
fn an_outage_keeps_the_queue_and_a_taken_key_is_done() {
    let dir = tempfile::tempdir().unwrap();
    let q = queued(dir.path(), 2, false);
    let http = ScriptedTransport::new();
    http.text(502, "a").text(502, "b").fail("c");
    let round = api_run(&http, dir.path(), CombatOptions::default());
    assert!(matches!(round.error, Some((ref e, _)) if e.fix() == crate::fix::Fix::Network));
    assert_eq!(q.len(), 2);
    assert_eq!(q.items()[0].attempts, 0, "an outage isn't a refusal");

    // The batch is refused as KEY_TAKEN: each goes alone, and the taken one is dropped.
    let http = ScriptedTransport::new();
    http.json(200, json!({ "ok": false, "error": { "code": "KEY_TAKEN", "message": "another person's" } }))
        .json(200, json!({ "ok": false, "error": { "code": "KEY_TAKEN", "message": "another person's" } }))
        .json(200, json!({ "ok": true, "data": { "encounters": [ { "key": "0000000000000001", "id": 12 } ] } }))
        .json(200, json!({ "ok": true, "data": {} }));
    let round = api_run(&http, dir.path(), CombatOptions::default());
    assert!(round.error.is_none(), "{:?}", round.error);
    assert!(
        matches!(round.uploaded[0], Uploaded::Taken { ref item } if item.key == "0000000000000000")
    );
    assert!(matches!(round.uploaded[1], Uploaded::Done { .. }));
    assert!(q.is_empty());
}

#[test]
fn describes_a_fight_in_words() {
    assert_eq!(
        describe(Some("Zephras Isle"), None, 186_674, 2),
        "Zephras Isle, 3 min, 2 players"
    );
    assert_eq!(
        describe(Some("Lair"), Some("Onyxia"), 45_000, 1),
        "Onyxia, 45 s, 1 player"
    );
}
