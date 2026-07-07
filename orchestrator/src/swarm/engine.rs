use std::path::PathBuf;
use std::sync::Arc;

use crate::agents::runner::{AgentCmd, AgentRunner, PtySize};
use crate::chat::spawn_chat_request;
use crate::memory::store::MemoryStore;
use crate::observability::metrics::SwarmMetrics;
use crate::policy::engine::PolicyEngine;
use crate::pty_screen::PtyScreen;
use crate::swarm::agent::{
    AgentInstance, AgentLifecycleState, AgentOutput, AgentRole, ChatOutput, PtyOutput,
};
use crate::swarm::gist::{Gist, GistBoard, GistStatus};
use crate::swarm::queue::{Task, TaskQueue};
use crate::swarm::verifier::{draft_to_gist, verify};

pub struct SwarmEngine {
    pub agents: Vec<AgentInstance>,
    pub gist_board: GistBoard,
    pub task_queue: TaskQueue,
    pub policy_engine: PolicyEngine,
    pub runner: Arc<dyn AgentRunner>,
    pub project_dir: PathBuf,
    pub metrics: SwarmMetrics,
    pub memory_store: MemoryStore,
}

impl SwarmEngine {
    pub fn new(
        project_dir: PathBuf,
        runner: Arc<dyn AgentRunner>,
        memory_store: MemoryStore,
    ) -> Self {
        Self {
            agents: Vec::new(),
            gist_board: GistBoard::new(),
            task_queue: TaskQueue::new(),
            policy_engine: PolicyEngine::default(),
            runner,
            project_dir,
            metrics: SwarmMetrics::default(),
            memory_store,
        }
    }

    pub fn with_policy(mut self, policy: PolicyEngine) -> Self {
        self.policy_engine = policy;
        self
    }

    pub fn spawn_agent(
        &mut self,
        name: String,
        role: AgentRole,
        provider: String,
        model: String,
    ) -> String {
        let id = uuid::Uuid::new_v4().to_string();
        let agent = AgentInstance::new(
            id.clone(),
            name,
            role,
            provider,
            model,
            self.memory_store.clone(),
        );
        self.agents.push(agent);
        id
    }

    pub fn pause_agent(&mut self, id: &str) -> bool {
        if let Some(a) = self.agents.iter_mut().find(|a| a.id == id) {
            a.pause(self.runner.as_ref());
            true
        } else {
            false
        }
    }

    pub fn resume_agent(&mut self, id: &str) -> bool {
        if let Some(a) = self.agents.iter_mut().find(|a| a.id == id) {
            a.resume();
            true
        } else {
            false
        }
    }

    pub fn edit_agent(&mut self, id: &str, text: String) -> bool {
        if let Some(a) = self.agents.iter_mut().find(|a| a.id == id) {
            a.edit_instruction(text);
            true
        } else {
            false
        }
    }

    pub fn restart_agent(&mut self, id: &str) -> bool {
        if let Some(a) = self.agents.iter_mut().find(|a| a.id == id) {
            a.restart();
            true
        } else {
            false
        }
    }

    pub fn kill_agent(&mut self, id: &str) -> bool {
        if let Some(a) = self.agents.iter_mut().find(|a| a.id == id) {
            a.set_state(AgentLifecycleState::Failed);
            if let AgentOutput::Pty(pty) = &mut a.output {
                if let Some(mut handle) = pty.run_handle.take() {
                    let _ = self.runner.kill(&mut handle);
                }
            }
            true
        } else {
            false
        }
    }

    pub fn get_agent(&self, id: &str) -> Option<&AgentInstance> {
        self.agents.iter().find(|a| a.id == id)
    }

    pub fn get_agent_mut(&mut self, id: &str) -> Option<&mut AgentInstance> {
        self.agents.iter_mut().find(|a| a.id == id)
    }

    pub fn find_agent_by_name(&self, name: &str) -> Option<&AgentInstance> {
        self.agents.iter().find(|a| a.name == name)
    }

    pub fn find_agent_id_by_name(&self, name: &str) -> Option<String> {
        self.find_agent_by_name(name).map(|a| a.id.clone())
    }

    pub fn run_swarm_task(&mut self, description: &str) {
        let id = uuid::Uuid::new_v4().to_string();
        self.task_queue.enqueue(Task {
            id,
            description: description.to_string(),
            claimed_by: None,
            source_agent: None,
        });
    }

    pub fn assign_tasks(&mut self) {
        for agent in self.agents.iter_mut() {
            if agent.state != AgentLifecycleState::Idle {
                continue;
            }
            if let Some(task) = self.task_queue.claim_next(&agent.id) {
                agent.current_task = Some(task.description.clone());
                let req = agent.spawn_chat(&task.description);
                let rx = spawn_chat_request(req);
                agent.output = AgentOutput::Chat(ChatOutput {
                    rx: Some(rx),
                    pending_response: String::new(),
                });
            }
        }
    }

    pub fn publish_gist(&mut self, agent_id: &str) -> Option<Gist> {
        let agent = self.agents.iter().find(|a| a.id == agent_id)?;
        let draft = agent.extract_gist()?;
        let mut gist = draft_to_gist(draft, agent.id.clone(), agent.name.clone());
        gist.status = GistStatus::Verifying;

        let decision = self.policy_engine.classify_and_act(&gist);
        if decision.action == crate::policy::policy::ActionMode::Reject {
            gist.status = GistStatus::Rejected;
            self.gist_board.add(gist);
            return None;
        }

        if verify(&crate::swarm::verifier::GistDraft {
            claim: gist.claim.clone(),
            summary: gist.summary.clone(),
            evidence: gist.evidence.clone(),
            source_type: gist.source_type,
        }) == GistStatus::Verified
        {
            gist.status = GistStatus::Verified;
            self.gist_board.add(gist.clone());
            Some(gist)
        } else {
            gist.status = GistStatus::Rejected;
            self.gist_board.add(gist);
            None
        }
    }

    pub fn spawn_pty_agent(
        &mut self,
        id: &str,
        mut cmd: AgentCmd,
        size: PtySize,
    ) -> Result<(), String> {
        let agent = self
            .agents
            .iter_mut()
            .find(|a| a.id == id)
            .ok_or("agent not found")?;
        cmd.agent_id = Some(id.to_string());
        let handle = self.runner.spawn(cmd, size).map_err(|e| e.to_string())?;
        let mut screen = PtyScreen::new(size.rows as usize, size.cols as usize);
        screen.resize(size.rows as usize, size.cols as usize);
        agent.output = AgentOutput::Pty(PtyOutput {
            run_handle: Some(handle),
            screen,
        });
        agent.set_state(AgentLifecycleState::Acting);
        Ok(())
    }

    pub fn selected_agent_index(&self) -> usize {
        self.agents
            .iter()
            .position(|a| a.selected)
            .unwrap_or(0)
            .min(self.agents.len().saturating_sub(1))
    }

    pub fn select_agent(&mut self, idx: usize) {
        let idx = idx.min(self.agents.len().saturating_sub(1));
        for (i, a) in self.agents.iter_mut().enumerate() {
            a.selected = i == idx;
        }
    }

    pub fn select_next(&mut self) {
        let idx = self.selected_agent_index();
        let next = if self.agents.is_empty() {
            0
        } else {
            (idx + 1) % self.agents.len()
        };
        self.select_agent(next);
    }

    pub fn select_prev(&mut self) {
        let idx = self.selected_agent_index();
        let prev = if idx == 0 {
            self.agents.len().saturating_sub(1)
        } else {
            idx - 1
        };
        self.select_agent(prev);
    }
}
