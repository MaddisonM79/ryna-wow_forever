//! The census without an account (docs/wow.md section 13.3, stage (a)): the `seen` events of an
//! account's journal, as uploads for `POST /v1/wow/census`, signed by the device key and sent with
//! no token.
//!
//! Each event is keyed by the first 32 hex of SHA-256 over `<observing char GUID>|<session>|<seq>`,
//! so the observing character itself never leaves and a repeated upload adds nothing (the API
//! keeps one row per submission ID and key). What leaves for each is the `seen` event's own fields,
//! when it was seen, and the observing character's realm and faction. The account's own
//! characters are left out. In stage (a) only `ryna census push` sends these, by hand; the app
//! keeps pushing snapshots as before.

use serde::Serialize;
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

use crate::snapshot::{EventType, Snapshot};

pub const CENSUS_SCHEMA: i64 = 1;
/// The API takes at most this many events in one upload.
pub const MAX_EVENTS: usize = 2000;
/// And a body of at most this many bytes.
pub const CENSUS_LIMIT: usize = 1024 * 1024;

/// One `POST /census` body.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CensusUpload {
    pub schema: i64,
    pub flavor: String,
    /// `ryna-agent/<version>`.
    pub agent: String,
    pub addon: String,
    /// Unix seconds; the API wants it within 15 minutes of its own clock.
    pub sent_at: i64,
    pub events: Vec<CensusEvent>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CensusEvent {
    /// The dedupe key ([`event_key`]).
    pub key: String,
    pub at: i64,
    pub observer: Observer,
    /// The journal `seen` event's own fields, as the addon wrote them.
    pub seen: Map<String, Value>,
}

/// The observing character's realm and faction (null where the snapshot doesn't say).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Observer {
    pub realm_id: Option<i64>,
    pub faction: Option<String>,
}

impl CensusUpload {
    /// The exact bytes that are sent (and signed).
    pub fn body(&self) -> Vec<u8> {
        serde_json::to_vec(self).expect("an upload always serialises")
    }
}

/// The first 32 lowercase hex of SHA-256 over `<observing char GUID>|<session>|<seq>`.
pub fn event_key(char_guid: &str, session: &str, seq: i64) -> String {
    Sha256::digest(format!("{char_guid}|{session}|{seq}").as_bytes())[..16]
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn text<'a>(m: &'a Map<String, Value>, k: &str) -> Option<&'a str> {
    m.get(k)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
}

/// The server id in a `Player-<server>-<hex>` GUID.
fn guid_server_id(guid: &str) -> Option<i64> {
    let mut parts = guid.split('-');
    parts.next()?;
    parts.next()?.parse().ok()
}

/// Every `seen` event in the snapshot's journal that isn't about one of the account's own
/// characters (by GUID, or by full name where the event has no GUID), in journal order.
pub fn census_events(snapshot: &Snapshot) -> Vec<CensusEvent> {
    let own_guids: Vec<&str> = snapshot
        .characters
        .iter()
        .map(|c| c.guid.as_str())
        .collect();
    let own_names: Vec<String> = snapshot
        .characters
        .iter()
        .filter_map(|c| {
            c.full_name
                .clone()
                .or_else(|| match (&c.first_name, &c.last_name) {
                    (Some(f), Some(l)) => Some(format!("{f}-{l}")),
                    _ => None,
                })
        })
        .map(|n| n.to_lowercase())
        .collect();
    snapshot
        .journal
        .iter()
        .filter(|e| e.known_type() == Some(EventType::Seen))
        .filter(|e| {
            let by_guid = text(&e.data, "guid").is_some_and(|g| own_guids.contains(&g));
            let by_name =
                text(&e.data, "fullName").is_some_and(|n| own_names.contains(&n.to_lowercase()));
            !by_guid && !by_name
        })
        .map(|e| {
            let observer = snapshot.characters.iter().find(|c| c.guid == e.char);
            CensusEvent {
                key: event_key(&e.char, &e.session, e.seq),
                at: e.at,
                observer: Observer {
                    realm_id: observer
                        .and_then(|c| c.realm_id)
                        .or_else(|| guid_server_id(&e.char)),
                    faction: observer.and_then(|c| c.faction.clone()),
                },
                seen: e.data.clone(),
            }
        })
        .collect()
}

/// The snapshot's census as uploads: each at most [`MAX_EVENTS`] events and [`CENSUS_LIMIT`]
/// bytes, `sentAt` = `now`. None when there's nothing to send.
pub fn uploads(snapshot: &Snapshot, now: i64) -> Vec<CensusUpload> {
    split(
        census_events(snapshot),
        &snapshot.flavor,
        &snapshot.addon,
        now,
    )
}

