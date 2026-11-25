use portline_core::compiler::{CompileInput, SingBox114Compiler};
use portline_core::core_manager::CoreManager;
use portline_core::model::{CompileOutput, CoreInfo, PreflightReport};
use portline_core::netguard::NetGuard;
use portline_core::rule_cache::RuleCache;
use portline_core::storage::{DataPaths, Store};
use portline_core::supervisor::{CoreSupervisor, LogRing, MAX_LOG_RING_ENTRIES};
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
        })
    }

    pub fn push_log(&self, level: &str, msg: impl Into<String>) {
        if let Ok(mut guard) = self.logs.lock() {
            guard.push(level, msg);
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

        CoreInfo {
            installed,
            running,
            version,
            binary_path: self.core_manager.binary_path().to_string_lossy().to_string(),
            sha256,
            portable_mode: self.paths.portable_mode,
            data_dir: self.paths.root_dir.to_string_lossy().to_string(),
            netguard_armed,
            netguard_recovered_from_crash: self.recovered_from_crash,
        }
    }

    pub fn reload_core_if_running(&self) -> Result<Option<PreflightReport>, String> {
        let running = self
            .supervisor
            .lock()
            .map(|mut s| s.is_running())
            .unwrap_or(false);
        if !running || !self.core_manager.is_installed() {
            return Ok(None);
        }
        let compiled = self.compile_current()?;
        let mut sup = self
            .supervisor
            .lock()
            .map_err(|_| "supervisor lock poisoned".to_string())?;
        let report = sup
            .apply_config(&self.core_manager.binary_path(), compiled.config)
            .map_err(|e| e.to_string())?;
        Ok(Some(report))
    }

    pub fn shutdown_cleanly(&self) {
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
