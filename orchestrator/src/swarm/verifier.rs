use std::path::PathBuf;
use std::time::Instant;

use serde::{Deserialize, Serialize};

use crate::chat::{ChatMessage, ChatRequest};
use crate::swarm::gist::{Gist, GistSource, GistStatus};
use crate::swarm::memory::AgentMemory;

#[derive(Debug, Clone)]
pub struct GistDraft {
    pub claim: String,
    pub summary: String,
    pub evidence: Vec<String>,
    pub source_type: GistSource,
}

impl GistDraft {
    pub fn extract_from(memory: &AgentMemory) -> Option<Self> {
        let assistant_text: String = memory
            .messages
            .iter()
            .filter(|m| m.role == "assistant")
            .map(|m| m.content.clone())
            .collect::<Vec<_>>()
            .join("\n");
        if assistant_text.trim().is_empty() {
            return None;
        }
        let claim = assistant_text.lines().next().unwrap_or("").to_string();
        let summary = if assistant_text.len() > 240 {
            format!("{}...", &assistant_text[..237])
        } else {
            assistant_text.clone()
        };
        let evidence = assistant_text
            .lines()
            .skip(1)
            .take(3)
            .map(|s| s.to_string())
            .filter(|s| !s.is_empty())
            .collect();
        Some(Self {
            claim,
            summary,
            evidence,
            source_type: GistSource::ModelClaim,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifierVerdict {
    pub gist_id: String,
    pub accepted: bool,
    pub confidence: f64,
    pub rationale: String,
    pub unsupported_claims: Vec<String>,
    pub verifier_model: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum VerifierTask {
    GistVerification,
    SafetyClassification,
    CodeReview,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelSkillEntry {
    pub provider: String,
    pub model: String,
    pub task: VerifierTask,
    pub score: f64,
    pub latency_ms: u64,
    pub false_positive_rate: f64,
    pub false_negative_rate: f64,
    pub samples: u64,
    pub last_evaluated: Option<std::time::SystemTime>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ModelSkillChart {
    pub entries: Vec<ModelSkillEntry>,
}

impl ModelSkillChart {
    pub fn load() -> Self {
        let path = skill_chart_path();
        if let Ok(contents) = std::fs::read_to_string(&path) {
            serde_json::from_str(&contents).unwrap_or_default()
        } else {
            Self::default()
        }
    }

    pub fn save(&self) -> std::io::Result<()> {
        let path = skill_chart_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, serde_json::to_string_pretty(self)?)
    }

    pub fn best_verifier(&self, task: VerifierTask) -> Option<(String, String)> {
        self.entries
            .iter()
            .filter(|e| e.task == task)
            .max_by(|a, b| {
                let score_a = a.score - (a.latency_ms as f64 / 10_000.0);
                let score_b = b.score - (b.latency_ms as f64 / 10_000.0);
                score_a
                    .partial_cmp(&score_b)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|e| (e.provider.clone(), e.model.clone()))
    }

    pub fn record(&mut self, entry: ModelSkillEntry) {
        self.entries.retain(|e| {
            !(e.provider == entry.provider && e.model == entry.model && e.task == entry.task)
        });
        self.entries.push(entry);
    }
}

fn skill_chart_path() -> PathBuf {
    let data_dir = dirs::data_dir().unwrap_or_else(|| PathBuf::from("."));
    data_dir
        .join("eocon-swarm")
        .join("verifier-skill-chart.json")
}

/// Rule-based gist verifier.
pub fn verify(draft: &GistDraft) -> GistStatus {
    if draft.claim.trim().is_empty() {
        return GistStatus::Rejected;
    }
    if draft.evidence.is_empty() {
        return GistStatus::Rejected;
    }
    if draft.claim.len() > 240 {
        return GistStatus::Rejected;
    }
    let lower = draft.claim.to_lowercase();
    if lower.contains("i think") || lower.contains("i believe") || lower.contains("in my opinion") {
        return GistStatus::Rejected;
    }
    GistStatus::Verified
}

pub struct LLMVerifier {
    pub provider: String,
    pub model: String,
}

impl LLMVerifier {
    pub fn verify_async(&self, draft: &GistDraft) -> ChatRequest {
        let prompt = format!(
            "You are a gist verifier. Review the following claim and evidence. \
            Respond with JSON {{\"accepted\": bool, \"confidence\": 0.0-1.0, \"rationale\": \"...\", \"unsupported_claims\": [\"...\"]}}.\n\n\
            Claim: {}\nSummary: {}\nEvidence:\n{}",
            draft.claim,
            draft.summary,
            draft.evidence.join("\n")
        );
        ChatRequest {
            provider: self.provider.clone(),
            model: self.model.clone(),
            messages: vec![ChatMessage {
                role: "user".to_string(),
                content: prompt,
            }],
            system_prompt: Some("You verify gists strictly based on evidence.".to_string()),
        }
    }
}

pub fn draft_to_gist(draft: GistDraft, agent_id: String, agent_name: String) -> Gist {
    Gist {
        schema_version: 1,
        id: uuid::Uuid::new_v4().to_string(),
        agent_id,
        agent_name,
        claim: draft.claim,
        summary: draft.summary,
        evidence: draft.evidence,
        source_type: draft.source_type,
        peer_rank: None,
        status: GistStatus::Draft,
        created_at: Instant::now(),
    }
}
