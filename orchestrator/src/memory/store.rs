use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::chat::ChatMessage;
use crate::swarm::agent::AgentRole;

#[derive(Debug, Error)]
pub enum MemoryStoreError {
    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("lock poisoned")]
    LockPoisoned,
}

impl<T> From<std::sync::PoisonError<T>> for MemoryStoreError {
    fn from(_: std::sync::PoisonError<T>) -> Self {
        MemoryStoreError::LockPoisoned
    }
}

pub type Result<T> = std::result::Result<T, MemoryStoreError>;

/// A single message as stored in SQLite.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryRecord {
    pub id: i64,
    pub agent_id: String,
    pub role: String,
    pub content: String,
    pub created_at: i64,
}

struct Inner {
    conn: Connection,
    jsonl_root: PathBuf,
}

/// Hybrid memory store: per-agent append-only JSONL for isolation, mirrored to
/// SQLite for queries and dashboards.
///
/// `MemoryStore` is cheap to clone: all clones share the same underlying SQLite
/// connection and JSONL root, synchronized by a mutex.
#[derive(Clone)]
pub struct MemoryStore {
    inner: Arc<Mutex<Inner>>,
}

impl MemoryStore {
    /// Open (or create) the SQLite database and JSONL root.
    pub fn open(sqlite_path: impl AsRef<Path>, jsonl_root: impl AsRef<Path>) -> Result<Self> {
        let sqlite_path = sqlite_path.as_ref().to_path_buf();
        let jsonl_root = jsonl_root.as_ref().to_path_buf();

        if let Some(parent) = sqlite_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::create_dir_all(&jsonl_root)?;

        let conn = Connection::open(&sqlite_path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;

        let store = Self {
            inner: Arc::new(Mutex::new(Inner { conn, jsonl_root })),
        };
        store.init_schema()?;
        Ok(store)
    }

    /// Open the default store used by the orchestrator:
    /// - SQLite: `~/.eocon/memory.sqlite`
    /// - JSONL root: `<cwd>/.eocon/memory`
    pub fn open_default() -> Result<Self> {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        let sqlite_path = home.join(".eocon").join("memory.sqlite");
        let jsonl_root = std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(".eocon")
            .join("memory");
        Self::open(sqlite_path, jsonl_root)
    }

    #[cfg(test)]
    pub fn temp() -> (Self, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let sqlite_path = dir.path().join("memory.sqlite");
        let jsonl_root = dir.path().join("memory");
        (Self::open(sqlite_path, jsonl_root).unwrap(), dir)
    }

    fn init_schema(&self) -> Result<()> {
        let inner = self.inner.lock()?;
        inner.conn.execute(
            "CREATE TABLE IF NOT EXISTS messages (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                agent_id TEXT NOT NULL,
                role TEXT NOT NULL,
                content TEXT NOT NULL,
                created_at INTEGER NOT NULL
            )",
            [],
        )?;
        inner.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_messages_agent ON messages(agent_id)",
            [],
        )?;
        Ok(())
    }

    /// Path to the per-agent JSONL file.
    pub fn jsonl_path(&self, agent_id: &str) -> PathBuf {
        let inner = self.inner.lock().expect("memory store lock poisoned");
        inner.jsonl_root.join(agent_id).join("messages.jsonl")
    }

    /// Path to the per-agent JSONL directory.
    pub fn agent_dir(&self, agent_id: &str) -> PathBuf {
        let inner = self.inner.lock().expect("memory store lock poisoned");
        inner.jsonl_root.join(agent_id)
    }

    /// Create a per-agent `AgentMemory` bound to this store.
    pub fn agent_memory(&self, agent_id: &str, role: &AgentRole) -> AgentMemory {
        AgentMemory::new(agent_id, role, self.clone())
    }

    /// Append a message to the agent's JSONL file and insert it into SQLite.
    pub fn append_message(&self, agent_id: &str, role: &str, content: &str) -> Result<()> {
        let path = self.jsonl_path(agent_id);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let msg = ChatMessage {
            role: role.to_string(),
            content: content.to_string(),
        };
        let line = serde_json::to_string(&msg)?;

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)?;
        writeln!(file, "{}", line)?;
        file.flush()?;

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        let inner = self.inner.lock()?;
        inner.conn.execute(
            "INSERT INTO messages (agent_id, role, content, created_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![agent_id, role, content, now],
        )?;
        Ok(())
    }

    /// Read all messages for an agent from SQLite.
    pub fn get_messages(&self, agent_id: &str) -> Result<Vec<ChatMessage>> {
        let inner = self.inner.lock()?;
        let mut stmt = inner.conn.prepare(
            "SELECT role, content FROM messages
             WHERE agent_id = ?1
             ORDER BY id ASC",
        )?;
        let rows = stmt.query_map([agent_id], |row| {
            Ok(ChatMessage {
                role: row.get(0)?,
                content: row.get(1)?,
            })
        })?;
        let mut messages = Vec::new();
        for row in rows {
            messages.push(row?);
        }
        Ok(messages)
    }

    /// Search message content. Pass `agent_id = None` to search across all
    /// agents (for dashboards).
    pub fn search_messages(
        &self,
        agent_id: Option<&str>,
        query: &str,
    ) -> Result<Vec<MemoryRecord>> {
        let pattern = format!(
            "%{}%",
            query.replace('%', "\\%").replace('_', "\\_")
        );
        let inner = self.inner.lock()?;
        if let Some(id) = agent_id {
            let mut stmt = inner.conn.prepare(
                "SELECT id, agent_id, role, content, created_at
                 FROM messages
                 WHERE agent_id = ?1
                   AND content LIKE ?2 ESCAPE '\\'
                 ORDER BY id ASC",
            )?;
            let rows = stmt.query_map(params![id, pattern], |row| {
                Ok(MemoryRecord {
                    id: row.get(0)?,
                    agent_id: row.get(1)?,
                    role: row.get(2)?,
                    content: row.get(3)?,
                    created_at: row.get(4)?,
                })
            })?;
            rows.map(|r| r.map_err(Into::into)).collect::<Result<Vec<_>>>()
        } else {
            let mut stmt = inner.conn.prepare(
                "SELECT id, agent_id, role, content, created_at
                 FROM messages
                 WHERE content LIKE ?1 ESCAPE '\\'
                 ORDER BY id ASC",
            )?;
            let rows = stmt.query_map([pattern], |row| {
                Ok(MemoryRecord {
                    id: row.get(0)?,
                    agent_id: row.get(1)?,
                    role: row.get(2)?,
                    content: row.get(3)?,
                    created_at: row.get(4)?,
                })
            })?;
            rows.map(|r| r.map_err(Into::into)).collect::<Result<Vec<_>>>()
        }
    }

    /// Re-read the agent's JSONL file and replace the SQLite mirror for that
    /// agent. Returns the number of messages mirrored.
    pub fn mirror_jsonl_to_sqlite(&self, agent_id: &str) -> Result<usize> {
        let path = self.jsonl_path(agent_id);
        if !path.exists() {
            return Ok(0);
        }

        let file = fs::File::open(&path)?;
        let reader = BufReader::new(file);

        let mut inner = self.inner.lock()?;
        let tx = inner.conn.transaction()?;
        tx.execute("DELETE FROM messages WHERE agent_id = ?1", [agent_id])?;

        let mut count = 0;
        let now_base = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        for (idx, line) in reader.lines().enumerate() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            let msg: ChatMessage = match serde_json::from_str(&line) {
                Ok(m) => m,
                Err(_) => continue,
            };
            tx.execute(
                "INSERT INTO messages (agent_id, role, content, created_at)
                 VALUES (?1, ?2, ?3, ?4)",
                params![agent_id, msg.role, msg.content, now_base + idx as i64],
            )?;
            count += 1;
        }
        tx.commit()?;
        Ok(count)
    }
}

