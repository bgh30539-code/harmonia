//! System tray integration.
//!
//! The tray provides always-available playback controls (play/pause, next,
//! previous), a way to bring the main window back when it is hidden to the
//! tray, and a Quit action. The tray is built in Rust so the menu can send
//! commands straight into the audio engine thread.

use tauri::menu::{IsMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager};

use crate::engine::AudioCommand;
use crate::state::AppState;

pub fn build_tray(app: &tauri::App) -> tauri::Result<()> {
    let play_pause = MenuItem::with_id(app, "play_pause", "Play/Pause", true, None::<&str>)?;
    let next = MenuItem::with_id(app, "next", "Next track", true, None::<&str>)?;
    let previous = MenuItem::with_id(app, "previous", "Previous track", true, None::<&str>)?;
    let show = MenuItem::with_id(app, "show", "Show / Hide", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let quit = PredefinedMenuItem::quit(app, Some("Quit"))?;

    let items: Vec<&dyn IsMenuItem<tauri::Wry>> =
        vec![&play_pause, &next, &previous, &separator, &show, &quit];
    let menu = Menu::with_items(app, &items)?;

    TrayIconBuilder::with_id("main")
        .icon(app.default_window_icon().unwrap().clone())
        .tooltip("Harmonia")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "play_pause" => send(app, AudioCommand::Toggle),
            "next" => send(app, AudioCommand::PlayNext),
            "previous" => send(app, AudioCommand::PlayPrevious),
            "show" => toggle_window(app),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { .. } = event {
                toggle_window(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}

fn send(app: &AppHandle, cmd: AudioCommand) {
    if let Some(state) = app.try_state::<AppState>() {
        let _ = state.engine_tx.send(cmd);
    }
}

fn toggle_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        if window.is_visible().unwrap_or(false) {
            let _ = window.hide();
        } else {
            let _ = window.show();
            let _ = window.set_focus();
        }
    }
}
