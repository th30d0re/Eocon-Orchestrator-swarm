//! End-to-end test of the CLI control plane: start a headless instance,
//! control it with `eos exec` / sugar subcommands, inspect it with
//! `eos ps` / `eos snapshot`, and shut it down with `/quit`.

use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use assert_cmd::cargo::CommandCargoExt;
use assert_cmd::prelude::*;
use predicates::prelude::*;

struct ServerGuard(Child);

impl Drop for ServerGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn eos(home: &std::path::Path) -> Command {
    let mut cmd = Command::cargo_bin("eos").unwrap();
    cmd.env("HOME", home);
    cmd.current_dir(home);
    cmd
}

#[test]
fn cli_control_plane_end_to_end() {
    let home = tempfile::tempdir().unwrap();

    let mut serve = eos(home.path());
    serve
        .args(["serve", "--name", "test-instance"])
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let child = serve.spawn().unwrap();
    let mut guard = ServerGuard(child);

    // Wait for the instance to register.
    let instances_dir = home.path().join(".eocon/instances");
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        let registered = std::fs::read_dir(&instances_dir)
            .map(|d| d.count() > 0)
            .unwrap_or(false);
        if registered {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "instance did not register within 15s"
        );
        std::thread::sleep(Duration::from_millis(100));
    }

    // ps lists the instance.
    eos(home.path())
        .args(["ps"])
        .assert()
        .success()
        .stdout(predicate::str::contains("test-instance"));

    // exec runs the same slash commands as the TUI.
    eos(home.path())
        .args([
            "exec",
            "--instance",
            "test-instance",
            "/agent spawn worker --role coder",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Spawned agent worker"));

    // Sugar subcommand goes through the identical dispatch path.
    eos(home.path())
        .args(["agent", "-i", "test-instance", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("worker"));

    // Unknown commands surface as failures.
    eos(home.path())
        .args(["exec", "-i", "test-instance", "/bogus"])
        .assert()
        .failure()
        .stdout(predicate::str::contains("Unknown command"));

    // Snapshot returns full JSON state.
    let output = eos(home.path())
        .args(["snapshot", "-i", "test-instance"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let snap: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(snap["name"], "test-instance");
    assert_eq!(snap["mode"], "headless");
    assert_eq!(snap["agents"].as_array().unwrap().len(), 1);
    assert_eq!(snap["agents"][0]["name"], "worker");
    assert_eq!(snap["agents"][0]["role"], "coder");
    // Remote commands are visible in the activity log for the dashboard.
    let recent = serde_json::to_string(&snap["recent"]).unwrap();
    assert!(recent.contains("cli> /agent spawn worker"));

    // Spawning with an unusable provider is loud at spawn time…
    eos(home.path())
        .args([
            "exec",
            "-i",
            "test-instance",
            "/agent spawn doomed --provider nosuchprovider",
        ])
        .assert()
        .failure()
        .stdout(predicate::str::contains("preflight"))
        .stdout(predicate::str::contains("WILL fail"));

    // …and the task failure reason is persisted on the agent, not swallowed.
    // Kill the healthy worker first so `doomed` is the only idle agent and
    // is guaranteed to claim the task.
    eos(home.path())
        .args(["agent", "-i", "test-instance", "kill", "worker"])
        .assert()
        .success();
    eos(home.path())
        .args(["swarm", "-i", "test-instance", "run", "say hello"])
        .assert()
        .success();
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        let output = eos(home.path())
            .args(["agent", "-i", "test-instance", "list"])
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        if stdout.contains("doomed") && stdout.contains("error:") {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "agent failure reason never surfaced in /agent list; last output:\n{}",
            stdout
        );
        std::thread::sleep(Duration::from_millis(200));
    }

    // /trace shows the full failure detail.
    eos(home.path())
        .args(["trace", "-i", "test-instance", "doomed"])
        .assert()
        .success()
        .stdout(predicate::str::contains("last error"))
        .stdout(predicate::str::contains("nosuchprovider"));

    // The traces provider replays a canned session fully offline, driving the
    // whole pipeline to success: task -> response -> gist -> verifier -> metrics.
    let sessions_path = home.path().join("sessions.jsonl");
    std::fs::write(
        &sessions_path,
        serde_json::json!({
            "session_id": "fixture01",
            "task": "say hello",
            "chunks": [
                "Replayed trace fixture01: created hello.py",
                "Wrote hello.py printing the greeting.",
                "Verified by running python hello.py."
            ]
        })
        .to_string()
            + "\n",
    )
    .unwrap();
    eos(home.path())
        .args([
            "exec",
            "-i",
            "test-instance",
            &format!(
                "/agent spawn replaybot --provider traces --model {}",
                sessions_path.display()
            ),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Spawned agent replaybot"));
    eos(home.path())
        .args(["swarm", "-i", "test-instance", "run", "greet the user"])
        .assert()
        .success();
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        let output = eos(home.path())
            .args(["agent", "-i", "test-instance", "list"])
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        if stdout.contains("replaybot") && stdout.contains("Completed") {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "replaybot never completed; last output:\n{}",
            stdout
        );
        std::thread::sleep(Duration::from_millis(200));
    }
    // The replayed response produced a verified gist and a recorded success.
    eos(home.path())
        .args(["gist", "-i", "test-instance", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("verified"))
        .stdout(predicate::str::contains("Replayed trace fixture01"));
    let output = eos(home.path())
        .args(["snapshot", "-i", "test-instance"])
        .output()
        .unwrap();
    let snap: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(snap["metrics"]["successes"], 1);
    assert!(snap["metrics"]["total_tokens"].as_u64().unwrap() > 0);

    // A late --help is a help request, not an agent name.
    eos(home.path())
        .args(["agent", "spawn", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Examples:"));

    // /quit shuts the instance down.
    eos(home.path())
        .args(["quit", "-i", "test-instance"])
        .assert()
        .success();
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Ok(Some(_)) = guard.0.try_wait() {
            break;
        }
        assert!(Instant::now() < deadline, "instance did not exit after /quit");
        std::thread::sleep(Duration::from_millis(100));
    }

    // Registry prunes the dead instance.
    eos(home.path())
        .args(["ps"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No running swarm instances"));
}
