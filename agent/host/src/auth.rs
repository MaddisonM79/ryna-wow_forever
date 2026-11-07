//! Signing the agent in as its person: the OAuth device grant (RFC 8628) at the IdP, the same
//! flow as palworld-agent's signin.ts, for this agent's client and resource.
//!
//! `start_device_login` asks for a code (a `DeviceLogin` to show), and `poll_device_login` polls
//! while the person allows it, until it's approved, refused, expired or cancelled; `login` is the
//! two in one call. The refresh token goes into the OS keychain (service `ryna`), never a
//! file. A run trades it for a short-lived access token (aud the resource, sub the person).
//! Refresh tokens rotate and are good for one use, and the IdP revokes the whole chain if a spent
//! one comes back after its replay window (30 s), so:
//! - the rotated token is stored at once, with retries;
//! - the access token is kept in the keychain too, so runs minutes apart refresh once per token
//!   lifetime rather than every run;
//! - a refresh that got no answer is sent again straight away with the same token, inside the
//!   replay window, where the IdP hands back the answer that was lost.

use std::cell::{Cell, RefCell};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::cancel::CancelToken;
use crate::config::{Config, INGEST_SCOPE};
use crate::fix::Fix;
use crate::http::{form_encode, HttpTransport, Request, Transport};
use crate::util::{Clock, SystemClock};

/// The keychain (credential store) service the tokens are kept under: the agent's name.
pub const KEYRING_SERVICE: &str = "ryna";
const DEVICE_CODE_PATH: &str = "/api/auth/device/code";
/// The IdP's token endpoint, as palworld-agent's signin.ts calls it.
const TOKEN_PATH: &str = "/api/auth/oauth2/token";
const DEVICE_GRANT: &str = "urn:ietf:params:oauth:grant-type:device_code";
/// `offline_access` is what earns a refresh token; `openid email` as signin.ts asks.
pub fn scope() -> String {
    format!("openid email offline_access {INGEST_SCOPE}")
}

/// Short enough that a resend lands well inside the IdP's 30 s replay window.
const TOKEN_TIMEOUT: Duration = Duration::from_secs(10);
const REFRESH_RESENDS: u32 = 2;
const RESEND_AFTER: Duration = Duration::from_secs(2);
const STORE_TRIES: u32 = 3;
/// An access token with less than this left is refreshed rather than used.
const MIN_TTL: i64 = 60;

#[derive(Debug, Error)]
pub enum AuthError {
    #[error("this build has no device client id: the IdP client for this agent isn't issued yet, so set RYNA_CLIENT_ID (see agent/README.md)")]
    NoClientId,
    #[error("sign-in was declined")]
    Declined,
    #[error("the sign-in code expired")]
    Expired,
    #[error("the sign-in no longer refreshes ({0})")]
    Lapsed(String),
    #[error("the sign-in was cancelled")]
    Cancelled,
    #[error("the sign-in provider is unavailable: {0}")]
    Unavailable(String),
    #[error("sign-in failed: {0}")]
    Failed(String),
    #[error("the OS keychain: {0}")]
    Store(String),
}

/// What `poll_device_login` fails with (`Cancelled` when its token fired).
pub type LoginError = AuthError;

impl AuthError {
    /// Only a new sign-in (or a client id) fixes this one.
    pub fn needs_login(&self) -> bool {
        matches!(
            self,
            AuthError::NoClientId | AuthError::Declined | AuthError::Expired | AuthError::Lapsed(_)
        )
    }

    pub fn fix(&self) -> Fix {
        match self {
            e if e.needs_login() => Fix::SignIn,
            AuthError::Unavailable(_) => Fix::Network,
            _ => Fix::Other,
        }
    }
}

/// An access token and when it expires (unix seconds).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccessToken {
    pub token: String,
    pub exp: i64,
}

impl AccessToken {
    /// One keychain value: `<exp> <token>`.
    pub fn encode(&self) -> String {
        format!("{} {}", self.exp, self.token)
    }
    pub fn decode(v: &str) -> Option<AccessToken> {
        let (exp, token) = v.trim().split_once(' ')?;
        if token.is_empty() || token.contains(char::is_whitespace) {
            return None;
        }
        Some(AccessToken {
            exp: exp.parse().ok()?,
            token: token.to_string(),
        })
    }
}

/// Where tokens live: the OS keychain, or memory in tests.
pub trait TokenStore {
    fn refresh_token(&self) -> Result<Option<String>, String>;
    fn set_refresh_token(&self, token: &str) -> Result<(), String>;
    /// A convenience; losing it only costs a refresh, so reading it never fails.
    fn access_token(&self) -> Option<AccessToken>;
    fn set_access_token(&self, token: &AccessToken) -> Result<(), String>;
    fn clear(&self) -> Result<(), String>;
    /// The kept sign-in lasts only until this computer restarts (Linux with no Secret Service
    /// running: the kernel keyring is all there is).
    fn until_reboot(&self) -> bool {
        false
    }
}

/// The OS keychain (macOS Keychain, Windows Credential Manager, Linux Secret Service with the
/// kernel keyring as its cache), service `ryna`. Sign-ins to an IdP other than the default are
/// kept under their own names, so a local-API sign-in never replaces the real one.
pub struct KeyringStore {
    refresh_account: String,
    access_account: String,
}

