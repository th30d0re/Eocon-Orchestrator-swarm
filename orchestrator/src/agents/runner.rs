use std::io::{Read, Write};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;

use color_eyre::Result;
pub use portable_pty::PtySize;
use portable_pty::{Child, CommandBuilder, MasterPty, NativePtySystem, PtySystem};

use crate::policy::policy::RiskLabel;

#[derive(Debug)]
pub enum RunnerMsg {
    Bytes(Vec<u8>),
    Error(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentCmd {
    pub program: String,
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
    pub cwd: Option<String>,
    pub risk_tier: RiskLabel,
    pub agent_id: Option<String>,
}

pub struct RunHandle {
    pub child: Box<dyn Child + Send>,
    pub master: Box<dyn MasterPty + Send>,
    pub writer: Box<dyn Write + Send>,
    pub rx: Receiver<RunnerMsg>,
    pub machine_name: Option<String>,
    pub dedicated: bool,
    pub container_pid: Option<u32>,
}

impl std::fmt::Debug for RunHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RunHandle").finish_non_exhaustive()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum RunnerError {
    #[error("openpty failed: {0}")]
    OpenPty(String),
    #[error("spawn_command failed: {0}")]
    Spawn(String),
    #[error("take_writer failed: {0}")]
    TakeWriter(String),
    #[error("clone_reader failed: {0}")]
    CloneReader(String),
    #[error("kill failed: {0}")]
    Kill(String),
}

pub trait AgentRunner: Send + Sync {
    fn spawn(&self, cmd: AgentCmd, size: PtySize) -> Result<RunHandle, RunnerError>;
    fn kill(&self, handle: &mut RunHandle) -> Result<(), RunnerError>;
    fn resize(&self, handle: &RunHandle, size: PtySize) -> Result<(), RunnerError> {
        handle
            .master
            .resize(size)
            .map_err(|e| RunnerError::OpenPty(e.to_string()))
    }
}

pub struct PtyRunner;

impl AgentRunner for PtyRunner {
    fn spawn(&self, cmd: AgentCmd, size: PtySize) -> Result<RunHandle, RunnerError> {
        let pty_system = NativePtySystem::default();
        let pair = pty_system
            .openpty(size)
            .map_err(|e| RunnerError::OpenPty(e.to_string()))?;

        let mut cmd_builder = CommandBuilder::new(&cmd.program);
        for arg in &cmd.args {
            cmd_builder.arg(arg);
        }
        for (k, v) in &cmd.env {
            cmd_builder.env(k, v);
        }
        if let Some(ref dir) = cmd.cwd {
            cmd_builder.cwd(dir);
        }
        cmd_builder.env("CLICOLOR_FORCE", "1");
        cmd_builder.env("FORCE_COLOR", "1");
        cmd_builder.env("TERM", "xterm-256color");
        cmd_builder.env("COLORTERM", "truecolor");

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
            let mut buf = [0u8; 8192];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        if tx.send(RunnerMsg::Bytes(buf[..n].to_vec())).is_err() {
                            break;
                        }
                    }
                    Err(e) => {
                        let _ = tx.send(RunnerMsg::Error(format!("PTY read error: {}", e)));
                        break;
                    }
                }
            }
        });

        Ok(RunHandle {
            child,
            master: pair.master,
            writer,
            rx,
            machine_name: None,
            dedicated: false,
            container_pid: None,
        })
    }

    fn kill(&self, handle: &mut RunHandle) -> Result<(), RunnerError> {
        handle
            .child
            .kill()
            .map_err(|e| RunnerError::Kill(e.to_string()))
    }
}

pub fn wait_child(child: &mut Box<dyn Child + Send>) -> Result<i32> {
    let status = child.wait()?;
    Ok(status.exit_code() as i32)
}

pub fn kill_child(child: &mut Box<dyn Child + Send>) -> Result<()> {
    child.kill()?;
    Ok(())
}
