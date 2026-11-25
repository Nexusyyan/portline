use rand::RngCore;
use serde::{Deserialize, Serialize};

/// Windows Named Pipe endpoint protected by local-interactive SDDL ACL.
pub const WINDOWS_PIPE_NAME: &str = r"\\.\pipe\portline-svc-v1";

/// Unix Domain Socket endpoint for macOS/Linux privileged helper.
pub const UNIX_SOCKET_PATH: &str = "/var/run/portline-svc.sock";

/// Explicit SDDL granting Full Control to LocalSystem (SY) and Builtin Administrators (BA),
/// and Read/Write only to Interactive Users (IU) -- blocking remote/anonymous network callers.
pub const WINDOWS_PIPE_SDDL: &str = "D:(A;;GA;;;SY)(A;;GA;;;BA)(A;;GRGW;;;IU)";

/// Default GUI heartbeat watchdog timeout (seconds) before the service triggers NetGuard rollback.
pub const HEARTBEAT_TIMEOUT_SECS: u64 = 15;

pub fn generate_session_token() -> String {
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    hex::encode(bytes)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IpcEnvelope {
    pub id: String,
    pub session_token: String,
    pub command: ServiceCommand,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "method", content = "params", rename_all = "snake_case")]
pub enum ServiceCommand {
    Ping,
    Heartbeat { gui_pid: u32 },
    StartCore {
        sing_box_path: String,
        config_json: String,
    },
    StopCore,
    Status,
    RestoreNetwork,
    Shutdown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IpcResponse {
    pub id: String,
    pub ok: bool,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub core_running: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub core_pid: Option<u32>,
}

impl IpcResponse {
    pub fn success(id: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            ok: true,
            message: message.into(),
            core_running: None,
            core_pid: None,
        }
    }

    pub fn error(id: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            ok: false,
            message: message.into(),
            core_running: None,
            core_pid: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_ipc_envelope_and_token_entropy() {
        let t1 = generate_session_token();
        let t2 = generate_session_token();
        assert_eq!(t1.len(), 64);
        assert_ne!(t1, t2);

        let env = IpcEnvelope {
            id: "req-1".to_string(),
            session_token: t1.clone(),
            command: ServiceCommand::Heartbeat { gui_pid: 4242 },
        };
        let json = serde_json::to_string(&env).expect("serialize");
        let parsed: IpcEnvelope = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, env);
        assert!(WINDOWS_PIPE_SDDL.contains("SY") && WINDOWS_PIPE_SDDL.contains("IU"));
    }
}
