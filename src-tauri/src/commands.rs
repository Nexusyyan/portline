use crate::state::AppState;
use crate::tray;
use portline_core::compiler::SingBox114Compiler;
use portline_core::control::ControlClient;
use portline_core::model::{
    AppSettings, CompileOutput, ConnectionHistoryRecord, ConnectionItem, CoreInfo, InboundPort,
    LatencyResult, LogEntry, NodeIR, PlatformTrafficStat, PreflightReport, RouteMode,
    RouteTestResult, RulePresetSpec, SimpleRoutingRule, SubscriptionGroup, TrafficSample,
};
use portline_core::parser::{parse_payload, ImportSummary};
use portline_core::route_eval::evaluate_route_target;
use portline_core::rule_cache::RuleSetStatus;
use portline_core::storage::{classify_platform, extract_host, rule_preset_catalog};
use portline_core::tester::{batch_real_delay_with_progress, batch_tcping_with_progress};
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, State};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BootstrapPayload {
    pub core_info: CoreInfo,
    pub settings: AppSettings,
    pub nodes: Vec<NodeIR>,
    pub subscriptions: Vec<SubscriptionGroup>,
    pub rules: Vec<SimpleRoutingRule>,
    pub extra_ports: Vec<InboundPort>,
    pub rule_sets: Vec<RuleSetStatus>,
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

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncRuleSetsResult {
    pub statuses: Vec<RuleSetStatus>,
    pub compiled: CompileOutput,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyStrategyResult {
    pub rules: Vec<SimpleRoutingRule>,
    pub rule_sets: Vec<RuleSetStatus>,
    pub compiled: CompileOutput,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionHistoryPage {
    pub items: Vec<ConnectionHistoryRecord>,
    pub total: usize,
    pub page: usize,
    pub page_size: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlatformTrafficPayload {
    pub items: Vec<PlatformTrafficStat>,
    pub total_upload: u64,
    pub total_download: u64,
    pub total_connections: u64,
}

fn notify_sync(app: &AppHandle) {
    let _ = app.emit("portline-state-sync", ());
}

#[tauri::command]
pub fn get_bootstrap_state(state: State<'_, AppState>) -> Result<BootstrapPayload, String> {
    let (mut settings, nodes, subscriptions, rules, extra_ports) = {
        let store = state.store.lock().map_err(|_| "store lock poisoned".to_string())?;
        (
            store.load_settings().map_err(|e| e.to_string())?,
            store.list_nodes().map_err(|e| e.to_string())?,
            store.list_subscriptions().map_err(|e| e.to_string())?,
            store.list_rules().map_err(|e| e.to_string())?,
            store.list_inbound_ports().map_err(|e| e.to_string())?,
        )
    };
    // Reconcile stored launch_at_login flag with actual OS startup registration.
    let os_autostart = portline_core::autostart::is_autostart_enabled();
    if settings.launch_at_login != os_autostart {
        settings.launch_at_login = os_autostart;
        if let Ok(store) = state.store.lock() {
            let _ = store.save_settings(&settings);
        }
    }
    let rule_sets = state.rule_cache.list_cached();
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
        rule_sets,
        compiled,
        logs,
    })
}

#[tauri::command]
pub fn import_nodes(
    app: AppHandle,
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
    notify_sync(&app);
    Ok(summary)
}

#[tauri::command]
pub async fn import_subscription(
    app: AppHandle,
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
    notify_sync(&app);
    Ok(sub)
}

#[tauri::command]
pub async fn refresh_subscription(
    app: AppHandle,
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
    notify_sync(&app);
    Ok(updated)
}

#[tauri::command]
pub fn delete_subscription(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let store = state.store.lock().map_err(|_| "store lock poisoned".to_string())?;
    store.delete_subscription(&id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn save_node(app: AppHandle, state: State<'_, AppState>, node: NodeIR) -> Result<(), String> {
    {
        let mut store = state.store.lock().map_err(|_| "store lock poisoned".to_string())?;
        store.upsert_nodes(&[node]).map_err(|e| e.to_string())?;
    }
    let _ = state.reload_core_if_running();
    notify_sync(&app);
    Ok(())
}

#[tauri::command]
pub fn delete_node(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> Result<bool, String> {
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
    notify_sync(&app);
    Ok(deleted)
}

#[tauri::command]
pub async fn select_main_node(
    app: AppHandle,
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
    notify_sync(&app);
    Ok(SelectNodeResult {
        hot_switched,
        main_node_id: id,
        compiled,
    })
}

#[tauri::command]
pub async fn set_route_mode(
    app: AppHandle,
    state: State<'_, AppState>,
    mode: RouteMode,
) -> Result<CompileOutput, String> {
    let control_port = {
        let store = state.store.lock().map_err(|_| "store lock poisoned".to_string())?;
        let mut settings = store.load_settings().map_err(|e| e.to_string())?;
        settings.route_mode = mode;
        store.save_settings(&settings).map_err(|e| e.to_string())?;
        settings.control_port
    };

    let running = state
        .supervisor
        .lock()
        .map(|mut s| s.is_running())
        .unwrap_or(false);

    if running {
        let mut live_switched = false;
        if let Ok(client) = ControlClient::new(control_port, &state.control_secret) {
            if client.set_mode(mode.as_str()).await.is_ok() {
                live_switched = true;
                let _ = client.close_all_connections().await;
                state.push_log(
                    "INFO",
                    format!("route mode switched -> {} (clash_mode)", mode.as_str()),
                );
            }
        }
        if !live_switched {
            state.reload_core_if_running()?;
            state.push_log(
                "INFO",
                format!("route mode switched -> {} (core reload)", mode.as_str()),
            );
        }
    } else {
        state.push_log("INFO", format!("route mode changed -> {}", mode.as_str()));
    }

    notify_sync(&app);
    state.compile_current()
}

#[tauri::command]
pub fn toggle_system_proxy(
    app: AppHandle,
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
            format!("netguard: enabled system proxy 127.0.0.1:{}", mixed_port),
        );
    } else {
        guard.disable_system_proxy().map_err(|e| e.to_string())?;
        state.push_log("INFO", "netguard: disabled system proxy & restored snapshot");
    }
    drop(guard);

    notify_sync(&app);
    Ok(state.build_core_info())
}

#[tauri::command]
pub fn toggle_tun(
    app: AppHandle,
    state: State<'_, AppState>,
    enabled: bool,
) -> Result<CompileOutput, String> {
    {
        let store = state.store.lock().map_err(|_| "store lock poisoned".to_string())?;
        let mut settings = store.load_settings().map_err(|e| e.to_string())?;
        settings.tun_enabled = enabled;
        store.save_settings(&settings).map_err(|e| e.to_string())?;
    }
    state.push_log(
        "INFO",
        if enabled {
            "tun: enabled TUN inbound in compiled profile"
        } else {
            "tun: disabled TUN inbound"
        },
    );
    let _ = state.reload_core_if_running();
    notify_sync(&app);
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
pub fn list_rule_presets() -> Vec<RulePresetSpec> {
    rule_preset_catalog()
}

#[tauri::command]
pub fn list_rule_sets(state: State<'_, AppState>) -> Vec<RuleSetStatus> {
    state.rule_cache.list_cached()
}

#[tauri::command]
pub async fn apply_routing_strategy(
    app: AppHandle,
    state: State<'_, AppState>,
    strategy_id: String,
) -> Result<ApplyStrategyResult, String> {
    let (rules, tags_to_sync, proxy_port) = {
        let mut store = state.store.lock().map_err(|_| "store lock poisoned".to_string())?;
        let rules = store
            .apply_routing_strategy(&strategy_id)
            .map_err(|e| e.to_string())?;
        let settings = store.load_settings().map_err(|e| e.to_string())?;
        let running = state
            .supervisor
            .lock()
            .map(|mut s| s.is_running())
            .unwrap_or(false);
        let mut missing: Vec<String> = Vec::new();
        for r in rules.iter().filter(|r| r.enabled) {
            if let Some(ref tag) = r.rule_set {
                let clean = tag.trim();
                if !clean.is_empty()
                    && state.rule_cache.local_srs_path(clean).is_none()
                    && !missing.iter().any(|x| x == clean)
                {
                    missing.push(clean.to_string());
                }
            }
        }
        (rules, missing, if running { Some(settings.mixed_port) } else { None })
    };

    for tag in &tags_to_sync {
        let _ = state.rule_cache.refresh_rule_set_auto(tag, proxy_port).await;
    }

    state.push_log(
        "INFO",
        format!(
            "routing: applied classic strategy [{}] ({} rules)",
            strategy_id,
            rules.len()
        ),
    );
    let _ = state.reload_core_if_running();
    let compiled = state.compile_current()?;
    notify_sync(&app);

    Ok(ApplyStrategyResult {
        rules,
        rule_sets: state.rule_cache.list_cached(),
        compiled,
    })
}

#[tauri::command]
pub async fn sync_rule_sets(
    state: State<'_, AppState>,
    tags: Vec<String>,
) -> Result<SyncRuleSetsResult, String> {
    let (target_tags, proxy_port) = {
        let store = state.store.lock().map_err(|_| "store lock poisoned".to_string())?;
        let settings = store.load_settings().map_err(|e| e.to_string())?;
        let rules = store.list_rules().map_err(|e| e.to_string())?;
        let running = state
            .supervisor
            .lock()
            .map(|mut s| s.is_running())
            .unwrap_or(false);

        let mut set: HashSet<String> = HashSet::new();
        if tags.is_empty() {
            for r in rules.iter().filter(|r| r.enabled) {
                if let Some(ref tag) = r.rule_set {
                    let clean = tag.trim();
                    if !clean.is_empty() {
                        set.insert(clean.to_string());
                    }
                }
            }
            for cr in settings.dns_custom_rules.iter().filter(|r| r.enabled) {
                if let Some(ref tag) = cr.rule_set {
                    let clean = tag.trim();
                    if !clean.is_empty() {
                        set.insert(clean.to_string());
                    }
                }
            }
            // Also fetch geoip-cn when geosite-cn is present for accurate DNS split.
            if set.contains("geosite-cn") {
                set.insert("geoip-cn".to_string());
            }
        } else {
            for t in tags {
                let clean = t.trim();
                if !clean.is_empty() {
                    set.insert(clean.to_string());
                }
            }
        }
        let mut sorted: Vec<String> = set.into_iter().collect();
        sorted.sort();
        (sorted, if running { Some(settings.mixed_port) } else { None })
    };

    let mut statuses = Vec::with_capacity(target_tags.len());
    for tag in &target_tags {
        let st = state.rule_cache.refresh_rule_set_auto(tag, proxy_port).await;
        statuses.push(st);
    }

    let ok_count = statuses.iter().filter(|s| s.cached_locally && !s.fallback_mode).count();
    state.push_log(
        "INFO",
        format!(
            "rule-sets: synced {}/{} .srs files",
            ok_count,
            target_tags.len()
        ),
    );

    let _ = state.reload_core_if_running();
    let compiled = state.compile_current()?;
    Ok(SyncRuleSetsResult {
        statuses: state.rule_cache.list_cached(),
        compiled,
    })
}

#[tauri::command]
pub fn test_route_target(
    state: State<'_, AppState>,
    input: String,
    process_name: Option<String>,
) -> Result<RouteTestResult, String> {
    let (settings, rules, nodes) = {
        let store = state.store.lock().map_err(|_| "store lock poisoned".to_string())?;
        (
            store.load_settings().map_err(|e| e.to_string())?,
            store.list_rules().map_err(|e| e.to_string())?,
            store.list_nodes().map_err(|e| e.to_string())?,
        )
    };
    let sb = state.core_manager.resolve_binary_path();
    Ok(evaluate_route_target(
        &input,
        process_name.as_deref(),
        &settings,
        &rules,
        &nodes,
        Some(&state.rule_cache),
        sb.as_deref(),
    ))
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
    app: AppHandle,
    state: State<'_, AppState>,
    settings: AppSettings,
) -> Result<CompileOutput, String> {
    let (prev_port, prev_autostart) = {
        let store = state.store.lock().map_err(|_| "store lock poisoned".to_string())?;
        let old = store.load_settings().ok();
        (
            old.as_ref().map(|s| s.mixed_port),
            old.as_ref().map(|s| s.launch_at_login),
        )
    };

    if prev_autostart != Some(settings.launch_at_login) {
        portline_core::autostart::set_autostart_enabled(settings.launch_at_login)
            .map_err(|e| e.to_string())?;
        state.push_log(
            "INFO",
            format!(
                "autostart: {} launch at OS login",
                if settings.launch_at_login {
                    "enabled"
                } else {
                    "disabled"
                }
            ),
        );
    }

    {
        let store = state.store.lock().map_err(|_| "store lock poisoned".to_string())?;
        store.save_settings(&settings).map_err(|e| e.to_string())?;
    }

    if prev_port != Some(settings.mixed_port) {
        if let Ok(guard) = state.netguard.lock() {
            if guard.is_armed() {
                let _ = guard.enable_system_proxy("127.0.0.1", settings.mixed_port);
            }
        }
    }
    let _ = state.reload_core_if_running();
    notify_sync(&app);
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
pub async fn start_core(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(CoreInfo, PreflightReport), String> {
    let bin = state.require_core_binary()?;
    let (control_port, route_mode, main_node_id) = {
        let store = state.store.lock().map_err(|_| "store lock poisoned".to_string())?;
        let s = store.load_settings().map_err(|e| e.to_string())?;
        (s.control_port, s.route_mode, s.main_node_id)
    };
    let compiled = state.compile_current()?;
    let report = {
        let mut sup = state
            .supervisor
            .lock()
            .map_err(|_| "supervisor lock poisoned".to_string())?;
        sup.apply_config(&bin, compiled.config)
            .map_err(|e| e.to_string())?
    };

    if report.ok {
        if let Ok(client) = ControlClient::new(control_port, &state.control_secret) {
            if client.wait_ready(1500).await {
                let _ = client.set_mode(route_mode.as_str()).await;
                if let Some(ref main_id) = main_node_id {
                    let _ = client.select_proxy("main-exit", main_id).await;
                }
            }
        }
    }

    notify_sync(&app);
    Ok((state.build_core_info(), report))
}

#[tauri::command]
pub fn stop_core(app: AppHandle, state: State<'_, AppState>) -> Result<CoreInfo, String> {
    state.flush_active_connections();
    if let Ok(mut sup) = state.supervisor.lock() {
        sup.stop();
    }
    notify_sync(&app);
    Ok(state.build_core_info())
}

#[tauri::command]
pub async fn download_official_core(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<CoreInfo, String> {
    state.push_log("INFO", "core-manager: downloading official sing-box release...");
    let (ver, sha) = state
        .core_manager
        .download_official_core(None)
        .await
        .map_err(|e| e.to_string())?;
    state.push_log(
        "INFO",
        format!("core-manager: installed {} (sha256: {}...)", ver, &sha[..12]),
    );
    notify_sync(&app);
    Ok(state.build_core_info())
}

#[tauri::command]
pub fn import_core_binary(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
) -> Result<CoreInfo, String> {
    let ver = state
        .core_manager
        .import_local_binary(&PathBuf::from(&path), None)
        .map_err(|e| e.to_string())?;
    state.push_log(
        "INFO",
        format!("core-manager: imported local binary {}", ver),
    );
    notify_sync(&app);
    Ok(state.build_core_info())
}

#[tauri::command]
pub async fn run_batch_tcping(
    app: AppHandle,
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
    let emit_handle = app.clone();
    let results = batch_tcping_with_progress(&targets, 2500, 16, move |ev| {
        let _ = emit_handle.emit("latency-test-progress", ev);
    })
    .await;

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
    app: AppHandle,
    state: State<'_, AppState>,
    node_ids: Vec<String>,
) -> Result<Vec<LatencyResult>, String> {
    let bin = state.require_core_binary()?;
    let (targets, test_url, local_dns, strip_handshake) = {
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
        (
            list,
            settings.latency_test_url,
            settings.dns_local_server,
            settings.latency_strip_handshake,
        )
    };
    let emit_handle = app.clone();
    let results = batch_real_delay_with_progress(
        &bin,
        &targets,
        &test_url,
        4500,
        12,
        Some(&local_dns),
        strip_handshake,
        move |ev| {
            let _ = emit_handle.emit("latency-test-progress", ev);
        },
    )
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
            "tester: real-delay finished ({} nodes, strip_handshake={})",
            results.len(),
            strip_handshake
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
    let mut memory_bytes = 0u64;

    if running {
        if let Ok(client) = ControlClient::new(control_port, &state.control_secret) {
            if let Ok((conns, up_total, down_total, mem)) = client.get_connections().await {
                state.record_connections_snapshot(&conns);
                connections = conns;
                memory_bytes = mem;
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
            memory_bytes,
            active_connections: connections.len(),
        },
        connections,
        logs,
    })
}

#[tauri::command]
pub fn get_connection_history(
    state: State<'_, AppState>,
    query: String,
    page: usize,
    page_size: usize,
) -> Result<ConnectionHistoryPage, String> {
    let store = state.store.lock().map_err(|_| "store lock poisoned".to_string())?;
    let safe_page = page.max(1);
    let safe_size = page_size.clamp(10, 200);
    let (items, total) = store
        .query_connection_history(&query, safe_page, safe_size)
        .map_err(|e| e.to_string())?;
    Ok(ConnectionHistoryPage {
        items,
        total,
        page: safe_page,
        page_size: safe_size,
    })
}

#[tauri::command]
pub fn get_platform_traffic(
    state: State<'_, AppState>,
    range: String,
) -> Result<PlatformTrafficPayload, String> {
    let since_iso: Option<String> = match range.as_str() {
        "today" => {
            let now_local = chrono::Local::now();
            now_local
                .date_naive()
                .and_hms_opt(0, 0, 0)
                .and_then(|dt| dt.and_local_timezone(chrono::Local).single())
                .map(|dt| dt.with_timezone(&chrono::Utc).to_rfc3339())
        }
        "7d" => Some((chrono::Utc::now() - chrono::Duration::days(7)).to_rfc3339()),
        "30d" => Some((chrono::Utc::now() - chrono::Duration::days(30)).to_rfc3339()),
        _ => None,
    };

    let stored = {
        let store = state.store.lock().map_err(|_| "store lock poisoned".to_string())?;
        store
            .query_platform_traffic(since_iso.as_deref())
            .map_err(|e| e.to_string())?
    };

    // Merge currently active (not yet flushed) connections so platform totals reflect live usage
    // in real time without double-counting closed rows.
    let active_conns = state
        .conn_tracker
        .lock()
        .map(|t| t.snapshot_active())
        .unwrap_or_default();

    let mut merged: HashMap<String, PlatformTrafficStat> = stored
        .into_iter()
        .map(|s| (s.platform.clone(), s))
        .collect();

    for conn in active_conns {
        let host = extract_host(&conn.destination);
        let platform = classify_platform(&host, &conn.process);
        let entry = merged
            .entry(platform.clone())
            .or_insert_with(|| PlatformTrafficStat {
                platform,
                upload_bytes: 0,
                download_bytes: 0,
                total_bytes: 0,
                connection_count: 0,
                active_count: 0,
                top_domains: Vec::new(),
            });
        entry.upload_bytes += conn.upload_bytes;
        entry.download_bytes += conn.download_bytes;
        entry.total_bytes += conn.upload_bytes + conn.download_bytes;
        entry.connection_count += 1;
        entry.active_count += 1;
        if !host.is_empty() && entry.top_domains.len() < 3 && !entry.top_domains.contains(&host) {
            entry.top_domains.push(host);
        }
    }

    let mut items: Vec<PlatformTrafficStat> = merged.into_values().collect();
    items.sort_by(|a, b| {
        b.total_bytes
            .cmp(&a.total_bytes)
            .then_with(|| b.connection_count.cmp(&a.connection_count))
            .then_with(|| a.platform.cmp(&b.platform))
    });

    let total_upload = items.iter().map(|i| i.upload_bytes).sum();
    let total_download = items.iter().map(|i| i.download_bytes).sum();
    let total_connections = items.iter().map(|i| i.connection_count).sum();

    Ok(PlatformTrafficPayload {
        items,
        total_upload,
        total_download,
        total_connections,
    })
}

#[tauri::command]
pub fn clear_connection_history(state: State<'_, AppState>) -> Result<(), String> {
    let store = state.store.lock().map_err(|_| "store lock poisoned".to_string())?;
    store
        .clear_connection_history()
        .map_err(|e| e.to_string())
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
pub fn restore_netguard_now(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<CoreInfo, String> {
    if let Ok(guard) = state.netguard.lock() {
        guard.disable_system_proxy().map_err(|e| e.to_string())?;
    }
    if let Ok(store) = state.store.lock() {
        if let Ok(mut settings) = store.load_settings() {
            settings.system_proxy_enabled = false;
            let _ = store.save_settings(&settings);
        }
    }
    state.push_log("INFO", "netguard: manual OS proxy restoration completed");
    notify_sync(&app);
    Ok(state.build_core_info())
}

#[tauri::command]
pub fn hide_to_tray(window: tauri::WebviewWindow) -> Result<(), String> {
    window.hide().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn show_main_window(app: AppHandle) -> Result<(), String> {
    tray::show_main_window(&app);
    Ok(())
}

#[tauri::command]
pub fn hide_tray_popup(app: AppHandle) -> Result<(), String> {
    tray::hide_tray_popup(&app);
    Ok(())
}

#[tauri::command]
pub fn quit_app(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    state.shutdown_cleanly();
    app.exit(0);
    Ok(())
}
