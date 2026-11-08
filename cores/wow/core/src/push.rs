//! Posting to the API: the snapshot to `/v1/wow/agent/ingest`, the heartbeat to
//! `/v1/wow/agent/heartbeat`, and fights to `/v1/wow/agent/encounters` (then each one's digest and
//! raw lines with a PUT), as the person's own token (auth.rs); and a census upload to
//! `/v1/wow/census` with no token at all.
//!
//! Every POST also carries the device key's three headers (docs/wow.md section 13.1: the
//! submission ID, the public key, and the signature over the exact body bytes sent), and so do the
//! Settings mod's PUTs and DELETEs to `/v1/wow/settings/backups` (section 15); a fight's PUTs and
//! the GETs don't.
//!
//! Every answer is the envelope `{ ok: true, data } | { ok: false, error: { code, message, detail } }`.
//! A refusal is a 200 with `ok:false`; `SCHEMA_UNSUPPORTED` means the agent and addon need
//! updating. 5xx and no answer are retried with backoff (3 tries in all); 4xx never is, except
//! one resend with a fresh token after a 401.

use std::time::Duration;

use serde::Serialize;
use serde_json::Value;
use thiserror::Error;

use crate::auth::{Auth, AuthError};
use crate::census::{CensusUpload, CENSUS_LIMIT};
use crate::config::INGEST_SCOPE;
use crate::fix::Fix;
use crate::http::{Method, Request, Response, Transport};
use crate::identity::DeviceKey;
use crate::snapshot::Snapshot;
use crate::util::Clock;

pub const INGEST_PATH: &str = "/v1/wow/agent/ingest";
pub const HEARTBEAT_PATH: &str = "/v1/wow/agent/heartbeat";
pub const ENCOUNTERS_PATH: &str = "/v1/wow/agent/encounters";
pub const CENSUS_PATH: &str = "/v1/wow/census";
/// The Settings mod's backups (docs/wow.md section 15).
pub const SETTINGS_BACKUPS_PATH: &str = "/v1/wow/settings/backups";
/// A backup's JSON bodies (`POST /backups` with up to 2,000 files, `complete`).
pub const SETTINGS_JSON_LIMIT: usize = 1024 * 1024;
pub const TRIES: u32 = 3;
/// The API refuses bodies over these.
pub const INGEST_LIMIT: usize = 16 * 1024 * 1024;
pub const HEARTBEAT_LIMIT: usize = 64 * 1024;
/// One `POST /encounters` (and at most `ENCOUNTERS_PER_CALL` fights in it).
pub const ENCOUNTERS_LIMIT: usize = 8 * 1024 * 1024;
pub const ENCOUNTERS_PER_CALL: usize = 50;
/// A digest's gzipped bytes, and a fight's gzipped raw lines.
pub const DIGEST_LIMIT: usize = 32 * 1024 * 1024;
pub const RAW_LIMIT: usize = 64 * 1024 * 1024;

#[derive(Debug, Error)]
pub enum PushError {
    #[error("this agent isn't signed in")]
    NotSignedIn,
    #[error("{0}")]
    Auth(AuthError),
    #[error("this agent's sign-in wasn't granted {INGEST_SCOPE}")]
    MissingScope,
    #[error("the API doesn't accept this snapshot schema (it accepts {accepted:?}): update the agent and the addon")]
    SchemaUnsupported { accepted: Vec<i64> },
    #[error("the API refused it: {code}: {message}")]
    Refused { code: String, message: String },
    #[error("the API answered {status}: {body}")]
    Rejected { status: u16, body: String },
    #[error("the API is unreachable or failing after {TRIES} tries: {0}")]
    Unavailable(String),
    #[error("the API's answer wasn't the envelope ({status}): {body}")]
    NotAnEnvelope { status: u16, body: String },
    #[error("the body is {size} bytes, over the API's limit of {limit}")]
    TooLarge { size: usize, limit: usize },
    #[error("there's no device key to sign the census with")]
    NoDeviceKey,
}

impl PushError {
    /// Only a new sign-in fixes it; pushing the other accounts would fail the same way.
    pub fn needs_login(&self) -> bool {
        match self {
            PushError::NotSignedIn | PushError::MissingScope => true,
            PushError::Auth(e) => e.needs_login(),
            _ => false,
        }
    }

