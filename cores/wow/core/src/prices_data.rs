//! The Prices mod's way back down (docs/ryna-agent-cores-mods.md 7 and 7.3; the API's docs/wow.md
//! 17.2): after a sync, the pooled vendor offers and auction aggregates for the items an account
//! knows are written as two Lua files beside the addon, which the game loads at login or `/reload`:
//!
//! ```lua
//! RynaData_vendors = {
//!   version = 1, writtenAt = 1762464000, flavor = "wowf",
//!   items = { [2589] = { { 3881, "Kiro", "Durotar", 20, 1, 2, false }, … } },  -- npcId, name, zone, price (copper for `stack`), stack, observers, costs something besides copper
//! }
//! RynaData_auctions = {
//!   version = 1, writtenAt = 1762464000, flavor = "wowf",
//!   items = { [2589] = { 80, 110, 3, 2, 2, 1762464060, 110, 1, 2 } },  -- today's agreed min, median, listings, snapshots, observers, asOf; the week's median, days, snapshots (0: none)
//! }
//! ```
//!
//! One plain global per file (`RynaData_<table>`, the form SavedVariables take, which [`crate::lua`]
//! reads back; the core addon's `Ryna.Data.Get(name)` reads it). Each table carries its `version`
//! (the addon reads only the one it knows) and `writtenAt`
//! (the addon's "as of"). The ids come from the account's `RynaDB` ([`crate::snapshot::item_ids`]:
//! bags and bank first, then loot seen, then the auction store), capped at [`MAX_IDS`] per
//! install, and are asked for a page of [`crate::push::AGENT_PRICES_PAGE`] at a time. Rows are
//! budgeted ([`BUDGET_BYTES`] a file): at most [`SELLERS`] sellers per item, one auction row per
//! item, and over the budget the vendor file first drops every item's third seller, then its
//! second, then items from the end of the id order; the auction file drops items from the end.
//! A file is written atomically (a temp file and a rename) and only when its rows changed
//! (`writtenAt` alone doesn't count), so a quiet sync costs no disk write and no reload shows a
//! different age for the same numbers. Nothing here is about a person: the files name NPCs, zones,
//! prices and counts, and the strings are escaped so no name can carry a colour code or link.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::agent_status::lua_string;
use crate::discovery::Discovery;
use crate::lua_write::write_atomic;
use crate::push::{Api, PushError, AGENT_PRICES_PAGE};
use crate::snapshot;
use crate::state::State;

/// The folder under the addon's.
pub const DIR: &str = "Data";
pub const VENDORS_FILE: &str = "vendors.lua";
pub const AUCTIONS_FILE: &str = "auctions.lua";
/// The tables' version: the addon reads only this one (Ryna_Prices Pooled.lua).
pub const VERSION: u32 = 1;
/// Ids asked for per install and refresh: the account's bags, bank and loot run to a few thousand.
pub const MAX_IDS: usize = 4000;
/// Sellers kept per item (the API gives at most this many too, docs/wow.md 17.2).
pub const SELLERS: usize = 3;
/// Each file's size budget: the data layer stays in the low hundreds of kilobytes.
pub const BUDGET_BYTES: usize = 256 * 1024;
/// How often an install's tables are refreshed when no account file changed: the pools move as
/// others play, the account's own ids don't.
pub const REFRESH_EVERY_S: i64 = 6 * 3600;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Vendor {
    pub npc_id: i64,
    pub name: String,
    pub zone: String,
    /// Copper, for `stack` pieces.
    pub price: i64,
    pub stack: i64,
    pub observers: i64,
    /// The item also costs something besides copper (the tooltip says "+ other costs").
    pub currency: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Auction {
    /// Today's row (the newest in the last 2 days), zeros when there is none.
    pub min: i64,
    pub med: i64,
    pub listings: i64,
    pub snapshots: i64,
    pub observers: i64,
    pub as_of: i64,
    /// The last 7 days, zeros when there are none.
    pub week_med: i64,
    pub week_days: i64,
    pub week_snapshots: i64,
}

impl Auction {
    pub fn is_empty(&self) -> bool {
        self.as_of == 0 && self.week_days == 0
    }
}

/// What the API said about a set of items: sellers per item (in the API's order, best first) and
/// the auction aggregate of each item that has one.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Pooled {
    pub vendors: BTreeMap<i64, Vec<Vendor>>,
    pub auctions: BTreeMap<i64, Auction>,
}

