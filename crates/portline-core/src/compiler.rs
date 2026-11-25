use crate::model::{
    AppSettings, CompileOutput, InboundPort, NodeIR, PreflightReport, Protocol, RouteMode,
    SimpleRoutingRule,
};
use crate::rule_cache::RuleCache;
use anyhow::{anyhow, Context, Result};
use serde_json::{json, Map, Value};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::Path;
use std::process::Command;

pub struct CompileInput<'a> {
    pub nodes: &'a [NodeIR],
    pub rules: &'a [SimpleRoutingRule],
    pub extra_ports: &'a [InboundPort],
    pub settings: &'a AppSettings,
    pub control_secret: &'a str,
    pub cache_db_path: Option<&'a Path>,
    pub rule_cache: Option<&'a RuleCache>,
}

pub struct SingBox114Compiler;

impl SingBox114Compiler {
    /// Compiles Portline's plain-language model into a strict zero-deprecation
    /// `sing-box v1.14+` configuration with reachability tree-shaking.
    pub fn compile(input: &CompileInput<'_>) -> Result<CompileOutput> {
        let node_map: HashMap<&str, &NodeIR> =
            input.nodes.iter().map(|n| (n.id.as_str(), n)).collect();

        // Pick active main node (fall back to first node if unset or deleted).
        let main_node_id: Option<String> = input
            .settings
            .main_node_id
            .as_deref()
            .filter(|id| node_map.contains_key(id))
            .map(|s| s.to_string())
            .or_else(|| input.nodes.first().map(|n| n.id.clone()));

        // Collect all referenced logical auto-groups and node IDs.
        let mut needed_autos: BTreeSet<String> = BTreeSet::new();
        let mut reachable: BTreeSet<String> = BTreeSet::new();

        if let Some(ref main_id) = main_node_id {
            reachable.insert(main_id.clone());
        }

        // In rule mode, gather targets from enabled rules and enabled extra ports.
        for port in input.extra_ports.iter().filter(|p| p.enabled) {
            collect_target_reachability(&port.target, &node_map, input.nodes, &mut needed_autos, &mut reachable);
        }
        if input.settings.route_mode == RouteMode::Rule {
            for rule in input.rules.iter().filter(|r| r.enabled) {
                collect_target_reachability(&rule.target, &node_map, input.nodes, &mut needed_autos, &mut reachable);
            }
        }

        // Expand chained proxy detours transitively so no dangling detour tag is produced.
        let mut queue: Vec<String> = reachable.iter().cloned().collect();
        let mut visited: HashSet<String> = reachable.iter().cloned().collect();
        while let Some(id) = queue.pop() {
            if let Some(node) = node_map.get(id.as_str()) {
                if let Some(ref detour_id) = node.detour {
                    if node_map.contains_key(detour_id.as_str()) && visited.insert(detour_id.clone()) {
                        reachable.insert(detour_id.clone());
                        queue.push(detour_id.clone());
                    }
                }
            }
        }

        // Build inbounds: primary mixed port + enabled dedicated ports + optional TUN.
        let listen_addr = if input.settings.allow_lan {
            "0.0.0.0"
        } else {
            "127.0.0.1"
        };
        let mut inbounds: Vec<Value> = vec![json!({
            "type": "mixed",
            "tag": format!("in-{}", input.settings.mixed_port),
            "listen": listen_addr,
            "listen_port": input.settings.mixed_port,
        })];

        for p in input.extra_ports.iter().filter(|p| p.enabled && p.port != input.settings.mixed_port) {
            let p_listen = if p.allow_lan || input.settings.allow_lan {
                "0.0.0.0"
            } else {
                "127.0.0.1"
            };
            inbounds.push(json!({
                "type": "mixed",
                "tag": p.id,
                "listen": p_listen,
                "listen_port": p.port,
            }));
        }

        if input.settings.tun_enabled {
            inbounds.push(json!({
                "type": "tun",
                "tag": "tun-in",
                "interface_name": "portline-tun0",
                "address": ["172.19.0.1/30"],
                "auto_route": true,
                "strict_route": true
            }));
        }

        // Build outbounds & v1.14 WireGuard endpoints.
        let mut outbounds: Vec<Value> = Vec::new();
        let mut endpoints: Vec<Value> = Vec::new();
        let mut synthesized_groups: Vec<String> = Vec::new();

        // Always synthesize `main-exit` selector so Control API hot-switch works in 0ms.
        let main_selector_members: Vec<String> = if reachable.is_empty() {
            vec!["direct".to_string()]
        } else {
            let mut list: Vec<String> = reachable.iter().cloned().collect();
            list.push("direct".to_string());
            list
        };
        let default_member = main_node_id
            .clone()
            .unwrap_or_else(|| "direct".to_string());

        outbounds.push(json!({
            "type": "selector",
            "tag": "main-exit",
            "outbounds": main_selector_members,
            "default": default_member,
        }));
        synthesized_groups.push("main-exit".to_string());

        // Synthesize dynamic urltest groups (auto-us, auto-jp, auto-hk, auto-sg, auto-all, auto-regex:*).
        let mut auto_tag_map: HashMap<String, String> = HashMap::new();
        for auto_key in &needed_autos {
            let tag = auto_target_to_tag(auto_key);
            let matched = match_auto_nodes(auto_key, input.nodes);
            let members: Vec<String> = if matched.is_empty() {
                vec![default_member.clone()]
            } else {
                matched
            };
            outbounds.push(json!({
                "type": "urltest",
                "tag": tag,
                "outbounds": members,
                "url": input.settings.latency_test_url,
                "interval": "3m",
                "tolerance": 50,
            }));
            synthesized_groups.push(tag.clone());
            auto_tag_map.insert(auto_key.clone(), tag);
        }

        // Direct outbound is always present.
        outbounds.push(json!({
            "type": "direct",
            "tag": "direct"
        }));

        // Emit only the reachable nodes (Tree-Shaking!).
        for id in &reachable {
            if let Some(node) = node_map.get(id.as_str()) {
                if node.protocol == Protocol::Wireguard {
                    endpoints.push(compile_wireguard_endpoint(node));
                } else {
                    outbounds.push(compile_proxy_outbound(node));
                }
            }
        }

        // Build route rules & optional local .srs rule sets (3-tier offline fallback).
        let mut route_rules: Vec<Value> = vec![
            json!({ "action": "sniff" }),
            json!({ "protocol": "dns", "action": "hijack-dns" }),
            json!({ "ip_is_private": true, "outbound": "direct" }),
        ];
        let mut rule_sets: Vec<Value> = Vec::new();
        let mut added_rule_sets: HashSet<String> = HashSet::new();

        // Dedicated extra port bindings take priority over generic domain rules.
        for port in input.extra_ports.iter().filter(|p| p.enabled && p.port != input.settings.mixed_port) {
            let resolved = resolve_compiled_outbound(&port.target, &auto_tag_map, &reachable);
            if resolved == "reject" {
                route_rules.push(json!({
                    "inbound": [port.id],
                    "action": "reject"
                }));
            } else {
                route_rules.push(json!({
                    "inbound": [port.id],
                    "outbound": resolved
                }));
            }
        }

        if input.settings.route_mode == RouteMode::Rule {
            let mut sorted_rules: Vec<&SimpleRoutingRule> =
                input.rules.iter().filter(|r| r.enabled).collect();
            sorted_rules.sort_by_key(|r| r.priority);

            for rule in sorted_rules {
                let mut rule_obj = Map::new();
                let mut has_matcher = false;

                // Use local cached .srs file if available; otherwise fall back safely to domain_suffix
                // so offline startup never fails waiting on remote rule-sets.
                if let Some(ref srs_tag) = rule.rule_set {
                    if let Some(cache) = input.rule_cache {
                        if let Some(local_path) = cache.local_srs_path(srs_tag) {
                            if added_rule_sets.insert(srs_tag.clone()) {
                                rule_sets.push(json!({
                                    "type": "local",
                                    "tag": srs_tag,
                                    "format": "binary",
                                    "path": local_path.to_string_lossy(),
                                }));
                            }
                            rule_obj.insert("rule_set".to_string(), json!([srs_tag]));
                            has_matcher = true;
                        }
                    }
                }

                if !has_matcher && !rule.patterns.is_empty() {
                    let clean_domains: Vec<String> = rule
                        .patterns
                        .iter()
                        .map(|p| p.trim().trim_start_matches('.').to_ascii_lowercase())
                        .filter(|p| !p.is_empty())
                        .collect();
                    if !clean_domains.is_empty() {
                        rule_obj.insert("domain_suffix".to_string(), json!(clean_domains));
                        has_matcher = true;
                    }
                }

                if !rule.process_names.is_empty() {
                    rule_obj.insert("process_name".to_string(), json!(rule.process_names));
                    has_matcher = true;
                }

                if !has_matcher {
                    continue;
                }

                let resolved = resolve_compiled_outbound(&rule.target, &auto_tag_map, &reachable);
                if resolved == "reject" {
                    rule_obj.insert("action".to_string(), json!("reject"));
                } else {
                    rule_obj.insert("outbound".to_string(), json!(resolved));
                }
                route_rules.push(Value::Object(rule_obj));
            }
        }

        let final_outbound = match input.settings.route_mode {
            RouteMode::Direct => "direct",
            RouteMode::Rule | RouteMode::Global => "main-exit",
        };

        // Strict sing-box v1.14+ DNS section with typed servers, evaluate action,
        // and explicit match_response: true on response-based IP matching rules.
        let mut dns_rules: Vec<Value> = vec![
            json!({
                "domain_suffix": ["cn"],
                "server": "dns-local"
            }),
            json!({
                "action": "evaluate",
                "server": "dns-local"
            }),
        ];
        if added_rule_sets.contains("geoip-cn") {
            dns_rules.push(json!({
                "rule_set": ["geoip-cn"],
                "match_response": true,
                "action": "respond"
            }));
        } else {
            dns_rules.push(json!({
                "ip_is_private": true,
                "match_response": true,
                "action": "respond"
            }));
        }

        let mut route_obj = Map::new();
        route_obj.insert("default_http_client".to_string(), json!("proxy-http"));
        route_obj.insert("default_domain_resolver".to_string(), json!("dns-local"));
        route_obj.insert("auto_detect_interface".to_string(), json!(true));
        if !rule_sets.is_empty() {
            route_obj.insert("rule_set".to_string(), Value::Array(rule_sets));
        }
        route_obj.insert("rules".to_string(), Value::Array(route_rules));
        route_obj.insert("final".to_string(), json!(final_outbound));

        let mut cache_file_obj = Map::new();
        cache_file_obj.insert("enabled".to_string(), json!(true));
        if let Some(p) = input.cache_db_path {
            cache_file_obj.insert("path".to_string(), json!(p.to_string_lossy()));
        }

        let mut root = Map::new();
        root.insert("log".to_string(), json!({ "level": "info", "timestamp": true }));
        root.insert(
            "http_clients".to_string(),
            json!([
                { "tag": "proxy-http", "detour": "main-exit" },
                { "tag": "direct-http", "detour": "direct" }
            ]),
        );
        root.insert(
            "dns".to_string(),
            json!({
                "servers": [
                    { "type": "udp", "tag": "dns-local", "server": "223.5.5.5" },
                    { "type": "https", "tag": "dns-remote", "server": "1.1.1.1", "path": "/dns-query", "detour": "main-exit" }
                ],
                "rules": dns_rules,
                "final": if input.settings.route_mode == RouteMode::Direct { "dns-local" } else { "dns-remote" }
            }),
        );
        if !endpoints.is_empty() {
            root.insert("endpoints".to_string(), Value::Array(endpoints));
        }
        root.insert("inbounds".to_string(), Value::Array(inbounds));
        root.insert("outbounds".to_string(), Value::Array(outbounds));
        root.insert("route".to_string(), Value::Object(route_obj));
        root.insert(
            "experimental".to_string(),
            json!({
                "cache_file": Value::Object(cache_file_obj),
                "clash_api": {
                    "external_controller": format!("127.0.0.1:{}", input.settings.control_port),
                    "secret": input.control_secret,
                    "default_mode": input.settings.route_mode.as_str()
                }
            }),
        );

        let mut config_value = Value::Object(root);

        // Layer 3 geek escape hatch: optional JSON merge patch override.
        if let Some(ref raw_override) = input.settings.custom_json_override {
            let trimmed = raw_override.trim();
            if !trimmed.is_empty() {
                let patch: Value =
                    serde_json::from_str(trimmed).context("Invalid custom JSON override")?;
                merge_json(&mut config_value, patch);
            }
        }

        let reachable_node_ids: Vec<String> = reachable.into_iter().collect();
        Ok(CompileOutput {
            config: config_value,
            total_nodes: input.nodes.len(),
            reachable_nodes: reachable_node_ids.len(),
            reachable_node_ids,
            synthesized_groups,
            control_secret: input.control_secret.to_string(),
        })
    }

