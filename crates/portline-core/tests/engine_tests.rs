use anyhow::Result;
use portline_core::compiler::{CompileInput, SingBox114Compiler};
use portline_core::control::ControlClient;
use portline_core::model::{
    AppSettings, InboundPort, NetworkSnapshot, NodeIR, Protocol, RouteMode, SimpleRoutingRule,
    TlsConfig, TransportConfig,
};
use portline_core::netguard::{NetGuard, SystemNetworkAdapter};
use portline_core::parser::parse_payload;
use portline_core::rule_cache::RuleCache;
use portline_core::storage::{default_simple_rules, DataPaths, Store};
use portline_core::supervisor::{CoreSupervisor, LogRing};
use portline_core::tester::{batch_real_delay, batch_tcping};
use std::collections::BTreeMap;
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

fn find_test_singbox() -> Option<PathBuf> {
    if let Ok(env_path) = std::env::var("PORTLINE_TEST_SINGBOX") {
        let p = PathBuf::from(env_path);
        if p.exists() {
            return Some(p);
        }
    }
    // Also check PATH if sing-box is installed locally.
    let bin = if cfg!(windows) { "sing-box.exe" } else { "sing-box" };
    if let Ok(path_var) = std::env::var("PATH") {
        for dir in std::env::split_paths(&path_var) {
            let cand = dir.join(bin);
            if cand.exists() {
                return Some(cand);
            }
        }
    }
    None
}

fn build_all_8_protocol_nodes() -> Vec<NodeIR> {
    let mut vless = NodeIR::new("n-vless", Protocol::Vless, "JP-Tokyo-01 Reality", "jp1.example.com", 443, "VPS");
    vless.uuid = Some("11111111-1111-1111-1111-111111111111".to_string());
    vless.flow = Some("xtls-rprx-vision".to_string());
    vless.tls = Some(TlsConfig {
        enabled: true,
        server_name: Some("www.microsoft.com".to_string()),
        insecure: false,
        alpn: Vec::new(),
        fingerprint: Some("chrome".to_string()),
        reality_public_key: Some("AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA".to_string()),
        reality_short_id: Some("abcd".to_string()),
    });

    let mut vmess = NodeIR::new("n-vmess", Protocol::Vmess, "TW-Taipei-01", "tw1.example.com", 8080, "Sub");
    vmess.uuid = Some("22222222-2222-2222-2222-222222222222".to_string());
    vmess.security = Some("auto".to_string());
    vmess.alter_id = Some(0);
    vmess.transport = Some(TransportConfig {
        transport_type: "ws".to_string(),
        path: Some("/ws".to_string()),
        host: Some("tw1.example.com".to_string()),
        service_name: None,
        headers: BTreeMap::from([("Host".to_string(), "tw1.example.com".to_string())]),
    });

    let mut trojan = NodeIR::new("n-trojan", Protocol::Trojan, "UK-London-01", "uk1.example.com", 443, "VPS");
    trojan.password = Some("secret-trojan".to_string());
    trojan.tls = Some(TlsConfig {
        enabled: true,
        server_name: Some("uk1.example.com".to_string()),
        ..Default::default()
    });

    let mut ss = NodeIR::new("n-ss", Protocol::Shadowsocks, "HK-IPLC-01", "hk1.example.com", 10443, "Sub");
    ss.method = Some("2022-blake3-aes-128-gcm".to_string());
    ss.password = Some("AAAAAAAAAAAAAAAAAAAAAA==".to_string());

    let mut hy2 = NodeIR::new("n-hy2", Protocol::Hysteria2, "US-LA-Residential-01", "us1.example.com", 8443, "US");
    hy2.password = Some("secret-hy2".to_string());
    hy2.tls = Some(TlsConfig {
        enabled: true,
        server_name: Some("us1.example.com".to_string()),
        ..Default::default()
    });

    let mut tuic = NodeIR::new("n-tuic", Protocol::Tuic, "SG-Unlock-01", "sg1.example.com", 20443, "Sub");
    tuic.uuid = Some("33333333-3333-3333-3333-333333333333".to_string());
    tuic.password = Some("secret-tuic".to_string());
    tuic.congestion_control = Some("bbr".to_string());
    tuic.tls = Some(TlsConfig {
        enabled: true,
        server_name: Some("sg1.example.com".to_string()),
        alpn: vec!["h3".to_string()],
        ..Default::default()
    });

    let mut anytls = NodeIR::new("n-anytls", Protocol::Anytls, "US-Seattle-02", "us2.example.com", 443, "US");
    anytls.password = Some("secret-anytls".to_string());
    anytls.tls = Some(TlsConfig {
        enabled: true,
        server_name: Some("us2.example.com".to_string()),
        ..Default::default()
    });

    let mut wg = NodeIR::new("n-wg", Protocol::Wireguard, "DE-Frankfurt-WG", "1.2.3.4", 51820, "VPS");
    wg.wg_private_key = Some("AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=".to_string());
    wg.wg_peer_public_key = Some("BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB=".to_string());
    wg.wg_local_address = vec!["10.0.0.2/32".to_string()];

    let mut http_n = NodeIR::new("n-http", Protocol::Http, "AU-Sydney-HTTP", "au1.example.com", 8080, "Proxy");
    http_n.username = Some("user".to_string());
    http_n.password = Some("pass".to_string());

    let mut socks_n = NodeIR::new("n-socks", Protocol::Socks, "AU-Melbourne-SOCKS5", "au2.example.com", 1080, "Proxy");
    socks_n.socks_version = Some("5".to_string());
    socks_n.username = Some("suser".to_string());
    socks_n.password = Some("spass".to_string());

    vec![vless, vmess, trojan, ss, hy2, tuic, anytls, wg, http_n, socks_n]
}

