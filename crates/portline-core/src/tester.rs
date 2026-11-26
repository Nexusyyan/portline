use crate::compiler::{compile_dns_server, compile_proxy_outbound, compile_wireguard_endpoint};
use crate::control::ControlClient;
use crate::model::{LatencyProgressEvent, LatencyResult, NodeIR, Protocol};
use crate::supervisor::spawn_managed_process;
use anyhow::{anyhow, Result};
use futures::stream::{self, StreamExt};
use serde_json::{json, Value};
use std::net::TcpListener;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::net::TcpStream;

/// Concurrent TCPing latency tester (no sing-box needed, non-disruptive).
pub async fn batch_tcping(
    nodes: &[NodeIR],
    timeout_ms: u64,
    concurrency: usize,
) -> Vec<LatencyResult> {
    batch_tcping_with_progress(nodes, timeout_ms, concurrency, |_| {}).await
}

/// Concurrent TCPing latency tester that reports incremental progress after every node.
pub async fn batch_tcping_with_progress<F>(
    nodes: &[NodeIR],
    timeout_ms: u64,
    concurrency: usize,
    on_progress: F,
) -> Vec<LatencyResult>
where
    F: Fn(LatencyProgressEvent) + Send + Sync + 'static,
{
    let total = nodes.len();
    let cb = Arc::new(on_progress);
    if total == 0 {
        cb(LatencyProgressEvent {
            kind: "tcping".to_string(),
            phase: "done".to_string(),
            completed: 0,
            total: 0,
            passed: 0,
            failed: 0,
            strip_handshake: false,
            latest: None,
        });
        return Vec::new();
    }

    cb(LatencyProgressEvent {
        kind: "tcping".to_string(),
        phase: "testing".to_string(),
        completed: 0,
        total,
        passed: 0,
        failed: 0,
        strip_handshake: false,
        latest: None,
    });

    let completed = Arc::new(AtomicUsize::new(0));
    let passed = Arc::new(AtomicUsize::new(0));
    let failed = Arc::new(AtomicUsize::new(0));
    let limit = concurrency.clamp(1, 64);
    let timeout = Duration::from_millis(timeout_ms.max(300));

    let results: Vec<LatencyResult> = stream::iter(nodes.iter().cloned())
        .map(|node| {
            let cb = Arc::clone(&cb);
            let completed = Arc::clone(&completed);
            let passed = Arc::clone(&passed);
            let failed = Arc::clone(&failed);
            async move {
                let addr = format!("{}:{}", node.server, node.port);
                let started = Instant::now();
                let res = match tokio::time::timeout(timeout, TcpStream::connect(&addr)).await {
                    Ok(Ok(_stream)) => LatencyResult {
                        node_id: node.id,
                        kind: "tcping".to_string(),
                        delay_ms: Some(started.elapsed().as_millis().max(1) as u32),
                        error: None,
                    },
                    Ok(Err(e)) => LatencyResult {
                        node_id: node.id,
                        kind: "tcping".to_string(),
                        delay_ms: None,
                        error: Some(e.to_string()),
                    },
                    Err(_) => LatencyResult {
                        node_id: node.id,
                        kind: "tcping".to_string(),
                        delay_ms: None,
                        error: Some("timeout".to_string()),
                    },
                };
                let c = completed.fetch_add(1, Ordering::SeqCst) + 1;
                let p = if res.delay_ms.is_some() {
                    passed.fetch_add(1, Ordering::SeqCst) + 1
                } else {
                    passed.load(Ordering::SeqCst)
                };
                let f = if res.delay_ms.is_none() {
                    failed.fetch_add(1, Ordering::SeqCst) + 1
                } else {
                    failed.load(Ordering::SeqCst)
                };
                cb(LatencyProgressEvent {
                    kind: "tcping".to_string(),
                    phase: if c >= total { "done" } else { "testing" }.to_string(),
                    completed: c,
                    total,
                    passed: p,
                    failed: f,
                    strip_handshake: false,
                    latest: Some(res.clone()),
                });
                res
            }
        })
        .buffer_unordered(limit)
        .collect()
        .await;

    results
}

fn allocate_free_loopback_port() -> Result<u16> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    Ok(listener.local_addr()?.port())
}

/// Isolated zero-disruption batch real-delay tester (defaults to unified strip-handshake RTT):
/// launches a temporary no-TUN `sing-box` process bound to ephemeral loopback ports,
/// probes all target nodes concurrently, and unconditionally kills and cleans up the process
/// via RAII guard + Windows Job Object (leaving zero orphan processes).
pub async fn batch_real_delay(
    sing_box_path: &Path,
    nodes: &[NodeIR],
    test_url: &str,
    timeout_ms: u32,
    concurrency: usize,
) -> Result<Vec<LatencyResult>> {
    batch_real_delay_with_progress(
        sing_box_path,
        nodes,
        test_url,
        timeout_ms,
        concurrency,
        None,
        true,
        |_| {},
    )
    .await
}

