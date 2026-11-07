//! The device key and the submission ID (docs/ryna-agent-cores-mods.md 7.1; the API's
//! docs/wow.md section 13.1).
//!
//! The agent holds one Ed25519 keypair per computer. Its 32-byte seed is kept in the OS keychain
//! (service `ryna`, account `device-key`), never in a file, minted on first use. An API other than
//! the default (a local one, the smoke test's) gets its own, `device-key <api>`, as sign-ins to
//! another IdP do, so a developer's ID never stands in for the real one. The **submission
//! ID** is the first 32 lowercase hex characters of the SHA-256 of the raw 32-byte public key, shown
//! to the person in groups of four (`a1b2-c3d4-…`). Every POST to the API carries three headers
//! ([`DeviceKey::headers`]): the ID, the public key (standard base64) and the Ed25519 signature
//! over the exact body bytes sent (standard base64).
//!
//! A seed the keychain holds that doesn't parse is an error, never a reason to mint a new key:
//! a new key is a new ID, which starts again at zero.

use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use ed25519_dalek::{Signature, Signer as _, SigningKey, Verifier as _, VerifyingKey};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::auth::KEYRING_SERVICE;
use crate::config::Config;

/// The keychain account the seed is kept under (service `ryna`).
pub const DEVICE_KEY_ACCOUNT: &str = "device-key";
/// The submission ID (32 lowercase hex).
pub const SUBMISSION_HEADER: &str = "X-Ryna-Submission";
/// The raw public key, standard base64 (44 characters).
pub const KEY_HEADER: &str = "X-Ryna-Key";
/// The signature over the body, standard base64 (88 characters).
pub const SIGNATURE_HEADER: &str = "X-Ryna-Signature";

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum IdentityError {
    #[error("the device key couldn't be read from the system keychain: {0}")]
    Unreadable(String),
    #[error("the device key in the system keychain isn't a key ({0}); it was left as it is, since a new one would be a new submission ID")]
    Corrupt(String),
    #[error("a new device key couldn't be kept in the system keychain: {0}")]
    Unwritable(String),
}

/// What signs the agent's posts: the device key, or an in-memory one in tests.
pub trait DeviceKey: Send + Sync {
    /// The raw 32-byte Ed25519 public key.
    fn public_key(&self) -> [u8; 32];
    /// The Ed25519 signature over `body`, raw.
    fn sign_raw(&self, body: &[u8]) -> [u8; 64];

    /// The submission ID: the first 32 lowercase hex of SHA-256 over the raw public key.
    fn submission_id(&self) -> String {
        submission_id(&self.public_key())
    }
    /// The ID as the person sees it: groups of four, joined with dashes.
    fn display_id(&self) -> String {
        display_id(&self.submission_id())
    }
    /// The public key, standard base64 (44 characters).
    fn public_key_b64(&self) -> String {
        STANDARD.encode(self.public_key())
    }
    /// The signature over `body`, standard base64 (88 characters).
    fn sign(&self, body: &[u8]) -> String {
        STANDARD.encode(self.sign_raw(body))
    }
    /// The three headers for a POST whose body is exactly `body`.
    fn headers(&self, body: &[u8]) -> Vec<(String, String)> {
        vec![
            (SUBMISSION_HEADER.to_string(), self.submission_id()),
            (KEY_HEADER.to_string(), self.public_key_b64()),
            (SIGNATURE_HEADER.to_string(), self.sign(body)),
        ]
    }
}