fn int(v: Option<&Value>) -> i64 {
    v.and_then(Value::as_i64).unwrap_or(0)
}

fn text(v: Option<&Value>) -> String {
    // A name the API let through holds no `|` or control character (docs/wow.md 16.2); cut it
    // out here too, so a file can never carry a colour code or a link whatever arrives.
    v.and_then(Value::as_str)
        .unwrap_or("")
        .chars()
        .filter(|c| *c != '|' && !c.is_control())
        .take(100)
        .collect()
}

/// Fold one page of `/agent/prices`' `data` into `pooled`. Anything that isn't shaped as
/// documented is skipped, never an error: the file is only ever as good as what was understood.
pub fn fold_page(pooled: &mut Pooled, data: &Value) {
    let Some(items) = data.get("items").and_then(Value::as_array) else {
        return;
    };
    for item in items {
        let id = int(item.get("itemId"));
        if id <= 0 {
            continue;
        }
        if let Some(vendors) = item.get("vendors").and_then(Value::as_array) {
            let rows: Vec<Vendor> = vendors
                .iter()
                .filter_map(|v| {
                    let npc_id = int(v.get("npcId"));
                    let price = int(v.get("price"));
                    let stack = int(v.get("stack")).max(1);
                    (npc_id > 0 && price >= 0).then(|| Vendor {
                        npc_id,
                        name: text(v.get("name")),
                        zone: text(v.get("zone")),
                        price,
                        stack,
                        observers: int(v.get("observers")),
                        currency: v.get("currency").and_then(Value::as_bool).unwrap_or(false),
                    })
                })
                .take(SELLERS)
                .collect();
            if !rows.is_empty() {
                pooled.vendors.insert(id, rows);
            }
        }
        let mut a = Auction::default();
        if let Some(today) = item.get("today").filter(|t| t.is_object()) {
            a.min = int(today.get("min"));
            a.med = int(today.get("med"));
            a.listings = int(today.get("listings"));
            a.snapshots = int(today.get("snapshots"));
            a.observers = int(today.get("observers"));
            a.as_of = int(today.get("asOf"));
        }
        if let Some(week) = item.get("week").filter(|w| w.is_object()) {
            a.week_med = int(week.get("med"));
            a.week_days = int(week.get("days"));
            a.week_snapshots = int(week.get("snapshots"));
        }
        if !a.is_empty() {
            pooled.auctions.insert(id, a);
        }
    }
}

/// Ask the API for `ids` (at most `MAX_IDS`, the rest dropped), a page at a time.
pub fn fetch(api: &Api<'_>, ids: &[i64], flavor: &str) -> Result<Pooled, PushError> {
    let mut pooled = Pooled::default();
    for page in ids
        .iter()
        .take(MAX_IDS)
        .collect::<Vec<_>>()
        .chunks(AGENT_PRICES_PAGE)
    {
        let page: Vec<i64> = page.iter().map(|i| **i).collect();
        let data = api.agent_prices(&page, flavor)?;
        fold_page(&mut pooled, &data);
    }
    Ok(pooled)
}

/// The global a table is read from: `RynaData_<name>`.
pub fn global(name: &str) -> String {
    format!("RynaData_{name}")
}

