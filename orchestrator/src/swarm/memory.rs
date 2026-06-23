use std::io::Write;
use std::path::{Path, PathBuf};

use crate::chat::ChatMessage;
use crate::swarm::agent::AgentRole;

pub struct AgentMemory {
    pub system_prompt: String,
    pub messages: Vec<ChatMessage>,
    pub max_history: usize,
    pub agent_id: String,
    pub memory_dir: PathBuf,
}

impl AgentMemory {
    pub fn new(agent_id: &str, role: &AgentRole) -> Self {
        let memory_dir = default_memory_dir(agent_id);
        let _ = std::fs::create_dir_all(&memory_dir);
        Self {
            system_prompt: build_system_prompt_for_role(role),
            messages: Vec::new(),
            max_history: 128,
            agent_id: agent_id.to_string(),
            memory_dir,
        }
    }

    pub fn with_memory_dir(mut self, dir: PathBuf) -> Self {
        self.memory_dir = dir;
        let _ = std::fs::create_dir_all(&self.memory_dir);
        self
    }

    pub fn push_user(&mut self, content: String) {
        self.push_raw("user", content);
    }

    pub fn push_assistant(&mut self, content: String) {
        self.push_raw("assistant", content);
    }

    pub fn push_system_correction(&mut self, content: String) {
        self.push_raw("system", format!("[user correction] {}", content));
    }

    pub fn push_raw(&mut self, role: &str, content: String) {
        self.messages.push(ChatMessage {
            role: role.to_string(),
            content,
        });
        self.truncate_if_needed();
        let _ = self.persist();
    }

    pub fn truncate_if_needed(&mut self) {
        if self.messages.len() > self.max_history {
            let excess = self.messages.len() - self.max_history;
            self.messages.drain(0..excess);
        }
    }

    pub fn persist(&self) -> std::io::Result<()> {
        let path = self.memory_dir.join("messages.jsonl");
        let file = std::fs::File::create(&path)?;
        let mut writer = std::io::BufWriter::new(file);
        for msg in &self.messages {
            let line = serde_json::to_string(msg)?;
            use std::io::Write;
            writeln!(writer, "{}", line)?;
        }
        writer.flush()?;
        Ok(())
    }

    pub fn load(&mut self) -> std::io::Result<()> {
        let path = self.memory_dir.join("messages.jsonl");
        if !path.exists() {
            return Ok(());
        }
        let file = std::fs::File::open(&path)?;
        let reader = std::io::BufReader::new(file);
        self.messages.clear();
        for line in std::io::BufRead::lines(reader) {
            let line = line?;
            if let Ok(msg) = serde_json::from_str::<ChatMessage>(&line) {
                self.messages.push(msg);
            }
        }
        self.truncate_if_needed();
        Ok(())
    }
}

pub fn default_memory_dir(agent_id: &str) -> PathBuf {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    cwd.join(".eocon").join("memory").join(agent_id)
}

pub fn project_memory_dir(project_dir: &Path, agent_id: &str) -> PathBuf {
    project_dir.join(".eocon").join("memory").join(agent_id)
}

fn build_system_prompt_for_role(role: &AgentRole) -> String {
    let base = "You are a specialist agent in the Eocon Swarm. Your reasoning is private memory. \
You may ONLY act on verified gists from the shared GistBoard. \
You must NEVER reference, read, or infer the private memory of another agent. \
When you have a concrete finding, output it as a gist with claim, summary, and evidence.";
    match role {
        AgentRole::Planner => format!("{}\n\nRole: planner. Decompose high-level tasks into actionable subtasks and assign them to the appropriate specialist agents.", base),
        AgentRole::Coder => format!("{}\n\nRole: coder. Write, refactor, and debug code. Prefer small, testable changes.", base),
        AgentRole::Reviewer => format!("{}\n\nRole: reviewer. Critically review code, gists, and plans for correctness, safety, and policy compliance.", base),
        AgentRole::Researcher => format!("{}\n\nRole: researcher. Gather facts from code, docs, and tools; always cite evidence.", base),
        AgentRole::Synthesizer => format!("{}\n\nRole: synthesizer. Aggregate verified gists into a final coherent plan or report.", base),
        AgentRole::SwiftBuilder => format!("{}\n\nRole: Swift build specialist. Invoke the BuildAgent command to compile Swift projects and report build outcomes as gists.", base),
        AgentRole::SwiftTester => format!("{}\n\nRole: Swift test specialist. Invoke the TestAgent command to run Swift tests and report test outcomes as gists.", base),
        AgentRole::RiskGuard => format!("{}\n\nRole: risk guard. Classify proposed actions against the safety policy and stop unsafe actions.", base),
        AgentRole::Quant => format!("{}\n\nRole: quantitative analyst. Perform numerical reasoning and model evaluation.", base),
        AgentRole::Generalist => base.to_string(),
    }
}
