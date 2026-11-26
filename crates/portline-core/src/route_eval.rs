use crate::compiler::{match_auto_nodes, offline_srs_fallback_domains};
use crate::model::{AppSettings, NodeIR, RouteMode, RouteTestResult, SimpleRoutingRule};
use crate::rule_cache::RuleCache;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::path::Path;

/// Extracts a clean lowercase hostname or IP address from user input
/// (supports full URLs, `host:port`, `[ipv6]:port`, or bare domains/IPs).
pub fn normalize_test_target(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    if trimmed.contains("://") {
        if let Ok(parsed) = url::Url::parse(trimmed) {
            if let Some(host) = parsed.host_str() {
                return host
                    .trim_start_matches('[')
                    .trim_end_matches(']')
                    .trim_end_matches('.')
                    .to_ascii_lowercase();
            }
        }
    }
    // Strip path/query if user pasted `example.com/path?x=1` without scheme.
    let before_slash = trimmed.split('/').next().unwrap_or(trimmed);
    if before_slash.starts_with('[') {
        if let Some(end) = before_slash.find(']') {
            return before_slash[1..end].to_ascii_lowercase();
        }
    }
    if before_slash.parse::<IpAddr>().is_ok() {
        return before_slash.to_ascii_lowercase();
    }
    if let Some((host, port)) = before_slash.rsplit_once(':') {
        if !host.is_empty() && port.chars().all(|c| c.is_ascii_digit()) {
            return host.trim_end_matches('.').to_ascii_lowercase();
        }
    }
    before_slash.trim_end_matches('.').to_ascii_lowercase()
}

pub fn is_private_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => is_private_ipv4(v4),
        IpAddr::V6(v6) => is_private_ipv6(v6),
    }
}

fn is_private_ipv4(ip: Ipv4Addr) -> bool {
    let [a, b, _, _] = ip.octets();
    ip.is_private()
        || ip.is_loopback()
        || ip.is_link_local()
        || ip.is_broadcast()
        || ip.is_unspecified()
        || ip.is_multicast()
        || (a == 100 && (64..=127).contains(&b))
        || (a == 198 && (b == 18 || b == 19))
}

fn is_private_ipv6(ip: Ipv6Addr) -> bool {
    let seg0 = ip.segments()[0];
    ip.is_loopback()
        || ip.is_unspecified()
        || ip.is_multicast()
        || (seg0 & 0xfe00) == 0xfc00
        || (seg0 & 0xffc0) == 0xfe80
}

pub fn ip_matches_cidr(ip: IpAddr, cidr: &str) -> bool {
    let trimmed = cidr.trim();
    if let Ok(exact) = trimmed.parse::<IpAddr>() {
        return ip == exact;
    }
    let Some((net_str, prefix_str)) = trimmed.split_once('/') else {
        return false;
    };
    let Ok(net_ip) = net_str.trim().parse::<IpAddr>() else {
        return false;
    };
    let Ok(prefix) = prefix_str.trim().parse::<u32>() else {
        return false;
    };
    match (ip, net_ip) {
        (IpAddr::V4(a), IpAddr::V4(b)) if prefix <= 32 => {
            let mask = if prefix == 0 {
                0u32
            } else {
                !0u32 << (32 - prefix)
            };
            (u32::from(a) & mask) == (u32::from(b) & mask)
        }
        (IpAddr::V6(a), IpAddr::V6(b)) if prefix <= 128 => {
            let mask = if prefix == 0 {
                0u128
            } else {
                !0u128 << (128 - prefix)
            };
            (u128::from(a) & mask) == (u128::from(b) & mask)
        }
        _ => false,
    }
}

fn domain_matches_suffix(host: &str, suffix: &str) -> bool {
    let clean = suffix.trim().trim_start_matches('.').to_ascii_lowercase();
    if clean.is_empty() {
        return false;
    }
    host == clean || host.ends_with(&format!(".{}", clean))
}

fn resolve_exit(
    target: &str,
    nodes: &[NodeIR],
    main_node_id: Option<&str>,
) -> (String, String, Option<String>, Option<String>) {
    if target == "direct" {
        return ("direct".to_string(), "direct".to_string(), None, None);
    }
    if target == "reject" {
        return ("reject".to_string(), "reject".to_string(), None, None);
    }
    let main_node = main_node_id
        .and_then(|id| nodes.iter().find(|n| n.id == id))
        .or_else(|| nodes.first());

    if target == "main" {
        return match main_node {
            Some(n) => (
                "node".to_string(),
                n.remark.clone(),
                Some(n.id.clone()),
                Some(n.region.clone()),
            ),
            None => ("none".to_string(), "none".to_string(), None, None),
        };
    }

    if target.starts_with("auto:") {
        let matched_ids = match_auto_nodes(target, nodes);
        let candidates: Vec<&NodeIR> = if matched_ids.is_empty() {
            nodes.iter().collect()
        } else {
            nodes
                .iter()
                .filter(|n| matched_ids.contains(&n.id))
                .collect()
        };
        let best = candidates
            .into_iter()
            .min_by_key(|n| n.real_delay_ms.or(n.tcping_ms).unwrap_or(u32::MAX));
        return match best {
            Some(n) => (
                "auto".to_string(),
                n.remark.clone(),
                Some(n.id.clone()),
                Some(n.region.clone()),
            ),
            None => ("none".to_string(), "none".to_string(), None, None),
        };
    }

    let specific = nodes.iter().find(|n| n.id == target).or(main_node);
    match specific {
        Some(n) => (
            "node".to_string(),
            n.remark.clone(),
            Some(n.id.clone()),
            Some(n.region.clone()),
        ),
        None => ("none".to_string(), "none".to_string(), None, None),
    }
}