#[test]
fn test_tree_shaking_2000_nodes_and_dynamic_matcher() {
    let mut nodes = Vec::with_capacity(2000);
    for i in 0..2000 {
        let remark = if i < 3 {
            format!("US-Residential-{:02}", i + 1)
        } else {
            format!("JP-Bulk-{:04}", i)
        };
        let mut n = NodeIR::new(
            format!("n-bulk-{:04}", i),
            Protocol::Vless,
            remark,
            format!("node{}.example.com", i),
            443,
            "MassivePool",
        );
        n.uuid = Some("11111111-1111-1111-1111-111111111111".to_string());
        nodes.push(n);
    }

    let settings = AppSettings {
        main_node_id: Some("n-bulk-0500".to_string()),
        ..Default::default()
    };
    let rules = vec![
        SimpleRoutingRule {
            id: "r-ai".to_string(),
            title: "AI".to_string(),
            title_en: "AI".to_string(),
            description: "OpenAI".to_string(),
            patterns: vec!["openai.com".to_string()],
            process_names: Vec::new(),
            rule_set: None,
            target: "auto:US".to_string(),
            enabled: true,
            priority: 1,
        },
        SimpleRoutingRule {
            id: "r-pinned".to_string(),
            title: "Pinned".to_string(),
            title_en: "Pinned".to_string(),
            description: "GitHub".to_string(),
            patterns: vec!["github.com".to_string()],
            process_names: Vec::new(),
            rule_set: None,
            target: "n-bulk-0999".to_string(),
            enabled: true,
            priority: 2,
        },
    ];

    let compiled = SingBox114Compiler::compile(&CompileInput {
        nodes: &nodes,
        rules: &rules,
        extra_ports: &[],
        settings: &settings,
        control_secret: "secret",
        cache_db_path: None,
        rule_cache: None,
    })
    .expect("compile");

    // Only main (1) + 3 US nodes + 1 pinned = 5 nodes out of 2,000!
    assert_eq!(compiled.total_nodes, 2000);
    assert_eq!(compiled.reachable_nodes, 5);

    // Simulate subscription refresh renaming the US nodes; dynamic regex/region matcher stays valid.
    nodes[0].remark = "US-NewName-LA".to_string();
    nodes[1].remark = "US-NewName-SJ".to_string();
    let recompiled = SingBox114Compiler::compile(&CompileInput {
        nodes: &nodes,
        rules: &[SimpleRoutingRule {
            target: "auto:regex:NewName".to_string(),
            ..rules[0].clone()
        }],
        extra_ports: &[],
        settings: &settings,
        control_secret: "secret",
        cache_db_path: None,
        rule_cache: None,
    })
    .expect("recompile");
    // 1 main + 2 matching regex = 3 reachable nodes, zero dangling references.
    assert_eq!(recompiled.reachable_nodes, 3);
}

