use crate::app::{App, InteractionMessage, SwarmPhase};
use crate::swarm::agent::AgentRole;
use crate::ui::theme::Voice;

pub fn dispatch(app: &mut App, cmd: &str) {
    let parts: Vec<&str> = cmd.split_whitespace().collect();
    if parts.is_empty() {
        return;
    }

    match parts[0] {
        "/agent" => handle_agent(app, &parts[1..]),
        "/gist" => handle_gist(app, &parts[1..]),
        "/swarm" => handle_swarm(app, &parts[1..]),
        "/policy" => handle_policy(app),
        "/trace" => handle_trace(app, &parts[1..]),
        "/metrics" => handle_metrics(app),
        "/layout" => app.update(crate::action::Action::ToggleSwarmLayout),
        "/help" => handle_help(app),
        "/quit" => app.should_quit = true,
        _ => {
            app.interaction.push(InteractionMessage {
                voice: Voice::Error,
                text: format!("Unknown command: {}", parts[0]),
            });
        }
    }
}

fn handle_agent(app: &mut App, args: &[&str]) {
    if args.is_empty() {
        reply(app, "Usage: /agent spawn|list|pause|resume|edit|restart|unfold|kill <args>");
        return;
    }
    match args[0] {
        "spawn" => {
            if args.len() < 2 {
                reply(app, "Usage: /agent spawn <name> [--role <role>] [--provider <p>] [--model <m>]");
                return;
            }
            let name = args[1].to_string();
            let mut role = AgentRole::Generalist;
            let mut provider = app.current_provider.clone().unwrap_or_else(|| "anthropic".to_string());
            let mut model = app.current_model.clone().unwrap_or_else(|| "claude-sonnet-4".to_string());
            let mut i = 2;
            while i + 1 < args.len() {
                match args[i] {
                    "--role" => role = parse_role(args[i + 1]),
                    "--provider" => provider = args[i + 1].to_string(),
                    "--model" => model = args[i + 1].to_string(),
                    _ => {}
                }
                i += 2;
            }
            let id = app.swarm_engine.spawn_agent(name.clone(), role, provider, model);
            reply(app, &format!("Spawned agent {} ({})", name, id));
        }
        "list" => {
            if app.swarm_engine.agents.is_empty() {
                reply(app, "No agents.");
            } else {
                let lines: Vec<String> = app.swarm_engine.agents
                    .iter()
                    .map(|a| format!("{} [{}] {:?} - {}", a.name, a.id, a.state, a.current_task.as_deref().unwrap_or("idle")))
                    .collect();
                for line in lines {
                    reply(app, &line);
                }
            }
        }
        "pause" => {
            if args.len() < 2 {
                reply(app, "Usage: /agent pause <name>");
                return;
            }
            if let Some(id) = app.swarm_engine.find_agent_id_by_name(args[1]) {
                app.swarm_engine.pause_agent(&id);
                reply(app, &format!("Paused agent {}", args[1]));
            } else {
                reply(app, &format!("Agent {} not found", args[1]));
            }
        }
        "resume" => {
            if args.len() < 2 {
                reply(app, "Usage: /agent resume <name>");
                return;
            }
            if let Some(id) = app.swarm_engine.find_agent_id_by_name(args[1]) {
                app.swarm_engine.resume_agent(&id);
                reply(app, &format!("Resumed agent {}", args[1]));
            } else {
                reply(app, &format!("Agent {} not found", args[1]));
            }
        }
        "edit" => {
            if args.len() < 3 {
                reply(app, "Usage: /agent edit <name> <instruction>");
                return;
            }
            if let Some(id) = app.swarm_engine.find_agent_id_by_name(args[1]) {
                let instruction = args[2..].join(" ");
                app.swarm_engine.edit_agent(&id, instruction);
                reply(app, &format!("Edited agent {}", args[1]));
            } else {
                reply(app, &format!("Agent {} not found", args[1]));
            }
        }
        "restart" => {
            if args.len() < 2 {
                reply(app, "Usage: /agent restart <name>");
                return;
            }
            if let Some(id) = app.swarm_engine.find_agent_id_by_name(args[1]) {
                app.swarm_engine.restart_agent(&id);
                reply(app, &format!("Restarted agent {}", args[1]));
            } else {
                reply(app, &format!("Agent {} not found", args[1]));
            }
        }
        "kill" => {
            if args.len() < 2 {
                reply(app, "Usage: /agent kill <name>");
                return;
            }
            if let Some(id) = app.swarm_engine.find_agent_id_by_name(args[1]) {
                app.swarm_engine.kill_agent(&id);
                reply(app, &format!("Killed agent {}", args[1]));
            } else {
                reply(app, &format!("Agent {} not found", args[1]));
            }
        }
        "unfold" => {
            if args.len() < 2 {
                reply(app, "Usage: /agent unfold <name>");
                return;
            }
            if let Some(agent) = app.swarm_engine.find_agent_by_name(args[1]) {
                let mut text = format!("Private memory for {}:\n", args[1]);
                for msg in &agent.memory.messages {
                    text.push_str(&format!("{}: {}\n", msg.role, msg.content));
                }
                reply(app, &text);
            } else {
                reply(app, &format!("Agent {} not found", args[1]));
            }
        }
        _ => reply(app, "Unknown /agent subcommand."),
    }
}

