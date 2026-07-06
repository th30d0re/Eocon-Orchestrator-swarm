//! CLI entry points for the control plane (`eos ps`, `eos exec`, sugar
//! subcommands). All output goes to stdout; errors return `Err(String)` so
//! `main` can print them and set the exit code.

use std::time::{SystemTime, UNIX_EPOCH};

use crate::control::client;
use crate::control::protocol::ControlRequest;
use crate::control::registry;

pub fn ps(json: bool) -> Result<(), String> {
    let records = registry::list();
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&records).map_err(|e| e.to_string())?
        );
        return Ok(());
    }
    if records.is_empty() {
        println!("No running swarm instances. Start one with `eos` or `eos serve`.");
        return Ok(());
    }
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    println!(
        "{:<24} {:<8} {:<9} {:<10} PROJECT",
        "NAME", "PID", "MODE", "UPTIME"
    );
    for r in records {
        let uptime = now.saturating_sub(r.started_at_epoch);
        println!(
            "{:<24} {:<8} {:<9} {:<10} {}",
            r.name,
            r.pid,
            r.mode,
            fmt_uptime(uptime),
            r.project_dir.display()
        );
    }
    Ok(())
}

/// Send one input line to an instance and print the replies.
pub fn exec(instance: Option<&str>, command: &str, json: bool) -> Result<(), String> {
    let request = ControlRequest::Command {
        command: command.to_string(),
    };
    let (_, response) = client::send_to_instance(instance, &request)?;
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&response).map_err(|e| e.to_string())?
        );
    } else {
        for msg in &response.messages {
            match msg.voice.as_str() {
                "error" => println!("error: {}", msg.text),
                "warning" => println!("warning: {}", msg.text),
                _ => println!("{}", msg.text),
            }
        }
        if let Some(err) = &response.error {
            println!("error: {}", err);
        }
    }
    if response.ok {
        Ok(())
    } else {
        Err(String::new()) // details already printed above
    }
}

pub fn snapshot(instance: Option<&str>) -> Result<(), String> {
    let (_, response) = client::send_to_instance(instance, &ControlRequest::Snapshot)?;
    match response.snapshot {
        Some(snap) => {
            println!(
                "{}",
                serde_json::to_string_pretty(&snap).map_err(|e| e.to_string())?
            );
            Ok(())
        }
        None => Err(response
            .error
            .unwrap_or_else(|| "instance returned no snapshot".to_string())),
    }
}

fn fmt_uptime(secs: u64) -> String {
    if secs >= 3600 {
        format!("{}h{:02}m", secs / 3600, (secs % 3600) / 60)
    } else if secs >= 60 {
        format!("{}m{:02}s", secs / 60, secs % 60)
    } else {
        format!("{}s", secs)
    }
}
