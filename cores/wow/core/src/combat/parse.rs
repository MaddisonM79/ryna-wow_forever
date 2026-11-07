//! One combat log line into an [`Event`] (docs/wow.md section 9.1).
//!
//! `M/D/YYYY HH:MM:SS.mmm±TZ  SUBEVENT,field,field,…`: the time is the machine's local clock with
//! its offset in hours, so it becomes unix ms with the offset applied. Fields are CSV with quoted
//! strings (which may hold commas); `nil` is null.
//!
//! What Forever (version 22, build 1.60.1) actually writes, checked field by field against two
//! real logs of 2026-11-06:
//!
//! - every unit event: `srcGUID, srcName, srcFlags, srcRaidFlags, dstGUID, dstName, dstFlags,
//!   dstRaidFlags`, then for spell events `spellId, spellName, spellSchool`;
//! - the advanced block, on `*_DAMAGE`, `*_HEAL`, `SPELL_ENERGIZE`, `SPELL_CAST_SUCCESS` and
//!   `ENVIRONMENTAL_DAMAGE`, is **19** fields here (retail's 17 plus two after `absorb`):
//!   `infoGUID, ownerGUID, currHP, maxHP, attackPower, spellPower, armor, absorb, ?, ?, powerType,
//!   currPower, maxPower, powerCost, posX, posY, uiMapID, facing, level`. It is detected per line
//!   by its shape (two GUID-like fields, then enough fields for a block), never by the header's
//!   `ADVANCED_LOG_ENABLED`, which said 0 on lines that carried it;
//! - the damage suffix is `amount, baseAmount, overkill, school, resisted, blocked, absorbed,
//!   critical, glancing, crushing`, and spell damage adds a trailing `ST` (kept opaque). There is
//!   no `isOffHand`. `overkill` is -1 when there was none;
//! - the heal suffix is `amount, baseAmount, overhealing, absorbed, critical`;
//! - `ENVIRONMENTAL_DAMAGE` has `environmentalType` before the damage suffix;
//! - `*_MISSED`: `missType, isOffHand[, amountMissed][, ST]`.
//!
//! Anything this doesn't know is [`Kind::Other`], counted by the caller, never an error.

/// A line that couldn't be read as an event at all (no timestamp, or no subevent).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unreadable;

/// The fields of the advanced block that the agent uses.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Advanced {
    /// The unit the block describes.
    pub info_guid: String,
    /// That unit's owner (a pet's player), when it has one.
    pub owner_guid: Option<String>,
    pub ui_map_id: Option<i64>,
    pub level: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Unit {
    /// `0000000000000000` (or empty) for none.
    pub guid: String,
    pub name: Option<String>,
    pub flags: u32,
}

impl Unit {
    pub fn is_none(&self) -> bool {
        self.guid.is_empty() || self.guid == NO_GUID
    }
}

pub const NO_GUID: &str = "0000000000000000";

/// Unit flags (COMBATLOG_OBJECT_*).
pub mod flags {
    pub const AFFILIATION_MINE: u32 = 0x1;
    pub const AFFILIATION_PARTY: u32 = 0x2;
    pub const AFFILIATION_RAID: u32 = 0x4;
    pub const AFFILIATION_GROUP: u32 = AFFILIATION_MINE | AFFILIATION_PARTY | AFFILIATION_RAID;
    pub const REACTION_FRIENDLY: u32 = 0x10;
    pub const REACTION_HOSTILE: u32 = 0x40;
    pub const CONTROL_PLAYER: u32 = 0x100;
    pub const TYPE_PLAYER: u32 = 0x400;
    pub const TYPE_NPC: u32 = 0x800;
    pub const TYPE_PET: u32 = 0x1000;
    pub const TYPE_GUARDIAN: u32 = 0x2000;
}

#[derive(Debug, Clone, PartialEq)]
pub struct Spell {
    pub id: i64,
    pub name: String,
    pub school: i64,
}