/// Linux: the Secret Service (gnome-keyring, KeePassXC, KWallet) keeps a sign-in across reboots,
/// with the kernel keyring (`keyutils`) in front of it as a cache. A desktop without one (a bare
/// Hyprland, a headless box) has only the kernel keyring, so the sign-in lasts until a reboot;
/// the choice is made once per process, by asking the session bus.
#[cfg(target_os = "linux")]
mod linux {
    use keyring::keyutils::KeyutilsCredential;
    use keyring::keyutils_persistent::KeyutilsPersistentCredential;
    use keyring::secret_service::SsCredential;
    use keyring::{Entry, Error};
    use std::sync::OnceLock;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Backend {
        /// Secret Service, cached in the kernel keyring: survives a reboot.
        SecretService,
        /// The kernel keyring alone: until a reboot.
        Keyutils,
    }

    /// What a probe of the Secret Service says about it. An item that isn't there, or a store
    /// that is locked or wants to prompt, is a Secret Service that answered; a failure to reach
    /// one (no session bus, nothing providing `org.freedesktop.secrets`) is none.
    pub fn backend_for(probe: Result<Vec<u8>, Error>) -> Backend {
        match probe {
            Ok(_) | Err(Error::NoEntry | Error::NoStorageAccess(_) | Error::Ambiguous(_)) => {
                Backend::SecretService
            }
            Err(_) => Backend::Keyutils,
        }
    }

    pub fn backend(service: &str) -> Backend {
        static BACKEND: OnceLock<Backend> = OnceLock::new();
        *BACKEND.get_or_init(|| {
            let probe = SsCredential::new_with_target(None, service, "probe")
                .and_then(|c| keyring::credential::CredentialApi::get_secret(&c));
            backend_for(probe)
        })
    }

    pub fn entry(service: &str, account: &str) -> Result<Entry, String> {
        let credential: Box<keyring::Credential> = match backend(service) {
            Backend::SecretService => Box::new(
                KeyutilsPersistentCredential::new_with_target(None, service, account)
                    .map_err(|e| e.to_string())?,
            ),
            Backend::Keyutils => Box::new(
                KeyutilsCredential::new_with_target(None, service, account)
                    .map_err(|e| e.to_string())?,
            ),
        };
        Ok(Entry::new_with_credential(credential))
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn a_secret_service_that_answers_is_used_even_when_locked_or_empty() {
            assert_eq!(backend_for(Ok(b"x".to_vec())), Backend::SecretService);
            assert_eq!(backend_for(Err(Error::NoEntry)), Backend::SecretService);
            assert_eq!(
                backend_for(Err(Error::NoStorageAccess("locked".into()))),
                Backend::SecretService
            );
        }

        #[test]
        fn no_secret_service_means_the_kernel_keyring() {
            assert_eq!(
                backend_for(Err(Error::PlatformFailure("no session bus".into()))),
                Backend::Keyutils
            );
        }
    }
}

impl KeyringStore {
    pub fn for_issuer(issuer: &str) -> Self {
        let suffix = if issuer == crate::config::DEFAULT_ISSUER {
            String::new()
        } else {
            format!(" {issuer}")
        };
        KeyringStore {
            refresh_account: format!("refresh-token{suffix}"),
            access_account: format!("access-token{suffix}"),
        }
    }

    #[cfg(target_os = "linux")]
    fn entry(account: &str) -> Result<keyring::Entry, String> {
        linux::entry(KEYRING_SERVICE, account)
    }

    #[cfg(not(target_os = "linux"))]
    fn entry(account: &str) -> Result<keyring::Entry, String> {
        keyring::Entry::new(KEYRING_SERVICE, account).map_err(|e| e.to_string())
    }

    fn get(account: &str) -> Result<Option<String>, String> {
        match Self::entry(account)?.get_password() {
            Ok(v) if v.trim().is_empty() => Ok(None),
            Ok(v) => Ok(Some(v.trim().to_string())),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }

    fn set(account: &str, value: &str) -> Result<(), String> {
        Self::entry(account)?
            .set_password(value)
            .map_err(|e| e.to_string())
    }

    fn delete(account: &str) -> Result<(), String> {
        match Self::entry(account)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    }
}

impl TokenStore for KeyringStore {
    fn until_reboot(&self) -> bool {
        #[cfg(target_os = "linux")]
        {
            linux::backend(KEYRING_SERVICE) == linux::Backend::Keyutils
        }
        #[cfg(not(target_os = "linux"))]
        {
            false
        }
    }
    fn refresh_token(&self) -> Result<Option<String>, String> {
        Self::get(&self.refresh_account)
    }
    fn set_refresh_token(&self, token: &str) -> Result<(), String> {
        Self::set(&self.refresh_account, token)
    }
    fn access_token(&self) -> Option<AccessToken> {
        Self::get(&self.access_account)
            .ok()
            .flatten()
            .and_then(|v| AccessToken::decode(&v))
    }
    fn set_access_token(&self, token: &AccessToken) -> Result<(), String> {
        Self::set(&self.access_account, &token.encode())
    }
    fn clear(&self) -> Result<(), String> {
        Self::delete(&self.access_account)?;
        Self::delete(&self.refresh_account)
    }
}

/// What the IdP shows the person.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceCode {
    pub user_code: String,
    pub verification_uri: String,
    pub verification_uri_complete: Option<String>,
    pub expires_in: i64,
}

impl DeviceCode {
    /// The link to open: the one with the code in it when the IdP gives one.
    pub fn link(&self) -> &str {
        self.verification_uri_complete
            .as_deref()
            .unwrap_or(&self.verification_uri)
    }
}

