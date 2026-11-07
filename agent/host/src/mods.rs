//! A core's mods (docs/ryna-agent-cores-mods.md section 6): the manifest each one declares itself
//! with (`mods/<core>/<id>/mod.json`, [`ModManifest`]) and the version requirement it names for its
//! core ([`VersionReq`]).
//!
//! What a mod *is* depends on its core (`kind`: WoW's is `addon`, an addon folder installed beside
//! the core's own), so this module only reads and checks the manifest; the core decides which kinds
//! it takes and how one is installed. Stage 2 has one source, `bundled` (the mod ships inside the
//! agent and versions with it); `git` is modelled, checked and refused by every core until stage 6.

use std::cmp::Ordering;
use std::fmt;

use serde::{Deserialize, Serialize};

/// `mods/<core>/<id>/mod.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModManifest {
    /// Short, lowercase: `chronicle`.
    pub id: String,
    /// The core it belongs to (`wow`).
    pub core: String,
    pub name: String,
    /// The mod's version (`x.y.z`); a bundled mod's is the agent's.
    pub version: String,
    /// One sentence: what it does.
    pub summary: String,
    /// What it is, in its core's terms (WoW: `addon`).
    pub kind: String,
    /// It does something in the game for the player (presses a button, automates): said every time.
    #[serde(default)]
    pub acts_in_game: bool,
    pub source: ModSource,
    #[serde(default)]
    pub requires: Requires,
    /// On until the person turns it off (false: off until they turn it on).
    #[serde(default)]
    pub default_on: bool,
    /// For an `addon` mod: the addon folders it installs (`Ryna_Chronicle`), each beside the core's.
    #[serde(default)]
    pub addons: Vec<String>,
}

/// Where a mod's files come from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ModSource {
    /// Inside the agent, versioned with it.
    Bundled,
    /// Fetched from a repository on the allow-list, by pinned tag, checked against `sha256` before
    /// anything is written (stage 6; no core installs one yet).
    Git {
        repo: String,
        tag: String,
        sha256: String,
    },
}

/// What a mod needs.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Requires {
    /// The core versions it works with (`>=0.3.0`, `^0.3`, `>=0.3.0, <0.5.0`); none: any.
    #[serde(default)]
    pub core: Option<String>,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ModManifestError {
    #[error("the mod manifest isn't valid JSON for one: {0}")]
    Json(String),
    #[error("the mod manifest's {0}")]
    Invalid(String),
}

