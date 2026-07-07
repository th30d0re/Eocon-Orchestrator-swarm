use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::Command;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Mutex;
use std::thread;

use portable_pty::{CommandBuilder, NativePtySystem, PtySize, PtySystem};

use crate::agents::runner::{AgentCmd, AgentRunner, RunnerError, RunnerMsg, RunHandle};
use crate::policy::policy::RiskLabel;

/// Apple Container Machine sandbox configuration.
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
            backend: "container".to_string(),
            tier_machine_map: map,
            cpus: 4,
            memory_gb: 8,
            home_mount_readonly: true,
            writable_memory_dir: PathBuf::from(".eocon/memory"),
        }
    }
}

/// AgentRunner backend that executes commands inside Apple Container Machines.
pub struct ContainerMachineRunner {
    pub config: SandboxConfig,
    pub project_dir: PathBuf,
    pub image_name: String,
    /// Tracks whether the shared pooled machine has already been ensured.
    pub pooled_machine: Mutex<Option<String>>,
}

impl ContainerMachineRunner {
    pub fn new(config: SandboxConfig, project_dir: PathBuf) -> Self {
        Self {
            config,
            project_dir,
            image_name: "eocon-agent-runner:latest".to_string(),
            pooled_machine: Mutex::new(None),
        }
    }

    /// Resolve the target machine name and whether it is dedicated from the
    /// command's risk tier. Tiers present in `tier_machine_map` share the pooled
    /// machine; all others get a dedicated `eocon-agent-<uuid>` machine.
    pub(crate) fn resolve_machine(&self, cmd: &AgentCmd) -> (String, bool) {
        if let Some(name) = self.config.tier_machine_map.get(&cmd.risk_tier) {
            (name.clone(), false)
        } else {
            (format!("eocon-agent-{}", uuid::Uuid::new_v4()), true)
        }
    }

    /// Build the arguments for `container machine run` without mutating state.
    pub(crate) fn build_run_args(&self, cmd: &AgentCmd, machine_name: &str) -> Vec<String> {
        let mut args = vec![
            "machine".to_string(),
            "run".to_string(),
            "--name".to_string(),
            machine_name.to_string(),
            "-w".to_string(),
            self.project_dir.to_string_lossy().to_string(),
        ];
        for (k, v) in &cmd.env {
            args.push("-e".to_string());
            args.push(format!("{}={}", k, v));
        }
        args.push("--".to_string());
        args.push(cmd.program.clone());
        args.extend(cmd.args.iter().cloned());
        args
    }

    fn ensure_image(&self) -> Result<(), RunnerError> {
        let output = Command::new("container")
            .args(["image", "inspect", &self.image_name])
            .output()
            .map_err(|e| {
                RunnerError::Spawn(format!(
                    "failed to inspect image {}: {}. Is the Apple container CLI installed?",
                    self.image_name, e
                ))
            })?;
        if output.status.success() {
            Ok(())
        } else {
            Err(RunnerError::Spawn(format!(
                "image {} not found; build it with ./deploy/run-eocon-agent-runner.sh build\n{}",
                self.image_name,
                String::from_utf8_lossy(&output.stderr)
            )))
        }
    }

    fn ensure_machine(&self, name: &str, dedicated: bool) -> Result<(), RunnerError> {
        if !dedicated {
            let mut pooled = self.pooled_machine.lock().unwrap();
            if pooled.is_some() {
                return Ok(());
            }
            if !self.machine_exists(name)? {
                self.create_machine(name)?;
                self.set_machine_resources(name)?;
            }
            *pooled = Some(name.to_string());
        } else {
            if !self.machine_exists(name)? {
                self.create_machine(name)?;
                self.set_machine_resources(name)?;
            }
        }
        Ok(())
    }