/// Measures single-node real HTTP RTT over an ephemeral `socks` (SOCKS5) inbound
/// (`auth_user` routed strictly to `node_id`):
/// - `strip_handshake == true` (Unified RTT):
///   - Pass 1 (Warmup, NOT timed): opens the persistent SOCKS5 tunnel through `node_id`,
///     absorbing DNS + proxy TCP/QUIC + outer TLS/Reality + inner HTTPS TLS handshakes,
///     and returns the warm tunnel to hyper's keep-alive pool.
///   - Pass 2 (Timed): measures a clean single HTTP HEAD round-trip over the warm tunnel,
///     stripping first-connection handshake overhead for both `http://` and `https://` URLs.
/// - `strip_handshake == false` (Cold RTT):
///   - Measures a single cold HTTP HEAD request including full connection & TLS setup.
async fn probe_node_rtt(
    proxy_port: u16,
    auth_user: &str,
    test_url: &str,
    timeout_ms: u32,
    strip_handshake: bool,
) -> Result<u32> {
    let proxy_url = format!("socks5h://{}:p@127.0.0.1:{}", auth_user, proxy_port);
    let per_req_timeout = Duration::from_millis((timeout_ms as u64).max(800));
    let http = reqwest::Client::builder()
        .no_proxy()
        .proxy(reqwest::Proxy::all(&proxy_url)?)
        .pool_max_idle_per_host(1)
        .pool_idle_timeout(Duration::from_secs(15))
        .redirect(reqwest::redirect::Policy::none())
        .timeout(per_req_timeout)
        .build()?;

    if strip_handshake {
        // Pass 1: Warm up DNS + proxy tunnel + inner TLS handshake.
        let resp1 = http
            .head(test_url)
            .header(reqwest::header::CONNECTION, "keep-alive")
            .send()
            .await?;
        let _ = resp1.bytes().await;

        // Pass 2: Timed pure round-trip over the warm keep-alive tunnel.
        let started = Instant::now();
        let resp2 = http
            .head(test_url)
            .header(reqwest::header::CONNECTION, "keep-alive")
            .send()
            .await?;
        let elapsed_ms = started.elapsed().as_millis().max(1) as u32;
        let _ = resp2.bytes().await;
        Ok(elapsed_ms)
    } else {
        let started = Instant::now();
        let resp = http.head(test_url).send().await?;
        let elapsed_ms = started.elapsed().as_millis().max(1) as u32;
        let _ = resp.bytes().await;
        Ok(elapsed_ms)
    }
}

