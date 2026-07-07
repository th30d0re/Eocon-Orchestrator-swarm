use std::path::{Path, PathBuf};

pub use crate::memory::store::{AgentMemory, MemoryStore, MemoryStoreError};

pub fn default_memory_dir(agent_id: &str) -> PathBuf {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    cwd.join(".eocon").join("memory").join(agent_id)
}

pub fn project_memory_dir(project_dir: &Path, agent_id: &str) -> PathBuf {
    project_dir.join(".eocon").join("memory").join(agent_id)
}
