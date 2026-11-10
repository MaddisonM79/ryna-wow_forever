//! Supported versions (docs/ryna-agent-cores-mods.md 9.1; the API's docs/wow.md section 18): what
//! the API last said about this app and its addons, kept to show, never to refuse.
//!
//! Every API answer that judges carries `support: { band, version, addon, channel, minimum,
//! preferred, perfect, note, update }`: the heartbeat's (always, even below minimum), the ingest's
//! and the census upload's, and `AGENT_UNSUPPORTED`'s `detail`. The band is one of `current`,
//! `behind` (an update is offered), `reduced` (your own data syncs; what you add to the census and
//! the price pools is paused) and `unsupported` (nothing syncs). The API decides and refuses; this
//! side only shows it. It **fails open**: no judgment known, or one this side can't read, is
//! `current`, and nothing here ever stops a sync.
//!
//! The last judgment is kept in `support.json` in the state folder (its own file, so the CLI's
//! census upload can write it without the run lock), with when it came and the app version it was
//! about: after an update, a judgment of the old version is never shown.

use std::fs::File;
use std::io::Write;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::fix::Fix;

pub const SUPPORT_FILE: &str = "support.json";

/// The release channel this build follows. Only `stable` exists; beta is a later stage (9.1
/// "Channels"), which adds a setting.
pub const CHANNEL: &str = ryna_agent::CHANNEL;

/// Maddison's words (2026-11-10), the same sentence in the app, the game and the portal.
pub const REDUCED_SENTENCE: &str = "This version of Ryna is getting old. Your characters still sync, but what you add to everyone's census and prices is paused until you update.";
pub const UNSUPPORTED_SENTENCE: &str =
    "This version of Ryna is no longer supported, so nothing syncs to ryna.io. Update to carry on.";

/// Where a person is, against the marks.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(rename_all = "camelCase")]
pub enum Band {
    /// At the newest release on the channel (or not judged, or not readable).
    #[default]
    Current,
    /// Everything works; an update is offered.
    Behind,
    /// Your own data syncs; contributions to the pools are paused.
    Reduced,
    /// Below minimum: the API refuses the agent's writes.
    Unsupported,
}

impl Band {
    /// A band as the API names it; anything else is `current` (failing open).
    pub fn parse(s: &str) -> Band {
        match s {
            "behind" => Band::Behind,
            "reduced" => Band::Reduced,
            "unsupported" => Band::Unsupported,
            _ => Band::Current,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Band::Current => "current",
            Band::Behind => "behind",
            Band::Reduced => "reduced",
            Band::Unsupported => "unsupported",
        }
    }

    /// The sentence for it, for the two bands that have one.
    pub fn sentence(self) -> Option<&'static str> {
        match self {
            Band::Reduced => Some(REDUCED_SENTENCE),
            Band::Unsupported => Some(UNSUPPORTED_SENTENCE),
            Band::Current | Band::Behind => None,
        }
    }
}

/// The API's `support`, as read (each string held to a sane length; missing ones None).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Support {
    pub band: Band,
    /// The app's version, as the API judged it.
    pub version: Option<String>,
    /// The oldest core addon judged with it (an install's TOC), or None.
    pub addon: Option<String>,
    pub channel: Option<String>,
    pub minimum: Option<String>,
    pub preferred: Option<String>,
    /// The newest release on the channel.
    pub perfect: Option<String>,
    /// The policy's one line for people ("0.3.4 mis-reads auction prices").
    pub note: Option<String>,
    /// Where to read how to update.
    pub update: Option<String>,
}

fn text(v: &Value, key: &str, max: usize) -> Option<String> {
    let s = v.get(key)?.as_str()?.trim();
    if s.is_empty() {
        return None;
    }
    Some(s.chars().take(max).collect())
}

