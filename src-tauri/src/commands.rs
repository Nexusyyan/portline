use crate::state::AppState;
use portline_core::compiler::SingBox114Compiler;
use portline_core::control::ControlClient;
use portline_core::model::{
    AppSettings, CompileOutput, ConnectionItem, CoreInfo, InboundPort, LatencyResult, LogEntry,
    NodeIR, PreflightReport, RouteMode, SimpleRoutingRule, SubscriptionGroup, TrafficSample,
};
use portline_core::parser::{parse_payload, ImportSummary};
use portline_core::tester::{batch_real_delay, batch_tcping};
use serde::Serialize;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use tauri::State;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BootstrapPayload {
    pub core_info: CoreInfo,
    pub settings: AppSettings,
    pub nodes: Vec<NodeIR>,
    pub subscriptions: Vec<SubscriptionGroup>,
    pub rules: Vec<SimpleRoutingRule>,
    pub extra_ports: Vec<InboundPort>,
    pub compiled: CompileOutput,
    pub logs: Vec<LogEntry>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveMonitorPayload {
    pub core_running: bool,
    pub connections: Vec<ConnectionItem>,
    pub traffic: TrafficSample,
    pub logs: Vec<LogEntry>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectNodeResult {
    pub hot_switched: bool,
    pub main_node_id: String,
    pub compiled: CompileOutput,
}

#[tauri::command]
pub fn get_bootstrap_state(state: State<'_, AppState>) -> Result<BootstrapPayload, String> {
    let (settings, nodes, subscriptions, rules, extra_ports) = {
        let store = state.store.lock().map_err(|_| "store lock poisoned".to_string())?;
        (
            store.load_settings().map_err(|e| e.to_string())?,
            store.list_nodes().map_err(|e| e.to_string())?,
            store.list_subscriptions().map_err(|e| e.to_string())?,
            store.list_rules().map_err(|e| e.to_string())?,
            store.list_inbound_ports().map_err(|e| e.to_string())?,
        )
    };
    let compiled = state.compile_current()?;
    let logs = state
        .logs
        .lock()
        .map(|l| l.snapshot())
        .unwrap_or_default();

    Ok(BootstrapPayload {
        core_info: state.build_core_info(),
        settings,
        nodes,
        subscriptions,
        rules,
        extra_ports,
        compiled,
        logs,
    })
}

#[tauri::command]
pub fn import_nodes(
    state: State<'_, AppState>,
    payload: String,
    group_name: Option<String>,
) -> Result<ImportSummary, String> {
    let group = group_name
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "Imported".to_string());
    let summary = parse_payload(&payload, &group).map_err(|e| e.to_string())?;
    if !summary.nodes.is_empty() {
        let mut store = state.store.lock().map_err(|_| "store lock poisoned".to_string())?;
        store.upsert_nodes(&summary.nodes).map_err(|e| e.to_string())?;
        let mut settings = store.load_settings().map_err(|e| e.to_string())?;
        if settings.main_node_id.is_none() {
            settings.main_node_id = Some(summary.nodes[0].id.clone());
            store.save_settings(&settings).map_err(|e| e.to_string())?;
        }
    }
    state.push_log(
        "INFO",
        format!(
            "import: added {} node(s) into group [{}], skipped {}",
            summary.nodes.len(),
            group,
            summary.skipped
        ),
    );
    Ok(summary)
}

#[tauri::command]
pub async fn import_subscription(
    state: State<'_, AppState>,
    name: String,
    url: String,
) -> Result<SubscriptionGroup, String> {
    let clean_url = url.trim().to_string();
    let clean_name = if name.trim().is_empty() {
        "Subscription".to_string()
    } else {
        name.trim().to_string()
    };

    let client = reqwest::Client::builder()
        .user_agent("Portline/0.1 sing-box")
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|e| e.to_string())?;
    let body = client
        .get(&clean_url)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?
        .text()
        .await
        .map_err(|e| e.to_string())?;

    let summary = parse_payload(&body, &clean_name).map_err(|e| e.to_string())?;
    let sub = SubscriptionGroup {
        id: format!("sub-{}", &uuid::Uuid::new_v4().simple().to_string()[..8]),
        name: clean_name.clone(),
        url: clean_url,
        auto_update_hours: 24,
        last_updated: Some(chrono::Utc::now().to_rfc3339()),
        node_count: summary.nodes.len(),
    };

    {
        let mut store = state.store.lock().map_err(|_| "store lock poisoned".to_string())?;
        store
            .replace_group_nodes(&clean_name, &summary.nodes)
            .map_err(|e| e.to_string())?;
        store.upsert_subscription(&sub).map_err(|e| e.to_string())?;
        let mut settings = store.load_settings().map_err(|e| e.to_string())?;
        if settings.main_node_id.is_none() {
            if let Some(first) = summary.nodes.first() {
                settings.main_node_id = Some(first.id.clone());
                store.save_settings(&settings).map_err(|e| e.to_string())?;
            }
        }
    }

    state.push_log(
        "INFO",
        format!(
            "subscription: synced [{}] ({} nodes)",
            clean_name,
            summary.nodes.len()
        ),
    );
    let _ = state.reload_core_if_running();
    Ok(sub)
}

