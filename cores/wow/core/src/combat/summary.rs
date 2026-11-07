//! One fight's summary (docs/wow.md section 9.3), computed from its digest events, so that the
//! digest alone recomputes it (the API's merge does the same): actors, abilities, the 5 s series
//! and the deaths. Casts, interrupts and dispels come from events the digest doesn't carry and are
//! added beside it.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

use super::digest::{bits, ActorMeta, DigestEvent, Header};

pub const STEP_MS: i64 = 5000;
/// A death keeps the hits taken in this window before it.
pub const LAST_HITS_WINDOW_MS: i64 = 10_000;
pub const LAST_HITS: usize = 5;
/// An actor counts as active for this long after each thing it does.
pub const ACTIVE_AFTER_MS: i64 = 2000;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActorRow {
    pub guid: String,
    pub name: Option<String>,
    pub kind: String,
    pub class: Option<String>,
    pub owner_guid: Option<String>,
    pub damage_done: i64,
    pub damage_taken: i64,
    pub healing_done: i64,
    pub healing_taken: i64,
    pub overheal: i64,
    pub absorbed: i64,
    pub deaths: i64,
    pub kills: i64,
    pub interrupts: i64,
    pub dispels: i64,
    pub active_ms: i64,
    pub events: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AbilityRow {
    pub actor: String,
    pub spell_id: i64,
    pub name: String,
    pub school: i64,
    /// `damage` or `heal`.
    pub kind: String,
    pub casts: i64,
    pub hits: i64,
    pub crits: i64,
    pub misses: i64,
    pub total: i64,
    pub max: i64,
    pub over: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SeriesRow {
    pub damage: Vec<i64>,
    pub healing: Vec<i64>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Series {
    pub step_ms: i64,
    pub actors: BTreeMap<String, SeriesRow>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LastHit {
    pub at_ms: i64,
    pub src: Option<String>,
    pub spell_id: i64,
    pub name: String,
    pub amount: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Death {
    pub actor: String,
    pub at_ms: i64,
    pub killer: Option<String>,
    pub killer_name: Option<String>,
    pub last_hits: Vec<LastHit>,
}

/// What the digest's events add up to.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Totals {
    pub participants: Vec<String>,
    pub actors: Vec<ActorRow>,
    pub abilities: Vec<AbilityRow>,
    pub series: Series,
    pub deaths: Vec<Death>,
}

/// What only the log's other events say (cast successes, interrupts, dispels).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Extras {
    pub casts: BTreeMap<(String, i64), i64>,
    pub interrupts: BTreeMap<String, i64>,
    pub dispels: BTreeMap<String, i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceInfo {
    pub parser: String,
    pub advanced: bool,
    pub events: u64,
    pub file: String,
    pub digest: bool,
    pub raw: bool,
}

/// One encounter as `POST /v1/wow/agent/encounters` takes it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub key: String,
    /// The logger's account key (the snapshot's `account.key`), when the agent knows it: the API
    /// files the source under that account.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account: Option<String>,
    pub flavor: String,
    pub realm_id: Option<i64>,
    pub kind: String,
    pub encounter_id: Option<i64>,
    pub name: Option<String>,
    pub zone: Option<String>,
    pub instance: Option<String>,
    pub difficulty: Option<i64>,
    pub ui_map_id: Option<i64>,
    pub started_at: i64,
    pub ended_at: i64,
    pub duration_ms: i64,
    pub success: Option<bool>,
    pub participants: Vec<String>,
    pub actors: Vec<ActorRow>,
    pub abilities: Vec<AbilityRow>,
    pub series: Series,
    pub deaths: Vec<Death>,
    pub source: SourceInfo,
}

/// `key`: the first 16 hex digits of sha256 over flavor, realm id, the logger's account key, the
/// file's name and the start (each on its own line), so one source's fight is one key however
/// often it is uploaded.
pub fn fight_key(
    flavor: &str,
    realm_id: Option<i64>,
    account_key: &str,
    file: &str,
    started_at: i64,
) -> String {
    let realm = realm_id.map(|r| r.to_string()).unwrap_or_default();
    let text = format!("{flavor}\n{realm}\n{account_key}\n{file}\n{started_at}");
    Sha256::digest(text.as_bytes())[..8]
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn is_person(kind: &str) -> bool {
    kind == "player" || kind == "pet"
}

/// Add the digest up. `duration_ms` sizes the series (events past it land in the last bucket).
pub fn totals(
    header: &Header,
    events: &[DigestEvent],
    duration_ms: i64,
    extras: &Extras,
) -> Totals {
    let meta = |g: &str| header.actors.get(g);
    let owner_of = |g: &str| meta(g).and_then(|m| m.owner.clone());
    let kind_of = |g: &str| meta(g).map_or("npc", |m| m.kind.as_str()).to_string();
    let mut rows: BTreeMap<String, ActorRow> = BTreeMap::new();
    fn row<'a>(
        rows: &'a mut BTreeMap<String, ActorRow>,
        header: &Header,
        guid: &str,
    ) -> &'a mut ActorRow {
        rows.entry(guid.to_string()).or_insert_with(|| {
            let m: Option<&ActorMeta> = header.actors.get(guid);
            ActorRow {
                guid: guid.to_string(),
                name: m.and_then(|m| m.name.clone()),
                kind: m.map_or_else(|| "npc".to_string(), |m| m.kind.clone()),
                class: m.and_then(|m| m.class.clone()),
                owner_guid: m.and_then(|m| m.owner.clone()),
                ..ActorRow::default()
            }
        })
    }
    let buckets = ((duration_ms.max(0) / STEP_MS) + 1) as usize;
    let mut series: BTreeMap<String, SeriesRow> = BTreeMap::new();
    let bump = |series: &mut BTreeMap<String, SeriesRow>, g: &str, t: i64, dmg: i64, heal: i64| {
        let r = series.entry(g.to_string()).or_insert_with(|| SeriesRow {
            damage: vec![0; buckets],
            healing: vec![0; buckets],
        });
        let b = ((t.max(0) / STEP_MS) as usize).min(buckets - 1);
        r.damage[b] += dmg;
        r.healing[b] += heal;
    };
    let mut abilities: BTreeMap<(String, i64, &'static str), AbilityRow> = BTreeMap::new();
    let mut active: BTreeMap<String, Vec<i64>> = BTreeMap::new();
    let mut deaths = Vec::new();
    let mut touched: BTreeSet<String> = BTreeSet::new();

    for (i, e) in events.iter().enumerate() {
        if let Some(s) = &e.s {
            row(&mut rows, header, s).events += 1;
        }
        if let Some(d) = &e.d {
            if e.s.as_deref() != Some(d.as_str()) {
                row(&mut rows, header, d).events += 1;
            }
        }
        let sp = e.sp.unwrap_or(0);
        let ability_kind = match e.e.as_str() {
            "D" | "M" => "damage",
            _ => "heal",
        };
        match e.e.as_str() {
            "D" | "H" | "A" => {
                let amount = e.effective();
                let heal = e.e != "D";
                if let Some(s) = &e.s {
                    let r = row(&mut rows, header, s);
                    if heal {
                        r.healing_done += amount;
                        if e.e == "H" {
                            r.overheal += e.over();
                        } else {
                            r.absorbed += amount;
                        }
                    } else {
                        r.damage_done += amount;
                    }
                    let (dmg, hl) = if heal { (0, amount) } else { (amount, 0) };
                    if is_person(&kind_of(s)) {
                        bump(&mut series, s, e.t, dmg, hl);
                    }
                    // A pet's numbers reach its owner too.
                    if let Some(owner) = owner_of(s) {
                        let o = row(&mut rows, header, &owner);
                        if heal {
                            o.healing_done += amount;
                        } else {
                            o.damage_done += amount;
                        }
                        bump(&mut series, &owner, e.t, dmg, hl);
                    }
                    active.entry(s.clone()).or_default().push(e.t);
                    touched.insert(s.clone());
                    let a = abilities
                        .entry((s.clone(), sp, ability_kind))
                        .or_insert_with(|| AbilityRow {
                            actor: s.clone(),
                            spell_id: sp,
                            kind: ability_kind.to_string(),
                            ..AbilityRow::default()
                        });
                    a.hits += 1;
                    if e.flags() & bits::CRIT != 0 {
                        a.crits += 1;
                    }
                    a.total += amount;
                    a.max = a.max.max(e.amount());
                    if e.e != "A" {
                        a.over += e.over();
                    }
                }
                if let Some(d) = &e.d {
                    let r = row(&mut rows, header, d);
                    if heal {
                        r.healing_taken += amount;
                    } else {
                        r.damage_taken += amount;
                    }
                    touched.insert(d.clone());
                }
            }
            "M" => {
                if let Some(s) = &e.s {
                    abilities
                        .entry((s.clone(), sp, "damage"))
                        .or_insert_with(|| AbilityRow {
                            actor: s.clone(),
                            spell_id: sp,
                            kind: "damage".to_string(),
                            ..AbilityRow::default()
                        })
                        .misses += 1;
                    touched.insert(s.clone());
                }
                if let Some(d) = &e.d {
                    touched.insert(d.clone());
                }
            }
            "X" => {
                let Some(d) = &e.d else { continue };
                row(&mut rows, header, d).deaths += 1;
                // The hits it took in the window before, newest last.
                let hits: Vec<&DigestEvent> = events[..i]
                    .iter()
                    .rev()
                    .take_while(|h| e.t - h.t <= LAST_HITS_WINDOW_MS)
                    .filter(|h| h.e == "D" && h.d.as_deref() == Some(d.as_str()))
                    .collect();
                let killer = hits.first().and_then(|h| h.s.clone());
                if let Some(k) = &killer {
                    row(&mut rows, header, k).kills += 1;
                    if let Some(owner) = owner_of(k) {
                        row(&mut rows, header, &owner).kills += 1;
                    }
                }
                if is_person(&kind_of(d)) {
                    let last_hits = hits
                        .iter()
                        .take(LAST_HITS)
                        .rev()
                        .map(|h| {
                            let sp = h.sp.unwrap_or(0);
                            LastHit {
                                at_ms: h.t,
                                src: h.s.clone(),
                                spell_id: sp,
                                name: header
                                    .spells
                                    .get(&sp)
                                    .map(|s| s.name.clone())
                                    .unwrap_or_default(),
                                amount: h.amount(),
                            }
                        })
                        .collect();
                    deaths.push(Death {
                        actor: d.clone(),
                        at_ms: e.t,
                        killer_name: killer
                            .as_deref()
                            .and_then(&meta)
                            .and_then(|m| m.name.clone()),
                        killer,
                        last_hits,
                    });
                }
            }
            _ => {}
        }
    }

    for (guid, mut times) in active {
        times.sort_unstable();
        let mut total = 0;
        let mut until = i64::MIN;
        for t in times {
            let end = (t + ACTIVE_AFTER_MS).min(duration_ms.max(t));
            let start = t.max(until);
            if end > start {
                total += end - start;
            }
            until = until.max(end);
        }
        row(&mut rows, header, &guid).active_ms = total;
    }
    for (g, n) in &extras.interrupts {
        if let Some(r) = rows.get_mut(g) {
            r.interrupts = *n;
        }
    }
    for (g, n) in &extras.dispels {
        if let Some(r) = rows.get_mut(g) {
            r.dispels = *n;
        }
    }
    let mut abilities: Vec<AbilityRow> = abilities
        .into_values()
        .map(|mut a| {
            if let Some(s) = header.spells.get(&a.spell_id) {
                a.name = s.name.clone();
                a.school = s.school;
            }
            a.casts = extras
                .casts
                .get(&(a.actor.clone(), a.spell_id))
                .copied()
                .unwrap_or(a.hits);
            a
        })
        .collect();
    abilities.sort_by(|a, b| {
        (&a.actor, std::cmp::Reverse(a.total), a.spell_id, &a.kind).cmp(&(
            &b.actor,
            std::cmp::Reverse(b.total),
            b.spell_id,
            &b.kind,
        ))
    });
    let participants: Vec<String> = rows
        .values()
        .filter(|r| r.kind == "player" && touched.contains(&r.guid))
        .map(|r| r.guid.clone())
        .collect();
    let mut actors: Vec<ActorRow> = rows.into_values().collect();
    actors.sort_by(|a, b| {
        (std::cmp::Reverse(a.damage_done), &a.guid)
            .cmp(&(std::cmp::Reverse(b.damage_done), &b.guid))
    });
    Totals {
        participants,
        actors,
        abilities,
        series: Series {
            step_ms: STEP_MS,
            actors: series,
        },
        deaths,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::digest::SpellMeta;

    #[allow(clippy::too_many_arguments)]
    fn ev(
        t: i64,
        e: &str,
        s: Option<&str>,
        d: &str,
        sp: i64,
        a: i64,
        o: i64,
        f: u32,
    ) -> DigestEvent {
        DigestEvent {
            t,
            e: e.into(),
            s: s.map(str::to_string),
            d: Some(d.into()),
            sp: Some(sp),
            a: Some(a),
            o: Some(o),
            sc: Some(1),
            f: Some(f),
            m: None,
            twin: false,
        }
    }

    fn header() -> Header {
        let mut h = Header::default();
        for (g, kind, owner) in [
            ("Player-1-A", "player", None),
            ("Pet-0-1-B", "pet", Some("Player-1-A")),
            ("Creature-0-1-C", "npc", None),
        ] {
            h.actors.insert(
                g.into(),
                ActorMeta {
                    name: Some(g.into()),
                    kind: kind.into(),
                    class: None,
                    owner: owner.map(str::to_string),
                },
            );
        }
        h.spells.insert(
            1,
            SpellMeta {
                name: "Melee".into(),
                school: 1,
            },
        );
        h
    }

    #[test]
    fn overkill_overheal_pets_and_a_death() {
        let events = vec![
            ev(
                0,
                "D",
                Some("Player-1-A"),
                "Creature-0-1-C",
                1,
                10,
                0,
                bits::CRIT,
            ),
            ev(1000, "D", Some("Pet-0-1-B"), "Creature-0-1-C", 1, 5, 0, 0),
            ev(2000, "D", Some("Creature-0-1-C"), "Player-1-A", 1, 7, 0, 0),
            ev(3000, "H", Some("Player-1-A"), "Player-1-A", 331, 20, 13, 0),
            ev(6000, "D", Some("Player-1-A"), "Creature-0-1-C", 1, 8, 3, 0),
            DigestEvent {
                t: 6001,
                e: "X".into(),
                d: Some("Creature-0-1-C".into()),
                ..ev(0, "X", None, "", 0, 0, 0, 0)
            },
        ];
        let t = totals(&header(), &events, 6001, &Extras::default());
        let me = t.actors.iter().find(|a| a.guid == "Player-1-A").unwrap();
        // 10 + 5 (the pet's) + 8 - 3 overkill.
        assert_eq!(me.damage_done, 20);
        assert_eq!(me.healing_done, 7);
        assert_eq!(me.overheal, 13);
        assert_eq!(me.damage_taken, 7);
        assert_eq!(me.kills, 1);
        let pet = t.actors.iter().find(|a| a.guid == "Pet-0-1-B").unwrap();
        assert_eq!(pet.damage_done, 5);
        let mob = t
            .actors
            .iter()
            .find(|a| a.guid == "Creature-0-1-C")
            .unwrap();
        assert_eq!(mob.deaths, 1);
        assert!(t.deaths.is_empty(), "an NPC's death isn't listed");
        assert_eq!(t.participants, vec!["Player-1-A"]);
        let melee = t
            .abilities
            .iter()
            .find(|a| a.actor == "Player-1-A" && a.spell_id == 1)
            .unwrap();
        assert_eq!(
            (melee.hits, melee.crits, melee.total, melee.max, melee.over),
            (2, 1, 15, 10, 3)
        );
        assert_eq!(melee.casts, 2, "no cast events: casts are the hits");
        assert_eq!(melee.name, "Melee");
        let s = &t.series.actors["Player-1-A"];
        assert_eq!(s.damage, vec![15, 5]);
        assert_eq!(s.healing, vec![7, 0]);
        // Active: [0,2000] ∪ [1000..] is the pet's; mine 0..2000, 3000..5000, 6000..6001.
        assert_eq!(me.active_ms, 2000 + 2000 + 1);
    }

    #[test]
    fn a_player_death_keeps_its_last_hits() {
        let mut events: Vec<DigestEvent> = (0..7)
            .map(|i| {
                ev(
                    i * 1000,
                    "D",
                    Some("Creature-0-1-C"),
                    "Player-1-A",
                    1,
                    10 + i,
                    0,
                    0,
                )
            })
            .collect();
        events.insert(
            0,
            ev(0, "D", Some("Creature-0-1-C"), "Player-1-A", 1, 1, 0, 0),
        );
        events.push(DigestEvent {
            t: 6500,
            e: "X".into(),
            d: Some("Player-1-A".into()),
            ..ev(0, "X", None, "", 0, 0, 0, 0)
        });
        let t = totals(&header(), &events, 6500, &Extras::default());
        let death = &t.deaths[0];
        assert_eq!(death.actor, "Player-1-A");
        assert_eq!(death.killer.as_deref(), Some("Creature-0-1-C"));
        let amounts: Vec<i64> = death.last_hits.iter().map(|h| h.amount).collect();
        assert_eq!(amounts, vec![12, 13, 14, 15, 16]);
        assert_eq!(death.last_hits[0].name, "Melee");
    }

    #[test]
    fn keys_are_stable_and_distinct() {
        let a = fight_key("wowf", Some(6933), "abc", "WoWCombatLog-1.txt", 1);
        assert_eq!(a.len(), 16);
        assert_eq!(
            a,
            fight_key("wowf", Some(6933), "abc", "WoWCombatLog-1.txt", 1)
        );
        assert_ne!(
            a,
            fight_key("wowf", Some(6933), "abc", "WoWCombatLog-1.txt", 2)
        );
    }
}
