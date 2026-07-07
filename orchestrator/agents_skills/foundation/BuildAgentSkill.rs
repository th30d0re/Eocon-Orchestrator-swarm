use std::path::PathBuf;

use crate::agents::runner::AgentCmd;
use crate::agents_skills::foundation::config::FoundationConfig;
use crate::agents_skills::foundation::FoundationAgentOutput;

/// Errors that can occur when resolving or translating a Foundation BuildAgent
/// skill invocation.
#[derive(Debug, thiserror::Error)]
pub enum FoundationSkillError {
    #[error("Foundation root does not exist: {0}")]
    RootNotFound(PathBuf),
    #[error("BuildAgent script not found or not executable: {0}")]
    ScriptNotFound(PathBuf),
}

/// Reusable wrapper around the Foundation Phase 5 `BuildAgent.sh` script.
///
/// The skill does not embed the agent source; it resolves the script via
/// `FoundationConfig` and translates a generic Swarm `AgentCmd` into a concrete
/// invocation of that script.
#[derive(Debug, Clone)]
pub struct BuildAgentSkill {
    pub config: FoundationConfig,
}

impl BuildAgentSkill {
    pub fn new(config: FoundationConfig) -> Self {
        Self { config }
    }

    /// Resolve the absolute path to `BuildAgent.sh`.
    pub fn script_path(&self) -> PathBuf {
        self.config.build_agent_script()
    }

    /// Translate a generic `AgentCmd` into a Foundation BuildAgent invocation.
    ///
    /// Recognized program names: `BuildAgent` and `BuildAgent.sh`. The returned
    /// command uses the resolved script path as its program, preserves the
    /// original arguments, and injects `EOCON_FOUNDATION_ROOT` into the
    /// environment so the script can locate its own libraries.
    pub fn translate_cmd(&self, cmd: &AgentCmd) -> Result<AgentCmd, FoundationSkillError> {
        let script = self.script_path();
        if !script.exists() {
            return Err(FoundationSkillError::ScriptNotFound(script));
        }

        let mut translated = cmd.clone();
        translated.program = script.to_string_lossy().to_string();
        translated
            .env
            .retain(|(k, _)| k != "EOCON_FOUNDATION_ROOT");
        translated.env.push((
            "EOCON_FOUNDATION_ROOT".to_string(),
            self.config.root.to_string_lossy().to_string(),
        ));
        Ok(translated)
    }

    /// Parse the last JSON envelope from raw agent output (which may contain
    /// ANSI-colored progress lines).
    pub fn parse_output(&self, raw: &str) -> Option<FoundationAgentOutput> {
        crate::agents::swift::parse_build_agent_output(raw).map(|swift| {
            FoundationAgentOutput {
                success: swift.success,
                output: swift.output,
                error: swift.error,
                duration: swift.duration,
                metadata: swift.metadata,
                exit_code: swift.exit_code,
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::policy::policy::RiskLabel;

    fn sample_cmd(program: &str) -> AgentCmd {
        AgentCmd {
            program: program.to_string(),
            args: vec![
                "--json".to_string(),
                "--prompt-string".to_string(),
                "fix login flow".to_string(),
            ],
            env: vec![("EXTRA".to_string(), "value".to_string())],
            cwd: Some("/tmp/project".to_string()),
            risk_tier: RiskLabel::Medium,
            agent_id: Some("agent-1".to_string()),
        }
    }

    fn temp_foundation_tree() -> (FoundationConfig, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let script = dir
            .path()
            .join(".Foundation")
            .join("Agents")
            .join("phases")
            .join("phase5")
            .join("BuildAgent.sh");
        std::fs::create_dir_all(script.parent().unwrap()).unwrap();
        std::fs::write(&script, "#!/usr/bin/env bash\n").unwrap();
        let cfg = FoundationConfig::new(dir.path());
        (cfg, dir)
    }

    #[test]
    fn translate_cmd_rewrites_program_and_preserves_args() {
        let (cfg, _dir) = temp_foundation_tree();
        let expected_script = cfg.build_agent_script();
        let skill = BuildAgentSkill::new(cfg);
        let cmd = sample_cmd("BuildAgent");

        let translated = skill.translate_cmd(&cmd).unwrap();

        assert_eq!(translated.program, expected_script.to_string_lossy());
        assert_eq!(translated.args, cmd.args);
        assert_eq!(translated.cwd, cmd.cwd);
        assert_eq!(translated.risk_tier, cmd.risk_tier);
        assert_eq!(translated.agent_id, cmd.agent_id);
        assert!(
            translated.env.contains(&(
                "EOCON_FOUNDATION_ROOT".to_string(),
                skill.config.root.to_string_lossy().to_string()
            )),
            "expected EOCON_FOUNDATION_ROOT env var"
        );
        assert!(
            translated.env.contains(&("EXTRA".to_string(), "value".to_string())),
            "expected original env var to be preserved"
        );
    }

    #[test]
    fn translate_cmd_accepts_buildagent_sh() {
        let (cfg, _dir) = temp_foundation_tree();
        let skill = BuildAgentSkill::new(cfg);
        let cmd = sample_cmd("BuildAgent.sh");

        let translated = skill.translate_cmd(&cmd).unwrap();
        assert!(translated.program.ends_with("BuildAgent.sh"));
    }

    #[test]
    fn translate_cmd_replaces_existing_foundation_root_env() {
        let (cfg, _dir) = temp_foundation_tree();
        let skill = BuildAgentSkill::new(cfg);
        let mut cmd = sample_cmd("BuildAgent");
        cmd.env.push(("EOCON_FOUNDATION_ROOT".to_string(), "/old".to_string()));

        let translated = skill.translate_cmd(&cmd).unwrap();
        let values: Vec<_> = translated
            .env
            .iter()
            .filter(|(k, _)| k == "EOCON_FOUNDATION_ROOT")
            .map(|(_, v)| v.clone())
            .collect();
        assert_eq!(values, vec![skill.config.root.to_string_lossy().to_string()]);
    }

    #[test]
    fn translate_cmd_fails_when_script_missing() {
        let cfg = FoundationConfig::new("/opt/does-not-exist");
        let skill = BuildAgentSkill::new(cfg);
        let cmd = sample_cmd("BuildAgent");

        let err = skill.translate_cmd(&cmd).unwrap_err();
        assert!(err.to_string().contains("BuildAgent script not found"));
    }

    #[test]
    fn parse_output_extracts_envelope() {
        let cfg = FoundationConfig::new("/opt/foundation");
        let skill = BuildAgentSkill::new(cfg);
        let raw = r#"{"success":true,"output":"Build succeeded","error":"","duration":12.3,"metadata":{},"exitCode":0}"#;

        let out = skill.parse_output(raw).unwrap();
        assert!(out.success);
        assert_eq!(out.exit_code, 0);
        assert_eq!(out.output, "Build succeeded");
    }
}
