use crate::compiler::{compile_proxy_outbound, compile_wireguard_endpoint};
use crate::control::ControlClient;
use crate::model::{LatencyResult, NodeIR, Protocol};
use crate::supervisor::spawn_managed_process;
use anyhow::{anyhow, Result};
use futures::stream::{self, StreamExt};
use serde_json::{json, Value};
use std::net::TcpListener;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::net::TcpStream;

/// Concurrent TCPing latency tester (no sing-box needed, non-disruptive).
pub async fn batch_tcping(
    nodes: &[NodeIR],
    timeout_ms: u64,
    concurrency: usize,
) -> Vec<LatencyResult> {
    let limit = concurrency.clamp(1, 64);
    let timeout = Duration::from_millis(timeout_ms.max(300));
    stream::iter(nodes.iter().cloned())
        .map(|node| async move {
            let addr = format!("{}:{}", node.server, node.port);
            let started = Instant::now();
            match tokio::time::timeout(timeout, TcpStream::connect(&addr)).await {
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
            }
        })
        .buffer_unordered(limit)
        .collect()
        .await
}

fn allocate_free_loopback_port() -> Result<u16> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    Ok(listener.local_addr()?.port())
}

/// Isolated zero-disruption batch real-delay tester:
/// launches a temporary no-TUN `sing-box` process bound to an ephemeral Clash API port,
/// probes all target nodes concurrently, and unconditionally kills and cleans up the process
/// via RAII guard + Windows Job Object (leaving zero orphan processes).
pub async fn batch_real_delay(
    sing_box_path: &Path,
    nodes: &[NodeIR],
    test_url: &str,
    timeout_ms: u32,
    concurrency: usize,
) -> Result<Vec<LatencyResult>> {
    if nodes.is_empty() {
        return Ok(Vec::new());
    }
    if !sing_box_path.exists() {
        return Err(anyhow!(
            "sing-box binary not found at {}",
            sing_box_path.display()
        ));
    }

    let control_port = allocate_free_loopback_port()?;
    let secret = uuid::Uuid::new_v4().simple().to_string();
    let tmp_dir = std::env::temp_dir().join(format!("portline-test-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&tmp_dir)?;
    let cfg_path = tmp_dir.join("tester.json");

    let mut outbounds: Vec<Value> = vec![json!({ "type": "direct", "tag": "direct" })];
    let mut endpoints: Vec<Value> = Vec::new();
    for node in nodes {
        if node.protocol == Protocol::Wireguard {
            endpoints.push(compile_wireguard_endpoint(node));
        } else {
            outbounds.push(compile_proxy_outbound(node));
        }
    }

    let mut cfg = serde_json::Map::new();
    cfg.insert("log".to_string(), json!({ "level": "error" }));
    cfg.insert(
        "dns".to_string(),
        json!({
            "servers": [{ "type": "udp", "tag": "dns-local", "server": "223.5.5.5" }],
            "final": "dns-local"
        }),
    );
    if !endpoints.is_empty() {
        cfg.insert("endpoints".to_string(), Value::Array(endpoints));
    }
    cfg.insert("inbounds".to_string(), json!([]));
    cfg.insert("outbounds".to_string(), Value::Array(outbounds));
    cfg.insert(
        "route".to_string(),
        json!({
            "default_domain_resolver": "dns-local",
            "final": "direct"
        }),
    );
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

    // Wait up to 4 seconds for ephemeral Clash API to come up.
    let mut ready = false;
    for _ in 0..40 {
        if client.get_version().await.is_ok() {
            ready = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    let results = if ready {
        let url_arc = Arc::new(test_url.to_string());
        let limit = concurrency.clamp(1, 32);
        stream::iter(nodes.iter().cloned())
            .map(|node| {
                let c = Arc::clone(&client);
                let u = Arc::clone(&url_arc);
                async move {
                    match c.get_proxy_delay(&node.id, &u, timeout_ms).await {
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
                    }
                }
            })
            .buffer_unordered(limit)
            .collect()
            .await
    } else {
        nodes
            .iter()
            .map(|n| LatencyResult {
                node_id: n.id.clone(),
                kind: "real".to_string(),
                delay_ms: None,
                error: Some("Isolated tester failed to start".to_string()),
            })
            .collect()
    };

    guard.terminate();
    let _ = std::fs::remove_dir_all(&tmp_dir);
    Ok(results)
}