fn is_id(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

impl ModManifest {
    /// Parse and check a `mod.json`.
    pub fn parse(json: &str) -> Result<ModManifest, ModManifestError> {
        let m: ModManifest =
            serde_json::from_str(json).map_err(|e| ModManifestError::Json(e.to_string()))?;
        m.check()?;
        Ok(m)
    }

    fn check(&self) -> Result<(), ModManifestError> {
        let bad = |why: String| Err(ModManifestError::Invalid(why));
        if !is_id(&self.id) {
            return bad("id must be lowercase letters, digits and dashes".into());
        }
        if !is_id(&self.core) {
            return bad("core must be a core's id".into());
        }
        if self.name.trim().is_empty() {
            return bad("name is empty".into());
        }
        if self.summary.trim().is_empty() {
            return bad("summary is empty".into());
        }
        if Version::parse(&self.version).is_none() {
            return bad("version isn't x.y.z".into());
        }
        if self.kind.trim().is_empty() {
            return bad("kind is empty".into());
        }
        if let ModSource::Git { repo, tag, sha256 } = &self.source {
            if !repo.starts_with("https://") {
                return bad("git source's repo isn't an https URL".into());
            }
            if tag.trim().is_empty() {
                return bad("git source names no tag".into());
            }
            if sha256.len() != 64 || !sha256.chars().all(|c| c.is_ascii_hexdigit()) {
                return bad("git source's sha256 isn't 64 hex digits".into());
            }
        }
        if let Some(req) = &self.requires.core {
            if let Err(e) = VersionReq::parse(req) {
                return bad(format!("requires.core: {e}"));
            }
        }
        for folder in &self.addons {
            let plain = !folder.is_empty()
                && folder != "."
                && folder != ".."
                && folder
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
            if !plain {
                return bad(format!("addon folder {folder:?} isn't a plain folder name"));
            }
        }
        Ok(())
    }

    /// Whether the core at `core_version` is one this mod works with (`requires.core`; none: any).
    /// A version that can't be read satisfies nothing.
    pub fn works_with_core(&self, core_version: &str) -> bool {
        let Some(have) = Version::parse(core_version) else {
            return false;
        };
        match &self.requires.core {
            None => true,
            Some(req) => VersionReq::parse(req).is_ok_and(|r| r.matches(&have)),
        }
    }
}

/// `x.y.z` (a `-pre` suffix is read and ignored for ordering: `0.3.0-rc1` counts as `0.3.0`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version(pub u64, pub u64, pub u64);

impl Version {
    pub fn parse(s: &str) -> Option<Version> {
        let core = s.trim().split(['-', '+']).next()?;
        let mut parts = core.split('.').map(|p| p.parse::<u64>().ok());
        let v = Version(parts.next()??, parts.next()??, parts.next()??);
        parts.next().is_none().then_some(v)
    }

    /// `x`, `x.y` or `x.y.z`, the missing parts 0, and how many were given.
    fn parse_partial(s: &str) -> Option<(Version, usize)> {
        let core = s.trim().split(['-', '+']).next()?;
        let parts: Vec<u64> = core
            .split('.')
            .map(|p| p.parse::<u64>().ok())
            .collect::<Option<_>>()?;
        if parts.is_empty() || parts.len() > 3 {
            return None;
        }
        let at = |i: usize| parts.get(i).copied().unwrap_or(0);
        Some((Version(at(0), at(1), at(2)), parts.len()))
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.0, self.1, self.2)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Op {
    Ge,
    Gt,
    Le,
    Lt,
    Eq,
}

/// A version requirement: comparators that must all hold, separated by commas or spaces. Each is
/// `>=`, `>`, `<=`, `<` or `=` and a version, a bare version (exactly it), `^x.y.z` (the same
/// leftmost non-zero part: `^0.3.1` is `>=0.3.1, <0.4.0`) or `~x.y.z` (the same minor).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionReq(Vec<(Op, Version)>);

impl VersionReq {
    pub fn parse(s: &str) -> Result<VersionReq, String> {
        let mut out = Vec::new();
        let text = s.replace(',', " ");
        let mut words = text.split_whitespace().peekable();
        if words.peek().is_none() {
            return Err("is empty".into());
        }
        while let Some(word) = words.next() {
            // ">= 0.3.0" (the operator alone) takes the next word as its version.
            let joined;
            let word = if [">=", ">", "<=", "<", "=", "^", "~"].contains(&word) {
                joined = format!("{word}{}", words.next().unwrap_or(""));
                joined.as_str()
            } else {
                word
            };
            let (op, rest) = [
                (">=", Some(Op::Ge)),
                ("<=", Some(Op::Le)),
                (">", Some(Op::Gt)),
                ("<", Some(Op::Lt)),
                ("=", Some(Op::Eq)),
                ("^", None),
                ("~", None),
            ]
            .iter()
            .find_map(|(p, op)| word.strip_prefix(p).map(|r| ((*p, *op), r)))
            .unwrap_or((("", Some(Op::Eq)), word));
            let (v, given) =
                Version::parse_partial(rest).ok_or_else(|| format!("{word:?} isn't a version"))?;
            match op {
                ("^", _) => {
                    let upper = if v.0 > 0 || given == 1 {
                        Version(v.0 + 1, 0, 0)
                    } else if v.1 > 0 || given == 2 {
                        Version(0, v.1 + 1, 0)
                    } else {
                        Version(0, 0, v.2 + 1)
                    };
                    out.push((Op::Ge, v));
                    out.push((Op::Lt, upper));
                }
                ("~", _) => {
                    let upper = if given == 1 {
                        Version(v.0 + 1, 0, 0)
                    } else {
                        Version(v.0, v.1 + 1, 0)
                    };
                    out.push((Op::Ge, v));
                    out.push((Op::Lt, upper));
                }
                (_, Some(op)) => out.push((op, v)),
                (_, None) => unreachable!("every prefix without an Op is ^ or ~"),
            }
        }
        Ok(VersionReq(out))
    }

    pub fn matches(&self, v: &Version) -> bool {
        self.0.iter().all(|(op, want)| {
            let o = v.cmp(want);
            match op {
                Op::Ge => o != Ordering::Less,
                Op::Gt => o == Ordering::Greater,
                Op::Le => o != Ordering::Greater,
                Op::Lt => o == Ordering::Less,
                Op::Eq => o == Ordering::Equal,
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn chronicle_like() -> serde_json::Value {
        json!({
            "id": "chronicle",
            "core": "wow",
            "name": "Chronicle",
            "version": "0.3.0",
            "summary": "Everything recorded about a character.",
            "kind": "addon",
            "actsInGame": false,
            "source": { "kind": "bundled" },
            "requires": { "core": ">=0.3.0" },
            "defaultOn": true,
            "addons": ["Ryna_Chronicle"]
        })
    }

    #[test]
    fn a_manifest_parses_with_every_field() {
        let m = ModManifest::parse(&chronicle_like().to_string()).unwrap();
        assert_eq!((m.id.as_str(), m.core.as_str()), ("chronicle", "wow"));
        assert_eq!(m.source, ModSource::Bundled);
        assert!(m.default_on && !m.acts_in_game);
        assert_eq!(m.addons, ["Ryna_Chronicle"]);
        assert_eq!(m.requires.core.as_deref(), Some(">=0.3.0"));
    }

    #[test]
    fn the_optional_parts_default_off_and_a_git_source_parses() {
        let mut v = chronicle_like();
        for k in ["actsInGame", "requires", "defaultOn", "addons"] {
            v.as_object_mut().unwrap().remove(k);
        }
        v["source"] = json!({ "kind": "git", "repo": "https://github.com/x/y", "tag": "v0.1.0", "sha256": "a".repeat(64) });
        let m = ModManifest::parse(&v.to_string()).unwrap();
        assert!(!m.default_on && !m.acts_in_game && m.addons.is_empty());
        assert_eq!(m.requires, Requires::default());
        assert!(matches!(m.source, ModSource::Git { .. }));
        assert!(m.works_with_core("0.0.1"), "no requirement: any core");
    }

    #[test]
    fn a_bad_manifest_is_refused() {
        assert!(matches!(
            ModManifest::parse("{"),
            Err(ModManifestError::Json(_))
        ));
        let refused = |k: &str, val: serde_json::Value| {
            let mut v = chronicle_like();
            v[k] = val;
            ModManifest::parse(&v.to_string()).unwrap_err().to_string()
        };
        assert!(refused("id", json!("Chronicle")).contains("id"));
        assert!(refused("core", json!("")).contains("core"));
        assert!(refused("name", json!(" ")).contains("name"));
        assert!(refused("summary", json!("")).contains("summary"));
        assert!(refused("version", json!("0.3")).contains("version"));
        assert!(refused("kind", json!("")).contains("kind"));
        assert!(refused("requires", json!({ "core": ">=zero" })).contains("requires.core"));
        assert!(refused("requires", json!({ "core": "" })).contains("requires.core"));
        assert!(refused("addons", json!(["../Ryna"])).contains("addon folder"));
        assert!(refused("addons", json!(["Ryna/Chronicle"])).contains("addon folder"));
        assert!(refused(
            "source",
            json!({ "kind": "git", "repo": "http://x", "tag": "v1", "sha256": "a".repeat(64) })
        )
        .contains("https"));
        assert!(refused(
            "source",
            json!({ "kind": "git", "repo": "https://x", "tag": "v1", "sha256": "abc" })
        )
        .contains("sha256"));
        // A source kind nobody knows isn't a manifest at all.
        let mut v = chronicle_like();
        v["source"] = json!({ "kind": "ftp" });
        assert!(matches!(
            ModManifest::parse(&v.to_string()),
            Err(ModManifestError::Json(_))
        ));
    }

    #[test]
    fn requires_core_is_checked_as_a_version_range() {
        let m = ModManifest::parse(&chronicle_like().to_string()).unwrap();
        assert!(m.works_with_core("0.3.0"));
        assert!(m.works_with_core("0.10.0"), "not as strings");
        assert!(m.works_with_core("1.0.0"));
        assert!(!m.works_with_core("0.2.6"));
        assert!(!m.works_with_core("unknown"));

        let req = |s: &str| VersionReq::parse(s).unwrap();
        let v = |s: &str| Version::parse(s).unwrap();
        assert!(req(">=0.3.0, <0.5.0").matches(&v("0.4.9")));
        assert!(!req(">=0.3.0, <0.5.0").matches(&v("0.5.0")));
        assert!(req(">= 0.3.0 < 0.5").matches(&v("0.3.0")));
        assert!(req("0.3.1").matches(&v("0.3.1")) && !req("0.3.1").matches(&v("0.3.2")));
        assert!(req("^0.3.1").matches(&v("0.3.9")) && !req("^0.3.1").matches(&v("0.4.0")));
        assert!(req("^1.2").matches(&v("1.9.0")) && !req("^1.2").matches(&v("2.0.0")));
        assert!(req("^0.0.3").matches(&v("0.0.3")) && !req("^0.0.3").matches(&v("0.0.4")));
        assert!(req("~0.3.1").matches(&v("0.3.5")) && !req("~0.3.1").matches(&v("0.4.0")));
        assert!(req(">0.3.0").matches(&v("0.3.1")) && !req(">0.3.0").matches(&v("0.3.0")));
        assert!(req("<=0.3.0").matches(&v("0.3.0-rc1")));
        assert!(VersionReq::parse("").is_err());
        assert!(VersionReq::parse(">=").is_err());
        assert!(VersionReq::parse("0.3.0.1").is_err());
        assert_eq!(Version::parse("0.3.0-rc1"), Some(Version(0, 3, 0)));
        assert_eq!(Version::parse("0.3"), None);
        assert_eq!(v("0.10.0").to_string(), "0.10.0");
    }
}