    /// Only a newer agent and addon fix it.
    pub fn needs_update(&self) -> bool {
        matches!(self, PushError::SchemaUnsupported { .. })
    }

    /// What fixes it.
    pub fn fix(&self) -> Fix {
        if self.needs_login() {
            Fix::SignIn
        } else if self.needs_update() {
            Fix::Update
        } else {
            match self {
                PushError::Unavailable(_) => Fix::Network,
                PushError::Auth(e) => e.fix(),
                _ => Fix::Other,
            }
        }
    }
}

impl From<AuthError> for PushError {
    fn from(e: AuthError) -> Self {
        PushError::Auth(e)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Heartbeat {
    pub agent: String,
    pub host: String,
    pub os: String,
    pub installs: Vec<HeartbeatInstall>,
    pub accounts: Vec<HeartbeatAccount>,
    pub combat_log: CombatLog,
    pub last_error: Option<String>,
    /// The device key's submission ID (32 hex), or null when the key couldn't be read.
    pub submission_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct HeartbeatInstall {
    pub path: String,
    pub flavor: Option<String>,
    /// The installed addon's `## Version:` (null when it isn't installed or doesn't say).
    pub addon: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HeartbeatAccount {
    pub key: String,
    pub label: String,
    pub saved_at: Option<i64>,
    pub pushed_at: Option<i64>,
    pub characters: Option<usize>,
}

/// The combat log's state on this computer (docs/wow.md section 9.3).
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CombatLog {
    /// A log was written to recently (logging is on and the game is playing).
    pub active: bool,
    /// Whether its lines carry the advanced block; null before any line said.
    pub advanced: Option<bool>,
    /// The newest log's file name.
    pub file: Option<String>,
    /// The newest event's time (unix seconds).
    pub last_event_at: Option<i64>,
    /// Fights read but not yet uploaded (queued, or still open).
    pub pending: usize,
}

/// Which of a fight's two bodies a PUT carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FightBody {
    Digest,
    Raw,
}

pub struct Api<'a> {
    origin: String,
    http: &'a dyn Transport,
    auth: &'a Auth<'a>,
    /// Signs every POST; None only when the keychain couldn't give the device key (the posts then
    /// go unsigned, which the API takes everywhere but `/census`).
    key: Option<&'a dyn DeviceKey>,
    clock: &'a dyn Clock,
}

/// Whether a call carries the person's token.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Bearer {
    Person,
    None,
}

impl<'a> Api<'a> {
    pub fn new(
        origin: &str,
        http: &'a dyn Transport,
        auth: &'a Auth<'a>,
        key: Option<&'a dyn DeviceKey>,
        clock: &'a dyn Clock,
    ) -> Self {
        Api {
            origin: origin.trim_end_matches('/').to_string(),
            http,
            auth,
            key,
            clock,
        }
    }

    pub fn auth(&self) -> &Auth<'a> {
        self.auth
    }

    /// The device key's submission ID (32 hex), when there is a key.
    pub fn submission_id(&self) -> Option<String> {
        self.key.map(|k| k.submission_id())
    }

    /// Post one census upload (docs/wow.md section 13.3), signed and with no token; returns the
    /// envelope's `data` (`{ submissionId, stored, repeated, pending }`).
    pub fn post_census(&self, upload: &CensusUpload) -> Result<Value, PushError> {
        if self.key.is_none() {
            return Err(PushError::NoDeviceKey);
        }
        self.send(
            Method::Post,
            CENSUS_PATH,
            "application/json",
            None,
            upload.body(),
            CENSUS_LIMIT,
            Duration::from_secs(60),
            Bearer::None,
            true,
        )
    }

    /// A Settings-mod call answered with the envelope: `GET`, `POST` (a JSON body, `{}` when none)
    /// or `DELETE` to `path`, as the person; every one but the GET signed. Returns `data`.
    pub fn settings_call(
        &self,
        method: Method,
        path: &str,
        body: Option<&Value>,
        timeout: Duration,
    ) -> Result<Value, PushError> {
        let bytes = match (method, body) {
            (Method::Get, _) => Vec::new(),
            (_, Some(b)) => serde_json::to_vec(b).expect("a value always serialises"),
            (Method::Post, None) => b"{}".to_vec(),
            (_, None) => Vec::new(),
        };
        self.send(
            method,
            path,
            "application/json",
            None,
            bytes,
            SETTINGS_JSON_LIMIT,
            timeout,
            Bearer::Person,
            method != Method::Get,
        )
    }

