use crate::model::{
    AppSettings, ConnectionHistoryRecord, InboundPort, NetworkSnapshot, NodeIR,
    PlatformTrafficStat, RulePresetSpec, SimpleRoutingRule, SubscriptionGroup,
};
use anyhow::{Context, Result};
use rusqlite::{params, Connection, OptionalExtension};
use std::collections::HashMap;
use std::net::IpAddr;
use std::path::{Path, PathBuf};

const MAX_CONNECTION_HISTORY_ROWS: i64 = 20_000;

#[derive(Debug, Clone)]
pub struct DataPaths {
    pub portable_mode: bool,
    pub root_dir: PathBuf,
    pub db_path: PathBuf,
    pub core_dir: PathBuf,
    pub rule_sets_dir: PathBuf,
    pub runtime_dir: PathBuf,
}

impl DataPaths {
    pub fn resolve() -> Result<Self> {
        let exe_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.to_path_buf()))
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
        Self::resolve_from_exe_dir(&exe_dir)
    }

    pub fn resolve_from_exe_dir(exe_dir: &Path) -> Result<Self> {
        let portable_marker = exe_dir.join("portable.json");
        let portable_mode = portable_marker.exists()
            && std::fs::read_to_string(&portable_marker)
                .ok()
                .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
                .and_then(|v| v.get("portable").and_then(|b| b.as_bool()))
                .unwrap_or(false);

        let root_dir = if portable_mode {
            exe_dir.join("portline-data")
        } else {
            default_os_data_dir()
        };

        Self::from_root(root_dir, portable_mode)
    }

    pub fn from_root(root_dir: PathBuf, portable_mode: bool) -> Result<Self> {
        let core_dir = root_dir.join("core");
        let rule_sets_dir = root_dir.join("rule-sets");
        let runtime_dir = root_dir.join("runtime");
        std::fs::create_dir_all(&root_dir)?;
        std::fs::create_dir_all(&core_dir)?;
        std::fs::create_dir_all(&rule_sets_dir)?;
        std::fs::create_dir_all(&runtime_dir)?;
        let db_path = root_dir.join("portline.db");
        Ok(Self {
            portable_mode,
            root_dir,
            db_path,
            core_dir,
            rule_sets_dir,
            runtime_dir,
        })
    }
}

fn default_os_data_dir() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        if let Ok(local) = std::env::var("LOCALAPPDATA") {
            return PathBuf::from(local).join("Portline");
        }
    }
    #[cfg(target_os = "macos")]
    {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home)
                .join("Library")
                .join("Application Support")
                .join("Portline");
        }
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
            return PathBuf::from(xdg).join("portline");
        }
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home).join(".local").join("share").join("portline");
        }
    }
    PathBuf::from("portline-data")
}

/// Extracts hostname or IP from `host:port` or `[ipv6]:port`.
pub fn extract_host(destination: &str) -> String {
    let trimmed = destination.trim();
    if trimmed.starts_with('[') {
        if let Some(end) = trimmed.find(']') {
            return trimmed[1..end].to_ascii_lowercase();
        }
    }
    if trimmed.parse::<IpAddr>().is_ok() {
        return trimmed.to_ascii_lowercase();
    }
    if let Some((host, port)) = trimmed.rsplit_once(':') {
        if !host.is_empty() && port.chars().all(|c| c.is_ascii_digit()) {
            return host.trim_end_matches('.').to_ascii_lowercase();
        }
    }
    trimmed.trim_end_matches('.').to_ascii_lowercase()
}

fn host_matches(host: &str, suffixes: &[&str]) -> bool {
    suffixes
        .iter()
        .any(|s| host == *s || host.ends_with(&format!(".{}", s)))
}

fn extract_root_domain(host: &str) -> String {
    if host.parse::<IpAddr>().is_ok() {
        return host.to_string();
    }
    let parts: Vec<&str> = host.split('.').filter(|p| !p.is_empty()).collect();
    if parts.len() <= 2 {
        return host.to_string();
    }
    let last_two = format!("{}.{}", parts[parts.len() - 2], parts[parts.len() - 1]);
    let multi_tld = matches!(
        last_two.as_str(),
        "co.uk"
            | "org.uk"
            | "co.jp"
            | "ne.jp"
            | "or.jp"
            | "com.cn"
            | "net.cn"
            | "org.cn"
            | "gov.cn"
            | "edu.cn"
            | "com.tw"
            | "com.hk"
            | "com.sg"
            | "com.au"
            | "co.kr"
            | "com.br"
    );
    if multi_tld && parts.len() >= 3 {
        format!(
            "{}.{}.{}",
            parts[parts.len() - 3],
            parts[parts.len() - 2],
            parts[parts.len() - 1]
        )
    } else {
        last_two
    }
}

