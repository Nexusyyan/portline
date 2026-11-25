use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Protocol {
    Vless,
    Vmess,
    Trojan,
    Shadowsocks,
    Hysteria2,
    Tuic,
    Anytls,
    Wireguard,
}

impl Protocol {
    pub fn as_singbox_type(self) -> &'static str {
        match self {
            Protocol::Vless => "vless",
            Protocol::Vmess => "vmess",
            Protocol::Trojan => "trojan",
            Protocol::Shadowsocks => "shadowsocks",
            Protocol::Hysteria2 => "hysteria2",
            Protocol::Tuic => "tuic",
            Protocol::Anytls => "anytls",
            Protocol::Wireguard => "wireguard",
        }
    }

    pub fn display_badge(self) -> &'static str {
        match self {
            Protocol::Vless => "VLESS",
            Protocol::Vmess => "VMESS",
            Protocol::Trojan => "TROJAN",
            Protocol::Shadowsocks => "SS",
            Protocol::Hysteria2 => "HY2",
            Protocol::Tuic => "TUIC",
            Protocol::Anytls => "ANYTLS",
            Protocol::Wireguard => "WG",
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransportConfig {
    pub transport_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub service_name: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub headers: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TlsConfig {
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server_name: Option<String>,
    #[serde(default)]
    pub insecure: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub alpn: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fingerprint: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reality_public_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reality_short_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeIR {
    pub id: String,
    pub protocol: Protocol,
    pub remark: String,
    pub server: String,
    pub port: u16,
    pub group: String,
    pub region: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uuid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flow: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alter_id: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub security: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub congestion_control: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub up_mbps: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub down_mbps: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub obfs_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub obfs_password: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wg_private_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wg_peer_public_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wg_preshared_key: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub wg_local_address: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wg_mtu: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wg_reserved: Option<Vec<u8>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tls: Option<TlsConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transport: Option<TransportConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detour: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tcping_ms: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub real_delay_ms: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw_uri: Option<String>,
    pub created_at: String,
}

impl NodeIR {
    pub fn new(
        id: impl Into<String>,
        protocol: Protocol,
        remark: impl Into<String>,
        server: impl Into<String>,
        port: u16,
        group: impl Into<String>,
    ) -> Self {
        let remark_str = remark.into();
        let server_str = server.into();
        let region = infer_region(&remark_str, &server_str);
        Self {
            id: id.into(),
            protocol,
            remark: remark_str,
            server: server_str,
            port,
            group: group.into(),
            region,
            uuid: None,
            password: None,
            method: None,
            flow: None,
            alter_id: None,
            security: None,
            congestion_control: None,
            up_mbps: None,
            down_mbps: None,
            obfs_type: None,
            obfs_password: None,
            wg_private_key: None,
            wg_peer_public_key: None,
            wg_preshared_key: None,
            wg_local_address: Vec::new(),
            wg_mtu: None,
            wg_reserved: None,
            tls: None,
            transport: None,
            detour: None,
            tcping_ms: None,
            real_delay_ms: None,
            raw_uri: None,
            created_at: chrono::Utc::now().to_rfc3339(),
        }
    }
}

pub fn infer_region(remark: &str, server: &str) -> String {
    let combined = format!("{} {}", remark.to_ascii_uppercase(), server.to_ascii_uppercase());
    // Match both ASCII codes and UTF-8 Chinese/Unicode region hints via ASCII escapes.
    if combined.contains("US")
        || combined.contains("USA")
        || combined.contains("UNITED STATES")
        || combined.contains("LOS ANGELES")
        || combined.contains("SAN JOSE")
        || combined.contains("SEATTLE")
        || combined.contains("NEW YORK")
        || remark.contains("\u{7f8e}\u{56fd}")
        || remark.contains("\u{6d1b}\u{6749}\u{77f6}")
        || remark.contains("\u{5723}\u{4f55}\u{585e}")
        || remark.contains("\u{897f}\u{96c5}\u{56fe}")
        || remark.contains("\u{7ebd}\u{7ea6}")
    {
        return "US".to_string();
    }
    if combined.contains("JP")
        || combined.contains("JAPAN")
        || combined.contains("TOKYO")
        || combined.contains("OSAKA")
        || remark.contains("\u{65e5}\u{672c}")
        || remark.contains("\u{4e1c}\u{4eac}")
        || remark.contains("\u{5927}\u{962a}")
    {
        return "JP".to_string();
    }
    if combined.contains("HK")
        || combined.contains("HONG KONG")
        || combined.contains("HONGKONG")
        || remark.contains("\u{9999}\u{6e2f}")
    {
        return "HK".to_string();
    }
    if combined.contains("SG")
        || combined.contains("SINGAPORE")
        || remark.contains("\u{65b0}\u{52a0}\u{5761}")
        || remark.contains("\u{72ee}\u{57ce}")
    {
        return "SG".to_string();
    }
    if combined.contains("TW")
        || combined.contains("TAIWAN")
        || combined.contains("TAIPEI")
        || remark.contains("\u{53f0}\u{6e7e}")
        || remark.contains("\u{53f0}\u{5317}")
    {
        return "TW".to_string();
    }
    if combined.contains("DE")
        || combined.contains("GERMANY")
        || combined.contains("FRANKFURT")
        || remark.contains("\u{5fb7}\u{56fd}")
        || remark.contains("\u{6cd5}\u{5170}\u{514b}\u{798f}")
    {
        return "DE".to_string();
    }
    if combined.contains("UK")
        || combined.contains("LONDON")
        || combined.contains("BRITAIN")
        || remark.contains("\u{82f1}\u{56fd}")
        || remark.contains("\u{4f26}\u{6566}")
    {
        return "UK".to_string();
    }
    "OTHER".to_string()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubscriptionGroup {
    pub id: String,
    pub name: String,
    pub url: String,
    pub auto_update_hours: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_updated: Option<String>,
    pub node_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InboundPort {
    pub id: String,
    pub port: u16,
    pub note: String,
    pub note_en: String,
    pub target: String,
    pub enabled: bool,
    #[serde(default)]
    pub allow_lan: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SimpleRoutingRule {
    pub id: String,
    pub title: String,
    pub title_en: String,
    pub description: String,
    pub patterns: Vec<String>,
    #[serde(default)]
    pub process_names: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rule_set: Option<String>,
    pub target: String,
    pub enabled: bool,
    pub priority: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RouteMode {
    Rule,
    Global,
    Direct,
}

impl RouteMode {
    pub fn as_str(self) -> &'static str {
        match self {
            RouteMode::Rule => "rule",
            RouteMode::Global => "global",
            RouteMode::Direct => "direct",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub locale: String,
    pub theme: String,
    pub route_mode: RouteMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub main_node_id: Option<String>,
    pub mixed_port: u16,
    pub control_port: u16,
    pub allow_lan: bool,
    pub system_proxy_enabled: bool,
    pub tun_enabled: bool,
    pub auto_start_core: bool,
    pub latency_test_url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_json_override: Option<String>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            locale: "zh-CN".to_string(),
            theme: "system".to_string(),
            route_mode: RouteMode::Rule,
            main_node_id: None,
            mixed_port: 10808,
            control_port: 9090,
            allow_lan: false,
            system_proxy_enabled: false,
            tun_enabled: false,
            auto_start_core: true,
            latency_test_url: "https://www.gstatic.com/generate_204".to_string(),
            custom_json_override: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkSnapshot {
    pub platform: String,
    pub proxy_enable: bool,
    pub proxy_server: String,
    pub proxy_override: String,
    pub dns_servers: Vec<String>,
    pub captured_at: String,
    pub active_sentinel: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LatencyResult {
    pub node_id: String,
    pub kind: String,
    pub delay_ms: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogEntry {
    pub timestamp: String,
    pub level: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionItem {
    pub id: String,
    pub process: String,
    pub destination: String,
    pub inbound: String,
    pub rule: String,
    pub chain: Vec<String>,
    pub upload_bytes: u64,
    pub download_bytes: u64,
    pub start: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrafficSample {
    pub up_bytes_per_sec: u64,
    pub down_bytes_per_sec: u64,
    pub memory_bytes: u64,
    pub active_connections: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoreInfo {
    pub installed: bool,
    pub running: bool,
    pub version: Option<String>,
    pub binary_path: String,
    pub sha256: Option<String>,
    pub portable_mode: bool,
    pub data_dir: String,
    pub netguard_armed: bool,
    pub netguard_recovered_from_crash: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreflightReport {
    pub ok: bool,
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub deprecated_warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompileOutput {
    pub config: serde_json::Value,
    pub total_nodes: usize,
    pub reachable_nodes: usize,
    pub reachable_node_ids: Vec<String>,
    pub synthesized_groups: Vec<String>,
    pub control_secret: String,
}