fn header(out: &mut String, what: &str, name: &str, written_at: i64, flavor: &str) {
    out.push_str("-- Written by the Ryna companion app after a sync. Edits are overwritten.\n");
    let _ = writeln!(out, "-- {what}");
    let _ = writeln!(out, "{} = {{", global(name));
    let _ = writeln!(out, "\tversion = {VERSION},");
    let _ = writeln!(out, "\twrittenAt = {written_at},");
    let _ = writeln!(out, "\tflavor = {},", lua_string(flavor));
    out.push_str("\titems = {\n");
}

fn footer(out: &mut String) {
    out.push_str("\t},\n}\n");
}

fn vendor_row(v: &Vendor) -> String {
    format!(
        "{{ {}, {}, {}, {}, {}, {}, {} }}",
        v.npc_id,
        lua_string(&v.name),
        lua_string(&v.zone),
        v.price,
        v.stack,
        v.observers,
        v.currency
    )
}

/// `Data/vendors.lua`'s text for `ids` (in that order) from `pooled`, inside the budget: over it,
/// every item's third seller goes, then its second, then items from the end. Returns the text and
/// how many items it holds.
pub fn vendors_lua(pooled: &Pooled, ids: &[i64], written_at: i64, flavor: &str) -> (String, usize) {
    let mut keep = SELLERS;
    let mut count = ids
        .iter()
        .filter(|id| pooled.vendors.contains_key(id))
        .count();
    loop {
        let mut out = String::new();
        header(
            &mut out,
            "Data/vendors.lua: which vendors sell an item, pooled from everyone's visits. Rows: npcId, name, zone, price (copper, for `stack` pieces), stack, observers, costs something besides copper.",
            "vendors",
            written_at,
            flavor,
        );
        let mut n = 0;
        for id in ids {
            if n == count {
                break;
            }
            let Some(rows) = pooled.vendors.get(id) else {
                continue;
            };
            n += 1;
            let _ = write!(out, "\t\t[{id}] = {{ ");
            let mut first = true;
            for v in rows.iter().take(keep) {
                if !first {
                    out.push_str(", ");
                }
                first = false;
                out.push_str(&vendor_row(v));
            }
            out.push_str(" },\n");
        }
        footer(&mut out);
        if out.len() <= BUDGET_BYTES || (keep == 1 && n == 0) {
            return (out, n);
        }
        if keep > 1 {
            keep -= 1;
        } else {
            // Drop a slice of the items from the end, in proportion to the overrun.
            let over = out.len() as f64 / BUDGET_BYTES as f64;
            let target = ((n as f64 / over) as usize).min(n.saturating_sub(1));
            count = target;
        }
    }
}

fn auction_row(a: &Auction) -> String {
    format!(
        "{{ {}, {}, {}, {}, {}, {}, {}, {}, {} }}",
        a.min,
        a.med,
        a.listings,
        a.snapshots,
        a.observers,
        a.as_of,
        a.week_med,
        a.week_days,
        a.week_snapshots
    )
}

/// `Data/auctions.lua`'s text for `ids` (in that order) from `pooled`, inside the budget: over it,
/// items from the end go. Returns the text and how many items it holds.
pub fn auctions_lua(
    pooled: &Pooled,
    ids: &[i64],
    written_at: i64,
    flavor: &str,
) -> (String, usize) {
    let mut count = ids
        .iter()
        .filter(|id| pooled.auctions.contains_key(id))
        .count();
    loop {
        let mut out = String::new();
        header(
            &mut out,
            "Data/auctions.lua: everyone's auction house prices per item. Rows: today's agreed min, median, listings, snapshots, observers, asOf (unix); the week's median, days, snapshots (0: none).",
            "auctions",
            written_at,
            flavor,
        );
        let mut n = 0;
        for id in ids {
            if n == count {
                break;
            }
            let Some(a) = pooled.auctions.get(id) else {
                continue;
            };
            n += 1;
            let _ = writeln!(out, "\t\t[{id}] = {},", auction_row(a));
        }
        footer(&mut out);
        if out.len() <= BUDGET_BYTES || n == 0 {
            return (out, n);
        }
        let over = out.len() as f64 / BUDGET_BYTES as f64;
        count = ((n as f64 / over) as usize).min(n.saturating_sub(1));
    }
}

