//! Shared app-wide state: the spawned llama-server child, its status fields,
//! and the rolling log-history buffer, guarded by a single `Mutex` managed as
//! Tauri state (`Mutex<AppState>`).
use std::collections::VecDeque;
use std::process::Child;
use std::sync::Mutex;

/// Maximum number of log lines kept for `get_log_history` backfill.
pub const LOG_HISTORY_LIMIT: usize = 200;

/// Default Web UI port used when no `--port`/`-p` arg is given.
pub const DEFAULT_WEB_PORT: u16 = 8080;

pub struct AppState {
    pub is_running: bool,
    pub server_pid: Option<u32>,
    pub web_port: u16,
    pub process: Option<Child>,
    pub log_history: VecDeque<String>,
    // Active run metadata for the API panel.
    pub model_alias: String,
    pub api_key: Option<String>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            is_running: false,
            server_pid: None,
            web_port: DEFAULT_WEB_PORT,
            process: None,
            log_history: VecDeque::new(),
            model_alias: String::new(),
            api_key: None,
        }
    }

    /// Atomically read the three status fields used by `emit_server_status`
    /// without exposing the whole struct.
    pub fn status_snapshot(&self) -> (bool, Option<u32>, u16) {
        (self.is_running, self.server_pid, self.web_port)
    }

    /// Mark the run as stopped and clear all per-run fields. Called from every
    /// path that tears the process down (graceful stop, process-exit watcher,
    /// tray "quit").
    pub fn clear_run(&mut self) {
        self.is_running = false;
        self.server_pid = None;
        self.web_port = DEFAULT_WEB_PORT;
        self.model_alias.clear();
        self.api_key = None;
        self.process = None;
    }
}

/// Cancellation handle for the llama.cpp directory scan.
pub struct ScanControl {
    pub cancel_flag: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
}

impl ScanControl {
    pub fn new() -> Self {
        Self {
            cancel_flag: None,
        }
    }
}

// Convenience alias so call sites can write `State<'_, Mutex<AppState>>`
// without re-implementing `std::sync::Mutex` qualification everywhere.
pub type AppStateLock = Mutex<AppState>;
