//! Cutting a log into encounters (docs/wow.md section 9.2), line by line, so a growing file is
//! read as it grows.
//!
//! - **Boss**: `ENCOUNTER_START` … `ENCOUNTER_END`.
//! - **Pull**: otherwise a fight opens at the first damage, heal or miss that involves the
//!   logger's group (the logger, their party or raid, and their pets: the unit flags'
//!   affiliation bits), and ends after 8 s without one. Inside an instance it is `trash`.
//! - **World**: out in the world, pulls in the same zone less than 5 minutes apart are one `world`
//!   encounter.
//!
//! An open-world log carries every player nearby (one real 15-minute log named 96), so "involves
//! a player" means the group: a stranger's own fights never open or extend one. A stranger's event
//! is still counted while a fight is open when it touches the group or a unit the group engaged
//! (the same mob, a heal on a group member), and that stranger is then an actor and a participant.
//!
//! A fight is dropped when it lasts under 2 s, has no damage done or taken by the group, or has no
//! player in it.

use std::collections::{BTreeMap, BTreeSet, HashSet, VecDeque};

use serde::{Deserialize, Serialize};

use super::digest::{bits, ActorMeta, DigestEvent, Header, SpellMeta};
use super::parse::{self, flags, Event, Kind, Unit};
use super::summary::{self, Extras, Totals};

/// A pull ends after this long without the group doing or taking anything.
pub const PULL_GAP_MS: i64 = 8000;
/// Pulls in one zone closer than this are one world encounter.
pub const WORLD_GAP_MS: i64 = 5 * 60 * 1000;
/// A boss fight with no group event for this long (no `ENCOUNTER_END` came) is closed.
pub const BOSS_IDLE_MS: i64 = 60_000;
pub const MIN_FIGHT_MS: i64 = 2000;
/// Casts this close before a pull opens count in it (the cast that pulled lands after it).
pub const PREROLL_MS: i64 = 3000;
/// A swing and its `_LANDED` twin are written this close together.
pub const TWIN_WINDOW_MS: i64 = 50;
/// The client writes the log in bursts: a fight is closed by the wall clock only this long after
/// its gap has passed, so a late burst of its own lines still lands in it.
pub const FLUSH_GRACE_MS: i64 = 15_000;

/// What the log has said about where the logger is, and who owns what.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Context {
    pub zone: Option<String>,
    pub instance_id: Option<i64>,
    /// ZONE_CHANGE's last field: nonzero inside an instance.
    pub difficulty: i64,
    pub ui_map_id: Option<i64>,
    /// The logging character (the player flagged as the logger's own).
    pub logger: Option<String>,
    #[serde(default)]
    pub owners: BTreeMap<String, String>,
    /// Units seen flagged as the logger's group.
    #[serde(default)]
    pub group: BTreeSet<String>,
    /// How often each realm id appears in player GUIDs (the fallback for the logger's realm).
    #[serde(default)]
    pub realms: BTreeMap<i64, u64>,
    pub advanced: Option<bool>,
}

impl Context {
    pub fn in_instance(&self) -> bool {
        self.difficulty != 0
    }

    /// The logger's realm, else the realm most players in the log are from.
    pub fn realm_id(&self) -> Option<i64> {
        self.logger
            .as_deref()
            .and_then(parse::player_realm)
            .or_else(|| {
                self.realms
                    .iter()
                    .max_by_key(|(id, n)| (**n, std::cmp::Reverse(**id)))
                    .map(|(id, _)| *id)
            })
    }

    fn is_group(&self, u: &Unit) -> bool {
        if u.is_none() {
            return false;
        }
        u.flags & flags::AFFILIATION_GROUP != 0
            || self.group.contains(&u.guid)
            || self
                .owners
                .get(&u.guid)
                .is_some_and(|o| self.group.contains(o))
    }

