//! Ryna, the desktop companion: a window and a tray icon over the agent's core. It syncs only
//! while it is open; closing the window hides it to the tray, and Quit stops everything.

// No console window behind the app on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod addon_settings;
mod agent;
mod commands;
mod cores;
mod frame;
mod game;
mod settings;
mod updates;

use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{App, AppHandle, Manager, RunEvent, WindowEvent};
use tauri_plugin_autostart::MacosLauncher;
use tauri_plugin_window_state::StateFlags;

/// Whether the tray icon exists. Without one, closing the window quits (there'd be no way back).
struct Tray(bool);

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

/// The tray, if the system can show one. On Linux the tray library loads
/// libayatana-appindicator3 (or libappindicator3) at run time and panics when neither is
/// installed; the app goes on without a tray then, and says so (agent/README.md "Linux").
fn try_build_tray(app: &App) -> bool {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| build_tray(app))) {
        Ok(Ok(())) => true,
        Ok(Err(e)) => {
            eprintln!("ryna: no tray icon ({e}); closing the window will quit");
            false
        }
        Err(_) => {
            eprintln!(
                "ryna: no tray icon: no appindicator library (on Linux, install libayatana-appindicator); closing the window will quit"
            );
            false
        }
    }
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
        // Always registered. `agent/app/updater.pub` is tracked, so every build through
        // `pnpm tauri` (dev or build, from any clone) carries the production public key and will
        // offer production-signed releases; only a plain `cargo build`, which reads
        // tauri.conf.json alone, has an empty pubkey, and `updates` never calls it then.
        .plugin(tauri_plugin_updater::Builder::new().build())
        // Off until the person turns it on in Settings. On macOS it's a login item (through
        // System Events), not a launchd job: the app opens at login, nothing else runs.
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::AppleScript,
            None,
        ))
        // The window's size and position come back next time. Not whether it was visible: it
        // is hidden to the tray at quit more often than not, and must still open at launch.
        .plugin(
            tauri_plugin_window_state::Builder::new()
                .with_state_flags(StateFlags::SIZE | StateFlags::POSITION | StateFlags::MAXIMIZED)
                .build(),
        )
        .setup(|app| {
            let config = agent::config().inspect_err(|e| eprintln!("ryna: {e}"))?;
            let signed_in = agent::signed_in(&config);
            app.manage(agent::Agent::new(config, app.path().app_config_dir().ok()));
            app.manage(std::sync::Mutex::new(cores::Sidecars::default()));
            app.manage(updates::Updates::default());
            let tray = try_build_tray(app);
            app.manage(Tray(tray));
            if signed_in {
                agent::start_watch(app.handle());
            }
            // The cores answer: now, then hourly; it fetches and starts each fetched core.
            cores::begin(app.handle());
            // The window is created hidden (tauri.conf.json `visible: false`) and the plugin has
            // restored its frame by now; show it once the frame is known to fit a monitor.
            if let Some(window) = app.get_webview_window("main") {
                frame::place_and_show(&window);
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            // Closing the window keeps the app (and the syncing) running in the tray; with no
            // tray to come back from, it quits (RunEvent::Exit stops the loop).
            if let WindowEvent::CloseRequested { api, .. } = event {
                if window.try_state::<Tray>().is_none_or(|t| t.0) {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_status,
            commands::get_submission_id,
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
            commands::set_mod_enabled,
            commands::set_core_enabled,
            commands::retry_core_fetch,
            commands::set_autostart,
            commands::open_portal,
            commands::open_link,
            commands::check_for_update,
            commands::install_update,
            commands::settings_view,
            commands::settings_game_running,
            commands::settings_diff,
            commands::settings_copy,
            commands::settings_backup,
            commands::settings_backups,
            commands::settings_restore,
            commands::settings_delete,
        ])
        .build(tauri::generate_context!())
        .expect("the app failed to start");
    app.run(|app, event| on_run_event(app, &event));
}