/// The text with its `writtenAt` line blanked: two files that differ only there hold the same data.
fn without_written_at(text: &str) -> String {
    text.lines()
        .filter(|l| !l.trim_start().starts_with("writtenAt = "))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Write `text` to `path` (creating its folder) unless the file there already holds the same
/// rows. Returns whether it was written.
pub fn write_if_changed(path: &Path, text: &str) -> std::io::Result<bool> {
    if let Ok(existing) = std::fs::read_to_string(path) {
        if without_written_at(&existing) == without_written_at(text) {
            return Ok(false);
        }
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    write_atomic(path, text.as_bytes())?;
    Ok(true)
}

/// One install's refresh, as the run reports it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refreshed {
    pub install: PathBuf,
    /// Ids asked for.
    pub asked: usize,
    pub vendor_items: usize,
    pub auction_items: usize,
    /// Which of the two files changed on disk.
    pub written: Vec<PathBuf>,
}

/// The item ids of every account under `install` that this run parsed, in `ids_by_account`
/// order, merged (each id at its first place), capped at `MAX_IDS`.
pub fn merge_ids<'a>(lists: impl Iterator<Item = &'a [i64]>) -> Vec<i64> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for list in lists {
        for &id in list {
            if seen.insert(id) {
                out.push(id);
                if out.len() == MAX_IDS {
                    return out;
                }
            }
        }
    }
    out
}

