use std::io::{BufRead, BufReader, Read, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::mpsc::{channel, Receiver};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone)]
pub struct ChatRequest {
    pub provider: String,
    pub model: String,
    pub messages: Vec<ChatMessage>,
    pub system_prompt: Option<String>,
}

/// Spawn a background thread that runs the provider's CLI and returns the
/// assistant's text via a channel.
pub fn spawn_chat_request(req: ChatRequest) -> Receiver<Result<String, String>> {
    let (tx, rx) = channel();
    std::thread::spawn(move || {
        let result = run_cli_chat(req);
        let _ = tx.send(result);
    });
    rx
}

fn run_cli_chat(req: ChatRequest) -> Result<String, String> {
    let cfg = build_cli_config(&req).ok_or_else(|| {
        format!(
            "No CLI tool found for provider '{}'. \
             Available mappings:\n\
               anthropic → claude\n\
               moonshot  → kimi\n\
               openai    → codex\n\
               codex     → codex\n\
               cursor    → cursor\n\
               ollama    → ollama\n\
               google    → gemini\n\
               coreai    → coreai-lm sidecar\n\
               mlx       → localmodel-lm sidecar\n\
             Install the provider's CLI and ensure it's in your PATH.",
            req.provider
        )
    })?;

    let prompt = format_messages(&req.messages);

    let mut args = cfg.args.clone();
    if cfg.prompt_via_arg {
        args.push(prompt.clone());
    }

    let mut cmd = Command::new(&cfg.program);
    cmd.args(&args);
    cmd.stdin(Stdio::piped());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Failed to spawn '{}': {}", cfg.program, e))?;

    if cfg.prompt_via_arg {
        if let Some(stdin) = child.stdin.take() {
            drop(stdin);
        }
    } else {
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(prompt.as_bytes());
            let _ = stdin.flush();
        }
    }

    let stdout = child.stdout.take().ok_or("Failed to capture stdout")?;
    let stderr = child.stderr.take().ok_or("Failed to capture stderr")?;

    let stdout_handle = std::thread::spawn(move || {
        let reader = BufReader::new(stdout);
        let mut out = String::new();
        for line in reader.lines() {
            if let Ok(l) = line {
                out.push_str(&strip_ansi(&l));
                out.push('\n');
            }
        }
        out
    });

    let stderr_handle = std::thread::spawn(move || {
        let mut reader = BufReader::new(stderr);
        let mut err = String::new();
        reader.read_to_string(&mut err).ok();
        err
    });

    let status = child
        .wait()
        .map_err(|e| format!("Failed to wait for '{}': {}", cfg.program, e))?;

    let mut output = stdout_handle.join().unwrap_or_default();
    let stderr_text = stderr_handle.join().unwrap_or_default();

    if let Some(ref path) = cfg.output_file {
        if path.exists() {
            match std::fs::read_to_string(path) {
                Ok(contents) => {
                    output = contents;
                }
                Err(e) => {
                    return Err(format!(
                        "Failed to read output file '{}': {}",
                        path.display(),
                        e
                    ));
                }
            }
            let _ = std::fs::remove_file(path);
        }
    }

    if !status.success() {
        let mut msg = format!("{} exited with status {:?}", cfg.program, status.code());
        let err_clean = strip_ansi(&stderr_text).trim().to_string();
        if !err_clean.is_empty() {
            msg.push_str("\n--- stderr ---\n");
            msg.push_str(&err_clean);
        }
        return Err(msg);
    }

    let result = output.trim_end().to_string();
    if result.is_empty() && !stderr_text.trim().is_empty() {
        return Ok(format!(
            "(no stdout)\n--- stderr ---\n{}",
            strip_ansi(&stderr_text).trim()
        ));
    }

    Ok(result)
}

struct CliConfig {
    program: String,
    args: Vec<String>,
    prompt_via_arg: bool,
    output_file: Option<PathBuf>,
}

fn build_cli_config(req: &ChatRequest) -> Option<CliConfig> {
    let provider = req.provider.as_str();
    let model = req.model.as_str();
    match provider {
        "anthropic" => {
            let exe = find_exe("claude", &["/opt/homebrew/bin/claude"])?;
            let mut args = vec!["-p".into(), "--model".into(), model.into()];
            if let Some(ref system) = req.system_prompt {
                args.push("--system-prompt".into());
                args.push(system.clone());
            }
            Some(CliConfig {
                program: exe,
                args,
                prompt_via_arg: false,
                output_file: None,
            })
        }
        "moonshot" => {
            let exe = find_exe("kimi", &["/opt/homebrew/bin/kimi"])?;
            let empty_mcp = empty_mcp_config_path();
            Some(CliConfig {
                program: exe,
                args: vec![
                    "--quiet".into(),
                    "--mcp-config-file".into(),
                    empty_mcp,
                ],
                prompt_via_arg: false,
                output_file: None,
            })
        }
        "openai" | "codex" => {
            let exe = find_exe("codex", &[
                "/Users/emmanuel/.bun/bin/codex",
                "/opt/homebrew/bin/codex",
            ])?;
            let output_file = std::env::temp_dir()
                .join(format!("eocon-codex-{}.txt", std::process::id()));
            Some(CliConfig {
                program: exe,
                args: vec![
                    "exec".into(),
                    "--skip-git-repo-check".into(),
                    "--color".into(),
                    "never".into(),
                    "--ephemeral".into(),
                    "--output-last-message".into(),
                    output_file.to_string_lossy().into_owned(),
                ],
                prompt_via_arg: true,
                output_file: Some(output_file),
            })
        }
        "cursor" => {
            let exe = find_exe("cursor", &["/Users/emmanuel/.local/bin/cursor"])?;
            Some(CliConfig {
                program: exe,
                args: vec!["agent".into()],
                prompt_via_arg: false,
                output_file: None,
            })
        }
        "ollama" => {
            let exe = find_exe("ollama", &[
                "/usr/local/bin/ollama",
                "/opt/homebrew/bin/ollama",
            ])?;
            Some(CliConfig {
                program: exe,
                args: vec!["run".into(), model.into()],
                prompt_via_arg: false,
                output_file: None,
            })
        }
        "google" => {
            let exe = find_exe("gemini", &["/opt/homebrew/bin/gemini"])?;
            Some(CliConfig {
                program: exe,
                args: vec![],
                prompt_via_arg: false,
                output_file: None,
            })
        }
        "coreai" => {
            let exe = find_exe("coreai-lm", &[])?;
            Some(CliConfig {
                program: exe,
                args: vec!["--backend".into(), "coreai".into()],
                prompt_via_arg: false,
                output_file: None,
            })
        }
        "mlx" => {
            let exe = find_exe("localmodel-lm", &[])?;
            Some(CliConfig {
                program: exe,
                args: vec!["--backend".into(), "mlx".into()],
                prompt_via_arg: false,
                output_file: None,
            })
        }
        _ => None,
    }
}