#[tauri::command]
pub async fn refresh_subscription(
    state: State<'_, AppState>,
    id: String,
) -> Result<SubscriptionGroup, String> {
    let existing = {
        let store = state.store.lock().map_err(|_| "store lock poisoned".to_string())?;
        store
            .list_subscriptions()
            .map_err(|e| e.to_string())?
            .into_iter()
            .find(|s| s.id == id)
            .ok_or_else(|| "Subscription not found".to_string())?
    };

    let client = reqwest::Client::builder()
        .user_agent("Portline/0.1 sing-box")
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|e| e.to_string())?;
    let body = client
        .get(&existing.url)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?
        .text()
        .await
        .map_err(|e| e.to_string())?;

    let summary = parse_payload(&body, &existing.name).map_err(|e| e.to_string())?;
    let updated = SubscriptionGroup {
        last_updated: Some(chrono::Utc::now().to_rfc3339()),
        node_count: summary.nodes.len(),
        ..existing
    };

    {
        let mut store = state.store.lock().map_err(|_| "store lock poisoned".to_string())?;
        store
            .replace_group_nodes(&updated.name, &summary.nodes)
            .map_err(|e| e.to_string())?;
        store
            .upsert_subscription(&updated)
            .map_err(|e| e.to_string())?;
    }

    state.push_log(
        "INFO",
        format!(
            "subscription: refreshed [{}] ({} nodes)",
            updated.name, updated.node_count
        ),
    );
    let _ = state.reload_core_if_running();
    Ok(updated)
}

