use crate::model::{NodeIR, Protocol, TlsConfig, TransportConfig};
use crate::parser::ImportSummary;
use anyhow::{anyhow, Context, Result};
use base64::engine::general_purpose::{STANDARD, STANDARD_NO_PAD, URL_SAFE, URL_SAFE_NO_PAD};
use base64::Engine;
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};
use url::Url;
use uuid::Uuid;

pub fn try_decode_base64_text(input: &str) -> Option<String> {
    let compact: String = input.chars().filter(|c| !c.is_whitespace()).collect();
    if compact.len() < 4 {
        return None;
    }
    let engines = [&STANDARD, &STANDARD_NO_PAD, &URL_SAFE, &URL_SAFE_NO_PAD];
    for engine in engines {
        if let Ok(bytes) = engine.decode(&compact) {
            if let Ok(text) = String::from_utf8(bytes) {
                return Some(text);
            }
        }
    }
    None
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(hex_str) = std::str::from_utf8(&bytes[i + 1..i + 3]) {
                if let Ok(val) = u8::from_str_radix(hex_str, 16) {
                    out.push(val);
                    i += 3;
                    continue;
                }
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn query_map(url: &Url) -> HashMap<String, String> {
    url.query_pairs()
        .map(|(k, v)| (k.to_ascii_lowercase(), v.into_owned()))
        .collect()
}

fn next_node_id(prefix: &str) -> String {
    let raw = Uuid::new_v4().simple().to_string();
    format!("n-{}-{}", prefix, &raw[..8])
}

fn build_transport(params: &HashMap<String, String>) -> Option<TransportConfig> {
    let net_type = params
        .get("type")
        .or_else(|| params.get("net"))
        .map(|s| s.to_ascii_lowercase())
        .unwrap_or_default();
    match net_type.as_str() {
        "ws" | "websocket" => {
            let mut headers = BTreeMap::new();
            if let Some(host) = params.get("host").filter(|s| !s.is_empty()) {
                headers.insert("Host".to_string(), host.clone());
            }
            Some(TransportConfig {
                transport_type: "ws".to_string(),
                path: params.get("path").cloned().or_else(|| Some("/".to_string())),
                host: params.get("host").cloned(),
                service_name: None,
                headers,
            })
        }
        "grpc" => Some(TransportConfig {
            transport_type: "grpc".to_string(),
            path: None,
            host: None,
            service_name: params
                .get("servicename")
                .or_else(|| params.get("service_name"))
                .or_else(|| params.get("path"))
                .cloned(),
            headers: BTreeMap::new(),
        }),
        "httpupgrade" => Some(TransportConfig {
            transport_type: "httpupgrade".to_string(),
            path: params.get("path").cloned().or_else(|| Some("/".to_string())),
            host: params.get("host").cloned(),
            service_name: None,
            headers: BTreeMap::new(),
        }),
        _ => None,
    }
}

fn build_tls(params: &HashMap<String, String>, server: &str, force_tls: bool) -> Option<TlsConfig> {
    let sec = params
        .get("security")
        .map(|s| s.to_ascii_lowercase())
        .unwrap_or_default();
    let is_reality = sec == "reality" || params.contains_key("pbk");
    let is_tls = force_tls || sec == "tls" || is_reality;
    if !is_tls {
        return None;
    }

    let sni = params
        .get("sni")
        .or_else(|| params.get("peer"))
        .filter(|s| !s.is_empty())
        .cloned()
        .unwrap_or_else(|| server.to_string());

    let insecure = params
        .get("insecure")
        .or_else(|| params.get("allowinsecure"))
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);

    let alpn = params
        .get("alpn")
        .map(|v| {
            v.split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let fp = params
        .get("fp")
        .filter(|s| !s.is_empty())
        .cloned()
        .or_else(|| if is_reality { Some("chrome".to_string()) } else { None });

    Some(TlsConfig {
        enabled: true,
        server_name: Some(sni),
        insecure,
        alpn,
        fingerprint: fp,
        reality_public_key: params.get("pbk").filter(|s| !s.is_empty()).cloned(),
        reality_short_id: params.get("sid").cloned(),
    })
}

pub fn parse_uri_batch(raw: &str, default_group: &str) -> ImportSummary {
    let mut nodes = Vec::new();
    let mut skipped = 0usize;
    let mut warnings = Vec::new();

    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        match parse_single_uri(trimmed, default_group) {
            Ok(node) => nodes.push(node),
            Err(err) => {
                skipped += 1;
                warnings.push(format!("Skipped line: {}", err));
            }
        }
    }

    ImportSummary {
        nodes,
        skipped,
        warnings,
    }
}

pub fn parse_single_uri(uri: &str, default_group: &str) -> Result<NodeIR> {
    let lower = uri.to_ascii_lowercase();
    if lower.starts_with("vless://") {
        parse_vless(uri, default_group)
    } else if lower.starts_with("vmess://") {
        parse_vmess(uri, default_group)
    } else if lower.starts_with("trojan://") {
        parse_trojan(uri, default_group)
    } else if lower.starts_with("ss://") {
        parse_shadowsocks(uri, default_group)
    } else if lower.starts_with("hysteria2://") || lower.starts_with("hy2://") {
        parse_hysteria2(uri, default_group)
    } else if lower.starts_with("tuic://") {
        parse_tuic(uri, default_group)
    } else if lower.starts_with("anytls://") {
        parse_anytls(uri, default_group)
    } else if lower.starts_with("wg://") || lower.starts_with("wireguard://") {
        parse_wireguard(uri, default_group)
    } else if lower.starts_with("http://") || lower.starts_with("https://") {
        parse_http_proxy(uri, default_group)
    } else if lower.starts_with("socks://")
        || lower.starts_with("socks5://")
        || lower.starts_with("socks5h://")
        || lower.starts_with("socks4://")
        || lower.starts_with("socks4a://")
    {
        parse_socks_proxy(uri, default_group)
    } else {
        Err(anyhow!("Unsupported URI scheme"))
    }
}

fn parse_vless(uri: &str, group: &str) -> Result<NodeIR> {
    let url = Url::parse(uri).context("Invalid VLESS URI")?;
    let server = url.host_str().ok_or_else(|| anyhow!("VLESS missing host"))?.to_string();
    let port = url.port().unwrap_or(443);
    let uuid = percent_decode(url.username());
    if uuid.is_empty() {
        return Err(anyhow!("VLESS missing UUID"));
    }
    let remark = url
        .fragment()
        .map(percent_decode)
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| format!("{}:{}", server, port));
    let params = query_map(&url);

    let mut node = NodeIR::new(next_node_id("vless"), Protocol::Vless, remark, &server, port, group);
    node.uuid = Some(uuid);
    node.flow = params.get("flow").filter(|s| !s.is_empty()).cloned();
    node.tls = build_tls(&params, &server, false);
    node.transport = build_transport(&params);
    node.raw_uri = Some(uri.to_string());
    Ok(node)
}

fn parse_vmess(uri: &str, group: &str) -> Result<NodeIR> {
    let b64 = uri
        .strip_prefix("vmess://")
        .or_else(|| uri.strip_prefix("VMESS://"))
        .ok_or_else(|| anyhow!("Invalid VMess prefix"))?;
    let json_text = try_decode_base64_text(b64).ok_or_else(|| anyhow!("Invalid VMess base64"))?;
    let v: Value = serde_json::from_str(&json_text).context("Invalid VMess JSON")?;

    let server = v["add"]
        .as_str()
        .ok_or_else(|| anyhow!("VMess missing add"))?
        .to_string();
    let port = match &v["port"] {
        Value::Number(n) => n.as_u64().unwrap_or(443) as u16,
        Value::String(s) => s.parse::<u16>().unwrap_or(443),
        _ => 443,
    };
    let uuid = v["id"]
        .as_str()
        .ok_or_else(|| anyhow!("VMess missing id"))?
        .to_string();
    let remark = v["ps"]
        .as_str()
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("{}:{}", server, port));

    let mut node = NodeIR::new(next_node_id("vmess"), Protocol::Vmess, remark, &server, port, group);
    node.uuid = Some(uuid);
    node.alter_id = Some(match &v["aid"] {
        Value::Number(n) => n.as_u64().unwrap_or(0) as u16,
        Value::String(s) => s.parse::<u16>().unwrap_or(0),
        _ => 0,
    });
    node.security = v["scy"]
        .as_str()
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .or_else(|| Some("auto".to_string()));

    let mut params = HashMap::new();
    if let Some(net) = v["net"].as_str() {
        params.insert("type".to_string(), net.to_string());
    }
    if let Some(host) = v["host"].as_str() {
        params.insert("host".to_string(), host.to_string());
    }
    if let Some(path) = v["path"].as_str() {
        params.insert("path".to_string(), path.to_string());
    }
    if let Some(sni) = v["sni"].as_str().filter(|s| !s.is_empty()) {
        params.insert("sni".to_string(), sni.to_string());
    }
    if v["tls"].as_str().map(|s| s.eq_ignore_ascii_case("tls")).unwrap_or(false) {
        params.insert("security".to_string(), "tls".to_string());
    }

    node.tls = build_tls(&params, &server, false);
    node.transport = build_transport(&params);
    node.raw_uri = Some(uri.to_string());
    Ok(node)
}

