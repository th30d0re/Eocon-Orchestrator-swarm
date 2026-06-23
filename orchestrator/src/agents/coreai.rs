use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{channel, Receiver};

/// Provider-sidecar manager for on-device Core AI and MLX backends.
/// v1 is a skeleton: it spawns `localmodel-lm` and reads status lines on stderr.
pub struct LocalModelProvider {
    pub backend: String,
    pub child: Option<Child>,
    pub stdout_rx: Option<Receiver<String>>,
    pub ready: bool,
}

impl LocalModelProvider {
    pub fn new(backend: &str) -> Self {
        Self {
            backend: backend.to_string(),
            child: None,
            stdout_rx: None,
            ready: false,
        }
    }

    pub fn spawn(&mut self) -> Result<(), String> {
        let program = match self.backend.as_str() {
            "coreai" => "coreai-lm",
            "mlx" => "localmodel-lm",
            _ => return Err(format!("unknown backend: {}", self.backend)),
        };

        let mut cmd = Command::new(program);
        cmd.arg("--backend").arg(&self.backend);
        cmd.stdin(Stdio::piped());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        let mut child = cmd.spawn().map_err(|e| format!("failed to spawn {}: {}", program, e))?;

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

        self.stdout_rx = Some(rx);
        self.child = Some(child);
        Ok(())
    }

    pub fn request(&mut self, session_id: &str, prompt: &str) -> Result<String, String> {
        let child = self.child.as_mut().ok_or("sidecar not running")?;
        let stdin = child.stdin.as_mut().ok_or("no stdin")?;
        let req = serde_json::json!({
            "session_id": session_id,
            "prompt": prompt,
            "role": "reasoning",
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
        self.ready = false;
        self.spawn()
    }
}
