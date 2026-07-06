use std::process::Command;

use assert_cmd::prelude::*;
use predicates::prelude::*;

#[test]
fn cli_help_works() {
    let mut cmd = Command::cargo_bin("eos").unwrap();
    cmd.arg("--help");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("swarm orchestrator"));
}