fn parse_trojan(uri: &str, group: &str) -> Result<NodeIR> {
    let url = Url::parse(uri).context("Invalid Trojan URI")?;
    let server = url.host_str().ok_or_else(|| anyhow!("Trojan missing host"))?.to_string();
    let port = url.port().unwrap_or(443);
    let password = percent_decode(url.username());
    if password.is_empty() {
        return Err(anyhow!("Trojan missing password"));
    }
    let remark = url
        .fragment()
        .map(percent_decode)
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| format!("{}:{}", server, port));
    let params = query_map(&url);

    let mut node = NodeIR::new(next_node_id("trojan"), Protocol::Trojan, remark, &server, port, group);
    node.password = Some(password);
    node.tls = build_tls(&params, &server, true);
    node.transport = build_transport(&params);
    node.raw_uri = Some(uri.to_string());
    Ok(node)
}

fn parse_shadowsocks(uri: &str, group: &str) -> Result<NodeIR> {
    let rest = uri
        .strip_prefix("ss://")
        .or_else(|| uri.strip_prefix("SS://"))
        .ok_or_else(|| anyhow!("Invalid SS prefix"))?;

    let (body, frag) = match rest.split_once('#') {
        Some((b, f)) => (b, Some(percent_decode(f))),
        None => (rest, None),
    };
    let (main_part, _query) = match body.split_once('?') {
        Some((m, q)) => (m, Some(q)),
        None => (body, None),
    };

    let (method, password, server, port) = if let Some((userinfo, hostport)) = main_part.split_once('@') {
        let decoded_user = if userinfo.contains(':') {
            percent_decode(userinfo)
        } else {
            try_decode_base64_text(userinfo).ok_or_else(|| anyhow!("Invalid SS SIP002 userinfo"))?
        };
        let (m, p) = decoded_user
            .split_once(':')
            .ok_or_else(|| anyhow!("Invalid SS method:password"))?;
        let (h, pt) = split_host_port(hostport)?;
        (m.to_string(), p.to_string(), h, pt)
    } else {
        let decoded = try_decode_base64_text(main_part).ok_or_else(|| anyhow!("Invalid legacy SS base64"))?;
        let (mp, hp) = decoded
            .rsplit_once('@')
            .ok_or_else(|| anyhow!("Invalid legacy SS format"))?;
        let (m, p) = mp
            .split_once(':')
            .ok_or_else(|| anyhow!("Invalid legacy SS method:password"))?;
        let (h, pt) = split_host_port(hp)?;
        (m.to_string(), p.to_string(), h, pt)
    };

    let remark = frag
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| format!("{}:{}", server, port));
    let mut node = NodeIR::new(next_node_id("ss"), Protocol::Shadowsocks, remark, server, port, group);
    node.method = Some(method);
    node.password = Some(password);
    node.raw_uri = Some(uri.to_string());
    Ok(node)
}