/// The melee "spell": `SWING_*` events carry none.
pub fn melee() -> Spell {
    Spell {
        id: 1,
        name: "Melee".into(),
        school: 1,
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Kind {
    Damage {
        amount: i64,
        /// 0 when the hit didn't kill (the log's -1).
        overkill: i64,
        school: i64,
        absorbed: i64,
        critical: bool,
        glancing: bool,
        crushing: bool,
        periodic: bool,
        /// `SWING_DAMAGE_LANDED` (the client writes it beside `SWING_DAMAGE` for most hits).
        landed: bool,
        /// `SWING_DAMAGE` (as opposed to its `_LANDED` twin, or any other damage).
        swing: bool,
        /// `ENVIRONMENTAL_DAMAGE`'s type (Falling, Drowning, …).
        environment: Option<String>,
    },
    Heal {
        amount: i64,
        overheal: i64,
        absorbed: i64,
        critical: bool,
        periodic: bool,
    },
    Miss {
        miss_type: String,
        amount: Option<i64>,
        periodic: bool,
    },
    /// `SPELL_ABSORBED`: `absorber` soaked `amount` of a hit on the event's dst with `shield`.
    Absorb {
        absorber: Unit,
        shield: Spell,
        amount: i64,
    },
    Died,
    PartyKill,
    CastSuccess,
    Summon,
    Interrupt,
    Dispel,
    ZoneChange {
        instance_id: i64,
        name: String,
        /// Nonzero inside an instance.
        difficulty: i64,
    },
    MapChange {
        ui_map_id: i64,
        name: String,
    },
    EncounterStart {
        encounter_id: i64,
        name: String,
        difficulty: i64,
        group_size: i64,
        instance_id: Option<i64>,
    },
    EncounterEnd {
        encounter_id: i64,
        name: String,
        difficulty: i64,
        group_size: i64,
        success: bool,
    },
    /// `COMBAT_LOG_VERSION,22,ADVANCED_LOG_ENABLED,0,…`: logging (re)started.
    Header {
        version: i64,
        advanced: bool,
    },
    /// Read, but not something the agent counts (auras, cast starts, energize, …).
    Other,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Event {
    /// Unix ms (the line's local time with its offset applied).
    pub ts: i64,
    pub subevent: String,
    pub src: Unit,
    pub dst: Unit,
    pub spell: Option<Spell>,
    pub advanced: Option<Advanced>,
    pub kind: Kind,
}

/// Split a line into its timestamp (unix ms) and the rest, the CSV after the two spaces.
pub fn split_time(line: &str) -> Option<(i64, &str)> {
    let line = line.trim_end_matches(['\r', '\n']);
    let (stamp, rest) = line.split_once("  ")?;
    Some((parse_time(stamp)?, rest))
}

/// `M/D/YYYY HH:MM:SS.mmm±H` (also `±H.5`, `±HH:MM`, or no offset) as unix ms.
pub fn parse_time(stamp: &str) -> Option<i64> {
    let (date, time) = stamp.trim().split_once(' ')?;
    let mut d = date.split('/');
    let month: i64 = d.next()?.parse().ok()?;
    let day: i64 = d.next()?.parse().ok()?;
    let year: i64 = d.next()?.parse().ok()?;
    if d.next().is_some() || !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    // The offset starts at the first sign after the seconds.
    let sign_at = time.find(['+', '-']);
    let (clock, offset_ms) = match sign_at {
        Some(i) => (&time[..i], parse_offset(&time[i..])?),
        None => (time, 0),
    };
    let mut c = clock.split(':');
    let h: i64 = c.next()?.parse().ok()?;
    let m: i64 = c.next()?.parse().ok()?;
    let secs = c.next()?;
    if c.next().is_some() {
        return None;
    }
    let (s, frac) = secs.split_once('.').unwrap_or((secs, "0"));
    let s: i64 = s.parse().ok()?;
    let mut ms: i64 = 0;
    let mut scale = 100;
    for ch in frac.chars().take(3) {
        ms += i64::from(ch.to_digit(10)?) * scale;
        scale /= 10;
    }
    let local =
        days_from_civil(year, month, day) * 86_400_000 + ((h * 60 + m) * 60 + s) * 1000 + ms;
    Some(local - offset_ms)
}

fn parse_offset(s: &str) -> Option<i64> {
    let sign = if s.starts_with('-') { -1 } else { 1 };
    let body = &s[1..];
    let minutes = if let Some((h, m)) = body.split_once(':') {
        h.parse::<i64>().ok()? * 60 + m.parse::<i64>().ok()?
    } else {
        let hours: f64 = body.parse().ok()?;
        (hours * 60.0).round() as i64
    };
    Some(sign * minutes * 60_000)
}

/// Days since 1970-01-01 (Howard Hinnant's days_from_civil).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// The CSV fields, quotes removed. A quoted field runs to the next `"` that ends the field.
pub fn split_fields(body: &str) -> Vec<&str> {
    let mut out = Vec::with_capacity(48);
    let bytes = body.as_bytes();
    let mut i = 0;
    loop {
        if i < bytes.len() && bytes[i] == b'"' {
            let start = i + 1;
            let mut j = start;
            // A quote ends the field only when a comma or the end follows it.
            loop {
                match body[j..].find('"') {
                    Some(k) => {
                        let q = j + k;
                        if q + 1 >= bytes.len() || bytes[q + 1] == b',' {
                            out.push(&body[start..q]);
                            i = q + 1;
                            break;
                        }
                        j = q + 1;
                    }
                    None => {
                        out.push(&body[start..]);
                        i = bytes.len();
                        break;
                    }
                }
            }
        } else {
            let end = body[i..].find(',').map_or(bytes.len(), |k| i + k);
            out.push(&body[i..end]);
            i = end;
        }
        if i >= bytes.len() {
            break;
        }
        // Skip the comma.
        i += 1;
        if i == bytes.len() {
            out.push("");
            break;
        }
    }
    out
}

fn opt(s: &str) -> Option<&str> {
    if s == "nil" || s.is_empty() {
        None
    } else {
        Some(s)
    }
}

fn int(s: &str) -> i64 {
    let s = s.trim();
    if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        return i64::from_str_radix(hex, 16).unwrap_or(0);
    }
    s.parse::<i64>()
        .ok()
        .or_else(|| s.parse::<f64>().ok().map(|f| f as i64))
        .unwrap_or(0)
}

fn flag_bits(s: &str) -> u32 {
    int(s) as u32
}

fn truthy(s: &str) -> bool {
    !matches!(s, "nil" | "" | "0")
}

fn looks_like_guid(s: &str) -> bool {
    s == NO_GUID || (s.contains('-') && s.chars().next().is_some_and(|c| c.is_ascii_alphabetic()))
}

fn is_number(s: &str) -> bool {
    !s.is_empty()
        && s.trim_start_matches('-')
            .bytes()
            .all(|b| b.is_ascii_digit())
}

fn unit(f: &[&str], at: usize) -> Unit {
    Unit {
        guid: f.get(at).copied().unwrap_or("").to_string(),
        name: f.get(at + 1).copied().and_then(opt).map(str::to_string),
        flags: f.get(at + 2).map_or(0, |s| flag_bits(s)),
    }
}

fn spell_at(f: &[&str], at: usize) -> Option<Spell> {
    Some(Spell {
        id: int(f.get(at)?),
        name: f.get(at + 1).copied().unwrap_or("").to_string(),
        school: int(f.get(at + 2).copied().unwrap_or("0")),
    })
}

/// The lengths of the advanced block this parser knows: Forever's, then retail's.
const ADVANCED_LENGTHS: [usize; 2] = [19, 17];

/// Whether an advanced block starts at `at`, and its length, given that `suffix` fields at least
/// must follow it.
fn advanced_at(f: &[&str], at: usize, suffix: usize) -> Option<usize> {
    let first = f.get(at)?;
    let second = f.get(at + 1)?;
    if !looks_like_guid(first) || !looks_like_guid(second) {
        return None;
    }
    ADVANCED_LENGTHS
        .into_iter()
        .find(|len| f.len() >= at + len + suffix)
}

fn read_advanced(f: &[&str], at: usize, len: usize) -> Advanced {
    let owner = f[at + 1];
    Advanced {
        info_guid: f[at].to_string(),
        owner_guid: (owner != NO_GUID && looks_like_guid(owner)).then(|| owner.to_string()),
        ui_map_id: f.get(at + len - 3).map(|s| int(s)).filter(|&n| n > 0),
        level: f.get(at + len - 1).map(|s| int(s)),
    }
}

const DAMAGE_SUFFIX: usize = 10;
const HEAL_SUFFIX: usize = 5;

fn damage(f: &[&str], s: usize, periodic: bool, landed: bool, swing: bool) -> Kind {
    let g = |i: usize| f.get(s + i).copied().unwrap_or("nil");
    let overkill = int(g(2));
    Kind::Damage {
        amount: int(g(0)).max(0),
        overkill: overkill.max(0),
        school: int(g(3)),
        absorbed: int(g(6)).max(0),
        critical: truthy(g(7)),
        glancing: truthy(g(8)),
        crushing: truthy(g(9)),
        periodic,
        landed,
        swing,
        environment: None,
    }
}

/// One line, or [`Unreadable`] when it has no timestamp or no subevent.
pub fn parse_line(line: &str) -> Result<Event, Unreadable> {
    let (ts, body) = split_time(line).ok_or(Unreadable)?;
    let f = split_fields(body);
    let sub = *f.first().ok_or(Unreadable)?;
    if sub.is_empty() {
        return Err(Unreadable);
    }
    let mut ev = Event {
        ts,
        subevent: sub.to_string(),
        src: Unit::default(),
        dst: Unit::default(),
        spell: None,
        advanced: None,
        kind: Kind::Other,
    };
    match sub {
        "COMBAT_LOG_VERSION" => {
            ev.kind = Kind::Header {
                version: int(f.get(1).copied().unwrap_or("0")),
                advanced: f
                    .iter()
                    .position(|x| *x == "ADVANCED_LOG_ENABLED")
                    .and_then(|i| f.get(i + 1))
                    .is_some_and(|v| truthy(v)),
            };
            return Ok(ev);
        }
        "ZONE_CHANGE" => {
            ev.kind = Kind::ZoneChange {
                instance_id: int(f.get(1).copied().unwrap_or("0")),
                name: f.get(2).copied().unwrap_or("").to_string(),
                difficulty: int(f.get(3).copied().unwrap_or("0")),
            };
            return Ok(ev);
        }
        "MAP_CHANGE" => {
            ev.kind = Kind::MapChange {
                ui_map_id: int(f.get(1).copied().unwrap_or("0")),
                name: f.get(2).copied().unwrap_or("").to_string(),
            };
            return Ok(ev);
        }
        "ENCOUNTER_START" => {
            ev.kind = Kind::EncounterStart {
                encounter_id: int(f.get(1).copied().unwrap_or("0")),
                name: f.get(2).copied().unwrap_or("").to_string(),
                difficulty: int(f.get(3).copied().unwrap_or("0")),
                group_size: int(f.get(4).copied().unwrap_or("0")),
                instance_id: f.get(5).map(|s| int(s)),
            };
            return Ok(ev);
        }
        "ENCOUNTER_END" => {
            ev.kind = Kind::EncounterEnd {
                encounter_id: int(f.get(1).copied().unwrap_or("0")),
                name: f.get(2).copied().unwrap_or("").to_string(),
                difficulty: int(f.get(3).copied().unwrap_or("0")),
                group_size: int(f.get(4).copied().unwrap_or("0")),
                success: f.get(5).is_some_and(|s| truthy(s)),
            };
            return Ok(ev);
        }
        _ => {}
    }
    // Every other event this parser reads has the eight-field unit prefix.
    if f.len() < 9 || !looks_like_guid(f[1]) && !f[1].is_empty() {
        return Ok(ev);
    }
    ev.src = unit(&f, 1);
    ev.dst = unit(&f, 5);
    let has_spell = sub.starts_with("SPELL_")
        || sub.starts_with("RANGE_")
        || sub.starts_with("DAMAGE_SHIELD")
        || sub == "DAMAGE_SPLIT";
    let after_prefix = if has_spell {
        ev.spell = spell_at(&f, 9);
        12
    } else {
        9
    };
    let periodic = sub.starts_with("SPELL_PERIODIC_");
    match sub {
        "SWING_DAMAGE" | "SWING_DAMAGE_LANDED" => {
            ev.spell = Some(melee());
            let mut s = after_prefix;
            if let Some(len) = advanced_at(&f, s, DAMAGE_SUFFIX) {
                ev.advanced = Some(read_advanced(&f, s, len));
                s += len;
            }
            let landed = sub == "SWING_DAMAGE_LANDED";
            ev.kind = damage(&f, s, false, landed, !landed);
        }
        "ENVIRONMENTAL_DAMAGE" => {
            let mut s = after_prefix;
            if let Some(len) = advanced_at(&f, s, DAMAGE_SUFFIX + 1) {
                ev.advanced = Some(read_advanced(&f, s, len));
                s += len;
            }
            let env = f.get(s).copied().unwrap_or("").to_string();
            let mut kind = damage(&f, s + 1, false, false, false);
            if let Kind::Damage { environment, .. } = &mut kind {
                *environment = Some(env);
            }
            ev.kind = kind;
        }
        _ if sub.ends_with("_DAMAGE") || sub == "DAMAGE_SHIELD" || sub == "DAMAGE_SPLIT" => {
            let mut s = after_prefix;
            if let Some(len) = advanced_at(&f, s, DAMAGE_SUFFIX) {
                ev.advanced = Some(read_advanced(&f, s, len));
                s += len;
            }
            ev.kind = damage(&f, s, periodic, false, false);
        }
        _ if sub.ends_with("_HEAL") => {
            let mut s = after_prefix;
            if let Some(len) = advanced_at(&f, s, HEAL_SUFFIX) {
                ev.advanced = Some(read_advanced(&f, s, len));
                s += len;
            }
            let g = |i: usize| f.get(s + i).copied().unwrap_or("nil");
            ev.kind = Kind::Heal {
                amount: int(g(0)).max(0),
                overheal: int(g(2)).max(0),
                absorbed: int(g(3)).max(0),
                critical: truthy(g(4)),
                periodic,
            };
        }
        _ if sub.ends_with("_MISSED") => {
            if sub == "SWING_MISSED" {
                ev.spell = Some(melee());
            }
            let s = after_prefix;
            let amount = f.get(s + 2).copied().filter(|a| is_number(a)).map(int);
            ev.kind = Kind::Miss {
                miss_type: f.get(s).copied().unwrap_or("").to_string(),
                amount,
                periodic,
            };
        }
        "SPELL_ABSORBED" => {
            // The attacking spell's triple is there only when the hit was a spell.
            let (q, attack) = if f.get(9).is_some_and(|s| is_number(s)) {
                (12, spell_at(&f, 9))
            } else {
                (9, Some(melee()))
            };
            ev.spell = attack;
            if f.len() >= q + 8 {
                ev.kind = Kind::Absorb {
                    absorber: unit(&f, q),
                    shield: spell_at(&f, q + 4).unwrap_or_else(melee),
                    amount: int(f[q + 7]).max(0),
                };
            }
        }
        "SPELL_CAST_SUCCESS" => {
            if let Some(len) = advanced_at(&f, after_prefix, 0) {
                ev.advanced = Some(read_advanced(&f, after_prefix, len));
            }
            ev.kind = Kind::CastSuccess;
        }
        "SPELL_ENERGIZE" => {
            if let Some(len) = advanced_at(&f, after_prefix, 0) {
                ev.advanced = Some(read_advanced(&f, after_prefix, len));
            }
        }
        "UNIT_DIED" | "UNIT_DESTROYED" => ev.kind = Kind::Died,
        "PARTY_KILL" => ev.kind = Kind::PartyKill,
        "SPELL_SUMMON" => ev.kind = Kind::Summon,
        "SPELL_INTERRUPT" => ev.kind = Kind::Interrupt,
        "SPELL_DISPEL" | "SPELL_STOLEN" => ev.kind = Kind::Dispel,
        _ => {}
    }
    Ok(ev)
}

/// `Name-Realm` from the log's `Name-Realm-Normal-US` (the realm type and region the client
/// appends to player names); other names unchanged.
pub fn display_name(name: &str, guid: &str) -> String {
    if !guid.starts_with("Player-") {
        return name.to_string();
    }
    let parts: Vec<&str> = name.split('-').collect();
    let region = parts.last().copied().unwrap_or("");
    if parts.len() >= 4 && region.len() == 2 && region.bytes().all(|b| b.is_ascii_uppercase()) {
        parts[..parts.len() - 2].join("-")
    } else {
        name.to_string()
    }
}

/// The realm (server) id in a `Player-<realm>-<hex>` GUID.
pub fn player_realm(guid: &str) -> Option<i64> {
    let mut p = guid.split('-');
    if p.next()? != "Player" {
        return None;
    }
    p.next()?.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    // The layouts of the real log, with invented names and GUIDs.
    const SPELL_DAMAGE: &str = "11/6/2026 15:48:35.757-6  SPELL_DAMAGE,Player-6933-0F00A001,\"Aerin-Stormcall-Normal-US\",0x518,0x80000000,Creature-0-6832-2991-190-250926-0F00B001,\"Scrawny Ursera\",0xa48,0x80000000,8921,\"Moonfire\",0x40,Creature-0-6832-2991-190-250926-0F00B001,0000000000000000,64,71,6,0,46,0,0,0,1,0,0,0,4009.64,1907.49,2521,3.7077,3,7,7,-1,64,0,0,0,nil,nil,nil,ST";
    const SWING: &str = "11/6/2026 15:48:30.753-6  SWING_DAMAGE,Creature-0-6832-2991-190-250926-0F00B001,\"Scrawny Ursera\",0xa48,0x80000000,Player-6933-0F00A001,\"Aerin-Stormcall-Normal-US\",0x518,0x80000000,Creature-0-6832-2991-190-250926-0F00B001,0000000000000000,71,71,6,0,46,0,0,0,1,0,0,0,4004.50,1925.74,2521,5.4978,3,3,3,-1,1,0,0,0,nil,1,nil";
    const HEAL: &str = "11/6/2026 15:56:29.311-6  SPELL_HEAL,Player-6929-0F00A002,\"Thorbrand-Ironhew-Normal-US\",0x518,0x80000000,Player-6929-0F00A002,\"Thorbrand-Ironhew-Normal-US\",0x518,0x80000000,331,\"Healing Wave\",0x8,Player-6929-0F00A002,0000000000000000,57,57,26,0,50,0,0,0,0,53,75,0,3958.66,1663.16,2521,5.2642,0,34,34,30,0,1";
    const ENV: &str = "11/6/2026 15:49:17.053-6  ENVIRONMENTAL_DAMAGE,0000000000000000,nil,0x80000000,0x80000000,Player-6796-0F00A003,\"Kesra-Duskmantle-Normal-US\",0x518,0x80000000,Player-6796-0F00A003,0000000000000000,109,136,69,0,303,0,0,0,1,0,1000,0,4030.01,1754.35,2521,4.3911,1,Falling,27,27,0,1,0,0,0,nil,nil,nil";

    #[test]
    fn times_with_their_offset() {
        // 15:48:15.091 at UTC-6 is 21:48:15.091 UTC.
        assert_eq!(
            parse_time("11/6/2026 15:48:15.091-6"),
            Some(1_794_001_695_091)
        );
        assert_eq!(
            parse_time("11/6/2026 21:48:15.091+0"),
            parse_time("11/6/2026 15:48:15.091-6")
        );
        assert_eq!(
            parse_time("11/7/2026 03:18:15.091+5.5"),
            parse_time("11/6/2026 21:48:15.091")
        );
        assert_eq!(
            parse_time("11/7/2026 03:18:15.091+05:30"),
            parse_time("11/6/2026 21:48:15.091")
        );
        assert_eq!(parse_time("11/6 15:48:15.091"), None);
        assert_eq!(parse_time("garbage"), None);
    }

    #[test]
    fn quoted_fields_keep_their_commas() {
        assert_eq!(
            split_fields("A,\"b, c\",nil,\"\",0x10"),
            vec!["A", "b, c", "nil", "", "0x10"]
        );
        assert_eq!(split_fields("A,"), vec!["A", ""]);
        assert_eq!(split_fields("\"x\"\"y\",z"), vec!["x\"\"y", "z"]);
    }

    #[test]
    fn spell_damage_has_the_19_field_block_and_a_trailing_st() {
        assert_eq!(split_fields(split_time(SPELL_DAMAGE).unwrap().1).len(), 42);
        let e = parse_line(SPELL_DAMAGE).unwrap();
        assert_eq!(e.src.guid, "Player-6933-0F00A001");
        assert_eq!(e.src.flags, 0x518);
        assert_eq!(e.dst.name.as_deref(), Some("Scrawny Ursera"));
        assert_eq!(
            e.spell,
            Some(Spell {
                id: 8921,
                name: "Moonfire".into(),
                school: 64
            })
        );
        let adv = e.advanced.unwrap();
        assert_eq!(adv.ui_map_id, Some(2521));
        assert_eq!(adv.level, Some(3));
        assert_eq!(adv.owner_guid, None);
        match e.kind {
            Kind::Damage {
                amount,
                overkill,
                school,
                critical,
                ..
            } => {
                assert_eq!((amount, overkill, school, critical), (7, 0, 64, false));
            }
            k => panic!("{k:?}"),
        }
    }

    #[test]
    fn swing_heal_environment_layouts() {
        assert_eq!(split_fields(split_time(SWING).unwrap().1).len(), 38);
        match parse_line(SWING).unwrap().kind {
            Kind::Damage {
                amount,
                glancing,
                swing,
                landed,
                ..
            } => assert_eq!((amount, glancing, swing, landed), (3, true, true, false)),
            k => panic!("{k:?}"),
        }
        assert_eq!(split_fields(split_time(HEAL).unwrap().1).len(), 36);
        match parse_line(HEAL).unwrap().kind {
            Kind::Heal {
                amount,
                overheal,
                critical,
                ..
            } => assert_eq!((amount, overheal, critical), (34, 30, true)),
            k => panic!("{k:?}"),
        }
        assert_eq!(split_fields(split_time(ENV).unwrap().1).len(), 39);
        let e = parse_line(ENV).unwrap();
        assert!(e.src.is_none());
        match e.kind {
            Kind::Damage {
                amount,
                environment,
                ..
            } => assert_eq!((amount, environment.as_deref()), (27, Some("Falling"))),
            k => panic!("{k:?}"),
        }
    }

    #[test]
    fn misses_deaths_zones_and_the_unknown() {
        let miss = "11/6/2026 15:49:12.819-6  SWING_MISSED,Creature-0-1-2-3-4-0F00B002,\"Juvenile Vuldren\",0xa28,0x80000000,Player-6796-0F00A004,\"Brannoc-Ashvale-Normal-US\",0x518,0x80000000,BLOCK,nil,1";
        match parse_line(miss).unwrap().kind {
            Kind::Miss {
                miss_type, amount, ..
            } => assert_eq!((miss_type.as_str(), amount), ("BLOCK", Some(1))),
            k => panic!("{k:?}"),
        }
        let spell_miss = "11/6/2026 15:52:55.427-6  SPELL_MISSED,Player-6931-0F00A005,\"Moraine-Thistlewood-Normal-US\",0x518,0x80000000,Creature-0-1-2-3-4-0F00B003,\"Pesky Cirrusfly\",0xa28,0x80000000,5176,\"Wrath\",0x8,MISS,nil,ST";
        match parse_line(spell_miss).unwrap().kind {
            Kind::Miss {
                miss_type, amount, ..
            } => assert_eq!((miss_type.as_str(), amount), ("MISS", None)),
            k => panic!("{k:?}"),
        }
        let died = "11/6/2026 15:48:45.056-6  UNIT_DIED,0000000000000000,nil,0x80000000,0x80000000,Creature-0-1-2-3-4-0F00B001,\"Scrawny Ursera\",0xa48,0x80000000,0";
        assert_eq!(parse_line(died).unwrap().kind, Kind::Died);
        let zone = "11/6/2026 15:48:15.091-6  ZONE_CHANGE,2991,\"Zephras Isle\",0";
        assert_eq!(
            parse_line(zone).unwrap().kind,
            Kind::ZoneChange {
                instance_id: 2991,
                name: "Zephras Isle".into(),
                difficulty: 0
            }
        );
        let map = "11/6/2026 15:48:15.091-6  MAP_CHANGE,2521,\"Zephras Isle\",4956.250000,1247.916992,4231.250000,-1331.250000";
        assert_eq!(
            parse_line(map).unwrap().kind,
            Kind::MapChange {
                ui_map_id: 2521,
                name: "Zephras Isle".into()
            }
        );
        let aura = "11/6/2026 15:48:15.903-6  SPELL_AURA_APPLIED,Player-6929-0F00A006,\"Sorvyn-Galecrest-Normal-US\",0x518,0x80000000,Player-6929-0F00A006,\"Sorvyn-Galecrest-Normal-US\",0x518,0x80000000,1259416,\"Walk on Air\",0x8,BUFF";
        assert_eq!(parse_line(aura).unwrap().kind, Kind::Other);
        assert_eq!(
            parse_line("11/6/2026 15:48:15.903-6  SOMETHING_NEW,1,2")
                .unwrap()
                .kind,
            Kind::Other
        );
        assert!(parse_line("half a line").is_err());
        let header = "11/6/2026 16:11:38.594-6  COMBAT_LOG_VERSION,22,ADVANCED_LOG_ENABLED,1,BUILD_VERSION,1.60.1,PROJECT_ID,18";
        assert_eq!(
            parse_line(header).unwrap().kind,
            Kind::Header {
                version: 22,
                advanced: true
            }
        );
        let start = "11/6/2026 20:00:00.000-6  ENCOUNTER_START,1084,\"Onyxia\",9,40,249";
        assert!(matches!(
            parse_line(start).unwrap().kind,
            Kind::EncounterStart { encounter_id: 1084, ref name, instance_id: Some(249), .. } if name == "Onyxia"
        ));
        let end = "11/6/2026 20:03:00.000-6  ENCOUNTER_END,1084,\"Onyxia\",9,40,1";
        assert!(matches!(
            parse_line(end).unwrap().kind,
            Kind::EncounterEnd { success: true, .. }
        ));
    }

    #[test]
    fn absorbs_name_the_absorber() {
        let melee = "11/6/2026 20:00:01.000-6  SPELL_ABSORBED,Creature-0-1-2-3-4-0F00B009,\"Ogre\",0xa48,0x80000000,Player-1-0F00A001,\"Aerin-Stormcall\",0x511,0x80000000,Player-1-0F00A002,\"Thorbrand-Ironhew\",0x512,0x80000000,17,\"Power Word: Shield\",0x2,40,40,nil";
        let e = parse_line(melee).unwrap();
        assert_eq!(e.spell.as_ref().unwrap().id, 1);
        match e.kind {
            Kind::Absorb {
                absorber,
                shield,
                amount,
            } => {
                assert_eq!(absorber.guid, "Player-1-0F00A002");
                assert_eq!(shield.id, 17);
                assert_eq!(amount, 40);
            }
            k => panic!("{k:?}"),
        }
    }

    #[test]
    fn names_lose_the_realm_type_and_region() {
        assert_eq!(
            display_name("Aerin-Stormcall-Normal-US", "Player-1-0F00A001"),
            "Aerin-Stormcall"
        );
        assert_eq!(
            display_name("Aerin-Stormcall", "Player-1-0F00A001"),
            "Aerin-Stormcall"
        );
        assert_eq!(
            display_name("Scrawny-Ursera-Big-US", "Creature-0"),
            "Scrawny-Ursera-Big-US"
        );
        assert_eq!(player_realm("Player-6933-0F00A001"), Some(6933));
        assert_eq!(player_realm("Creature-0-1"), None);
    }
}
