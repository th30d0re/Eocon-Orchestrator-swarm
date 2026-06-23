use crate::app::{App, InteractionMessage, SwarmPhase};
use crate::swarm::agent::{AgentLifecycleState, AgentRole};
use crate::swarm::gist::{GistSource, GistStatus};
use crate::ui::theme::Voice;

pub fn seed_interaction(app: &mut App) {
    app.interaction.push(InteractionMessage {
        voice: Voice::Agent,
        text: "Eocon Swarm ready (mock mode).".to_string(),
    });
    app.interaction.push(InteractionMessage {
        voice: Voice::System,
        text: "Synthetic agents and gists loaded.".to_string(),
    });

    let planner = app.swarm_engine.spawn_agent(
        "planner".to_string(),
        AgentRole::Planner,
        "anthropic".to_string(),
        "claude-sonnet-4".to_string(),
    );
    let coder = app.swarm_engine.spawn_agent(
        "coder".to_string(),
        AgentRole::Coder,
        "anthropic".to_string(),
        "claude-sonnet-4".to_string(),
    );
    let reviewer = app.swarm_engine.spawn_agent(
        "reviewer".to_string(),
        AgentRole::Reviewer,
        "anthropic".to_string(),
        "claude-sonnet-4".to_string(),
    );

    if let Some(a) = app.swarm_engine.get_agent_mut(&planner) {
        a.set_state(AgentLifecycleState::Thinking);
        a.current_task = Some("Decompose auth module design".to_string());
    }
    if let Some(a) = app.swarm_engine.get_agent_mut(&coder) {
        a.set_state(AgentLifecycleState::Acting);
        a.current_task = Some("Implement login flow".to_string());
    }
    if let Some(a) = app.swarm_engine.get_agent_mut(&reviewer) {
        a.set_state(AgentLifecycleState::Idle);
        a.current_task = Some("Await code for review".to_string());
    }

    app.swarm_engine.gist_board.add(crate::swarm::gist::Gist {
        schema_version: 1,
        id: "gist-1".to_string(),
        agent_id: planner.clone(),
        agent_name: "planner".to_string(),
        claim: "Auth module should use token-based sessions.".to_string(),
        summary: "REST-first, cookie-backed session design.".to_string(),
        evidence: vec![
            "Existing API uses JWT".to_string(),
            "OAuth2 is out of scope".to_string(),
        ],
        source_type: GistSource::ModelClaim,
        peer_rank: Some(0.92),
        status: GistStatus::Verified,
        created_at: std::time::Instant::now(),
    });

    app.swarm_engine.select_agent(0);
}

pub fn update_mock(app: &mut App) {
    app.tick_count += 1;

    if app.tick_count % 60 == 0 {
        app.swarm_phase = SwarmPhase::Running;
    }

    for (idx, agent) in app.swarm_engine.agents.iter_mut().enumerate() {
        if idx % 3 == (app.tick_count as usize / 30) % 3 {
            agent.set_state(AgentLifecycleState::Thinking);
        } else if idx % 3 == (app.tick_count as usize / 30 + 1) % 3 {
            agent.set_state(AgentLifecycleState::Acting);
        } else {
            agent.set_state(AgentLifecycleState::Idle);
        }
    }
}