/// A started device grant: what to show the person, and what `poll_device_login` needs. The
/// device code is the poller's secret, so it is never serialised.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceLogin {
    pub user_code: String,
    pub verification_uri: String,
    pub verification_uri_complete: Option<String>,
    /// When the code stops working (unix seconds).
    pub expires_at: i64,
    /// Seconds between polls, as the provider asked.
    pub interval: u64,
    #[serde(skip)]
    pub device_code: String,
}

impl DeviceLogin {
    /// The link to open: the one with the code in it when the IdP gives one.
    pub fn link(&self) -> &str {
        self.verification_uri_complete
            .as_deref()
            .unwrap_or(&self.verification_uri)
    }
}

/// How the polling is going, for a front end that shows it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum LoginStatus {
    /// The person hasn't allowed it yet.
    Pending,
    /// The provider asked for slower polling; `interval` is the new gap in seconds.
    #[serde(rename_all = "camelCase")]
    SlowDown { interval: u64 },
    /// Allowed; the tokens are stored.
    Approved,
}

/// A finished sign-in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    /// Who it's for, from the new access token's claims (display only; see `Auth::identity`).
    pub signed_in_as: Option<String>,
}

/// `Auth::start_device_login` with the real network and keychain.
pub fn start_device_login(config: &Config) -> Result<DeviceLogin, LoginError> {
    let http = HttpTransport::new().map_err(AuthError::Unavailable)?;
    let store = KeyringStore::for_issuer(&config.issuer);
    Auth::new(config, &http, &store, &SystemClock).start_device_login()
}

/// `Auth::poll_device_login` with the real network and keychain: polls until the person allows
/// or refuses it, the code expires, or `cancel` fires (then `Cancelled`, at once while waiting
/// between polls, or as soon as a poll in flight answers).
pub fn poll_device_login(
    config: &Config,
    login: &DeviceLogin,
    cancel: &CancelToken,
    on_status: &mut dyn FnMut(LoginStatus),
) -> Result<Session, LoginError> {
    let http = HttpTransport::new().map_err(AuthError::Unavailable)?;
    let store = KeyringStore::for_issuer(&config.issuer);
    Auth::new(config, &http, &store, &SystemClock).poll_device_login(login, cancel, on_status)
}

#[derive(Debug, Default, Deserialize)]
struct DeviceAnswer {
    device_code: Option<String>,
    user_code: Option<String>,
    verification_uri: Option<String>,
    verification_uri_complete: Option<String>,
    interval: Option<i64>,
    expires_in: Option<i64>,
    error: Option<String>,
    error_description: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct TokenAnswer {
    access_token: Option<String>,
    refresh_token: Option<String>,
    expires_in: Option<i64>,
    error: Option<String>,
    error_description: Option<String>,
}

pub struct Auth<'a> {
    config: &'a Config,
    http: &'a dyn Transport,
    store: &'a dyn TokenStore,
    clock: &'a dyn Clock,
    cached: RefCell<Option<AccessToken>>,
    /// After a 401: don't fall back to the access token the keychain kept.
    forgotten: Cell<bool>,
}

impl<'a> Auth<'a> {
    pub fn new(
        config: &'a Config,
        http: &'a dyn Transport,
        store: &'a dyn TokenStore,
        clock: &'a dyn Clock,
    ) -> Self {
        Auth {
            config,
            http,
            store,
            clock,
            cached: RefCell::new(None),
            forgotten: Cell::new(false),
        }
    }

    /// Whether a refresh token is kept. Doesn't touch the network.
    pub fn signed_in(&self) -> Result<bool, AuthError> {
        Ok(self
            .store
            .refresh_token()
            .map_err(AuthError::Store)?
            .is_some())
    }

    /// Who the kept access token is for, for display: its `name`, `preferred_username`, or the
    /// local part of its `email` claim. Read from the token without checking it (it is only
    /// shown, never trusted); None when there's no token in hand or it carries none of them.
    pub fn identity(&self) -> Option<String> {
        let token = self
            .cached
            .borrow()
            .as_ref()
            .map(|a| a.token.clone())
            .or_else(|| self.store.access_token().map(|a| a.token))?;
        identity_from_jwt(&token)
    }

    /// Forget the sign-in on this computer.
    pub fn logout(&self) -> Result<(), AuthError> {
        *self.cached.borrow_mut() = None;
        self.store.clear().map_err(AuthError::Store)
    }

    /// Forget the access token in hand (after a 401), so the next call refreshes.
    pub fn forget_access_token(&self) {
        *self.cached.borrow_mut() = None;
        self.forgotten.set(true);
    }

    fn post_form(
        &self,
        path: &str,
        fields: &[(&str, &str)],
        timeout: Duration,
    ) -> Result<crate::http::Response, String> {
        self.http.post(&Request {
            method: crate::http::Method::Post,
            url: format!("{}{path}", self.config.issuer),
            content_type: "application/x-www-form-urlencoded",
            content_encoding: None,
            body: form_encode(fields),
            bearer: None,
            headers: Vec::new(),
            timeout,
        })
    }