/// The first 32 lowercase hex of SHA-256 over a raw public key.
pub fn submission_id(public_key: &[u8; 32]) -> String {
    Sha256::digest(public_key)[..16]
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// `a1b2c3d4…` as `a1b2-c3d4-…`.
pub fn display_id(id: &str) -> String {
    id.as_bytes()
        .chunks(4)
        .map(|c| String::from_utf8_lossy(c).into_owned())
        .collect::<Vec<_>>()
        .join("-")
}

/// Whether `signature_b64` is `public_key_b64`'s signature over `body` (what the API checks).
pub fn verify(public_key_b64: &str, body: &[u8], signature_b64: &str) -> bool {
    let (Ok(key), Ok(sig)) = (
        STANDARD.decode(public_key_b64),
        STANDARD.decode(signature_b64),
    ) else {
        return false;
    };
    let (Ok(key), Ok(sig)) = (<[u8; 32]>::try_from(key), <[u8; 64]>::try_from(sig)) else {
        return false;
    };
    let Ok(key) = VerifyingKey::from_bytes(&key) else {
        return false;
    };
    key.verify(body, &Signature::from_bytes(&sig)).is_ok()
}

/// An Ed25519 device key held in memory (loaded from the keychain, or a test's).
#[derive(Clone)]
pub struct Ed25519Key {
    signing: SigningKey,
}

impl std::fmt::Debug for Ed25519Key {
    // Never the seed.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Ed25519Key")
            .field("submission_id", &self.submission_id())
            .finish()
    }
}

impl Ed25519Key {
    pub fn from_seed(seed: &[u8; 32]) -> Self {
        Ed25519Key {
            signing: SigningKey::from_bytes(seed),
        }
    }

    /// A new key from the OS's random numbers.
    pub fn generate() -> Self {
        Ed25519Key {
            signing: SigningKey::generate(&mut rand_core::OsRng),
        }
    }

    /// The seed as the keychain keeps it: standard base64 of the 32 bytes.
    fn seed_b64(&self) -> String {
        STANDARD.encode(self.signing.to_bytes())
    }

    /// A kept seed back into a key.
    fn parse_seed(v: &str) -> Result<Self, String> {
        let bytes = STANDARD
            .decode(v.trim())
            .map_err(|e| format!("not base64: {e}"))?;
        let seed = <[u8; 32]>::try_from(bytes).map_err(|b| format!("{} bytes, not 32", b.len()))?;
        Ok(Ed25519Key::from_seed(&seed))
    }
}

impl DeviceKey for Ed25519Key {
    fn public_key(&self) -> [u8; 32] {
        self.signing.verifying_key().to_bytes()
    }
    fn sign_raw(&self, body: &[u8]) -> [u8; 64] {
        self.signing.sign(body).to_bytes()
    }
}

/// Where the seed is kept: the OS keychain, or memory in tests.
pub trait SeedStore {
    fn get(&self) -> Result<Option<String>, String>;
    fn set(&self, seed_b64: &str) -> Result<(), String>;
}

/// The OS keychain, service `ryna`, account `device-key` (`device-key <api>` for another API).
pub struct KeyringSeedStore {
    account: String,
}

impl KeyringSeedStore {
    pub fn for_api(api: &str) -> Self {
        let api = api.trim_end_matches('/');
        KeyringSeedStore {
            account: if api == crate::config::DEFAULT_API {
                DEVICE_KEY_ACCOUNT.to_string()
            } else {
                format!("{DEVICE_KEY_ACCOUNT} {api}")
            },
        }
    }

    fn entry(&self) -> Result<keyring::Entry, String> {
        keyring::Entry::new(KEYRING_SERVICE, &self.account).map_err(|e| e.to_string())
    }
}