    /// Runs `sing-box check -c <temp.json>` to validate a candidate config before activation.
    pub fn preflight_check(sing_box_path: &Path, config: &Value) -> Result<PreflightReport> {
        if !sing_box_path.exists() {
            return Err(anyhow!(
                "sing-box binary not found at {}",
                sing_box_path.display()
            ));
        }
        let tmp_dir = std::env::temp_dir().join(format!("portline-preflight-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&tmp_dir)?;
        let cfg_path = tmp_dir.join("candidate.json");
        let pretty = serde_json::to_string_pretty(config)?;
        std::fs::write(&cfg_path, pretty)?;

        let mut cmd = Command::new(sing_box_path);
        cmd.arg("check").arg("-c").arg(&cfg_path);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x08000000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }

        let output = cmd.output().context("Failed to execute sing-box check")?;
        let _ = std::fs::remove_dir_all(&tmp_dir);

        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let combined = format!("{}\n{}", stdout, stderr);
        let deprecated_warnings: Vec<String> = combined
            .lines()
            .filter(|line| {
                let lower = line.to_ascii_lowercase();
                lower.contains("deprecated") || lower.contains("legacy")
            })
            .map(|l| l.trim().to_string())
            .collect();

        Ok(PreflightReport {
            ok: output.status.success(),
            exit_code: output.status.code().unwrap_or(-1),
            stdout,
            stderr,
            deprecated_warnings,
        })
    }
}

fn collect_target_reachability(
    target: &str,
    node_map: &HashMap<&str, &NodeIR>,
    all_nodes: &[NodeIR],
    needed_autos: &mut BTreeSet<String>,
    reachable: &mut BTreeSet<String>,
) {
    if target == "direct" || target == "reject" || target == "main" {
        return;
    }
    if target.starts_with("auto:") {
        needed_autos.insert(target.to_string());
        for id in match_auto_nodes(target, all_nodes) {
            reachable.insert(id);
        }
        return;
    }
    if node_map.contains_key(target) {
        reachable.insert(target.to_string());
    }
}

pub fn auto_target_to_tag(target: &str) -> String {
    let suffix = target
        .strip_prefix("auto:")
        .unwrap_or(target)
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>();
    format!("auto-{}", suffix.trim_matches('-'))
}

pub fn match_auto_nodes(target: &str, nodes: &[NodeIR]) -> Vec<String> {
    let key = target.strip_prefix("auto:").unwrap_or("ALL");
    if key.eq_ignore_ascii_case("ALL") {
        return nodes.iter().map(|n| n.id.clone()).collect();
    }
    if let Some(pattern) = key.strip_prefix("regex:") {
        if let Ok(re) = regex::RegexBuilder::new(pattern).case_insensitive(true).build() {
            return nodes
                .iter()
                .filter(|n| re.is_match(&n.remark) || re.is_match(&n.group) || re.is_match(&n.region))
                .map(|n| n.id.clone())
                .collect();
        }
    }
    let upper = key.to_ascii_uppercase();
    nodes
        .iter()
        .filter(|n| n.region.eq_ignore_ascii_case(&upper) || n.remark.to_ascii_uppercase().contains(&upper))
        .map(|n| n.id.clone())
        .collect()
}

fn resolve_compiled_outbound(
    target: &str,
    auto_tag_map: &HashMap<String, String>,
    reachable: &BTreeSet<String>,
) -> String {
    match target {
        "direct" => "direct".to_string(),
        "reject" => "reject".to_string(),
        "main" => "main-exit".to_string(),
        t if t.starts_with("auto:") => auto_tag_map
            .get(t)
            .cloned()
            .unwrap_or_else(|| "main-exit".to_string()),
        id if reachable.contains(id) => id.to_string(),
        _ => "main-exit".to_string(),
    }
}

fn compile_tls(node: &NodeIR) -> Option<Value> {
    let tls = node.tls.as_ref()?;
    if !tls.enabled {
        return None;
    }
    let mut obj = Map::new();
    obj.insert("enabled".to_string(), json!(true));
    if let Some(ref sni) = tls.server_name {
        if !sni.is_empty() {
            obj.insert("server_name".to_string(), json!(sni));
        }
    }
    if tls.insecure {
        obj.insert("insecure".to_string(), json!(true));
    }
    if !tls.alpn.is_empty() {
        obj.insert("alpn".to_string(), json!(tls.alpn));
    }
    if let Some(ref fp) = tls.fingerprint {
        if !fp.is_empty() {
            obj.insert("utls".to_string(), json!({ "enabled": true, "fingerprint": fp }));
        }
    }
    if let Some(ref pbk) = tls.reality_public_key {
        if !pbk.is_empty() {
            let mut reality = Map::new();
            reality.insert("enabled".to_string(), json!(true));
            reality.insert("public_key".to_string(), json!(pbk));
            if let Some(ref sid) = tls.reality_short_id {
                reality.insert("short_id".to_string(), json!(sid));
            }
            obj.insert("reality".to_string(), Value::Object(reality));
            if !obj.contains_key("utls") {
                obj.insert("utls".to_string(), json!({ "enabled": true, "fingerprint": "chrome" }));
            }
        }
    }
    Some(Value::Object(obj))
}

fn compile_transport(node: &NodeIR) -> Option<Value> {
    let tr = node.transport.as_ref()?;
    let mut obj = Map::new();
    obj.insert("type".to_string(), json!(tr.transport_type));
    match tr.transport_type.as_str() {
        "ws" => {
            if let Some(ref path) = tr.path {
                obj.insert("path".to_string(), json!(path));
            }
            if !tr.headers.is_empty() {
                obj.insert("headers".to_string(), json!(tr.headers));
            } else if let Some(ref host) = tr.host {
                obj.insert("headers".to_string(), json!({ "Host": host }));
            }
        }
        "grpc" => {
            if let Some(ref svc) = tr.service_name {
                obj.insert("service_name".to_string(), json!(svc));
            }
        }
        "httpupgrade" => {
            if let Some(ref path) = tr.path {
                obj.insert("path".to_string(), json!(path));
            }
            if let Some(ref host) = tr.host {
                obj.insert("host".to_string(), json!(host));
            }
        }
        _ => {}
    }
    Some(Value::Object(obj))
}

pub fn compile_proxy_outbound(node: &NodeIR) -> Value {
    let mut obj = Map::new();
    obj.insert("type".to_string(), json!(node.protocol.as_singbox_type()));
    obj.insert("tag".to_string(), json!(node.id));
    obj.insert("server".to_string(), json!(node.server));
    obj.insert("server_port".to_string(), json!(node.port));

    match node.protocol {
        Protocol::Vless => {
            obj.insert(
                "uuid".to_string(),
                json!(node.uuid.as_deref().unwrap_or("00000000-0000-0000-0000-000000000000")),
            );
            if let Some(ref flow) = node.flow {
                if !flow.is_empty() {
                    obj.insert("flow".to_string(), json!(flow));
                }
            }
        }
        Protocol::Vmess => {
            obj.insert(
                "uuid".to_string(),
                json!(node.uuid.as_deref().unwrap_or("00000000-0000-0000-0000-000000000000")),
            );
            obj.insert(
                "security".to_string(),
                json!(node.security.as_deref().unwrap_or("auto")),
            );
            obj.insert("alter_id".to_string(), json!(node.alter_id.unwrap_or(0)));
        }
        Protocol::Trojan => {
            obj.insert(
                "password".to_string(),
                json!(node.password.as_deref().unwrap_or("")),
            );
        }
        Protocol::Shadowsocks => {
            obj.insert(
                "method".to_string(),
                json!(node.method.as_deref().unwrap_or("aes-256-gcm")),
            );
            obj.insert(
                "password".to_string(),
                json!(node.password.as_deref().unwrap_or("")),
            );
        }
        Protocol::Hysteria2 => {
            obj.insert(
                "password".to_string(),
                json!(node.password.as_deref().unwrap_or("")),
            );
            if let Some(up) = node.up_mbps {
                obj.insert("up_mbps".to_string(), json!(up));
            }
            if let Some(down) = node.down_mbps {
                obj.insert("down_mbps".to_string(), json!(down));
            }
            if let Some(ref obfs) = node.obfs_type {
                let mut obfs_obj = Map::new();
                obfs_obj.insert("type".to_string(), json!(obfs));
                if let Some(ref pass) = node.obfs_password {
                    obfs_obj.insert("password".to_string(), json!(pass));
                }
                obj.insert("obfs".to_string(), Value::Object(obfs_obj));
            }
        }
        Protocol::Tuic => {
            obj.insert(
                "uuid".to_string(),
                json!(node.uuid.as_deref().unwrap_or("00000000-0000-0000-0000-000000000000")),
            );
            obj.insert(
                "password".to_string(),
                json!(node.password.as_deref().unwrap_or("")),
            );
            obj.insert(
                "congestion_control".to_string(),
                json!(node.congestion_control.as_deref().unwrap_or("bbr")),
            );
        }
        Protocol::Anytls => {
            obj.insert(
                "password".to_string(),
                json!(node.password.as_deref().unwrap_or("")),
            );
        }
        Protocol::Wireguard => {}
    }

    if let Some(tls) = compile_tls(node) {
        obj.insert("tls".to_string(), tls);
    } else if matches!(
        node.protocol,
        Protocol::Trojan | Protocol::Hysteria2 | Protocol::Tuic | Protocol::Anytls
    ) {
        obj.insert(
            "tls".to_string(),
            json!({ "enabled": true, "server_name": node.server }),
        );
    }

    if let Some(tr) = compile_transport(node) {
        obj.insert("transport".to_string(), tr);
    }

    if let Some(ref detour) = node.detour {
        if !detour.is_empty() {
            obj.insert("detour".to_string(), json!(detour));
        }
    }

    Value::Object(obj)
}

pub fn compile_wireguard_endpoint(node: &NodeIR) -> Value {
    let addresses = if node.wg_local_address.is_empty() {
        vec!["10.0.0.2/32".to_string()]
    } else {
        node.wg_local_address.clone()
    };
    let mut peer = Map::new();
    peer.insert("address".to_string(), json!(node.server));
    peer.insert("port".to_string(), json!(node.port));
    peer.insert(
        "public_key".to_string(),
        json!(node
            .wg_peer_public_key
            .as_deref()
            .unwrap_or("BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB=")),
    );
    peer.insert("allowed_ips".to_string(), json!(["0.0.0.0/0", "::/0"]));
    if let Some(ref psk) = node.wg_preshared_key {
        if !psk.is_empty() {
            peer.insert("pre_shared_key".to_string(), json!(psk));
        }
    }
    if let Some(ref reserved) = node.wg_reserved {
        if reserved.len() == 3 {
            peer.insert("reserved".to_string(), json!(reserved));
        }
    }

    let mut obj = Map::new();
    obj.insert("type".to_string(), json!("wireguard"));
    obj.insert("tag".to_string(), json!(node.id));
    obj.insert("address".to_string(), json!(addresses));
    obj.insert(
        "private_key".to_string(),
        json!(node
            .wg_private_key
            .as_deref()
            .unwrap_or("AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=")),
    );
    if let Some(mtu) = node.wg_mtu {
        obj.insert("mtu".to_string(), json!(mtu));
    }
    obj.insert("peers".to_string(), Value::Array(vec![Value::Object(peer)]));
    Value::Object(obj)
}

fn merge_json(base: &mut Value, patch: Value) {
    match (base, patch) {
        (Value::Object(b), Value::Object(p)) => {
            for (k, v) in p {
                if v.is_null() {
                    b.remove(&k);
                } else {
                    merge_json(b.entry(k).or_insert(Value::Null), v);
                }
            }
        }
        (b, p) => *b = p,
    }
}