    /// A token-endpoint call. `Err` is "no answer": the request may still have been used.
    fn token_call(&self, fields: &[(&str, &str)]) -> Result<TokenAnswer, String> {
        let res = self
            .post_form(TOKEN_PATH, fields, TOKEN_TIMEOUT)
            .map_err(|e| format!("sign-in provider unreachable: {e}"))?;
        serde_json::from_slice(&res.body)
            .map_err(|_| format!("sign-in provider answered {} with no JSON", res.status))
    }

    fn keep(&self, access: AccessToken, refresh: &str) -> Result<(), AuthError> {
        if let Err(e) = self.store.set_access_token(&access) {
            eprintln!("couldn't keep the access token for the next run: {e}");
        }
        *self.cached.borrow_mut() = Some(access);
        self.forgotten.set(false);
        let mut last = String::new();
        for i in 0..STORE_TRIES {
            if i > 0 {
                self.clock.sleep(Duration::from_secs(1));
            }
            match self.store.set_refresh_token(refresh) {
                Ok(()) => return Ok(()),
                Err(e) => last = e,
            }
        }
        Err(AuthError::Store(format!(
            "couldn't store the refresh token after {STORE_TRIES} tries ({last})"
        )))
    }

    /// Run the device flow in one call: `show` is called once with the link and code to show
    /// the person, then it polls until the end. `start_device_login` and `poll_device_login` are
    /// the two halves, for a front end that wants to cancel.
    pub fn login(&self, show: &mut dyn FnMut(&DeviceCode)) -> Result<(), AuthError> {
        let login = self.start_device_login()?;
        show(&DeviceCode {
            user_code: login.user_code.clone(),
            verification_uri: login.verification_uri.clone(),
            verification_uri_complete: login.verification_uri_complete.clone(),
            expires_in: login.expires_at - self.clock.now(),
        });
        self.poll_device_login(&login, &CancelToken::new(), &mut |_| {})
            .map(|_| ())
    }

    /// Ask the provider for a device code. Nothing is stored until the poll succeeds.
    pub fn start_device_login(&self) -> Result<DeviceLogin, AuthError> {
        if !self.config.client_id_issued() {
            return Err(AuthError::NoClientId);
        }
        let scope = scope();
        let res = self
            .post_form(
                DEVICE_CODE_PATH,
                &[
                    ("client_id", &self.config.client_id),
                    ("scope", &scope),
                    ("resource", &self.config.resource),
                ],
                Duration::from_secs(30),
            )
            .map_err(AuthError::Unavailable)?;
        let answer: DeviceAnswer = serde_json::from_slice(&res.body).unwrap_or_default();
        let (Some(device_code), Some(user_code), Some(verification_uri)) = (
            answer.device_code,
            answer.user_code,
            answer
                .verification_uri
                .or(answer.verification_uri_complete.clone()),
        ) else {
            let why = answer
                .error_description
                .or(answer.error)
                .unwrap_or_else(|| format!("HTTP {}: {}", res.status, res.excerpt()));
            return Err(AuthError::Failed(format!(
                "couldn't get a sign-in code: {why}"
            )));
        };
        let expires_in = answer.expires_in.unwrap_or(600);
        Ok(DeviceLogin {
            user_code,
            verification_uri,
            verification_uri_complete: answer.verification_uri_complete,
            expires_at: self.clock.now() + expires_in,
            interval: answer.interval.unwrap_or(10).max(1) as u64,
            device_code,
        })
    }

    /// Poll for the person's answer to `login`, telling `on_status` after each poll, and store
    /// the tokens once it's allowed. The wait between polls ends at once when `cancel` fires.
    pub fn poll_device_login(
        &self,
        login: &DeviceLogin,
        cancel: &CancelToken,
        on_status: &mut dyn FnMut(LoginStatus),
    ) -> Result<Session, AuthError> {
        let mut interval = login.interval.max(1);
        while self.clock.now() < login.expires_at {
            if self
                .clock
                .sleep_unless_cancelled(Duration::from_secs(interval), cancel)
            {
                return Err(AuthError::Cancelled);
            }
            let answer = self.token_call(&[
                ("grant_type", DEVICE_GRANT),
                ("client_id", &self.config.client_id),
                ("device_code", &login.device_code),
            ]);
            if cancel.is_cancelled() {
                return Err(AuthError::Cancelled);
            }
            let t = match answer {
                Ok(t) => t,
                // A blip while polling: keep polling until the code expires.
                Err(_) => continue,
            };
            if let Some(access) = t.access_token {
                let refresh = t.refresh_token.ok_or_else(|| {
                    AuthError::Failed(
                        "signed in, but the provider sent no refresh token (offline_access)".into(),
                    )
                })?;
                let exp = self.clock.now() + t.expires_in.unwrap_or(900);
                let signed_in_as = identity_from_jwt(&access);
                self.keep(AccessToken { token: access, exp }, &refresh)?;
                on_status(LoginStatus::Approved);
                return Ok(Session { signed_in_as });
            }
            match t.error.as_deref() {
                Some("authorization_pending") => on_status(LoginStatus::Pending),
                Some("slow_down") => {
                    interval += 5;
                    on_status(LoginStatus::SlowDown { interval });
                }
                Some("access_denied") => return Err(AuthError::Declined),
                Some("expired_token") => return Err(AuthError::Expired),
                other => {
                    return Err(AuthError::Failed(
                        t.error_description
                            .or(other.map(str::to_string))
                            .unwrap_or_else(|| "unknown".into()),
                    ))
                }
            }
        }
        Err(AuthError::Expired)
    }