fn evaluate_dns_decision(
    host: &str,
    is_ip: bool,
    settings: &AppSettings,
    rules: &[SimpleRoutingRule],
    rule_cache: Option<&RuleCache>,
    sing_box_path: Option<&Path>,
) -> (String, String) {
    if is_ip {
        return ("none".to_string(), "ip-literal".to_string());
    }
    for h in &settings.dns_hosts {
        if h.domain.trim().eq_ignore_ascii_case(host) && !h.ips.is_empty() {
            return ("dns-hosts".to_string(), format!("hosts:{}", h.ips[0]));
        }
    }
    for custom in settings.dns_custom_rules.iter().filter(|r| r.enabled) {
        let mut hit = false;
        if let Some(ref tag) = custom.rule_set {
            if let (Some(cache), Some(sb)) = (rule_cache, sing_box_path) {
                if cache.match_local_srs(sb, tag, host) == Some(true) {
                    hit = true;
                }
            }
        }
        if !hit && custom.domains.iter().any(|d| domain_matches_suffix(host, d)) {
            hit = true;
        }
        if hit {
            let reason = if custom.title.trim().is_empty() {
                "dns-custom".to_string()
            } else {
                format!("dns-rule:{}", custom.title.trim())
            };
            return (custom.server.clone(), reason);
        }
    }
    if settings.route_mode == RouteMode::Direct {
        return ("dns-local".to_string(), "mode:direct".to_string());
    }
    if domain_matches_suffix(host, "cn") {
        return ("dns-local".to_string(), "domain_suffix:cn".to_string());
    }
    for rule in rules.iter().filter(|r| r.enabled && r.target == "direct") {
        if rule.patterns.iter().any(|p| domain_matches_suffix(host, p)) {
            return ("dns-local".to_string(), format!("direct-rule:{}", rule.id));
        }
    }
    if let (Some(cache), Some(sb)) = (rule_cache, sing_box_path) {
        if cache.match_local_srs(sb, "geosite-cn", host) == Some(true) {
            return ("dns-local".to_string(), "rule_set:geosite-cn".to_string());
        }
    }
    if settings.dns_fake_ip {
        return ("dns-fakeip".to_string(), "fakeip".to_string());
    }
    if settings.route_mode == RouteMode::Global {
        return ("dns-remote".to_string(), "mode:global".to_string());
    }
    ("dns-remote".to_string(), "final".to_string())
}

