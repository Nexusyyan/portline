use portline_core::compiler::{CompileInput, SingBox114Compiler};
use portline_core::core_manager::CoreManager;
use portline_core::model::{
    CompileOutput, ConnectionHistoryRecord, ConnectionItem, CoreInfo, PreflightReport,
};
use portline_core::netguard::NetGuard;
use portline_core::rule_cache::RuleCache;
use portline_core::storage::{classify_platform, extract_host, DataPaths, Store};
use portline_core::supervisor::{CoreSupervisor, LogRing, MAX_LOG_RING_ENTRIES};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::Instant;

pub struct TrafficCursor {
    pub last_up: u64,
    pub last_down: u64,
    pub last_at: Instant,
}

impl Default for TrafficCursor {
    fn default() -> Self {
        Self {
            last_up: 0,
            last_down: 0,
            last_at: Instant::now(),
        }
    }
}

#[derive(Default)]
pub struct ConnectionTracker {
    active: HashMap<String, ConnectionItem>,
}

impl ConnectionTracker {
    /// Updates active connection map from a live Clash API snapshot and returns
    /// records for connections that just closed so they can be persisted to SQLite.
    pub fn sync_snapshot(&mut self, live: &[ConnectionItem]) -> Vec<ConnectionHistoryRecord> {
        let mut live_ids: HashSet<&str> = HashSet::with_capacity(live.len());
        for item in live {
            if item.id.is_empty() {
                continue;
            }
            live_ids.insert(item.id.as_str());
            self.active.insert(item.id.clone(), item.clone());
        }

        let closed_ids: Vec<String> = self
            .active
            .keys()
            .filter(|id| !live_ids.contains(id.as_str()))
            .cloned()
            .collect();

        let now = chrono::Utc::now().to_rfc3339();
        let mut closed = Vec::with_capacity(closed_ids.len());
        for id in closed_ids {
            if let Some(item) = self.active.remove(&id) {
                closed.push(to_history_record(item, &now));
            }
        }
        closed
    }

    pub fn drain_all(&mut self) -> Vec<ConnectionHistoryRecord> {
        let now = chrono::Utc::now().to_rfc3339();
        self.active
            .drain()
            .map(|(_, item)| to_history_record(item, &now))
            .collect()
    }

    pub fn snapshot_active(&self) -> Vec<ConnectionItem> {
        self.active.values().cloned().collect()
    }
}

fn to_history_record(item: ConnectionItem, closed_at: &str) -> ConnectionHistoryRecord {
    let host = extract_host(&item.destination);
    let platform = classify_platform(&host, &item.process);
    ConnectionHistoryRecord {
        id: item.id,
        process: item.process,
        destination: item.destination,
        host,
        platform,
        inbound: item.inbound,
        rule: item.rule,
        chain: item.chain,
        upload_bytes: item.upload_bytes,
        download_bytes: item.download_bytes,
        started_at: if item.start.is_empty() {
            closed_at.to_string()
        } else {
            item.start
        },
        closed_at: closed_at.to_string(),
    }
}

pub struct AppState {
    pub paths: DataPaths,
    pub store: Mutex<Store>,
    pub netguard: Mutex<NetGuard>,
    pub rule_cache: RuleCache,
    pub core_manager: CoreManager,
    pub supervisor: Mutex<CoreSupervisor>,
    pub logs: Arc<Mutex<LogRing>>,
    pub control_secret: String,
    pub recovered_from_crash: bool,
    pub traffic_cursor: Mutex<TrafficCursor>,
    pub conn_tracker: Mutex<ConnectionTracker>,
}

impl AppState {
    pub fn initialize() -> anyhow::Result<Self> {
        let paths = DataPaths::resolve()?;
        let store = Store::open(&paths.db_path)?;
        let netguard = NetGuard::new(&paths.runtime_dir);
        let recovered_from_crash = netguard.recover_if_dirty().unwrap_or(false);

        let logs = Arc::new(Mutex::new(LogRing::new(MAX_LOG_RING_ENTRIES)));
        if let Ok(mut guard) = logs.lock() {
            guard.push(
                "INFO",
                format!(
                    "portline initialized (portable={}, data_dir={})",
                    paths.portable_mode,
                    paths.root_dir.display()
                ),
            );
            if recovered_from_crash {
                guard.push(
                    "WARN",
                    "netguard: detected unclean exit snapshot and restored OS proxy/DNS cleanly",
                );
            }
        }

        let rule_cache = RuleCache::new(paths.rule_sets_dir.clone());
        let core_manager = CoreManager::new(paths.core_dir.clone());
        let supervisor = CoreSupervisor::new(paths.runtime_dir.clone(), Arc::clone(&logs));
        let control_secret = portline_ipc::generate_session_token();

        Ok(Self {
            paths,
            store: Mutex::new(store),
            netguard: Mutex::new(netguard),
            rule_cache,
            core_manager,
            supervisor: Mutex::new(supervisor),
            logs,
            control_secret,
            recovered_from_crash,
            traffic_cursor: Mutex::new(TrafficCursor::default()),
            conn_tracker: Mutex::new(ConnectionTracker::default()),
        })
    }