    /// An access token for a push, or None when this agent was never signed in.
    pub fn access_token(&self) -> Result<Option<String>, AuthError> {
        let now = self.clock.now();
        let fresh = |a: &AccessToken| a.exp - now > MIN_TTL;
        if let Some(a) = self.cached.borrow().as_ref().filter(|a| fresh(a)) {
            return Ok(Some(a.token.clone()));
        }
        if !self.forgotten.get() {
            if let Some(a) = self.store.access_token().filter(fresh) {
                let token = a.token.clone();
                *self.cached.borrow_mut() = Some(a);
                return Ok(Some(token));
            }
        }
        let Some(refresh) = self.store.refresh_token().map_err(AuthError::Store)? else {
            return Ok(None);
        };
        if !self.config.client_id_issued() {
            return Err(AuthError::NoClientId);
        }
        let fields = [
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh.as_str()),
            ("client_id", self.config.client_id.as_str()),
            ("resource", self.config.resource.as_str()),
        ];
        let mut resend = 0;
        let t = loop {
            match self.token_call(&fields) {
                Ok(t) => break t,
                Err(e) if resend < REFRESH_RESENDS => {
                    // The IdP may have rotated without our hearing it: the same token again, at
                    // once, gets the lost answer back; waiting for the next run would revoke the chain.
                    eprintln!("{e}; sending the refresh again");
                    resend += 1;
                    self.clock.sleep(RESEND_AFTER);
                }
                Err(e) => return Err(AuthError::Unavailable(e)),
            }
        };
        let (Some(access), Some(new_refresh)) = (t.access_token, t.refresh_token) else {
            let error = t.error.unwrap_or_else(|| "no token".into());
            if error == "server_error" || error == "temporarily_unavailable" {
                return Err(AuthError::Unavailable(error));
            }
            return Err(AuthError::Lapsed(error));
        };
        let exp = self.clock.now() + t.expires_in.unwrap_or(900);
        let token = access.clone();
        if let Err(e) = self.keep(AccessToken { token: access, exp }, &new_refresh) {
            // The old refresh token is spent; this run can still push with the access token.
            eprintln!("{e}: the sign-in is lost once this run ends; sign in again");
        }
        Ok(Some(token))
    }
}

/// RFC 4648 base64url, padding optional.
fn base64url_decode(s: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let (mut acc, mut bits) = (0u32, 0u32);
    for c in s.trim_end_matches('=').bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'-' | b'+' => 62,
            b'_' | b'/' => 63,
            _ => return None,
        };
        acc = (acc << 6) | u32::from(v);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
    }
    Some(out)
}

/// The display identity in a JWT's payload (see `Auth::identity`).
pub fn identity_from_jwt(token: &str) -> Option<String> {
    let payload = token.split('.').nth(1)?;
    let claims: serde_json::Value = serde_json::from_slice(&base64url_decode(payload)?).ok()?;
    let text = |k: &str| {
        claims
            .get(k)
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|v| !v.is_empty())
    };
    text("name")
        .or_else(|| text("preferred_username"))
        .map(str::to_string)
        .or_else(|| {
            text("email")
                .and_then(|e| e.split('@').next())
                .map(str::to_string)
        })
}

/// Test doubles, for this crate's tests and (feature `testing`) a core's.
#[cfg(any(test, feature = "testing"))]
#[doc(hidden)]
pub mod testing {
    use super::*;
    use std::cell::RefCell;

    #[derive(Default)]
    pub struct MemoryStore {
        pub refresh: RefCell<Option<String>>,
        pub access: RefCell<Option<AccessToken>>,
        pub fail_writes: Cell<u32>,
        /// Plays a Linux box with no Secret Service.
        pub until_reboot: Cell<bool>,
    }

    impl MemoryStore {
        pub fn with_refresh(token: &str) -> Self {
            let s = Self::default();
            *s.refresh.borrow_mut() = Some(token.into());
            s
        }
        pub fn with_access(token: &str, exp: i64) -> Self {
            let s = Self::with_refresh("r0");
            *s.access.borrow_mut() = Some(AccessToken {
                token: token.into(),
                exp,
            });
            s
        }
    }