/// Classifies a connection's destination host and local process into a readable platform name
/// for cumulative per-platform traffic analytics.
pub fn classify_platform(host: &str, process: &str) -> String {
    let h = host.trim().to_ascii_lowercase();
    let p = process.trim().to_ascii_lowercase();

    if host_matches(
        &h,
        &[
            "youtube.com",
            "googlevideo.com",
            "ytimg.com",
            "youtu.be",
            "ggpht.com",
        ],
    ) {
        return "YouTube".to_string();
    }
    if host_matches(
        &h,
        &[
            "openai.com",
            "chatgpt.com",
            "oaistatic.com",
            "oaiusercontent.com",
            "sora.com",
        ],
    ) {
        return "OpenAI / ChatGPT".to_string();
    }
    if host_matches(&h, &["claude.ai", "anthropic.com", "claudeusercontent.com"]) {
        return "Claude".to_string();
    }
    if host_matches(
        &h,
        &[
            "gemini.google.com",
            "aistudio.google.com",
            "generativelanguage.googleapis.com",
            "deepmind.google",
            "google.com",
            "googleapis.com",
            "gstatic.com",
            "googleusercontent.com",
            "gmail.com",
            "gvt1.com",
            "gvt2.com",
            "android.com",
            "chromium.org",
            "withgoogle.com",
        ],
    ) {
        return "Google".to_string();
    }
    if host_matches(
        &h,
        &[
            "github.com",
            "githubusercontent.com",
            "githubassets.com",
            "github.io",
            "ghcr.io",
            "githubcopilot.com",
        ],
    ) {
        return "GitHub".to_string();
    }
    if host_matches(
        &h,
        &[
            "bilibili.com",
            "hdslb.com",
            "bilivideo.com",
            "bilivideo.cn",
            "biliapi.net",
            "biliapi.com",
            "b23.tv",
        ],
    ) {
        return "Bilibili".to_string();
    }
    if host_matches(
        &h,
        &[
            "telegram.org",
            "t.me",
            "telegram.me",
            "telegra.ph",
            "tdesktop.com",
            "telesco.pe",
        ],
    ) || p.contains("telegram")
    {
        return "Telegram".to_string();
    }
    if host_matches(&h, &["x.com", "twitter.com", "twimg.com", "t.co", "x.ai", "grok.com"]) {
        return "X / Twitter".to_string();
    }
    if host_matches(
        &h,
        &[
            "netflix.com",
            "nflxvideo.net",
            "nflximg.net",
            "nflximg.com",
            "nflxext.com",
            "nflxso.net",
        ],
    ) {
        return "Netflix".to_string();
    }
    if host_matches(
        &h,
        &["disneyplus.com", "disney-plus.net", "dssott.com", "bamgrid.com"],
    ) {
        return "Disney+".to_string();
    }
    if host_matches(&h, &["twitch.tv", "ttvnw.net", "jtvnw.net", "twitchcdn.net"]) {
        return "Twitch".to_string();
    }
    if host_matches(&h, &["spotify.com", "scdn.co", "spotifycdn.com"]) {
        return "Spotify".to_string();
    }
    if host_matches(
        &h,
        &["discord.com", "discordapp.com", "discordapp.net", "discord.gg", "discord.media"],
    ) || p.contains("discord")
    {
        return "Discord".to_string();
    }
    if host_matches(
        &h,
        &[
            "steampowered.com",
            "steamcommunity.com",
            "steamstatic.com",
            "steamcontent.com",
            "steamserver.net",
        ],
    ) || p.contains("steam")
    {
        return "Steam".to_string();
    }
    if host_matches(
        &h,
        &[
            "tiktok.com",
            "tiktokv.com",
            "tiktokcdn.com",
            "douyin.com",
            "douyinpic.com",
            "douyinvod.com",
            "byteimg.com",
            "bytedance.com",
            "snssdk.com",
        ],
    ) {
        return "TikTok / Douyin".to_string();
    }
    if host_matches(
        &h,
        &[
            "apple.com",
            "icloud.com",
            "icloud-content.com",
            "mzstatic.com",
            "cdn-apple.com",
            "aaplimg.com",
        ],
    ) {
        return "Apple".to_string();
    }
    if host_matches(
        &h,
        &[
            "microsoft.com",
            "live.com",
            "office.com",
            "office365.com",
            "onedrive.com",
            "sharepoint.com",
            "windowsupdate.com",
            "windows.com",
            "bing.com",
            "msn.com",
            "azure.com",
            "msedge.net",
            "visualstudio.com",
        ],
    ) {
        return "Microsoft".to_string();
    }
    if host_matches(&h, &["reddit.com", "redd.it", "redditstatic.com", "redditmedia.com"]) {
        return "Reddit".to_string();
    }
    if host_matches(
        &h,
        &[
            "instagram.com",
            "cdninstagram.com",
            "facebook.com",
            "fbcdn.net",
            "meta.com",
            "whatsapp.com",
            "whatsapp.net",
            "threads.net",
        ],
    ) {
        return "Meta / Instagram".to_string();
    }
    if host_matches(
        &h,
        &[
            "qq.com",
            "weixin.qq.com",
            "tencent.com",
            "qpic.cn",
            "gtimg.com",
            "myqcloud.com",
            "wechat.com",
        ],
    ) || p.contains("wechat")
        || p == "qq.exe"
    {
        return "Tencent / WeChat".to_string();
    }
    if host_matches(
        &h,
        &[
            "taobao.com",
            "tmall.com",
            "alicdn.com",
            "alipay.com",
            "aliyun.com",
            "aliyuncs.com",
            "alibaba.com",
        ],
    ) {
        return "Alibaba / Taobao".to_string();
    }
    if host_matches(&h, &["baidu.com", "bdstatic.com", "baidupcs.com", "bcebos.com"]) {
        return "Baidu".to_string();
    }
    if host_matches(&h, &["zhihu.com", "zhimg.com"]) {
        return "Zhihu".to_string();
    }
    if host_matches(&h, &["jd.com", "360buyimg.com", "jdcloud.com"]) {
        return "JD".to_string();
    }
    if host_matches(&h, &["xiaohongshu.com", "xhscdn.com"]) {
        return "Xiaohongshu".to_string();
    }
    if host_matches(&h, &["163.com", "126.net", "netease.com"]) {
        return "NetEase".to_string();
    }
    if host_matches(&h, &["cloudflare.com", "cloudflareinsights.com", "workers.dev", "pages.dev"]) {
        return "Cloudflare".to_string();
    }
    if host_matches(
        &h,
        &[
            "cursor.sh",
            "cursor.com",
            "npmjs.org",
            "npmjs.com",
            "pypi.org",
            "pythonhosted.org",
            "crates.io",
            "docker.io",
            "docker.com",
            "gitlab.com",
            "vercel.com",
            "stackoverflow.com",
            "huggingface.co",
        ],
    ) {
        return "Dev & Packages".to_string();
    }
    if host_matches(&h, &["wikipedia.org", "wikimedia.org"]) {
        return "Wikipedia".to_string();
    }

    if !h.is_empty() && h != "unknown" {
        if h.parse::<IpAddr>().is_ok() {
            if !p.is_empty() && p != "local app" {
                return process.to_string();
            }
            return h;
        }
        return extract_root_domain(&h);
    }

    if !process.trim().is_empty() {
        process.trim().to_string()
    } else {
        "Other".to_string()
    }
}

