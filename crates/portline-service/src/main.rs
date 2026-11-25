use anyhow::Result;
use portline_core::netguard::NetGuard;
use portline_core::storage::DataPaths;
use portline_core::supervisor::{CoreSupervisor, LogRing, MAX_LOG_RING_ENTRIES};
use portline_ipc::{IpcEnvelope, IpcResponse, ServiceCommand, HEARTBEAT_TIMEOUT_SECS};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

struct ServiceState {
    expected_token: String,
    supervisor: CoreSupervisor,
    netguard: NetGuard,
    last_heartbeat: Option<Instant>,
}

impl ServiceState {
    fn handle(&mut self, env: IpcEnvelope) -> (IpcResponse, bool) {
        if env.session_token != self.expected_token {
            return (IpcResponse::error(env.id, "Unauthorized session token"), false);
        }
        match env.command {
            ServiceCommand::Ping => (IpcResponse::success(env.id, "pong"), false),
            ServiceCommand::Heartbeat { gui_pid } => {
                self.last_heartbeat = Some(Instant::now());
                (
                    IpcResponse::success(env.id, format!("heartbeat ok (pid {})", gui_pid)),
                    false,
                )
            }
            ServiceCommand::StartCore {
                sing_box_path,
                config_json,
            } => {
                let parsed: serde_json::Value = match serde_json::from_str(&config_json) {
                    Ok(v) => v,
                    Err(e) => return (IpcResponse::error(env.id, format!("Invalid JSON: {}", e)), false),
                };
                match self
                    .supervisor
                    .apply_config(&PathBuf::from(sing_box_path), parsed)
                {
                    Ok(rep) if rep.ok => {
                        let pid = self.supervisor.require_running_pid().ok();
                        let mut resp = IpcResponse::success(env.id, "Core started");
                        resp.core_running = Some(true);
                        resp.core_pid = pid;
                        (resp, false)
                    }
                    Ok(rep) => (
                        IpcResponse::error(env.id, format!("Preflight failed: {}", rep.stderr)),
                        false,
                    ),
                    Err(e) => (IpcResponse::error(env.id, format!("Start error: {}", e)), false),
                }
            }
            ServiceCommand::StopCore => {
                self.supervisor.stop();
                let mut resp = IpcResponse::success(env.id, "Core stopped");
                resp.core_running = Some(false);
                (resp, false)
            }
            ServiceCommand::Status => {
                let running = self.supervisor.is_running();
                let pid = if running {
                    self.supervisor.require_running_pid().ok()
                } else {
                    None
                };
                let mut resp = IpcResponse::success(env.id, "ok");
                resp.core_running = Some(running);
                resp.core_pid = pid;
                (resp, false)
            }
            ServiceCommand::RestoreNetwork => match self.netguard.disable_system_proxy() {
                Ok(_) => (IpcResponse::success(env.id, "Network restored"), false),
                Err(e) => (IpcResponse::error(env.id, format!("Restore failed: {}", e)), false),
            },
            ServiceCommand::Shutdown => {
                self.supervisor.stop();
                let _ = self.netguard.recover_if_dirty();
                (IpcResponse::success(env.id, "Shutting down"), true)
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let paths = DataPaths::resolve()?;
    let logs = Arc::new(Mutex::new(LogRing::new(MAX_LOG_RING_ENTRIES)));
    let token = std::env::var("PORTLINE_IPC_TOKEN")
        .unwrap_or_else(|_| portline_ipc::generate_session_token());

    let state = Arc::new(Mutex::new(ServiceState {
        expected_token: token,
        supervisor: CoreSupervisor::new(paths.runtime_dir.clone(), logs),
        netguard: NetGuard::new(&paths.runtime_dir),
        last_heartbeat: None,
    }));

    // Background GUI heartbeat watchdog: if GUI stops heartbeating while core is active,
    // cleanly stop the core and rollback NetGuard so the machine is never left offline.
    let watchdog_state = Arc::clone(&state);
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(3));
        loop {
            interval.tick().await;
            if let Ok(mut guard) = watchdog_state.lock() {
                if let Some(last) = guard.last_heartbeat {
                    if last.elapsed() > Duration::from_secs(HEARTBEAT_TIMEOUT_SECS)
                        && guard.supervisor.is_running()
                    {
                        tracing::warn!("GUI heartbeat lost; stopping core and restoring NetGuard");
                        guard.supervisor.stop();
                        let _ = guard.netguard.recover_if_dirty();
                        guard.last_heartbeat = None;
                    }
                }
            }
        }
    });

    // Listen on local IPC transport (Windows Named Pipe or Unix Domain Socket).
    run_ipc_server(state).await
}

async fn run_ipc_server(state: Arc<Mutex<ServiceState>>) -> Result<()> {
    use interprocess::local_socket::tokio::prelude::*;
    use interprocess::local_socket::ListenerOptions;

    #[cfg(windows)]
    let name = {
        use interprocess::local_socket::GenericNamespaced;
        "portline-svc-v1".to_ns_name::<GenericNamespaced>()?
    };
    #[cfg(not(windows))]
    let name = {
        use interprocess::local_socket::GenericFilePath;
        let _ = std::fs::remove_file(portline_ipc::UNIX_SOCKET_PATH);
        portline_ipc::UNIX_SOCKET_PATH.to_fs_name::<GenericFilePath>()?
    };

    let listener = ListenerOptions::new().name(name).create_tokio()?;
    tracing::info!("portline-service IPC listener ready");

    loop {
        let stream = listener.accept().await?;
        let state_ref = Arc::clone(&state);
        tokio::spawn(async move {
            let (reader, mut writer) = tokio::io::split(stream);
            let mut lines = BufReader::new(reader).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                if line.trim().is_empty() {
                    continue;
                }
                let (resp, should_exit) = match serde_json::from_str::<IpcEnvelope>(&line) {
                    Ok(env) => match state_ref.lock() {
                        Ok(mut guard) => guard.handle(env),
                        Err(_) => (IpcResponse::error("lock", "State lock poisoned"), false),
                    },
                    Err(e) => (IpcResponse::error("parse", format!("Bad envelope: {}", e)), false),
                };
                if let Ok(encoded) = serde_json::to_string(&resp) {
                    let _ = writer.write_all(encoded.as_bytes()).await;
                    let _ = writer.write_all(b"\n").await;
                    let _ = writer.flush().await;
                }
                if should_exit {
                    std::process::exit(0);
                }
            }
        });
    }
}