    fn kind_of(&self, u: &Unit) -> &'static str {
        if u.guid.starts_with("Player-") || u.flags & flags::TYPE_PLAYER != 0 {
            "player"
        } else if u.guid.starts_with("Pet-")
            || u.flags & (flags::TYPE_PET | flags::TYPE_GUARDIAN) != 0
            || self.owners.contains_key(&u.guid)
        {
            "pet"
        } else {
            "npc"
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FightKind {
    Boss,
    Trash,
    World,
}

impl FightKind {
    pub fn as_str(self) -> &'static str {
        match self {
            FightKind::Boss => "boss",
            FightKind::Trash => "trash",
            FightKind::World => "world",
        }
    }
}

/// A finished encounter: what the summary is made of.
#[derive(Debug, Clone, PartialEq)]
pub struct Fight {
    pub kind: FightKind,
    pub encounter_id: Option<i64>,
    pub name: Option<String>,
    pub zone: Option<String>,
    pub instance: Option<String>,
    pub difficulty: Option<i64>,
    pub ui_map_id: Option<i64>,
    pub started_at: i64,
    pub ended_at: i64,
    pub success: Option<bool>,
    /// How many pulls a world encounter rolled up (1 otherwise).
    pub pulls: u32,
    /// Lines that carried the advanced block.
    pub advanced: bool,
    /// Log events counted in it.
    pub events: u64,
    pub header: Header,
    /// Times in ms from `started_at`.
    pub digest: Vec<DigestEvent>,
    pub extras: Extras,
    /// The fight's own lines, when raw lines are kept.
    pub raw: Vec<String>,
    pub realm_id: Option<i64>,
    /// The byte offset its first line started at.
    pub start_offset: u64,
}

impl Fight {
    pub fn duration_ms(&self) -> i64 {
        self.ended_at - self.started_at
    }

    pub fn totals(&self) -> Totals {
        summary::totals(&self.header, &self.digest, self.duration_ms(), &self.extras)
    }

    pub fn digest(&self) -> super::digest::Digest {
        super::digest::Digest {
            header: self.header.clone(),
            events: self.digest.clone(),
        }
    }

    /// The players in it (participants), for "2 players".
    pub fn players(&self) -> usize {
        self.totals().participants.len()
    }
}

#[derive(Debug, Clone)]
struct Builder {
    kind: FightKind,
    encounter_id: Option<i64>,
    name: Option<String>,
    difficulty: Option<i64>,
    ctx: Context,
    first_ts: i64,
    last_ts: i64,
    last_group_ts: i64,
    success: Option<bool>,
    pulls: u32,
    advanced: bool,
    events: u64,
    actors: BTreeMap<String, ActorMeta>,
    spells: BTreeMap<i64, SpellMeta>,
    /// Absolute times until the fight is finished.
    digest: Vec<DigestEvent>,
    extras: Extras,
    engaged: HashSet<String>,
    raw: Vec<String>,
    start_offset: u64,
    group_damage: bool,
}

impl Builder {
    fn new(kind: FightKind, ctx: &Context, ts: i64, offset: u64) -> Builder {
        Builder {
            kind,
            encounter_id: None,
            name: None,
            difficulty: ctx.in_instance().then_some(ctx.difficulty),
            ctx: ctx.clone(),
            first_ts: ts,
            last_ts: ts,
            last_group_ts: ts,
            success: None,
            pulls: 1,
            advanced: false,
            events: 0,
            actors: BTreeMap::new(),
            spells: BTreeMap::new(),
            digest: Vec::new(),
            extras: Extras::default(),
            engaged: HashSet::new(),
            raw: Vec::new(),
            start_offset: offset,
            group_damage: false,
        }
    }

    fn knows(&self, u: &Unit) -> bool {
        !u.is_none() && (self.engaged.contains(&u.guid) || self.actors.contains_key(&u.guid))
    }

    fn actor(&mut self, ctx: &Context, u: &Unit) {
        if u.is_none() {
            return;
        }
        let name = u.name.as_deref().map(|n| parse::display_name(n, &u.guid));
        let kind = ctx.kind_of(u);
        let owner = ctx.owners.get(&u.guid).cloned();
        let e = self.actors.entry(u.guid.clone()).or_insert(ActorMeta {
            name: None,
            kind: kind.to_string(),
            class: None,
            owner: None,
        });
        if e.name.is_none() {
            e.name = name;
        }
        if owner.is_some() {
            e.owner = owner;
            e.kind = "pet".into();
        }
    }