impl Support {
    /// `support` from an answer (or `detail`), or None when it isn't one. An unknown band is
    /// `current`.
    pub fn from_value(v: &Value) -> Option<Support> {
        let band = v.get("band")?.as_str()?;
        Some(Support {
            band: Band::parse(band),
            version: text(v, "version", 40),
            addon: text(v, "addon", 40),
            channel: text(v, "channel", 20),
            minimum: text(v, "minimum", 40),
            preferred: text(v, "preferred", 40),
            perfect: text(v, "perfect", 40),
            note: text(v, "note", 200),
            update: text(v, "update", 200).filter(|u| u.starts_with("https://")),
        })
    }

    /// The `support` field of an answer's `data`, if it has one.
    pub fn in_answer(data: &Value) -> Option<Support> {
        data.get("support").and_then(Support::from_value)
    }
}

/// The last judgment and what came with it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KnownSupport {
    pub support: Support,
    /// When it came (unix seconds).
    pub received_at: i64,
    /// The app version it was about (this build's `AGENT_VERSION` when it was kept).
    pub agent: String,
    /// The last ingest's `pooled`: false while contributions are paused (reduced mode).
    #[serde(default)]
    pub pooled: Option<bool>,
}

impl KnownSupport {
    /// The kept judgment, if there is one this build can show: one kept by another version of
    /// the app (before an update) is about that version, so it's None.
    pub fn load(dir: &Path) -> Option<KnownSupport> {
        Self::load_any(dir).filter(|k| k.agent == crate::AGENT_VERSION)
    }

    fn load_any(dir: &Path) -> Option<KnownSupport> {
        std::fs::read(dir.join(SUPPORT_FILE))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
    }

    pub fn save(&self, dir: &Path) -> std::io::Result<()> {
        std::fs::create_dir_all(dir)?;
        let tmp = dir.join(format!(".{SUPPORT_FILE}.{}.tmp", std::process::id()));
        let result = (|| {
            let mut f = File::create(&tmp)?;
            f.write_all(&serde_json::to_vec_pretty(self).map_err(std::io::Error::other)?)?;
            f.sync_all()?;
            drop(f);
            std::fs::rename(&tmp, dir.join(SUPPORT_FILE))
        })();
        if result.is_err() {
            let _ = std::fs::remove_file(&tmp);
        }
        result
    }
}

/// The band to act on: the known one, else `current`.
pub fn band(known: Option<&KnownSupport>) -> Band {
    known.map_or(Band::Current, |k| k.support.band)
}

/// Keep `support`, received at `now`; returns the kept record and whether the band differs from
/// the one kept before (none kept counts as `current`). A failure to write is only said.
pub fn record(dir: &Path, support: Support, now: i64) -> (KnownSupport, bool) {
    let before = KnownSupport::load(dir);
    let changed = band(before.as_ref()) != support.band;
    let known = KnownSupport {
        support,
        received_at: now,
        agent: crate::AGENT_VERSION.to_string(),
        pooled: before.and_then(|b| b.pooled),
    };
    if let Err(e) = known.save(dir) {
        eprintln!("couldn't keep the API's version judgment: {e}");
    }
    (known, changed)
}

/// Note the last ingest's `pooled`. Returns true when contributions have just been paused (this
/// answer said `pooled: false` and the last one didn't), so the activity log says it once, not
/// every pass. Needs a kept judgment to hang on (the heartbeat brings one every pass).
pub fn note_pooled(dir: &Path, pooled: bool) -> bool {
    let Some(mut known) = KnownSupport::load(dir) else {
        return false;
    };
    let newly_paused = !pooled && known.pooled != Some(false);
    if known.pooled != Some(pooled) {
        known.pooled = Some(pooled);
        if let Err(e) = known.save(dir) {
            eprintln!("couldn't keep the API's version judgment: {e}");
        }
    }
    newly_paused
}

/// The words for a failure, given the last judgment: an update failure while unsupported is the
/// unsupported sentence; anything else is `Fix::message`.
pub fn advice(fix: Fix, error: &str, support: Option<&Support>) -> String {
    match support {
        Some(s) if fix == Fix::Update && s.band == Band::Unsupported => {
            UNSUPPORTED_SENTENCE.to_string()
        }
        _ => fix.message(error),
    }
}

