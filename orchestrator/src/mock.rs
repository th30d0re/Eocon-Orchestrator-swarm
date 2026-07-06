use std::sync::mpsc::{channel, Receiver};
use std::time::Duration;

use crate::app::{App, InteractionMessage, SwarmPhase};
use crate::chat::ChatEvent;
use crate::swarm::agent::{AgentLifecycleState, AgentOutput, AgentRole, ChatOutput};
use crate::swarm::gist::{GistSource, GistStatus};
use crate::ui::theme::Voice;

/// Synthetic assistant replies streamed in demo mode so the streaming path can be
/// exercised without a real provider.
const MOCK_STREAM_RESPONSES: &[&str] = &[
    "Decomposing the auth module into token issuance, session storage, and refresh rotation. \
     Each concern gets its own gist so reviewers can verify them independently.",
    "Implementing the login flow now: validate credentials, mint a short-lived access token, \
     and persist a rotating refresh token keyed to the device fingerprint.",
    "Reviewing the diff. The happy path looks correct; flagging one race on concurrent refresh \
     where two requests could both rotate the token. Recommending a compare-and-swap guard.",
];

/// Spawn a background thread that emits `text` word-by-word as `ChatEvent::Chunk`s
/// (with a small delay between each) then `ChatEvent::Done`. This mirrors the shape
/// of `chat::spawn_chat_request` but is fully synthetic — no provider, no API key —
/// so demo mode can drive the real `drain_chat_responses` streaming path.
fn spawn_mock_chat_stream(text: &str) -> Receiver<ChatEvent> {
    let (tx, rx) = channel();
    let owned = text.to_string();
    std::thread::spawn(move || {
        for chunk in owned.split_inclusive(' ') {
            if tx.send(ChatEvent::Chunk(chunk.to_string())).is_err() {
                return; // receiver dropped (e.g. app quit)
            }
            std::thread::sleep(Duration::from_millis(60));
        }
        let _ = tx.send(ChatEvent::Done);
    });
    rx
}

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
        "claude-sonnet-4-6".to_string(),
    );
    let coder = app.swarm_engine.spawn_agent(
        "coder".to_string(),
        AgentRole::Coder,
        "anthropic".to_string(),
        "claude-sonnet-4-6".to_string(),
    );
    let reviewer = app.swarm_engine.spawn_agent(
        "reviewer".to_string(),
        AgentRole::Reviewer,
        "anthropic".to_string(),
        "claude-sonnet-4-6".to_string(),
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

    // Drive the streaming feature in demo mode. Every ~200 ticks, if no agent is
    // currently streaming, attach a fresh synthetic token stream to a rotating
    // agent. The existing `App::drain_chat_responses` then consumes it through the
    // real path (chunk accumulation, chat-panel mirroring, finalize-to-memory,
    // gist publish) so the feature is observable without any provider.
    let anyone_streaming = app
        .swarm_engine
        .agents
        .iter()
        .any(|a| matches!(&a.output, AgentOutput::Chat(chat) if chat.rx.is_some()));

    if !anyone_streaming
        && app.tick_count % 200 == 0
        && !app.swarm_engine.agents.is_empty()
    {
        let cycle = app.tick_count as usize / 200;
        let which = cycle % app.swarm_engine.agents.len();
        let id = app.swarm_engine.agents[which].id.clone();
        let text = MOCK_STREAM_RESPONSES[cycle % MOCK_STREAM_RESPONSES.len()];
        if let Some(agent) = app.swarm_engine.get_agent_mut(&id) {
            agent.set_state(AgentLifecycleState::Acting);
            agent.current_task = Some("Streaming synthetic response".to_string());
            agent.output = AgentOutput::Chat(ChatOutput {
                rx: Some(spawn_mock_chat_stream(text)),
                pending_response: String::new(),
            });
        }
    }

    // Cycle agent states for visual life — but leave any agent with a live chat
    // stream alone so the animator doesn't fight the demo stream.
    for (idx, agent) in app.swarm_engine.agents.iter_mut().enumerate() {
        if matches!(&agent.output, AgentOutput::Chat(chat) if chat.rx.is_some()) {
            continue;
        }
        if idx % 3 == (app.tick_count as usize / 30) % 3 {
            agent.set_state(AgentLifecycleState::Thinking);
        } else if idx % 3 == (app.tick_count as usize / 30 + 1) % 3 {
            agent.set_state(AgentLifecycleState::Acting);
        } else {
            agent.set_state(AgentLifecycleState::Idle);
        }
    }
}