pub struct Store {
    conn: Connection,
}

impl Store {
    pub fn open(db_path: &Path) -> Result<Self> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(db_path)
            .with_context(|| format!("Failed to open SQLite DB at {}", db_path.display()))?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        let store = Self { conn };
        store.migrate()?;
        store.seed_defaults_if_empty()?;
        Ok(store)
    }

    fn migrate(&self) -> Result<()> {
        self.conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS nodes (
                id TEXT PRIMARY KEY,
                group_name TEXT NOT NULL,
                sort_order INTEGER NOT NULL DEFAULT 0,
                payload TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS subscriptions (
                id TEXT PRIMARY KEY,
                payload TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS routing_rules (
                id TEXT PRIMARY KEY,
                priority INTEGER NOT NULL DEFAULT 0,
                payload TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS inbound_ports (
                id TEXT PRIMARY KEY,
                port INTEGER NOT NULL UNIQUE,
                payload TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS kv_store (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS connection_history (
                id TEXT PRIMARY KEY,
                process TEXT NOT NULL,
                destination TEXT NOT NULL,
                host TEXT NOT NULL,
                platform TEXT NOT NULL,
                inbound TEXT NOT NULL,
                rule TEXT NOT NULL,
                chain_json TEXT NOT NULL,
                upload_bytes INTEGER NOT NULL,
                download_bytes INTEGER NOT NULL,
                started_at TEXT NOT NULL,
                closed_at TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_conn_history_closed ON connection_history(closed_at DESC);
            CREATE INDEX IF NOT EXISTS idx_conn_history_platform ON connection_history(platform);
            ",
        )?;
        Ok(())
    }

    fn seed_defaults_if_empty(&self) -> Result<()> {
        let schema_ver = self.get_kv("rules_schema_version")?;
        let rule_count: i64 =
            self.conn
                .query_row("SELECT COUNT(*) FROM routing_rules", [], |r| r.get(0))?;

        if rule_count == 0 {
            for rule in default_simple_rules() {
                self.upsert_rule(&rule)?;
            }
            self.set_kv("rules_schema_version", "2")?;
        } else if schema_ver.as_deref() != Some("2") {
            // One-time migration: remove legacy custom hand-coded rules while preserving
            // any user-created `r-custom-*` rules, then install the classic site/ip strategy.
            let legacy_ids = [
                "r-ai",
                "r-youtube",
                "r-media",
                "r-telegram",
                "r-dev",
                "r-cn",
                "r-ads",
            ];
            for id in legacy_ids {
                let _ = self.delete_rule(id);
            }
            for rule in default_simple_rules() {
                self.upsert_rule(&rule)?;
            }
            self.set_kv("rules_schema_version", "2")?;
        }
        if self.get_kv("settings")?.is_none() {
            self.save_settings(&AppSettings::default())?;
        }
        Ok(())
    }

    fn get_kv(&self, key: &str) -> Result<Option<String>> {
        let val = self
            .conn
            .query_row(
                "SELECT value FROM kv_store WHERE key = ?1",
                params![key],
                |r| r.get::<_, String>(0),
            )
            .optional()?;
        Ok(val)
    }

    fn set_kv(&self, key: &str, value: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO kv_store (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    pub fn load_settings(&self) -> Result<AppSettings> {
        match self.get_kv("settings")? {
            Some(raw) => Ok(serde_json::from_str(&raw).unwrap_or_default()),
            None => Ok(AppSettings::default()),
        }
    }

    pub fn save_settings(&self, settings: &AppSettings) -> Result<()> {
        self.set_kv("settings", &serde_json::to_string(settings)?)
    }

    pub fn load_netguard_snapshot(&self) -> Result<Option<NetworkSnapshot>> {
        match self.get_kv("netguard_snapshot")? {
            Some(raw) => Ok(serde_json::from_str(&raw).ok()),
            None => Ok(None),
        }
    }

    pub fn save_netguard_snapshot(&self, snapshot: Option<&NetworkSnapshot>) -> Result<()> {
        match snapshot {
            Some(s) => self.set_kv("netguard_snapshot", &serde_json::to_string(s)?),
            None => {
                self.conn
                    .execute("DELETE FROM kv_store WHERE key = 'netguard_snapshot'", [])?;
                Ok(())
            }
        }
    }

    pub fn list_nodes(&self) -> Result<Vec<NodeIR>> {
        let mut stmt = self
            .conn
            .prepare("SELECT payload FROM nodes ORDER BY sort_order ASC, rowid ASC")?;
        let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
        let mut out = Vec::new();
        for row in rows {
            if let Ok(mut node) = serde_json::from_str::<NodeIR>(&row?) {
                if node.region == "OTHER" || node.region.is_empty() {
                    node.region = crate::model::infer_region(&node.remark, &node.server);
                }
                out.push(node);
            }
        }
        Ok(out)
    }

    pub fn upsert_nodes(&mut self, nodes: &[NodeIR]) -> Result<()> {
        let tx = self.conn.transaction()?;
        let max_order: i64 = tx.query_row(
            "SELECT COALESCE(MAX(sort_order), 0) FROM nodes",
            [],
            |r| r.get(0),
        )?;
        for (i, node) in nodes.iter().enumerate() {
            let payload = serde_json::to_string(node)?;
            tx.execute(
                "INSERT INTO nodes (id, group_name, sort_order, payload) VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(id) DO UPDATE SET group_name = excluded.group_name, payload = excluded.payload",
                params![node.id, node.group, max_order + (i as i64) + 1, payload],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn replace_group_nodes(&mut self, group_name: &str, nodes: &[NodeIR]) -> Result<()> {
        let tx = self.conn.transaction()?;
        tx.execute("DELETE FROM nodes WHERE group_name = ?1", params![group_name])?;
        let max_order: i64 = tx.query_row(
            "SELECT COALESCE(MAX(sort_order), 0) FROM nodes",
            [],
            |r| r.get(0),
        )?;
        for (i, node) in nodes.iter().enumerate() {
            let payload = serde_json::to_string(node)?;
            tx.execute(
                "INSERT INTO nodes (id, group_name, sort_order, payload) VALUES (?1, ?2, ?3, ?4)",
                params![node.id, node.group, max_order + (i as i64) + 1, payload],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn delete_node(&self, id: &str) -> Result<bool> {
        let changed = self
            .conn
            .execute("DELETE FROM nodes WHERE id = ?1", params![id])?;
        Ok(changed > 0)
    }

    pub fn update_node_latency(
        &self,
        id: &str,
        tcping_ms: Option<u32>,
        real_delay_ms: Option<u32>,
    ) -> Result<()> {
        let existing: Option<String> = self
            .conn
            .query_row("SELECT payload FROM nodes WHERE id = ?1", params![id], |r| {
                r.get(0)
            })
            .optional()?;
        if let Some(raw) = existing {
            let mut node: NodeIR = serde_json::from_str(&raw)?;
            if tcping_ms.is_some() {
                node.tcping_ms = tcping_ms;
            }
            if real_delay_ms.is_some() {
                node.real_delay_ms = real_delay_ms;
            }
            let updated = serde_json::to_string(&node)?;
            self.conn.execute(
                "UPDATE nodes SET payload = ?2 WHERE id = ?1",
                params![id, updated],
            )?;
        }
        Ok(())
    }

    pub fn list_subscriptions(&self) -> Result<Vec<SubscriptionGroup>> {
        let mut stmt = self
            .conn
            .prepare("SELECT payload FROM subscriptions ORDER BY rowid ASC")?;
        let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
        let mut out = Vec::new();
        for row in rows {
            if let Ok(sub) = serde_json::from_str::<SubscriptionGroup>(&row?) {
                out.push(sub);
            }
        }
        Ok(out)
    }

    pub fn upsert_subscription(&self, sub: &SubscriptionGroup) -> Result<()> {
        let payload = serde_json::to_string(sub)?;
        self.conn.execute(
            "INSERT INTO subscriptions (id, payload) VALUES (?1, ?2)
             ON CONFLICT(id) DO UPDATE SET payload = excluded.payload",
            params![sub.id, payload],
        )?;
        Ok(())
    }

    pub fn delete_subscription(&self, id: &str) -> Result<()> {
        self.conn
            .execute("DELETE FROM subscriptions WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn list_rules(&self) -> Result<Vec<SimpleRoutingRule>> {
        let mut stmt = self
            .conn
            .prepare("SELECT payload FROM routing_rules ORDER BY priority ASC, rowid ASC")?;
        let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
        let mut out = Vec::new();
        for row in rows {
            if let Ok(rule) = serde_json::from_str::<SimpleRoutingRule>(&row?) {
                out.push(rule);
            }
        }
        Ok(out)
    }

    pub fn upsert_rule(&self, rule: &SimpleRoutingRule) -> Result<()> {
        let payload = serde_json::to_string(rule)?;
        self.conn.execute(
            "INSERT INTO routing_rules (id, priority, payload) VALUES (?1, ?2, ?3)
             ON CONFLICT(id) DO UPDATE SET priority = excluded.priority, payload = excluded.payload",
            params![rule.id, rule.priority, payload],
        )?;
        Ok(())
    }

    pub fn delete_rule(&self, id: &str) -> Result<bool> {
        let changed = self
            .conn
            .execute("DELETE FROM routing_rules WHERE id = ?1", params![id])?;
        Ok(changed > 0)
    }

    /// Replaces built-in rules with a classic community site/ip strategy while preserving
    /// any user-created custom rules (`r-custom-*`).
    pub fn apply_routing_strategy(&mut self, strategy_id: &str) -> Result<Vec<SimpleRoutingRule>> {
        let tx = self.conn.transaction()?;
        tx.execute(
            "DELETE FROM routing_rules WHERE id NOT LIKE 'r-custom-%'",
            [],
        )?;
        for rule in classic_strategy_rules(strategy_id) {
            let payload = serde_json::to_string(&rule)?;
            tx.execute(
                "INSERT INTO routing_rules (id, priority, payload) VALUES (?1, ?2, ?3)
                 ON CONFLICT(id) DO UPDATE SET priority = excluded.priority, payload = excluded.payload",
                params![rule.id, rule.priority, payload],
            )?;
        }
        tx.commit()?;
        self.set_kv("rules_schema_version", "2")?;
        self.list_rules()
    }

    pub fn list_inbound_ports(&self) -> Result<Vec<InboundPort>> {
        let mut stmt = self
            .conn
            .prepare("SELECT payload FROM inbound_ports ORDER BY port ASC")?;
        let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
        let mut out = Vec::new();
        for row in rows {
            if let Ok(port) = serde_json::from_str::<InboundPort>(&row?) {
                out.push(port);
            }
        }
        Ok(out)
    }

    pub fn upsert_inbound_port(&self, port: &InboundPort) -> Result<()> {
        let payload = serde_json::to_string(port)?;
        self.conn.execute(
            "INSERT INTO inbound_ports (id, port, payload) VALUES (?1, ?2, ?3)
             ON CONFLICT(id) DO UPDATE SET port = excluded.port, payload = excluded.payload",
            params![port.id, port.port, payload],
        )?;
        Ok(())
    }

    pub fn delete_inbound_port(&self, id: &str) -> Result<bool> {
        let changed = self
            .conn
            .execute("DELETE FROM inbound_ports WHERE id = ?1", params![id])?;
        Ok(changed > 0)
    }

    pub fn insert_connection_history(&mut self, records: &[ConnectionHistoryRecord]) -> Result<()> {
        if records.is_empty() {
            return Ok(());
        }
        let tx = self.conn.transaction()?;
        for rec in records {
            let chain_json = serde_json::to_string(&rec.chain)?;
            tx.execute(
                "INSERT INTO connection_history (
                    id, process, destination, host, platform, inbound, rule, chain_json,
                    upload_bytes, download_bytes, started_at, closed_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
                 ON CONFLICT(id) DO UPDATE SET
                    upload_bytes = MAX(connection_history.upload_bytes, excluded.upload_bytes),
                    download_bytes = MAX(connection_history.download_bytes, excluded.download_bytes),
                    closed_at = excluded.closed_at",
                params![
                    rec.id,
                    rec.process,
                    rec.destination,
                    rec.host,
                    rec.platform,
                    rec.inbound,
                    rec.rule,
                    chain_json,
                    rec.upload_bytes as i64,
                    rec.download_bytes as i64,
                    rec.started_at,
                    rec.closed_at
                ],
            )?;
        }
        // Keep history table bounded cleanly so disk footprint stays flat.
        let count: i64 =
            tx.query_row("SELECT COUNT(*) FROM connection_history", [], |r| r.get(0))?;
        if count > MAX_CONNECTION_HISTORY_ROWS {
            let excess = count - MAX_CONNECTION_HISTORY_ROWS;
            tx.execute(
                "DELETE FROM connection_history WHERE id IN (
                    SELECT id FROM connection_history ORDER BY closed_at ASC LIMIT ?1
                 )",
                params![excess],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn query_connection_history(
        &self,
        query: &str,
        page: usize,
        page_size: usize,
    ) -> Result<(Vec<ConnectionHistoryRecord>, usize)> {
        let safe_size = page_size.clamp(10, 200) as i64;
        let safe_page = page.max(1) as i64;
        let offset = (safe_page - 1) * safe_size;
        let q = query.trim().to_ascii_lowercase();

        let (total, rows_vec) = if q.is_empty() {
            let total: i64 = self.conn.query_row(
                "SELECT COUNT(*) FROM connection_history",
                [],
                |r| r.get(0),
            )?;
            let mut stmt = self.conn.prepare(
                "SELECT id, process, destination, host, platform, inbound, rule, chain_json,
                        upload_bytes, download_bytes, started_at, closed_at
                 FROM connection_history
                 ORDER BY closed_at DESC
                 LIMIT ?1 OFFSET ?2",
            )?;
            let mapped = stmt.query_map(params![safe_size, offset], map_history_row)?;
            let mut items = Vec::new();
            for r in mapped {
                items.push(r?);
            }
            (total as usize, items)
        } else {
            let like = format!("%{}%", q);
            let total: i64 = self.conn.query_row(
                "SELECT COUNT(*) FROM connection_history
                 WHERE LOWER(process) LIKE ?1
                    OR LOWER(destination) LIKE ?1
                    OR LOWER(platform) LIKE ?1
                    OR LOWER(rule) LIKE ?1
                    OR LOWER(chain_json) LIKE ?1",
                params![&like],
                |r| r.get(0),
            )?;
            let mut stmt = self.conn.prepare(
                "SELECT id, process, destination, host, platform, inbound, rule, chain_json,
                        upload_bytes, download_bytes, started_at, closed_at
                 FROM connection_history
                 WHERE LOWER(process) LIKE ?1
                    OR LOWER(destination) LIKE ?1
                    OR LOWER(platform) LIKE ?1
                    OR LOWER(rule) LIKE ?1
                    OR LOWER(chain_json) LIKE ?1
                 ORDER BY closed_at DESC
                 LIMIT ?2 OFFSET ?3",
            )?;
            let mapped = stmt.query_map(params![&like, safe_size, offset], map_history_row)?;
            let mut items = Vec::new();
            for r in mapped {
                items.push(r?);
            }
            (total as usize, items)
        };

        Ok((rows_vec, total))
    }

    pub fn query_platform_traffic(&self, since_iso: Option<&str>) -> Result<Vec<PlatformTrafficStat>> {
        let mut by_platform: HashMap<String, (u64, u64, u64, HashMap<String, u64>)> = HashMap::new();

        let sql = if since_iso.is_some() {
            "SELECT platform, host, upload_bytes, download_bytes
             FROM connection_history
             WHERE closed_at >= ?1"
        } else {
            "SELECT platform, host, upload_bytes, download_bytes
             FROM connection_history"
        };

        let mut stmt = self.conn.prepare(sql)?;
        let mut rows = if let Some(since) = since_iso {
            stmt.query(params![since])?
        } else {
            stmt.query([])?
        };

        while let Some(row) = rows.next()? {
            let platform: String = row.get(0)?;
            let host: String = row.get(1)?;
            let up = row.get::<_, i64>(2)?.max(0) as u64;
            let down = row.get::<_, i64>(3)?.max(0) as u64;
            let entry = by_platform
                .entry(platform)
                .or_insert_with(|| (0, 0, 0, HashMap::new()));
            entry.0 += up;
            entry.1 += down;
            entry.2 += 1;
            if !host.is_empty() {
                *entry.3.entry(host).or_insert(0) += up + down;
            }
        }

        let mut out: Vec<PlatformTrafficStat> = by_platform
            .into_iter()
            .map(|(platform, (up, down, count, hosts))| {
                let mut domain_list: Vec<(String, u64)> = hosts.into_iter().collect();
                domain_list.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
                let top_domains: Vec<String> = domain_list
                    .into_iter()
                    .take(3)
                    .map(|(d, _)| d)
                    .collect();
                PlatformTrafficStat {
                    platform,
                    upload_bytes: up,
                    download_bytes: down,
                    total_bytes: up + down,
                    connection_count: count,
                    active_count: 0,
                    top_domains,
                }
            })
            .collect();

        out.sort_by(|a, b| {
            b.total_bytes
                .cmp(&a.total_bytes)
                .then_with(|| b.connection_count.cmp(&a.connection_count))
                .then_with(|| a.platform.cmp(&b.platform))
        });
        Ok(out)
    }

    pub fn clear_connection_history(&self) -> Result<()> {
        self.conn.execute("DELETE FROM connection_history", [])?;
        Ok(())
    }
}

fn map_history_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<ConnectionHistoryRecord> {
    let chain_json: String = r.get(7)?;
    let chain: Vec<String> = serde_json::from_str(&chain_json).unwrap_or_default();
    Ok(ConnectionHistoryRecord {
        id: r.get(0)?,
        process: r.get(1)?,
        destination: r.get(2)?,
        host: r.get(3)?,
        platform: r.get(4)?,
        inbound: r.get(5)?,
        rule: r.get(6)?,
        chain,
        upload_bytes: r.get::<_, i64>(8)?.max(0) as u64,
        download_bytes: r.get::<_, i64>(9)?.max(0) as u64,
        started_at: r.get(10)?,
        closed_at: r.get(11)?,
    })
}

fn make_srs_rule(
    id: &str,
    title: &str,
    description: &str,
    rule_set: &str,
    target: &str,
    priority: i32,
) -> SimpleRoutingRule {
    SimpleRoutingRule {
        id: id.to_string(),
        title: title.to_string(),
        title_en: title.to_string(),
        description: description.to_string(),
        patterns: Vec::new(),
        domain_keywords: Vec::new(),
        ip_cidrs: Vec::new(),
        process_names: Vec::new(),
        rule_set: Some(rule_set.to_string()),
        target: target.to_string(),
        enabled: true,
        priority,
    }
}

/// Community classic geosite / geoip routing strategies:
/// - `bypass-cn` (default): Bypass Mainland China whitelist (ads reject, private direct, geosite-cn direct, geoip-cn direct, final -> main)
/// - `gfw-list`: GFW + non-CN proxy list + CN site/ip direct
/// - `bypass-cn-split`: Bypass Mainland China + dedicated AI / Streaming / Telegram site & IP splits
pub fn classic_strategy_rules(strategy_id: &str) -> Vec<SimpleRoutingRule> {
    match strategy_id {
        "gfw-list" => vec![
            make_srs_rule(
                "classic-ads",
                "Ads & Trackers (Geosite)",
                "geosite-category-ads-all",
                "geosite-category-ads-all",
                "reject",
                10,
            ),
            make_srs_rule(
                "classic-private",
                "Private & LAN (Geosite)",
                "geosite-private",
                "geosite-private",
                "direct",
                20,
            ),
            make_srs_rule(
                "classic-gfw",
                "GFW Proxy List (Geosite)",
                "geosite-gfw",
                "geosite-gfw",
                "main",
                30,
            ),
            make_srs_rule(
                "classic-geolocation-nocn",
                "Non-CN Domains (Geosite)",
                "geosite-geolocation-!cn",
                "geosite-geolocation-!cn",
                "main",
                40,
            ),
            make_srs_rule(
                "classic-geosite-cn",
                "China Mainland Domains (Geosite)",
                "geosite-cn",
                "geosite-cn",
                "direct",
                50,
            ),
            make_srs_rule(
                "classic-geoip-cn",
                "China Mainland IPs (GeoIP)",
                "geoip-cn",
                "geoip-cn",
                "direct",
                60,
            ),
        ],
        "bypass-cn-split" => vec![
            make_srs_rule(
                "classic-ads",
                "Ads & Trackers (Geosite)",
                "geosite-category-ads-all",
                "geosite-category-ads-all",
                "reject",
                10,
            ),
            make_srs_rule(
                "classic-private",
                "Private & LAN (Geosite)",
                "geosite-private",
                "geosite-private",
                "direct",
                20,
            ),
            make_srs_rule(
                "classic-ai",
                "AI Platforms (Geosite)",
                "geosite-category-ai-!cn",
                "geosite-category-ai-!cn",
                "auto:ALL",
                25,
            ),
            make_srs_rule(
                "classic-netflix",
                "Netflix (Geosite)",
                "geosite-netflix",
                "geosite-netflix",
                "auto:ALL",
                28,
            ),
            make_srs_rule(
                "classic-youtube",
                "YouTube (Geosite)",
                "geosite-youtube",
                "geosite-youtube",
                "main",
                30,
            ),
            make_srs_rule(
                "classic-telegram-site",
                "Telegram Domains (Geosite)",
                "geosite-telegram",
                "geosite-telegram",
                "main",
                32,
            ),
            make_srs_rule(
                "classic-telegram-ip",
                "Telegram IPs (GeoIP)",
                "geoip-telegram",
                "geoip-telegram",
                "main",
                34,
            ),
            make_srs_rule(
                "classic-geosite-cn",
                "China Mainland Domains (Geosite)",
                "geosite-cn",
                "geosite-cn",
                "direct",
                40,
            ),
            make_srs_rule(
                "classic-geoip-cn",
                "China Mainland IPs (GeoIP)",
                "geoip-cn",
                "geoip-cn",
                "direct",
                50,
            ),
        ],
        _ => vec![
            make_srs_rule(
                "classic-ads",
                "Ads & Trackers (Geosite)",
                "geosite-category-ads-all",
                "geosite-category-ads-all",
                "reject",
                10,
            ),
            make_srs_rule(
                "classic-private",
                "Private & LAN (Geosite)",
                "geosite-private",
                "geosite-private",
                "direct",
                20,
            ),
            make_srs_rule(
                "classic-geosite-cn",
                "China Mainland Domains (Geosite)",
                "geosite-cn",
                "geosite-cn",
                "direct",
                30,
            ),
            make_srs_rule(
                "classic-geoip-cn",
                "China Mainland IPs (GeoIP)",
                "geoip-cn",
                "geoip-cn",
                "direct",
                40,
            ),
        ],
    }
}

pub fn default_simple_rules() -> Vec<SimpleRoutingRule> {
    classic_strategy_rules("bypass-cn")
}

/// Curated catalog of official SagerNet sing-geosite / sing-geoip binary rule-sets.
pub fn rule_preset_catalog() -> Vec<RulePresetSpec> {
    let mk = |id: &str, cat: &str, title: &str, desc: &str, rs: &str, target: &str, prio: i32| {
        RulePresetSpec {
            id: id.to_string(),
            category: cat.to_string(),
            title: title.to_string(),
            title_en: title.to_string(),
            description: desc.to_string(),
            patterns: Vec::new(),
            process_names: Vec::new(),
            rule_set: Some(rs.to_string()),
            default_target: target.to_string(),
            default_priority: prio,
        }
    };
    vec![
        mk("classic-ads", "block", "Ads & Trackers (Geosite)", "geosite-category-ads-all", "geosite-category-ads-all", "reject", 10),
        mk("classic-private", "direct", "Private & LAN (Geosite)", "geosite-private", "geosite-private", "direct", 20),
        mk("classic-geosite-cn", "direct", "China Mainland Domains (Geosite)", "geosite-cn", "geosite-cn", "direct", 30),
        mk("classic-geoip-cn", "direct", "China Mainland IPs (GeoIP)", "geoip-cn", "geoip-cn", "direct", 40),
        mk("classic-gfw", "proxy", "GFW Proxy List (Geosite)", "geosite-gfw", "geosite-gfw", "main", 30),
        mk("classic-geolocation-nocn", "proxy", "Non-CN Domains (Geosite)", "geosite-geolocation-!cn", "geosite-geolocation-!cn", "main", 35),
        mk("classic-ai", "ai", "AI Platforms (Geosite)", "geosite-category-ai-!cn", "geosite-category-ai-!cn", "auto:ALL", 25),
        mk("r-openai", "ai", "OpenAI / ChatGPT", "geosite-openai", "geosite-openai", "auto:ALL", 26),
        mk("r-anthropic", "ai", "Claude / Anthropic", "geosite-anthropic", "geosite-anthropic", "auto:ALL", 27),
        mk("classic-youtube", "media", "YouTube (Geosite)", "geosite-youtube", "geosite-youtube", "main", 28),
        mk("classic-netflix", "media", "Netflix (Geosite)", "geosite-netflix", "geosite-netflix", "auto:ALL", 28),
        mk("r-netflix-ip", "media", "Netflix IPs (GeoIP)", "geoip-netflix", "geoip-netflix", "auto:ALL", 29),
        mk("r-disney", "media", "Disney+", "geosite-disney", "geosite-disney", "auto:ALL", 29),
        mk("r-spotify", "media", "Spotify", "geosite-spotify", "geosite-spotify", "main", 30),
        mk("r-twitch", "media", "Twitch", "geosite-twitch", "geosite-twitch", "main", 30),
        mk("r-tiktok", "media", "TikTok", "geosite-tiktok", "geosite-tiktok", "auto:ALL", 30),
        mk("r-bahamut", "media", "Bahamut", "geosite-bahamut", "geosite-bahamut", "auto:TW", 30),
        mk("r-bilibili", "media", "Bilibili", "geosite-bilibili", "geosite-bilibili", "direct", 31),
        mk("classic-telegram-site", "social", "Telegram Domains (Geosite)", "geosite-telegram", "geosite-telegram", "main", 32),
        mk("classic-telegram-ip", "social", "Telegram IPs (GeoIP)", "geoip-telegram", "geoip-telegram", "main", 33),
        mk("r-twitter", "social", "X / Twitter", "geosite-twitter", "geosite-twitter", "main", 33),
        mk("r-discord", "social", "Discord", "geosite-discord", "geosite-discord", "main", 34),
        mk("r-reddit", "social", "Reddit", "geosite-reddit", "geosite-reddit", "main", 34),
        mk("r-dev", "dev", "GitHub (Geosite)", "geosite-github", "geosite-github", "main", 35),
        mk("r-scholar", "dev", "Academic & Scholar", "geosite-category-scholar-!cn", "geosite-category-scholar-!cn", "main", 36),
        mk("r-google", "tech", "Google Domains (Geosite)", "geosite-google", "geosite-google", "main", 36),
        mk("r-google-ip", "tech", "Google IPs (GeoIP)", "geoip-google", "geoip-google", "main", 37),
        mk("r-apple", "tech", "Apple & iCloud", "geosite-apple", "geosite-apple", "direct", 37),
        mk("r-microsoft", "tech", "Microsoft Services", "geosite-microsoft", "geosite-microsoft", "direct", 38),
        mk("r-cloudflare-ip", "tech", "Cloudflare IPs (GeoIP)", "geoip-cloudflare", "geoip-cloudflare", "main", 38),
        mk("r-steam", "gaming", "Steam", "geosite-steam", "geosite-steam", "main", 39),
        mk("r-paypal", "tech", "PayPal", "geosite-paypal", "geosite-paypal", "main", 39),
    ]
}
