//! CLI-side client for the instance control socket.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::Duration;

use crate::control::protocol::{ControlRequest, ControlResponse};
use crate::control::registry::{self, InstanceRecord};

const READ_TIMEOUT: Duration = Duration::from_secs(20);

pub fn send(socket: &Path, request: &ControlRequest) -> Result<ControlResponse, String> {
    let stream = UnixStream::connect(socket)
        .map_err(|e| format!("cannot connect to instance socket {}: {}", socket.display(), e))?;
    stream
        .set_read_timeout(Some(READ_TIMEOUT))
        .map_err(|e| e.to_string())?;
    let mut writer = stream.try_clone().map_err(|e| e.to_string())?;
    let mut json = serde_json::to_string(request).map_err(|e| e.to_string())?;
    json.push('\n');
    writer
        .write_all(json.as_bytes())
        .map_err(|e| format!("write failed: {}", e))?;
    writer.flush().map_err(|e| e.to_string())?;

    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader
        .read_line(&mut line)
        .map_err(|e| format!("read failed: {}", e))?;
    if line.trim().is_empty() {
        return Err("instance closed the connection without responding".to_string());
    }
    serde_json::from_str(&line).map_err(|e| format!("invalid response: {}", e))
}

pub fn send_to_instance(
    selector: Option<&str>,
    request: &ControlRequest,
) -> Result<(InstanceRecord, ControlResponse), String> {
    let record = registry::resolve(selector)?;
    let response = send(&record.socket, request)?;
    Ok((record, response))
}