/// Isolated batch real-delay tester with incremental progress reporting and optional
/// first-handshake stripping (Unified RTT).
#[allow(clippy::too_many_arguments)]
pub async fn batch_real_delay_with_progress<F>(
    sing_box_path: &Path,
    nodes: &[NodeIR],
    test_url: &str,
    timeout_ms: u32,
    concurrency: usize,
    local_dns_server: Option<&str>,
    strip_handshake: bool,
    on_progress: F,
) -> Result<Vec<LatencyResult>>
where
    F: Fn(LatencyProgressEvent) + Send + Sync + 'static,
{
    let total = nodes.len();
    let cb = Arc::new(on_progress);
    if total == 0 {
        cb(LatencyProgressEvent {
            kind: "real".to_string(),
            phase: "done".to_string(),
            completed: 0,
            total: 0,
            passed: 0,
            failed: 0,
            strip_handshake,
            latest: None,
        });
        return Ok(Vec::new());
    }
    if !sing_box_path.exists() {
        return Err(anyhow!(
            "sing-box binary not found at {}",
            sing_box_path.display()
        ));
    }

    cb(LatencyProgressEvent {
        kind: "real".to_string(),
        phase: "starting".to_string(),
        completed: 0,
        total,
        passed: 0,
        failed: 0,
        strip_handshake,
        latest: None,
    });

    let control_port = allocate_free_loopback_port()?;
    let proxy_port = allocate_free_loopback_port()?;
    let secret = uuid::Uuid::new_v4().simple().to_string();
    let tmp_dir = std::env::temp_dir().join(format!("portline-test-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&tmp_dir)?;
    let cfg_path = tmp_dir.join("tester.json");

    let mut outbounds: Vec<Value> = vec![json!({ "type": "direct", "tag": "direct" })];
    let mut endpoints: Vec<Value> = Vec::new();
    let mut users: Vec<Value> = Vec::new();
    let mut route_rules: Vec<Value> = Vec::new();
    let mut probe_items: Vec<(NodeIR, String)> = Vec::with_capacity(nodes.len());
    for (idx, node) in nodes.iter().enumerate() {
        if node.protocol == Protocol::Wireguard {
            endpoints.push(compile_wireguard_endpoint(node));
        } else {
            outbounds.push(compile_proxy_outbound(node));
        }
        let auth_user = format!("u{}", idx);
        users.push(json!({ "username": auth_user, "password": "p" }));
        route_rules.push(json!({ "auth_user": [auth_user], "outbound": node.id }));
        probe_items.push((node.clone(), auth_user));
    }

    let dns_spec = local_dns_server.unwrap_or("223.5.5.5");
    let (local_dns, needs_boot) = compile_dns_server("dns-local", dns_spec, None);
    let mut dns_servers = Vec::new();
    if needs_boot {
        dns_servers.push(json!({
            "type": "udp",
            "tag": "dns-bootstrap",
            "server": "223.5.5.5"
        }));
    }
    dns_servers.push(local_dns);

    let inbounds = json!([{
        "type": "socks",
        "tag": "probe-socks",
        "listen": "127.0.0.1",
        "listen_port": proxy_port,
        "users": users
    }]);

    let mut route_obj = serde_json::Map::new();
    route_obj.insert("default_domain_resolver".to_string(), json!("dns-local"));
    if !route_rules.is_empty() {
        route_obj.insert("rules".to_string(), Value::Array(route_rules));
    }
    route_obj.insert("final".to_string(), json!("direct"));

    let mut cfg = serde_json::Map::new();
    cfg.insert("log".to_string(), json!({ "level": "error" }));
    cfg.insert(
        "dns".to_string(),
        json!({
            "servers": dns_servers,
            "final": "dns-local"
        }),
    );
    if !endpoints.is_empty() {
        cfg.insert("endpoints".to_string(), Value::Array(endpoints));
    }
    cfg.insert("inbounds".to_string(), inbounds);
    cfg.insert("outbounds".to_string(), Value::Array(outbounds));
    cfg.insert("route".to_string(), Value::Object(route_obj));
    cfg.insert(
        "experimental".to_string(),
        json!({
            "clash_api": {
                "external_controller": format!("127.0.0.1:{}", control_port),
                "secret": secret
            }
        }),
    );
    std::fs::write(&cfg_path, serde_json::to_string_pretty(&Value::Object(cfg))?)?;

    // Spawn temporary isolated sing-box with RAII kill-on-drop guard.
    let mut guard = spawn_managed_process(sing_box_path, &cfg_path)?;
    let client = Arc::new(ControlClient::new(control_port, &secret)?);

    // Wait up to 4 seconds for ephemeral Clash API and SOCKS5 probe inbound to come up.
    let mut ready = false;
    for _ in 0..40 {
        let api_ok = client.get_version().await.is_ok();
        let proxy_ok = TcpStream::connect(("127.0.0.1", proxy_port)).await.is_ok();
        if api_ok && proxy_ok {
            ready = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    let results = if ready {
        cb(LatencyProgressEvent {
            kind: "real".to_string(),
            phase: "testing".to_string(),
            completed: 0,
            total,
            passed: 0,
            failed: 0,
            strip_handshake,
            latest: None,
        });

        let completed = Arc::new(AtomicUsize::new(0));
        let passed = Arc::new(AtomicUsize::new(0));
        let failed = Arc::new(AtomicUsize::new(0));
        let url_arc = Arc::new(test_url.to_string());
        let limit = concurrency.clamp(1, 32);

        stream::iter(probe_items)
            .map(|(node, auth_user)| {
                let u = Arc::clone(&url_arc);
                let cb = Arc::clone(&cb);
                let completed = Arc::clone(&completed);
                let passed = Arc::clone(&passed);
                let failed = Arc::clone(&failed);
                async move {
                    let probe =
                        probe_node_rtt(proxy_port, &auth_user, &u, timeout_ms, strip_handshake)
                            .await;
                    let res = match probe {
                        Ok(ms) => LatencyResult {
                            node_id: node.id,
                            kind: "real".to_string(),
                            delay_ms: Some(ms.max(1)),
                            error: None,
                        },
                        Err(err) => LatencyResult {
                            node_id: node.id,
                            kind: "real".to_string(),
                            delay_ms: None,
                            error: Some(err.to_string()),
                        },
                    };
                    let done = completed.fetch_add(1, Ordering::SeqCst) + 1;
                    let p = if res.delay_ms.is_some() {
                        passed.fetch_add(1, Ordering::SeqCst) + 1
                    } else {
                        passed.load(Ordering::SeqCst)
                    };
                    let f = if res.delay_ms.is_none() {
                        failed.fetch_add(1, Ordering::SeqCst) + 1
                    } else {
                        failed.load(Ordering::SeqCst)
                    };
                    cb(LatencyProgressEvent {
                        kind: "real".to_string(),
                        phase: if done >= total { "done" } else { "testing" }.to_string(),
                        completed: done,
                        total,
                        passed: p,
                        failed: f,
                        strip_handshake,
                        latest: Some(res.clone()),
                    });
                    res
                }
            })
            .buffer_unordered(limit)
            .collect()
            .await
    } else {
        let errs: Vec<LatencyResult> = nodes
            .iter()
            .map(|n| LatencyResult {
                node_id: n.id.clone(),
                kind: "real".to_string(),
                delay_ms: None,
                error: Some("Isolated tester failed to start".to_string()),
            })
            .collect();
        cb(LatencyProgressEvent {
            kind: "real".to_string(),
            phase: "done".to_string(),
            completed: total,
            total,
            passed: 0,
            failed: total,
            strip_handshake,
            latest: None,
        });
        errs
    };

    guard.terminate();
    let _ = std::fs::remove_dir_all(&tmp_dir);
    Ok(results)
}
