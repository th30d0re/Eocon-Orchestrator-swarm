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
    /// One-line views (dashboard, /gist list) render the claim,
    /// so it stays bounded; the summary is the full body and is
    /// only shown by unfold/snapshot consumers.
    const MAX_CLAIM_CHARS: usize = 160;

    pub fn extract_from(memory: &AgentMemory) -> Option<Self> {
        let assistant_text: String = memory
            .messages
            .iter()
            .filter(|m| m.role == "assistant")
            .map(|m| m.content.clone())
            .collect::<Vec<_>>()
            .join("\n");
        Self::from_text(&assistant_text)
    }

    pub fn from_text(assistant_text: &str) -> Option<Self> {
        if assistant_text.trim().is_empty() {
            return None;
        }
        // Markdown reports often open with a horizontal rule or
        // heading marker; a decoration-only line makes a useless
        // claim, so take the first line with real content.
        let claim = assistant_text
            .lines()
            .map(str::trim)
            .find(|l| !l.is_empty() && !l.chars().all(|c| matches!(c, '-' | '*' | '=' | '_' | '#' | '~' | ' ')))
            .unwrap_or_default();
        let claim = char_truncate(claim, Self::MAX_CLAIM_CHARS);
        let summary = assistant_text.to_string();
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

/// Truncate on a char boundary; a byte slice would panic when
/// the cut lands inside a multi-byte char (e.g. an emoji).
fn char_truncate(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        return s.to_string();
    }
    let cut: String = s.chars().take(max_chars.saturating_sub(3)).collect();
    format!("{}...", cut)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn draft_keeps_full_summary() {
        let long = format!("Headline\n{}", "x".repeat(5000));
        let draft = GistDraft::from_text(&long).unwrap();
        assert_eq!(draft.summary, long);
    }

    #[test]
    fn draft_claim_skips_markdown_decoration() {
        let text = "---\n\n## Build Result: BUILD SUCCEEDED\ndetails";
        let draft = GistDraft::from_text(text).unwrap();
        assert_eq!(draft.claim, "## Build Result: BUILD SUCCEEDED");
    }

    #[test]
    fn draft_claim_is_bounded_and_char_safe() {
        let text = "✅".repeat(400);
        let draft = GistDraft::from_text(&text).unwrap();
        assert!(draft.claim.chars().count() <= GistDraft::MAX_CLAIM_CHARS);
        assert!(draft.claim.ends_with("..."));
    }

    #[test]
    fn draft_rejects_empty_text() {
        assert!(GistDraft::from_text("  \n ").is_none());
    }
}
