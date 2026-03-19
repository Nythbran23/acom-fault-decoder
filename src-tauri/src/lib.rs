#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod database;
mod decoder;
mod serial;

use std::sync::Mutex;
use tauri::Manager;
use serial::SerialHandle;

pub struct AppState {
    pub serial_handle: Mutex<Option<SerialHandle>>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
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
            commands::decode_legacy,
            commands::get_app_version,
            commands::open_signatures_folder,
            commands::get_signatures_dir,
            commands::read_signature_file,
            commands::save_and_open_report,
        ])
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::Destroyed = event {
                let state = window.state::<AppState>();
                let mut handle = state.serial_handle.lock().unwrap();
                if let Some(h) = handle.as_mut() {
                    h.stop();
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running ACOM Fault Decoder");
}
