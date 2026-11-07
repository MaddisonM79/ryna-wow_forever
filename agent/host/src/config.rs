//! The four constants the agent carries (design doc 2a): the IdP, the device client, the API and
//! the resource its tokens are for. Each can be overridden from the environment, for a local API.

/// `RYNA_ISSUER`.
pub const DEFAULT_ISSUER: &str = "https://auth.nyratech.dev";
/// `RYNA_API`.
pub const DEFAULT_API: &str = "https://api.ryna.io";
/// `RYNA_RESOURCE`. The audience of the agent's tokens (RFC 8707 resource indicator).
pub const DEFAULT_RESOURCE: &str = "https://api.ryna.io/wow/ingest";
/// The scope the API requires on the agent's routes.
pub const INGEST_SCOPE: &str = "wow:ingest";
/// The public device client registered for this agent at auth.nyratech.dev (named `ryna-wow` there,
/// 2026-11-06). Public by design (RFC 8628: no secret), so it can live here; `RYNA_CLIENT_ID`
/// overrides it for another IdP.
pub const DEFAULT_CLIENT_ID: &str = "lgiJjuqYkThXPGdHOUseZThNlwvFaNLe";
/// What `client_id` is when no client exists for the configured IdP; `login` refuses it.
pub const PLACEHOLDER_CLIENT_ID: &str = "ryna-device-client-NOT-ISSUED-YET";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub issuer: String,
    pub api: String,
    pub client_id: String,
    pub resource: String,
}

fn env(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

impl Config {
    /// Explicit values (the CLI's flags) first, then the environment, then the defaults.
    pub fn resolve(api: Option<String>, issuer: Option<String>) -> Config {
        let clean = |s: String| s.trim().trim_end_matches('/').to_string();
        Config {
            issuer: clean(
                issuer
                    .or_else(|| env("RYNA_ISSUER"))
                    .unwrap_or_else(|| DEFAULT_ISSUER.into()),
            ),
            api: clean(
                api.or_else(|| env("RYNA_API"))
                    .unwrap_or_else(|| DEFAULT_API.into()),
            ),
            client_id: env("RYNA_CLIENT_ID").unwrap_or_else(|| DEFAULT_CLIENT_ID.into()),
            resource: env("RYNA_RESOURCE").unwrap_or_else(|| DEFAULT_RESOURCE.into()),
        }
    }

    pub fn client_id_issued(&self) -> bool {
        self.client_id != PLACEHOLDER_CLIENT_ID
    }
}

/// A config for tests (this crate's, and with feature `testing` a core's).
#[cfg(any(test, feature = "testing"))]
#[doc(hidden)]
pub fn test_config() -> Config {
    Config {
        issuer: "https://idp.test".into(),
        api: "https://api.test".into(),
        client_id: "test-client".into(),
        resource: DEFAULT_RESOURCE.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_values_win_and_lose_their_trailing_slash() {
        let c = Config::resolve(
            Some("http://localhost:8797/".into()),
            Some("http://localhost:5174/".into()),
        );
        assert_eq!(c.api, "http://localhost:8797");
        assert_eq!(c.issuer, "http://localhost:5174");
        assert_eq!(c.resource, DEFAULT_RESOURCE);
    }

    #[test]
    fn placeholder_is_not_issued() {
        let mut c = test_config();
        assert!(c.client_id_issued());
        c.client_id = PLACEHOLDER_CLIENT_ID.into();
        assert!(!c.client_id_issued());
    }
}