/// Refresh every install that has the addon: with the ids this run parsed for its accounts
/// (`ids_by_account`, by account file path), or, when none of them changed and the install's
/// tables are `REFRESH_EVERY_S` old, by reading its account files for their ids. Each install's
/// failure is returned, never raised: the run itself isn't the worse for a stale price table.
pub fn refresh(
    discovery: &Discovery,
    state: &mut State,
    api: &Api<'_>,
    ids_by_account: &BTreeMap<PathBuf, Vec<i64>>,
    now: i64,
) -> Vec<Result<Refreshed, (PathBuf, String)>> {
    let mut out = Vec::new();
    for i in &discovery.installs {
        let addon = crate::addon_install::addon_dir(i);
        if !addon.is_dir() {
            continue;
        }
        let key = i.path.to_string_lossy().into_owned();
        let accounts: Vec<_> = discovery
            .accounts
            .iter()
            .filter(|a| a.install == i.path)
            .collect();
        let fresh: Vec<&[i64]> = accounts
            .iter()
            .filter_map(|a| ids_by_account.get(&a.path).map(Vec::as_slice))
            .collect();
        let ids = if !fresh.is_empty() {
            merge_ids(fresh.into_iter())
        } else {
            let due = state
                .prices_data_at
                .get(&key)
                .is_none_or(|at| now - at >= REFRESH_EVERY_S);
            if !due {
                continue;
            }
            let mut lists = Vec::new();
            for a in &accounts {
                match snapshot::read_file(&a.path, &a.account, None, a.flavor.as_deref()) {
                    Ok(b) => lists.push(b.item_ids),
                    Err(e) => {
                        out.push(Err((i.path.clone(), e.to_string())));
                    }
                }
            }
            merge_ids(lists.iter().map(Vec::as_slice))
        };
        let flavor = i.flavor.as_deref().unwrap_or("wowf");
        let pooled = match fetch(api, &ids, flavor) {
            Ok(p) => p,
            Err(e) => {
                out.push(Err((i.path.clone(), e.to_string())));
                continue;
            }
        };
        let dir = addon.join(DIR);
        let (vendors, vendor_items) = vendors_lua(&pooled, &ids, now, flavor);
        let (auctions, auction_items) = auctions_lua(&pooled, &ids, now, flavor);
        let mut written = Vec::new();
        let mut failed = None;
        for (name, text) in [(VENDORS_FILE, vendors), (AUCTIONS_FILE, auctions)] {
            let path = dir.join(name);
            match write_if_changed(&path, &text) {
                Ok(true) => written.push(path),
                Ok(false) => {}
                Err(e) => failed = Some(format!("couldn't write {}: {e}", path.display())),
            }
        }
        if let Some(e) = failed {
            out.push(Err((i.path.clone(), e)));
            continue;
        }
        state.prices_data_at.insert(key, now);
        out.push(Ok(Refreshed {
            install: i.path.clone(),
            asked: ids.len(),
            vendor_items,
            auction_items,
            written,
        }));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::testing::MemoryStore;
    use crate::auth::Auth;
    use crate::config::test_config;
    use crate::http::testing::ScriptedTransport;
    use crate::lua;
    use crate::util::testing::FakeClock;
    use serde_json::json;

    fn vendor(npc: i64, name: &str, zone: &str, price: i64, observers: i64) -> Value {
        json!({ "npcId": npc, "name": name, "zone": zone, "price": price, "stack": 1, "currency": false, "observers": observers, "lastSeen": 5 })
    }

    fn page() -> Value {
        json!({ "flavor": "wowf", "items": [
            { "itemId": 2589, "vendors": [vendor(3881, "Kiro", "Durotar", 20, 2), vendor(3882, "Grol", "The Barrens", 25, 1), vendor(6929, "Innkeeper Gryshka", "Orgrimmar", 30, 1)],
              "today": { "min": 80, "med": 110, "listings": 3, "quantity": 10, "snapshots": 2, "observers": 2, "asOf": 1_762_464_060 },
              "week": { "med": 110, "days": 1, "snapshots": 2 } },
            { "itemId": 2592, "vendors": [], "today": null, "week": { "med": 400, "days": 3, "snapshots": 5 } },
            { "itemId": 159, "vendors": [ { "npcId": 6929, "name": "Inn|cff00ff00keeper\u{7}", "zone": null, "price": 25, "stack": 5, "currency": true, "observers": 1, "lastSeen": 5 } ], "today": null, "week": null },
            { "itemId": 77777, "vendors": [], "today": null, "week": null },
            { "itemId": "bad", "vendors": "x" },
        ] })
    }

    #[test]
    fn folds_a_page_and_writes_lua_the_reader_reads_back() {
        let mut pooled = Pooled::default();
        fold_page(&mut pooled, &page());
        assert_eq!(pooled.vendors.len(), 2);
        assert_eq!(pooled.vendors[&2589].len(), 3);
        // A name is stripped of `|` and control characters whatever arrived; a null zone is "".
        assert_eq!(pooled.vendors[&159][0].name, "Inncff00ff00keeper");
        assert_eq!(pooled.vendors[&159][0].zone, "");
        assert!(pooled.vendors[&159][0].currency);
        assert_eq!(pooled.auctions.len(), 2);
        assert_eq!(pooled.auctions[&2592].as_of, 0);
        assert_eq!(pooled.auctions[&2592].week_med, 400);
        assert!(!pooled.auctions.contains_key(&77777));

        let ids = [159, 2589, 2592, 77777];
        let (v, n) = vendors_lua(&pooled, &ids, 1_762_464_000, "wowf");
        assert_eq!(n, 2);
        let read = lua::parse(&v).unwrap();
        assert_eq!(read["RynaData_vendors"]["version"], json!(VERSION));
        assert_eq!(read["RynaData_vendors"]["writtenAt"], json!(1_762_464_000));
        assert_eq!(read["RynaData_vendors"]["flavor"], json!("wowf"));
        assert_eq!(
            read["RynaData_vendors"]["items"]["2589"],
            json!([
                [3881, "Kiro", "Durotar", 20, 1, 2, false],
                [3882, "Grol", "The Barrens", 25, 1, 1, false],
                [6929, "Innkeeper Gryshka", "Orgrimmar", 30, 1, 1, false]
            ])
        );
        assert_eq!(
            read["RynaData_vendors"]["items"]["159"],
            json!([[6929, "Inncff00ff00keeper", "", 25, 5, 1, true]])
        );
        assert!(read["RynaData_vendors"]["items"].get("77777").is_none());
        assert!(v.starts_with("-- Written by the Ryna companion app"));
        assert!(v.contains("\nRynaData_vendors = {\n"));

        let (a, n) = auctions_lua(&pooled, &ids, 1_762_464_000, "wowf");
        assert_eq!(n, 2);
        let read = lua::parse(&a).unwrap();
        assert_eq!(
            read["RynaData_auctions"]["items"]["2589"],
            json!([80, 110, 3, 2, 2, 1_762_464_060, 110, 1, 2])
        );
        assert_eq!(
            read["RynaData_auctions"]["items"]["2592"],
            json!([0, 0, 0, 0, 0, 0, 400, 3, 5])
        );
        // Both files together: two globals.
        let both = lua::parse(&format!("{v}{a}")).unwrap();
        assert!(both["RynaData_vendors"].is_object() && both["RynaData_auctions"].is_object());
    }

    #[test]
    fn the_budget_cuts_sellers_first_then_items_from_the_end() {
        let mut pooled = Pooled::default();
        // 6,000 items, three sellers each: about 3 × 60 bytes × 6,000, well over the budget. The
        // ids don't start at 1, so the reader gives a keyed table rather than an array.
        let ids: Vec<i64> = (1001..=7000).collect();
        for &id in &ids {
            pooled.vendors.insert(
                id,
                (0..3)
                    .map(|k| Vendor {
                        npc_id: 1000 + k,
                        name: format!("Vendor Number {k}"),
                        zone: "Some Zone Name".into(),
                        price: 100 + k,
                        stack: 1,
                        observers: 1,
                        currency: false,
                    })
                    .collect(),
            );
            pooled.auctions.insert(
                id,
                Auction {
                    min: 1,
                    med: 2,
                    listings: 3,
                    snapshots: 4,
                    observers: 5,
                    as_of: 1_762_464_000,
                    week_med: 2,
                    week_days: 1,
                    week_snapshots: 4,
                },
            );
        }
        let (v, n) = vendors_lua(&pooled, &ids, 1, "wowf");
        assert!(v.len() <= BUDGET_BYTES, "{}", v.len());
        let read = lua::parse(&v).unwrap();
        let items = read["RynaData_vendors"]["items"].as_object().unwrap();
        assert_eq!(items.len(), n);
        // One seller each (the cheapest), and the first ids kept.
        assert_eq!(items["1001"].as_array().unwrap().len(), 1);
        assert_eq!(items["1001"][0][0], json!(1000));
        assert!(items.contains_key("1001") && items.contains_key(&(1000 + n).to_string()));
        assert!(!items.contains_key("7000"));
        assert!(n >= 3000, "{n}");

        let (a, n) = auctions_lua(&pooled, &ids, 1, "wowf");
        assert!(a.len() <= BUDGET_BYTES, "{}", a.len());
        let read = lua::parse(&a).unwrap();
        let items = read["RynaData_auctions"]["items"].as_object().unwrap();
        assert_eq!(items.len(), n);
        assert!(items.contains_key("1001") && !items.contains_key("7000"));
        assert!(n >= 3000, "{n}");

        // Nothing to say: a valid, empty table.
        let (e, n) = vendors_lua(&Pooled::default(), &ids, 1, "wowf");
        assert_eq!(n, 0);
        assert_eq!(
            lua::parse(&e).unwrap()["RynaData_vendors"]["items"],
            json!([])
        );
    }

    #[test]
    fn writes_only_when_the_rows_changed() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("Ryna").join(DIR).join(VENDORS_FILE);
        let mut pooled = Pooled::default();
        fold_page(&mut pooled, &page());
        let (first, _) = vendors_lua(&pooled, &[2589], 100, "wowf");
        assert!(write_if_changed(&p, &first).unwrap());
        assert_eq!(std::fs::read_to_string(&p).unwrap(), first);
        // The same rows, later: not written, the old age stays.
        let (later, _) = vendors_lua(&pooled, &[2589], 200, "wowf");
        assert!(!write_if_changed(&p, &later).unwrap());
        assert_eq!(std::fs::read_to_string(&p).unwrap(), first);
        // A changed row: written.
        pooled.vendors.get_mut(&2589).unwrap()[0].price = 19;
        let (changed, _) = vendors_lua(&pooled, &[2589], 300, "wowf");
        assert!(write_if_changed(&p, &changed).unwrap());
        assert_eq!(std::fs::read_to_string(&p).unwrap(), changed);
        // No temp file left behind.
        assert_eq!(std::fs::read_dir(p.parent().unwrap()).unwrap().count(), 1);
    }

    #[test]
    fn fetches_in_pages_of_200_and_folds_them() {
        let http = ScriptedTransport::new();
        let config = test_config();
        let store = MemoryStore::with_access("tok", 2_000_000_000);
        let clock = FakeClock::at(1_800_000_000);
        let auth = Auth::new(&config, &http, &store, &clock);
        let api = Api::new(&config.api, &http, &auth, None, &clock);
        http.json(200, json!({ "ok": true, "data": page() }))
            .json(200, json!({ "ok": true, "data": { "flavor": "wowf", "items": [ { "itemId": 9001, "vendors": [vendor(1, "A", "B", 1, 1)], "today": null, "week": null } ] } }));
        let ids: Vec<i64> = (1..=201).collect();
        let pooled = fetch(&api, &ids, "wowf").unwrap();
        let urls = http.urls();
        assert_eq!(urls.len(), 2);
        assert!(urls[0].starts_with("https://api.test/v1/wow/agent/prices?items=1,2,3,"));
        assert!(urls[0].ends_with(",200&flavor=wowf"), "{}", urls[0]);
        assert_eq!(
            urls[1],
            "https://api.test/v1/wow/agent/prices?items=201&flavor=wowf"
        );
        let req = http.requests.borrow()[0].clone();
        assert_eq!(req.method, crate::http::Method::Get);
        assert_eq!(req.bearer.as_deref(), Some("tok"));
        assert!(
            req.body.is_empty() && req.headers.is_empty(),
            "a GET: unsigned, no body"
        );
        assert_eq!(pooled.vendors.len(), 3);
        assert!(pooled.vendors.contains_key(&9001));

        // A refusal is the error, and nothing is written from it.
        let http = ScriptedTransport::new();
        let auth = Auth::new(&config, &http, &store, &clock);
        let api = Api::new(&config.api, &http, &auth, None, &clock);
        http.json(
            403,
            json!({ "ok": false, "error": { "code": "FORBIDDEN", "message": "no" } }),
        );
        assert!(fetch(&api, &[1], "wowf").is_err());
        // Nothing to ask: no call.
        let http = ScriptedTransport::new();
        let auth = Auth::new(&config, &http, &store, &clock);
        let api = Api::new(&config.api, &http, &auth, None, &clock);
        assert_eq!(fetch(&api, &[], "wowf").unwrap(), Pooled::default());
        assert!(http.urls().is_empty());
    }

    #[test]
    fn merges_ids_in_order_and_caps_them() {
        let a = [5, 1, 2];
        let b = [2, 3, 1, 9];
        assert_eq!(
            merge_ids([a.as_slice(), b.as_slice()].into_iter()),
            vec![5, 1, 2, 3, 9]
        );
        let many: Vec<i64> = (1..=(MAX_IDS as i64 + 10)).collect();
        assert_eq!(merge_ids([many.as_slice()].into_iter()).len(), MAX_IDS);
    }
}