    fn machine_exists(&self, name: &str) -> Result<bool, RunnerError> {
        let output = Command::new("container")
            .args(["machine", "inspect", name])
            .output()
            .map_err(|e| {
                RunnerError::Spawn(format!(
                    "failed to inspect machine {}: {}. Is the Apple container CLI installed?",
                    name, e
                ))
            })?;
        if output.status.success() {
            Ok(true)
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr).to_lowercase();
            if stderr.contains("not found") || stderr.contains("does not exist") {
                Ok(false)
            } else {
                Err(RunnerError::Spawn(format!(
                    "container machine inspect {} failed: {}",
                    name,
                    String::from_utf8_lossy(&output.stderr)
                )))
            }
        }
    }

    fn create_machine(&self, name: &str) -> Result<(), RunnerError> {
        let status = Command::new("container")
            .args([
                "machine",
                "create",
                "--name",
                name,
                "--set-default",
                &self.image_name,
            ])
            .status()
            .map_err(|e| RunnerError::Spawn(format!("failed to create machine {}: {}", name, e)))?;
        if !status.success() {
            return Err(RunnerError::Spawn(format!(
                "container machine create {} failed",
                name
            )));
        }
        Ok(())
    }

    fn set_machine_resources(&self, name: &str) -> Result<(), RunnerError> {
        let memory = format!("{}G", self.config.memory_gb);
        let status = Command::new("container")
            .args([
                "machine",
                "set",
                "--name",
                name,
                &format!("cpus={}", self.config.cpus),
                &format!("memory={}", memory),
                &format!(
                    "home-mount={}",
                    if self.config.home_mount_readonly {
                        "ro"
                    } else {
                        "rw"
                    }
                ),
            ])
            .status()
            .map_err(|e| RunnerError::Spawn(format!("failed to set machine {}: {}", name, e)))?;
        if !status.success() {
            return Err(RunnerError::Spawn(format!(
                "container machine set {} failed",
                name
            )));
        }
        Ok(())
    }

    fn delete_machine(&self, name: &str) -> Result<(), RunnerError> {
        let _ = Command::new("container")
            .args(["machine", "stop", name])
            .status();
        let status = Command::new("container")
            .args(["machine", "delete", name])
            .status()
            .map_err(|e| RunnerError::Kill(format!("failed to delete machine {}: {}", name, e)))?;
        if !status.success() {
            return Err(RunnerError::Kill(format!(
                "container machine delete {} failed",
                name
            )));
        }
        let start = std::time::Instant::now();
        while start.elapsed().as_secs() < 10 {
            match self.machine_exists(name) {
                Ok(false) => return Ok(()),
                Ok(true) => thread::sleep(std::time::Duration::from_millis(200)),
                Err(e) => {
                    // Inspect errors during polling are usually because the
                    // machine is gone but the CLI still reports transient state.
                    if e.to_string().to_lowercase().contains("not found") {
                        return Ok(());
                    }
                    thread::sleep(std::time::Duration::from_millis(200));
                }
            }
        }
        Err(RunnerError::Kill(format!(
            "machine {} still present after delete",
            name
        )))
    }
}

impl AgentRunner for ContainerMachineRunner {
    fn spawn(&self, cmd: AgentCmd, size: PtySize) -> Result<RunHandle, RunnerError> {
        let (machine_name, dedicated) = self.resolve_machine(&cmd);

        self.ensure_image()?;
        self.ensure_machine(&machine_name, dedicated)?;

        let run_args = self.build_run_args(&cmd, &machine_name);

        let pty_system = NativePtySystem::default();
        let pair = pty_system
            .openpty(size)
            .map_err(|e| RunnerError::OpenPty(e.to_string()))?;

        let mut cmd_builder = CommandBuilder::new("container");
        for arg in &run_args {
            cmd_builder.arg(arg);
        }
        if let Some(ref dir) = cmd.cwd {
            cmd_builder.cwd(dir);
        }

        let child = pair
            .slave
            .spawn_command(cmd_builder)
            .map_err(|e| RunnerError::Spawn(e.to_string()))?;
        drop(pair.slave);

        let writer = pair
            .master
            .take_writer()
            .map_err(|e| RunnerError::TakeWriter(e.to_string()))?;

        let (tx, rx): (Sender<RunnerMsg>, Receiver<RunnerMsg>) = channel();

        let mut reader = pair
            .master
            .try_clone_reader()
            .map_err(|e| RunnerError::CloneReader(e.to_string()))?;
        thread::spawn(move || {
            let mut buf_reader = BufReader::new(&mut reader);
            let mut buf = Vec::new();
            loop {
                buf.clear();
                match buf_reader.read_until(b'\n', &mut buf) {
                    Ok(0) => break,
                    Ok(_) => {
                        if tx.send(RunnerMsg::Bytes(buf.clone())).is_err() {
                            break;
                        }
                    }
                    Err(e) => {
                        let _ = tx.send(RunnerMsg::Error(format!("Container read error: {}", e)));
                        break;
                    }
                }
            }
        });

        let container_pid = child.process_id();
        Ok(RunHandle {
            child,
            master: pair.master,
            writer,
            rx,
            machine_name: Some(machine_name),
            dedicated,
            container_pid,
        })
    }

