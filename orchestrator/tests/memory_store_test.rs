//! Integration tests for the SQLite + JSONL hybrid memory store.
//!
//! These tests exercise the public `MemoryStore` and `AgentMemory` APIs using
//! temporary directories so they do not depend on the default `~/.eocon`
//! location.

use eocon_swarm::memory::store::{AgentMemory, MemoryStore};
use eocon_swarm::swarm::agent::AgentRole;

fn temp_store() -> (MemoryStore, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let sqlite_path = dir.path().join("memory.sqlite");
    let jsonl_root = dir.path().join("memory");
    (MemoryStore::open(sqlite_path, jsonl_root).unwrap(), dir)
}

#[test]
fn memory_store_round_trip() {
    let (store, _dir) = temp_store();

    store.append_message("alpha", "system", "base prompt").unwrap();
    store.append_message("alpha", "user", "first task").unwrap();
    store.append_message("alpha", "assistant", "first result").unwrap();
    store.append_message("beta", "user", "beta task").unwrap();

    let alpha = store.get_messages("alpha").unwrap();
    assert_eq!(alpha.len(), 3);
    assert_eq!(alpha[0].role, "system");
    assert_eq!(alpha[1].content, "first task");

    let beta = store.get_messages("beta").unwrap();
    assert_eq!(beta.len(), 1);
    assert_eq!(beta[0].content, "beta task");
}

#[test]
fn search_isolated_by_agent_and_across_agents() {
    let (store, _dir) = temp_store();

    store
        .append_message("alice", "assistant", "alice knows auth")
        .unwrap();
    store
        .append_message("bob", "assistant", "bob knows db")
        .unwrap();
    store
        .append_message("alice", "user", "alice asks about db")
        .unwrap();

    let alice_auth = store.search_messages(Some("alice"), "auth").unwrap();
    assert_eq!(alice_auth.len(), 1);

    let db_anyone = store.search_messages(None, "db").unwrap();
    assert_eq!(db_anyone.len(), 2);
    assert!(db_anyone.iter().any(|r| r.agent_id == "alice"));
    assert!(db_anyone.iter().any(|r| r.agent_id == "bob"));
}

#[test]
fn mirror_syncs_existing_jsonl() {
    let (store, _dir) = temp_store();
    let path = store.jsonl_path("legacy");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();

    let payload = [
        serde_json::json!({"role": "user", "content": "legacy prompt"}),
        serde_json::json!({"role": "assistant", "content": "legacy answer"}),
    ];
    std::fs::write(&path, payload.map(|v| v.to_string()).join("\n") + "\n").unwrap();

    let before = store.get_messages("legacy").unwrap();
    assert!(before.is_empty(), "SQLite should be empty before mirroring");

    let count = store.mirror_jsonl_to_sqlite("legacy").unwrap();
    assert_eq!(count, 2);

    let after = store.get_messages("legacy").unwrap();
    assert_eq!(after.len(), 2);
    assert_eq!(after[1].content, "legacy answer");
}

#[test]
fn agent_memory_pushes_to_store_and_loads() {
    let (store, _dir) = temp_store();
    let mut memory = AgentMemory::new("worker", &AgentRole::Coder, store.clone());

    memory.push_user("refactor the parser".to_string());
    memory.push_assistant("refactored successfully".to_string());

    // In-memory cache reflects the pushes immediately.
    assert_eq!(memory.messages.len(), 2);

    // A fresh AgentMemory for the same agent can load from SQLite.
    let mut loaded = AgentMemory::new("worker", &AgentRole::Coder, store);
    loaded.load().unwrap();
    assert_eq!(loaded.messages.len(), 2);
    assert_eq!(loaded.messages[0].content, "refactor the parser");
    assert_eq!(loaded.messages[1].content, "refactored successfully");
}

#[test]
fn private_memory_isolation() {
    let (store, _dir) = temp_store();

    let mut a = AgentMemory::new("agent-a", &AgentRole::Generalist, store.clone());
    a.push_user("secret for a".to_string());

    let mut b = AgentMemory::new("agent-b", &AgentRole::Generalist, store.clone());
    b.push_user("secret for b".to_string());

    let a_msgs = store.get_messages("agent-a").unwrap();
    let b_msgs = store.get_messages("agent-b").unwrap();

    assert!(a_msgs.iter().all(|m| !m.content.contains("agent-b")));
    assert!(b_msgs.iter().all(|m| !m.content.contains("agent-a")));
}