    fn spell(&mut self, id: i64, name: &str, school: i64) {
        self.spells.entry(id).or_insert_with(|| SpellMeta {
            name: name.to_string(),
            school,
        });
    }

    /// Fold a later pull into this world encounter.
    fn absorb(&mut self, other: Builder) {
        self.last_ts = self.last_ts.max(other.last_ts);
        self.last_group_ts = self.last_group_ts.max(other.last_group_ts);
        self.pulls += other.pulls;
        self.advanced |= other.advanced;
        self.events += other.events;
        self.group_damage |= other.group_damage;
        for (g, m) in other.actors {
            match self.actors.get_mut(&g) {
                Some(mine) => {
                    if mine.name.is_none() {
                        mine.name = m.name;
                    }
                    if m.owner.is_some() {
                        mine.owner = m.owner;
                        mine.kind = m.kind;
                    }
                }
                None => {
                    self.actors.insert(g, m);
                }
            }
        }
        for (id, s) in other.spells {
            self.spells.entry(id).or_insert(s);
        }
        self.digest.extend(other.digest);
        for (k, n) in other.extras.casts {
            *self.extras.casts.entry(k).or_default() += n;
        }
        for (k, n) in other.extras.interrupts {
            *self.extras.interrupts.entry(k).or_default() += n;
        }
        for (k, n) in other.extras.dispels {
            *self.extras.dispels.entry(k).or_default() += n;
        }
        self.engaged.extend(other.engaged);
        self.raw.extend(other.raw);
    }

    /// The fight, or None when it doesn't count (too short, no damage, no player).
    fn finish(mut self, ctx: &Context) -> Option<Fight> {
        if self.last_ts - self.first_ts < MIN_FIGHT_MS || !self.group_damage {
            return None;
        }
        dedupe_swings(&mut self.digest);
        let start = self.first_ts;
        for e in &mut self.digest {
            e.t -= start;
        }
        // Pets learned of after their first event.
        for (pet, owner) in &ctx.owners {
            if let Some(m) = self.actors.get_mut(pet) {
                if m.owner.is_none() {
                    m.owner = Some(owner.clone());
                    m.kind = "pet".into();
                }
            }
        }
        let instance = self
            .ctx
            .in_instance()
            .then(|| self.ctx.zone.clone())
            .flatten();
        let fight = Fight {
            kind: self.kind,
            encounter_id: self.encounter_id,
            name: self.name,
            zone: self.ctx.zone.clone(),
            instance,
            difficulty: self.difficulty,
            ui_map_id: self.ctx.ui_map_id,
            started_at: self.first_ts,
            ended_at: self.last_ts,
            success: self.success,
            pulls: self.pulls,
            advanced: self.advanced,
            events: self.events,
            header: Header {
                actors: self.actors,
                spells: self.spells,
            },
            digest: self.digest,
            extras: self.extras,
            raw: self.raw,
            realm_id: ctx.realm_id(),
            start_offset: self.start_offset,
        };
        if fight.totals().participants.is_empty() {
            return None;
        }
        Some(fight)
    }
}

/// Drop each `SWING_DAMAGE` whose `SWING_DAMAGE_LANDED` twin (same source, target and amount,
/// within a few ms) is there: the client writes both for most hits, and some hits with only one
/// of the two, so the pair counts once and a lone one of either kind still counts.
pub fn dedupe_swings(events: &mut Vec<DigestEvent>) {
    let mut drop = vec![false; events.len()];
    let mut used = vec![false; events.len()];
    for i in 0..events.len() {
        if !events[i].twin {
            continue;
        }
        let key = (&events[i].s, &events[i].d, events[i].a);
        // Look both ways inside the window for an unused landed twin.
        let lo = (0..i)
            .rev()
            .take_while(|&j| events[i].t - events[j].t <= TWIN_WINDOW_MS)
            .last()
            .unwrap_or(i);
        let hi = (i + 1..events.len())
            .take_while(|&j| events[j].t - events[i].t <= TWIN_WINDOW_MS)
            .last()
            .unwrap_or(i);
        let twin = (lo..=hi).find(|&j| {
            j != i
                && !used[j]
                && events[j].flags() & bits::SWING_LANDED != 0
                && (&events[j].s, &events[j].d, events[j].a) == key
        });
        if let Some(j) = twin {
            used[j] = true;
            drop[i] = true;
        }
    }
    let mut i = 0;
    events.retain(|_| {
        let keep = !drop[i];
        i += 1;
        keep
    });
    for e in events.iter_mut() {
        e.twin = false;
    }
}

/// Reads a log a line at a time and hands back each encounter as it ends.
#[derive(Debug, Default)]
pub struct Segmenter {
    pub ctx: Context,
    /// Keep each fight's own lines (for the raw upload).
    pub keep_raw: bool,
    pull: Option<Builder>,
    world: Option<Builder>,
    recent_casts: VecDeque<(i64, String, i64, String)>,
    done: Vec<Fight>,
    /// Lines read as events, by subevent; and lines that weren't events at all.
    pub subevents: BTreeMap<String, u64>,
    pub unreadable: u64,
    pub dropped: u64,
    /// The newest event time seen (unix ms).
    pub last_event_at: Option<i64>,
}

fn is_unit_kind(k: &Kind) -> bool {
    matches!(
        k,
        Kind::Damage { .. } | Kind::Heal { .. } | Kind::Miss { .. } | Kind::Absorb { .. }
    )
}

impl Segmenter {
    pub fn new(keep_raw: bool) -> Segmenter {
        Segmenter {
            keep_raw,
            ..Segmenter::default()
        }
    }