#[test]
fn test_multi_format_parsers_uri_base64_json_yaml() {
    use base64::engine::general_purpose::STANDARD;
    use base64::Engine;

    let raw_uris = [
        "vless://11111111-1111-1111-1111-111111111111@jp1.example.com:443?security=reality&pbk=PUBKEY&sid=abcd&sni=www.microsoft.com&fp=chrome#JP-Reality",
        "trojan://pass123@uk1.example.com:443?sni=uk1.example.com#UK-Trojan",
        "hysteria2://hy2pass@us1.example.com:8443?sni=us1.example.com#US-Hy2",
        "tuic://22222222-2222-2222-2222-222222222222:tuicpass@sg1.example.com:20443?congestion_control=bbr#SG-TUIC",
        "anytls://anypass@us2.example.com:443?sni=us2.example.com#US-AnyTLS",
        "wireguard://PRIVKEY@1.2.3.4:51820?publickey=PUBKEY&address=10.0.0.2/32#DE-WG",
        "http://alice:secret@au1.example.com:8080#\u{1f1e6}\u{1f1fa}%20oracle-arm-http",
        "https://bob:pw@au2.example.com:443#AU-HTTPS-Proxy",
        "socks5://suser:spass@au3.example.com:1080#AU-SOCKS5",
        "socks://c3VzZXI6c3Bhc3M=@au4.example.com:1080#AU-V2rayN-SOCKS",
    ]
    .join("\n");

    // 1. Direct URI batch including HTTP, HTTPS, SOCKS5, and Base64 SOCKS
    let direct = parse_payload(&raw_uris, "Imported").expect("parse direct");
    assert_eq!(direct.nodes.len(), 10);
    assert_eq!(direct.nodes[0].region, "JP");
    assert_eq!(direct.nodes[2].region, "US");
    assert_eq!(direct.nodes[6].protocol, Protocol::Http);
    assert_eq!(direct.nodes[6].region, "AU");
    assert_eq!(direct.nodes[6].username.as_deref(), Some("alice"));
    assert_eq!(direct.nodes[7].tls.as_ref().map(|t| t.enabled), Some(true));
    assert_eq!(direct.nodes[8].protocol, Protocol::Socks);
    assert_eq!(direct.nodes[9].username.as_deref(), Some("suser"));
    assert_eq!(direct.nodes[9].password.as_deref(), Some("spass"));

    // 2. Base64 subscription payload
    let b64 = STANDARD.encode(raw_uris.as_bytes());
    let from_b64 = parse_payload(&b64, "Sub").expect("parse base64");
    assert_eq!(from_b64.nodes.len(), 10);

    // 3. Clash YAML payload
    let yaml = r#"
proxies:
  - name: "US-Clash-Trojan"
    type: trojan
    server: us-clash.example.com
    port: 443
    password: secret
    sni: us-clash.example.com
  - name: "HK-Clash-SS"
    type: ss
    server: hk-clash.example.com
    port: 8388
    cipher: aes-256-gcm
    password: sspw
"#;
    let from_yaml = parse_payload(yaml, "ClashSub").expect("parse yaml");
    assert_eq!(from_yaml.nodes.len(), 2);
    assert_eq!(from_yaml.nodes[0].region, "US");
    assert_eq!(from_yaml.nodes[1].region, "HK");
}

