use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{channel, Receiver};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

/// Provider-sidecar manager for on-device Core AI and MLX backends.
/// Spawns the xcodebuild-produced `localmodel-lm` binary and gates requests on
/// the stderr `ready` status line.
pub struct LocalModelProvider {
    pub backend: String,
    pub project_dir: PathBuf,
    pub child: Option<Child>,
    pub stdout_rx: Option<Receiver<String>>,
    pub ready: Arc<Mutex<bool>>,
    pub stderr_messages: Arc<Mutex<Vec<String>>>,
}

impl LocalModelProvider {
    pub fn new(backend: &str, project_dir: PathBuf) -> Self {
        Self {
            backend: backend.to_string(),
            project_dir,
            child: None,
            stdout_rx: None,
            ready: Arc::new(Mutex::new(false)),
            stderr_messages: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Resolve the sidecar binary path.
    ///
    /// Priority:
    /// 1. `EOCON_LOCALMODEL_LM_PATH` environment variable.
    /// 2. `<project_dir>/../localmodel-lm/.build/xcode/Build/Products/Debug/localmodel-lm`
    /// 3. `<repo_root>/localmodel-lm/.build/xcode/Build/Products/Debug/localmodel-lm`
    ///    (where repo_root is project_dir's parent).
    fn resolve_binary_path(&self) -> Result<PathBuf, String> {
        if let Ok(env_path) = std::env::var("EOCON_LOCALMODEL_LM_PATH") {
            let p = PathBuf::from(env_path);
            if p.exists() {
                return Ok(p);
            }
        }

        let candidates = [
            self.project_dir
                .join("../localmodel-lm/.build/xcode/Build/Products/Debug/localmodel-lm"),
            self.project_dir
                .parent()
                .map(|p| p.join("localmodel-lm/.build/xcode/Build/Products/Debug/localmodel-lm"))
                .unwrap_or_default(),
        ];

        for candidate in &candidates {
            if candidate.exists() {
                return Ok(candidate
                    .canonicalize()
                    .unwrap_or_else(|_| candidate.clone()));
            }
        }

        Err(format!(
            "localmodel-lm sidecar binary not found. Searched: {:?}",
            candidates
        ))
    }

    pub fn spawn(&mut self) -> Result<(), String> {
        let program = self.resolve_binary_path()?;

        let mut cmd = Command::new(&program);
        cmd.arg("--backend").arg(&self.backend);
        cmd.stdin(Stdio::piped());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());
        // Ensure the sidecar can locate its companion bundle (xcodebuild places it
        // next to the binary).
        if let Some(dir) = program.parent() {
            cmd.current_dir(dir);
        }

        let mut child = cmd
            .spawn()
            .map_err(|e| format!("failed to spawn {}: {}", program.display(), e))?;

        let stdout = child.stdout.take().ok_or("no stdout")?;
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines() {
                if let Ok(l) = line {
                    if tx.send(l).is_err() {
                        break;
                    }
                }
            }
        });

        let stderr = child.stderr.take().ok_or("no stderr")?;
        let ready = Arc::clone(&self.ready);
        let stderr_messages = Arc::clone(&self.stderr_messages);
        std::thread::spawn(move || {
            let reader = BufReader::new(stderr);
            for line in reader.lines() {
                if let Ok(l) = line {
                    if let Ok(value) = serde_json::from_str::<serde_json::Value>(&l) {
                        if let Some(status) = value.get("status").and_then(|s| s.as_str()) {
                            match status {
                                "ready" => {
                                    if let Ok(mut r) = ready.lock() {
                                        *r = true;
                                    }
                                }
                                "error" => {
                                    if let Ok(mut r) = ready.lock() {
                                        *r = false;
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                    if let Ok(mut msgs) = stderr_messages.lock() {
                        msgs.push(l);
                    }
                }
            }
        });

        self.stdout_rx = Some(rx);
        self.child = Some(child);
        Ok(())
    }

    pub fn wait_for_ready(&self, timeout: Duration) -> Result<(), String> {
        let start = Instant::now();
        loop {
            if let Ok(ready) = self.ready.lock() {
                if *ready {
                    return Ok(());
                }
            }
            if start.elapsed() >= timeout {
                let messages = self
                    .stderr_messages
                    .lock()
                    .map(|m| m.join("; "))
                    .unwrap_or_default();
                return Err(format!(
                    "sidecar did not become ready within {:?}. stderr: {}",
                    timeout, messages
                ));
            }
            thread::sleep(Duration::from_millis(50));
        }
    }

    pub fn request(&mut self, session_id: &str, prompt: &str) -> Result<String, String> {
        if self.child.is_none() || !self.health_check() {
            self.restart()?;
        }

        let child = self.child.as_mut().ok_or("sidecar not running")?;
        let stdin = child.stdin.as_mut().ok_or("no stdin")?;
        let req = serde_json::json!({
            "session_id": session_id,
            "prompt": prompt,
        });
        writeln!(stdin, "{}", req).map_err(|e| e.to_string())?;
        stdin.flush().map_err(|e| e.to_string())?;

        if let Some(rx) = &self.stdout_rx {
            match rx.recv() {
                Ok(line) => Ok(line),
                Err(_) => Err("sidecar stdout closed".to_string()),
            }
        } else {
            Err("no stdout receiver".to_string())
        }
    }

    pub fn health_check(&mut self) -> bool {
        if let Some(child) = &mut self.child {
            matches!(child.try_wait(), Ok(None))
        } else {
            false
        }
    }

    pub fn restart(&mut self) -> Result<(), String> {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
        }
        if let Ok(mut r) = self.ready.lock() {
            *r = false;
        }
        if let Ok(mut msgs) = self.stderr_messages.lock() {
            msgs.clear();
        }
        self.stdout_rx = None;
        self.spawn()
    }
}
