use crate::model::{NodeIR, Protocol, TlsConfig, TransportConfig};
use crate::parser::ImportSummary;
use anyhow::{anyhow, Result};
use saphyr::{LoadableYamlNode, Yaml};
use std::collections::BTreeMap;
use uuid::Uuid;

pub fn parse_clash_yaml(raw: &str, default_group: &str) -> Result<ImportSummary> {
    let docs = Yaml::load_from_str(raw).map_err(|e| anyhow!("YAML parse error: {:?}", e))?;
    let mut nodes = Vec::new();
    let mut skipped = 0usize;
    let mut warnings = Vec::new();

    for doc in docs {
        let Some(proxies) = doc.as_mapping_get("proxies").and_then(|v| v.as_vec()) else {
            continue;
        };
        for p in proxies {
            match parse_clash_proxy(p, default_group) {
                Some(n) => nodes.push(n),
                None => {
                    skipped += 1;
                    warnings.push("Skipped unsupported Clash proxy entry".to_string());
                }
            }
        }
    }

    Ok(ImportSummary {
        nodes,
        skipped,
        warnings,
    })
}

fn next_id(prefix: &str) -> String {
    let raw = Uuid::new_v4().simple().to_string();
    format!("n-{}-{}", prefix, &raw[..8])
}

fn yaml_get<'a>(node: &'a Yaml<'a>, key: &str) -> Option<&'a Yaml<'a>> {
    node.as_mapping_get(key)
}

fn yaml_str<'a>(node: &'a Yaml<'a>, key: &str) -> Option<String> {
    match yaml_get(node, key)? {
        Yaml::Value(saphyr::Scalar::String(s)) => Some(s.to_string()),
        Yaml::Value(saphyr::Scalar::Integer(i)) => Some(i.to_string()),
        _ => None,
    }
}

fn yaml_u16(node: &Yaml<'_>, key: &str) -> Option<u16> {
    match yaml_get(node, key)? {
        Yaml::Value(saphyr::Scalar::Integer(i)) => u16::try_from(*i).ok(),
        Yaml::Value(saphyr::Scalar::String(s)) => s.parse::<u16>().ok(),
        _ => None,
    }
}

fn yaml_bool(node: &Yaml<'_>, key: &str) -> bool {
    matches!(yaml_get(node, key), Some(Yaml::Value(saphyr::Scalar::Boolean(true))))
}

fn parse_clash_proxy(p: &Yaml<'_>, group: &str) -> Option<NodeIR> {
    let ptype = yaml_str(p, "type")?.to_ascii_lowercase();
    let server = yaml_str(p, "server")?;
    let port = yaml_u16(p, "port")?;
    let name = yaml_str(p, "name").unwrap_or_else(|| format!("{}:{}", server, port));

    let protocol = match ptype.as_str() {
        "vless" => Protocol::Vless,
        "vmess" => Protocol::Vmess,
        "trojan" => Protocol::Trojan,
        "ss" | "shadowsocks" => Protocol::Shadowsocks,
        "hysteria2" | "hy2" => Protocol::Hysteria2,
        "tuic" => Protocol::Tuic,
        "anytls" => Protocol::Anytls,
        "wireguard" => Protocol::Wireguard,
        "http" => Protocol::Http,
        "socks5" | "socks" => Protocol::Socks,
        _ => return None,
    };

    let mut node = NodeIR::new(next_id(&ptype), protocol, name, &server, port, group);
    node.uuid = yaml_str(p, "uuid");
    node.username = yaml_str(p, "username");
    node.password = yaml_str(p, "password");
    if protocol == Protocol::Socks {
        node.socks_version = Some("5".to_string());
    }
    node.method = yaml_str(p, "cipher").or_else(|| yaml_str(p, "method"));
    node.flow = yaml_str(p, "flow");
    node.alter_id = yaml_u16(p, "alterId");
    node.congestion_control = yaml_str(p, "congestion-controller");

    let tls_flag = yaml_bool(p, "tls")
        || matches!(
            protocol,
            Protocol::Trojan | Protocol::Hysteria2 | Protocol::Tuic | Protocol::Anytls
        );
    if tls_flag {
        let sni = yaml_str(p, "sni")
            .or_else(|| yaml_str(p, "servername"))
            .unwrap_or_else(|| server.clone());
        let reality_pub = yaml_get(p, "reality-opts").and_then(|o| yaml_str(o, "public-key"));
        let reality_sid = yaml_get(p, "reality-opts").and_then(|o| yaml_str(o, "short-id"));
        node.tls = Some(TlsConfig {
            enabled: true,
            server_name: Some(sni),
            insecure: yaml_bool(p, "skip-cert-verify"),
            alpn: Vec::new(),
            fingerprint: yaml_str(p, "client-fingerprint"),
            reality_public_key: reality_pub,
            reality_short_id: reality_sid,
        });
    }

    if let Some(net) = yaml_str(p, "network") {
        if net.eq_ignore_ascii_case("ws") {
            let mut headers = BTreeMap::new();
            let ws_opts = yaml_get(p, "ws-opts");
            let path = ws_opts
                .and_then(|o| yaml_str(o, "path"))
                .or_else(|| Some("/".to_string()));
            if let Some(host) = ws_opts
                .and_then(|o| yaml_get(o, "headers"))
                .and_then(|h| yaml_str(h, "Host"))
            {
                headers.insert("Host".to_string(), host);
            }
            node.transport = Some(TransportConfig {
                transport_type: "ws".to_string(),
                path,
                host: headers.get("Host").cloned(),
                service_name: None,
                headers,
            });
        } else if net.eq_ignore_ascii_case("grpc") {
            node.transport = Some(TransportConfig {
                transport_type: "grpc".to_string(),
                path: None,
                host: None,
                service_name: yaml_get(p, "grpc-opts")
                    .and_then(|o| yaml_str(o, "grpc-service-name")),
                headers: BTreeMap::new(),
            });
        }
    }

    if protocol == Protocol::Wireguard {
        node.wg_private_key = yaml_str(p, "private-key");
        node.wg_peer_public_key = yaml_str(p, "public-key");
        if let Some(ip) = yaml_str(p, "ip") {
            node.wg_local_address = vec![if ip.contains('/') { ip } else { format!("{}/32", ip) }];
        } else {
            node.wg_local_address = vec!["10.0.0.2/32".to_string()];
        }
    }

    Some(node)
}