    /// Resume with what an earlier read knew.
    pub fn with_context(ctx: Context, keep_raw: bool) -> Segmenter {
        Segmenter {
            ctx,
            keep_raw,
            ..Segmenter::default()
        }
    }

    /// The fights that ended since the last call.
    pub fn take(&mut self) -> Vec<Fight> {
        std::mem::take(&mut self.done)
    }

    /// Encounters still open.
    pub fn open(&self) -> usize {
        usize::from(self.pull.is_some()) + usize::from(self.world.is_some())
    }

    /// Where a re-read must start to rebuild what's open (the first line of the oldest open
    /// encounter), and the context as it was there; None when nothing is open.
    pub fn resume_point(&self) -> Option<(u64, Context)> {
        [self.world.as_ref(), self.pull.as_ref()]
            .into_iter()
            .flatten()
            .min_by_key(|b| b.start_offset)
            .map(|b| (b.start_offset, b.ctx.clone()))
    }

    fn emit(&mut self, b: Builder) {
        match b.finish(&self.ctx) {
            Some(f) => self.done.push(f),
            None => self.dropped += 1,
        }
    }

    fn close_world(&mut self) {
        if let Some(w) = self.world.take() {
            self.emit(w);
        }
    }

    /// End the open pull: trash and bosses are done; a world pull joins the open world
    /// encounter if it's close enough, else ends it and starts the next.
    fn close_pull(&mut self) {
        let Some(p) = self.pull.take() else { return };
        match p.kind {
            FightKind::World => {
                let joins = self.world.as_ref().is_some_and(|w| {
                    w.ctx.zone == p.ctx.zone && p.first_ts - w.last_ts < WORLD_GAP_MS
                });
                if joins {
                    if let Some(w) = self.world.as_mut() {
                        w.absorb(p);
                    }
                } else if p.last_ts - p.first_ts >= MIN_FIGHT_MS && p.group_damage {
                    self.close_world();
                    self.world = Some(p);
                } else {
                    // A stray tick doesn't end the roll-up; it's just dropped.
                    self.dropped += 1;
                }
            }
            _ => self.emit(p),
        }
    }

    /// Close what the clock says is over: `now` is unix ms (the wall clock while tailing).
    pub fn tick(&mut self, now: i64) {
        if let Some(p) = &self.pull {
            let gap = if p.kind == FightKind::Boss {
                BOSS_IDLE_MS
            } else {
                PULL_GAP_MS
            };
            if now - p.last_group_ts > gap + FLUSH_GRACE_MS {
                self.close_pull();
            }
        }
        if self.pull.is_none() {
            if let Some(w) = &self.world {
                if now - w.last_ts > WORLD_GAP_MS + FLUSH_GRACE_MS {
                    self.close_world();
                }
            }
        }
    }