    /// `PUT` raw bytes (a backup's part) to `path`, as the person, signed over those bytes.
    pub fn settings_put(
        &self,
        path: &str,
        bytes: Vec<u8>,
        limit: usize,
        timeout: Duration,
    ) -> Result<Value, PushError> {
        self.send(
            Method::Put,
            path,
            "application/octet-stream",
            None,
            bytes,
            limit,
            timeout,
            Bearer::Person,
            true,
        )
    }

    /// `GET` raw bytes (a backup's ciphertext) from `path`, as the person. An answer in the
    /// envelope (a refusal) is that refusal.
    pub fn settings_download(&self, path: &str, timeout: Duration) -> Result<Vec<u8>, PushError> {
        let res = self.send_raw(
            Method::Get,
            path,
            "application/octet-stream",
            None,
            Vec::new(),
            usize::MAX,
            timeout,
            Bearer::Person,
            false,
        )?;
        let is_json = res.content_type.starts_with("application/json");
        if (200..300).contains(&res.status) && !is_json {
            return Ok(res.body);
        }
        read_envelope(&res)?;
        Err(PushError::NotAnEnvelope {
            status: res.status,
            body: "expected the backup's bytes, got JSON".into(),
        })
    }

    /// Post fights (`{ encounters: [...] }`, at most `ENCOUNTERS_PER_CALL`); returns the
    /// envelope's `data` (`{ encounters: [ { key, id, merged, sources } ] }`).
    pub fn encounters(&self, body: &Value) -> Result<Value, PushError> {
        let body = serde_json::to_vec(body).expect("a value always serialises");
        self.send_body(
            Method::Post,
            ENCOUNTERS_PATH,
            "application/json",
            None,
            body,
            ENCOUNTERS_LIMIT,
            Duration::from_secs(120),
        )
    }

    /// `PUT /encounters/:id/digest?key=` (or `/raw`) with already-gzipped bytes.
    pub fn fight_body(
        &self,
        which: FightBody,
        id: &str,
        key: &str,
        gzipped: Vec<u8>,
    ) -> Result<Value, PushError> {
        let (part, limit) = match which {
            FightBody::Digest => ("digest", DIGEST_LIMIT),
            FightBody::Raw => ("raw", RAW_LIMIT),
        };
        let path = format!(
            "{ENCOUNTERS_PATH}/{}/{part}?key={}",
            path_segment(id),
            path_segment(key)
        );
        self.send_body(
            Method::Put,
            &path,
            "application/octet-stream",
            Some("gzip"),
            gzipped,
            limit,
            Duration::from_secs(300),
        )
    }

    /// Post one snapshot; returns the envelope's `data` (snapshotId, account, counts).
    pub fn ingest(&self, snapshot: &Snapshot) -> Result<Value, PushError> {
        let body = serde_json::to_vec(snapshot).expect("a snapshot always serialises");
        self.post(INGEST_PATH, body, INGEST_LIMIT, Duration::from_secs(120))
    }

    pub fn heartbeat(&self, hb: &Heartbeat) -> Result<Value, PushError> {
        let body = serde_json::to_vec(hb).expect("a heartbeat always serialises");
        self.post(
            HEARTBEAT_PATH,
            body,
            HEARTBEAT_LIMIT,
            Duration::from_secs(30),
        )
    }

    fn token(&self) -> Result<String, PushError> {
        match self.auth.access_token()? {
            Some(t) => Ok(t),
            None => Err(PushError::NotSignedIn),
        }
    }