fn split_host_port(hp: &str) -> Result<(String, u16)> {
    let cleaned = hp.trim_end_matches('/');
    if let Some(rest) = cleaned.strip_prefix('[') {
        let (host, port_part) = rest
            .split_once("]:")
            .ok_or_else(|| anyhow!("Invalid IPv6 host:port"))?;
        let port = port_part.parse::<u16>().context("Invalid port")?;
        return Ok((host.to_string(), port));
    }
    let (host, port_part) = cleaned
        .rsplit_once(':')
        .ok_or_else(|| anyhow!("Missing port in host:port"))?;
    let port = port_part.parse::<u16>().context("Invalid port")?;
    Ok((host.to_string(), port))
}

fn parse_hysteria2(uri: &str, group: &str) -> Result<NodeIR> {
    let normalized = if uri.to_ascii_lowercase().starts_with("hy2://") {
        format!("hysteria2://{}", &uri[6..])
    } else {
        uri.to_string()
    };
    let url = Url::parse(&normalized).context("Invalid Hysteria2 URI")?;
    let server = url.host_str().ok_or_else(|| anyhow!("Hysteria2 missing host"))?.to_string();
    let port = url.port().unwrap_or(443);
    let password = percent_decode(url.username());
    let remark = url
        .fragment()
        .map(percent_decode)
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| format!("{}:{}", server, port));
    let params = query_map(&url);

    let mut node = NodeIR::new(next_node_id("hy2"), Protocol::Hysteria2, remark, &server, port, group);
    node.password = Some(password);
    node.obfs_type = params.get("obfs").filter(|s| !s.is_empty() && *s != "none").cloned();
    node.obfs_password = params.get("obfs-password").filter(|s| !s.is_empty()).cloned();
    node.up_mbps = params.get("upmbps").and_then(|s| s.parse::<u32>().ok());
    node.down_mbps = params.get("downmbps").and_then(|s| s.parse::<u32>().ok());
    node.tls = build_tls(&params, &server, true);
    node.raw_uri = Some(uri.to_string());
    Ok(node)
}

