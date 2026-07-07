use serde::{Deserialize, Serialize};

pub mod config;

#[path = "BuildAgentSkill.rs"]
pub mod build_agent_skill;

#[path = "TestAgentSkill.rs"]
pub mod test_agent_skill;

/// Structured result envelope produced by Foundation Phase 5 agents when run
/// with `--json`. This shape is intentionally identical to the JSON emitted by
/// `BuildAgent.sh` and `testAgent.sh` so the Swarm can ingest it without
/// additional transformation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FoundationAgentOutput {
    pub success: bool,
    pub output: String,
    pub error: String,
    pub duration: f64,
    #[serde(default)]
    pub metadata: serde_json::Value,
    #[serde(rename = "exitCode")]
    pub exit_code: i32,
}

impl FoundationAgentOutput {
    /// One-line status summary suitable for agent memory or gist evidence.
    pub fn summary(&self) -> String {
        let status = if self.success && self.exit_code == 0 {
            "success"
        } else {
            "failure"
        };
        format!(
            "{} (exit={}, duration={:.2}s)",
            status, self.exit_code, self.duration
        )
    }
}