    impl TokenStore for MemoryStore {
        fn refresh_token(&self) -> Result<Option<String>, String> {
            Ok(self.refresh.borrow().clone())
        }
        fn set_refresh_token(&self, token: &str) -> Result<(), String> {
            if self.fail_writes.get() > 0 {
                self.fail_writes.set(self.fail_writes.get() - 1);
                return Err("locked".into());
            }
            *self.refresh.borrow_mut() = Some(token.into());
            Ok(())
        }
        fn access_token(&self) -> Option<AccessToken> {
            self.access.borrow().clone()
        }
        fn set_access_token(&self, token: &AccessToken) -> Result<(), String> {
            *self.access.borrow_mut() = Some(token.clone());
            Ok(())
        }
        fn clear(&self) -> Result<(), String> {
            *self.refresh.borrow_mut() = None;
            *self.access.borrow_mut() = None;
            Ok(())
        }
        fn until_reboot(&self) -> bool {
            self.until_reboot.get()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::testing::MemoryStore;
    use super::*;
    use crate::config::{test_config, PLACEHOLDER_CLIENT_ID};
    use crate::http::testing::ScriptedTransport;
    use crate::util::testing::FakeClock;
    use serde_json::json;

    const NOW: i64 = 1_800_000_000;

    fn device_answer() -> serde_json::Value {
        json!({ "device_code": "dev-1", "user_code": "ABCD-EFGH", "verification_uri": "https://idp.test/device",
                "verification_uri_complete": "https://idp.test/device?user_code=ABCD-EFGH", "interval": 5, "expires_in": 600 })
    }

    #[test]
    fn device_flow_polls_through_pending_and_slow_down() {
        let config = test_config();
        let http = ScriptedTransport::new();
        http.json(200, device_answer())
            .json(400, json!({ "error": "authorization_pending" }))
            .json(400, json!({ "error": "slow_down" }))
            .fail("connection reset")
            .json(
                200,
                json!({ "access_token": "at-1", "refresh_token": "rt-1", "expires_in": 900 }),
            );
        let store = MemoryStore::default();
        let clock = FakeClock::at(NOW);
        let auth = Auth::new(&config, &http, &store, &clock);
        let mut shown = None;
        auth.login(&mut |c| shown = Some(c.clone())).unwrap();

        let shown = shown.unwrap();
        assert_eq!(shown.user_code, "ABCD-EFGH");
        assert_eq!(shown.link(), "https://idp.test/device?user_code=ABCD-EFGH");
        assert_eq!(
            http.urls(),
            vec![
                "https://idp.test/api/auth/device/code",
                "https://idp.test/api/auth/oauth2/token",
                "https://idp.test/api/auth/oauth2/token",
                "https://idp.test/api/auth/oauth2/token",
                "https://idp.test/api/auth/oauth2/token",
            ]
        );
        let first = http.form(0);
        assert!(first.contains("client_id=test-client"), "{first}");
        assert!(
            first.contains("scope=openid+email+offline_access+wow%3Aingest"),
            "{first}"
        );
        assert!(
            first.contains("resource=https%3A%2F%2Fapi.ryna.io%2Fwow%2Fingest"),
            "{first}"
        );
        assert!(http
            .form(1)
            .contains("grant_type=urn%3Aietf%3Aparams%3Aoauth%3Agrant-type%3Adevice_code"));
        assert!(http.form(1).contains("device_code=dev-1"));
        // 5 s, 5 s, then slow_down adds 5.
        let sleeps: Vec<u64> = clock.sleeps.borrow().iter().map(|d| d.as_secs()).collect();
        assert_eq!(sleeps, vec![5, 5, 10, 10]);
        assert_eq!(store.refresh.borrow().as_deref(), Some("rt-1"));
        assert_eq!(store.access.borrow().as_ref().unwrap().token, "at-1");
        assert_eq!(auth.access_token().unwrap().as_deref(), Some("at-1"));
        assert_eq!(http.remaining(), 0);
    }

    #[test]
    fn device_flow_refusals() {
        for (error, expect) in [
            ("access_denied", "declined"),
            ("expired_token", "expired"),
            ("invalid_grant", "invalid_grant"),
        ] {
            let config = test_config();
            let http = ScriptedTransport::new();
            http.json(200, device_answer())
                .json(400, json!({ "error": error }));
            let store = MemoryStore::default();
            let clock = FakeClock::at(NOW);
            let e = Auth::new(&config, &http, &store, &clock)
                .login(&mut |_| {})
                .unwrap_err();
            assert!(e.to_string().contains(expect), "{error}: {e}");
            assert!(store.refresh.borrow().is_none());
        }
    }

    #[test]
    fn device_flow_gives_up_when_the_code_expires() {
        let config = test_config();
        let http = ScriptedTransport::new();
        let mut answer = device_answer();
        answer["expires_in"] = json!(12);
        http.json(200, answer);
        for _ in 0..3 {
            http.json(400, json!({ "error": "authorization_pending" }));
        }
        let store = MemoryStore::default();
        let clock = FakeClock::at(NOW);
        let e = Auth::new(&config, &http, &store, &clock)
            .login(&mut |_| {})
            .unwrap_err();
        assert!(matches!(e, AuthError::Expired));
    }

    #[test]
    fn login_needs_an_issued_client_id() {
        let mut config = test_config();
        config.client_id = PLACEHOLDER_CLIENT_ID.into();
        let http = ScriptedTransport::new();
        let store = MemoryStore::default();
        let clock = FakeClock::at(NOW);
        let e = Auth::new(&config, &http, &store, &clock)
            .login(&mut |_| {})
            .unwrap_err();
        assert!(matches!(e, AuthError::NoClientId));
        assert!(e.to_string().contains("RYNA_CLIENT_ID"));
        assert!(http.requests.borrow().is_empty());
    }

    #[test]
    fn device_code_refusal_is_reported() {
        let config = test_config();
        let http = ScriptedTransport::new();
        http.json(
            400,
            json!({ "error": "invalid_client", "error_description": "unknown client" }),
        );
        let store = MemoryStore::default();
        let clock = FakeClock::at(NOW);
        let e = Auth::new(&config, &http, &store, &clock)
            .login(&mut |_| {})
            .unwrap_err();
        assert!(e.to_string().contains("unknown client"), "{e}");
    }

    #[test]
    fn refresh_rotates_and_is_reused_until_near_expiry() {
        let config = test_config();
        let http = ScriptedTransport::new();
        http.json(
            200,
            json!({ "access_token": "at-2", "refresh_token": "rt-2", "expires_in": 900 }),
        );
        let store = MemoryStore::with_refresh("rt-1");
        let clock = FakeClock::at(NOW);
        let auth = Auth::new(&config, &http, &store, &clock);
        assert_eq!(auth.access_token().unwrap().as_deref(), Some("at-2"));
        let form = http.form(0);
        assert!(
            form.contains("grant_type=refresh_token")
                && form.contains("refresh_token=rt-1")
                && form.contains("resource="),
            "{form}"
        );
        assert_eq!(store.refresh.borrow().as_deref(), Some("rt-2"));
        // In memory, then from the store by a new Auth (the next run): no more calls.
        assert_eq!(auth.access_token().unwrap().as_deref(), Some("at-2"));
        let next_run = Auth::new(&config, &http, &store, &clock);
        assert_eq!(next_run.access_token().unwrap().as_deref(), Some("at-2"));
        assert_eq!(http.requests.borrow().len(), 1);
        // Within a minute of expiry it refreshes again, with the rotated token.
        clock.now.set(NOW + 900 - 30);
        http.json(
            200,
            json!({ "access_token": "at-3", "refresh_token": "rt-3" }),
        );
        assert_eq!(next_run.access_token().unwrap().as_deref(), Some("at-3"));
        assert!(http.form(1).contains("refresh_token=rt-2"));
    }

    #[test]
    fn unanswered_refresh_is_resent_with_the_same_token() {
        let config = test_config();
        let http = ScriptedTransport::new();
        http.fail("timed out")
            .text(502, "<html>bad gateway</html>")
            .json(
                200,
                json!({ "access_token": "at", "refresh_token": "rt-2" }),
            );
        let store = MemoryStore::with_refresh("rt-1");
        let clock = FakeClock::at(NOW);
        let auth = Auth::new(&config, &http, &store, &clock);
        assert_eq!(auth.access_token().unwrap().as_deref(), Some("at"));
        for i in 0..3 {
            assert!(http.form(i).contains("refresh_token=rt-1"));
        }
        assert_eq!(clock.sleeps.borrow().len(), 2);

        let http = ScriptedTransport::new();
        http.fail("a").fail("b").fail("c");
        let store = MemoryStore::with_refresh("rt-1");
        let auth = Auth::new(&config, &http, &store, &clock);
        assert!(matches!(
            auth.access_token(),
            Err(AuthError::Unavailable(_))
        ));
        assert_eq!(store.refresh.borrow().as_deref(), Some("rt-1"));
    }

    #[test]
    fn a_refused_refresh_needs_login_but_provider_trouble_does_not() {
        let config = test_config();
        let clock = FakeClock::at(NOW);
        let http = ScriptedTransport::new();
        http.json(400, json!({ "error": "invalid_grant" }));
        let store = MemoryStore::with_refresh("rt-1");
        let e = Auth::new(&config, &http, &store, &clock)
            .access_token()
            .unwrap_err();
        assert!(e.needs_login(), "{e}");
        let http = ScriptedTransport::new();
        http.json(503, json!({ "error": "temporarily_unavailable" }));
        let e = Auth::new(&config, &http, &store, &clock)
            .access_token()
            .unwrap_err();
        assert!(!e.needs_login(), "{e}");
    }

    #[test]
    fn not_signed_in_is_none_and_logout_clears() {
        let config = test_config();
        let http = ScriptedTransport::new();
        let clock = FakeClock::at(NOW);
        let store = MemoryStore::default();
        let auth = Auth::new(&config, &http, &store, &clock);
        assert_eq!(auth.access_token().unwrap(), None);
        assert!(!auth.signed_in().unwrap());
        let store = MemoryStore::with_access("at", NOW + 3600);
        let auth = Auth::new(&config, &http, &store, &clock);
        assert!(auth.signed_in().unwrap());
        auth.logout().unwrap();
        assert!(!auth.signed_in().unwrap());
        assert_eq!(auth.access_token().unwrap(), None);
    }

    #[test]
    fn forgetting_skips_the_kept_access_token() {
        let config = test_config();
        let http = ScriptedTransport::new();
        http.json(
            200,
            json!({ "access_token": "new", "refresh_token": "rt-2" }),
        );
        let clock = FakeClock::at(NOW);
        let store = MemoryStore::with_access("old", NOW + 3600);
        let auth = Auth::new(&config, &http, &store, &clock);
        assert_eq!(auth.access_token().unwrap().as_deref(), Some("old"));
        auth.forget_access_token();
        assert_eq!(auth.access_token().unwrap().as_deref(), Some("new"));
    }

    #[test]
    fn a_refresh_store_that_fails_once_is_retried() {
        let config = test_config();
        let http = ScriptedTransport::new();
        http.json(
            200,
            json!({ "access_token": "at", "refresh_token": "rt-2" }),
        );
        let clock = FakeClock::at(NOW);
        let store = MemoryStore::with_refresh("rt-1");
        store.fail_writes.set(1);
        Auth::new(&config, &http, &store, &clock)
            .access_token()
            .unwrap();
        assert_eq!(store.refresh.borrow().as_deref(), Some("rt-2"));
    }

    #[test]
    fn identity_from_the_token_claims() {
        // {"sub":"x","email":"aerin@example.test"} and {"name":"Aerin S","email":"a@b"}
        let email_only = "h.eyJzdWIiOiJ4IiwiZW1haWwiOiJhZXJpbkBleGFtcGxlLnRlc3QifQ.s";
        let named = "h.eyJuYW1lIjoiQWVyaW4gUyIsImVtYWlsIjoiYUBiIn0.s";
        assert_eq!(identity_from_jwt(email_only).as_deref(), Some("aerin"));
        assert_eq!(identity_from_jwt(named).as_deref(), Some("Aerin S"));
        assert_eq!(identity_from_jwt("opaque-token"), None);
        assert_eq!(base64url_decode("aGk").unwrap(), b"hi");
        let config = test_config();
        let http = ScriptedTransport::new();
        let clock = FakeClock::at(NOW);
        let store = MemoryStore::with_access(email_only, NOW + 3600);
        assert_eq!(
            Auth::new(&config, &http, &store, &clock)
                .identity()
                .as_deref(),
            Some("aerin")
        );
    }

    #[test]
    fn start_then_poll_reports_each_answer() {
        let config = test_config();
        let http = ScriptedTransport::new();
        http.json(200, device_answer())
            .json(400, json!({ "error": "authorization_pending" }))
            .json(400, json!({ "error": "slow_down" }))
            .json(
                200,
                // {"name":"Aerin S","email":"a@b"}
                json!({ "access_token": "h.eyJuYW1lIjoiQWVyaW4gUyIsImVtYWlsIjoiYUBiIn0.s", "refresh_token": "rt-1" }),
            );
        let store = MemoryStore::default();
        let clock = FakeClock::at(NOW);
        let auth = Auth::new(&config, &http, &store, &clock);
        let login = auth.start_device_login().unwrap();
        assert_eq!(login.expires_at, NOW + 600);
        assert_eq!(login.interval, 5);
        assert_eq!(login.device_code, "dev-1");
        assert_eq!(login.link(), "https://idp.test/device?user_code=ABCD-EFGH");
        let shown = serde_json::to_value(&login).unwrap();
        assert_eq!(shown["userCode"], json!("ABCD-EFGH"));
        assert!(shown.get("deviceCode").is_none(), "{shown}");
        assert!(store.refresh.borrow().is_none());

        let mut seen = Vec::new();
        let session = auth
            .poll_device_login(&login, &CancelToken::new(), &mut |s| seen.push(s))
            .unwrap();
        assert_eq!(session.signed_in_as.as_deref(), Some("Aerin S"));
        assert_eq!(
            seen,
            vec![
                LoginStatus::Pending,
                LoginStatus::SlowDown { interval: 10 },
                LoginStatus::Approved
            ]
        );
        assert_eq!(store.refresh.borrow().as_deref(), Some("rt-1"));
    }

    #[test]
    fn a_cancelled_poll_stops_and_stores_nothing() {
        let config = test_config();
        let http = ScriptedTransport::new();
        http.json(200, device_answer())
            .json(400, json!({ "error": "authorization_pending" }));
        let store = MemoryStore::default();
        let clock = FakeClock::at(NOW);
        let auth = Auth::new(&config, &http, &store, &clock);
        let login = auth.start_device_login().unwrap();
        let cancel = CancelToken::new();
        let e = auth
            .poll_device_login(&login, &cancel, &mut |_| cancel.cancel())
            .unwrap_err();
        assert!(matches!(e, AuthError::Cancelled), "{e}");
        assert_eq!(e.fix(), Fix::Other);
        // One poll, then the cancel ended it before the next.
        assert_eq!(http.requests.borrow().len(), 2);
        assert!(store.refresh.borrow().is_none());
        // Already cancelled: no poll at all.
        let e = auth
            .poll_device_login(&login, &cancel, &mut |_| {})
            .unwrap_err();
        assert!(matches!(e, AuthError::Cancelled));
        assert_eq!(http.requests.borrow().len(), 2);
    }

    #[test]
    fn the_real_clock_wakes_a_poll_wait_on_cancel() {
        let cancel = CancelToken::new();
        let other = cancel.clone();
        let t = std::thread::spawn(move || {
            let start = std::time::Instant::now();
            let cancelled = SystemClock.sleep_unless_cancelled(Duration::from_secs(30), &other);
            (cancelled, start.elapsed())
        });
        std::thread::sleep(Duration::from_millis(50));
        cancel.cancel();
        let (cancelled, took) = t.join().unwrap();
        assert!(cancelled && took < Duration::from_secs(5));
    }

    #[test]
    fn errors_name_no_command_and_carry_their_fix() {
        for e in [
            AuthError::Expired,
            AuthError::Lapsed("invalid_grant".into()),
            AuthError::Declined,
        ] {
            assert!(!e.to_string().contains("ryna login"), "{e}");
            assert_eq!(e.fix(), Fix::SignIn);
        }
        assert_eq!(AuthError::Unavailable("x".into()).fix(), Fix::Network);
        assert_eq!(AuthError::Store("x".into()).fix(), Fix::Other);
    }

    #[test]
    fn access_token_encoding() {
        let a = AccessToken {
            token: "abc.def".into(),
            exp: 42,
        };
        assert_eq!(AccessToken::decode(&a.encode()), Some(a));
        assert_eq!(AccessToken::decode("nope"), None);
        assert_eq!(AccessToken::decode("x abc"), None);
    }

    #[test]
    fn keyring_accounts_are_per_issuer() {
        let d = KeyringStore::for_issuer(crate::config::DEFAULT_ISSUER);
        assert_eq!(d.refresh_account, "refresh-token");
        let l = KeyringStore::for_issuer("http://localhost:5174");
        assert_eq!(l.refresh_account, "refresh-token http://localhost:5174");
    }
}