/// Evaluates how Portline's compiled sing-box v1.14+ ruleset will route and resolve a target
/// domain, URL, or IP address (including locally cached binary `.srs` rule-set checks).
pub fn evaluate_route_target(
    raw_input: &str,
    process_name: Option<&str>,
    settings: &AppSettings,
    rules: &[SimpleRoutingRule],
    nodes: &[NodeIR],
    rule_cache: Option<&RuleCache>,
    sing_box_path: Option<&Path>,
) -> RouteTestResult {
    let normalized = normalize_test_target(raw_input);
    let parsed_ip = normalized.parse::<IpAddr>().ok();
    let is_ip = parsed_ip.is_some();
    let private = parsed_ip.map(is_private_ip).unwrap_or(false);
    let proc_clean = process_name
        .map(|s| s.trim().to_ascii_lowercase())
        .filter(|s| !s.is_empty());

    let (dns_server, dns_reason) =
        evaluate_dns_decision(&normalized, is_ip, settings, rules, rule_cache, sing_box_path);

    // 1. Private IP always goes direct (matches `ip_is_private: true -> direct` preamble).
    if private {
        let (kind, label, node_id, region) =
            resolve_exit("direct", nodes, settings.main_node_id.as_deref());
        return RouteTestResult {
            input: raw_input.to_string(),
            normalized_host: normalized,
            is_ip,
            is_private_ip: true,
            route_mode: settings.route_mode,
            matched_rule_id: None,
            matched_rule_title: None,
            matched_by: "ip_is_private".to_string(),
            target: "direct".to_string(),
            resolved_exit_kind: kind,
            resolved_exit_label: label,
            resolved_node_id: node_id,
            resolved_node_region: region,
            dns_server,
            dns_reason,
        };
    }

    // 2. Global / Direct clash_mode overrides rule list.
    if settings.route_mode == RouteMode::Direct {
        let (kind, label, node_id, region) =
            resolve_exit("direct", nodes, settings.main_node_id.as_deref());
        return RouteTestResult {
            input: raw_input.to_string(),
            normalized_host: normalized,
            is_ip,
            is_private_ip: false,
            route_mode: RouteMode::Direct,
            matched_rule_id: None,
            matched_rule_title: None,
            matched_by: "clash_mode:direct".to_string(),
            target: "direct".to_string(),
            resolved_exit_kind: kind,
            resolved_exit_label: label,
            resolved_node_id: node_id,
            resolved_node_region: region,
            dns_server,
            dns_reason,
        };
    }

    if settings.route_mode == RouteMode::Global {
        let (kind, label, node_id, region) =
            resolve_exit("main", nodes, settings.main_node_id.as_deref());
        return RouteTestResult {
            input: raw_input.to_string(),
            normalized_host: normalized,
            is_ip,
            is_private_ip: false,
            route_mode: RouteMode::Global,
            matched_rule_id: None,
            matched_rule_title: None,
            matched_by: "clash_mode:global".to_string(),
            target: "main".to_string(),
            resolved_exit_kind: kind,
            resolved_exit_label: label,
            resolved_node_id: node_id,
            resolved_node_region: region,
            dns_server,
            dns_reason,
        };
    }

    // 3. Rule mode: walk enabled rules in strict priority order.
    let mut sorted: Vec<&SimpleRoutingRule> = rules.iter().filter(|r| r.enabled).collect();
    sorted.sort_by_key(|r| r.priority);

    for rule in sorted {
        let mut matched_by: Option<String> = None;

        // Check local .srs binary rule-set first (if synced & sing-box available),
        // falling back to minimal built-in domains only when `.srs` is not yet cached.
        if let Some(ref srs_tag) = rule.rule_set {
            let clean_tag = srs_tag.trim();
            if !clean_tag.is_empty() {
                let mut checked_srs = false;
                if let (Some(cache), Some(sb)) = (rule_cache, sing_box_path) {
                    if let Some(hit) = cache.match_local_srs(sb, clean_tag, &normalized) {
                        checked_srs = true;
                        if hit {
                            matched_by = Some(format!("rule_set:{}.srs", clean_tag));
                        }
                    }
                }
                if !checked_srs && !is_ip && rule.patterns.is_empty() {
                    let fallbacks = offline_srs_fallback_domains(clean_tag);
                    if let Some(pat) = fallbacks
                        .iter()
                        .find(|p| domain_matches_suffix(&normalized, p))
                    {
                        matched_by = Some(format!("rule_set:{}(fallback:{})", clean_tag, pat));
                    }
                }
            }
        }

        // Check domain_suffix patterns.
        if matched_by.is_none() && !is_ip {
            if let Some(pat) = rule
                .patterns
                .iter()
                .find(|p| domain_matches_suffix(&normalized, p))
            {
                matched_by = Some(format!("domain_suffix:{}", pat.trim().trim_start_matches('.')));
            }
        }

        // Check domain_keyword patterns.
        if matched_by.is_none() && !is_ip {
            if let Some(kw) = rule.domain_keywords.iter().find(|k| {
                let needle = k.trim().to_ascii_lowercase();
                !needle.is_empty() && normalized.contains(&needle)
            }) {
                matched_by = Some(format!("domain_keyword:{}", kw.trim()));
            }
        }

        // Check IP CIDRs.
        if matched_by.is_none() {
            if let Some(ip) = parsed_ip {
                if let Some(cidr) = rule.ip_cidrs.iter().find(|c| ip_matches_cidr(ip, c)) {
                    matched_by = Some(format!("ip_cidr:{}", cidr.trim()));
                }
            }
        }

        // Check optional process name.
        if matched_by.is_none() {
            if let Some(ref proc_val) = proc_clean {
                if let Some(p) = rule
                    .process_names
                    .iter()
                    .find(|pn| pn.trim().eq_ignore_ascii_case(proc_val))
                {
                    matched_by = Some(format!("process_name:{}", p.trim()));
                }
            }
        }

        if let Some(reason) = matched_by {
            let (kind, label, node_id, region) =
                resolve_exit(&rule.target, nodes, settings.main_node_id.as_deref());
            return RouteTestResult {
                input: raw_input.to_string(),
                normalized_host: normalized,
                is_ip,
                is_private_ip: false,
                route_mode: RouteMode::Rule,
                matched_rule_id: Some(rule.id.clone()),
                matched_rule_title: Some(rule.title_en.clone()),
                matched_by: reason,
                target: rule.target.clone(),
                resolved_exit_kind: kind,
                resolved_exit_label: label,
                resolved_node_id: node_id,
                resolved_node_region: region,
                dns_server,
                dns_reason,
            };
        }
    }

    // 4. Fallback (`final: main-exit`).
    let (kind, label, node_id, region) =
        resolve_exit("main", nodes, settings.main_node_id.as_deref());
    RouteTestResult {
        input: raw_input.to_string(),
        normalized_host: normalized,
        is_ip,
        is_private_ip: false,
        route_mode: RouteMode::Rule,
        matched_rule_id: None,
        matched_rule_title: None,
        matched_by: "final".to_string(),
        target: "main".to_string(),
        resolved_exit_kind: kind,
        resolved_exit_label: label,
        resolved_node_id: node_id,
        resolved_node_region: region,
        dns_server,
        dns_reason,
    }
}
