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

fn extract_flag_region_code(text: &str) -> Option<String> {
    let mut prev_ri: Option<u32> = None;
    for ch in text.chars() {
        let cp = ch as u32;
        if (0x1F1E6..=0x1F1FF).contains(&cp) {
            if let Some(first) = prev_ri {
                let c1 = (b'A' + (first - 0x1F1E6) as u8) as char;
                let c2 = (b'A' + (cp - 0x1F1E6) as u8) as char;
                let code = format!("{}{}", c1, c2);
                return Some(if code == "GB" { "UK".to_string() } else { code });
            } else {
                prev_ri = Some(cp);
            }
        } else {
            prev_ri = None;
        }
    }
    None
}

pub fn infer_region(remark: &str, server: &str) -> String {
    // 1. Any Unicode Regional Indicator flag pair (e.g. US, JP, HK, SG, AU, GB->UK, CH, SE, ...).
    if let Some(code) = extract_flag_region_code(remark) {
        return code;
    }

    let upper = format!("{} {}", remark.to_ascii_uppercase(), server.to_ascii_uppercase());

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
        || upper.contains("DALLAS")
        || upper.contains("CHICAGO")
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
    if has_code_token(&upper, "RU")
        || upper.contains("RUSSIA")
        || upper.contains("MOSCOW")
        || remark.contains("\u{4fc4}\u{7f57}\u{65af}")
    {
        return "RU".to_string();
    }
    if has_code_token(&upper, "TR")
        || upper.contains("TURKEY")
        || upper.contains("ISTANBUL")
        || remark.contains("\u{571f}\u{8033}\u{5176}")
    {
        return "TR".to_string();
    }
    if has_code_token(&upper, "CH")
        || upper.contains("SWITZERLAND")
        || upper.contains("ZURICH")
        || remark.contains("\u{745e}\u{58eb}")
    {
        return "CH".to_string();
    }
    if has_code_token(&upper, "SE")
        || upper.contains("SWEDEN")
        || upper.contains("STOCKHOLM")
        || remark.contains("\u{745e}\u{5178}")
    {
        return "SE".to_string();
    }
    if has_code_token(&upper, "IT")
        || upper.contains("ITALY")
        || upper.contains("MILAN")
        || remark.contains("\u{610f}\u{5927}\u{5229}")
    {
        return "IT".to_string();
    }
    if has_code_token(&upper, "ES")
        || upper.contains("SPAIN")
        || upper.contains("MADRID")
        || remark.contains("\u{897f}\u{73ed}\u{7259}")
    {
        return "ES".to_string();
    }
    if has_code_token(&upper, "PL")
        || upper.contains("POLAND")
        || upper.contains("WARSAW")
        || remark.contains("\u{6ce2}\u{5170}")
    {
        return "PL".to_string();
    }
    if has_code_token(&upper, "MY")
        || upper.contains("MALAYSIA")
        || remark.contains("\u{9a6c}\u{6765}\u{897f}\u{4e9a}")
    {
        return "MY".to_string();
    }
    if has_code_token(&upper, "TH")
        || upper.contains("THAILAND")
        || remark.contains("\u{6cf0}\u{56fd}")
    {
        return "TH".to_string();
    }
    if has_code_token(&upper, "VN")
        || upper.contains("VIETNAM")
        || remark.contains("\u{8d8a}\u{5357}")
    {
        return "VN".to_string();
    }
    if has_code_token(&upper, "PH")
        || upper.contains("PHILIPPINES")
        || remark.contains("\u{83f2}\u{5f8b}\u{5bbe}")
    {
        return "PH".to_string();
    }
    if has_code_token(&upper, "BR")
        || upper.contains("BRAZIL")
        || remark.contains("\u{5df4}\u{897f}")
    {
        return "BR".to_string();
    }
    if has_code_token(&upper, "AE")
        || upper.contains("DUBAI")
        || remark.contains("\u{963f}\u{8054}\u{914b}")
        || remark.contains("\u{8fea}\u{62dc}")
    {
        return "AE".to_string();
    }
    if has_code_token(&upper, "AR")
        || upper.contains("ARGENTINA")
        || remark.contains("\u{963f}\u{6839}\u{5ef7}")
    {
        return "AR".to_string();
    }
    if has_code_token(&upper, "NZ")
        || upper.contains("NEW ZEALAND")
        || remark.contains("\u{65b0}\u{897f}\u{5170}")
    {
        return "NZ".to_string();
    }
    if has_code_token(&upper, "MO")
        || upper.contains("MACAU")
        || remark.contains("\u{6fb3}\u{95e8}")
    {
        return "MO".to_string();
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
    pub domain_keywords: Vec<String>,
    #[serde(default)]
    pub ip_cidrs: Vec<String>,
    #[serde(default)]
    pub process_names: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rule_set: Option<String>,
    pub target: String,
    pub enabled: bool,
    pub priority: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DnsRuleItem {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub domains: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rule_set: Option<String>,
    pub server: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DnsHostEntry {
    pub domain: String,
    pub ips: Vec<String>,
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

fn default_dns_bootstrap() -> String {
    "223.5.5.5".to_string()
}

fn default_dns_remote_detour() -> String {
    "main".to_string()
}

fn default_dns_strategy() -> String {
    "prefer_ipv4".to_string()
}

fn default_fake_ip_inet4() -> String {
    "198.18.0.0/15".to_string()
}

fn default_fake_ip_inet6() -> String {
    "fc00::/18".to_string()
}

fn default_dns_cache_capacity() -> u32 {
    4096
}

fn default_log_level() -> String {
    "info".to_string()
}

fn default_sniff_timeout() -> String {
    "300ms".to_string()
}

fn default_tun_iface() -> String {
    "portline-tun0".to_string()
}

fn default_tun_stack() -> String {
    "mixed".to_string()
}

fn default_tun_mtu() -> u16 {
    9000
}

fn default_tun_inet4() -> String {
    "172.19.0.1/30".to_string()
}

fn default_urltest_interval() -> String {
    "3m".to_string()
}

fn default_urltest_tolerance() -> u16 {
    50
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
    #[serde(default)]
    pub launch_at_login: bool,
    #[serde(default = "default_true")]
    pub minimize_to_tray: bool,
    #[serde(default = "default_dns_local")]
    pub dns_local_server: String,
    #[serde(default = "default_dns_remote")]
    pub dns_remote_server: String,
    #[serde(default = "default_dns_bootstrap")]
    pub dns_bootstrap_server: String,
    #[serde(default = "default_dns_remote_detour")]
    pub dns_remote_detour: String,
    #[serde(default = "default_dns_strategy")]
    pub dns_strategy: String,
    #[serde(default)]
    pub dns_fake_ip: bool,
    #[serde(default = "default_fake_ip_inet4")]
    pub dns_fake_ip_inet4: String,
    #[serde(default = "default_fake_ip_inet6")]
    pub dns_fake_ip_inet6: String,
    #[serde(default)]
    pub dns_disable_cache: bool,
    #[serde(default)]
    pub dns_disable_expire: bool,
    #[serde(default)]
    pub dns_reverse_mapping: bool,
    #[serde(default = "default_dns_cache_capacity")]
    pub dns_cache_capacity: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dns_client_subnet: Option<String>,
    #[serde(default)]
    pub dns_custom_rules: Vec<DnsRuleItem>,
    #[serde(default)]
    pub dns_hosts: Vec<DnsHostEntry>,
    #[serde(default = "default_log_level")]
    pub log_level: String,
    #[serde(default = "default_true")]
    pub sniff_enabled: bool,
    #[serde(default = "default_sniff_timeout")]
    pub sniff_timeout: String,
    #[serde(default = "default_true")]
    pub find_process: bool,
    #[serde(default = "default_true")]
    pub auto_detect_interface: bool,
    #[serde(default = "default_tun_iface")]
    pub tun_interface_name: String,
    #[serde(default = "default_tun_stack")]
    pub tun_stack: String,
    #[serde(default = "default_tun_mtu")]
    pub tun_mtu: u16,
    #[serde(default = "default_tun_inet4")]
    pub tun_inet4_address: String,
    #[serde(default = "default_true")]
    pub tun_auto_route: bool,
    #[serde(default = "default_true")]
    pub tun_strict_route: bool,
    #[serde(default = "default_urltest_interval")]
    pub urltest_interval: String,
    #[serde(default = "default_urltest_tolerance")]
    pub urltest_tolerance: u16,
    #[serde(default = "default_true")]
    pub cache_file_enabled: bool,
    #[serde(default = "default_true")]
    pub cache_store_fakeip: bool,
    #[serde(default = "default_true")]
    pub latency_strip_handshake: bool,
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
            launch_at_login: false,
            minimize_to_tray: true,
            dns_local_server: default_dns_local(),
            dns_remote_server: default_dns_remote(),
            dns_bootstrap_server: default_dns_bootstrap(),
            dns_remote_detour: default_dns_remote_detour(),
            dns_strategy: default_dns_strategy(),
            dns_fake_ip: false,
            dns_fake_ip_inet4: default_fake_ip_inet4(),
            dns_fake_ip_inet6: default_fake_ip_inet6(),
            dns_disable_cache: false,
            dns_disable_expire: false,
            dns_reverse_mapping: false,
            dns_cache_capacity: default_dns_cache_capacity(),
            dns_client_subnet: None,
            dns_custom_rules: Vec::new(),
            dns_hosts: Vec::new(),
            log_level: default_log_level(),
            sniff_enabled: true,
            sniff_timeout: default_sniff_timeout(),
            find_process: true,
            auto_detect_interface: true,
            tun_interface_name: default_tun_iface(),
            tun_stack: default_tun_stack(),
            tun_mtu: default_tun_mtu(),
            tun_inet4_address: default_tun_inet4(),
            tun_auto_route: true,
            tun_strict_route: true,
            urltest_interval: default_urltest_interval(),
            urltest_tolerance: default_urltest_tolerance(),
            cache_file_enabled: true,
            cache_store_fakeip: true,
            latency_strip_handshake: true,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LatencyProgressEvent {
    pub kind: String,
    pub phase: String,
    pub completed: usize,
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    #[serde(default)]
    pub strip_handshake: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest: Option<LatencyResult>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionHistoryRecord {
    pub id: String,
    pub process: String,
    pub destination: String,
    pub host: String,
    pub platform: String,
    pub inbound: String,
    pub rule: String,
    pub chain: Vec<String>,
    pub upload_bytes: u64,
    pub download_bytes: u64,
    pub started_at: String,
    pub closed_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlatformTrafficStat {
    pub platform: String,
    pub upload_bytes: u64,
    pub download_bytes: u64,
    pub total_bytes: u64,
    pub connection_count: u64,
    pub active_count: u64,
    pub top_domains: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RouteTestResult {
    pub input: String,
    pub normalized_host: String,
    pub is_ip: bool,
    pub is_private_ip: bool,
    pub route_mode: RouteMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub matched_rule_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub matched_rule_title: Option<String>,
    pub matched_by: String,
    pub target: String,
    pub resolved_exit_kind: String,
    pub resolved_exit_label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved_node_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved_node_region: Option<String>,
    pub dns_server: String,
    pub dns_reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RulePresetSpec {
    pub id: String,
    pub category: String,
    pub title: String,
    pub title_en: String,
    pub description: String,
    pub patterns: Vec<String>,
    #[serde(default)]
    pub process_names: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rule_set: Option<String>,
    pub default_target: String,
    pub default_priority: i32,
}