fn split(events: Vec<CensusEvent>, flavor: &str, addon: &str, now: i64) -> Vec<CensusUpload> {
    let empty = || CensusUpload {
        schema: CENSUS_SCHEMA,
        flavor: flavor.to_string(),
        agent: crate::user_agent(),
        addon: addon.to_string(),
        sent_at: now,
        events: Vec::new(),
    };
    let base = empty().body().len();
    let mut out = Vec::new();
    let mut current = empty();
    let mut size = base;
    for e in events {
        // Its bytes in the array, with the comma before it.
        let len = serde_json::to_vec(&e).map_or(0, |b| b.len()) + 1;
        if !current.events.is_empty()
            && (current.events.len() >= MAX_EVENTS || size + len > CENSUS_LIMIT)
        {
            out.push(std::mem::replace(&mut current, empty()));
            size = base;
        }
        size += len;
        current.events.push(e);
    }
    if !current.events.is_empty() {
        out.push(current);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::snapshot::{build, Source};
    use serde_json::json;

    const FIXTURE: &str = include_str!("../../fixtures/Ryna.lua");
    const NOW: i64 = 1_762_500_000;

    fn fixture() -> Snapshot {
        let parsed = crate::lua::parse(FIXTURE).unwrap();
        build(
            &parsed,
            &Source {
                account_folder: "FAKEACCOUNT1",
                label: None,
                saved_at: NOW - 100,
                install_flavor: None,
            },
        )
        .unwrap()
        .snapshot
    }

    #[test]
    fn the_dedupe_key_is_over_char_session_and_seq() {
        assert_eq!(
            event_key("Player-91-0F00A001", "1762400000-0F00A001", 9),
            "6950973b89f8b6965a39bb22f5aa6e98"
        );
        assert_ne!(
            event_key("Player-91-0F00A001", "1762400000-0F00A001", 10),
            event_key("Player-91-0F00A001", "1762400000-0F00A001", 9)
        );
    }

    #[test]
    fn every_seen_event_with_its_observer_and_never_the_observer_itself() {
        let s = fixture();
        let seen = s.journal.iter().filter(|e| e.kind == "seen").count();
        let events = census_events(&s);
        assert!(!events.is_empty());
        assert!(events.len() <= seen);
        let first = &events[0];
        assert_eq!(
            first.key,
            event_key("Player-91-0F00A001", "1762400000-0F00A001", 9)
        );
        assert_eq!(first.at, 1762404300);
        assert_eq!(first.seen["fullName"], json!("Kesra-Duskmantle"));
        assert_eq!(first.seen["source"], json!("group"));
        assert_eq!(first.observer.realm_id, Some(91));
        let aerin = s
            .characters
            .iter()
            .find(|c| c.guid == "Player-91-0F00A001")
            .unwrap();
        assert_eq!(first.observer.faction, aerin.faction);
        let body = String::from_utf8(uploads(&s, NOW)[0].body()).unwrap();
        // The observing character, its session and the journal's own fields never leave.
        for gone in ["0F00A001", "\"session\"", "\"seq\"", "\"char\"", "\"type\""] {
            assert!(!body.contains(gone), "{gone} in {body}");
        }
    }

    #[test]
    fn the_accounts_own_characters_are_left_out() {
        let mut s = fixture();
        let n = census_events(&s).len();
        let mut own = s.characters[0].clone();
        own.guid = "Player-91-0F00B001".into(); // Kesra, seen twice in the fixture
        own.full_name = None;
        s.characters.push(own);
        let without = census_events(&s);
        assert_eq!(without.len(), n - 2);
        assert!(without
            .iter()
            .all(|e| e.seen.get("guid") != Some(&json!("Player-91-0F00B001"))));
        // A sighting with no GUID is matched by name.
        let mut tavi = s.characters[0].clone();
        tavi.guid = "Player-91-0F00C999".into();
        tavi.full_name = Some("tavi-emberleaf".into());
        s.characters.push(tavi);
        assert!(census_events(&s)
            .iter()
            .all(|e| e.seen.get("fullName") != Some(&json!("Tavi-Emberleaf"))));
    }

    #[test]
    fn the_upload_is_schema_1_with_the_snapshots_flavor_and_addon() {
        let s = fixture();
        let u = &uploads(&s, NOW)[0];
        let v: Value = serde_json::from_slice(&u.body()).unwrap();
        assert_eq!(v["schema"], json!(1));
        assert_eq!(v["flavor"], json!(s.flavor));
        assert_eq!(v["addon"], json!(s.addon));
        assert_eq!(v["agent"], json!(crate::user_agent()));
        assert_eq!(v["sentAt"], json!(NOW));
        let e = &v["events"][0];
        assert_eq!(
            e.as_object().unwrap().keys().collect::<Vec<_>>(),
            ["at", "key", "observer", "seen"]
        );
        assert!(e["observer"].get("realmId").is_some() && e["observer"].get("faction").is_some());
    }

    fn many(n: usize, pad: usize) -> Vec<CensusEvent> {
        (0..n)
            .map(|i| CensusEvent {
                key: event_key("Player-1-1", "s", i as i64),
                at: i as i64,
                observer: Observer {
                    realm_id: Some(1),
                    faction: None,
                },
                seen: json!({ "fullName": "x".repeat(pad), "source": "who" })
                    .as_object()
                    .unwrap()
                    .clone(),
            })
            .collect()
    }

    #[test]
    fn more_than_2000_events_go_up_in_several_uploads() {
        let u = split(many(4500, 1), "wowf", "0.3.2", NOW);
        assert_eq!(
            u.iter().map(|u| u.events.len()).collect::<Vec<_>>(),
            [2000, 2000, 500]
        );
        assert_eq!(u[1].events[0].at, 2000);
        assert!(split(Vec::new(), "wowf", "0.3.2", NOW).is_empty());
        assert_eq!(split(many(2000, 1), "wowf", "0.3.2", NOW).len(), 1);
    }

    #[test]
    fn no_upload_is_over_the_byte_limit() {
        // ~2 KB each: about 500 to a megabyte.
        let u = split(many(1200, 2000), "wowf", "0.3.2", NOW);
        assert!(u.len() >= 3, "{}", u.len());
        assert!(u.iter().all(|u| u.body().len() <= CENSUS_LIMIT));
        assert_eq!(u.iter().map(|u| u.events.len()).sum::<usize>(), 1200);
    }
}
