use std::path::PathBuf;

use crate::agents::runner::{AgentCmd, PtySize};
use crate::swarm::agent::{AgentInstance, AgentLifecycleState, AgentOutput};
use crate::swarm::engine::SwarmEngine;

/// BuildAgent/TestAgent JSON result envelope.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct SwiftBuildOutput {
    pub success: bool,
    pub output: String,
    pub error: String,
    pub duration: f64,
    #[serde(default)]
    pub metadata: serde_json::Value,
    #[serde(rename = "exitCode")]
    pub exit_code: i32,
}

impl SwiftBuildOutput {
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

/// Strip ANSI escape sequences from a string.
fn strip_ansi(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            for next in chars.by_ref() {
                if ('@'..='~').contains(&next) {
                    break;
                }
            }
        } else {
            result.push(c);
        }
    }
    result
}

/// Parse the BuildAgent/TestAgent `--json` envelope from raw screen text.
///
/// Agents may emit ANSI-colored progress lines before the final JSON object.
/// This function strips ANSI escapes and returns the last successfully parsed
/// `SwiftBuildOutput`, or `None` if no valid envelope is found.
pub fn parse_build_agent_output(text: &str) -> Option<SwiftBuildOutput> {
    let cleaned = strip_ansi(text);
    for line in cleaned.lines().rev() {
        let trimmed = line.trim();
        if trimmed.starts_with('{') && trimmed.ends_with('}') {
            if let Ok(output) = serde_json::from_str::<SwiftBuildOutput>(trimmed) {
                return Some(output);
            }
        }
    }
    None
}

/// Apply a parsed Swift build/test envelope to an agent instance.
///
/// Sets the lifecycle state based on `success`/`exit_code`, appends a summary to
/// the agent's memory, and surfaces the parsed result as the agent output.
pub fn apply_swift_output(agent: &mut AgentInstance, output: SwiftBuildOutput) {
    let summary = output.summary();
    let detail = if output.error.is_empty() {
        output.output.clone()
    } else {
        format!("{}\nError: {}", output.output, output.error)
    };

    if output.exit_code != 0 || !output.success {
        agent.set_state(AgentLifecycleState::Failed);
        agent.append_to_memory(
            "system",
            format!(
                "Swift agent {} failed. {}\n{}",
                agent.role.display_name(),
                summary,
                detail
            ),
        );
    } else {
        agent.set_state(AgentLifecycleState::Completed);
        agent.append_to_memory(
            "assistant",
            format!(
                "Swift agent {} completed. {}\n{}",
                agent.role.display_name(),
                summary,
                detail
            ),
        );
    }

    // TODO: Extract a GistDraft from the structured output instead of relying on
    // the generic memory extractor.
    let _ = agent.extract_gist();

    agent.output = AgentOutput::SwiftResult(output);
}

/// Build a BuildAgent command for the Swift build specialist agent.
pub fn build_agent_cmd(
    task: &str,
    project: Option<&str>,
    scheme: Option<&str>,
    artifacts_dir: Option<&PathBuf>,
    provider: Option<&str>,
    model: Option<&str>,
) -> AgentCmd {
    let mut args = vec![
        "--json".to_string(),
        "--prompt-string".to_string(),
        task.to_string(),
    ];
    if let Some(p) = project {
        args.push("--project".to_string());
        args.push(p.to_string());
    }
    if let Some(s) = scheme {
        args.push("--scheme".to_string());
        args.push(s.to_string());
    }
    if let Some(dir) = artifacts_dir {
        args.push("--artifacts-dir".to_string());
        args.push(dir.to_string_lossy().to_string());
    }
    if let Some(p) = provider {
        args.push("--ai-provider".to_string());
        args.push(p.to_string());
    }
    if let Some(m) = model {
        args.push("--ai-model".to_string());
        args.push(m.to_string());
    }
    args.push("--yolo".to_string());

    AgentCmd {
        program: "BuildAgent".to_string(),
        args,
        env: Vec::new(),
        cwd: None,
    }
}

/// Build a TestAgent command for the Swift test specialist agent.
pub fn test_agent_cmd(
    task: &str,
    project: Option<&str>,
    scheme: Option<&str>,
    artifacts_dir: Option<&PathBuf>,
    provider: Option<&str>,
    model: Option<&str>,
) -> AgentCmd {
    let mut args = vec![
        "--json".to_string(),
        "--prompt-string".to_string(),
        task.to_string(),
    ];
    if let Some(p) = project {
        args.push("--project".to_string());
        args.push(p.to_string());
    }
    if let Some(s) = scheme {
        args.push("--scheme".to_string());
        args.push(s.to_string());
    }
    if let Some(dir) = artifacts_dir {
        args.push("--artifacts-dir".to_string());
        args.push(dir.to_string_lossy().to_string());
    }
    if let Some(p) = provider {
        args.push("--ai-provider".to_string());
        args.push(p.to_string());
    }
    if let Some(m) = model {
        args.push("--ai-model".to_string());
        args.push(m.to_string());
    }
    args.push("--yolo".to_string());

    AgentCmd {
        program: "TestAgent".to_string(),
        args,
        env: Vec::new(),
        cwd: None,
    }
}

/// Spawn a Swift build/test agent through the swarm engine.
pub fn spawn_swift_agent(
    engine: &mut SwarmEngine,
    name: &str,
    role: crate::swarm::agent::AgentRole,
    cmd: AgentCmd,
    size: PtySize,
) -> Result<String, String> {
    let provider = engine
        .agents
        .iter()
        .find(|a| a.name == name)
        .map(|a| a.provider.clone())
        .unwrap_or_else(|| "anthropic".to_string());
    let model = engine
        .agents
        .iter()
        .find(|a| a.name == name)
        .map(|a| a.model.clone())
        .unwrap_or_else(|| "claude-sonnet-4-6".to_string());

    let id = engine.spawn_agent(name.to_string(), role, provider, model);
    engine.spawn_pty_agent(&id, cmd, size)?;
    if let Some(a) = engine.get_agent_mut(&id) {
        a.set_state(AgentLifecycleState::Acting);
    }
    Ok(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_success_envelope() {
        let text = r#"{"success":true,"output":"Build succeeded","error":"","duration":12.3,"metadata":{},"exitCode":0}"#;
        let output = parse_build_agent_output(text).unwrap();
        assert!(output.success);
        assert_eq!(output.exit_code, 0);
        assert_eq!(output.output, "Build succeeded");
    }

    #[test]
    fn parse_ansi_colored_envelope() {
        let text = "\x1b[32mProgress: 50%\x1b[0m\n\x1b[31m\x1b[1m";
        let envelope = r#"{"success":true,"output":"Done","error":"","duration":5.0,"metadata":{},"exitCode":0}"#;
        let text = format!("{}\n{}", text, envelope);
        let output = parse_build_agent_output(&text).unwrap();
        assert!(output.success);
        assert_eq!(output.exit_code, 0);
        assert_eq!(output.output, "Done");
    }

    #[test]
    fn parse_missing_json_returns_none() {
        let text = "Just some progress output\nwith no JSON envelope";
        assert!(parse_build_agent_output(text).is_none());
    }

    #[test]
    fn parse_exit_code_11_failure() {
        let text = r#"{"success":false,"output":"","error":"Invalid scheme","duration":1.2,"metadata":{},"exitCode":11}"#;
        let output = parse_build_agent_output(text).unwrap();
        assert!(!output.success);
        assert_eq!(output.exit_code, 11);
        assert_eq!(output.error, "Invalid scheme");
    }
}