    pub fn push_log(&self, level: &str, msg: impl Into<String>) {
        if let Ok(mut guard) = self.logs.lock() {
            guard.push(level, msg);
        }
    }

    pub fn record_connections_snapshot(&self, conns: &[ConnectionItem]) {
        let closed = if let Ok(mut tracker) = self.conn_tracker.lock() {
            tracker.sync_snapshot(conns)
        } else {
            Vec::new()
        };
        if !closed.is_empty() {
            if let Ok(mut store) = self.store.lock() {
                let _ = store.insert_connection_history(&closed);
            }
        }
    }

    pub fn flush_active_connections(&self) {
        let flushed = if let Ok(mut tracker) = self.conn_tracker.lock() {
            tracker.drain_all()
        } else {
            Vec::new()
        };
        if !flushed.is_empty() {
            if let Ok(mut store) = self.store.lock() {
                let _ = store.insert_connection_history(&flushed);
            }
        }
    }

    pub fn compile_current(&self) -> Result<CompileOutput, String> {
        let store = self.store.lock().map_err(|_| "store lock poisoned".to_string())?;
        let nodes = store.list_nodes().map_err(|e| e.to_string())?;
        let rules = store.list_rules().map_err(|e| e.to_string())?;
        let extra_ports = store.list_inbound_ports().map_err(|e| e.to_string())?;
        let settings = store.load_settings().map_err(|e| e.to_string())?;
        let cache_db = self.paths.runtime_dir.join("cache.db");

        SingBox114Compiler::compile(&CompileInput {
            nodes: &nodes,
            rules: &rules,
            extra_ports: &extra_ports,
            settings: &settings,
            control_secret: &self.control_secret,
            cache_db_path: Some(&cache_db),
            rule_cache: Some(&self.rule_cache),
        })
        .map_err(|e| e.to_string())
    }

    pub fn build_core_info(&self) -> CoreInfo {
        let installed = self.core_manager.is_installed();
        let version = self.core_manager.detect_version();
        let sha256 = self.core_manager.sha256_of_binary().ok().flatten();
        let running = self
            .supervisor
            .lock()
            .map(|mut s| s.is_running())
            .unwrap_or(false);
        let netguard_armed = self
            .netguard
            .lock()
            .map(|g| g.is_armed())
            .unwrap_or(false);

        let bin_path = self
            .core_manager
            .resolve_binary_path()
            .unwrap_or_else(|| self.core_manager.binary_path());

        CoreInfo {
            installed,
            running,
            version,
            binary_path: bin_path.to_string_lossy().to_string(),
            sha256,
            portable_mode: self.paths.portable_mode,
            data_dir: self.paths.root_dir.to_string_lossy().to_string(),
            netguard_armed,
            netguard_recovered_from_crash: self.recovered_from_crash,
        }
    }

    /// Resolves an installed sing-box binary or returns a structured `CORE_NOT_INSTALLED` error
    /// immediately (never silently hanging on blocked GitHub networks during start or latency test).
    pub fn require_core_binary(&self) -> Result<std::path::PathBuf, String> {
        self.core_manager
            .resolve_binary_path()
            .ok_or_else(|| "CORE_NOT_INSTALLED".to_string())
    }

    pub fn reload_core_if_running(&self) -> Result<Option<PreflightReport>, String> {
        let running = self
            .supervisor
            .lock()
            .map(|mut s| s.is_running())
            .unwrap_or(false);
        let Some(bin_path) = self.core_manager.resolve_binary_path() else {
            return Ok(None);
        };
        if !running {
            return Ok(None);
        }
        let compiled = self.compile_current()?;
        let mut sup = self
            .supervisor
            .lock()
            .map_err(|_| "supervisor lock poisoned".to_string())?;
        let report = match sup.apply_config(&bin_path, compiled.config) {
            Ok(r) => r,
            Err(e) => {
                let msg = format!("reload failed: {}", e);
                drop(sup);
                self.push_log("ERROR", &msg);
                return Err(msg);
            }
        };
        if !report.ok {
            drop(sup);
            self.push_log(
                "ERROR",
                format!("reload blocked by preflight (exit {})", report.exit_code),
            );
        }
        Ok(Some(report))
    }

    pub fn shutdown_cleanly(&self) {
        self.flush_active_connections();
        if let Ok(mut sup) = self.supervisor.lock() {
            sup.stop();
        }
        if let Ok(guard) = self.netguard.lock() {
            if guard.is_armed() {
                let _ = guard.disable_system_proxy();
            }
        }
    }
}