    /// End everything (the end of a file nobody writes any more, or a quit).
    pub fn finish(&mut self) {
        self.close_pull();
        self.close_world();
    }

    /// One line, starting at byte `offset` of its file.
    pub fn feed(&mut self, line: &str, offset: u64) {
        let ev = match parse::parse_line(line) {
            Ok(e) => e,
            Err(_) => {
                if !line.trim().is_empty() {
                    self.unreadable += 1;
                }
                return;
            }
        };
        *self.subevents.entry(ev.subevent.clone()).or_default() += 1;
        self.last_event_at = Some(self.last_event_at.map_or(ev.ts, |t| t.max(ev.ts)));
        self.learn(&ev);
        // The event clock closes what its gap has passed.
        if let Some(p) = &self.pull {
            let gap = if p.kind == FightKind::Boss {
                BOSS_IDLE_MS
            } else {
                PULL_GAP_MS
            };
            if ev.ts - p.last_group_ts > gap {
                self.close_pull();
            }
        }
        if self.pull.is_none() {
            if let Some(w) = &self.world {
                if ev.ts - w.last_ts > WORLD_GAP_MS {
                    self.close_world();
                }
            }
        }
        match &ev.kind {
            Kind::ZoneChange {
                instance_id,
                name,
                difficulty,
            } => {
                let known = name != "UNKNOWN AREA" && !name.is_empty();
                let moved = (known && self.ctx.zone.as_deref() != Some(name.as_str()))
                    || self.ctx.difficulty != *difficulty
                    || self.ctx.instance_id.is_some_and(|i| i != *instance_id);
                if moved {
                    self.finish();
                }
                if known {
                    self.ctx.zone = Some(name.clone());
                }
                self.ctx.instance_id = Some(*instance_id);
                self.ctx.difficulty = *difficulty;
            }
            Kind::MapChange { ui_map_id, name } => {
                self.ctx.ui_map_id = Some(*ui_map_id);
                if self.ctx.zone.is_none() && !name.is_empty() {
                    self.ctx.zone = Some(name.clone());
                }
            }
            Kind::Header { advanced, .. } => {
                self.ctx.advanced = Some(*advanced);
            }
            Kind::EncounterStart {
                encounter_id,
                name,
                difficulty,
                ..
            } => {
                self.close_pull();
                self.close_world();
                let mut b = Builder::new(FightKind::Boss, &self.ctx, ev.ts, offset);
                b.encounter_id = Some(*encounter_id);
                b.name = Some(name.clone());
                b.difficulty = Some(*difficulty);
                self.pull = Some(b);
            }
            Kind::EncounterEnd {
                encounter_id,
                success,
                ..
            } => {
                if let Some(p) = self.pull.as_mut() {
                    if p.kind == FightKind::Boss && p.encounter_id == Some(*encounter_id) {
                        p.success = Some(*success);
                        p.last_ts = p.last_ts.max(ev.ts);
                        if self.keep_raw {
                            p.raw.push(line.to_string());
                        }
                        let b = self.pull.take().expect("checked");
                        self.emit(b);
                    }
                }
            }
            Kind::CastSuccess => self.cast(&ev, line),
            Kind::Interrupt | Kind::Dispel => {
                if let Some(p) = self.pull.as_mut() {
                    if self.ctx.is_group(&ev.src) || p.knows(&ev.src) {
                        let map = if ev.kind == Kind::Interrupt {
                            &mut p.extras.interrupts
                        } else {
                            &mut p.extras.dispels
                        };
                        *map.entry(ev.src.guid.clone()).or_default() += 1;
                        if self.keep_raw {
                            p.raw.push(line.to_string());
                        }
                    }
                }
            }
            Kind::Died => {
                if let Some(p) = self.pull.as_mut() {
                    if self.ctx.is_group(&ev.dst) || p.knows(&ev.dst) {
                        p.actor(&self.ctx, &ev.dst);
                        p.digest.push(DigestEvent {
                            t: ev.ts,
                            e: "X".into(),
                            s: None,
                            d: Some(ev.dst.guid.clone()),
                            sp: None,
                            a: None,
                            o: None,
                            sc: None,
                            f: None,
                            m: None,
                            twin: false,
                        });
                        p.last_ts = p.last_ts.max(ev.ts);
                        p.events += 1;
                        if self.keep_raw {
                            p.raw.push(line.to_string());
                        }
                    }
                }
            }
            k if is_unit_kind(k) => self.combat(&ev, line, offset),
            _ => {}
        }
    }

