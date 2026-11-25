mod commands;
mod state;
mod tray;

use state::AppState;
use tauri::{Manager, RunEvent};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt::init();

    let app_state = AppState::initialize().expect("Failed to initialize Portline state");

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(win) = app.get_webview_window("main") {
                let _ = win.show();
                let _ = win.unminimize();
                let _ = win.set_focus();
            }
        }))
        .manage(app_state)
        .setup(|app| {
            let _ = tray::setup_tray(app.handle());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_bootstrap_state,
            commands::import_nodes,
            commands::import_subscription,
            commands::refresh_subscription,
            commands::delete_subscription,
            commands::save_node,
            commands::delete_node,
            commands::select_main_node,
            commands::set_route_mode,
            commands::toggle_system_proxy,
            commands::toggle_tun,
            commands::save_rule,
            commands::delete_rule,
            commands::save_inbound_port,
            commands::delete_inbound_port,
            commands::update_settings,
            commands::compile_preview,
            commands::run_preflight_check,
            commands::start_core,
            commands::stop_core,
            commands::download_official_core,
            commands::import_core_binary,
            commands::run_batch_tcping,
            commands::run_batch_real_delay,
            commands::get_live_monitor,
            commands::close_connection,
            commands::close_all_connections,
            commands::clear_logs,
            commands::restore_netguard_now,
        ])
        .build(tauri::generate_context!())
        .expect("Failed to build Portline desktop application");

    app.run(|app_handle, event| {
        if let RunEvent::Exit = event {
            if let Some(state) = app_handle.try_state::<AppState>() {
                state.shutdown_cleanly();
            }
        }
    });
}
