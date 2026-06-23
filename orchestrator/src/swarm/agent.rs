use std::sync::mpsc::Receiver;
use std::time::Instant;

use crate::agents::runner::RunHandle;
use crate::agents::swift::SwiftBuildOutput;
use crate::chat::ChatRequest;
use crate::policy::policy::RiskLabel;
use crate::pty_screen::PtyScreen;
use crate::swarm::memory::AgentMemory;
use crate::swarm::verifier::GistDraft;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentLifecycleState {
    Idle,
    Thinking,
    Acting,
    Paused,
    Failed,
    Completed,
    Preparing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AgentRole {
    #[default]
    Generalist,
    Planner,
    Coder,
    Reviewer,
    Researcher,
    Synthesizer,
    SwiftBuilder,
    SwiftTester,
    RiskGuard,
    Quant,
}

impl AgentRole {
    pub fn display_name(&self) -> &'static str {
        match self {
            AgentRole::Generalist => "generalist",
            AgentRole::Planner => "planner",
            AgentRole::Coder => "coder",
            AgentRole::Reviewer => "reviewer",
            AgentRole::Researcher => "researcher",
            AgentRole::Synthesizer => "synthesizer",
            AgentRole::SwiftBuilder => "swift-builder",
            AgentRole::SwiftTester => "swift-tester",
            AgentRole::RiskGuard => "risk-guard",
            AgentRole::Quant => "quant",
        }
    }

    pub fn default_risk_tier(&self) -> RiskLabel {
        match self {
            AgentRole::Generalist
            | AgentRole::Planner
            | AgentRole::Researcher
            | AgentRole::Synthesizer => RiskLabel::Low,
            AgentRole::Coder | AgentRole::Reviewer => RiskLabel::Medium,
            AgentRole::SwiftBuilder | AgentRole::SwiftTester => RiskLabel::Medium,
            AgentRole::RiskGuard => RiskLabel::High,
            AgentRole::Quant => RiskLabel::Medium,
        }
    }
}

pub struct AgentInstance {
    pub id: String,
    pub name: String,
    pub role: AgentRole,
    pub provider: String,
    pub model: String,
    pub state: AgentLifecycleState,
    pub current_task: Option<String>,
    pub memory: AgentMemory,
    pub output: AgentOutput,
    pub selected: bool,
    pub created_at: Instant,
    pub last_updated: Instant,
    pub risk_tier: RiskLabel,
    pub stall_count: u32,
}

pub enum AgentOutput {
    None,
    Chat(ChatOutput),
    Pty(PtyOutput),
    SwiftResult(SwiftBuildOutput),
}

pub struct ChatOutput {
    pub rx: Option<Receiver<Result<String, String>>>,
    pub pending_response: String,
}

pub struct PtyOutput {
    pub run_handle: Option<RunHandle>,
    pub screen: PtyScreen,
}

impl AgentInstance {
    pub fn new(id: String, name: String, role: AgentRole, provider: String, model: String) -> Self {
        let now = Instant::now();
        let memory = AgentMemory::new(&id, &role);
        Self {
            id,
            name,
            role,
            provider,
            model,
            state: AgentLifecycleState::Idle,
            current_task: None,
            memory,
            output: AgentOutput::None,
            selected: false,
            created_at: now,
            last_updated: now,
            risk_tier: role.default_risk_tier(),
            stall_count: 0,
        }
    }

    pub fn display_name(&self) -> &str {
        &self.name
    }

    pub fn elapsed_secs(&self) -> u64 {
        self.created_at.elapsed().as_secs()
    }

    pub fn set_state(&mut self, state: AgentLifecycleState) {
        self.state = state;
        self.last_updated = Instant::now();
    }

    pub fn spawn_chat(&mut self, task: &str) -> ChatRequest {
        self.current_task = Some(task.to_string());
        self.set_state(AgentLifecycleState::Thinking);
        self.memory.push_user(task.to_string());
        ChatRequest {
            provider: self.provider.clone(),
            model: self.model.clone(),
            messages: self.memory.messages.clone(),
            system_prompt: Some(self.memory.system_prompt.clone()),
        }
    }

    pub fn append_to_memory(&mut self, role: &str, content: String) {
        self.memory.push_raw(role, content);
        self.last_updated = Instant::now();
    }

    pub fn pause(&mut self) {
        self.set_state(AgentLifecycleState::Paused);
        match &mut self.output {
            AgentOutput::Chat(chat) => {
                chat.rx = None;
            }
            AgentOutput::Pty(pty) => {
                if let Some(mut handle) = pty.run_handle.take() {
                    let _ = handle.child.kill();
                }
            }
            AgentOutput::SwiftResult(_) => {}
            AgentOutput::None => {}
        }
    }

    pub fn resume(&mut self) {
        if self.state == AgentLifecycleState::Paused {
            self.set_state(AgentLifecycleState::Idle);
        }
    }

    pub fn edit_instruction(&mut self, text: String) {
        self.memory.push_system_correction(text);
        self.last_updated = Instant::now();
    }

    pub fn restart(&mut self) {
        self.set_state(AgentLifecycleState::Idle);
        self.output = AgentOutput::None;
        self.stall_count = 0;
    }

    pub fn extract_gist(&self) -> Option<GistDraft> {
        GistDraft::extract_from(&self.memory)
    }
}
