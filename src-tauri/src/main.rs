// ============================================================================
// main.rs — ACOM Fault Decoder — Tauri 2 application entry point.
// ============================================================================

// Prevents a second terminal window appearing on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::Mutex;
use crate::serial::SerialHandle;

mod commands;
mod database;
mod decoder;
mod serial;

// ============================================================================
// Application state — shared across all commands via Tauri's State extractor.
// ============================================================================
pub struct AppState {
    /// Active serial capture handle.  None when not connected.
    pub serial_handle: Mutex<Option<SerialHandle>>,
}

// ============================================================================
// Entry point
// ============================================================================
fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_log::Builder::default().build())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState {
            serial_handle: Mutex::new(None),
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_serial_ports,
            commands::connect_serial,
            commands::disconnect_serial,
            commands::decode_signature,
            commands::auto_save_signature,
            commands::save_report,
        ])
        .on_window_event(|window, event| {
            // Ensure serial port is closed cleanly when the window is destroyed.
            if let tauri::WindowEvent::Destroyed = event {
                let state = window.state::<AppState>();
                let mut handle = state.serial_handle.lock().unwrap();
                if let Some(ref mut h) = *handle {
                    h.stop();
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running ACOM Fault Decoder");
}
