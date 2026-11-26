mod commands;
mod state;
mod tray;

use portline_core::control::ControlClient;
use portline_core::model::RouteMode;
use state::AppState;
use std::time::Duration;
use tauri::{Emitter, Manager, RunEvent, WindowEvent};

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
        .on_window_event(|window, event| {
            if window.label() == "main" {
                if let WindowEvent::CloseRequested { api, .. } = event {
                    let minimize_to_tray = window
                        .app_handle()
                        .try_state::<AppState>()
                        .and_then(|st| st.store.lock().ok().and_then(|s| s.load_settings().ok()))
                        .map(|s| s.minimize_to_tray)
                        .unwrap_or(true);
                    if minimize_to_tray {
                        api.prevent_close();
                        let _ = window.hide();
                    }
                }
            }
        })
        .setup(|app| {
            let _ = tray::setup_tray(app.handle());

            // When launched via OS login autostart (--autostart), keep the main window hidden
            // in the system tray (when minimize_to_tray is enabled) and auto-start the core.
            let launched_via_autostart =
                std::env::args().any(|a| a == portline_core::autostart::AUTOSTART_ARG);
            if launched_via_autostart {
                if let Some(state) = app.try_state::<AppState>() {
                    let startup_cfg = state
                        .store
                        .lock()
                        .ok()
                        .and_then(|s| {
                            let settings = s.load_settings().ok()?;
                            let has_nodes = !s.list_nodes().unwrap_or_default().is_empty();
                            Some((settings, has_nodes))
                        });
                    if let Some((settings, has_nodes)) = startup_cfg {
                        if settings.minimize_to_tray {
                            if let Some(win) = app.get_webview_window("main") {
                                let _ = win.hide();
                            }
                        }
                        if settings.auto_start_core
                            && has_nodes
                            && state.core_manager.is_installed()
                        {
                            let start_handle = app.handle().clone();
                            tauri::async_runtime::spawn(async move {
                                let Some(st) = start_handle.try_state::<AppState>() else {
                                    return;
                                };
                                let Some(bin) = st.core_manager.resolve_binary_path() else {
                                    return;
                                };
                                let Ok(compiled) = st.compile_current() else {
                                    return;
                                };
                                let started_ok = st
                                    .supervisor
                                    .lock()
                                    .ok()
                                    .and_then(|mut sup| sup.apply_config(&bin, compiled.config).ok())
                                    .map(|r| r.ok)
                                    .unwrap_or(false);
                                if started_ok {
                                    if let Ok(client) =
                                        ControlClient::new(settings.control_port, &st.control_secret)
                                    {
                                        if client.wait_ready(2000).await {
                                            let clash_mode = match settings.route_mode {
                                                RouteMode::Direct => "direct",
                                                RouteMode::Global => "global",
                                                RouteMode::Rule => "rule",
                                            };
                                            let _ = client.set_mode(clash_mode).await;
                                            if let Some(ref main_id) = settings.main_node_id {
                                                let _ = client
                                                    .select_proxy("main-exit", main_id)
                                                    .await;
                                            }
                                        }
                                    }
                                    if settings.system_proxy_enabled {
                                        if let Ok(guard) = st.netguard.lock() {
                                            let _ = guard.enable_system_proxy(
                                                "127.0.0.1",
                                                settings.mixed_port,
                                            );
                                        }
                                    }
                                    st.push_log(
                                        "INFO",
                                        "autostart: core started silently in background",
                                    );
                                    let _ = start_handle.emit("portline-state-sync", ());
                                }
                            });
                        }
                    }
                }
            }

            // Auto-sync missing classic .srs binary rule-sets in the background on startup
            // so geosite/geoip rules activate seamlessly without blocking launch.
            let srs_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let Some(state) = srs_handle.try_state::<AppState>() else {
                    return;
                };
                let (missing_tags, proxy_port) = {
                    let Ok(store) = state.store.lock() else {
                        return;
                    };
                    let rules = store.list_rules().unwrap_or_default();
                    let settings = store.load_settings().unwrap_or_default();
                    let running = state
                        .supervisor
                        .lock()
                        .map(|mut s| s.is_running())
                        .unwrap_or(false);
                    let mut tags: Vec<String> = Vec::new();
                    for r in rules.iter().filter(|r| r.enabled) {
                        if let Some(ref tag) = r.rule_set {
                            let clean = tag.trim();
                            if !clean.is_empty()
                                && state.rule_cache.local_srs_path(clean).is_none()
                                && !tags.iter().any(|x| x == clean)
                            {
                                tags.push(clean.to_string());
                            }
                        }
                    }
                    (tags, if running { Some(settings.mixed_port) } else { None })
                };
                if missing_tags.is_empty() {
                    return;
                }
                let mut synced = 0usize;
                for tag in &missing_tags {
                    let st = state.rule_cache.refresh_rule_set_auto(tag, proxy_port).await;
                    if st.cached_locally && !st.fallback_mode {
                        synced += 1;
                    }
                }
                if synced > 0 {
                    state.push_log(
                        "INFO",
                        format!("rule-sets: auto-synced {}/{} .srs files", synced, missing_tags.len()),
                    );
                    let _ = state.reload_core_if_running();
                    let _ = srs_handle.emit("portline-state-sync", ());
                }
            });

            // Lightweight background collector: persists closed connections and updates tray tooltip
            // even while Portline is minimized to the system tray.
            let bg_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let mut interval = tokio::time::interval(Duration::from_secs(2));
                loop {
                    interval.tick().await;
                    let Some(state) = bg_handle.try_state::<AppState>() else {
                        continue;
                    };
                    let running = state
                        .supervisor
                        .lock()
                        .map(|mut s| s.is_running())
                        .unwrap_or(false);
                    let (control_port, sys_proxy, main_node_name) = state
                        .store
                        .lock()
                        .ok()
                        .and_then(|store| {
                            let settings = store.load_settings().ok()?;
                            let nodes = store.list_nodes().ok().unwrap_or_default();
                            let active = settings
                                .main_node_id
                                .as_deref()
                                .and_then(|id| nodes.iter().find(|n| n.id == id))
                                .or_else(|| nodes.first())
                                .map(|n| n.remark.clone())
                                .unwrap_or_default();
                            Some((settings.control_port, settings.system_proxy_enabled, active))
                        })
                        .unwrap_or((9090, false, String::new()));

                    if running {
                        if let Ok(client) = ControlClient::new(control_port, &state.control_secret)
                        {
                            if let Ok((conns, _, _, _)) = client.get_connections().await {
                                state.record_connections_snapshot(&conns);
                            }
                        }
                        let tip = if main_node_name.is_empty() {
                            if sys_proxy {
                                "Portline - Running (System Proxy)".to_string()
                            } else {
                                "Portline - Running".to_string()
                            }
                        } else if sys_proxy {
                            format!("Portline - {} (Proxy ON)", main_node_name)
                        } else {
                            format!("Portline - {}", main_node_name)
                        };
                        tray::update_tray_tooltip(&bg_handle, &tip);
                    } else {
                        tray::update_tray_tooltip(&bg_handle, "Portline - Stopped");
                    }
                }
            });

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
            commands::list_rule_presets,
            commands::list_rule_sets,
            commands::apply_routing_strategy,
            commands::sync_rule_sets,
            commands::test_route_target,
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
            commands::get_connection_history,
            commands::get_platform_traffic,
            commands::clear_connection_history,
            commands::close_connection,
            commands::close_all_connections,
            commands::clear_logs,
            commands::restore_netguard_now,
            commands::hide_to_tray,
            commands::show_main_window,
            commands::hide_tray_popup,
            commands::quit_app,
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
