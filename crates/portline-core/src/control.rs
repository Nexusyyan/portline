use crate::model::ConnectionItem;
use anyhow::{anyhow, Result};
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION};
use serde_json::{json, Value};
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct ControlClient {
    base_url: String,
    http: reqwest::Client,
}

impl ControlClient {
    pub fn new(port: u16, secret: &str) -> Result<Self> {
        let mut headers = HeaderMap::new();
        if !secret.is_empty() {
            let value = HeaderValue::from_str(&format!("Bearer {}", secret))?;
            headers.insert(AUTHORIZATION, value);
        }
        let http = reqwest::Client::builder()
            .no_proxy()
            .default_headers(headers)
            .timeout(Duration::from_secs(5))
            .build()?;
        Ok(Self {
            base_url: format!("http://127.0.0.1:{}", port),
            http,
        })
    }

    pub async fn get_version(&self) -> Result<String> {
        let resp = self
            .http
            .get(format!("{}/version", self.base_url))
            .send()
            .await?;
        let val: Value = resp.json().await?;
        Ok(val["version"].as_str().unwrap_or("unknown").to_string())
    }

    /// 0ms zero-downtime selector hot-switch via sing-box Clash API.
    pub async fn select_proxy(&self, group_tag: &str, outbound_tag: &str) -> Result<()> {
        let url = format!("{}/proxies/{}", self.base_url, group_tag);
        let resp = self
            .http
            .put(url)
            .json(&json!({ "name": outbound_tag }))
            .send()
            .await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(anyhow!("Control API select_proxy failed ({}): {}", status, body));
        }
        Ok(())
    }

    pub async fn get_proxy_now(&self, group_tag: &str) -> Result<String> {
        let url = format!("{}/proxies/{}", self.base_url, group_tag);
        let val: Value = self.http.get(url).send().await?.json().await?;
        Ok(val["now"].as_str().unwrap_or_default().to_string())
    }

    pub async fn get_proxy_delay(
        &self,
        proxy_tag: &str,
        test_url: &str,
        timeout_ms: u32,
    ) -> Result<u32> {
        let resp = self
            .http
            .get(format!("{}/proxies/{}/delay", self.base_url, proxy_tag))
            .query(&[
                ("url", test_url),
                ("timeout", &timeout_ms.to_string()),
            ])
            .timeout(Duration::from_millis((timeout_ms as u64) + 1500))
            .send()
            .await?;
        if !resp.status().is_success() {
            return Err(anyhow!("Delay probe failed with status {}", resp.status()));
        }
        let val: Value = resp.json().await?;
        val["delay"]
            .as_u64()
            .map(|d| d as u32)
            .ok_or_else(|| anyhow!("Missing delay field in response"))
    }

    pub async fn get_connections(&self) -> Result<(Vec<ConnectionItem>, u64, u64, u64)> {
        let resp = self
            .http
            .get(format!("{}/connections", self.base_url))
            .send()
            .await?;
        if !resp.status().is_success() {
            return Err(anyhow!("GET /connections failed: {}", resp.status()));
        }
        let val: Value = resp.json().await?;
        let upload_total = val["uploadTotal"].as_u64().unwrap_or(0);
        let download_total = val["downloadTotal"].as_u64().unwrap_or(0);
        let memory_bytes = val["memory"].as_u64().unwrap_or(0);
        let mut items = Vec::new();
        if let Some(arr) = val["connections"].as_array() {
            for c in arr {
                let meta = &c["metadata"];
                let host = meta["host"]
                    .as_str()
                    .filter(|s| !s.is_empty())
                    .or_else(|| meta["destinationIP"].as_str())
                    .unwrap_or("unknown");
                let port = meta["destinationPort"].as_str().unwrap_or("");
                let dest = if port.is_empty() {
                    host.to_string()
                } else {
                    format!("{}:{}", host, port)
                };
                let proc_name = meta["processPath"]
                    .as_str()
                    .and_then(|p| p.rsplit(['\\', '/']).next())
                    .filter(|s| !s.is_empty())
                    .or_else(|| meta["process"].as_str().filter(|s| !s.is_empty()))
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| {
                        let src_ip = meta["sourceIP"].as_str().unwrap_or("");
                        if src_ip.is_empty() || src_ip == "127.0.0.1" || src_ip == "::1" {
                            "Local App".to_string()
                        } else {
                            src_ip.to_string()
                        }
                    });
                let raw_inbound = meta["inboundName"]
                    .as_str()
                    .filter(|s| !s.is_empty())
                    .or_else(|| meta["type"].as_str())
                    .unwrap_or("mixed");
                let inbound = normalize_inbound_label(raw_inbound);
                let chains = c["chains"]
                    .as_array()
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|x| x.as_str().map(|s| s.to_string()))
                            .collect()
                    })
                    .unwrap_or_default();
                items.push(ConnectionItem {
                    id: c["id"].as_str().unwrap_or_default().to_string(),
                    process: proc_name,
                    destination: dest,
                    inbound,
                    rule: c["rule"].as_str().unwrap_or("final").to_string(),
                    chain: chains,
                    upload_bytes: c["upload"].as_u64().unwrap_or(0),
                    download_bytes: c["download"].as_u64().unwrap_or(0),
                    start: c["start"].as_str().unwrap_or_default().to_string(),
                });
            }
        }
        Ok((items, upload_total, download_total, memory_bytes))
    }

    pub async fn close_connection(&self, id: &str) -> Result<()> {
        let resp = self
            .http
            .delete(format!("{}/connections/{}", self.base_url, id))
            .send()
            .await?;
        if !resp.status().is_success() {
            return Err(anyhow!("DELETE /connections/{} failed", id));
        }
        Ok(())
    }

    pub async fn close_all_connections(&self) -> Result<()> {
        let resp = self
            .http
            .delete(format!("{}/connections", self.base_url))
            .send()
            .await?;
        if !resp.status().is_success() {
            return Err(anyhow!("DELETE /connections failed"));
        }
        Ok(())
    }
}

pub fn normalize_inbound_label(raw: &str) -> String {
    let trimmed = raw.trim();
    let lower = trimmed.to_ascii_lowercase();
    if lower.contains("tun") {
        return "TUN".to_string();
    }
    let tail = trimmed.rsplit('/').next().unwrap_or(trimmed);
    if let Some(port) = tail.strip_prefix("in-") {
        if port.chars().all(|c| c.is_ascii_digit()) && !port.is_empty() {
            return format!(":{}", port);
        }
    }
    tail.to_string()
}
