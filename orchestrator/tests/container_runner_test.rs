//! Integration tests for the Apple Container Machine runner backend.
//!
//! These tests require the Apple `container` CLI and its system service to be
//! running. If the CLI is missing, the container-related assertions are skipped
//! and the test passes. The PTY runner test runs regardless.

use std::process::{Command, Stdio};
use std::sync::mpsc::RecvTimeoutError;
use std::time::{Duration, Instant};

use eocon_swarm::agents::runner::{AgentCmd, AgentRunner, PtyRunner, RunnerMsg, PtySize};
use eocon_swarm::agents::sandbox::{ContainerMachineRunner, SandboxConfig};
use eocon_swarm::policy::RiskLabel;

fn container_cli_available() -> bool {
    Command::new("container")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn build_image() -> Result<(), String> {
    let project_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let script = project_dir.join("deploy/run-eocon-agent-runner.sh");
    let output = Command::new(&script)
        .arg("build")
        .current_dir(&project_dir)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .output()
        .map_err(|e| format!("failed to run build script: {e}"))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!("image build failed with status {}", output.status))
    }
}

fn machine_exists(name: &str) -> bool {
    Command::new("container")
        .args(["machine", "inspect", name])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn delete_machine(name: &str) {
    let _ = Command::new("container")
        .args(["machine", "stop", "--name", name])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    let _ = Command::new("container")
        .args(["machine", "delete", "--name", name])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

struct MachineGuard(String);

impl Drop for MachineGuard {
    fn drop(&mut self) {
        delete_machine(&self.0);
    }
}

fn read_output(rx: &std::sync::mpsc::Receiver<RunnerMsg>, timeout: Duration) -> Vec<u8> {
    let deadline = Instant::now() + timeout;
    let mut out = Vec::new();
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        match rx.recv_timeout(remaining) {
            Ok(RunnerMsg::Bytes(buf)) => out.extend(buf),
            Ok(RunnerMsg::Error(e)) => panic!("runner error: {e}"),
            Err(RecvTimeoutError::Timeout) => break,
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }
    out
}

fn pty_size() -> PtySize {
    PtySize {
        rows: 24,
        cols: 80,
        pixel_width: 0,
        pixel_height: 0,
    }
}

#[test]
fn container_runner_lifecycle() {
    if !container_cli_available() {
        eprintln!("skipping container tests: Apple container CLI not installed");
        return;
    }

    // Test 1: build the agent-runner image.
    if let Err(e) = build_image() {
        eprintln!("skipping container tests: image build failed: {e}");
        return;
    }

    let project_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let runner = ContainerMachineRunner::new(SandboxConfig::default(), project_dir.clone());
    let size = pty_size();

    // Remove any pooled machine left over from a previous aborted run.
    delete_machine("eocon-pool");
    let _pool_guard = MachineGuard("eocon-pool".to_string());

    // Test 2: Safe/Low tier uses the pooled machine and survives kill.
    let low_cmd = AgentCmd {
        program: "uname".to_string(),
        args: vec!["-a".to_string()],
        env: vec![],
        cwd: None,
        risk_tier: RiskLabel::Low,
    };

    let mut low_handle = runner.spawn(low_cmd, size).expect("spawn Low agent failed");
    assert_eq!(low_handle.machine_name.as_deref(), Some("eocon-pool"));
    assert!(!low_handle.dedicated);

    let output = read_output(&low_handle.rx, Duration::from_secs(120));
    let text = String::from_utf8_lossy(&output);
    assert!(
        text.contains("Linux"),
        "expected 'Linux' in uname output, got: {text}"
    );

    assert!(
        machine_exists("eocon-pool"),
        "eocon-pool machine should exist after spawn"
    );

    runner.kill(&mut low_handle).expect("kill Low agent failed");

    assert!(
        machine_exists("eocon-pool"),
        "eocon-pool machine should still exist after kill (pooled)"
    );

    // Test 3: Medium tier gets a dedicated machine that is deleted on kill.
    let medium_cmd = AgentCmd {
        program: "uname".to_string(),
        args: vec!["-a".to_string()],
        env: vec![],
        cwd: None,
        risk_tier: RiskLabel::Medium,
    };

    let mut medium_handle = runner.spawn(medium_cmd, size).expect("spawn Medium agent failed");
    let machine_name = medium_handle
        .machine_name
        .clone()
        .expect("dedicated machine name missing");
    assert!(medium_handle.dedicated);
    assert!(machine_name.starts_with("eocon-agent-"));

    // Ensure cleanup even if the assertion below fails.
    let _dedicated_guard = MachineGuard(machine_name.clone());

    let output = read_output(&medium_handle.rx, Duration::from_secs(120));
    let text = String::from_utf8_lossy(&output);
    assert!(
        text.contains("Linux"),
        "expected 'Linux' in uname output, got: {text}"
    );

    assert!(
        machine_exists(&machine_name),
        "dedicated machine {machine_name} should exist after spawn"
    );

    runner.kill(&mut medium_handle).expect("kill Medium agent failed");

    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline && machine_exists(&machine_name) {
        std::thread::sleep(Duration::from_millis(200));
    }
    assert!(
        !machine_exists(&machine_name),
        "dedicated machine {machine_name} should be gone within 10s after kill"
    );
}

#[test]
fn pty_runner_still_works() {
    let runner = PtyRunner;
    let size = pty_size();

    let cmd = AgentCmd {
        program: "echo".to_string(),
        args: vec!["hello-from-pty".to_string()],
        env: vec![],
        cwd: None,
        risk_tier: RiskLabel::Low,
    };

    let mut handle = runner.spawn(cmd, size).expect("PTY spawn failed");
    let output = read_output(&handle.rx, Duration::from_secs(10));
    let text = String::from_utf8_lossy(&output);
    assert!(
        text.contains("hello-from-pty"),
        "expected PTY output to contain 'hello-from-pty', got: {text}"
    );

    runner.kill(&mut handle).expect("PTY kill failed");
}
