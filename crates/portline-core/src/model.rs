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
    Http,
    Socks,
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
            Protocol::Http => "http",
            Protocol::Socks => "socks",
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
            Protocol::Http => "HTTP",
            Protocol::Socks => "SOCKS",
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
    pub username: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub socks_version: Option<String>,
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
            username: None,
            password: None,
            socks_version: None,
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

fn has_code_token(text: &str, code: &str) -> bool {
    text.split(|c: char| !c.is_ascii_alphanumeric())
        .any(|tok| tok.eq_ignore_ascii_case(code))
}

pub fn infer_region(remark: &str, server: &str) -> String {
    let upper = format!("{} {}", remark.to_ascii_uppercase(), server.to_ascii_uppercase());

    // 1. Regional indicator flag emojis (pure 7-bit ASCII Unicode escapes).
    let flags: &[(&str, &str)] = &[
        ("\u{1f1e6}\u{1f1fa}", "AU"),
        ("\u{1f1fa}\u{1f1f8}", "US"),
        ("\u{1f1ef}\u{1f1f5}", "JP"),
        ("\u{1f1ed}\u{1f1f0}", "HK"),
        ("\u{1f1f8}\u{1f1ec}", "SG"),
        ("\u{1f1f9}\u{1f1fc}", "TW"),
        ("\u{1f1e9}\u{1f1ea}", "DE"),
        ("\u{1f1ec}\u{1f1e7}", "UK"),
        ("\u{1f1f0}\u{1f1f7}", "KR"),
        ("\u{1f1e8}\u{1f1e6}", "CA"),
        ("\u{1f1f3}\u{1f1f1}", "NL"),
        ("\u{1f1eb}\u{1f1f7}", "FR"),
        ("\u{1f1ee}\u{1f1f3}", "IN"),
        ("\u{1f1f7}\u{1f1fa}", "RU"),
        ("\u{1f1f9}\u{1f1f7}", "TR"),
    ];
    for (flag, code) in flags {
        if remark.contains(flag) {
            return (*code).to_string();
        }
    }

    // 2. Explicit country/city tokens and localized names.
    if has_code_token(&upper, "AU")
        || upper.contains("AUSTRALIA")
        || upper.contains("SYDNEY")
        || upper.contains("MELBOURNE")
        || remark.contains("\u{6fb3}\u{5927}\u{5229}\u{4e9a}")
        || remark.contains("\u{6fb3}\u{6d32}")
        || remark.contains("\u{6089}\u{5c3c}")
        || remark.contains("\u{58a8}\u{5c14}\u{672c}")
    {
        return "AU".to_string();
    }
    if has_code_token(&upper, "US")
        || has_code_token(&upper, "USA")
        || upper.contains("UNITED STATES")
        || upper.contains("LOS ANGELES")
        || upper.contains("SAN JOSE")
        || upper.contains("SEATTLE")
        || upper.contains("NEW YORK")
        || upper.contains("SILICON VALLEY")
        || remark.contains("\u{7f8e}\u{56fd}")
        || remark.contains("\u{6d1b}\u{6749}\u{77f6}")
        || remark.contains("\u{5723}\u{4f55}\u{585e}")
        || remark.contains("\u{897f}\u{96c5}\u{56fe}")
        || remark.contains("\u{7ebd}\u{7ea6}")
    {
        return "US".to_string();
    }
    if has_code_token(&upper, "JP")
        || upper.contains("JAPAN")
        || upper.contains("TOKYO")
        || upper.contains("OSAKA")
        || remark.contains("\u{65e5}\u{672c}")
        || remark.contains("\u{4e1c}\u{4eac}")
        || remark.contains("\u{5927}\u{962a}")
    {
        return "JP".to_string();
    }
    if has_code_token(&upper, "HK")
        || upper.contains("HONG KONG")
        || upper.contains("HONGKONG")
        || remark.contains("\u{9999}\u{6e2f}")
    {
        return "HK".to_string();
    }
    if has_code_token(&upper, "SG")
        || upper.contains("SINGAPORE")
        || remark.contains("\u{65b0}\u{52a0}\u{5761}")
        || remark.contains("\u{72ee}\u{57ce}")
    {
        return "SG".to_string();
    }
    if has_code_token(&upper, "TW")
        || upper.contains("TAIWAN")
        || upper.contains("TAIPEI")
        || remark.contains("\u{53f0}\u{6e7e}")
        || remark.contains("\u{53f0}\u{5317}")
    {
        return "TW".to_string();
    }
    if has_code_token(&upper, "DE")
        || upper.contains("GERMANY")
        || upper.contains("FRANKFURT")
        || remark.contains("\u{5fb7}\u{56fd}")
        || remark.contains("\u{6cd5}\u{5170}\u{514b}\u{798f}")
    {
        return "DE".to_string();
    }
    if has_code_token(&upper, "UK")
        || has_code_token(&upper, "GB")
        || upper.contains("LONDON")
        || upper.contains("BRITAIN")
        || remark.contains("\u{82f1}\u{56fd}")
        || remark.contains("\u{4f26}\u{6566}")
    {
        return "UK".to_string();
    }
    if has_code_token(&upper, "KR")
        || upper.contains("KOREA")
        || upper.contains("SEOUL")
        || remark.contains("\u{97e9}\u{56fd}")
        || remark.contains("\u{9996}\u{5c14}")
    {
        return "KR".to_string();
    }
    if has_code_token(&upper, "CA")
        || upper.contains("CANADA")
        || upper.contains("TORONTO")
        || upper.contains("VANCOUVER")
        || remark.contains("\u{52a0}\u{62ff}\u{5927}")
    {
        return "CA".to_string();
    }
    if has_code_token(&upper, "NL")
        || upper.contains("NETHERLANDS")
        || upper.contains("AMSTERDAM")
        || remark.contains("\u{8377}\u{5170}")
    {
        return "NL".to_string();
    }
    if has_code_token(&upper, "FR")
        || upper.contains("FRANCE")
        || upper.contains("PARIS")
        || remark.contains("\u{6cd5}\u{56fd}")
    {
        return "FR".to_string();
    }
    if has_code_token(&upper, "IN")
        || upper.contains("INDIA")
        || upper.contains("MUMBAI")
        || remark.contains("\u{5370}\u{5ea6}")
    {
        return "IN".to_string();
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

fn default_true() -> bool {
    true
}

fn default_dns_local() -> String {
    "223.5.5.5".to_string()
}

fn default_dns_remote() -> String {
    "https://1.1.1.1/dns-query".to_string()
}

fn default_dns_strategy() -> String {
    "prefer_ipv4".to_string()
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
    #[serde(default = "default_true")]
    pub minimize_to_tray: bool,
    #[serde(default = "default_dns_local")]
    pub dns_local_server: String,
    #[serde(default = "default_dns_remote")]
    pub dns_remote_server: String,
    #[serde(default = "default_dns_strategy")]
    pub dns_strategy: String,
    #[serde(default)]
    pub dns_fake_ip: bool,
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
            minimize_to_tray: true,
            dns_local_server: default_dns_local(),
            dns_remote_server: default_dns_remote(),
            dns_strategy: default_dns_strategy(),
            dns_fake_ip: false,
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