#[test]
fn test_portable_mode_and_sqlite_store() {
    let tmp = tempfile::tempdir().expect("tempdir");
    std::fs::write(tmp.path().join("portable.json"), r#"{"portable":true}"#).expect("write");
    let paths = DataPaths::resolve_from_exe_dir(tmp.path()).expect("resolve");
    assert!(paths.portable_mode);
    assert!(paths.root_dir.ends_with("portline-data"));

    let mut store = Store::open(&paths.db_path).expect("open store");
    assert_eq!(store.list_rules().expect("rules").len(), 5);

    let nodes = build_all_8_protocol_nodes();
    store.upsert_nodes(&nodes).expect("upsert");
    assert_eq!(store.list_nodes().expect("list").len(), 10);
}

#[derive(Default)]
struct MockOsState {
    proxy_enable: bool,
    proxy_server: String,
    proxy_override: String,
    dns_servers: Vec<String>,
}

struct MockNetAdapter {
    state: Arc<Mutex<MockOsState>>,
}

impl SystemNetworkAdapter for MockNetAdapter {
    fn capture_snapshot(&self) -> Result<NetworkSnapshot> {
        let g = self.state.lock().unwrap();
        Ok(NetworkSnapshot {
            platform: "mock".to_string(),
            proxy_enable: g.proxy_enable,
            proxy_server: g.proxy_server.clone(),
            proxy_override: g.proxy_override.clone(),
            dns_servers: g.dns_servers.clone(),
            captured_at: "2026-11-25T00:00:00Z".to_string(),
            active_sentinel: false,
        })
    }

    fn apply_system_proxy(&self, host: &str, port: u16, bypass: &str) -> Result<()> {
        let mut g = self.state.lock().unwrap();
        g.proxy_enable = true;
        g.proxy_server = format!("{}:{}", host, port);
        g.proxy_override = bypass.to_string();
        Ok(())
    }

    fn restore_snapshot(&self, snapshot: &NetworkSnapshot) -> Result<()> {
        let mut g = self.state.lock().unwrap();
        g.proxy_enable = snapshot.proxy_enable;
        g.proxy_server = snapshot.proxy_server.clone();
        g.proxy_override = snapshot.proxy_override.clone();
        g.dns_servers = snapshot.dns_servers.clone();
        Ok(())
    }
}

#[test]
fn test_netguard_crash_recovery_snapshot_rollback() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let os = Arc::new(Mutex::new(MockOsState {
        proxy_enable: false,
        proxy_server: String::new(),
        proxy_override: "corp.local".to_string(),
        dns_servers: vec!["8.8.8.8".to_string()],
    }));

    // Session 1 enables proxy and then "crashes" without calling disable_system_proxy.
    {
        let guard = NetGuard::with_adapter(tmp.path(), MockNetAdapter { state: Arc::clone(&os) });
        guard.enable_system_proxy("127.0.0.1", 10808).expect("enable");
        assert!(guard.is_armed());
        assert_eq!(os.lock().unwrap().proxy_server, "127.0.0.1:10808");
    }

    // Session 2 cold-starts and detects dirty crash sentinel -> restores original settings!
    let guard2 = NetGuard::with_adapter(tmp.path(), MockNetAdapter { state: Arc::clone(&os) });
    let recovered = guard2.recover_if_dirty().expect("recover");
    assert!(recovered);
    assert!(!guard2.is_armed());
    assert!(!os.lock().unwrap().proxy_enable);
    assert_eq!(os.lock().unwrap().proxy_override, "corp.local");
    assert_eq!(os.lock().unwrap().dns_servers, vec!["8.8.8.8".to_string()]);
}

#[tokio::test]
async fn test_srs_offline_graceful_degradation() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let cache = RuleCache::new(tmp.path().to_path_buf());
    // Unreachable local port simulates total network outage.
    let status = cache
        .refresh_rule_set("geosite-openai", "http://127.0.0.1:1/nonexistent.srs", None)
        .await;
    assert!(status.fallback_mode);
    assert!(!status.cached_locally);
}

