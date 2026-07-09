// Prevents an extra console window on Windows in release.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod sidecar;
mod state;

use state::AppState;

fn main() {
    let app_state = AppState::init().expect("failed to initialize FitLLM state");

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            commands::get_hardware_profile,
            commands::get_recommendations,
            commands::get_catalog,
            commands::get_consent,
            commands::set_consent,
            commands::start_quick_benchmark,
            commands::export_data,
            commands::wipe_data,
        ])
        .run(tauri::generate_context!())
        .expect("error while running FitLLM");
}
