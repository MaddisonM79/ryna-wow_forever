//! The digest (docs/wow.md section 9.3): NDJSON, one event per line with its time in ms from the
//! fight's start, after one first line naming the actors (and, beside the contract's `actors`,
//! the `spells` the events name, so the names survive a recompute from the digest alone).
//!
//! ```text
//! {"actors":{"Player-…":{"name":"…","kind":"player","class":null,"owner":null}},"spells":{"8921":{"name":"Moonfire","school":64}}}
//! {"t":0,"e":"D","s":"Player-…","d":"Creature-…","sp":8921,"a":64,"o":0,"sc":64,"f":0}
//! ```

use std::collections::BTreeMap;
use std::io::{Read, Write};

use serde::{Deserialize, Serialize};

/// `f` bits.
pub mod bits {
    pub const CRIT: u32 = 1;
    pub const GLANCING: u32 = 2;
    pub const CRUSHING: u32 = 4;
    pub const OFF_HAND: u32 = 8;
    pub const PERIODIC: u32 = 16;
    pub const SWING_LANDED: u32 = 32;
}

/// One event: `e` is `D` damage, `H` heal, `M` miss, `X` death, `A` absorb.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DigestEvent {
    pub t: i64,
    pub e: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub s: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub d: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sp: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub a: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub o: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sc: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub f: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub m: Option<String>,
    /// A `SWING_DAMAGE` whose `_LANDED` twin may follow (dropped when it does). Never written.
    #[serde(skip)]
    pub twin: bool,
}

impl DigestEvent {
    pub fn flags(&self) -> u32 {
        self.f.unwrap_or(0)
    }
    pub fn amount(&self) -> i64 {
        self.a.unwrap_or(0)
    }
    pub fn over(&self) -> i64 {
        self.o.unwrap_or(0)
    }
    /// What it did: damage less overkill, healing less overheal, an absorb's amount.
    pub fn effective(&self) -> i64 {
        (self.amount() - self.over()).max(0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActorMeta {
    pub name: Option<String>,
    /// `player`, `pet` or `npc`.
    pub kind: String,
    pub class: Option<String>,
    pub owner: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpellMeta {
    pub name: String,
    pub school: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Header {
    pub actors: BTreeMap<String, ActorMeta>,
    #[serde(default)]
    pub spells: BTreeMap<i64, SpellMeta>,
}

/// What a digest holds, read back.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Digest {
    pub header: Header,
    pub events: Vec<DigestEvent>,
}

impl Digest {
    pub fn to_ndjson(&self) -> String {
        let mut out = serde_json::to_string(&self.header).expect("a header serialises");
        out.push('\n');
        for e in &self.events {
            out.push_str(&serde_json::to_string(e).expect("an event serialises"));
            out.push('\n');
        }
        out
    }

    pub fn from_ndjson(text: &str) -> Result<Digest, serde_json::Error> {
        let mut lines = text.lines().filter(|l| !l.trim().is_empty());
        let header = match lines.next() {
            Some(l) => serde_json::from_str(l)?,
            None => Header::default(),
        };
        let events = lines
            .map(serde_json::from_str)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Digest { header, events })
    }
}

pub fn gzip(bytes: &[u8]) -> Vec<u8> {
    let mut enc = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    enc.write_all(bytes).expect("writing to memory");
    enc.finish().expect("writing to memory")
}

pub fn gunzip(bytes: &[u8]) -> std::io::Result<Vec<u8>> {
    let mut out = Vec::new();
    flate2::read::GzDecoder::new(bytes).read_to_end(&mut out)?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_in_the_contract_s_order_and_absent_when_empty() {
        let d = DigestEvent {
            t: 0,
            e: "D".into(),
            s: Some("Player-1-0F00A001".into()),
            d: Some("Creature-0-1".into()),
            sp: Some(8921),
            a: Some(64),
            o: Some(0),
            sc: Some(64),
            f: Some(0),
            m: None,
            twin: true,
        };
        assert_eq!(
            serde_json::to_string(&d).unwrap(),
            r#"{"t":0,"e":"D","s":"Player-1-0F00A001","d":"Creature-0-1","sp":8921,"a":64,"o":0,"sc":64,"f":0}"#
        );
        let x = DigestEvent {
            t: 134_000,
            e: "X".into(),
            s: None,
            d: Some("Player-1-0F00A001".into()),
            sp: None,
            a: None,
            o: None,
            sc: None,
            f: None,
            m: None,
            twin: false,
        };
        assert_eq!(
            serde_json::to_string(&x).unwrap(),
            r#"{"t":134000,"e":"X","d":"Player-1-0F00A001"}"#
        );
        let digest = Digest {
            header: Header::default(),
            events: vec![d.clone(), x],
        };
        let back = Digest::from_ndjson(
            &String::from_utf8(gunzip(&gzip(digest.to_ndjson().as_bytes())).unwrap()).unwrap(),
        )
        .unwrap();
        // `twin` is never written.
        assert!(!back.events[0].twin);
        assert_eq!(back.events[0].a, d.a);
        assert_eq!(back.events.len(), 2);
    }
}
