use anyhow::{anyhow, Result};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleSetStatus {
    pub tag: String,
    pub cached_locally: bool,
    pub bytes: u64,
    pub fallback_mode: bool,
}

/// Builds candidate download URLs for official SagerNet sing-geosite / sing-geoip binary `.srs` files.
pub fn default_rule_set_urls(tag: &str) -> Vec<String> {
    let clean = sanitize_tag(tag);
    let repo = if clean.starts_with("geoip-") {
        "sing-geoip"
    } else {
        "sing-geosite"
    };
    let raw_gh = format!(
        "https://raw.githubusercontent.com/SagerNet/{}/rule-set/{}.srs",
        repo, clean
    );
    vec![
        format!(
            "https://cdn.jsdelivr.net/gh/SagerNet/{}@rule-set/{}.srs",
            repo, clean
        ),
        raw_gh.clone(),
        format!("https://ghfast.top/{}", raw_gh),
    ]
}

/// 3-tier `.srs` rule-set cache:
/// 1. Existing local `.srs` file in `<data_dir>/rule-sets/<tag>.srs`
/// 2. Dual-channel download (via local proxy port first, then direct / CDN mirrors)
/// 3. Offline fallback: never blocks startup; compiler transparently falls back to domain suffixes.
pub struct RuleCache {
    dir: PathBuf,
}

impl RuleCache {
    pub fn new(dir: PathBuf) -> Self {
        let _ = std::fs::create_dir_all(&dir);
        Self { dir }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn local_srs_path(&self, tag: &str) -> Option<PathBuf> {
        let candidate = self.dir.join(format!("{}.srs", sanitize_tag(tag)));
        if candidate.exists() && candidate.metadata().map(|m| m.len() > 0).unwrap_or(false) {
            Some(candidate)
        } else {
            None
        }
    }

    pub fn list_cached(&self) -> Vec<RuleSetStatus> {
        let mut out = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&self.dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("srs") {
                    if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                        let bytes = entry.metadata().map(|m| m.len()).unwrap_or(0);
                        out.push(RuleSetStatus {
                            tag: stem.to_string(),
                            cached_locally: bytes > 0,
                            bytes,
                            fallback_mode: bytes == 0,
                        });
                    }
                }
            }
        }
        out.sort_by(|a, b| a.tag.cmp(&b.tag));
        out
    }

    /// Evaluates whether `target` (domain or IP address) matches a locally cached binary `.srs`
    /// file using `sing-box rule-set match -f binary`. Returns `None` if binary or `.srs` is absent.
    pub fn match_local_srs(
        &self,
        sing_box_path: &Path,
        tag: &str,
        target: &str,
    ) -> Option<bool> {
        if !sing_box_path.exists() {
            return None;
        }
        let srs_path = self.local_srs_path(tag)?;
        let mut cmd = Command::new(sing_box_path);
        cmd.arg("rule-set")
            .arg("match")
            .arg("-f")
            .arg("binary")
            .arg(&srs_path)
            .arg(target);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x08000000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }
        let out = cmd.output().ok()?;
        if !out.status.success() {
            return None;
        }
        let stdout = String::from_utf8_lossy(&out.stdout);
        let stderr = String::from_utf8_lossy(&out.stderr);
        let matched = stdout.contains("match ") || stderr.contains("match ");
        Some(matched)
    }

    /// Attempts multi-mirror download (via active local proxy first, then direct CDN/mirrors).
    pub async fn refresh_rule_set_auto(
        &self,
        tag: &str,
        local_proxy_port: Option<u16>,
    ) -> RuleSetStatus {
        let safe_tag = sanitize_tag(tag);
        let dest = self.dir.join(format!("{}.srs", safe_tag));
        let urls = default_rule_set_urls(&safe_tag);

        if let Some(port) = local_proxy_port {
            for url in &urls {
                if let Ok(bytes) = fetch_bytes(url, Some(port)).await {
                    if write_atomic(&dest, &bytes).is_ok() {
                        return RuleSetStatus {
                            tag: safe_tag,
                            cached_locally: true,
                            bytes: bytes.len() as u64,
                            fallback_mode: false,
                        };
                    }
                }
            }
        }

        for url in &urls {
            if let Ok(bytes) = fetch_bytes(url, None).await {
                if write_atomic(&dest, &bytes).is_ok() {
                    return RuleSetStatus {
                        tag: safe_tag,
                        cached_locally: true,
                        bytes: bytes.len() as u64,
                        fallback_mode: false,
                    };
                }
            }
        }

        let existing_len = dest.metadata().map(|m| m.len()).unwrap_or(0);
        RuleSetStatus {
            tag: safe_tag,
            cached_locally: existing_len > 0,
            bytes: existing_len,
            fallback_mode: true,
        }
    }

    /// Attempts dual-channel download (proxy -> direct); on network failure, degrades cleanly
    /// to existing local cache or offline fallback without returning an unrecoverable error.
    pub async fn refresh_rule_set(
        &self,
        tag: &str,
        remote_url: &str,
        local_proxy_port: Option<u16>,
    ) -> RuleSetStatus {
        let safe_tag = sanitize_tag(tag);
        let dest = self.dir.join(format!("{}.srs", safe_tag));

        // Channel 1: Try via local proxy if available.
        if let Some(port) = local_proxy_port {
            if let Ok(bytes) = fetch_bytes(remote_url, Some(port)).await {
                if write_atomic(&dest, &bytes).is_ok() {
                    return RuleSetStatus {
                        tag: safe_tag,
                        cached_locally: true,
                        bytes: bytes.len() as u64,
                        fallback_mode: false,
                    };
                }
            }
        }

        // Channel 2: Try direct download.
        if let Ok(bytes) = fetch_bytes(remote_url, None).await {
            if write_atomic(&dest, &bytes).is_ok() {
                return RuleSetStatus {
                    tag: safe_tag,
                    cached_locally: true,
                    bytes: bytes.len() as u64,
                    fallback_mode: false,
                };
            }
        }

        // Channel 3: Graceful degradation to existing cache or built-in offline domain list.
        let existing_len = dest.metadata().map(|m| m.len()).unwrap_or(0);
        RuleSetStatus {
            tag: safe_tag,
            cached_locally: existing_len > 0,
            bytes: existing_len,
            fallback_mode: true,
        }
    }
}

async fn fetch_bytes(url: &str, proxy_port: Option<u16>) -> Result<Vec<u8>> {
    let mut builder = reqwest::Client::builder()
        .user_agent("Portline/0.1")
        .connect_timeout(Duration::from_secs(6))
        .timeout(Duration::from_secs(20));
    if let Some(port) = proxy_port {
        builder = builder.proxy(reqwest::Proxy::all(format!("http://127.0.0.1:{}", port))?);
    } else {
        builder = builder.no_proxy();
    }
    let client = builder.build()?;
    let resp = client.get(url).send().await?;
    if !resp.status().is_success() {
        return Err(anyhow!("HTTP {}", resp.status()));
    }
    let bytes = resp.bytes().await?.to_vec();
    if bytes.is_empty() {
        return Err(anyhow!("Empty .srs payload"));
    }
    Ok(bytes)
}

fn write_atomic(dest: &Path, data: &[u8]) -> Result<()> {
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = dest.with_extension("srs.tmp");
    std::fs::write(&tmp, data)?;
    std::fs::rename(&tmp, dest)?;
    Ok(())
}

pub fn sanitize_tag(tag: &str) -> String {
    tag.trim()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '!' | '@' | '.') {
                c
            } else {
                '-'
            }
        })
        .collect()
}