    fn post(
        &self,
        path: &str,
        body: Vec<u8>,
        limit: usize,
        timeout: Duration,
    ) -> Result<Value, PushError> {
        self.send_body(
            Method::Post,
            path,
            "application/json",
            None,
            body,
            limit,
            timeout,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn send_body(
        &self,
        method: Method,
        path: &str,
        content_type: &'static str,
        content_encoding: Option<&'static str>,
        body: Vec<u8>,
        limit: usize,
        timeout: Duration,
    ) -> Result<Value, PushError> {
        self.send(
            method,
            path,
            content_type,
            content_encoding,
            body,
            limit,
            timeout,
            Bearer::Person,
            method == Method::Post,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn send(
        &self,
        method: Method,
        path: &str,
        content_type: &'static str,
        content_encoding: Option<&'static str>,
        body: Vec<u8>,
        limit: usize,
        timeout: Duration,
        bearer: Bearer,
        signed: bool,
    ) -> Result<Value, PushError> {
        let res = self.send_raw(
            method,
            path,
            content_type,
            content_encoding,
            body,
            limit,
            timeout,
            bearer,
            signed,
        )?;
        read_envelope(&res)
    }

    /// Send with the retries (5xx and no answer, 3 tries; one fresh token after a 401) and return
    /// the answer that ended it, whatever its status.
    #[allow(clippy::too_many_arguments)]
    fn send_raw(
        &self,
        method: Method,
        path: &str,
        content_type: &'static str,
        content_encoding: Option<&'static str>,
        body: Vec<u8>,
        limit: usize,
        timeout: Duration,
        bearer: Bearer,
        signed: bool,
    ) -> Result<Response, PushError> {
        if body.len() > limit {
            return Err(PushError::TooLarge {
                size: body.len(),
                limit,
            });
        }
        let url = format!("{}{path}", self.origin);
        // Signed once, over the very bytes every try sends.
        let headers = match (signed, self.key) {
            (true, Some(key)) => key.headers(&body),
            _ => Vec::new(),
        };
        let mut last = String::new();
        let mut reauthed = false;
        let mut attempt = 0;
        while attempt < TRIES {
            if attempt > 0 {
                // 2 s, then 4 s.
                self.clock.sleep(Duration::from_secs(1 << attempt));
            }
            attempt += 1;
            let req = Request {
                method,
                url: url.clone(),
                content_type,
                content_encoding,
                body: body.clone(),
                bearer: match bearer {
                    Bearer::Person => Some(self.token()?),
                    Bearer::None => None,
                },
                headers: headers.clone(),
                timeout,
            };
            let res = match self.http.post(&req) {
                Ok(r) => r,
                Err(e) => {
                    last = e;
                    continue;
                }
            };
            if res.status == 401 && bearer == Bearer::Person && !reauthed {
                // The token was refused as unauthenticated: once more with a fresh one.
                reauthed = true;
                self.auth.forget_access_token();
                attempt -= 1;
                continue;
            }
            if res.status >= 500 {
                last = format!("HTTP {}: {}", res.status, res.excerpt());
                continue;
            }
            return Ok(res);
        }
        Err(PushError::Unavailable(last))
    }
}

/// A path or query part, percent-encoded (ids and keys are plain, but never trust that).
pub fn path_segment(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// The envelope's `data`, or why there isn't one.
fn read_envelope(res: &Response) -> Result<Value, PushError> {
    let envelope = res.json();
    let error = envelope
        .as_ref()
        .filter(|e| e.get("ok") == Some(&Value::Bool(false)))
        .and_then(|e| e.get("error"));
    if let Some(error) = error {
        let code = error
            .get("code")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        if code == "SCHEMA_UNSUPPORTED" {
            let accepted = error
                .pointer("/detail/accepted")
                .and_then(Value::as_array)
                .map(|a| a.iter().filter_map(Value::as_i64).collect())
                .unwrap_or_default();
            return Err(PushError::SchemaUnsupported { accepted });
        }
        if res.status == 403
            && error.pointer("/detail/scope").and_then(Value::as_str) == Some(INGEST_SCOPE)
        {
            return Err(PushError::MissingScope);
        }
        let message = error
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        if (200..300).contains(&res.status) {
            return Err(PushError::Refused { code, message });
        }
        return Err(PushError::Rejected {
            status: res.status,
            body: format!("{code}: {message}"),
        });
    }
    if !(200..300).contains(&res.status) {
        return Err(PushError::Rejected {
            status: res.status,
            body: res.excerpt(),
        });
    }
    match envelope {
        Some(e) if e.get("ok") == Some(&Value::Bool(true)) => {
            Ok(e.get("data").cloned().unwrap_or(Value::Null))
        }
        _ => Err(PushError::NotAnEnvelope {
            status: res.status,
            body: res.excerpt(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::testing::MemoryStore;
    use crate::census::{CensusEvent, Observer, CENSUS_SCHEMA};
    use crate::config::test_config;
    use crate::http::testing::ScriptedTransport;
    use crate::identity::testing::test_key;
    use crate::identity::{verify, KEY_HEADER, SIGNATURE_HEADER, SUBMISSION_HEADER};
    use crate::util::testing::FakeClock;
    use serde_json::json;

    const NOW: i64 = 1_800_000_000;

    fn hb() -> Heartbeat {
        Heartbeat {
            agent: "0.1.0".into(),
            host: "test-host".into(),
            os: "macos".into(),
            installs: vec![HeartbeatInstall {
                path: "/Applications/World of Warcraft/_forever_".into(),
                flavor: Some("wowf".into()),
                addon: Some("0.1.0".into()),
            }],
            accounts: vec![],
            combat_log: CombatLog::default(),
            last_error: None,
            submission_id: Some(test_key().submission_id()),
        }
    }

    fn run(
        http: &ScriptedTransport,
        store: &MemoryStore,
        clock: &FakeClock,
    ) -> Result<Value, PushError> {
        let config = test_config();
        let auth = Auth::new(&config, http, store, clock);
        let key = test_key();
        Api::new(&config.api, http, &auth, Some(&key), clock).heartbeat(&hb())
    }

    #[test]
    fn heartbeat_shape() {
        let v = serde_json::to_value(hb()).unwrap();
        assert_eq!(
            v,
            json!({ "agent": "0.1.0", "host": "test-host", "os": "macos",
                    "installs": [ { "path": "/Applications/World of Warcraft/_forever_", "flavor": "wowf", "addon": "0.1.0" } ],
                    "accounts": [], "combatLog": { "active": false, "advanced": null, "file": null, "lastEventAt": null, "pending": 0 }, "lastError": null,
                    "submissionId": "21fe31dfa154a261626bf854046fd227" })
        );
    }

    #[test]
    fn success_sends_bearer_and_json() {
        let http = ScriptedTransport::new();
        http.json(200, json!({ "ok": true, "data": { "seen": 1 } }));
        let store = MemoryStore::with_access("tok", NOW + 3600);
        let clock = FakeClock::at(NOW);
        assert_eq!(run(&http, &store, &clock).unwrap(), json!({ "seen": 1 }));
        let req = http.requests.borrow()[0].clone();
        assert_eq!(req.url, "https://api.test/v1/wow/agent/heartbeat");
        assert_eq!(req.bearer.as_deref(), Some("tok"));
        assert_eq!(req.content_type, "application/json");
    }

    /// Request `i`'s three headers verify over the body it sent, for the test key.
    fn assert_signed(http: &ScriptedTransport, i: usize) {
        let body = http.requests.borrow()[i].body.clone();
        assert_eq!(
            http.header(i, SUBMISSION_HEADER),
            Some(test_key().submission_id())
        );
        let pk = http.header(i, KEY_HEADER).unwrap();
        assert_eq!(pk, test_key().public_key_b64());
        let sig = http.header(i, SIGNATURE_HEADER).unwrap();
        assert_eq!(sig.len(), 88);
        assert!(
            verify(&pk, &body, &sig),
            "request {i} isn't signed over its body"
        );
    }

    #[test]
    fn every_post_is_signed_over_the_bytes_it_sends_each_try() {
        let http = ScriptedTransport::new();
        http.text(502, "bad gateway")
            .json(200, json!({ "ok": true, "data": null }));
        let store = MemoryStore::with_access("tok", NOW + 3600);
        let clock = FakeClock::at(NOW);
        run(&http, &store, &clock).unwrap();
        assert_signed(&http, 0);
        assert_signed(&http, 1);
    }

    #[test]
    fn without_a_key_posts_go_unsigned() {
        let http = ScriptedTransport::new();
        http.json(200, json!({ "ok": true, "data": null }));
        let store = MemoryStore::with_access("tok", NOW + 3600);
        let clock = FakeClock::at(NOW);
        let config = test_config();
        let auth = Auth::new(&config, &http, &store, &clock);
        let api = Api::new(&config.api, &http, &auth, None, &clock);
        assert_eq!(api.submission_id(), None);
        api.heartbeat(&hb()).unwrap();
        assert!(http.requests.borrow()[0].headers.is_empty());
    }

    fn upload() -> CensusUpload {
        CensusUpload {
            schema: CENSUS_SCHEMA,
            flavor: "wowf".into(),
            agent: crate::user_agent(),
            addon: "0.3.2".into(),
            sent_at: NOW,
            events: vec![CensusEvent {
                key: crate::census::event_key("Player-1-1", "s", 1),
                at: NOW - 60,
                observer: Observer {
                    realm_id: Some(1),
                    faction: Some("Horde".into()),
                },
                seen: json!({ "fullName": "Kesra-Duskmantle", "source": "who" })
                    .as_object()
                    .unwrap()
                    .clone(),
            }],
        }
    }

    #[test]
    fn a_census_upload_is_signed_and_carries_no_token() {
        let http = ScriptedTransport::new();
        http.json(200, json!({ "ok": true, "data": { "submissionId": test_key().submission_id(), "stored": 1, "repeated": 0, "pending": true } }));
        // Not signed in at all: the census needs no sign-in.
        let store = MemoryStore::default();
        let clock = FakeClock::at(NOW);
        let config = test_config();
        let auth = Auth::new(&config, &http, &store, &clock);
        let key = test_key();
        let api = Api::new(&config.api, &http, &auth, Some(&key), &clock);
        let data = api.post_census(&upload()).unwrap();
        assert_eq!(data["stored"], json!(1));
        let req = http.requests.borrow()[0].clone();
        assert_eq!(req.url, "https://api.test/v1/wow/census");
        assert_eq!(req.method, Method::Post);
        assert_eq!(req.bearer, None);
        assert_eq!(req.body, upload().body());
        assert_signed(&http, 0);

        // No key, nothing sent.
        let http = ScriptedTransport::new();
        let api = Api::new(&config.api, &http, &auth, None, &clock);
        assert!(matches!(
            api.post_census(&upload()),
            Err(PushError::NoDeviceKey)
        ));
        assert!(http.requests.borrow().is_empty());
    }

    #[test]
    fn a_refused_census_signature_is_reported_and_not_retried() {
        let http = ScriptedTransport::new();
        http.json(
            400,
            json!({ "ok": false, "error": { "code": "BAD_SIGNATURE", "message": "no" } }),
        );
        let store = MemoryStore::default();
        let clock = FakeClock::at(NOW);
        let config = test_config();
        let auth = Auth::new(&config, &http, &store, &clock);
        let key = test_key();
        let api = Api::new(&config.api, &http, &auth, Some(&key), &clock);
        let e = api.post_census(&upload()).unwrap_err();
        assert!(e.to_string().contains("BAD_SIGNATURE"), "{e}");
        assert_eq!(http.requests.borrow().len(), 1);
    }

    #[test]
    fn retries_5xx_and_network_errors_then_succeeds() {
        let http = ScriptedTransport::new();
        http.text(502, "bad gateway")
            .fail("connection refused")
            .json(200, json!({ "ok": true, "data": null }));
        let store = MemoryStore::with_access("tok", NOW + 3600);
        let clock = FakeClock::at(NOW);
        run(&http, &store, &clock).unwrap();
        let sleeps: Vec<u64> = clock.sleeps.borrow().iter().map(|d| d.as_secs()).collect();
        assert_eq!(sleeps, vec![2, 4]);
    }

    #[test]
    fn gives_up_after_three_tries() {
        let http = ScriptedTransport::new();
        http.text(500, "a").text(503, "b").fail("c");
        let store = MemoryStore::with_access("tok", NOW + 3600);
        let clock = FakeClock::at(NOW);
        let e = run(&http, &store, &clock).unwrap_err();
        assert!(matches!(&e, PushError::Unavailable(m) if m == "c"), "{e}");
        assert_eq!(e.fix(), Fix::Network);
        assert_eq!(http.requests.borrow().len(), 3);
    }

    #[test]
    fn never_retries_4xx() {
        let http = ScriptedTransport::new();
        http.json(
            400,
            json!({ "ok": false, "error": { "code": "BAD_REQUEST", "message": "not the shape" } }),
        );
        let store = MemoryStore::with_access("tok", NOW + 3600);
        let clock = FakeClock::at(NOW);
        let e = run(&http, &store, &clock).unwrap_err();
        assert!(matches!(&e, PushError::Rejected { status: 400, .. }), "{e}");
        assert!(e.to_string().contains("not the shape"));
        assert_eq!(http.requests.borrow().len(), 1);
        assert!(clock.sleeps.borrow().is_empty());
    }

    #[test]
    fn schema_unsupported_means_update() {
        let http = ScriptedTransport::new();
        http.json(200, json!({ "ok": false, "error": { "code": "SCHEMA_UNSUPPORTED", "message": "no", "detail": { "accepted": [1] } } }));
        let store = MemoryStore::with_access("tok", NOW + 3600);
        let clock = FakeClock::at(NOW);
        let e = run(&http, &store, &clock).unwrap_err();
        assert!(e.needs_update());
        assert!(matches!(&e, PushError::SchemaUnsupported { accepted } if accepted == &vec![1]));
        assert!(e.to_string().contains("update the agent"));
    }

    #[test]
    fn other_refusals_are_reported() {
        let http = ScriptedTransport::new();
        http.json(
            200,
            json!({ "ok": false, "error": { "code": "FORBIDDEN", "message": "no app:wow:use" } }),
        );
        let store = MemoryStore::with_access("tok", NOW + 3600);
        let clock = FakeClock::at(NOW);
        let e = run(&http, &store, &clock).unwrap_err();
        assert_eq!(
            e.to_string(),
            "the API refused it: FORBIDDEN: no app:wow:use"
        );
        assert!(!e.needs_login() && !e.needs_update());
    }

    #[test]
    fn a_401_refreshes_once() {
        let http = ScriptedTransport::new();
        http.text(401, "")
            .json(
                200,
                json!({ "access_token": "fresh", "refresh_token": "rt-2" }),
            )
            .json(200, json!({ "ok": true, "data": 1 }));
        let store = MemoryStore::with_access("stale", NOW + 3600);
        let clock = FakeClock::at(NOW);
        run(&http, &store, &clock).unwrap();
        let reqs = http.requests.borrow();
        assert_eq!(reqs[0].bearer.as_deref(), Some("stale"));
        assert_eq!(reqs[1].url, "https://idp.test/api/auth/oauth2/token");
        assert_eq!(reqs[2].bearer.as_deref(), Some("fresh"));
        drop(reqs);

        // A second 401 is the answer.
        let http = ScriptedTransport::new();
        http.text(401, "")
            .json(
                200,
                json!({ "access_token": "fresh", "refresh_token": "rt-3" }),
            )
            .text(401, "no");
        let e = run(&http, &store, &clock).unwrap_err();
        assert!(matches!(e, PushError::Rejected { status: 401, .. }), "{e}");
    }

    #[test]
    fn missing_scope_needs_login() {
        let http = ScriptedTransport::new();
        http.json(403, json!({ "ok": false, "error": { "code": "FORBIDDEN", "message": "scope", "detail": { "scope": "wow:ingest" } } }));
        let store = MemoryStore::with_access("tok", NOW + 3600);
        let clock = FakeClock::at(NOW);
        assert!(run(&http, &store, &clock).unwrap_err().needs_login());
    }

    #[test]
    fn not_signed_in_sends_nothing() {
        let http = ScriptedTransport::new();
        let store = MemoryStore::default();
        let clock = FakeClock::at(NOW);
        let e = run(&http, &store, &clock).unwrap_err();
        assert!(matches!(e, PushError::NotSignedIn) && e.needs_login());
        assert_eq!(e.fix(), Fix::SignIn);
        assert!(!e.to_string().contains("ryna"), "{e}");
        assert!(http.requests.borrow().is_empty());
    }

    #[test]
    fn a_login_page_is_not_an_envelope() {
        let http = ScriptedTransport::new();
        http.text(200, "<html>sign in</html>");
        let store = MemoryStore::with_access("tok", NOW + 3600);
        let clock = FakeClock::at(NOW);
        assert!(matches!(
            run(&http, &store, &clock),
            Err(PushError::NotAnEnvelope { .. })
        ));
    }
}