    fn kill(&self, handle: &mut RunHandle) -> Result<(), RunnerError> {
        handle
            .child
            .kill()
            .map_err(|e| RunnerError::Kill(e.to_string()))?;

        if handle.dedicated {
            if let Some(ref name) = handle.machine_name {
                self.delete_machine(name)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_cmd(risk_tier: RiskLabel) -> AgentCmd {
        AgentCmd {
            program: "bash".to_string(),
            args: vec!["-c".to_string(), "echo hi".to_string()],
            env: vec![("FOO".to_string(), "bar".to_string())],
            cwd: None,
            risk_tier,
        }
    }

    #[test]
    fn resolve_pooled_tier() {
        let runner = ContainerMachineRunner::new(SandboxConfig::default(), PathBuf::from("/tmp/project"));
        let cmd = sample_cmd(RiskLabel::Low);
        let (name, dedicated) = runner.resolve_machine(&cmd);
        assert_eq!(name, "eocon-pool");
        assert!(!dedicated);
    }

    #[test]
    fn resolve_dedicated_tier() {
        let runner = ContainerMachineRunner::new(SandboxConfig::default(), PathBuf::from("/tmp/project"));
        let cmd = sample_cmd(RiskLabel::Medium);
        let (name, dedicated) = runner.resolve_machine(&cmd);
        assert!(name.starts_with("eocon-agent-"));
        assert!(dedicated);

        let cmd = sample_cmd(RiskLabel::High);
        let (name, dedicated) = runner.resolve_machine(&cmd);
        assert!(name.starts_with("eocon-agent-"));
        assert!(dedicated);

        let cmd = sample_cmd(RiskLabel::Critical);
        let (name, dedicated) = runner.resolve_machine(&cmd);
        assert!(name.starts_with("eocon-agent-"));
        assert!(dedicated);
    }

    #[test]
    fn build_run_command_includes_name_and_workdir() {
        let runner = ContainerMachineRunner::new(SandboxConfig::default(), PathBuf::from("/tmp/project"));
        let cmd = sample_cmd(RiskLabel::Low);
        let args = runner.build_run_args(&cmd, "eocon-pool");

        assert!(args.contains(&"machine".to_string()));
        assert!(args.contains(&"run".to_string()));
        assert!(args.contains(&"--name".to_string()));
        assert!(args.contains(&"eocon-pool".to_string()));
        assert!(args.contains(&"-w".to_string()));
        assert!(args.contains(&"/tmp/project".to_string()));
        assert!(args.contains(&"-e".to_string()));
        assert!(args.contains(&"FOO=bar".to_string()));
        assert!(args.contains(&"--".to_string()));
        assert!(args.contains(&"bash".to_string()));
        assert!(args.contains(&"echo hi".to_string()));
    }

    #[test]
    fn build_run_command_escapes_env_values() {
        let runner = ContainerMachineRunner::new(SandboxConfig::default(), PathBuf::from("/tmp/project"));
        let cmd = AgentCmd {
            program: "bash".to_string(),
            args: vec![],
            env: vec![("KEY".to_string(), "value with spaces".to_string())],
            cwd: None,
            risk_tier: RiskLabel::Safe,
        };
        let args = runner.build_run_args(&cmd, "eocon-pool");
        assert!(args.contains(&"KEY=value with spaces".to_string()));
    }
}
