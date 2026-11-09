//! The snapshot the agent posts (docs/wow.md section 2), built from the addon's `RynaDB` table
//! (section 1).
//!
//! The addon writes section 2's fields one for one, so building a snapshot is mostly reshaping:
//! `characters` and `guilds` are keyed by GUID in Lua and are arrays on the wire, and the agent adds
//! `agent`, `account` and `savedAt`. Every struct keeps the fields it doesn't know in `extra`, so an
//! addon ahead of this agent loses nothing on the way to the API; journal events are kept whole.
//!
//! Lua can't tell an empty list from an empty map (both are `{}`), and a list the addon builds by
//! index can come out sparse, so every list field here accepts an array, an object keyed by index,
//! or nothing, and every table field accepts an empty list as "empty".

use std::collections::BTreeMap;
use std::path::Path;

use serde::de::{DeserializeOwned, Error as _};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::lua;
use crate::AGENT_VERSION;

/// The major of section 2 this agent reads and writes.
pub const SCHEMA_MAJOR: i64 = 1;

/// The SavedVariable the addon declares.
pub const SAVED_VARIABLE: &str = "RynaDB";

#[derive(Debug, Error)]
pub enum SnapshotError {
    #[error("no {SAVED_VARIABLE} in this file: is it the Ryna addon's SavedVariables (Ryna.lua)?")]
    NoRynaDb,
    #[error("{SAVED_VARIABLE} is not a table")]
    NotATable,
    #[error("{SAVED_VARIABLE} has no schema number")]
    NoSchema,
    #[error(
        "{SAVED_VARIABLE} is schema {found}, and this agent reads schema {SCHEMA_MAJOR}: update the agent and the addon to the same release"
    )]
    SchemaUnsupported { found: String },
    #[error("{SAVED_VARIABLE} has no flavor, and the install's .flavor.info didn't say either")]
    NoFlavor,
    #[error("{SAVED_VARIABLE}.{field} is not a table")]
    NotATableField { field: &'static str },
    #[error("character {key}: {source}")]
    Character {
        key: String,
        source: serde_json::Error,
    },
    #[error("guild {key}: {source}")]
    Guild {
        key: String,
        source: serde_json::Error,
    },
}

/// Reading a file into a snapshot: the file, the Lua, or the table's shape.
#[derive(Debug, Error)]
pub enum ReadError {
    #[error("{path}: {source}")]
    Io {
        path: String,
        source: std::io::Error,
    },
    #[error("{path}: {source}")]
    Lua {
        path: String,
        source: lua::ParseError,
    },
    #[error("{path}: {source}")]
    Snapshot { path: String, source: SnapshotError },
}

impl ReadError {
    /// True when only a newer agent (or an older addon) can read this file.
    pub fn is_schema_unsupported(&self) -> bool {
        matches!(
            self,
            ReadError::Snapshot {
                source: SnapshotError::SchemaUnsupported { .. },
                ..
            }
        )
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub schema: i64,
    pub addon: String,
    pub agent: String,
    pub flavor: String,
    pub account: Account,
    pub saved_at: i64,
    pub characters: Vec<Character>,
    pub guilds: Vec<Guild>,
    pub journal: Vec<JournalEvent>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Account {
    /// The first 16 hex digits of the sha256 of the WTF account folder's name.
    pub key: String,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Character {
    #[serde(default)]
    pub guid: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub full_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub realm: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub realm_id: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub class: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub race: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub faction: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gender: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub level: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub xp: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub xp_max: Option<i64>,
    /// Copper.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gold: Option<i64>,
    /// Seconds; null until TIME_PLAYED_MSG arrived.
    #[serde(default)]
    pub played_total: Option<i64>,
    #[serde(default)]
    pub played_level: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub zone: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subzone: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_seen: Option<i64>,
    #[serde(default, deserialize_with = "opt_table")]
    pub guild: Option<CharacterGuild>,
    #[serde(default, deserialize_with = "list")]
    pub gear: Vec<GearItem>,
    #[serde(default)]
    pub talents: Option<String>,
    #[serde(default, deserialize_with = "list")]
    pub professions: Vec<Profession>,
    #[serde(default, deserialize_with = "list")]
    pub reputations: Vec<Reputation>,
    #[serde(default, deserialize_with = "table")]
    pub quests: Quests,
    #[serde(default, deserialize_with = "list")]
    pub containers: Vec<ContainerItem>,
    #[serde(default)]
    pub bank_as_of: Option<i64>,
    /// Creatures killed by the character or its group, by npc id (section 12.1).
    #[serde(
        default,
        deserialize_with = "map",
        skip_serializing_if = "BTreeMap::is_empty"
    )]
    pub kills: BTreeMap<String, KillStat>,
    /// Items looted, by item id.
    #[serde(
        default,
        deserialize_with = "map",
        skip_serializing_if = "BTreeMap::is_empty"
    )]
    pub loot: BTreeMap<String, LootStat>,
    /// Game objects opened (chests, veins, herbs, pools), by object id.
    #[serde(
        default,
        deserialize_with = "map",
        skip_serializing_if = "BTreeMap::is_empty"
    )]
    pub objects: BTreeMap<String, ObjectStat>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// `kills[npcId]`: kills the character took part in, its killing blows, the xp they gave and the
/// corpses it opened.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KillStat {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default)]
    pub n: i64,
    #[serde(default)]
    pub own: i64,
    #[serde(default)]
    pub xp: i64,
    #[serde(default)]
    pub looted: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub zone: Option<String>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// `loot[itemId]`: how often it dropped, how many pieces, and from what (`c:<npcId>`,
