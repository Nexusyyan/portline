use crate::model::{
    AppSettings, CompileOutput, InboundPort, NodeIR, PreflightReport, Protocol, SimpleRoutingRule,
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

        // Always collect targets from enabled rules, extra ports, and remote DNS detour so 0ms live
        // `clash_mode` switching between direct / global / rule works without restarting core.
        for port in input.extra_ports.iter().filter(|p| p.enabled) {
            collect_target_reachability(&port.target, &node_map, input.nodes, &mut needed_autos, &mut reachable);
        }
        for rule in input.rules.iter().filter(|r| r.enabled) {
            collect_target_reachability(&rule.target, &node_map, input.nodes, &mut needed_autos, &mut reachable);
        }
        collect_target_reachability(
            &input.settings.dns_remote_detour,
            &node_map,
            input.nodes,
            &mut needed_autos,
            &mut reachable,
        );

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
            let iface = if input.settings.tun_interface_name.trim().is_empty() {
                "portline-tun0"
            } else {
                input.settings.tun_interface_name.trim()
            };
            let addr = if input.settings.tun_inet4_address.trim().is_empty() {
                "172.19.0.1/30"
            } else {
                input.settings.tun_inet4_address.trim()
            };
            let stack = match input.settings.tun_stack.as_str() {
                "system" | "gvisor" | "mixed" => input.settings.tun_stack.as_str(),
                _ => "mixed",
            };
            let mtu = input.settings.tun_mtu.clamp(1280, 65535);
            inbounds.push(json!({
                "type": "tun",
                "tag": "tun-in",
                "interface_name": iface,
                "address": [addr],
                "mtu": mtu,
                "stack": stack,
                "auto_route": input.settings.tun_auto_route,
                "strict_route": input.settings.tun_strict_route
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
        let urltest_interval = if input.settings.urltest_interval.trim().is_empty() {
            "3m"
        } else {
            input.settings.urltest_interval.trim()
        };
        let urltest_tolerance = input.settings.urltest_tolerance.clamp(10, 2000);
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
                "interval": urltest_interval,
                "tolerance": urltest_tolerance,
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
        // `clash_mode` rules sit immediately after sniff / hijack-dns / private LAN bypass so
        // PATCH /configs {"mode":"direct"|"global"|"rule"} switches live traffic in 0ms.
        let mut route_rules: Vec<Value> = Vec::new();
        if input.settings.sniff_enabled {
            let timeout = input.settings.sniff_timeout.trim();
            if timeout.is_empty() {
                route_rules.push(json!({ "action": "sniff" }));
            } else {
                route_rules.push(json!({ "action": "sniff", "timeout": timeout }));
            }
        }
        route_rules.push(json!({ "protocol": "dns", "action": "hijack-dns" }));
        route_rules.push(json!({ "ip_is_private": true, "outbound": "direct" }));
        route_rules.push(json!({ "clash_mode": "direct", "outbound": "direct" }));
        route_rules.push(json!({ "clash_mode": "global", "outbound": "main-exit" }));

        let mut rule_sets: Vec<Value> = Vec::new();
        let mut added_rule_sets: HashSet<String> = HashSet::new();

        // Dedicated extra port bindings take priority over generic domain rules in rule mode.
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

        let mut sorted_rules: Vec<&SimpleRoutingRule> =
            input.rules.iter().filter(|r| r.enabled).collect();
        sorted_rules.sort_by_key(|r| r.priority);

        for rule in sorted_rules {
            let mut branches: Vec<Map<String, Value>> = Vec::new();

            // Use local cached .srs file if available, and keep any user-added patterns/keywords/CIDRs
            // as OR branches so custom entries are never silently dropped.
            // If `.srs` is not yet cached locally and `patterns` is empty, emit a minimal offline
            // emergency domain fallback for core rules so offline cold-start never breaks.
            let mut srs_attached = false;
            if let Some(ref srs_tag) = rule.rule_set {
                let tag_clean = srs_tag.trim();
                if !tag_clean.is_empty() {
                    if let Some(cache) = input.rule_cache {
                        if let Some(local_path) = cache.local_srs_path(tag_clean) {
                            if added_rule_sets.insert(tag_clean.to_string()) {
                                rule_sets.push(json!({
                                    "type": "local",
                                    "tag": tag_clean,
                                    "format": "binary",
                                    "path": local_path.to_string_lossy(),
                                }));
                            }
                            let mut b = Map::new();
                            b.insert("rule_set".to_string(), json!([tag_clean]));
                            branches.push(b);
                            srs_attached = true;
                        }
                    }
                    if !srs_attached && rule.patterns.is_empty() {
                        let emergency = offline_srs_fallback_domains(tag_clean);
                        if !emergency.is_empty() {
                            let mut b = Map::new();
                            b.insert("domain_suffix".to_string(), json!(emergency));
                            branches.push(b);
                        }
                    }
                }
            }

            let clean_domains: Vec<String> = rule
                .patterns
                .iter()
                .map(|p| p.trim().trim_start_matches('.').to_ascii_lowercase())
                .filter(|p| !p.is_empty())
                .collect();
            if !clean_domains.is_empty() {
                let mut b = Map::new();
                b.insert("domain_suffix".to_string(), json!(clean_domains));
                branches.push(b);
            }

            let clean_keywords: Vec<String> = rule
                .domain_keywords
                .iter()
                .map(|k| k.trim().to_ascii_lowercase())
                .filter(|k| !k.is_empty())
                .collect();
            if !clean_keywords.is_empty() {
                let mut b = Map::new();
                b.insert("domain_keyword".to_string(), json!(clean_keywords));
                branches.push(b);
            }

            let clean_cidrs: Vec<String> = rule
                .ip_cidrs
                .iter()
                .map(|c| c.trim().to_string())
                .filter(|c| !c.is_empty())
                .collect();
            if !clean_cidrs.is_empty() {
                let mut b = Map::new();
                b.insert("ip_cidr".to_string(), json!(clean_cidrs));
                branches.push(b);
            }

            let clean_procs: Vec<String> = rule
                .process_names
                .iter()
                .map(|p| p.trim().to_string())
                .filter(|p| !p.is_empty())
                .collect();
            if !clean_procs.is_empty() {
                let mut b = Map::new();
                b.insert("process_name".to_string(), json!(clean_procs));
                branches.push(b);
            }

            if branches.is_empty() {
                continue;
            }

            let mut rule_obj = if branches.len() == 1 {
                branches.remove(0)
            } else {
                let mut logical = Map::new();
                logical.insert("type".to_string(), json!("logical"));
                logical.insert("mode".to_string(), json!("or"));
                logical.insert(
                    "rules".to_string(),
                    Value::Array(branches.into_iter().map(Value::Object).collect()),
                );
                logical
            };

            let resolved = resolve_compiled_outbound(&rule.target, &auto_tag_map, &reachable);
            if resolved == "reject" {
                rule_obj.insert("action".to_string(), json!("reject"));
            } else {
                rule_obj.insert("outbound".to_string(), json!(resolved));
            }
            route_rules.push(Value::Object(rule_obj));
        }

        let final_outbound = "main-exit";

        // Strict sing-box v1.14+ DNS section honoring user-configured local/remote/bootstrap DNS,
        // detour, strategy, hosts, custom rules, cache behavior, and optional FakeIP mode.
        let mut dns_servers: Vec<Value> = Vec::new();
        let (local_srv, local_needs_boot) = compile_dns_server(
            "dns-local",
            &input.settings.dns_local_server,
            None,
        );
        let remote_detour_resolved =
            resolve_compiled_outbound(&input.settings.dns_remote_detour, &auto_tag_map, &reachable);
        let remote_detour_opt = if remote_detour_resolved == "direct"
            || remote_detour_resolved == "reject"
            || input.settings.dns_remote_detour.trim().eq_ignore_ascii_case("direct")
        {
            None
        } else {
            Some(remote_detour_resolved.as_str())
        };
        let (remote_srv, remote_needs_boot) = compile_dns_server(
            "dns-remote",
            &input.settings.dns_remote_server,
            remote_detour_opt,
        );
        if local_needs_boot || remote_needs_boot {
            let boot_srv = if input.settings.dns_bootstrap_server.trim().is_empty() {
                "223.5.5.5"
            } else {
                input.settings.dns_bootstrap_server.trim()
            };
            dns_servers.push(json!({
                "type": "udp",
                "tag": "dns-bootstrap",
                "server": boot_srv
            }));
        }
        dns_servers.push(local_srv);
        dns_servers.push(remote_srv);

        let mut host_domains: Vec<String> = Vec::new();
        let mut predefined_hosts = Map::new();
        for entry in &input.settings.dns_hosts {
            let dom = entry.domain.trim().to_ascii_lowercase();
            let ips: Vec<String> = entry
                .ips
                .iter()
                .map(|ip| ip.trim().to_string())
                .filter(|ip| !ip.is_empty())
                .collect();
            if !dom.is_empty() && !ips.is_empty() {
                host_domains.push(dom.clone());
                predefined_hosts.insert(dom, json!(ips));
            }
        }
        if !predefined_hosts.is_empty() {
            dns_servers.push(json!({
                "type": "hosts",
                "tag": "dns-hosts",
                "predefined": Value::Object(predefined_hosts)
            }));
        }

        if input.settings.dns_fake_ip {
            let inet4 = if input.settings.dns_fake_ip_inet4.trim().is_empty() {
                "198.18.0.0/15"
            } else {
                input.settings.dns_fake_ip_inet4.trim()
            };
            let inet6 = if input.settings.dns_fake_ip_inet6.trim().is_empty() {
                "fc00::/18"
            } else {
                input.settings.dns_fake_ip_inet6.trim()
            };
            dns_servers.push(json!({
                "type": "fakeip",
                "tag": "dns-fakeip",
                "inet4_range": inet4,
                "inet6_range": inet6
            }));
        }

        let mut direct_domains: Vec<String> = vec!["cn".to_string()];
        for rule in input.rules.iter().filter(|r| r.enabled && r.target == "direct") {
            for p in &rule.patterns {
                let cleaned = p.trim().trim_start_matches('.').to_ascii_lowercase();
                if !cleaned.is_empty() && !direct_domains.contains(&cleaned) {
                    direct_domains.push(cleaned);
                }
            }
        }

        let mut dns_rules: Vec<Value> = Vec::new();
        if !host_domains.is_empty() {
            dns_rules.push(json!({
                "domain": host_domains,
                "server": "dns-hosts"
            }));
        }

        for custom in input.settings.dns_custom_rules.iter().filter(|r| r.enabled) {
            let mut r_obj = Map::new();
            let mut matched = false;
            if let Some(ref srs_tag) = custom.rule_set {
                let tag_clean = srs_tag.trim();
                if !tag_clean.is_empty() {
                    if let Some(cache) = input.rule_cache {
                        if let Some(local_path) = cache.local_srs_path(tag_clean) {
                            if added_rule_sets.insert(tag_clean.to_string()) {
                                rule_sets.push(json!({
                                    "type": "local",
                                    "tag": tag_clean,
                                    "format": "binary",
                                    "path": local_path.to_string_lossy(),
                                }));
                            }
                            r_obj.insert("rule_set".to_string(), json!([tag_clean]));
                            matched = true;
                        }
                    }
                }
            }
            let doms: Vec<String> = custom
                .domains
                .iter()
                .map(|d| d.trim().trim_start_matches('.').to_ascii_lowercase())
                .filter(|d| !d.is_empty())
                .collect();
            if !doms.is_empty() && !matched {
                r_obj.insert("domain_suffix".to_string(), json!(doms));
                matched = true;
            }
            if !matched {
                continue;
            }
            match custom.server.as_str() {
                "reject" => {
                    r_obj.insert("action".to_string(), json!("reject"));
                }
                "dns-fakeip" if input.settings.dns_fake_ip => {
                    r_obj.insert("server".to_string(), json!("dns-fakeip"));
                }
                "dns-local" => {
                    r_obj.insert("server".to_string(), json!("dns-local"));
                }
                _ => {
                    r_obj.insert("server".to_string(), json!("dns-remote"));
                }
            }
            dns_rules.push(Value::Object(r_obj));
        }

        dns_rules.push(json!({
            "clash_mode": "direct",
            "server": "dns-local"
        }));
        if input.settings.dns_fake_ip {
            dns_rules.push(json!({
                "domain_suffix": direct_domains,
                "server": "dns-local"
            }));
            dns_rules.push(json!({
                "clash_mode": "global",
                "query_type": ["A", "AAAA"],
                "server": "dns-fakeip"
            }));
            dns_rules.push(json!({
                "query_type": ["A", "AAAA"],
                "server": "dns-fakeip"
            }));
        } else {
            dns_rules.push(json!({
                "clash_mode": "global",
                "server": "dns-remote"
            }));
            dns_rules.push(json!({
                "domain_suffix": direct_domains,
                "server": "dns-local"
            }));
            if added_rule_sets.contains("geosite-cn") {
                dns_rules.push(json!({
                    "rule_set": ["geosite-cn"],
                    "server": "dns-local"
                }));
            }
            if added_rule_sets.contains("geoip-cn") {
                dns_rules.push(json!({
                    "action": "evaluate",
                    "server": "dns-local"
                }));
                dns_rules.push(json!({
                    "rule_set": ["geoip-cn"],
                    "match_response": true,
                    "action": "respond"
                }));
            }
        }

        let dns_strategy = match input.settings.dns_strategy.as_str() {
            "prefer_ipv4" | "prefer_ipv6" | "ipv4_only" | "ipv6_only" => {
                input.settings.dns_strategy.as_str()
            }
            _ => "prefer_ipv4",
        };

        let mut dns_obj = Map::new();
        dns_obj.insert("servers".to_string(), Value::Array(dns_servers));
        dns_obj.insert("rules".to_string(), Value::Array(dns_rules));
        dns_obj.insert("final".to_string(), json!("dns-remote"));
        dns_obj.insert("strategy".to_string(), json!(dns_strategy));
        if input.settings.dns_disable_cache {
            dns_obj.insert("disable_cache".to_string(), json!(true));
        }
        if input.settings.dns_disable_expire {
            dns_obj.insert("disable_expire".to_string(), json!(true));
        }
        if input.settings.dns_reverse_mapping {
            dns_obj.insert("reverse_mapping".to_string(), json!(true));
        }
        if input.settings.dns_cache_capacity > 0 && input.settings.dns_cache_capacity != 4096 {
            dns_obj.insert(
                "cache_capacity".to_string(),
                json!(input.settings.dns_cache_capacity),
            );
        }
        if let Some(ref subnet) = input.settings.dns_client_subnet {
            let trimmed = subnet.trim();
            if !trimmed.is_empty() {
                dns_obj.insert("client_subnet".to_string(), json!(trimmed));
            }
        }

        let mut route_obj = Map::new();
        route_obj.insert("find_process".to_string(), json!(input.settings.find_process));
        route_obj.insert("default_http_client".to_string(), json!("proxy-http"));
        route_obj.insert("default_domain_resolver".to_string(), json!("dns-local"));
        route_obj.insert(
            "auto_detect_interface".to_string(),
            json!(input.settings.auto_detect_interface),
        );
        if !rule_sets.is_empty() {
            route_obj.insert("rule_set".to_string(), Value::Array(rule_sets));
        }
        route_obj.insert("rules".to_string(), Value::Array(route_rules));
        route_obj.insert("final".to_string(), json!(final_outbound));

        let mut cache_file_obj = Map::new();
        cache_file_obj.insert("enabled".to_string(), json!(input.settings.cache_file_enabled));
        if input.settings.cache_file_enabled {
            if let Some(p) = input.cache_db_path {
                cache_file_obj.insert("path".to_string(), json!(p.to_string_lossy()));
            }
            if input.settings.dns_fake_ip && input.settings.cache_store_fakeip {
                cache_file_obj.insert("store_fakeip".to_string(), json!(true));
            }
        }

        let log_level = match input.settings.log_level.as_str() {
            "trace" | "debug" | "info" | "warn" | "error" => input.settings.log_level.as_str(),
            _ => "info",
        };

        let mut root = Map::new();
        root.insert("log".to_string(), json!({ "level": log_level, "timestamp": true }));
        root.insert(
            "http_clients".to_string(),
            json!([
                { "tag": "proxy-http", "detour": "main-exit" },
                { "tag": "direct-http", "detour": "direct" }
            ]),
        );
        root.insert("dns".to_string(), Value::Object(dns_obj));
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
        Protocol::Http => {
            if let Some(ref user) = node.username {
                if !user.is_empty() {
                    obj.insert("username".to_string(), json!(user));
                }
            }
            if let Some(ref pass) = node.password {
                if !pass.is_empty() {
                    obj.insert("password".to_string(), json!(pass));
                }
            }
        }
        Protocol::Socks => {
            obj.insert(
                "version".to_string(),
                json!(node.socks_version.as_deref().unwrap_or("5")),
            );
            if let Some(ref user) = node.username {
                if !user.is_empty() {
                    obj.insert("username".to_string(), json!(user));
                }
            }
            if let Some(ref pass) = node.password {
                if !pass.is_empty() {
                    obj.insert("password".to_string(), json!(pass));
                }
            }
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

pub(crate) fn compile_dns_server(tag: &str, raw_spec: &str, detour: Option<&str>) -> (Value, bool) {
    let spec = raw_spec.trim();
    let fallback_ip = if detour.is_some() { "1.1.1.1" } else { "223.5.5.5" };
    let effective = if spec.is_empty() { fallback_ip } else { spec };

    let mut obj = Map::new();
    obj.insert("tag".to_string(), json!(tag));
    let mut needs_bootstrap = false;

    if let Ok(parsed) = url::Url::parse(effective) {
        let scheme = parsed.scheme().to_ascii_lowercase();
        let host = parsed.host_str().unwrap_or(fallback_ip).to_string();
        let is_domain = host.parse::<std::net::IpAddr>().is_err();
        match scheme.as_str() {
            "https" | "h3" => {
                obj.insert("type".to_string(), json!(scheme));
                obj.insert("server".to_string(), json!(host));
                if let Some(port) = parsed.port() {
                    obj.insert("server_port".to_string(), json!(port));
                }
                let path = if parsed.path().is_empty() || parsed.path() == "/" {
                    "/dns-query"
                } else {
                    parsed.path()
                };
                obj.insert("path".to_string(), json!(path));
                if is_domain {
                    obj.insert("domain_resolver".to_string(), json!("dns-bootstrap"));
                    needs_bootstrap = true;
                }
            }
            "tls" | "quic" => {
                obj.insert("type".to_string(), json!(scheme));
                obj.insert("server".to_string(), json!(host));
                if let Some(port) = parsed.port() {
                    obj.insert("server_port".to_string(), json!(port));
                }
                if is_domain {
                    obj.insert("domain_resolver".to_string(), json!("dns-bootstrap"));
                    needs_bootstrap = true;
                }
            }
            "tcp" => {
                obj.insert("type".to_string(), json!("tcp"));
                obj.insert("server".to_string(), json!(host));
                if let Some(port) = parsed.port() {
                    obj.insert("server_port".to_string(), json!(port));
                }
            }
            _ => {
                obj.insert("type".to_string(), json!("udp"));
                obj.insert("server".to_string(), json!(host));
                if let Some(port) = parsed.port() {
                    obj.insert("server_port".to_string(), json!(port));
                }
            }
        }
    } else {
        obj.insert("type".to_string(), json!("udp"));
        obj.insert("server".to_string(), json!(effective));
    }

    if let Some(d) = detour {
        obj.insert("detour".to_string(), json!(d));
    }
    (Value::Object(obj), needs_bootstrap)
}

pub(crate) fn offline_srs_fallback_domains(tag: &str) -> Vec<&'static str> {
    match tag {
        "geosite-cn" => vec!["cn", "baidu.com", "qq.com", "bilibili.com", "taobao.com", "jd.com", "zhihu.com"],
        "geosite-private" => vec!["local", "lan", "localhost", "home.arpa"],
        "geosite-gfw" | "geosite-geolocation-!cn" => vec!["google.com", "youtube.com", "github.com", "openai.com"],
        "geosite-category-ai-!cn" | "geosite-openai" => vec!["openai.com", "chatgpt.com", "claude.ai", "anthropic.com"],
        "geosite-netflix" => vec!["netflix.com", "nflxvideo.net"],
        "geosite-youtube" => vec!["youtube.com", "youtu.be", "googlevideo.com"],
        "geosite-telegram" => vec!["telegram.org", "t.me"],
        _ => Vec::new(),
    }
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
