use std::path::PathBuf;

use crate::agents::runner::{AgentCmd, PtySize};
use crate::swarm::agent::AgentLifecycleState;
use crate::swarm::engine::SwarmEngine;

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
        .unwrap_or_else(|| "claude-sonnet-4".to_string());

    let id = engine.spawn_agent(name.to_string(), role, provider, model);
    engine.spawn_pty_agent(&id, cmd, size)?;
    if let Some(a) = engine.get_agent_mut(&id) {
        a.set_state(AgentLifecycleState::Acting);
    }
    Ok(id)
}
