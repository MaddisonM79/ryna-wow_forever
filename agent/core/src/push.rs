//! Posting to the API: the snapshot to `/v1/wow/agent/ingest` and the heartbeat to
//! `/v1/wow/agent/heartbeat`, as the person's own token (auth.rs).
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
use crate::config::INGEST_SCOPE;
use crate::http::{Request, Response, Transport};
use crate::snapshot::Snapshot;
use crate::util::Clock;

pub const INGEST_PATH: &str = "/v1/wow/agent/ingest";
pub const HEARTBEAT_PATH: &str = "/v1/wow/agent/heartbeat";
pub const TRIES: u32 = 3;
/// The API refuses bodies over these.
pub const INGEST_LIMIT: usize = 16 * 1024 * 1024;
pub const HEARTBEAT_LIMIT: usize = 64 * 1024;

#[derive(Debug, Error)]
pub enum PushError {
    #[error("this agent isn't signed in: run `ryna-wow login`")]
    NotSignedIn,
    #[error("{0}")]
    Auth(AuthError),
    #[error("this agent's sign-in wasn't granted {INGEST_SCOPE}: run `ryna-wow login` again")]
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
}

impl PushError {
    /// Only `ryna-wow login` fixes it; pushing the other accounts would fail the same way.
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
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct HeartbeatInstall {
    pub path: String,
    pub flavor: Option<String>,
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

/// Stage 2 fills these; until then the log is never read.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct CombatLog {
    pub active: bool,
    pub advanced: Option<bool>,
}

pub struct Api<'a> {
    origin: String,
    http: &'a dyn Transport,
    auth: &'a Auth<'a>,
    clock: &'a dyn Clock,
}

impl<'a> Api<'a> {
    pub fn new(
        origin: &str,
        http: &'a dyn Transport,
        auth: &'a Auth<'a>,
        clock: &'a dyn Clock,
    ) -> Self {
        Api {
            origin: origin.trim_end_matches('/').to_string(),
            http,
            auth,
            clock,
        }
    }

    pub fn auth(&self) -> &Auth<'a> {
        self.auth
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

    fn send(
        &self,
        url: &str,
        body: &[u8],
        timeout: Duration,
        token: String,
    ) -> Result<Response, String> {
        self.http.post(&Request {
            url: url.to_string(),
            content_type: "application/json",
            body: body.to_vec(),
            bearer: Some(token),
            timeout,
        })
    }

    fn post(
        &self,
        path: &str,
        body: Vec<u8>,
        limit: usize,
        timeout: Duration,
    ) -> Result<Value, PushError> {
        if body.len() > limit {
            return Err(PushError::TooLarge {
                size: body.len(),
                limit,
            });
        }
        let url = format!("{}{path}", self.origin);
        let mut last = String::new();
        let mut reauthed = false;
        let mut attempt = 0;
        while attempt < TRIES {
            if attempt > 0 {
                // 2 s, then 4 s.
                self.clock.sleep(Duration::from_secs(1 << attempt));
            }
            attempt += 1;
            let res = match self.send(&url, &body, timeout, self.token()?) {
                Ok(r) => r,
                Err(e) => {
                    last = e;
                    continue;
                }
            };
            if res.status == 401 && !reauthed {
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
            return read_envelope(&res);
        }
        Err(PushError::Unavailable(last))
    }
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
    use crate::config::test_config;
    use crate::http::testing::ScriptedTransport;
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
            }],
            accounts: vec![],
            combat_log: CombatLog::default(),
            last_error: None,
        }
    }

    fn run(
        http: &ScriptedTransport,
        store: &MemoryStore,
        clock: &FakeClock,
    ) -> Result<Value, PushError> {
        let config = test_config();
        let auth = Auth::new(&config, http, store, clock);
        Api::new(&config.api, http, &auth, clock).heartbeat(&hb())
    }

    #[test]
    fn heartbeat_shape() {
        let v = serde_json::to_value(hb()).unwrap();
        assert_eq!(
            v,
            json!({ "agent": "0.1.0", "host": "test-host", "os": "macos",
                    "installs": [ { "path": "/Applications/World of Warcraft/_forever_", "flavor": "wowf" } ],
                    "accounts": [], "combatLog": { "active": false, "advanced": null }, "lastError": null })
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