    /// Owners, the logger, the group and realms, from any event.
    fn learn(&mut self, ev: &Event) {
        if let Some(adv) = &ev.advanced {
            if let Some(owner) = &adv.owner_guid {
                self.ctx.owners.insert(adv.info_guid.clone(), owner.clone());
            }
        }
        if ev.kind == Kind::Summon && !ev.src.is_none() && !ev.dst.is_none() {
            self.ctx
                .owners
                .insert(ev.dst.guid.clone(), ev.src.guid.clone());
        }
        for u in [&ev.src, &ev.dst] {
            if u.is_none() {
                continue;
            }
            if u.guid.starts_with("Player-") {
                if u.flags & flags::AFFILIATION_MINE != 0
                    && self.ctx.logger.as_deref() != Some(&u.guid)
                {
                    self.ctx.logger = Some(u.guid.clone());
                }
                if let Some(r) = parse::player_realm(&u.guid) {
                    *self.ctx.realms.entry(r).or_default() += 1;
                }
            }
            if u.flags & flags::AFFILIATION_GROUP != 0 && !self.ctx.group.contains(&u.guid) {
                self.ctx.group.insert(u.guid.clone());
            }
        }
    }

    fn cast(&mut self, ev: &Event, line: &str) {
        let Some(spell) = &ev.spell else { return };
        if let Some(p) = self.pull.as_mut() {
            if self.ctx.is_group(&ev.src) || p.knows(&ev.src) {
                *p.extras
                    .casts
                    .entry((ev.src.guid.clone(), spell.id))
                    .or_default() += 1;
                if self.keep_raw {
                    p.raw.push(line.to_string());
                }
            }
            return;
        }
        if self.ctx.is_group(&ev.src) {
            self.recent_casts.push_back((
                ev.ts,
                ev.src.guid.clone(),
                spell.id,
                if self.keep_raw {
                    line.to_string()
                } else {
                    String::new()
                },
            ));
            while self
                .recent_casts
                .front()
                .is_some_and(|(t, ..)| ev.ts - t > PREROLL_MS)
            {
                self.recent_casts.pop_front();
            }
        }
    }