fn parse_tuic(uri: &str, group: &str) -> Result<NodeIR> {
    let url = Url::parse(uri).context("Invalid TUIC URI")?;
    let server = url.host_str().ok_or_else(|| anyhow!("TUIC missing host"))?.to_string();
    let port = url.port().unwrap_or(443);
    let uuid = percent_decode(url.username());
    let password = url.password().map(percent_decode).unwrap_or_default();
    let remark = url
        .fragment()
        .map(percent_decode)
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| format!("{}:{}", server, port));
    let params = query_map(&url);

    let mut node = NodeIR::new(next_node_id("tuic"), Protocol::Tuic, remark, &server, port, group);
    node.uuid = Some(uuid);
    node.password = Some(password);
    node.congestion_control = params
        .get("congestion_control")
        .filter(|s| !s.is_empty())
        .cloned()
        .or_else(|| Some("bbr".to_string()));
    let mut tls = build_tls(&params, &server, true).unwrap_or_default();
    if tls.alpn.is_empty() {
        tls.alpn.push("h3".to_string());
    }
    node.tls = Some(tls);
    node.raw_uri = Some(uri.to_string());
    Ok(node)
}

fn parse_anytls(uri: &str, group: &str) -> Result<NodeIR> {
    let url = Url::parse(uri).context("Invalid AnyTLS URI")?;
    let server = url.host_str().ok_or_else(|| anyhow!("AnyTLS missing host"))?.to_string();
    let port = url.port().unwrap_or(443);
    let password = percent_decode(url.username());
    let remark = url
        .fragment()
        .map(percent_decode)
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| format!("{}:{}", server, port));
    let params = query_map(&url);

    let mut node = NodeIR::new(next_node_id("anytls"), Protocol::Anytls, remark, &server, port, group);
    node.password = Some(password);
    node.tls = build_tls(&params, &server, true);
    node.raw_uri = Some(uri.to_string());
    Ok(node)
}