fn handle_gist(app: &mut App, args: &[&str]) {
    if args.is_empty() {
        reply(app, "Usage: /gist list|unfold|reject <args>");
        return;
    }
    match args[0] {
        "list" => {
            let lines: Vec<String> = app.swarm_engine.gist_board.gists
                .iter()
                .map(|g| format!("{} [{}] {} - {}", g.id, g.agent_name, g.status_string(), g.claim))
                .collect();
            for line in lines {
                reply(app, &line);
            }
        }
        "unfold" => {
            if args.len() < 2 {
                reply(app, "Usage: /gist unfold <id>");
                return;
            }
            if let Some(text) = app.swarm_engine.gist_board.unfold(args[1], crate::swarm::gist::UnfoldLevel::Evidence) {
                reply(app, &text);
            } else {
                reply(app, &format!("Gist {} not found", args[1]));
            }
        }
        "reject" => {
            if args.len() < 2 {
                reply(app, "Usage: /gist reject <id>");
                return;
            }
            if app.swarm_engine.gist_board.reject(args[1]) {
                reply(app, &format!("Rejected gist {}", args[1]));
            } else {
                reply(app, &format!("Gist {} not found", args[1]));
            }
        }
        _ => reply(app, "Unknown /gist subcommand."),
    }
}

fn handle_swarm(app: &mut App, args: &[&str]) {
    if args.is_empty() {
        reply(app, "Usage: /swarm run|stop|reset|status <args>");
        return;
    }
    match args[0] {
        "run" => {
            let task = args[1..].join(" ");
            if task.is_empty() {
                reply(app, "Usage: /swarm run <task description>");
                return;
            }
            app.swarm_engine.run_swarm_task(&task);
            app.swarm_phase = SwarmPhase::Running;
            reply(app, &format!("Enqueued swarm task: {}", task));
        }
        "stop" => {
            app.swarm_phase = SwarmPhase::Idle;
            reply(app, "Swarm stopped.");
        }
        "reset" => {
            app.swarm_engine = crate::swarm::engine::SwarmEngine::new(app.project_dir.clone());
            app.swarm_phase = SwarmPhase::Idle;
            reply(app, "Swarm reset.");
        }
        "status" => {
            reply(app, &format!("Phase: {:?}, Agents: {}, Verified gists: {}",
                app.swarm_phase,
                app.swarm_engine.agents.len(),
                app.swarm_engine.gist_board.verified().len()
            ));
        }
        _ => reply(app, "Unknown /swarm subcommand."),
    }
}

fn handle_policy(app: &mut App) {
    let global = &app.swarm_engine.policy_engine.global;
    let user = &app.swarm_engine.policy_engine.user;
    let mut text = "Global policy:\n".to_string();
    for p in &global.forbidden_patterns {
        text.push_str(&format!("  - {}\n", p));
    }
    text.push_str(&format!("Max allowed risk: {:?}\n", global.max_allowed_risk));
    text.push_str(&format!("User autonomous risk: {:?}\n", user.max_autonomous_risk));
    reply(app, &text);
}

fn handle_trace(app: &mut App, args: &[&str]) {
    let id = args.first().copied().unwrap_or("root");
    reply(app, &format!("Trace for {}: (in-memory trace tree exported to stderr on exit)", id));
}

fn handle_metrics(app: &mut App) {
    let m = &app.swarm_engine.metrics;
    let text = format!(
        "Success rate: {:.2}\nCompliance rate: {:.2}\nCSR: {:.2}\nMachiavellian Gap: {:.2}\nTokens: {}",
        m.success_rate(),
        m.compliance_rate(),
        m.csr(0.5),
        m.machiavellian_gap(),
        m.total_tokens_consumed
    );
    reply(app, &text);
}

fn handle_help(app: &mut App) {
    for cmd in crate::app::SLASH_COMMANDS {
        reply(app, &format!("{} - {}", cmd.name, cmd.description));
    }
}

fn reply(app: &mut App, text: &str) {
    app.interaction.push(InteractionMessage {
        voice: Voice::Agent,
        text: text.to_string(),
    });
}

fn parse_role(s: &str) -> AgentRole {
    match s.to_lowercase().as_str() {
        "planner" => AgentRole::Planner,
        "coder" => AgentRole::Coder,
        "reviewer" => AgentRole::Reviewer,
        "researcher" => AgentRole::Researcher,
        "synthesizer" => AgentRole::Synthesizer,
        "swift-builder" => AgentRole::SwiftBuilder,
        "swift-tester" => AgentRole::SwiftTester,
        "risk-guard" => AgentRole::RiskGuard,
        "quant" => AgentRole::Quant,
        _ => AgentRole::Generalist,
    }
}
