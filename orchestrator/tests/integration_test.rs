use std::process::Command;

use assert_cmd::prelude::*;
use predicates::prelude::*;

#[test]
fn cli_help_works() {
    let mut cmd = Command::cargo_bin("eocon-swarm").unwrap();
    cmd.arg("--help");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("eocon-swarm"));
}
