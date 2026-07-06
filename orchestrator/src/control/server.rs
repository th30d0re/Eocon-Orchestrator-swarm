//! Unix-socket control server embedded in every running instance.
//!
//! The main event loop stays synchronous: connection threads parse requests
//! and hand them over an mpsc channel; `drain()` is called once per loop
//! iteration on the main thread, executes each request against `&mut App`,
//! and sends the response back to the waiting connection thread.

use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::Duration;

use crate::app::{App, InputSource};
use crate::control::protocol::{
    build_snapshot, voice_name, ControlRequest, ControlResponse, SnapshotMessage,
};
use crate::ui::theme::Voice;

/// How long a connection thread waits for the main loop to execute a request.
const REPLY_TIMEOUT: Duration = Duration::from_secs(15);

struct PendingRequest {
    request: ControlRequest,
    reply_tx: Sender<ControlResponse>,
}

pub struct ControlServer {
    pub socket_path: PathBuf,
    /// Instance name reported in snapshots.
    pub instance_name: String,
    /// "tui" or "headless".
    pub mode: String,
    rx: Receiver<PendingRequest>,
}

impl ControlServer {
    pub fn start(
        socket_path: &Path,
        instance_name: String,
        mode: &str,
    ) -> std::io::Result<Self> {
        if let Some(parent) = socket_path.parent() {
            fs::create_dir_all(parent)?;
        }
        // A previous crashed instance may have left the socket file behind.
        let _ = fs::remove_file(socket_path);
        let listener = UnixListener::bind(socket_path)?;

        let (tx, rx) = channel::<PendingRequest>();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(stream) = stream else { continue };
                let tx = tx.clone();
                std::thread::spawn(move || handle_connection(stream, tx));
            }
        });

        Ok(Self {
            socket_path: socket_path.to_path_buf(),
            instance_name,
            mode: mode.to_string(),
            rx,
        })
    }

    /// Execute all queued control requests. Called from the main event loop.
    pub fn drain(&self, app: &mut App) {
        while let Ok(pending) = self.rx.try_recv() {
            let response = execute(app, pending.request, &self.instance_name, &self.mode);
            let _ = pending.reply_tx.send(response);
        }
    }
}

fn handle_connection(stream: UnixStream, tx: Sender<PendingRequest>) {
    let Ok(reader_stream) = stream.try_clone() else {
        return;
    };
    let reader = BufReader::new(reader_stream);
    let mut writer = stream;
    for line in reader.lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let response = match serde_json::from_str::<ControlRequest>(&line) {
            Ok(request) => {
                let (reply_tx, reply_rx) = channel();
                if tx.send(PendingRequest { request, reply_tx }).is_err() {
                    ControlResponse::error("instance is shutting down")
                } else {
                    match reply_rx.recv_timeout(REPLY_TIMEOUT) {
                        Ok(resp) => resp,
                        Err(_) => ControlResponse::error("timed out waiting for instance"),
                    }
                }
            }
            Err(e) => ControlResponse::error(format!("invalid request: {}", e)),
        };
        let Ok(mut json) = serde_json::to_string(&response) else {
            break;
        };
        json.push('\n');
        if writer.write_all(json.as_bytes()).is_err() {
            break;
        }
        let _ = writer.flush();
    }
}

fn execute(app: &mut App, request: ControlRequest, name: &str, mode: &str) -> ControlResponse {
    match request {
        ControlRequest::Ping => ControlResponse::ok(),
        ControlRequest::Snapshot => {
            let mut resp = ControlResponse::ok();
            resp.snapshot = Some(build_snapshot(app, name, mode));
            resp
        }
        ControlRequest::Command { command } => {
            let line = command.trim().to_string();
            if line.is_empty() {
                return ControlResponse::error("empty command");
            }
            app.begin_capture();
            app.submit_line(&line, InputSource::Remote);
            let captured = app.take_capture();
            let ok = !captured.iter().any(|m| m.voice == Voice::Error);
            ControlResponse {
                ok,
                error: None,
                messages: captured
                    .into_iter()
                    .map(|m| SnapshotMessage {
                        voice: voice_name(m.voice).to_string(),
                        text: m.text,
                    })
                    .collect(),
                snapshot: None,
            }
        }
    }
}
