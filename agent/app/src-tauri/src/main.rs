//! Ryna, the desktop companion: a window and a tray icon over the agent's core. It syncs only
//! while it is open; closing the window hides it to the tray, and Quit stops everything.

// No console window behind the app on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod agent;
mod commands;
mod game;
mod settings;
mod updates;

use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{App, AppHandle, Manager, RunEvent, WindowEvent};
use tauri_plugin_autostart::MacosLauncher;

fn show_main(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

fn quit(app: &AppHandle) {
    agent::shutdown(app);
    app.exit(0);
}

fn build_tray(app: &App) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "Open Ryna", true, None::<&str>)?;
    let sync = MenuItem::with_id(app, "sync", "Sync now", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "Quit Ryna", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &sync, &quit_item])?;
    let mut tray = TrayIconBuilder::with_id("main")
        .tooltip("Ryna")
        .menu(&menu)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => show_main(app),
            "sync" => agent::sync_now(app),
            "quit" => quit(app),
            _ => {}
        });
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.build(app)?;
    Ok(())
}

fn on_run_event(app: &AppHandle, event: &RunEvent) {
    if matches!(event, RunEvent::Exit) {
        agent::shutdown(app);
    }
    // macOS: clicking the Dock icon while the window is hidden brings it back.
    #[cfg(target_os = "macos")]
    let reopened = matches!(event, RunEvent::Reopen { .. });
    #[cfg(not(target_os = "macos"))]
    let reopened = false;
    if reopened {
        show_main(app);
    }
}

fn main() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        // Always registered: a build without a key has an empty pubkey, and `updates` never
        // calls it then.
        .plugin(tauri_plugin_updater::Builder::new().build())
        // Off until the person turns it on in Settings. On macOS it's a login item (through
        // System Events), not a launchd job: the app opens at login, nothing else runs.
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::AppleScript,
            None,
        ))
        .setup(|app| {
            let config = agent::config();
            let signed_in = agent::signed_in(&config);
            app.manage(agent::Agent::new(config, app.path().app_config_dir().ok()));
            app.manage(updates::Updates::default());
            build_tray(app)?;
            if signed_in {
                agent::start_watch(app.handle());
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            // Closing the window keeps the app (and the syncing) running in the tray.
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_status,
            commands::start_login,
            commands::cancel_login,
            commands::open_sign_in_page,
            commands::logout,
            commands::start_watch,
            commands::stop_watch,
            commands::sync_now,
            commands::set_install_override,
            commands::set_combat_settings,
            commands::install_addon,
            commands::set_autostart,
            commands::open_portal,
            commands::check_for_update,
            commands::install_update,
        ])
        .build(tauri::generate_context!())
        .expect("the app failed to start");
    app.run(|app, event| on_run_event(app, &event));
}