#[tokio::test]
async fn test_tcping_against_local_listener() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().unwrap().port();
    let node = NodeIR::new("n-local", Protocol::Vless, "Local", "127.0.0.1", port, "Test");
    let results = batch_tcping(&[node], 1500, 4).await;
    assert_eq!(results.len(), 1);
    assert!(results[0].delay_ms.is_some());
}

#[test]
fn test_singbox_v114_all_8_protocols_strict_check() {
    let Some(sb) = find_test_singbox() else {
        eprintln!("Skipping real sing-box check test (set PORTLINE_TEST_SINGBOX to enable)");
        return;
    };

    let nodes = build_all_8_protocol_nodes();
    let mut rules = default_simple_rules();
    // Force all 8 nodes to be reachable via auto:ALL so every protocol is checked!
    rules[0].target = "auto:ALL".to_string();

    let extra_ports = vec![InboundPort {
        id: "in-10809".to_string(),
        port: 10809,
        note: "AI CLI".to_string(),
        note_en: "AI CLI".to_string(),
        target: "auto:US".to_string(),
        enabled: true,
        allow_lan: false,
    }];

    for (mode, fake_ip, local_dns) in [
        (RouteMode::Rule, false, "223.5.5.5"),
        (RouteMode::Rule, true, "https://dns.alidns.com/dns-query"),
        (RouteMode::Global, false, "119.29.29.29"),
        (RouteMode::Direct, false, "223.5.5.5"),
    ] {
        let settings = AppSettings {
            route_mode: mode,
            main_node_id: Some("n-vless".to_string()),
            dns_local_server: local_dns.to_string(),
            dns_remote_server: "https://1.1.1.1/dns-query".to_string(),
            dns_strategy: "prefer_ipv4".to_string(),
            dns_fake_ip: fake_ip,
            ..Default::default()
        };
        let compiled = SingBox114Compiler::compile(&CompileInput {
            nodes: &nodes,
            rules: &rules,
            extra_ports: &extra_ports,
            settings: &settings,
            control_secret: "strict-check",
            cache_db_path: None,
            rule_cache: None,
        })
        .expect("compile");

        let report = SingBox114Compiler::preflight_check(&sb, &compiled.config).expect("preflight");
        assert!(
            report.ok,
            "sing-box v1.14 check failed for mode {:?}: stderr={} stdout={}",
            mode, report.stderr, report.stdout
        );
        assert!(
            report.deprecated_warnings.is_empty(),
            "Found deprecation warnings in mode {:?}: {:?}",
            mode,
            report.deprecated_warnings
        );
    }
}