/// Per-agent memory. Holds an in-memory cache of messages and writes every
/// append to both JSONL and SQLite via the shared `MemoryStore`.
pub struct AgentMemory {
    pub system_prompt: String,
    pub messages: Vec<ChatMessage>,
    pub max_history: usize,
    pub agent_id: String,
    pub memory_dir: PathBuf,
    store: MemoryStore,
}

impl AgentMemory {
    pub fn new(agent_id: &str, role: &AgentRole, store: MemoryStore) -> Self {
        let memory_dir = store.agent_dir(agent_id);
        let _ = fs::create_dir_all(&memory_dir);
        Self {
            system_prompt: build_system_prompt_for_role(role),
            messages: Vec::new(),
            max_history: 128,
            agent_id: agent_id.to_string(),
            memory_dir,
            store,
        }
    }

    pub fn with_memory_dir(mut self, dir: PathBuf) -> Self {
        self.memory_dir = dir;
        let _ = fs::create_dir_all(&self.memory_dir);
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
            content: content.clone(),
        });
        self.truncate_if_needed();
        let _ = self.store.append_message(&self.agent_id, role, &content);
    }

    pub fn truncate_if_needed(&mut self) {
        if self.messages.len() > self.max_history {
            let excess = self.messages.len() - self.max_history;
            self.messages.drain(0..excess);
        }
    }

    /// Rewrite the agent's JSONL file from the in-memory cache and re-mirror to
    /// SQLite. Kept for compatibility with code that expects explicit persistence.
    pub fn persist(&self) -> std::io::Result<()> {
        let path = self.store.jsonl_path(&self.agent_id);
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let file = fs::File::create(&path)?;
        let mut writer = std::io::BufWriter::new(file);
        for msg in &self.messages {
            let line = serde_json::to_string(msg)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
            writeln!(writer, "{}", line)?;
        }
        writer.flush()?;
        let _ = self.mirror_jsonl_to_sqlite();
        Ok(())
    }

    /// Load messages from SQLite into the in-memory cache.
    pub fn load(&mut self) -> std::io::Result<()> {
        self.messages = self
            .store
            .get_messages(&self.agent_id)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;
        self.truncate_if_needed();
        Ok(())
    }

    pub fn mirror_jsonl_to_sqlite(&self) -> Result<usize> {
        self.store.mirror_jsonl_to_sqlite(&self.agent_id)
    }
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
        AgentRole::FoundationBuilder => format!("{}\n\nRole: Foundation Phase 5 build specialist. Invoke the Foundation BuildAgent script (via the FoundationBuilder skill) to generate, build, and repair Apple-platform code. Report build outcomes as gists.", base),
        AgentRole::FoundationTester => format!("{}\n\nRole: Foundation Phase 5 test specialist. Invoke the Foundation TestAgent script (via the FoundationTester skill) to generate and run unit, accessibility, flow, UI, Maestro, and Argent verification tests. Report test outcomes as gists.", base),
        AgentRole::RiskGuard => format!("{}\n\nRole: risk guard. Classify proposed actions against the safety policy and stop unsafe actions.", base),
        AgentRole::Quant => format!("{}\n\nRole: quantitative analyst. Perform numerical reasoning and model evaluation.", base),
        AgentRole::Generalist => base.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn append_and_get_messages() {
        let (store, _dir) = MemoryStore::temp();
        store.append_message("agent-1", "user", "hello").unwrap();
        store.append_message("agent-1", "assistant", "hi there").unwrap();
        store.append_message("agent-2", "user", "other").unwrap();

        let msgs = store.get_messages("agent-1").unwrap();
        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[0].role, "user");
        assert_eq!(msgs[0].content, "hello");
        assert_eq!(msgs[1].role, "assistant");
        assert_eq!(msgs[1].content, "hi there");
    }

    #[test]
    fn search_messages_finds_by_content() {
        let (store, _dir) = MemoryStore::temp();
        store.append_message("a", "user", "build the auth module").unwrap();
        store.append_message("a", "assistant", "auth module built").unwrap();
        store.append_message("b", "user", "build the db layer").unwrap();

        let results = store.search_messages(Some("a"), "auth").unwrap();
        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|r| r.agent_id == "a"));

        let all = store.search_messages(None, "build").unwrap();
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn mirror_jsonl_rebuilds_sqlite() {
        let (store, _dir) = MemoryStore::temp();
        let path = store.jsonl_path("mirror-agent");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut file = fs::File::create(&path).unwrap();
        writeln!(
            file,
            "{}",
            serde_json::json!({"role": "user", "content": "first"})
        )
        .unwrap();
        writeln!(
            file,
            "{}",
            serde_json::json!({"role": "assistant", "content": "second"})
        )
        .unwrap();
        drop(file);

        let count = store.mirror_jsonl_to_sqlite("mirror-agent").unwrap();
        assert_eq!(count, 2);

        let msgs = store.get_messages("mirror-agent").unwrap();
        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[1].content, "second");
    }

    #[test]
    fn agent_memory_persists_and_loads() {
        let (store, _dir) = MemoryStore::temp();
        let mut memory = AgentMemory::new("agent-x", &AgentRole::Coder, store.clone());
        memory.push_user("task".to_string());
        memory.push_assistant("done".to_string());

        assert_eq!(memory.messages.len(), 2);

        let mut memory2 = AgentMemory::new("agent-x", &AgentRole::Coder, store);
        memory2.load().unwrap();
        assert_eq!(memory2.messages.len(), 2);
        assert_eq!(memory2.messages[0].content, "task");
        assert_eq!(memory2.messages[1].content, "done");
    }

    #[test]
    fn agent_memory_is_isolated() {
        let (store, _dir) = MemoryStore::temp();
        let mut a = AgentMemory::new("a", &AgentRole::Generalist, store.clone());
        a.push_user("secret a".to_string());

        let mut b = AgentMemory::new("b", &AgentRole::Generalist, store);
        b.push_user("secret b".to_string());

        assert!(!a.messages.iter().any(|m| m.content.contains("secret b")));
        assert!(!b.messages.iter().any(|m| m.content.contains("secret a")));
    }
}
