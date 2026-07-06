//! JSON-lines protocol spoken over each instance's Unix control socket.
//!
//! Requests and responses are single-line JSON documents. The same slash
//! commands accepted by the TUI prompt are accepted here, so the CLI surface
//! is exactly the TUI surface.

use serde::{Deserialize, Serialize};

use crate::app::App;
use crate::ui::theme::Voice;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ControlRequest {
    /// Liveness probe; also used by the registry to prune dead instances.
    Ping,
    /// Run one input line exactly as if typed into the TUI prompt.
    /// Slash commands dispatch to the command handlers; anything else is
    /// treated as a natural-language swarm task.
    Command { command: String },
    /// Full serializable state of the instance.
    Snapshot,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlResponse {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub messages: Vec<SnapshotMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snapshot: Option<Snapshot>,
}

impl ControlResponse {
    pub fn ok() -> Self {
        Self {
            ok: true,
            error: None,
            messages: Vec::new(),
            snapshot: None,
        }
    }

    pub fn error(msg: impl Into<String>) -> Self {
        Self {
            ok: false,
            error: Some(msg.into()),
            messages: Vec::new(),
            snapshot: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotMessage {
    pub voice: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotAgent {
    pub id: String,
    pub name: String,
    pub role: String,
    pub provider: String,
    pub model: String,
    pub state: String,
    pub current_task: Option<String>,
    pub risk_tier: String,
    pub stall_count: u32,
    pub elapsed_secs: u64,
    pub memory_messages: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotGist {
    pub id: String,
    pub agent_name: String,
    pub status: String,
    pub claim: String,
    pub summary: String,
    pub evidence_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotMetrics {
    pub success_rate: f64,
    pub compliance_rate: f64,
    pub csr: f64,
    pub machiavellian_gap: f64,
    pub total_tokens: u64,
    pub successes: u64,
    pub failures: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    pub pid: u32,
    pub name: String,
    pub mode: String,
    pub phase: String,
    pub project_dir: String,
    pub uptime_secs: u64,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub queued_tasks: usize,
    pub agents: Vec<SnapshotAgent>,
    pub gists: Vec<SnapshotGist>,
    pub metrics: SnapshotMetrics,
    /// Tail of the interaction log (most recent last).
    pub recent: Vec<SnapshotMessage>,
}

pub fn voice_name(voice: Voice) -> &'static str {
    match voice {
        Voice::User => "user",
        Voice::Agent => "agent",
        Voice::Shell => "shell",
        Voice::Error => "error",
        Voice::Warning => "warning",
        Voice::Gist => "gist",
        Voice::System => "system",
    }
}

const RECENT_TAIL: usize = 40;

pub fn build_snapshot(app: &App, name: &str, mode: &str) -> Snapshot {
    let agents = app
        .swarm_engine
        .agents
        .iter()
        .map(|a| SnapshotAgent {
            id: a.id.clone(),
            name: a.name.clone(),
            role: a.role.display_name().to_string(),
            provider: a.provider.clone(),
            model: a.model.clone(),
            state: format!("{:?}", a.state),
            current_task: a.current_task.clone(),
            risk_tier: format!("{:?}", a.risk_tier),
            stall_count: a.stall_count,
            elapsed_secs: a.elapsed_secs(),
            memory_messages: a.memory.messages.len(),
            last_error: a.last_error.clone(),
        })
        .collect();

    let gists = app
        .swarm_engine
        .gist_board
        .gists
        .iter()
        .map(|g| SnapshotGist {
            id: g.id.clone(),
            agent_name: g.agent_name.clone(),
            status: g.status_string().to_string(),
            claim: g.claim.clone(),
            summary: g.summary.clone(),
            evidence_count: g.evidence.len(),
        })
        .collect();

    let m = &app.swarm_engine.metrics;
    let metrics = SnapshotMetrics {
        success_rate: m.success_rate(),
        compliance_rate: m.compliance_rate(),
        csr: m.csr(0.5),
        machiavellian_gap: m.machiavellian_gap(),
        total_tokens: m.total_tokens_consumed,
        successes: m.successes,
        failures: m.failures,
    };

    let recent = app
        .interaction
        .iter()
        .rev()
        .take(RECENT_TAIL)
        .rev()
        .map(|msg| SnapshotMessage {
            voice: voice_name(msg.voice).to_string(),
            text: msg.text.clone(),
        })
        .collect();

    Snapshot {
        pid: std::process::id(),
        name: name.to_string(),
        mode: mode.to_string(),
        phase: format!("{:?}", app.swarm_phase),
        project_dir: app.project_dir.display().to_string(),
        uptime_secs: app.elapsed_secs(),
        provider: app.current_provider.clone(),
        model: app.current_model.clone(),
        queued_tasks: app.swarm_engine.task_queue.pending().len(),
        agents,
        gists,
        metrics,
        recent,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_round_trips() {
        let req = ControlRequest::Command {
            command: "/agent list".to_string(),
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"type\":\"command\""));
        let back: ControlRequest = serde_json::from_str(&json).unwrap();
        match back {
            ControlRequest::Command { command } => assert_eq!(command, "/agent list"),
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn response_omits_empty_fields() {
        let json = serde_json::to_string(&ControlResponse::ok()).unwrap();
        assert_eq!(json, "{\"ok\":true}");
    }
}
