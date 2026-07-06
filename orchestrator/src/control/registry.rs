//! Registry of running swarm instances.
//!
//! Every instance (TUI or headless) writes `~/.eocon/instances/<pid>.json`
//! on startup and removes it on clean exit. `list()` probes each recorded
//! control socket and prunes records whose instance is gone, so crashed
//! instances self-heal out of the registry.

use std::fs;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstanceRecord {
    pub pid: u32,
    pub name: String,
    pub socket: PathBuf,
    pub project_dir: PathBuf,
    /// "tui" or "headless"
    pub mode: String,
    pub started_at_epoch: u64,
}

fn eocon_home() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".eocon")
}

pub fn instances_dir() -> PathBuf {
    eocon_home().join("instances")
}

pub fn run_dir() -> PathBuf {
    eocon_home().join("run")
}

pub fn socket_path_for(pid: u32) -> PathBuf {
    run_dir().join(format!("eos-{}.sock", pid))
}

fn record_path(pid: u32) -> PathBuf {
    instances_dir().join(format!("{}.json", pid))
}

/// Write this process's instance record. Returns a guard that removes the
/// record and socket file when dropped.
pub fn register(
    name: String,
    socket: PathBuf,
    project_dir: PathBuf,
    mode: &str,
) -> std::io::Result<InstanceGuard> {
    let pid = std::process::id();
    let record = InstanceRecord {
        pid,
        name,
        socket: socket.clone(),
        project_dir,
        mode: mode.to_string(),
        started_at_epoch: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0),
    };
    fs::create_dir_all(instances_dir())?;
    let path = record_path(pid);
    fs::write(&path, serde_json::to_string_pretty(&record)?)?;
    Ok(InstanceGuard {
        record_path: path,
        socket_path: socket,
        record,
    })
}

/// List live instances, pruning records whose control socket no longer
/// accepts connections.
pub fn list() -> Vec<InstanceRecord> {
    let dir = instances_dir();
    let Ok(entries) = fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut records = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let Ok(contents) = fs::read_to_string(&path) else {
            continue;
        };
        let Ok(record) = serde_json::from_str::<InstanceRecord>(&contents) else {
            let _ = fs::remove_file(&path);
            continue;
        };
        if is_alive(&record) {
            records.push(record);
        } else {
            let _ = fs::remove_file(&path);
            let _ = fs::remove_file(&record.socket);
        }
    }
    records.sort_by_key(|r| r.started_at_epoch);
    records
}

/// Resolve an instance by name or pid; with no selector, succeeds only when
/// exactly one instance is running.
pub fn resolve(selector: Option<&str>) -> Result<InstanceRecord, String> {
    let records = list();
    match selector {
        Some(sel) => records
            .into_iter()
            .find(|r| r.name == sel || r.pid.to_string() == sel)
            .ok_or_else(|| format!("no running instance matches '{}' (try `eos ps`)", sel)),
        None => match records.len() {
            0 => Err("no running swarm instances (start one with `eos` or `eos serve`)".into()),
            1 => Ok(records.into_iter().next().unwrap()),
            n => Err(format!(
                "{} instances running; pick one with --instance <name|pid> (try `eos ps`)",
                n
            )),
        },
    }
}

fn is_alive(record: &InstanceRecord) -> bool {
    UnixStream::connect(&record.socket).is_ok()
}

pub struct InstanceGuard {
    record_path: PathBuf,
    socket_path: PathBuf,
    pub record: InstanceRecord,
}

impl Drop for InstanceGuard {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.record_path);
        let _ = fs::remove_file(&self.socket_path);
    }
}
