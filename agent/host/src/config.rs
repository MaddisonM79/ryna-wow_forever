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

/// An endpoint the agent won't talk to: the access and refresh tokens (and the device key's
/// signatures) would cross the network in the clear, or the value isn't a URL at all.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{name} is {value:?}, which {why}: use an https:// URL (http:// is taken only for localhost, 127.0.0.1 or [::1])")]
pub struct ConfigError {
    /// The environment variable (or the CLI flag beside it) the value came from.
    pub name: &'static str,
    pub value: String,
    pub why: &'static str,
}

/// Refuse anything but `https://`, or `http://` to a loopback host (local development).
fn checked(name: &'static str, value: String) -> Result<String, ConfigError> {
    let refuse = |why| ConfigError {
        name,
        value: value.clone(),
        why,
    };
    let url = reqwest::Url::parse(&value).map_err(|_| refuse("isn't a URL"))?;
    match url.scheme() {
        "https" if url.host_str().is_some() => Ok(value),
        "http" => match url.host_str() {
            Some("localhost" | "127.0.0.1" | "[::1]") => Ok(value),
            _ => Err(refuse("would send the sign-in tokens unencrypted")),
        },
        _ => Err(refuse("isn't an https:// URL")),
    }
}

impl Config {
    /// Explicit values (the CLI's flags) first, then the environment, then the defaults. The API and
    /// the issuer must be `https://` (or `http://` to localhost): anything else is refused, naming
    /// the variable, since the tokens would cross the network in the clear.
    pub fn resolve(api: Option<String>, issuer: Option<String>) -> Result<Config, ConfigError> {
        let clean = |s: String| s.trim().trim_end_matches('/').to_string();
        Ok(Config {
            issuer: checked(
                "RYNA_ISSUER (--issuer)",
                clean(
                    issuer
                        .or_else(|| env("RYNA_ISSUER"))
                        .unwrap_or_else(|| DEFAULT_ISSUER.into()),
                ),
            )?,
            api: checked(
                "RYNA_API (--api)",
                clean(
                    api.or_else(|| env("RYNA_API"))
                        .unwrap_or_else(|| DEFAULT_API.into()),
                ),
            )?,
            client_id: env("RYNA_CLIENT_ID").unwrap_or_else(|| DEFAULT_CLIENT_ID.into()),
            resource: env("RYNA_RESOURCE").unwrap_or_else(|| DEFAULT_RESOURCE.into()),
        })
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
        )
        .unwrap();
        assert_eq!(c.api, "http://localhost:8797");
        assert_eq!(c.issuer, "http://localhost:5174");
        assert_eq!(c.resource, DEFAULT_RESOURCE);
    }

    #[test]
    fn https_and_loopback_http_only() {
        let ok = |u: &str| Config::resolve(Some(u.into()), Some("https://idp.test".into()));
        for good in [
            "https://api.ryna.io",
            "https://api.ryna.io:8443/x",
            "http://localhost:8797",
            "http://127.0.0.1:8797/",
            "http://[::1]:8797",
            "HTTPS://API.RYNA.IO",
        ] {
            assert!(ok(good).is_ok(), "{good}");
        }
        for bad in [
            "http://api.ryna.io",
            "http://localhost.evil.test",
            "http://localhost@evil.test",
            "http://127.0.0.2",
            "http://10.0.0.1:8797",
            "ftp://api.ryna.io",
            "api.ryna.io",
            "https://",
            "file:///etc/passwd",
        ] {
            let e = ok(bad).unwrap_err();
            assert_eq!(e.name, "RYNA_API (--api)", "{bad}");
            assert!(e.to_string().starts_with("RYNA_API (--api) is "), "{e}");
        }
        let e = Config::resolve(
            Some("https://api.test".into()),
            Some("http://idp.example".into()),
        )
        .unwrap_err();
        assert_eq!(e.name, "RYNA_ISSUER (--issuer)");
        assert!(e.to_string().contains("unencrypted"), "{e}");
    }

    #[test]
    fn placeholder_is_not_issued() {
        let mut c = test_config();
        assert!(c.client_id_issued());
        c.client_id = PLACEHOLDER_CLIENT_ID.into();
        assert!(!c.client_id_issued());
    }
}