fn find_exe(name: &str, fallbacks: &[&str]) -> Option<String> {
    if let Ok(output) = Command::new("which").arg(name).output() {
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path.is_empty() {
                return Some(path);
            }
        }
    }
    for path in fallbacks {
        if std::path::Path::new(path).exists() {
            return Some(path.to_string());
        }
    }
    None
}

fn empty_mcp_config_path() -> String {
    let path = std::env::temp_dir().join("eocon-empty-mcp.json");
    if !path.exists() {
        let _ = std::fs::write(&path, r#"{"mcpServers": {}}"#);
    }
    path.to_string_lossy().into_owned()
}

fn format_messages(messages: &[ChatMessage]) -> String {
    let mut parts = Vec::new();
    for msg in messages {
        let label = match msg.role.as_str() {
            "system" => "System",
            "user" => "User",
            "assistant" => "Assistant",
            _ => &msg.role,
        };
        parts.push(format!("{}: {}", label, msg.content));
    }
    parts.join("\n\n")
}

fn strip_ansi(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            for next in chars.by_ref() {
                if ('@'..='~').contains(&next) {
                    break;
                }
            }
        } else {
            result.push(c);
        }
    }
    result
}

/// Build a system prompt for the swarm context.
pub fn build_system_prompt(
    provider: &str,
    model: &str,
    agents_dir: &std::path::Path,
    cwd: &std::path::Path,
) -> String {
    let agents = agents_dir.display().to_string();
    let cwd = cwd.display().to_string();

    let orchestrate_path = agents_dir
        .parent()
        .and_then(|p| p.parent())
        .map(|p| p.join("orchestrators").join("orchestrate.md"))
        .filter(|p| p.is_file());

    let orchestrate_body = orchestrate_path
        .as_deref()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .map(|s| strip_front_matter(&s))
        .unwrap_or_default();

    let runtime_context = format!(
        "You are the Eocon Swarm Orchestrator, operating inside a multi-agent terminal.\n\
         You are a specialist agent with isolated private memory.\n\
         You may ONLY consume verified gists from the shared GistBoard; you must NEVER read \
         another agent's private memory or raw reasoning.\n\
         When you have a finding, compress it into a gist with claim, summary, and evidence.\n\n\
         Current runtime context:\n\
         - Provider: {}\n\
         - Model: {}\n\
         - Agents directory: {}\n\
         - Working directory: {}\n\n",
        provider, model, agents, cwd
    );

    if orchestrate_body.is_empty() {
        runtime_context + "\nNo orchestrate.md prompt was found. Use your built-in knowledge to assist."
    } else {
        runtime_context + &orchestrate_body
    }
}

fn strip_front_matter(text: &str) -> String {
    let trimmed = text.trim_start();
    if !trimmed.starts_with("---") {
        return text.to_string();
    }
    let rest = &trimmed[3..];
    if let Some(end) = rest.find("\n---") {
        let after = &rest[end + 4..];
        return after.trim_start().to_string();
    }
    text.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn strip_front_matter_removes_yaml_header() {
        let input = "---\nname: foo\ndescription: bar\n---\n# Heading\nbody";
        let result = strip_front_matter(input);
        assert_eq!(result, "# Heading\nbody");
    }

    #[test]
    fn build_system_prompt_loads_orchestrate_md() {
        let temp = tempfile::tempdir().unwrap();
        let agents_dir = temp.path().join("phases").join("phase5");
        let orchestrators_dir = temp.path().join("orchestrators");
        std::fs::create_dir_all(&agents_dir).unwrap();
        std::fs::create_dir_all(&orchestrators_dir).unwrap();

        let orchestrate_md = orchestrators_dir.join("orchestrate.md");
        let mut f = std::fs::File::create(&orchestrate_md).unwrap();
        write!(
            f,
            "---\nname: test-orchestrator\n---\n# Test Orchestrator\nYou are in charge."
        )
        .unwrap();

        let prompt = build_system_prompt("anthropic", "claude-sonnet-4", &agents_dir, temp.path());
        assert!(prompt.contains("Eocon Swarm Orchestrator"));
        assert!(prompt.contains("You are in charge."));
        assert!(!prompt.contains("name: test-orchestrator"));
    }
}
