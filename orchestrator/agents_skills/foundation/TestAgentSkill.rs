use std::path::PathBuf;

use crate::agents::runner::AgentCmd;
use crate::agents_skills::foundation::config::FoundationConfig;
use crate::agents_skills::foundation::FoundationAgentOutput;

/// Errors that can occur when resolving or translating a Foundation TestAgent
/// skill invocation.
#[derive(Debug, thiserror::Error)]
pub enum FoundationSkillError {
    #[error("Foundation root does not exist: {0}")]
    RootNotFound(PathBuf),
    #[error("TestAgent script not found or not executable: {0}")]
    ScriptNotFound(PathBuf),
}

/// Reusable wrapper around the Foundation Phase 5 `testAgent.sh` script.
#[derive(Debug, Clone)]
pub struct TestAgentSkill {
    pub config: FoundationConfig,
}

impl TestAgentSkill {
    pub fn new(config: FoundationConfig) -> Self {
        Self { config }
    }

    /// Resolve the absolute path to `testAgent.sh`.
    pub fn script_path(&self) -> PathBuf {
        self.config.test_agent_script()
    }

    /// Translate a generic `AgentCmd` into a Foundation TestAgent invocation.
    ///
    /// Recognized program names: `TestAgent` and `TestAgent.sh`. The returned
    /// command uses the resolved script path as its program, preserves the
    /// original arguments, and injects `EOCON_FOUNDATION_ROOT` into the
    /// environment.
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

    /// Parse the last JSON envelope from raw agent output.
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
                "run auth tests".to_string(),
            ],
            env: vec![],
            cwd: None,
            risk_tier: RiskLabel::Medium,
            agent_id: None,
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
            .join("testAgent.sh");
        std::fs::create_dir_all(script.parent().unwrap()).unwrap();
        std::fs::write(&script, "#!/usr/bin/env bash\n").unwrap();
        let cfg = FoundationConfig::new(dir.path());
        (cfg, dir)
    }

    #[test]
    fn translate_cmd_rewrites_program_and_preserves_args() {
        let (cfg, _dir) = temp_foundation_tree();
        let expected_script = cfg.test_agent_script();
        let skill = TestAgentSkill::new(cfg);
        let cmd = sample_cmd("TestAgent");

        let translated = skill.translate_cmd(&cmd).unwrap();

        assert_eq!(translated.program, expected_script.to_string_lossy());
        assert_eq!(translated.args, cmd.args);
        assert!(
            translated.env.contains(&(
                "EOCON_FOUNDATION_ROOT".to_string(),
                skill.config.root.to_string_lossy().to_string()
            ))
        );
    }

    #[test]
    fn translate_cmd_accepts_testagent_sh() {
        let (cfg, _dir) = temp_foundation_tree();
        let skill = TestAgentSkill::new(cfg);
        let cmd = sample_cmd("TestAgent.sh");

        let translated = skill.translate_cmd(&cmd).unwrap();
        assert!(translated.program.ends_with("testAgent.sh"));
    }

    #[test]
    fn translate_cmd_fails_when_script_missing() {
        let cfg = FoundationConfig::new("/opt/does-not-exist");
        let skill = TestAgentSkill::new(cfg);
        let cmd = sample_cmd("TestAgent");

        let err = skill.translate_cmd(&cmd).unwrap_err();
        assert!(err.to_string().contains("TestAgent script not found"));
    }

    #[test]
    fn parse_output_extracts_envelope() {
        let cfg = FoundationConfig::new("/opt/foundation");
        let skill = TestAgentSkill::new(cfg);
        let raw = r#"{"success":false,"output":"","error":"No simulator specified","duration":1.2,"metadata":{},"exitCode":11}"#;

        let out = skill.parse_output(raw).unwrap();
        assert!(!out.success);
        assert_eq!(out.exit_code, 11);
        assert_eq!(out.error, "No simulator specified");
    }
}