#[tokio::test]
async fn test_real_singbox_multiport_hotswitch_and_bad_config_rollback() {
    let Some(sb) = find_test_singbox() else {
        eprintln!("Skipping live sing-box multiport/hotswitch test (set PORTLINE_TEST_SINGBOX)");
        return;
    };

    let p1 = TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
    let p2 = TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
    let ctrl_port = TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();

    let nodes = build_all_8_protocol_nodes();
    let mut rules = default_simple_rules();
    rules[0].target = "auto:US".to_string();

    let settings = AppSettings {
        route_mode: RouteMode::Rule,
        main_node_id: Some("n-vless".to_string()),
        mixed_port: p1,
        control_port: ctrl_port,
        ..Default::default()
    };
    let extra_ports = vec![InboundPort {
        id: format!("in-{}", p2),
        port: p2,
        note: "Dedicated CLI".to_string(),
        note_en: "Dedicated CLI".to_string(),
        target: "n-hy2".to_string(),
        enabled: true,
        allow_lan: false,
    }];

    let tmp = tempfile::tempdir().expect("tempdir");
    let cache_db = tmp.path().join("cache.db");
    let compiled = SingBox114Compiler::compile(&CompileInput {
        nodes: &nodes,
        rules: &rules,
        extra_ports: &extra_ports,
        settings: &settings,
        control_secret: "e2e-secret",
        cache_db_path: Some(&cache_db),
        rule_cache: None,
    })
    .expect("compile");

    let logs = Arc::new(Mutex::new(LogRing::new(500)));
    let mut supervisor = CoreSupervisor::new(tmp.path().to_path_buf(), logs);

    let report = supervisor
        .apply_config(&sb, compiled.config.clone())
        .expect("start core");
    assert!(report.ok);
    let initial_pid = supervisor.require_running_pid().expect("running pid");

    // Wait for Clash API to come online.
    let client = ControlClient::new(ctrl_port, "e2e-secret").expect("client");
    let mut online = false;
    for _ in 0..40 {
        if client.get_version().await.is_ok() {
            online = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert!(online, "sing-box Clash API did not come online");

    // 1. Verify BOTH local inbound ports (p1 and p2) are actively listening in the single sing-box process.
    assert!(tokio::net::TcpStream::connect(("127.0.0.1", p1)).await.is_ok());
    assert!(tokio::net::TcpStream::connect(("127.0.0.1", p2)).await.is_ok());

    // 2. Verify 0ms hot-switch on main-exit via Control API WITHOUT changing PID!
    assert_eq!(client.get_proxy_now("main-exit").await.unwrap(), "n-vless");
    client
        .select_proxy("main-exit", "n-hy2")
        .await
        .expect("hot switch");
    assert_eq!(client.get_proxy_now("main-exit").await.unwrap(), "n-hy2");
    assert_eq!(supervisor.require_running_pid().unwrap(), initial_pid);

    // 2b. Verify 0ms live route-mode switching (rule -> direct -> global -> rule) via clash_mode!
    assert_eq!(client.get_mode().await.unwrap(), "rule");
    client.set_mode("direct").await.expect("set direct mode");
    assert_eq!(client.get_mode().await.unwrap(), "direct");
    client.set_mode("global").await.expect("set global mode");
    assert_eq!(client.get_mode().await.unwrap(), "global");
    client.set_mode("rule").await.expect("set rule mode");
    assert_eq!(client.get_mode().await.unwrap(), "rule");
    assert_eq!(supervisor.require_running_pid().unwrap(), initial_pid);

    // 3. Inject an intentionally invalid config override -> preflight MUST reject it and keep initial_pid alive!
    let mut bad_cfg = compiled.config.clone();
    bad_cfg["outbounds"] = serde_json::json!([{ "type": "nonexistent_proto_xyz", "tag": "broken" }]);
    let bad_report = supervisor.apply_config(&sb, bad_cfg).expect("apply bad");
    assert!(!bad_report.ok);
    assert_eq!(
        supervisor.require_running_pid().unwrap(),
        initial_pid,
        "Bad config must NOT interrupt active sing-box process"
    );

    // 4. Verify isolated batch real-delay tester cleans up cleanly without touching active core.
    let test_nodes = vec![nodes[0].clone()];
    let _res = batch_real_delay(&sb, &test_nodes, "http://127.0.0.1:1/204", 600, 2)
        .await
        .expect("batch real delay");
    assert_eq!(supervisor.require_running_pid().unwrap(), initial_pid);

    supervisor.stop();
    assert!(!supervisor.is_running());
}

#[test]
fn test_singbox_log_level_and_inbound_label_parsing() {
    use portline_core::control::normalize_inbound_label;
    use portline_core::supervisor::parse_singbox_log_line;

    let (lvl, msg) = parse_singbox_log_line(
        "+0800 2026-11-26 00:46:45 INFO inbound/mixed[in-20808]: tcp server started",
    );
    assert_eq!(lvl, "INFO");
    assert_eq!(msg, "inbound/mixed[in-20808]: tcp server started");

    let (lvl2, msg2) = parse_singbox_log_line(
        "\x1b[31m+0800 2026-11-26 00:46:45 ERROR outbound/vless: dial tcp timeout\x1b[0m",
    );
    assert_eq!(lvl2, "ERROR");
    assert_eq!(msg2, "outbound/vless: dial tcp timeout");

    assert_eq!(normalize_inbound_label("mixed/in-20808"), ":20808");
    assert_eq!(normalize_inbound_label("in-10809"), ":10809");
    assert_eq!(normalize_inbound_label("tun/tun-in"), "TUN");
}