/// `o:<objectId>`, `i:<itemId>`, `f`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LootStat {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quality: Option<i64>,
    #[serde(default)]
    pub n: i64,
    #[serde(default)]
    pub qty: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last: Option<i64>,
    #[serde(default, deserialize_with = "map")]
    pub from: BTreeMap<String, i64>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// `objects[objectId]`: times opened, and where last.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectStat {
    #[serde(default)]
    pub n: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub zone: Option<String>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CharacterGuild {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rank_index: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rank_name: Option<String>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GearItem {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slot: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub item_id: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quality: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enchant: Option<i64>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Profession {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rank: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<i64>,
    #[serde(default, deserialize_with = "id_list")]
    pub recipes: Vec<i64>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Reputation {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub faction_id: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub standing: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<i64>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Quests {
    #[serde(default, deserialize_with = "list")]
    pub active: Vec<ActiveQuest>,
    /// Quest ids. The addon may store them as a list or as a set (`[id] = true`).
    #[serde(default, deserialize_with = "id_list")]
    pub completed: Vec<i64>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActiveQuest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub level: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accepted_at: Option<i64>,
    #[serde(default, deserialize_with = "list")]
    pub objectives: Vec<Objective>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Objective {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub done: Option<bool>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContainerItem {
    /// `bag`, `bank` or `keyring`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub container: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bag: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slot: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub item_id: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub count: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quality: Option<i64>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Guild {
    /// The guild's GUID, or `name-realmId` where the client gave none (the addon's key).
    #[serde(default)]
    pub guid: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub realm_id: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub roster_as_of: Option<i64>,
    #[serde(default, deserialize_with = "list")]
    pub roster: Vec<RosterEntry>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RosterEntry {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub full_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub level: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub class: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rank_index: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rank_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub zone: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    #[serde(default)]
    pub officer_note: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub online: Option<bool>,
    #[serde(default)]
    pub last_online_at: Option<i64>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// One journal event. The identifying fields are typed; everything else (`kind`, `zone`,
/// `questId`, `members`, ...) is kept exactly as the addon wrote it, for every type, known or not.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JournalEvent {
    pub session: String,
    pub seq: i64,
    pub at: i64,
    pub char: String,
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(flatten)]
    pub data: Map<String, Value>,
}

/// The journal event types section 2 names. Others pass through untouched.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventType {
    Session,
    Zone,
    QuestAccept,
    QuestDone,
    Level,
    Death,
    Gold,
    Group,
    Seen,
    Who,
}

impl JournalEvent {
    pub fn known_type(&self) -> Option<EventType> {
        Some(match self.kind.as_str() {
            "session" => EventType::Session,
            "zone" => EventType::Zone,
            "questAccept" => EventType::QuestAccept,
            "questDone" => EventType::QuestDone,
            "level" => EventType::Level,
            "death" => EventType::Death,
            "gold" => EventType::Gold,
            "group" => EventType::Group,
            "seen" => EventType::Seen,
            "who" => EventType::Who,
            _ => return None,
        })
    }
}

/// What the snapshot is built for: the account it came from and when its file was written.
#[derive(Debug, Clone)]
pub struct Source<'a> {
    /// The folder under `WTF/Account` the file sits in.
    pub account_folder: &'a str,
    /// What the person calls the account; default the last four characters of the folder name.
    pub label: Option<&'a str>,
    /// The file's mtime, unix seconds.
    pub saved_at: i64,
    /// The install's `.flavor.info`, for an addon that didn't write `flavor`.
    pub install_flavor: Option<&'a str>,
}

/// A snapshot and what was dropped on the way (malformed journal events), with what the agent keeps
/// for itself about the file and never sends: the addon's account id and how far its journal goes.
#[derive(Debug, Clone)]
pub struct Built {
    pub snapshot: Snapshot,
    pub warnings: Vec<String>,
    /// `RynaDB.meta.accountId`: 16 hex digits the core mints once per RynaDB. It means nothing
    /// beyond matching this file to its entry in `AgentStatus.lua` (the core drops bulky journal
    /// events once that entry's `journal_through` reaches them). None from a core without it.
    pub account_id: Option<String>,
    /// The newest journal event's `at` in this file: what a push of it has carried.
    pub journal_through: Option<i64>,
    /// The item ids this account knows, most useful first (`item_ids`): what the agent asks the
    /// pooled prices for (`prices_data`). Not sent anywhere.
    pub item_ids: Vec<i64>,
}

/// The item ids an account's `RynaDB` knows, in the order the data layer wants them when it has
/// to cut: what the characters hold (`containers`: bags, bank, keyring), then what they have
/// looted (Chronicle's `loot` keys), then what the Prices mod's own auction store has seen
/// (`prices.auction` keys). Each id once, at its first place; anything that isn't an item id the
/// API takes (1 to 10,000,000; one odd key would otherwise refuse a whole page) is skipped.
pub fn item_ids(db: &Map<String, Value>) -> Vec<i64> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    let mut push = |id: i64| {
        if (1..=10_000_000).contains(&id) && seen.insert(id) {
            out.push(id);
        }
    };
    let key_id = |k: &str| k.parse::<i64>().ok();
    let characters: Vec<Value> = match db.get("characters") {
        Some(Value::Object(m)) => m.values().cloned().collect(),
        Some(Value::Array(a)) => a.clone(),
        _ => Vec::new(),
    };
    for c in &characters {
        for item in values_in_order(c.get("containers").cloned().unwrap_or(Value::Null)) {
            if let Some(id) = item.get("itemId").and_then(Value::as_i64) {
                push(id);
            }
        }
    }
    for c in &characters {
        if let Some(Value::Object(loot)) = c.get("loot") {
            for k in loot.keys() {
                if let Some(id) = key_id(k) {
                    push(id);
                }
            }
        }
    }
    if let Some(Value::Object(auction)) = db.get("prices").and_then(|p| p.get("auction")) {
        for k in auction.keys() {
            if let Some(id) = key_id(k) {
                push(id);
            }
        }
    }
    out
}

/// `RynaDB.meta.accountId`, when it is the 16 hex digits the core writes.
fn meta_account_id(db: &Map<String, Value>) -> Option<String> {
    match db.get("meta").and_then(|m| m.get("accountId")) {
        Some(Value::String(id)) if id.len() == 16 && id.chars().all(|c| c.is_ascii_hexdigit()) => {
            Some(id.to_ascii_lowercase())
        }
        _ => None,
    }
}

/// `account.key`: the first 16 hex digits of sha256(folder name). The API never sees the name.
pub fn account_key(folder: &str) -> String {
    let digest = Sha256::digest(folder.as_bytes());
    digest[..8].iter().map(|b| format!("{b:02x}")).collect()
}

/// The default label: the last four characters of the folder name.
pub fn default_label(folder: &str) -> String {
    let chars: Vec<char> = folder.chars().collect();
    chars[chars.len().saturating_sub(4)..].iter().collect()
}

/// The server id in a `Player-<server>-<hex>` (or `Guild-...`) GUID.
fn guid_server_id(guid: &str) -> Option<i64> {
    let mut parts = guid.split('-');
    parts.next()?;
    parts.next()?.parse().ok()
}

/// What the client said about itself (`RynaDB.client`: GetBuildInfo's version, build, date and
/// interface, and the raw UnitName / UnitFullName / realm values), when the addon recorded it.
/// Not part of the snapshot; `status` and `parse` show it, since it is how the real Interface
/// number and Forever's name split are learned. Other top-level keys (`settings`) are ignored.
pub fn client_info(parsed: &Value) -> Option<&Map<String, Value>> {
    parsed.get(SAVED_VARIABLE)?.get("client")?.as_object()
}

/// `client` in one line: `interface 11600, version 1.60.0 (build 12345), UnitFullName "A-B" ...`.
pub fn client_summary(client: &Map<String, Value>) -> String {
    let show = |v: &Value| match v {
        Value::String(s) => format!("{s:?}"),
        other => other.to_string(),
    };
    let mut parts = Vec::new();
    for key in ["interface", "version", "build", "date"] {
        if let Some(v) = client.get(key) {
            parts.push(format!("{key} {}", show(v)));
        }
    }
    for key in [
        "unitName",
        "unitFullName",
        "unitFullNameRealm",
        "realm",
        "realmName",
    ] {
        if let Some(v) = client.get(key) {
            parts.push(format!("{key} {}", show(v)));
        }
    }
    for (k, v) in client {
        if ![
            "interface",
            "version",
            "build",
            "date",
            "unitName",
            "unitFullName",
            "unitFullNameRealm",
            "realm",
            "realmName",
        ]
        .contains(&k.as_str())
        {
            parts.push(format!("{k} {}", show(v)));
        }
    }
    parts.join(", ")
}

/// Check the schema major before anything else is read.
pub fn check_schema(db: &Map<String, Value>) -> Result<(), SnapshotError> {
    match db.get("schema") {
        None => Err(SnapshotError::NoSchema),
        Some(Value::Number(n)) if n.as_i64() == Some(SCHEMA_MAJOR) => Ok(()),
        Some(other) => Err(SnapshotError::SchemaUnsupported {
            found: other.to_string(),
        }),
    }
}

/// Build the snapshot from a parsed SavedVariables file (`lua::parse`'s output).
pub fn build(parsed: &Value, source: &Source<'_>) -> Result<Built, SnapshotError> {
    let db = match parsed.get(SAVED_VARIABLE) {
        None => return Err(SnapshotError::NoRynaDb),
        Some(Value::Object(m)) => m,
        // `RynaDB = {}`: an addon that never got as far as writing its schema.
        Some(Value::Array(a)) if a.is_empty() => return Err(SnapshotError::NoSchema),
        Some(_) => return Err(SnapshotError::NotATable),
    };
    check_schema(db)?;
    let mut warnings = Vec::new();

    let addon = match db.get("addon") {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Number(n)) => n.to_string(),
        _ => {
            warnings.push("RynaDB has no addon version; sending \"unknown\"".to_string());
            "unknown".to_string()
        }
    };
    let flavor = match db.get("flavor") {
        Some(Value::String(s)) if !s.is_empty() => s.clone(),
        _ => source
            .install_flavor
            .map(str::to_string)
            .ok_or(SnapshotError::NoFlavor)?,
    };

    let mut characters = Vec::new();
    for (key, value) in keyed_entries(db.get("characters"), "characters")? {
        let mut c: Character =
            serde_json::from_value(value).map_err(|source| SnapshotError::Character {
                key: key.clone(),
                source,
            })?;
        if c.guid.is_empty() {
            c.guid = key;
        }
        // Forever names characters First-Last; fill a missing half from the full name.
        if let Some((first, last)) = c.full_name.as_deref().and_then(|f| f.split_once('-')) {
            if c.first_name.is_none() {
                c.first_name = Some(first.to_string());
            }
            if c.last_name.is_none() {
                c.last_name = Some(last.to_string());
            }
        }
        if c.realm_id.is_none() {
            c.realm_id = guid_server_id(&c.guid);
        }
        characters.push(c);
    }
    characters.sort_by(|a, b| a.guid.cmp(&b.guid));

    let mut guilds = Vec::new();
    for (key, value) in keyed_entries(db.get("guilds"), "guilds")? {
        let mut g: Guild =
            serde_json::from_value(value).map_err(|source| SnapshotError::Guild {
                key: key.clone(),
                source,
            })?;
        // The guild's key is opaque (the addon uses "<name>-<realmId>": no guild GUID API is
        // known on this client), so nothing is read out of it.
        if g.guid.is_empty() {
            g.guid = key;
        }
        guilds.push(g);
    }
    guilds.sort_by(|a, b| a.guid.cmp(&b.guid));

    let mut journal = Vec::new();
    let mut dropped = 0usize;
    let mut first_problem = None;
    for value in values_in_order(db.get("journal").cloned().unwrap_or(Value::Null)) {
        match serde_json::from_value::<JournalEvent>(value) {
            Ok(e) => journal.push(e),
            Err(e) => {
                dropped += 1;
                first_problem.get_or_insert_with(|| e.to_string());
            }
        }
    }
    if dropped > 0 {
        warnings.push(format!(
            "dropped {dropped} journal event(s) without session, seq, at, char and type (first: {})",
            first_problem.unwrap_or_default()
        ));
    }

    let journal_through = journal.iter().map(|e| e.at).max();
    Ok(Built {
        account_id: meta_account_id(db),
        journal_through,
        item_ids: item_ids(db),
        snapshot: Snapshot {
            schema: SCHEMA_MAJOR,
            addon,
            agent: AGENT_VERSION.to_string(),
            flavor,
            account: Account {
                key: account_key(source.account_folder),
                label: source
                    .label
                    .map(str::to_string)
                    .unwrap_or_else(|| default_label(source.account_folder)),
            },
            saved_at: source.saved_at,
            characters,
            guilds,
            journal,
        },
        warnings,
    })
}

/// Read, parse and build one account's `Ryna.lua`. `saved_at` is taken from the file's mtime.
pub fn read_file(
    path: &Path,
    account_folder: &str,
    label: Option<&str>,
    install_flavor: Option<&str>,
) -> Result<Built, ReadError> {
    let shown = path.display().to_string();
    let io = |source| ReadError::Io {
        path: shown.clone(),
        source,
    };
    let bytes = std::fs::read(path).map_err(io)?;
    let saved_at = crate::util::mtime(path).map_err(io)?.secs;
    let text = String::from_utf8_lossy(&bytes);
    let parsed = lua::parse(&text).map_err(|source| ReadError::Lua {
        path: shown.clone(),
        source,
    })?;
    build(
        &parsed,
        &Source {
            account_folder,
            label,
            saved_at,
            install_flavor,
        },
    )
    .map_err(|source| ReadError::Snapshot {
        path: shown.clone(),
        source,
    })
}

/// A GUID-keyed table as (key, record) pairs in key order. A list (an addon that wrote one) is
/// keyed by position; the record's own `guid` then names it.
fn keyed_entries(
    v: Option<&Value>,
    field: &'static str,
) -> Result<Vec<(String, Value)>, SnapshotError> {
    match v {
        None | Some(Value::Null) => Ok(Vec::new()),
        Some(Value::Object(m)) => Ok(m.iter().map(|(k, v)| (k.clone(), v.clone())).collect()),
        Some(Value::Array(a)) => Ok(a
            .iter()
            .enumerate()
            .map(|(i, v)| ((i + 1).to_string(), v.clone()))
            .collect()),
        Some(_) => Err(SnapshotError::NotATableField { field }),
    }
}

/// Order object keys as Lua indexes when both are integers, else as strings.
fn key_order(a: &str, b: &str) -> std::cmp::Ordering {
    match (a.parse::<i64>(), b.parse::<i64>()) {
        (Ok(x), Ok(y)) => x.cmp(&y),
        (Ok(_), Err(_)) => std::cmp::Ordering::Less,
        (Err(_), Ok(_)) => std::cmp::Ordering::Greater,
        (Err(_), Err(_)) => a.cmp(b),
    }
}

/// The values of a list-like table: an array as is, an index-keyed object in index order.
fn values_in_order(v: Value) -> Vec<Value> {
    match v {
        Value::Null => Vec::new(),
        Value::Array(a) => a,
        Value::Object(m) => {
            let mut entries: Vec<(String, Value)> = m.into_iter().collect();
            entries.sort_by(|a, b| key_order(&a.0, &b.0));
            entries.into_iter().map(|(_, v)| v).collect()
        }
        other => vec![other],
    }
}

fn list<'de, D, T>(d: D) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: DeserializeOwned,
{
    values_in_order(Value::deserialize(d)?)
        .into_iter()
        .map(|v| serde_json::from_value(v).map_err(D::Error::custom))
        .collect()
}

/// Ids as a list (`{ 1, 2 }`) or as a set (`{ [1] = true, [2] = true }`), sorted when a set.
fn id_list<'de, D>(d: D) -> Result<Vec<i64>, D::Error>
where
    D: Deserializer<'de>,
{
    let v = Value::deserialize(d)?;
    if let Value::Object(m) = &v {
        if !m.is_empty() && m.values().all(|x| x == &Value::Bool(true)) {
            let mut ids = m
                .keys()
                .map(|k| {
                    k.parse::<i64>()
                        .map_err(|_| D::Error::custom(format!("`{k}` is not an id")))
                })
                .collect::<Result<Vec<_>, _>>()?;
            ids.sort_unstable();
            return Ok(ids);
        }
    }
    values_in_order(v)
        .into_iter()
        .map(|x| {
            x.as_i64()
                .ok_or_else(|| D::Error::custom(format!("`{x}` is not an id")))
        })
        .collect()
}

fn is_empty_table(v: &Value) -> bool {
    match v {
        Value::Null => true,
        Value::Array(a) => a.is_empty(),
        _ => false,
    }
}

fn table<'de, D, T>(d: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: DeserializeOwned + Default,
{
    let v = Value::deserialize(d)?;
    if is_empty_table(&v) {
        return Ok(T::default());
    }
    serde_json::from_value(v).map_err(D::Error::custom)
}

/// A string-keyed map. An empty Lua table parses as `[]`, so that is the empty map; a list (an
/// addon that wrote integer keys 1..n) is keyed by position.
fn map<'de, D, T>(d: D) -> Result<BTreeMap<String, T>, D::Error>
where
    D: Deserializer<'de>,
    T: DeserializeOwned,
{
    let entries: Vec<(String, Value)> = match Value::deserialize(d)? {
        Value::Null => Vec::new(),
        Value::Object(m) => m.into_iter().collect(),
        Value::Array(a) => a
            .into_iter()
            .enumerate()
            .map(|(i, v)| ((i + 1).to_string(), v))
            .collect(),
        other => return Err(D::Error::custom(format!("`{other}` is not a table"))),
    };
    entries
        .into_iter()
        .map(|(k, v)| {
            serde_json::from_value(v)
                .map(|v| (k.clone(), v))
                .map_err(|e| D::Error::custom(format!("`{k}`: {e}")))
        })
        .collect()
}

fn opt_table<'de, D, T>(d: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: DeserializeOwned,
{
    let v = Value::deserialize(d)?;
    if is_empty_table(&v) {
        return Ok(None);
    }
    serde_json::from_value(v)
        .map(Some)
        .map_err(D::Error::custom)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const FIXTURE: &str = include_str!("../../fixtures/Ryna.lua");

    fn source(folder: &str) -> Source<'_> {
        Source {
            account_folder: folder,
            label: None,
            saved_at: 1_762_464_000,
            install_flavor: Some("wowf"),
        }
    }

    fn fixture() -> Built {
        build(&lua::parse(FIXTURE).unwrap(), &source("FAKEACCOUNT1")).unwrap()
    }

    fn db(body: &str) -> Value {
        lua::parse(&format!(
            "RynaDB = {{ schema = 1, addon = \"0.1.0\", flavor = \"wowf\", {body} }}"
        ))
        .unwrap()
    }

    #[test]
    fn account_key_and_label() {
        assert_eq!(account_key("FAKEACCOUNT1").len(), 16);
        assert!(account_key("FAKEACCOUNT1")
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
        // sha256("abc") = ba7816bf 8f01cfea 4141...
        assert_eq!(account_key("abc"), "ba7816bf8f01cfea");
        assert_eq!(default_label("FAKEACCOUNT1"), "UNT1");
        assert_eq!(default_label("12345678#1"), "78#1");
        assert_eq!(default_label("AB"), "AB");
        assert_eq!(default_label("ÄÖÜßé"), "ÖÜßé");
    }

    #[test]
    fn fixture_builds_the_section_2_shape() {
        let built = fixture();
        assert!(built.warnings.is_empty(), "{:?}", built.warnings);
        let s = &built.snapshot;
        assert_eq!(s.schema, 1);
        assert_eq!(s.addon, "0.1.0");
        assert_eq!(s.agent, AGENT_VERSION);
        assert_eq!(s.flavor, "wowf");
        assert_eq!(s.account.key, account_key("FAKEACCOUNT1"));
        assert_eq!(s.account.label, "UNT1");
        assert_eq!(s.saved_at, 1_762_464_000);
        assert_eq!(s.characters.len(), 2);
        assert_eq!(s.guilds.len(), 1);

        let aerin = s
            .characters
            .iter()
            .find(|c| c.guid == "Player-91-0F00A001")
            .unwrap();
        assert_eq!(aerin.full_name.as_deref(), Some("Aerin-Stormcall"));
        assert_eq!(aerin.first_name.as_deref(), Some("Aerin"));
        assert_eq!(aerin.last_name.as_deref(), Some("Stormcall"));
        assert_eq!(aerin.realm_id, Some(91));
        assert_eq!(aerin.class.as_deref(), Some("SHAMAN"));
        assert_eq!(aerin.guild.as_ref().unwrap().rank_index, Some(3));
        assert!(!aerin.gear.is_empty());
        assert!(aerin
            .containers
            .iter()
            .any(|c| c.container.as_deref() == Some("bank")));
        assert!(aerin
            .containers
            .iter()
            .any(|c| c.container.as_deref() == Some("keyring")));
        assert!(!aerin.quests.active.is_empty());
        assert!(aerin.quests.completed.len() >= 5);
        assert_eq!(aerin.professions[0].name.as_deref(), Some("Enchanting"));
        assert!(!aerin.professions[0].recipes.is_empty());

        let thor = s
            .characters
            .iter()
            .find(|c| c.guid == "Player-91-0F00A002")
            .unwrap();
        assert_eq!(thor.full_name.as_deref(), Some("Thorbrand-Ironhew"));
        // Played time hadn't arrived for this one, no bank visit, no guild: explicit nulls.
        let tj = serde_json::to_value(thor).unwrap();
        assert_eq!(tj["playedTotal"], Value::Null);
        assert_eq!(tj["bankAsOf"], Value::Null);
        assert_eq!(tj["guild"], Value::Null);
        assert_eq!(tj["talents"], Value::Null);
        assert_eq!(
            tj["containers"].as_array().map(Vec::len),
            Some(thor.containers.len())
        );

        let g = &s.guilds[0];
        assert_eq!(g.guid, "Ember Wardens-91");
        assert_eq!(g.realm_id, Some(91));
        assert!(g.roster.len() >= 3);
        assert!(g.roster.iter().any(|r| r.online == Some(true)));
    }

    #[test]
    fn fixture_journal_has_every_event_type_and_keeps_unknown_ones() {
        let s = fixture().snapshot;
        let mut seen = Vec::new();
        for e in &s.journal {
            if let Some(t) = e.known_type() {
                if !seen.contains(&t) {
                    seen.push(t);
                }
            }
        }
        for t in [
            EventType::Session,
            EventType::Zone,
            EventType::QuestAccept,
            EventType::QuestDone,
            EventType::Level,
            EventType::Death,
            EventType::Gold,
            EventType::Group,
            EventType::Seen,
            EventType::Who,
        ] {
            assert!(seen.contains(&t), "fixture journal lacks {t:?}");
        }
        let unknown = s
            .journal
            .iter()
            .find(|e| e.known_type().is_none())
            .expect("an unknown type");
        let wire = serde_json::to_value(unknown).unwrap();
        assert_eq!(wire["type"], json!("mailbox"));
        assert_eq!(wire["future"], json!({ "nested": [1, 2, 3], "flag": true }));
        // Order is the file's.
        let seqs: Vec<i64> = s.journal.iter().take(3).map(|e| e.seq).collect();
        assert_eq!(seqs, vec![1, 2, 3]);
        let group = s.journal.iter().find(|e| e.kind == "group").unwrap();
        assert_eq!(
            group.data["members"],
            json!(["Player-91-0F00B001", "Player-91-0F00B002"])
        );
    }

    #[test]
    fn wire_json_matches_section_2_field_names() {
        let v = serde_json::to_value(fixture().snapshot).unwrap();
        for k in [
            "schema",
            "addon",
            "agent",
            "flavor",
            "account",
            "savedAt",
            "characters",
            "guilds",
            "journal",
        ] {
            assert!(v.get(k).is_some(), "missing {k}");
        }
        assert_eq!(v.as_object().unwrap().len(), 9);
        let c = &v["characters"][0];
        for k in [
            "guid",
            "firstName",
            "lastName",
            "fullName",
            "realm",
            "realmId",
            "class",
            "race",
            "faction",
            "gender",
            "level",
            "xp",
            "xpMax",
            "gold",
            "playedTotal",
            "playedLevel",
            "zone",
            "subzone",
            "lastSeen",
            "guild",
            "gear",
            "talents",
            "professions",
            "reputations",
            "quests",
            "containers",
            "bankAsOf",
        ] {
            assert!(c.get(k).is_some(), "character missing {k}");
        }
        assert!(c["gear"][0].get("itemId").is_some());
        assert!(c["reputations"][0].get("factionId").is_some());
        assert!(c["quests"]["active"][0].get("acceptedAt").is_some());
        let r = &v["guilds"][0]["roster"][0];
        for k in [
            "fullName",
            "guid",
            "level",
            "class",
            "rankIndex",
            "rankName",
            "officerNote",
            "online",
            "lastOnlineAt",
        ] {
            assert!(r.get(k).is_some(), "roster entry missing {k}");
        }
        let e = &v["journal"][0];
        for k in ["session", "seq", "at", "char", "type"] {
            assert!(e.get(k).is_some(), "event missing {k}");
        }
        // Round-trips through the typed structs unchanged.
        let back: Snapshot = serde_json::from_value(v.clone()).unwrap();
        assert_eq!(serde_json::to_value(back).unwrap(), v);
    }

    #[test]
    fn schema_other_than_1_is_refused() {
        for s in ["2", "0", "\"1\"", "1.5"] {
            let v = lua::parse(&format!("RynaDB = {{ schema = {s}, flavor = \"wowf\" }}")).unwrap();
            let e = build(&v, &source("A")).unwrap_err();
            assert!(
                matches!(e, SnapshotError::SchemaUnsupported { .. }),
                "{s}: {e}"
            );
            assert!(e.to_string().contains("update the agent"), "{e}");
        }
        let v = lua::parse("RynaDB = { flavor = \"wowf\" }").unwrap();
        assert!(matches!(
            build(&v, &source("A")),
            Err(SnapshotError::NoSchema)
        ));
        // The schema is checked before anything else, so a bad field in a future schema can't mask it.
        let v = lua::parse("RynaDB = { schema = 2, characters = 7 }").unwrap();
        assert!(matches!(
            build(&v, &source("A")),
            Err(SnapshotError::SchemaUnsupported { .. })
        ));
    }

    #[test]
    fn not_a_ryna_file() {
        let v = lua::parse("OtherAddonDB = { }").unwrap();
        assert!(matches!(
            build(&v, &source("A")),
            Err(SnapshotError::NoRynaDb)
        ));
        let v = lua::parse("RynaDB = 3").unwrap();
        assert!(matches!(
            build(&v, &source("A")),
            Err(SnapshotError::NotATable)
        ));
    }

    #[test]
    fn empty_tables_and_missing_fields() {
        let v = db("characters = {}, guilds = {}, journal = {}");
        let s = build(&v, &source("A")).unwrap().snapshot;
        assert!(s.characters.is_empty() && s.guilds.is_empty() && s.journal.is_empty());

        let v = db(
            r#"characters = { ["Player-7-00000001"] = { fullName = "Ab-Cd", quests = {}, guild = {}, gear = {} } }"#,
        );
        let s = build(&v, &source("A")).unwrap().snapshot;
        let c = &s.characters[0];
        assert_eq!(c.guid, "Player-7-00000001");
        assert_eq!(c.realm_id, Some(7));
        assert_eq!(
            (c.first_name.as_deref(), c.last_name.as_deref()),
            (Some("Ab"), Some("Cd"))
        );
        assert!(c.guild.is_none());
        assert_eq!(c.quests, Quests::default());
        let wire = serde_json::to_value(c).unwrap();
        assert_eq!(wire["quests"], json!({ "active": [], "completed": [] }));
        assert_eq!(wire["gear"], json!([]));
        assert!(wire.get("level").is_none(), "absent stays absent");
    }

    #[test]
    fn sets_and_sparse_lists() {
        let v = db(r#"characters = { ["Player-1-00000001"] = {
            quests = { completed = { [30] = true, [4] = true, [100] = true } },
            professions = { { id = 333, recipes = { [7420] = true, [7418] = true } } },
            gear = { [16] = { slot = 16, itemId = 2 }, [1] = { slot = 1, itemId = 1 }, [5] = { slot = 5, itemId = 3 } },
        } }"#);
        let c = &build(&v, &source("A")).unwrap().snapshot.characters[0];
        assert_eq!(c.quests.completed, vec![4, 30, 100]);
        assert_eq!(c.professions[0].recipes, vec![7418, 7420]);
        assert_eq!(
            c.gear.iter().map(|g| g.slot.unwrap()).collect::<Vec<_>>(),
            vec![1, 5, 16]
        );
    }

    #[test]
    fn kills_loot_and_objects() {
        let v = db(r#"characters = { ["Player-1-00000001"] = {
            kills = {
                ["6"] = { name = "Kobold Vermin", n = 43, own = 40, xp = 1720, looted = 38,
                          last = 1762464000, zone = "Elwynn Forest" },
                ["40"] = { n = 2 },
            },
            loot = {
                ["2589"] = { name = "Linen Cloth", link = "|cffffffff|Hitem:2589::::::::5:::::::|h[Linen Cloth]|h|r",
                             quality = 1, n = 30, qty = 45, last = 1762464000,
                             from = { ["c:6"] = 22, ["c:40"] = 8 }, future = true },
            },
            objects = { ["1731"] = { n = 5, last = 1762464000, zone = "Elwynn Forest" } },
        } }"#);
        let c = &build(&v, &source("A")).unwrap().snapshot.characters[0];
        assert_eq!(
            c.kills["6"],
            KillStat {
                name: Some("Kobold Vermin".into()),
                n: 43,
                own: 40,
                xp: 1720,
                looted: 38,
                last: Some(1_762_464_000),
                zone: Some("Elwynn Forest".into()),
                extra: Map::new(),
            }
        );
        assert_eq!(
            c.kills["40"],
            KillStat {
                n: 2,
                ..KillStat::default()
            },
            "missing counters are 0"
        );
        let l = &c.loot["2589"];
        assert_eq!((l.n, l.qty, l.quality), (30, 45, Some(1)));
        assert_eq!(
            l.from,
            BTreeMap::from([("c:40".to_string(), 8), ("c:6".to_string(), 22)])
        );
        assert_eq!(c.objects["1731"].n, 5);

        let wire = serde_json::to_value(c).unwrap();
        assert_eq!(
            wire["kills"],
            json!({
                "6": { "name": "Kobold Vermin", "n": 43, "own": 40, "xp": 1720, "looted": 38,
                       "last": 1_762_464_000, "zone": "Elwynn Forest" },
                "40": { "n": 2, "own": 0, "xp": 0, "looted": 0 },
            })
        );
        assert_eq!(
            wire["loot"]["2589"],
            json!({
                "name": "Linen Cloth",
                "link": "|cffffffff|Hitem:2589::::::::5:::::::|h[Linen Cloth]|h|r",
                "quality": 1, "n": 30, "qty": 45, "last": 1_762_464_000,
                "from": { "c:6": 22, "c:40": 8 }, "future": true,
            })
        );
        assert_eq!(
            wire["objects"],
            json!({ "1731": { "n": 5, "last": 1_762_464_000, "zone": "Elwynn Forest" } })
        );
        let text = serde_json::to_string(c).unwrap();
        assert!(text.contains(r#""kills":{"40":{"#), "{text}");
        assert!(text.contains(r#""6":{"name":"Kobold Vermin""#), "{text}");
    }

    #[test]
    fn empty_kills_loot_and_objects() {
        let v = db(
            r#"characters = { ["Player-1-00000001"] = { kills = {}, loot = { ["1"] = { from = {} } }, objects = {} } }"#,
        );
        let c = &build(&v, &source("A")).unwrap().snapshot.characters[0];
        assert!(c.kills.is_empty() && c.objects.is_empty());
        assert!(c.loot["1"].from.is_empty());
        let wire = serde_json::to_value(c).unwrap();
        assert!(wire.get("kills").is_none() && wire.get("objects").is_none());
        assert_eq!(wire["loot"]["1"]["from"], json!({}));

        let v = db(r#"characters = { ["Player-1-00000001"] = { level = 1 } }"#);
        let c = &build(&v, &source("A")).unwrap().snapshot.characters[0];
        assert!(c.kills.is_empty() && c.loot.is_empty() && c.objects.is_empty());
        let wire = serde_json::to_value(c).unwrap();
        assert!(wire.get("kills").is_none() && wire.get("loot").is_none());

        let v = db(r#"characters = { ["Player-1-00000001"] = { kills = 5 } }"#);
        assert!(build(&v, &source("A")).is_err());
    }

    #[test]
    fn unknown_fields_pass_through() {
        let v = db(
            r#"characters = { ["Player-1-00000001"] = { level = 3, mount = { id = 5 }, gear = { { slot = 1, transmog = 99 } } } }, futureTable = { 1 }"#,
        );
        let c = &build(&v, &source("A")).unwrap().snapshot.characters[0];
        let wire = serde_json::to_value(c).unwrap();
        assert_eq!(wire["mount"], json!({ "id": 5 }));
        assert_eq!(wire["gear"][0]["transmog"], json!(99));
    }

    #[test]
    fn a_wrong_type_names_the_character() {
        let v = db(r#"characters = { ["Player-1-00000009"] = { level = "high" } }"#);
        let e = build(&v, &source("A")).unwrap_err();
        assert!(
            e.to_string().starts_with("character Player-1-00000009:"),
            "{e}"
        );
    }

    #[test]
    fn the_account_id_and_journal_reach_are_kept_and_never_sent() {
        let v = db(r#"meta = { accountId = "0123456789ABCDEF" }, journal = {
            { session = "s", seq = 1, at = 100, char = "c", type = "zone" },
            { session = "s", seq = 2, at = 300, char = "c", type = "auction" },
            { session = "s", seq = 3, at = 200, char = "c", type = "gold" },
        }"#);
        let b = build(&v, &source("FAKEACCOUNT1")).unwrap();
        assert_eq!(b.account_id.as_deref(), Some("0123456789abcdef"));
        assert_eq!(b.journal_through, Some(300));
        let sent = serde_json::to_string(&b.snapshot).unwrap();
        assert!(
            !sent.contains("0123456789") && !sent.contains("accountId") && !sent.contains("meta")
        );

        // A core without the id (or a malformed one), and an empty journal.
        for meta in [
            "",
            r#"meta = { accountId = "short" },"#,
            r#"meta = { accountId = 7 },"#,
        ] {
            let b = build(
                &db(&format!("{meta} journal = {{}}")),
                &source("FAKEACCOUNT1"),
            )
            .unwrap();
            assert_eq!(b.account_id, None, "{meta}");
            assert_eq!(b.journal_through, None);
        }
    }

    #[test]
    fn malformed_journal_events_are_dropped_with_a_warning() {
        let v = db(r#"journal = {
            { session = "s", seq = 1, at = 10, char = "Player-1-00000001", type = "death" },
            { seq = 2, at = 11, type = "death" },
            { session = "s", seq = 3, at = 12, char = "Player-1-00000001", type = "teleport", to = "Moonglade" },
        }"#);
        let b = build(&v, &source("A")).unwrap();
        assert_eq!(b.snapshot.journal.len(), 2);
        assert_eq!(b.snapshot.journal[1].data["to"], json!("Moonglade"));
        assert_eq!(b.warnings.len(), 1);
        assert!(b.warnings[0].contains("dropped 1 journal event"));
    }

    #[test]
    fn flavor_falls_back_to_the_install() {
        let v = lua::parse("RynaDB = { schema = 1 }").unwrap();
        let s = build(&v, &source("A")).unwrap();
        assert_eq!(s.snapshot.flavor, "wowf");
        assert_eq!(s.snapshot.addon, "unknown");
        let mut no_flavor = source("A");
        no_flavor.install_flavor = None;
        assert!(matches!(
            build(&v, &no_flavor),
            Err(SnapshotError::NoFlavor)
        ));
    }

    #[test]
    fn guild_keys_are_opaque() {
        let v = db(r#"guilds = { ["Night-Watch-91"] = { name = "Night-Watch", roster = {} } }"#);
        let g = &build(&v, &source("A")).unwrap().snapshot.guilds[0];
        assert_eq!(g.guid, "Night-Watch-91");
        assert_eq!(g.realm_id, None, "nothing is parsed out of a guild key");
    }

    #[test]
    fn settings_and_client_are_not_in_the_snapshot_but_client_is_readable() {
        let parsed = lua::parse(FIXTURE).unwrap();
        let wire = serde_json::to_value(fixture().snapshot).unwrap();
        assert!(wire.get("settings").is_none() && wire.get("client").is_none());
        let client = client_info(&parsed).expect("fixture records client");
        assert_eq!(client["interface"], json!(11600));
        let line = client_summary(client);
        assert!(
            line.starts_with("interface 11600, version \"1.60.0\""),
            "{line}"
        );
        assert!(line.contains("unitFullName \"Aerin-Stormcall\""), "{line}");
    }

    #[test]
    fn read_file_uses_the_mtime() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Ryna.lua");
        std::fs::write(&path, FIXTURE).unwrap();
        let built = read_file(&path, "FAKEACCOUNT1", Some("Main"), None).unwrap();
        let mtime = crate::util::mtime(&path).unwrap().secs;
        assert_eq!(built.snapshot.saved_at, mtime);
        assert_eq!(built.snapshot.account.label, "Main");

        std::fs::write(&path, "RynaDB = { schema = 2 }").unwrap();
        assert!(read_file(&path, "A", None, None)
            .unwrap_err()
            .is_schema_unsupported());
        std::fs::write(&path, "RynaDB = {").unwrap();
        assert!(matches!(
            read_file(&path, "A", None, None),
            Err(ReadError::Lua { .. })
        ));
    }
}
