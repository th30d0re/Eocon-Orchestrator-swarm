use std::path::PathBuf;

use crate::agents::runner::{AgentCmd, AgentRunner, RunHandle, RunnerError};
use portable_pty::PtySize;
use crate::policy::policy::RiskLabel;

/// v2 placeholder: Apple Container Machine sandbox configuration.
#[derive(Debug, Clone)]
pub struct SandboxConfig {
    pub backend: String,
    pub tier_machine_map: std::collections::HashMap<RiskLabel, String>,
    pub cpus: u32,
    pub memory_gb: u32,
    pub home_mount_readonly: bool,
    pub writable_memory_dir: PathBuf,
}

impl Default for SandboxConfig {
    fn default() -> Self {
        let mut map = std::collections::HashMap::new();
        map.insert(RiskLabel::Safe, "eocon-pool".to_string());
        map.insert(RiskLabel::Low, "eocon-pool".to_string());
        Self {
            backend: "pty".to_string(),
            tier_machine_map: map,
            cpus: 4,
            memory_gb: 8,
            home_mount_readonly: true,
            writable_memory_dir: PathBuf::from(".eocon/memory"),
        }
    }
}

/// v2 placeholder: ContainerMachineRunner will implement AgentRunner.
pub struct ContainerMachineRunner {
    pub config: SandboxConfig,
}

impl AgentRunner for ContainerMachineRunner {
    fn spawn(&self, _cmd: AgentCmd, _size: PtySize) -> Result<RunHandle, RunnerError> {
        Err(RunnerError::Spawn(
            "ContainerMachineRunner is a v2 placeholder".to_string(),
        ))
    }

    fn kill(&self, _handle: &mut RunHandle) -> Result<(), RunnerError> {
        Err(RunnerError::Kill(
            "ContainerMachineRunner is a v2 placeholder".to_string(),
        ))
    }
}