/// A version as `0.3.9`, `v0.3.9` or `0.4.0-rc.1`: its numbers and pre-release, or None.
fn parse(v: &str) -> Option<(Vec<u64>, Option<String>)> {
    let v = v.trim().trim_start_matches(['v', 'V']);
    let v = v.split('+').next()?;
    let (nums, pre) = match v.split_once('-') {
        Some((n, p)) => (n, Some(p.to_string())),
        None => (v, None),
    };
    let mut parts = Vec::new();
    for p in nums.split('.') {
        parts.push(p.parse::<u64>().ok()?);
    }
    while parts.len() > 1 && parts.last() == Some(&0) {
        parts.pop();
    }
    Some((parts, pre))
}

/// Whether `a` is an older release than `b`; None when either isn't a version (failing open: an
/// unreadable version is never "older"). A pre-release is older than its release.
pub fn older_than(a: &str, b: &str) -> Option<bool> {
    let (pa, xa) = parse(a)?;
    let (pb, xb) = parse(b)?;
    Some(match pa.cmp(&pb) {
        std::cmp::Ordering::Less => true,
        std::cmp::Ordering::Greater => false,
        std::cmp::Ordering::Equal => match (xa, xb) {
            (Some(x), Some(y)) => x < y,
            (Some(_), None) => true,
            _ => false,
        },
    })
}

/// `latest.agent` for `AgentStatus.lua`: `perfect` when this app is older than it.
pub fn latest_agent(support: &Support) -> Option<String> {
    let perfect = support.perfect.as_deref()?;
    older_than(crate::AGENT_VERSION, perfect)
        .unwrap_or(false)
        .then(|| perfect.to_string())
}

