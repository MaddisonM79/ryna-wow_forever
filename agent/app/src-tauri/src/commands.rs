//! What the window calls. Anything that touches the keychain, the disk or the network runs on
//! the blocking pool, so the window never waits on it.

use std::path::PathBuf;

use tauri::AppHandle;
use tauri_plugin_autostart::ManagerExt as _;
use tauri_plugin_opener::OpenerExt as _;

use crate::agent::{self, Installed, LoginCode, Status};

/// The portal's page for this app.
pub const PORTAL_URL: &str = "https://ryna.io/wow";

async fn blocking<T, F>(f: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, String> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| e.to_string())?
}

/// Everything the window shows, plus whether launch at login is on.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusWithSettings {
    #[serde(flatten)]
    pub status: Status,
    pub autostart: bool,
}

#[tauri::command]
pub async fn get_status(app: AppHandle) -> Result<StatusWithSettings, String> {
    blocking(move || {
        let autostart = app.autolaunch().is_enabled().unwrap_or(false);
        Ok(StatusWithSettings {
            status: agent::status(&app),
            autostart,
        })
    })
    .await
}

/// Begins the device grant: the code to show and the page to check it on. Polling carries on
/// in the background; `login-finished` says how it ended.
#[tauri::command]
pub async fn start_login(app: AppHandle) -> Result<LoginCode, String> {
    blocking(move || agent::start_login(&app)).await
}

/// Opens the pending sign-in's page in the browser.
#[tauri::command]
pub fn open_sign_in_page(app: AppHandle) -> Result<(), String> {
    let link = agent::login_link(&app).ok_or_else(|| "No sign-in is waiting.".to_string())?;
    app.opener()
        .open_url(link, None::<&str>)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn logout(app: AppHandle) -> Result<(), String> {
    blocking(move || agent::logout(&app)).await
}

#[tauri::command]
pub fn start_watch(app: AppHandle) {
    agent::start_watch(&app);
}

#[tauri::command]
pub fn stop_watch(app: AppHandle) {
    agent::stop_watch(&app);
}

#[tauri::command]
pub fn sync_now(app: AppHandle) {
    agent::sync_now(&app);
}

/// A folder to look in for the game, or null to go back to the usual places.
#[tauri::command]
pub async fn set_install_override(app: AppHandle, path: Option<String>) -> Result<(), String> {
    blocking(move || {
        let path = path
            .map(|p| p.trim().to_string())
            .filter(|p| !p.is_empty())
            .map(PathBuf::from);
        agent::set_install_override(&app, path)
    })
    .await
}

#[tauri::command]
pub async fn install_addon(app: AppHandle) -> Result<Vec<Installed>, String> {
    blocking(move || agent::install_addon(&app)).await
}

/// Turns launch at login on or off; returns what the OS now says.
#[tauri::command]
pub async fn set_autostart(app: AppHandle, enabled: bool) -> Result<bool, String> {
    blocking(move || {
        let manager = app.autolaunch();
        let changed = if enabled {
            manager.enable()
        } else {
            manager.disable()
        };
        changed.map_err(|e| e.to_string())?;
        manager.is_enabled().map_err(|e| e.to_string())
    })
    .await
}

#[tauri::command]
pub fn open_portal(app: AppHandle) -> Result<(), String> {
    app.opener()
        .open_url(PORTAL_URL, None::<&str>)
        .map_err(|e| e.to_string())
}