#[tauri::command]
pub fn delete_subscription(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let store = state.store.lock().map_err(|_| "store lock poisoned".to_string())?;
    store.delete_subscription(&id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn save_node(state: State<'_, AppState>, node: NodeIR) -> Result<(), String> {
    {
        let mut store = state.store.lock().map_err(|_| "store lock poisoned".to_string())?;
        store.upsert_nodes(&[node]).map_err(|e| e.to_string())?;
    }
    let _ = state.reload_core_if_running();
    Ok(())
}

#[tauri::command]
pub fn delete_node(state: State<'_, AppState>, id: String) -> Result<bool, String> {
    let deleted = {
        let store = state.store.lock().map_err(|_| "store lock poisoned".to_string())?;
        let ok = store.delete_node(&id).map_err(|e| e.to_string())?;
        let mut settings = store.load_settings().map_err(|e| e.to_string())?;
        if settings.main_node_id.as_deref() == Some(id.as_str()) {
            settings.main_node_id = store
                .list_nodes()
                .ok()
                .and_then(|list| list.first().map(|n| n.id.clone()));
            store.save_settings(&settings).map_err(|e| e.to_string())?;
        }
        ok
    };
    let _ = state.reload_core_if_running();
    Ok(deleted)
}

#[tauri::command]
pub async fn select_main_node(
    state: State<'_, AppState>,
    id: String,
) -> Result<SelectNodeResult, String> {
    let control_port = {
        let store = state.store.lock().map_err(|_| "store lock poisoned".to_string())?;
        let mut settings = store.load_settings().map_err(|e| e.to_string())?;
        settings.main_node_id = Some(id.clone());
        store.save_settings(&settings).map_err(|e| e.to_string())?;
        settings.control_port
    };

    let running = state
        .supervisor
        .lock()
        .map(|mut s| s.is_running())
        .unwrap_or(false);

    let mut hot_switched = false;
    if running {
        if let Ok(client) = ControlClient::new(control_port, &state.control_secret) {
            if client.select_proxy("main-exit", &id).await.is_ok() {
                hot_switched = true;
                state.push_log(
                    "INFO",
                    format!("hot-switch: main-exit -> {} (0ms, no core restart)", id),
                );
            }
        }
        if !hot_switched {
            let _ = state.reload_core_if_running();
            state.push_log(
                "INFO",
                format!("reload: switched main-exit -> {} via preflight reload", id),
            );
        }
    } else {
        state.push_log("INFO", format!("main-exit selected -> {}", id));
    }

    let compiled = state.compile_current()?;
    Ok(SelectNodeResult {
        hot_switched,
        main_node_id: id,
        compiled,
    })
}

#[tauri::command]
pub fn set_route_mode(
    state: State<'_, AppState>,
    mode: RouteMode,
) -> Result<CompileOutput, String> {
    {
        let store = state.store.lock().map_err(|_| "store lock poisoned".to_string())?;
        let mut settings = store.load_settings().map_err(|e| e.to_string())?;
        settings.route_mode = mode;
        store.save_settings(&settings).map_err(|e| e.to_string())?;
    }
    state.push_log("INFO", format!("route mode changed -> {}", mode.as_str()));
    let _ = state.reload_core_if_running();
    state.compile_current()
}

#[tauri::command]
pub fn toggle_system_proxy(
    state: State<'_, AppState>,
    enabled: bool,
) -> Result<CoreInfo, String> {
    let mixed_port = {
        let store = state.store.lock().map_err(|_| "store lock poisoned".to_string())?;
        let mut settings = store.load_settings().map_err(|e| e.to_string())?;
        settings.system_proxy_enabled = enabled;
        store.save_settings(&settings).map_err(|e| e.to_string())?;
        settings.mixed_port
    };

    let guard = state
        .netguard
        .lock()
        .map_err(|_| "netguard lock poisoned".to_string())?;
    if enabled {
        guard
            .enable_system_proxy("127.0.0.1", mixed_port)
            .map_err(|e| e.to_string())?;
        state.push_log(
            "INFO",
            format!("netguard: saved OS snapshot & enabled system proxy 127.0.0.1:{}", mixed_port),
        );
    } else {
        guard.disable_system_proxy().map_err(|e| e.to_string())?;
        state.push_log("INFO", "netguard: disabled system proxy & restored OS snapshot");
    }
    drop(guard);

    Ok(state.build_core_info())
}

#[tauri::command]
pub fn toggle_tun(state: State<'_, AppState>, enabled: bool) -> Result<CompileOutput, String> {
    {
        let store = state.store.lock().map_err(|_| "store lock poisoned".to_string())?;
        let mut settings = store.load_settings().map_err(|e| e.to_string())?;
        settings.tun_enabled = enabled;
        store.save_settings(&settings).map_err(|e| e.to_string())?;
    }
    state.push_log(
        "INFO",
        if enabled {
            "tun: enabled TUN inbound in compiled profile (requires portline-service for non-elevated GUI)"
        } else {
            "tun: disabled TUN inbound"
        },
    );
    let _ = state.reload_core_if_running();
    state.compile_current()
}

#[tauri::command]
pub fn save_rule(
    state: State<'_, AppState>,
    rule: SimpleRoutingRule,
) -> Result<CompileOutput, String> {
    {
        let store = state.store.lock().map_err(|_| "store lock poisoned".to_string())?;
        store.upsert_rule(&rule).map_err(|e| e.to_string())?;
    }
    state.push_log("INFO", format!("routing: updated rule [{}]", rule.title_en));
    let _ = state.reload_core_if_running();
    state.compile_current()
}

#[tauri::command]
pub fn delete_rule(state: State<'_, AppState>, id: String) -> Result<CompileOutput, String> {
    {
        let store = state.store.lock().map_err(|_| "store lock poisoned".to_string())?;
        store.delete_rule(&id).map_err(|e| e.to_string())?;
    }
    state.push_log("INFO", format!("routing: deleted rule [{}]", id));
    let _ = state.reload_core_if_running();
    state.compile_current()
}

#[tauri::command]
pub fn save_inbound_port(
    state: State<'_, AppState>,
    port: InboundPort,
) -> Result<CompileOutput, String> {
    {
        let store = state.store.lock().map_err(|_| "store lock poisoned".to_string())?;
        store.upsert_inbound_port(&port).map_err(|e| e.to_string())?;
    }
    state.push_log(
        "INFO",
        format!("inbound: configured dedicated port :{} -> {}", port.port, port.target),
    );
    let _ = state.reload_core_if_running();
    state.compile_current()
}

#[tauri::command]
pub fn delete_inbound_port(
    state: State<'_, AppState>,
    id: String,
) -> Result<CompileOutput, String> {
    {
        let store = state.store.lock().map_err(|_| "store lock poisoned".to_string())?;
        store.delete_inbound_port(&id).map_err(|e| e.to_string())?;
    }
    state.push_log("INFO", format!("inbound: removed port [{}]", id));
    let _ = state.reload_core_if_running();
    state.compile_current()
}

#[tauri::command]
pub fn update_settings(
    state: State<'_, AppState>,
    settings: AppSettings,
) -> Result<CompileOutput, String> {
    {
        let store = state.store.lock().map_err(|_| "store lock poisoned".to_string())?;
        store.save_settings(&settings).map_err(|e| e.to_string())?;
    }
    let _ = state.reload_core_if_running();
    state.compile_current()
}

#[tauri::command]
pub fn compile_preview(state: State<'_, AppState>) -> Result<CompileOutput, String> {
    state.compile_current()
}

#[tauri::command]
pub fn run_preflight_check(
    state: State<'_, AppState>,
    simulate_bad: bool,
) -> Result<PreflightReport, String> {
    let mut compiled = state.compile_current()?;
    if simulate_bad {
        compiled.config["outbounds"] =
            serde_json::json!([{ "type": "invalid_protocol_test", "tag": "broken-test" }]);
    }
    let bin = match state.core_manager.resolve_binary_path() {
        Some(p) => p,
        None if simulate_bad => {
            return Ok(PreflightReport {
                ok: false,
                exit_code: 1,
                stdout: String::new(),
                stderr: "Simulated bad config rejected before core swap".to_string(),
                deprecated_warnings: Vec::new(),
            });
        }
        None => return Err("CORE_NOT_INSTALLED".to_string()),
    };
    let report = SingBox114Compiler::preflight_check(&bin, &compiled.config)
        .map_err(|e| e.to_string())?;
    state.push_log(
        if report.ok { "INFO" } else { "ERROR" },
        format!(
            "preflight check -c -> exit {} (warnings={})",
            report.exit_code,
            report.deprecated_warnings.len()
        ),
    );
    Ok(report)
}

#[tauri::command]
pub fn start_core(state: State<'_, AppState>) -> Result<(CoreInfo, PreflightReport), String> {
    let bin = state.require_core_binary()?;
    let compiled = state.compile_current()?;
    let report = {
        let mut sup = state
            .supervisor
            .lock()
            .map_err(|_| "supervisor lock poisoned".to_string())?;
        sup.apply_config(&bin, compiled.config)
            .map_err(|e| e.to_string())?
    };
    Ok((state.build_core_info(), report))
}

#[tauri::command]
pub fn stop_core(state: State<'_, AppState>) -> Result<CoreInfo, String> {
    if let Ok(mut sup) = state.supervisor.lock() {
        sup.stop();
    }
    Ok(state.build_core_info())
}

#[tauri::command]
pub async fn download_official_core(state: State<'_, AppState>) -> Result<CoreInfo, String> {
    state.push_log("INFO", "core-manager: downloading latest official sing-box release...");
    let (ver, sha) = state
        .core_manager
        .download_official_core(None)
        .await
        .map_err(|e| e.to_string())?;
    state.push_log(
        "INFO",
        format!("core-manager: installed {} (sha256: {}...)", ver, &sha[..12]),
    );
    Ok(state.build_core_info())
}

#[tauri::command]
pub fn import_core_binary(state: State<'_, AppState>, path: String) -> Result<CoreInfo, String> {
    let ver = state
        .core_manager
        .import_local_binary(&PathBuf::from(&path), None)
        .map_err(|e| e.to_string())?;
    state.push_log(
        "INFO",
        format!("core-manager: imported local binary {}", ver),
    );
    Ok(state.build_core_info())
}

#[tauri::command]
pub async fn run_batch_tcping(
    state: State<'_, AppState>,
    node_ids: Vec<String>,
) -> Result<Vec<LatencyResult>, String> {
    let targets = {
        let store = state.store.lock().map_err(|_| "store lock poisoned".to_string())?;
        let all = store.list_nodes().map_err(|e| e.to_string())?;
        if node_ids.is_empty() {
            all
        } else {
            all.into_iter()
                .filter(|n| node_ids.contains(&n.id))
                .collect()
        }
    };
    let results = batch_tcping(&targets, 2500, 16).await;
    if let Ok(store) = state.store.lock() {
        for r in &results {
            let _ = store.update_node_latency(&r.node_id, r.delay_ms, None);
        }
    }
    state.push_log(
        "INFO",
        format!("tester: TCPing finished ({} nodes)", results.len()),
    );
    Ok(results)
}

#[tauri::command]
pub async fn run_batch_real_delay(
    state: State<'_, AppState>,
    node_ids: Vec<String>,
) -> Result<Vec<LatencyResult>, String> {
    let bin = state.require_core_binary()?;
    let (targets, test_url) = {
        let store = state.store.lock().map_err(|_| "store lock poisoned".to_string())?;
        let settings = store.load_settings().map_err(|e| e.to_string())?;
        let all = store.list_nodes().map_err(|e| e.to_string())?;
        let list = if node_ids.is_empty() {
            all
        } else {
            all.into_iter()
                .filter(|n| node_ids.contains(&n.id))
                .collect()
        };
        (list, settings.latency_test_url)
    };
    let results = batch_real_delay(&bin, &targets, &test_url, 4000, 12)
        .await
        .map_err(|e| e.to_string())?;
    if let Ok(store) = state.store.lock() {
        for r in &results {
            let _ = store.update_node_latency(&r.node_id, None, r.delay_ms);
        }
    }
    state.push_log(
        "INFO",
        format!(
            "tester: isolated real-delay test finished ({} nodes, 0ms proxy disruption)",
            results.len()
        ),
    );
    Ok(results)
}

#[tauri::command]
pub async fn get_live_monitor(state: State<'_, AppState>) -> Result<LiveMonitorPayload, String> {
    let running = state
        .supervisor
        .lock()
        .map(|mut s| s.is_running())
        .unwrap_or(false);
    let control_port = state
        .store
        .lock()
        .ok()
        .and_then(|s| s.load_settings().ok())
        .map(|s| s.control_port)
        .unwrap_or(9090);

    let mut connections = Vec::new();
    let mut up_bps = 0u64;
    let mut down_bps = 0u64;

    if running {
        if let Ok(client) = ControlClient::new(control_port, &state.control_secret) {
            if let Ok((conns, up_total, down_total)) = client.get_connections().await {
                connections = conns;
                if let Ok(mut cursor) = state.traffic_cursor.lock() {
                    let elapsed = cursor.last_at.elapsed().as_secs_f64().max(0.2);
                    if up_total >= cursor.last_up && down_total >= cursor.last_down {
                        up_bps = ((up_total - cursor.last_up) as f64 / elapsed) as u64;
                        down_bps = ((down_total - cursor.last_down) as f64 / elapsed) as u64;
                    }
                    cursor.last_up = up_total;
                    cursor.last_down = down_total;
                    cursor.last_at = Instant::now();
                }
            }
        }
    }

    let logs = state
        .logs
        .lock()
        .map(|l| l.snapshot())
        .unwrap_or_default();

    Ok(LiveMonitorPayload {
        core_running: running,
        traffic: TrafficSample {
            up_bytes_per_sec: up_bps,
            down_bytes_per_sec: down_bps,
            memory_bytes: 0,
            active_connections: connections.len(),
        },
        connections,
        logs,
    })
}

#[tauri::command]
pub async fn close_connection(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let control_port = state
        .store
        .lock()
        .ok()
        .and_then(|s| s.load_settings().ok())
        .map(|s| s.control_port)
        .unwrap_or(9090);
    let client = ControlClient::new(control_port, &state.control_secret).map_err(|e| e.to_string())?;
    client.close_connection(&id).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn close_all_connections(state: State<'_, AppState>) -> Result<(), String> {
    let control_port = state
        .store
        .lock()
        .ok()
        .and_then(|s| s.load_settings().ok())
        .map(|s| s.control_port)
        .unwrap_or(9090);
    let client = ControlClient::new(control_port, &state.control_secret).map_err(|e| e.to_string())?;
    client.close_all_connections().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub fn clear_logs(state: State<'_, AppState>) -> Result<(), String> {
    if let Ok(mut guard) = state.logs.lock() {
        guard.clear();
    }
    Ok(())
}

#[tauri::command]
pub fn restore_netguard_now(state: State<'_, AppState>) -> Result<CoreInfo, String> {
    if let Ok(guard) = state.netguard.lock() {
        guard.disable_system_proxy().map_err(|e| e.to_string())?;
    }
    if let Ok(store) = state.store.lock() {
        if let Ok(mut settings) = store.load_settings() {
            settings.system_proxy_enabled = false;
            let _ = store.save_settings(&settings);
        }
    }
    state.push_log("INFO", "netguard: manual OS proxy & DNS restoration completed");
    Ok(state.build_core_info())
}

#[tauri::command]
pub fn hide_to_tray(window: tauri::WebviewWindow) -> Result<(), String> {
    window.hide().map_err(|e| e.to_string())
}