/// `latest.addon` for one install: `perfect` only when that install's TOC is older than it. An
/// addon through 0.3.9 compares `latest.addon` with its frozen `0.3.2`, so filling it when the TOC
/// is already current would offer an update that isn't there.
pub fn latest_addon(support: &Support, toc: Option<&str>) -> Option<String> {
    let perfect = support.perfect.as_deref()?;
    older_than(toc?, perfect)
        .unwrap_or(false)
        .then(|| perfect.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn wire(band: &str) -> Value {
        json!({ "band": band, "version": "0.3.5", "addon": "0.3.4", "channel": "stable",
                "minimum": "0.3.5", "preferred": "0.3.7", "perfect": "0.3.9",
                "note": "0.3.4 mis-reads auction prices", "update": "https://ryna.io/wow/guide#upkeep" })
    }

    #[test]
    fn reads_the_api_s_support_and_fails_open() {
        let s = Support::from_value(&wire("reduced")).unwrap();
        assert_eq!(s.band, Band::Reduced);
        assert_eq!(s.perfect.as_deref(), Some("0.3.9"));
        assert_eq!(s.note.as_deref(), Some("0.3.4 mis-reads auction prices"));
        assert_eq!(
            s.update.as_deref(),
            Some("https://ryna.io/wow/guide#upkeep")
        );
        // An unknown band is current; no band is no judgment.
        assert_eq!(
            Support::from_value(&wire("sideways")).unwrap().band,
            Band::Current
        );
        assert_eq!(Support::from_value(&json!({ "version": "0.3.5" })), None);
        assert_eq!(Support::from_value(&json!("unsupported")), None);
        // Nulls and blanks are None; a non-https update is dropped; a long note is cut.
        let s = Support::from_value(&json!({ "band": "behind", "minimum": null, "note": " ",
            "update": "javascript:alert(1)", "perfect": "x".repeat(100) }))
        .unwrap();
        assert_eq!((s.minimum, s.note, s.update), (None, None, None));
        assert_eq!(s.perfect.unwrap().len(), 40);
        assert_eq!(
            Support::in_answer(&json!({ "host": "h", "support": wire("unsupported") }))
                .unwrap()
                .band,
            Band::Unsupported
        );
        assert_eq!(Support::in_answer(&json!({ "host": "h" })), None);
    }

    #[test]
    fn serialises_for_a_front_end() {
        let s = Support::from_value(&wire("unsupported")).unwrap();
        let v = serde_json::to_value(&s).unwrap();
        assert_eq!(v["band"], json!("unsupported"));
        assert_eq!(v["perfect"], json!("0.3.9"));
        assert_eq!(Band::Behind.as_str(), "behind");
        assert_eq!(Band::Reduced.sentence(), Some(REDUCED_SENTENCE));
        assert_eq!(Band::Behind.sentence(), None);
    }

    #[test]
    fn keeps_the_last_judgment_for_this_version_only() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(KnownSupport::load(dir.path()), None);
        assert_eq!(band(None), Band::Current);
        let s = Support::from_value(&wire("reduced")).unwrap();
        let (k, changed) = record(dir.path(), s.clone(), 100);
        assert!(changed, "from nothing (current) to reduced");
        assert_eq!(k.received_at, 100);
        assert_eq!(KnownSupport::load(dir.path()), Some(k.clone()));
        let (_, changed) = record(dir.path(), s.clone(), 160);
        assert!(!changed);
        // Contributions paused: said once, cleared when pooling comes back.
        assert!(note_pooled(dir.path(), false));
        assert!(!note_pooled(dir.path(), false));
        let (k, _) = record(dir.path(), s, 220);
        assert_eq!(k.pooled, Some(false), "kept across a new judgment");
        assert!(!note_pooled(dir.path(), true));
        assert!(note_pooled(dir.path(), false));
        // Kept by another version of the app: not this one's to show.
        let mut old = KnownSupport::load(dir.path()).unwrap();
        old.agent = "0.0.1".into();
        old.save(dir.path()).unwrap();
        assert_eq!(KnownSupport::load(dir.path()), None);
        // No temp files left behind; a corrupt file is no judgment.
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
        std::fs::write(dir.path().join(SUPPORT_FILE), "{ nope").unwrap();
        assert_eq!(KnownSupport::load(dir.path()), None);
        assert!(!note_pooled(dir.path(), false), "nothing to hang it on");
    }

    #[test]
    fn advice_while_unsupported_is_the_sentence() {
        let s = Support::from_value(&wire("unsupported")).unwrap();
        assert_eq!(advice(Fix::Update, "x", Some(&s)), UNSUPPORTED_SENTENCE);
        assert_eq!(advice(Fix::Update, "x", None), Fix::Update.message("x"));
        assert_eq!(
            advice(Fix::Network, "x", Some(&s)),
            "Couldn't reach the API"
        );
        let r = Support::from_value(&wire("reduced")).unwrap();
        assert_eq!(advice(Fix::Update, "x", Some(&r)), Fix::Update.message("x"));
    }

    #[test]
    fn compares_releases() {
        assert_eq!(older_than("0.3.8", "0.3.9"), Some(true));
        assert_eq!(older_than("0.3.10", "0.3.9"), Some(false));
        assert_eq!(older_than("v0.3.9", "0.3.9"), Some(false));
        assert_eq!(older_than("0.3", "0.3.0"), Some(false));
        assert_eq!(older_than("0.4.0-rc.1", "0.4.0"), Some(true));
        assert_eq!(older_than("0.4.0", "0.4.0-rc.1"), Some(false));
        assert_eq!(older_than("dev", "0.3.9"), None);
        assert_eq!(older_than("0.3.9", ""), None);
    }

    #[test]
    fn latest_is_filled_only_when_older() {
        let mut s = Support::from_value(&wire("behind")).unwrap();
        s.perfect = Some("999.0.0".into());
        assert_eq!(latest_agent(&s).as_deref(), Some("999.0.0"));
        assert_eq!(latest_addon(&s, Some("0.3.9")).as_deref(), Some("999.0.0"));
        assert_eq!(latest_addon(&s, None), None);
        s.perfect = Some(crate::AGENT_VERSION.into());
        assert_eq!(latest_agent(&s), None);
        assert_eq!(latest_addon(&s, Some(crate::AGENT_VERSION)), None);
        s.perfect = None;
        assert_eq!(latest_agent(&s), None);
        assert_eq!(latest_addon(&s, Some("0.0.1")), None);
    }
}
