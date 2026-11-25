use crate::model::{
    AppSettings, InboundPort, NetworkSnapshot, NodeIR, SimpleRoutingRule, SubscriptionGroup,
};
use anyhow::{Context, Result};
use rusqlite::{params, Connection, OptionalExtension};
use std::path::{Path, PathBuf};

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
            ",
        )?;
        Ok(())
    }

    fn seed_defaults_if_empty(&self) -> Result<()> {
        let rule_count: i64 =
            self.conn
                .query_row("SELECT COUNT(*) FROM routing_rules", [], |r| r.get(0))?;
        if rule_count == 0 {
            for rule in default_simple_rules() {
                self.upsert_rule(&rule)?;
            }
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
            if let Ok(node) = serde_json::from_str::<NodeIR>(&row?) {
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
}

pub fn default_simple_rules() -> Vec<SimpleRoutingRule> {
    vec![
        SimpleRoutingRule {
            id: "r-ai".to_string(),
            title: "AI Platforms".to_string(),
            title_en: "AI Platforms".to_string(),
            description: "ChatGPT, Claude, Gemini, OpenAI API".to_string(),
            patterns: vec![
                "openai.com".to_string(),
                "chatgpt.com".to_string(),
                "claude.ai".to_string(),
                "anthropic.com".to_string(),
                "gemini.google.com".to_string(),
            ],
            process_names: Vec::new(),
            rule_set: Some("geosite-openai".to_string()),
            target: "auto:US".to_string(),
            enabled: true,
            priority: 10,
        },
        SimpleRoutingRule {
            id: "r-media".to_string(),
            title: "Streaming Media".to_string(),
            title_en: "Streaming Media".to_string(),
            description: "Netflix, Disney+, Hulu".to_string(),
            patterns: vec![
                "netflix.com".to_string(),
                "nflxvideo.net".to_string(),
                "disneyplus.com".to_string(),
                "hulu.com".to_string(),
            ],
            process_names: Vec::new(),
            rule_set: Some("geosite-netflix".to_string()),
            target: "auto:ALL".to_string(),
            enabled: true,
            priority: 20,
        },
        SimpleRoutingRule {
            id: "r-dev".to_string(),
            title: "Dev & Code".to_string(),
            title_en: "Dev & Code".to_string(),
            description: "GitHub, GitLab, npmjs, crates.io".to_string(),
            patterns: vec![
                "github.com".to_string(),
                "githubusercontent.com".to_string(),
                "gitlab.com".to_string(),
                "npmjs.org".to_string(),
                "crates.io".to_string(),
            ],
            process_names: Vec::new(),
            rule_set: None,
            target: "main".to_string(),
            enabled: true,
            priority: 30,
        },
        SimpleRoutingRule {
            id: "r-cn".to_string(),
            title: "China Direct & LAN".to_string(),
            title_en: "China Direct & LAN".to_string(),
            description: "baidu.com, qq.com, weixin.qq.com, bilibili.com, .cn".to_string(),
            patterns: vec![
                "baidu.com".to_string(),
                "qq.com".to_string(),
                "bilibili.com".to_string(),
                "taobao.com".to_string(),
                "cn".to_string(),
            ],
            process_names: Vec::new(),
            rule_set: Some("geosite-cn".to_string()),
            target: "direct".to_string(),
            enabled: true,
            priority: 40,
        },
        SimpleRoutingRule {
            id: "r-ads".to_string(),
            title: "Ads & Trackers".to_string(),
            title_en: "Ads & Trackers".to_string(),
            description: "doubleclick.net, googlesyndication.com, adservice.google.com".to_string(),
            patterns: vec![
                "doubleclick.net".to_string(),
                "googlesyndication.com".to_string(),
                "adservice.google.com".to_string(),
            ],
            process_names: Vec::new(),
            rule_set: Some("geosite-category-ads-all".to_string()),
            target: "reject".to_string(),
            enabled: true,
            priority: 50,
        },
    ]
}
