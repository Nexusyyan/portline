pub mod clash_yaml;
pub mod singbox_json;
pub mod uri;

use crate::model::NodeIR;
use anyhow::Result;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportSummary {
    pub nodes: Vec<NodeIR>,
    pub skipped: usize,
    pub warnings: Vec<String>,
}

/// Auto-detects format (sing-box JSON, Clash YAML, raw URI lines, or Base64 subscription)
/// and parses all supported nodes into strongly typed `NodeIR`.
pub fn parse_payload(raw: &str, default_group: &str) -> Result<ImportSummary> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(ImportSummary {
            nodes: Vec::new(),
            skipped: 0,
            warnings: Vec::new(),
        });
    }

    if trimmed.starts_with('{') || trimmed.starts_with('[') {
        if let Ok(summary) = singbox_json::parse_singbox_json(trimmed, default_group) {
            if !summary.nodes.is_empty() {
                return Ok(summary);
            }
        }
    }

    if trimmed.contains("proxies:") && !trimmed.contains("://") {
        if let Ok(summary) = clash_yaml::parse_clash_yaml(trimmed, default_group) {
            if !summary.nodes.is_empty() {
                return Ok(summary);
            }
        }
    }

    // Try direct URI lines first; if none found, attempt Base64 subscription decode.
    let direct = uri::parse_uri_batch(trimmed, default_group);
    if !direct.nodes.is_empty() {
        return Ok(direct);
    }

    if let Some(decoded) = uri::try_decode_base64_text(trimmed) {
        let decoded_trim = decoded.trim();
        if decoded_trim.contains("proxies:") && !decoded_trim.contains("://") {
            if let Ok(summary) = clash_yaml::parse_clash_yaml(decoded_trim, default_group) {
                if !summary.nodes.is_empty() {
                    return Ok(summary);
                }
            }
        }
        return Ok(uri::parse_uri_batch(decoded_trim, default_group));
    }

    // Last attempt: Clash YAML even if URIs existed in comments
    if trimmed.contains("proxies:") {
        if let Ok(summary) = clash_yaml::parse_clash_yaml(trimmed, default_group) {
            if !summary.nodes.is_empty() {
                return Ok(summary);
            }
        }
    }

    Ok(direct)
}
