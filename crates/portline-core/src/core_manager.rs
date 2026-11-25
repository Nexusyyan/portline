use anyhow::{anyhow, Context, Result};
use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

pub const PINNED_CORE_VERSION: &str = "v1.14.4";
pub const PINNED_WIN_AMD64_ZIP_SHA256: &str =
    "99185d75d314a07dbba4a130c8ad986cbeb629a07927975636b341ca61f991df";
pub const PINNED_WIN_AMD64_EXE_SHA256: &str =
    "8f30682ee198855440d2a7fe070d5d3653793b2d01bb8c85eea88f93bbe7c8b9";

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OfficialReleaseInfo {
    pub tag_name: String,
    pub asset_name: String,
    pub download_url: String,
}

pub struct CoreManager {
    core_dir: PathBuf,
}

impl CoreManager {
    pub fn new(core_dir: PathBuf) -> Self {
        let _ = std::fs::create_dir_all(&core_dir);
        Self { core_dir }
    }

    pub fn binary_path(&self) -> PathBuf {
        self.core_dir.join(core_binary_name())
    }

    /// Resolves an existing sing-box binary across managed data dir, executable sibling dir,
    /// and system PATH so users who already own sing-box work out of the box.
    pub fn resolve_binary_path(&self) -> Option<PathBuf> {
        let managed = self.binary_path();
        if managed.exists() {
            return Some(managed);
        }
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                for cand in [
                    dir.join(core_binary_name()),
                    dir.join("core").join(core_binary_name()),
                ] {
                    if cand.exists() {
                        return Some(cand);
                    }
                }
            }
        }
        if let Ok(path_var) = std::env::var("PATH") {
            for dir in std::env::split_paths(&path_var) {
                let cand = dir.join(core_binary_name());
                if cand.exists() {
                    return Some(cand);
                }
            }
        }
        None
    }

    pub fn is_installed(&self) -> bool {
        self.resolve_binary_path().is_some()
    }

    pub fn sha256_of_binary(&self) -> Result<Option<String>> {
        let Some(path) = self.resolve_binary_path() else {
            return Ok(None);
        };
        Ok(Some(compute_file_sha256(&path)?))
    }

    pub fn detect_version(&self) -> Option<String> {
        let path = self.resolve_binary_path()?;
        let mut cmd = Command::new(&path);
        cmd.arg("version");
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
        let text = String::from_utf8_lossy(&out.stdout);
        for line in text.lines() {
            if let Some(rest) = line.strip_prefix("sing-box version ") {
                return Some(format!("v{}", rest.trim()));
            }
        }
        None
    }

    /// Copies a user-supplied official sing-box binary into the managed core folder
    /// after validating it responds to `version`.
    pub fn import_local_binary(&self, source: &Path, expected_sha256: Option<&str>) -> Result<String> {
        if !source.exists() {
            return Err(anyhow!("Source binary does not exist: {}", source.display()));
        }
        let digest = compute_file_sha256(source)?;
        if let Some(expected) = expected_sha256 {
            if !digest.eq_ignore_ascii_case(expected.trim()) {
                return Err(anyhow!(
                    "SHA-256 mismatch: expected {}, got {}",
                    expected,
                    digest
                ));
            }
        }
        std::fs::create_dir_all(&self.core_dir)?;
        let dest = self.binary_path();
        std::fs::copy(source, &dest)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&dest)?.permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&dest, perms)?;
        }
        self.detect_version()
            .ok_or_else(|| anyhow!("Imported binary failed `sing-box version` verification"))
    }

    /// Queries the official SagerNet/sing-box GitHub Releases API for the matching platform asset.
    pub async fn fetch_latest_official_release(&self) -> Result<OfficialReleaseInfo> {
        let client = reqwest::Client::builder()
            .user_agent("Portline-Desktop/0.1")
            .timeout(Duration::from_secs(15))
            .build()?;
        let val: serde_json::Value = client
            .get("https://api.github.com/repos/SagerNet/sing-box/releases/latest")
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        let tag = val["tag_name"]
            .as_str()
            .ok_or_else(|| anyhow!("Missing tag_name"))?
            .to_string();
        let needle = target_asset_suffix();
        let assets = val["assets"]
            .as_array()
            .ok_or_else(|| anyhow!("Missing assets"))?;
        for asset in assets {
            let name = asset["name"].as_str().unwrap_or_default();
            if name.ends_with(needle) && !name.contains("legacy") && !name.contains("musl") {
                let url = asset["browser_download_url"]
                    .as_str()
                    .ok_or_else(|| anyhow!("Missing download URL"))?
                    .to_string();
                return Ok(OfficialReleaseInfo {
                    tag_name: tag,
                    asset_name: name.to_string(),
                    download_url: url,
                });
            }
        }
        Err(anyhow!("No matching official release asset for {}", needle))
    }

    /// Downloads the pinned official sing-box Windows archive via direct GitHub + mirror
    /// fallbacks with a short connect timeout and strict mandatory SHA-256 verification
    /// so mirrors can never tamper with a single byte and restricted networks do not hang.
    pub async fn download_official_core(&self, expected_sha256: Option<&str>) -> Result<(String, String)> {
        let (tag_name, asset_name, official_url, pinned_zip_sha, pinned_exe_sha) =
            if cfg!(all(target_os = "windows", target_arch = "x86_64")) {
                let ver_num = PINNED_CORE_VERSION.trim_start_matches('v');
                let asset = format!("sing-box-{}-windows-amd64.zip", ver_num);
                let url = format!(
                    "https://github.com/SagerNet/sing-box/releases/download/{}/{}",
                    PINNED_CORE_VERSION, asset
                );
                (
                    PINNED_CORE_VERSION.to_string(),
                    asset,
                    url,
                    Some(expected_sha256.unwrap_or(PINNED_WIN_AMD64_ZIP_SHA256)),
                    Some(PINNED_WIN_AMD64_EXE_SHA256),
                )
            } else {
                let info = self.fetch_latest_official_release().await?;
                (
                    info.tag_name,
                    info.asset_name,
                    info.download_url,
                    expected_sha256,
                    None,
                )
            };

        let candidate_urls = vec![
            official_url.clone(),
            format!("https://ghfast.top/{}", official_url),
            format!("https://gh-proxy.com/{}", official_url),
        ];

        let client = reqwest::Client::builder()
            .user_agent("Portline-Desktop/0.1")
            .connect_timeout(Duration::from_secs(8))
            .timeout(Duration::from_secs(45))
            .build()?;

        let mut downloaded_bytes: Option<Vec<u8>> = None;
        let mut last_err = String::new();

        for url in &candidate_urls {
            match client.get(url).send().await {
                Ok(resp) if resp.status().is_success() => match resp.bytes().await {
                    Ok(bytes) => {
                        if let Some(expected) = pinned_zip_sha {
                            let got = hex::encode(Sha256::digest(&bytes));
                            if !got.eq_ignore_ascii_case(expected.trim()) {
                                last_err = format!(
                                    "SHA-256 mismatch from {} (expected {}, got {})",
                                    url, expected, got
                                );
                                continue;
                            }
                        }
                        downloaded_bytes = Some(bytes.to_vec());
                        break;
                    }
                    Err(e) => last_err = format!("{}: {}", url, e),
                },
                Ok(resp) => last_err = format!("{}: HTTP {}", url, resp.status()),
                Err(e) => last_err = format!("{}: {}", url, e),
            }
        }

        let bytes = downloaded_bytes.ok_or_else(|| {
            anyhow!(
                "Unable to download official sing-box {} from GitHub or mirrors ({}). Use the Portable With-Core bundle or import a local sing-box binary.",
                tag_name,
                last_err
            )
        })?;

        std::fs::create_dir_all(&self.core_dir)?;
        let dest = self.binary_path();

        if asset_name.ends_with(".zip") {
            let reader = std::io::Cursor::new(bytes.as_slice());
            let mut archive = zip::ZipArchive::new(reader).context("Invalid ZIP archive")?;
            let mut extracted = false;
            for i in 0..archive.len() {
                let mut file = archive.by_index(i)?;
                if file.name().ends_with("sing-box.exe") || file.name().ends_with("/sing-box") {
                    let mut buf = Vec::new();
                    file.read_to_end(&mut buf)?;
                    if let Some(expected_exe) = pinned_exe_sha {
                        let got_exe = hex::encode(Sha256::digest(&buf));
                        if !got_exe.eq_ignore_ascii_case(expected_exe) {
                            return Err(anyhow!(
                                "Extracted binary SHA-256 mismatch: expected {}, got {}",
                                expected_exe,
                                got_exe
                            ));
                        }
                    }
                    let tmp = dest.with_extension("tmp");
                    std::fs::write(&tmp, &buf)?;
                    std::fs::rename(&tmp, &dest)?;
                    extracted = true;
                    break;
                }
            }
            if !extracted {
                return Err(anyhow!("sing-box binary not found inside ZIP archive"));
            }
        } else {
            return Err(anyhow!(
                "Automatic extraction currently supports ZIP archives; import binary manually on this platform"
            ));
        }

        let version = self.detect_version().unwrap_or(tag_name);
        let bin_sha = compute_file_sha256(&dest)?;
        Ok((version, bin_sha))
    }
}

fn core_binary_name() -> &'static str {
    #[cfg(target_os = "windows")]
    {
        "sing-box.exe"
    }
    #[cfg(not(target_os = "windows"))]
    {
        "sing-box"
    }
}

fn target_asset_suffix() -> &'static str {
    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    {
        "-windows-amd64.zip"
    }
    #[cfg(all(target_os = "windows", target_arch = "aarch64"))]
    {
        "-windows-arm64.zip"
    }
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    {
        "-darwin-arm64.tar.gz"
    }
    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    {
        "-darwin-amd64.tar.gz"
    }
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        "-linux-amd64.tar.gz"
    }
    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    {
        "-linux-arm64.tar.gz"
    }
}

pub fn compute_file_sha256(path: &Path) -> Result<String> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 65536];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex::encode(hasher.finalize()))
}
