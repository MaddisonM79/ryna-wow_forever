//! Updates: Tauri's updater against this repository's GitHub Releases (`latest.json`, each
//! artifact checked against the minisign public key baked in at build time). The app carries
//! the addon, so a new app is also a new addon to offer.
//!
//! A build made without `agent/app/updater.pub` has an empty `plugins.updater.pubkey` (see
//! `agent/app/scripts/tauri.mjs`): it never checks, and says updates aren't set up.

use std::sync::Mutex;

use tauri::utils::config::PluginConfig;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_updater::{Update, UpdaterExt};

use crate::agent;

/// `{ downloaded, total }` while an update downloads.
pub const UPDATE_PROGRESS: &str = "update-progress";

/// The update the last check found, kept for "Update and restart".
#[derive(Default)]
pub struct Updates {
    pending: Mutex<Option<Update>>,
}

/// What a check found.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCheck {
    /// The build carries an updater public key; without one it never checks.
    pub configured: bool,
    /// This app's version.
    pub current: String,
    /// The newer release, when there is one.
    pub available: Option<Available>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Available {
    pub version: String,
    pub notes: Option<String>,
    /// RFC 3339, when the release says.
    pub date: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub downloaded: u64,
    pub total: Option<u64>,
}

/// The updater's public key from the merged config, when the build was given one.
pub fn pubkey(plugins: &PluginConfig) -> Option<&str> {
    plugins
        .0
        .get("updater")?
        .get("pubkey")?
        .as_str()
        .map(str::trim)
        .filter(|k| !k.is_empty())
}

pub fn configured(app: &AppHandle) -> bool {
    pubkey(&app.config().plugins).is_some()
}

fn current_version(app: &AppHandle) -> String {
    app.package_info().version.to_string()
}

fn updater(app: &AppHandle) -> Result<tauri_plugin_updater::Updater, String> {
    let handle = app.clone();
    app.updater_builder()
        // Windows: the installer runs and the process exits without returning; stop the loop
        // first so what's open is queued, as on Quit.
        .on_before_exit(move || agent::shutdown(&handle))
        .build()
        .map_err(|e| e.to_string())
}

pub async fn check(app: &AppHandle) -> Result<UpdateCheck, String> {
    let current = current_version(app);
    if !configured(app) {
        return Ok(UpdateCheck {
            configured: false,
            current,
            available: None,
        });
    }
    let found = updater(app)?.check().await.map_err(|e| e.to_string())?;
    let available = found.as_ref().map(|u| Available {
        version: u.version.clone(),
        notes: u.body.clone().filter(|b| !b.trim().is_empty()),
        // As the manifest wrote it (latest.json's `pub_date`).
        date: u
            .raw_json
            .get("pub_date")
            .and_then(|d| d.as_str())
            .map(str::to_string),
    });
    *app.state::<Updates>().pending.lock().unwrap() = found;
    Ok(UpdateCheck {
        configured: true,
        current,
        available,
    })
}

/// Downloads the update the last check found (checking again if there isn't one), installs it
/// and starts the new version. On Windows the installer takes over and the process exits
/// inside `install`; on macOS the bundle is replaced in place and the app restarts here.
pub async fn install(app: &AppHandle) -> Result<(), String> {
    if !configured(app) {
        return Err("Updates aren't set up in this build.".into());
    }
    let pending = app.state::<Updates>().pending.lock().unwrap().take();
    let update = match pending {
        Some(u) => u,
        None => updater(app)?
            .check()
            .await
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "Ryna is already up to date.".to_string())?,
    };
    let mut downloaded: u64 = 0;
    let progress = app.clone();
    update
        .download_and_install(
            move |chunk, total| {
                downloaded += chunk as u64;
                let _ = progress.emit(UPDATE_PROGRESS, Progress { downloaded, total });
            },
            || {},
        )
        .await
        .map_err(|e| e.to_string())?;
    agent::shutdown(app);
    app.restart();
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn plugins(v: serde_json::Value) -> PluginConfig {
        serde_json::from_value(v).unwrap()
    }

    #[test]
    fn pubkey_only_when_the_build_was_given_one() {
        assert_eq!(pubkey(&plugins(json!({}))), None);
        assert_eq!(
            pubkey(&plugins(json!({ "updater": { "pubkey": "" } }))),
            None
        );
        assert_eq!(
            pubkey(&plugins(json!({ "updater": { "pubkey": "  \n" } }))),
            None
        );
        assert_eq!(
            pubkey(&plugins(
                json!({ "updater": { "pubkey": " dW50cnVzdGVk\n" } })
            )),
            Some("dW50cnVzdGVk")
        );
    }

    #[test]
    fn the_shipped_config_has_no_key_and_points_at_releases() {
        let conf: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        let updater = &conf["plugins"]["updater"];
        assert_eq!(updater["pubkey"], json!(""));
        assert_eq!(
            updater["endpoints"],
            json!([
                "https://github.com/MaddisonM79/ryna_agent/releases/latest/download/latest.json"
            ])
        );
        // Artifacts are switched on by scripts/tauri.mjs only when a key is there to sign them.
        assert!(conf["bundle"].get("createUpdaterArtifacts").is_none());
    }
}