impl SeedStore for KeyringSeedStore {
    fn get(&self) -> Result<Option<String>, String> {
        match self.entry()?.get_password() {
            Ok(v) if v.trim().is_empty() => Ok(None),
            Ok(v) => Ok(Some(v)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }
    fn set(&self, seed_b64: &str) -> Result<(), String> {
        self.entry()?
            .set_password(seed_b64)
            .map_err(|e| e.to_string())
    }
}

/// The kept key, or a new one minted and kept when there is none. A kept value that doesn't
/// parse is [`IdentityError::Corrupt`] and is left alone.
pub fn load_or_mint(store: &dyn SeedStore) -> Result<Ed25519Key, IdentityError> {
    if let Some(kept) = store.get().map_err(IdentityError::Unreadable)? {
        return Ed25519Key::parse_seed(&kept).map_err(IdentityError::Corrupt);
    }
    let minted = Ed25519Key::generate();
    store
        .set(&minted.seed_b64())
        .map_err(IdentityError::Unwritable)?;
    // Read it back: another process (the CLI beside the app) may have minted at the same moment,
    // and the one the keychain kept is the one both must use.
    match store.get().map_err(IdentityError::Unreadable)? {
        Some(kept) => Ed25519Key::parse_seed(&kept).map_err(IdentityError::Corrupt),
        None => Err(IdentityError::Unwritable(
            "the keychain kept nothing".into(),
        )),
    }
}

/// The kept key, if there is one; never mints.
pub fn load(store: &dyn SeedStore) -> Result<Option<Ed25519Key>, IdentityError> {
    match store.get().map_err(IdentityError::Unreadable)? {
        Some(kept) => Ed25519Key::parse_seed(&kept)
            .map(Some)
            .map_err(IdentityError::Corrupt),
        None => Ok(None),
    }
}

/// [`load_or_mint`] with the OS keychain, for the API at `config.api`.
pub fn device_key(config: &Config) -> Result<Ed25519Key, IdentityError> {
    load_or_mint(&KeyringSeedStore::for_api(&config.api))
}

/// [`load`] with the OS keychain, for the API at `config.api`.
pub fn stored_device_key(config: &Config) -> Result<Option<Ed25519Key>, IdentityError> {
    load(&KeyringSeedStore::for_api(&config.api))
}

/// Test doubles, for this crate's tests and (feature `testing`) a core's.
#[cfg(any(test, feature = "testing"))]
#[doc(hidden)]
pub mod testing {
    use super::*;
    use std::cell::{Cell, RefCell};

    /// RFC 8032's first test key (section 7.1, TEST 1).
    pub const RFC8032_SEED: [u8; 32] = [
        0x9d, 0x61, 0xb1, 0x9d, 0xef, 0xfd, 0x5a, 0x60, 0xba, 0x84, 0x4a, 0xf4, 0x92, 0xec, 0x2c,
        0xc4, 0x44, 0x49, 0xc5, 0x69, 0x7b, 0x32, 0x69, 0x19, 0x70, 0x3b, 0xac, 0x03, 0x1c, 0xae,
        0x7f, 0x60,
    ];

    /// An in-memory key with a fixed seed (RFC 8032 TEST 1).
    pub fn test_key() -> Ed25519Key {
        Ed25519Key::from_seed(&RFC8032_SEED)
    }

    #[derive(Default)]
    pub struct MemorySeedStore {
        pub seed: RefCell<Option<String>>,
        pub fail_reads: Cell<bool>,
        pub fail_writes: Cell<bool>,
        pub writes: Cell<u32>,
    }

    impl SeedStore for MemorySeedStore {
        fn get(&self) -> Result<Option<String>, String> {
            if self.fail_reads.get() {
                return Err("the keychain is locked".into());
            }
            Ok(self.seed.borrow().clone())
        }
        fn set(&self, seed_b64: &str) -> Result<(), String> {
            if self.fail_writes.get() {
                return Err("the keychain is read-only".into());
            }
            self.writes.set(self.writes.get() + 1);
            *self.seed.borrow_mut() = Some(seed_b64.to_string());
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::testing::*;
    use super::*;

    const RFC8032_PUBLIC_B64: &str = "11qYAYKxCrfVS/7TyWQHOg7hcvPapiMlrwIaaPcHURo=";
    const RFC8032_EMPTY_SIG_B64: &str =
        "5VZDAMNgrHKQhuLMgG6CioSHfx645dl02HPgZSJJAVVfuIIVkKM7rMYeOXAc+bRr0lv18FlbviRlUUFDjnoQCw==";

    #[test]
    fn the_known_key_has_the_known_id_and_signature() {
        let k = test_key();
        assert_eq!(k.public_key_b64(), RFC8032_PUBLIC_B64);
        assert_eq!(k.public_key_b64().len(), 44);
        assert_eq!(k.submission_id(), "21fe31dfa154a261626bf854046fd227");
        // RFC 8032 TEST 1's signature over the empty message.
        assert_eq!(k.sign(b""), RFC8032_EMPTY_SIG_B64);
        assert_eq!(k.sign(b"x").len(), 88);
    }

    #[test]
    fn the_id_is_shown_in_groups_of_four() {
        assert_eq!(
            test_key().display_id(),
            "21fe-31df-a154-a261-626b-f854-046f-d227"
        );
        assert_eq!(display_id("abcdefgh"), "abcd-efgh");
        assert_eq!(display_id(""), "");
    }

    #[test]
    fn a_signature_verifies_over_its_body_only() {
        let k = Ed25519Key::generate();
        let body = br#"{"schema":1}"#;
        let sig = k.sign(body);
        assert!(verify(&k.public_key_b64(), body, &sig));
        assert!(!verify(&k.public_key_b64(), br#"{"schema":2}"#, &sig));
        assert!(!verify(&test_key().public_key_b64(), body, &sig));
        assert!(!verify("nope", body, &sig));
        assert!(!verify(&k.public_key_b64(), body, "AAAA"));
    }

    #[test]
    fn headers_carry_the_id_the_key_and_the_signature() {
        let k = test_key();
        let h = k.headers(b"body");
        assert_eq!(h[0], (SUBMISSION_HEADER.into(), k.submission_id()));
        assert_eq!(h[1], (KEY_HEADER.into(), k.public_key_b64()));
        assert_eq!(h[2].0, SIGNATURE_HEADER);
        assert!(verify(&h[1].1, b"body", &h[2].1));
    }

    #[test]
    fn minted_once_then_loaded() {
        let store = MemorySeedStore::default();
        let first = load_or_mint(&store).unwrap();
        assert_eq!(store.writes.get(), 1);
        let kept = store.seed.borrow().clone().unwrap();
        assert_eq!(STANDARD.decode(&kept).unwrap().len(), 32);
        let again = load_or_mint(&store).unwrap();
        assert_eq!(store.writes.get(), 1);
        assert_eq!(first.submission_id(), again.submission_id());
        // Two computers, two IDs.
        let other = load_or_mint(&MemorySeedStore::default()).unwrap();
        assert_ne!(other.submission_id(), first.submission_id());
    }

    #[test]
    fn a_kept_seed_that_isnt_one_is_an_error_and_left_alone() {
        for bad in ["not base64!", "AAAA"] {
            let store = MemorySeedStore::default();
            *store.seed.borrow_mut() = Some(bad.into());
            let e = load_or_mint(&store).unwrap_err();
            assert!(matches!(e, IdentityError::Corrupt(_)), "{e}");
            assert_eq!(store.writes.get(), 0);
            assert_eq!(store.seed.borrow().as_deref(), Some(bad));
        }
    }

    #[test]
    fn an_unreadable_or_unwritable_keychain_is_an_error() {
        let store = MemorySeedStore::default();
        store.fail_reads.set(true);
        assert!(matches!(
            load_or_mint(&store),
            Err(IdentityError::Unreadable(_))
        ));
        assert_eq!(store.writes.get(), 0);
        let store = MemorySeedStore::default();
        store.fail_writes.set(true);
        assert!(matches!(
            load_or_mint(&store),
            Err(IdentityError::Unwritable(_))
        ));
    }

    #[test]
    fn loading_never_mints() {
        let store = MemorySeedStore::default();
        assert!(load(&store).unwrap().is_none());
        assert_eq!(store.writes.get(), 0);
        let minted = load_or_mint(&store).unwrap();
        assert_eq!(
            load(&store).unwrap().unwrap().submission_id(),
            minted.submission_id()
        );
    }

    #[test]
    fn keychain_accounts_are_per_api() {
        let d = KeyringSeedStore::for_api(crate::config::DEFAULT_API);
        assert_eq!(d.account, "device-key");
        assert_eq!(
            KeyringSeedStore::for_api("https://api.ryna.io/").account,
            "device-key"
        );
        let l = KeyringSeedStore::for_api("http://localhost:8797");
        assert_eq!(l.account, "device-key http://localhost:8797");
    }

    #[test]
    fn debug_never_shows_the_seed() {
        let shown = format!("{:?}", test_key());
        assert!(shown.contains("21fe31df"), "{shown}");
        assert!(!shown.contains("nWGxne"), "{shown}");
    }
}