fn parse_wireguard(uri: &str, group: &str) -> Result<NodeIR> {
    let normalized = if uri.to_ascii_lowercase().starts_with("wg://") {
        format!("wireguard://{}", &uri[5..])
    } else {
        uri.to_string()
    };
    let url = Url::parse(&normalized).context("Invalid WireGuard URI")?;
    let server = url.host_str().ok_or_else(|| anyhow!("WireGuard missing host"))?.to_string();
    let port = url.port().unwrap_or(51820);
    let private_key = percent_decode(url.username());
    let remark = url
        .fragment()
        .map(percent_decode)
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| format!("{}:{}", server, port));
    let params = query_map(&url);

    let mut node = NodeIR::new(next_node_id("wg"), Protocol::Wireguard, remark, &server, port, group);
    node.wg_private_key = if private_key.is_empty() {
        params.get("privatekey").or_else(|| params.get("private_key")).cloned()
    } else {
        Some(private_key)
    };
    node.wg_peer_public_key = params
        .get("publickey")
        .or_else(|| params.get("public_key"))
        .or_else(|| params.get("peer_public_key"))
        .cloned();
    node.wg_preshared_key = params
        .get("presharedkey")
        .or_else(|| params.get("pre_shared_key"))
        .cloned();
    node.wg_local_address = params
        .get("address")
        .map(|a| {
            a.split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        })
        .filter(|v: &Vec<String>| !v.is_empty())
        .unwrap_or_else(|| vec!["10.0.0.2/32".to_string()]);
    node.wg_mtu = params.get("mtu").and_then(|s| s.parse::<u16>().ok());
    node.wg_reserved = params.get("reserved").map(|r| {
        r.split(',')
            .filter_map(|n| n.trim().parse::<u8>().ok())
            .collect()
    });
    node.raw_uri = Some(uri.to_string());
    Ok(node)
}

fn parse_http_proxy(uri: &str, group: &str) -> Result<NodeIR> {
    let is_https = uri.to_ascii_lowercase().starts_with("https://");
    let url = Url::parse(uri).context("Invalid HTTP(S) proxy URI")?;
    let server = url
        .host_str()
        .ok_or_else(|| anyhow!("HTTP proxy missing host"))?
        .to_string();
    let port = url.port().unwrap_or(if is_https { 443 } else { 8080 });
    let remark = url
        .fragment()
        .map(percent_decode)
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| format!("{}:{}", server, port));
    let params = query_map(&url);

    let mut node = NodeIR::new(next_node_id("http"), Protocol::Http, remark, &server, port, group);
    apply_basic_auth(&mut node, url.username(), url.password());
    node.tls = build_tls(&params, &server, is_https);
    node.raw_uri = Some(uri.to_string());
    Ok(node)
}

fn parse_socks_proxy(uri: &str, group: &str) -> Result<NodeIR> {
    let lower = uri.to_ascii_lowercase();
    let (scheme_len, default_ver) = if lower.starts_with("socks5h://") {
        (10, "5")
    } else if lower.starts_with("socks5://") {
        (9, "5")
    } else if lower.starts_with("socks4a://") {
        (10, "4a")
    } else if lower.starts_with("socks4://") {
        (9, "4")
    } else {
        (8, "5")
    };

    let rest = &uri[scheme_len..];
    let (body, frag) = match rest.split_once('#') {
        Some((b, f)) => (b, Some(percent_decode(f))),
        None => (rest, None),
    };

    let normalized = if !body.contains(':') && !body.contains('@') {
        if let Some(decoded) = try_decode_base64_text(body) {
            format!("socks5://{}", decoded.trim())
        } else {
            format!("socks5://{}", body)
        }
    } else {
        format!("socks5://{}", body)
    };

    let url = Url::parse(&normalized).context("Invalid SOCKS proxy URI")?;
    let server = url
        .host_str()
        .ok_or_else(|| anyhow!("SOCKS proxy missing host"))?
        .to_string();
    let port = url.port().unwrap_or(1080);
    let remark = frag
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| format!("{}:{}", server, port));

    let mut node = NodeIR::new(next_node_id("socks"), Protocol::Socks, remark, server, port, group);
    node.socks_version = Some(default_ver.to_string());
    apply_basic_auth(&mut node, url.username(), url.password());
    node.raw_uri = Some(uri.to_string());
    Ok(node)
}

fn apply_basic_auth(node: &mut NodeIR, raw_user: &str, raw_pass: Option<&str>) {
    let user = percent_decode(raw_user);
    let pass = raw_pass.map(percent_decode);
    if !user.is_empty() && pass.is_none() {
        if let Some(decoded) = try_decode_base64_text(&user) {
            if let Some((u, p)) = decoded.split_once(':') {
                node.username = Some(u.to_string());
                node.password = Some(p.to_string());
                return;
            }
        }
        node.username = Some(user);
    } else {
        if !user.is_empty() {
            node.username = Some(user);
        }
        node.password = pass.filter(|s| !s.is_empty());
    }
}
