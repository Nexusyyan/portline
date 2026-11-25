use crate::compiler::SingBox114Compiler;
use crate::model::{LogEntry, PreflightReport};
use anyhow::{anyhow, Context, Result};
use serde_json::Value;
use std::collections::VecDeque;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};

pub const MAX_LOG_RING_ENTRIES: usize = 500;

/// Bounded ring buffer keeping long-running desktop memory strictly flat.
#[derive(Debug, Default)]
pub struct LogRing {
    entries: VecDeque<LogEntry>,
    cap: usize,
}

impl LogRing {
    pub fn new(cap: usize) -> Self {
        Self {
            entries: VecDeque::with_capacity(cap.max(1)),
            cap: cap.max(1),
        }
    }

    pub fn push(&mut self, level: impl Into<String>, message: impl Into<String>) {
        if self.entries.len() >= self.cap {
            self.entries.pop_back();
        }
        self.entries.push_front(LogEntry {
            timestamp: chrono::Local::now().format("%H:%M:%S").to_string(),
            level: level.into(),
            message: message.into(),
        });
    }

    pub fn snapshot(&self) -> Vec<LogEntry> {
        self.entries.iter().cloned().collect()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

/// Owns a spawned `sing-box` child process and ensures clean termination on drop.
/// On Windows, binds the child to a Job Object with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`
/// so crashing or killing Portline never leaves an orphaned `sing-box.exe` behind.
pub struct ManagedProcess {
    child: Child,
    #[cfg(windows)]
    job: Option<WindowsJobGuard>,
}

impl ManagedProcess {
    pub fn pid(&self) -> u32 {
        self.child.id()
    }

    pub fn is_running(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }

    pub fn terminate(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        #[cfg(windows)]
        {
            self.job = None;
        }
    }
}

impl Drop for ManagedProcess {
    fn drop(&mut self) {
        self.terminate();
    }
}

#[cfg(windows)]
struct WindowsJobGuard(windows::Win32::Foundation::HANDLE);

#[cfg(windows)]
unsafe impl Send for WindowsJobGuard {}
#[cfg(windows)]
unsafe impl Sync for WindowsJobGuard {}

#[cfg(windows)]
impl Drop for WindowsJobGuard {
    fn drop(&mut self) {
        unsafe {
            let _ = windows::Win32::Foundation::CloseHandle(self.0);
        }
    }
}

#[cfg(windows)]
fn attach_kill_on_close_job(child: &Child) -> Option<WindowsJobGuard> {
    use std::os::windows::io::AsRawHandle;
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
        SetInformationJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
        JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    };

    unsafe {
        let job = CreateJobObjectW(None, None).ok()?;
        let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        let ok = SetInformationJobObject(
            job,
            JobObjectExtendedLimitInformation,
            &info as *const _ as *const std::ffi::c_void,
            std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
        );
        if ok.is_err() {
            let _ = windows::Win32::Foundation::CloseHandle(job);
            return None;
        }
        let proc_handle = HANDLE(child.as_raw_handle());
        if AssignProcessToJobObject(job, proc_handle).is_err() {
            let _ = windows::Win32::Foundation::CloseHandle(job);
            return None;
        }
        Some(WindowsJobGuard(job))
    }
}

pub fn spawn_managed_process(sing_box_path: &Path, config_path: &Path) -> Result<ManagedProcess> {
    let mut cmd = Command::new(sing_box_path);
    cmd.arg("run")
        .arg("-c")
        .arg(config_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let child = cmd
        .spawn()
        .with_context(|| format!("Failed to spawn {}", sing_box_path.display()))?;
    #[cfg(windows)]
    let job = attach_kill_on_close_job(&child);
    Ok(ManagedProcess {
        child,
        #[cfg(windows)]
        job,
    })
}

/// Supervises the active runtime `sing-box` instance with preflight-before-swap rollback.
pub struct CoreSupervisor {
    runtime_dir: PathBuf,
    process: Option<ManagedProcess>,
    active_config: Option<Value>,
    logs: Arc<Mutex<LogRing>>,
}

impl CoreSupervisor {
    pub fn new(runtime_dir: PathBuf, logs: Arc<Mutex<LogRing>>) -> Self {
        let _ = std::fs::create_dir_all(&runtime_dir);
        Self {
            runtime_dir,
            process: None,
            active_config: None,
            logs,
        }
    }

    pub fn is_running(&mut self) -> bool {
        match self.process.as_mut() {
            Some(p) => p.is_running(),
            None => false,
        }
    }

    pub fn active_config(&self) -> Option<&Value> {
        self.active_config.as_ref()
    }

    /// Runs `sing-box check -c` first. If preflight fails, the currently running `sing-box`
    /// process is NEVER stopped or interrupted, preserving active connections cleanly.
    pub fn apply_config(
        &mut self,
        sing_box_path: &Path,
        candidate_config: Value,
    ) -> Result<PreflightReport> {
        let report = SingBox114Compiler::preflight_check(sing_box_path, &candidate_config)?;
        if !report.ok {
            self.push_log(
                "ERROR",
                format!(
                    "preflight blocked bad config (exit {}): {}",
                    report.exit_code,
                    first_line(&report.stderr)
                ),
            );
            return Ok(report);
        }

        // Write verified candidate atomically to runtime.json.
        std::fs::create_dir_all(&self.runtime_dir)?;
        let cfg_path = self.runtime_dir.join("runtime.json");
        let tmp_path = self.runtime_dir.join("runtime.json.tmp");
        std::fs::write(&tmp_path, serde_json::to_string_pretty(&candidate_config)?)?;
        std::fs::rename(&tmp_path, &cfg_path)?;

        // Stop previous instance only AFTER preflight passed.
        if let Some(mut old) = self.process.take() {
            old.terminate();
        }

        let mut proc = spawn_managed_process(sing_box_path, &cfg_path)?;
        let stdout = proc.child.stdout.take();
        let stderr = proc.child.stderr.take();
        // sing-box writes all normal INFO logs to stderr by default; never classify stderr as WARN.
        spawn_log_reader(stdout, Arc::clone(&self.logs));
        spawn_log_reader(stderr, Arc::clone(&self.logs));

        self.push_log(
            "INFO",
            format!("supervisor: started sing-box (pid {})", proc.pid()),
        );
        self.process = Some(proc);
        self.active_config = Some(candidate_config);
        Ok(report)
    }

    pub fn stop(&mut self) {
        if let Some(mut proc) = self.process.take() {
            let pid = proc.pid();
            proc.terminate();
            self.push_log("INFO", format!("supervisor: stopped sing-box (pid {})", pid));
        }
    }

    pub fn require_running_pid(&mut self) -> Result<u32> {
        if let Some(p) = self.process.as_mut() {
            if p.is_running() {
                return Ok(p.pid());
            }
        }
        Err(anyhow!("sing-box core is not running"))
    }

    fn push_log(&self, level: &str, msg: String) {
        if let Ok(mut guard) = self.logs.lock() {
            guard.push(level, msg);
        }
    }
}

pub fn parse_singbox_log_line(raw: &str) -> (&'static str, String) {
    let clean = strip_ansi_codes(raw);
    let trimmed = clean.trim();

    // Format 1: "+0800 2026-11-26 00:46:45 INFO inbound/mixed[in-10808]: ..."
    let tokens: Vec<&str> = trimmed.splitn(5, ' ').collect();
    if tokens.len() == 5
        && (tokens[0].starts_with('+') || tokens[0].starts_with('-') || tokens[0] == "UTC")
        && tokens[1].contains('-')
        && tokens[2].contains(':')
    {
        if let Some(level) = normalize_log_level(tokens[3]) {
            return (level, tokens[4].trim().to_string());
        }
    }

    // Format 2: "INFO[0000] ..." or "ERROR[0012] ..."
    for (prefix, level) in [
        ("FATAL[", "ERROR"),
        ("ERROR[", "ERROR"),
        ("WARN[", "WARN"),
        ("INFO[", "INFO"),
        ("DEBUG[", "DEBUG"),
        ("TRACE[", "DEBUG"),
    ] {
        if let Some(rest) = trimmed.strip_prefix(prefix) {
            if let Some((_, msg)) = rest.split_once(']') {
                return (level, msg.trim().to_string());
            }
        }
    }

    // Fallback level detection by standalone token.
    let upper = trimmed.to_ascii_uppercase();
    let level = if upper.contains(" FATAL ") || upper.contains(" ERROR ") || upper.starts_with("FATAL") || upper.starts_with("ERROR") {
        "ERROR"
    } else if upper.contains(" WARN ") || upper.starts_with("WARN") {
        "WARN"
    } else if upper.contains(" DEBUG ") || upper.starts_with("DEBUG") {
        "DEBUG"
    } else {
        "INFO"
    };
    (level, trimmed.to_string())
}

fn normalize_log_level(tok: &str) -> Option<&'static str> {
    match tok.to_ascii_uppercase().as_str() {
        "FATAL" | "PANIC" | "ERROR" => Some("ERROR"),
        "WARN" | "WARNING" => Some("WARN"),
        "INFO" => Some("INFO"),
        "DEBUG" | "TRACE" => Some("DEBUG"),
        _ => None,
    }
}

fn strip_ansi_codes(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' && chars.peek() == Some(&'[') {
            chars.next();
            for next in chars.by_ref() {
                if next.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

fn spawn_log_reader<R: std::io::Read + Send + 'static>(
    stream: Option<R>,
    logs: Arc<Mutex<LogRing>>,
) {
    let Some(reader) = stream else {
        return;
    };
    std::thread::spawn(move || {
        let buf = BufReader::new(reader);
        for line in buf.lines().map_while(Result::ok) {
            if line.trim().is_empty() {
                continue;
            }
            let (level, message) = parse_singbox_log_line(&line);
            if message.is_empty() {
                continue;
            }
            if let Ok(mut guard) = logs.lock() {
                guard.push(level, message);
            }
        }
    });
}

fn first_line(s: &str) -> &str {
    s.lines().next().unwrap_or("unknown error")
}
