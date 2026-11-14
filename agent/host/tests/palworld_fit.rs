//! The host against the real Palworld sidecar: a package built by `scripts/core-package.mjs` and
//! signed by `tauri signer`, fetched, checked and unpacked by the host's own code, then run as
//! `node agent.mjs serve` for one pass against an API address that refuses connections, with an
//! empty game folder (so it reads no saves) and a fake token.
//!
//! Ignored by default: it needs the package, which CI doesn't build. Run it with
//!
//! ```text
//! RYNA_CORE_PACKAGES=<folder with the .tar.gz and .sig> RYNA_FIT_PUBKEY=<the key's .pub file> \
//!   cargo test -p ryna-agent --test palworld_fit -- --ignored --nocapture
//! ```

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use ryna_agent::auth::{AccessToken, AuthError, TokenStore};
use ryna_agent::config::Config;
use ryna_agent::core::{Chip, Core, CoreContext, CoreControl, TokenProvider};
use ryna_agent::package::{self, Source};
use ryna_agent::sidecar::{SidecarCore, SidecarEvent, DEBUG_LOG};

const FAKE_TOKEN: &str = "fake-token-FIT-5b8e0c1d9a";
/// Nothing listens on the discard port here: every call the sidecar makes is refused at once.
const NO_API: &str = "http://127.0.0.1:9";

struct NoStore;
impl TokenStore for NoStore {
    fn refresh_token(&self) -> Result<Option<String>, String> {
        Ok(None)
    }
    fn set_refresh_token(&self, _: &str) -> Result<(), String> {
        Ok(())
    }
    fn access_token(&self) -> Option<AccessToken> {
        None
    }
    fn set_access_token(&self, _: &AccessToken) -> Result<(), String> {
        Ok(())
    }
    fn clear(&self) -> Result<(), String> {
        Ok(())
    }
}

struct FakeToken(Config);
impl TokenProvider for FakeToken {
    fn config(&self) -> &Config {
        &self.0
    }
    fn store(&self) -> Box<dyn TokenStore> {
        Box::new(NoStore)
    }
    fn access_token(&self) -> Result<Option<String>, AuthError> {
        Ok(Some(FAKE_TOKEN.into()))
    }
}

#[test]
#[ignore = "needs a built Palworld package: RYNA_CORE_PACKAGES and RYNA_FIT_PUBKEY"]
fn the_real_palworld_sidecar_fits_the_host() {
    std::env::var(package::PACKAGES_ENV).expect("RYNA_CORE_PACKAGES names the package folder");
    let key_file = std::env::var("RYNA_FIT_PUBKEY").expect("RYNA_FIT_PUBKEY names the key.pub");
    let key = std::fs::read_to_string(key_file).unwrap();
    let version = env!("CARGO_PKG_VERSION");

    // Fetch, check and unpack, exactly as the app does.
    let state = tempfile::tempdir().unwrap();
    let source = Source::for_version(version).unwrap();
    assert!(matches!(source, Source::Folder(_)), "{source:?}");
    let mut progress = Vec::new();
    let (dir, manifest) = package::fetch(
        state.path(),
        "palworld",
        version,
        &source,
        &key,
        &mut |d, t| progress.push((d, t)),
    )
    .unwrap_or_else(|e| panic!("fetch: {e}"));
    println!(
        "unpacked to {} ({} progress calls)",
        dir.display(),
        progress.len()
    );
    println!("manifest: {manifest:?}");
    assert_eq!(manifest.id, "palworld");
    assert_eq!(manifest.version, version);
    assert_eq!(
        manifest.platforms.as_deref(),
        Some(&["macos".to_string()][..])
    );

    // Run it: one sync, then stop.
    let game = tempfile::tempdir().unwrap();
    let core = SidecarCore::new(manifest, dir.clone(), NO_API);
    let ctx = CoreContext {
        state_dir: package::core_dir(state.path(), "palworld"),
        tokens: Arc::new(FakeToken(Config {
            issuer: "http://127.0.0.1:9".into(),
            api: NO_API.into(),
            client_id: "fit".into(),
            resource: "https://api.ryna.io/agent".into(),
        })),
        device_key: None,
        game_folder: Some(game.path().to_path_buf()),
    };
    let control = CoreControl::new();
    let events: Arc<Mutex<Vec<SidecarEvent>>> = Arc::default();
    let (c, e, ctx2) = (control.clone(), events.clone(), ctx.clone());
    let core_rows = Arc::new(core);
    let runner = core_rows.clone();
    let thread = std::thread::spawn(move || {
        runner
            .start(&ctx2, &c, &mut |ev| e.lock().unwrap().push(ev))
            .map_err(|e| e.to_string())
    });
    let deadline = Instant::now() + Duration::from_secs(60);
    while !events
        .lock()
        .unwrap()
        .iter()
        .any(|e| matches!(e, SidecarEvent::PassFinished { .. }))
    {
        assert!(
            Instant::now() < deadline,
            "no done: {:#?}",
            events.lock().unwrap()
        );
        assert!(
            !thread.is_finished(),
            "it stopped: {:#?}",
            events.lock().unwrap()
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    let stopping = Instant::now();
    control.stop();
    let ended = thread.join().unwrap();
    let took = stopping.elapsed();

    let events = events.lock().unwrap().clone();
    println!("--- the pass, as the host saw it ---");
    for ev in &events {
        println!("{}", serde_json::to_string(ev).unwrap());
    }
    let rows = core_rows.status_rows(&ctx);
    println!("--- status rows ---");
    for r in &rows {
        println!("{r:?}");
    }
    let log_path = ctx.state_dir.join(DEBUG_LOG);
    let log = std::fs::read_to_string(&log_path).unwrap_or_default();
    println!("--- {} ---\n{log}", log_path.display());
    println!("stopped in {took:?}: {ended:?}");

    assert_eq!(ended, Ok(()));
    assert!(took < Duration::from_secs(5), "{took:?}");
    assert_eq!(
        events.first(),
        Some(&SidecarEvent::Started {
            version: version.into()
        })
    );
    assert!(events
        .iter()
        .any(|e| matches!(e, SidecarEvent::PassFinished { .. })));
    assert!(
        rows.iter()
            .any(|r| r.value == "no worlds found" && r.chip == Some(Chip::Waiting)),
        "{rows:?}"
    );
    // `ready` named this core (the host logs it when it names another).
    assert!(!log.contains("ready names another core"), "{log}");
    let shown = serde_json::to_string(&events).unwrap();
    assert!(!shown.contains(FAKE_TOKEN), "{shown}");
    assert!(!log.contains(FAKE_TOKEN), "{log}");
}
