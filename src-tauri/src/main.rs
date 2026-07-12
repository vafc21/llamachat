// Prevents an extra console window on Windows in release.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod agent;
mod commands;
mod desktop;
mod memory;
mod ollama;
mod settings;
mod sidecar;
mod state;

use state::AppState;
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    Manager,
};

fn main() {
    let app_state = AppState::init().expect("failed to initialize LlamaChat state");

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(app_state)
        .setup(|app| {
            // System-tray menu — the always-available "Jarvis-style" entry point.
            let open = MenuItem::with_id(app, "open", "Open LlamaChat", true, None::<&str>)?;
            let bench =
                MenuItem::with_id(app, "benchmark", "Run Quick Benchmark", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit LlamaChat", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&open, &bench, &quit])?;

            TrayIconBuilder::with_id("main")
                .icon(app.default_window_icon().unwrap().clone())
                .tooltip("LlamaChat")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "open" => {
                        if let Some(w) = app.get_webview_window("main") {
                            let _ = w.show();
                            let _ = w.unminimize();
                            let _ = w.set_focus();
                        }
                    }
                    "benchmark" => commands::start_quick_benchmark(app.clone()),
                    "quit" => app.exit(0),
                    _ => {}
                })
                .build(app)?;

            // Warm up Ollama in the background so the first model download / chat
            // doesn't pay the server cold-start cost.
            std::thread::spawn(|| {
                let _ = ollama::ensure_running();
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_hardware_profile,
            commands::get_recommendations,
            commands::get_benchmark_plan,
            commands::get_catalog,
            commands::get_consent,
            commands::set_consent,
            commands::start_quick_benchmark,
            commands::start_benchmark,
            commands::export_data,
            commands::wipe_data,
            commands::get_settings,
            commands::set_settings,
            commands::add_custom_model,
            commands::remove_custom_model,
            commands::download_model,
            commands::list_installed_models,
            commands::save_conversation,
            commands::list_conversations,
            commands::delete_conversation,
            commands::get_memory,
            commands::set_memory,
            commands::get_memory_dir,
            commands::run_agent,
            commands::approve_agent,
            commands::stop_agent,
            commands::check_permissions,
            commands::request_accessibility,
            commands::open_settings_pane,
            commands::list_tools,
            commands::execute_tool,
            commands::tool_needs_approval,
            commands::get_tool_system_prompt,
            commands::send_message,
        ])
        .run(tauri::generate_context!())
        .expect("error while running LlamaChat");
}