    fn combat(&mut self, ev: &Event, line: &str, offset: u64) {
        let absorber = match &ev.kind {
            Kind::Absorb { absorber, .. } => Some(absorber.clone()),
            _ => None,
        };
        let group_involved = self.ctx.is_group(&ev.src)
            || self.ctx.is_group(&ev.dst)
            || absorber.as_ref().is_some_and(|a| self.ctx.is_group(a));
        if self.pull.is_none() {
            let opens = group_involved
                && matches!(
                    ev.kind,
                    Kind::Damage { .. } | Kind::Heal { .. } | Kind::Miss { .. }
                );
            if !opens {
                return;
            }
            let kind = if self.ctx.in_instance() {
                FightKind::Trash
            } else {
                FightKind::World
            };
            let mut b = Builder::new(kind, &self.ctx, ev.ts, offset);
            for (t, src, spell, raw) in self.recent_casts.drain(..) {
                if ev.ts - t <= PREROLL_MS {
                    *b.extras.casts.entry((src, spell)).or_default() += 1;
                    if self.keep_raw {
                        b.raw.push(raw);
                    }
                }
            }
            self.pull = Some(b);
        }
        let ctx = &self.ctx;
        let p = self.pull.as_mut().expect("opened above");
        let relevant = group_involved
            || p.knows(&ev.src)
            || p.knows(&ev.dst)
            || absorber.as_ref().is_some_and(|a| p.knows(a));
        if !relevant {
            return;
        }
        if group_involved {
            p.last_group_ts = p.last_group_ts.max(ev.ts);
            for u in [&ev.src, &ev.dst] {
                if !u.is_none() && !ctx.is_group(u) && ctx.kind_of(u) == "npc" {
                    p.engaged.insert(u.guid.clone());
                }
            }
        }
        p.last_ts = p.last_ts.max(ev.ts);
        p.events += 1;
        if ev.advanced.is_some() {
            p.advanced = true;
        }
        if self.keep_raw {
            p.raw.push(line.to_string());
        }
        p.actor(ctx, &ev.src);
        p.actor(ctx, &ev.dst);
        let src = (!ev.src.is_none()).then(|| ev.src.guid.clone());
        let dst = (!ev.dst.is_none()).then(|| ev.dst.guid.clone());
        let spell = ev.spell.clone().unwrap_or_else(parse::melee);
        let base = DigestEvent {
            t: ev.ts,
            e: String::new(),
            s: src,
            d: dst,
            sp: Some(spell.id),
            a: None,
            o: None,
            sc: None,
            f: None,
            m: None,
            twin: false,
        };
        let digest = match &ev.kind {
            Kind::Damage {
                amount,
                overkill,
                school,
                critical,
                glancing,
                crushing,
                periodic,
                landed,
                swing,
                environment,
                ..
            } => {
                let (sp, name, school) = match environment {
                    Some(env) => (environment_id(env), format!("{env} (environment)"), *school),
                    None => (spell.id, spell.name.clone(), *school),
                };
                p.spell(sp, &name, school);
                if group_involved {
                    p.group_damage = true;
                }
                let mut f = 0;
                if *critical {
                    f |= bits::CRIT;
                }
                if *glancing {
                    f |= bits::GLANCING;
                }
                if *crushing {
                    f |= bits::CRUSHING;
                }
                if *periodic {
                    f |= bits::PERIODIC;
                }
                if *landed {
                    f |= bits::SWING_LANDED;
                }
                DigestEvent {
                    e: "D".into(),
                    sp: Some(sp),
                    a: Some(*amount),
                    o: Some(*overkill),
                    sc: Some(school),
                    f: Some(f),
                    twin: *swing,
                    ..base
                }
            }
            Kind::Heal {
                amount,
                overheal,
                critical,
                periodic,
                ..
            } => {
                p.spell(spell.id, &spell.name, spell.school);
                let mut f = 0;
                if *critical {
                    f |= bits::CRIT;
                }
                if *periodic {
                    f |= bits::PERIODIC;
                }
                DigestEvent {
                    e: "H".into(),
                    a: Some(*amount),
                    o: Some(*overheal),
                    sc: Some(spell.school),
                    f: Some(f),
                    ..base
                }
            }
            Kind::Miss {
                miss_type, amount, ..
            } => {
                p.spell(spell.id, &spell.name, spell.school);
                DigestEvent {
                    e: "M".into(),
                    a: *amount,
                    m: Some(miss_type.clone()),
                    ..base
                }
            }
            Kind::Absorb {
                absorber,
                shield,
                amount,
            } => {
                p.actor(ctx, absorber);
                p.spell(shield.id, &shield.name, shield.school);
                DigestEvent {
                    e: "A".into(),
                    s: (!absorber.is_none()).then(|| absorber.guid.clone()),
                    sp: Some(shield.id),
                    a: Some(*amount),
                    o: Some(0),
                    sc: Some(shield.school),
                    f: Some(0),
                    ..base
                }
            }
            _ => return,
        };
        p.digest.push(digest);
    }
}

/// Environmental damage has no spell: each type gets a fixed negative id.
pub fn environment_id(env: &str) -> i64 {
    match env.to_ascii_lowercase().as_str() {
        "falling" => -1,
        "drowning" => -2,
        "fatigue" => -3,
        "fire" => -4,
        "lava" => -5,
        "slime" => -6,
        _ => -9,
    }
}

/// Every fight in a whole file's text (the CLI's `combat`, and the tests).
pub fn segment_text(text: &str, keep_raw: bool) -> (Vec<Fight>, Segmenter) {
    let mut seg = Segmenter::new(keep_raw);
    let mut offset = 0u64;
    for line in text.split_inclusive('\n') {
        seg.feed(line, offset);
        offset += line.len() as u64;
    }
    seg.finish();
    let fights = seg.take();
    (fights, seg)
}
