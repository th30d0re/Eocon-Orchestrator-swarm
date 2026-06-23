use std::time::Instant;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GistStatus {
    #[default]
    Draft,
    Verifying,
    Verified,
    Rejected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnfoldLevel {
    Gist,
    Summary,
    Evidence,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GistSource {
    ToolOutput,
    FileRead,
    ModelClaim,
    UserProvided,
}

#[derive(Debug, Clone, Serialize)]
pub struct Gist {
    pub schema_version: u32,
    pub id: String,
    pub agent_id: String,
    pub agent_name: String,
    pub claim: String,
    pub summary: String,
    pub evidence: Vec<String>,
    pub source_type: GistSource,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub peer_rank: Option<f64>,
    #[serde(skip)]
    pub status: GistStatus,
    #[serde(skip)]
    pub created_at: Instant,
}

impl Gist {
    pub fn is_verified(&self) -> bool {
        self.status == GistStatus::Verified
    }

    pub fn status_string(&self) -> &'static str {
        match self.status {
            GistStatus::Draft => "draft",
            GistStatus::Verifying => "verifying",
            GistStatus::Verified => "verified",
            GistStatus::Rejected => "rejected",
        }
    }
}

pub struct GistBoard {
    pub gists: Vec<Gist>,
}

impl Default for GistBoard {
    fn default() -> Self {
        Self::new()
    }
}

impl GistBoard {
    pub fn new() -> Self {
        Self { gists: Vec::new() }
    }

    pub fn add(&mut self, gist: Gist) {
        self.gists.push(gist);
    }

    pub fn verified(&self) -> Vec<&Gist> {
        self.gists.iter().filter(|g| g.is_verified()).collect()
    }

    pub fn pending(&self) -> Vec<&Gist> {
        self.gists
            .iter()
            .filter(|g| matches!(g.status, GistStatus::Draft | GistStatus::Verifying))
            .collect()
    }

    pub fn reject(&mut self, id: &str) -> bool {
        if let Some(g) = self.gists.iter_mut().find(|g| g.id == id) {
            g.status = GistStatus::Rejected;
            true
        } else {
            false
        }
    }

    pub fn get(&self, id: &str) -> Option<&Gist> {
        self.gists.iter().find(|g| g.id == id)
    }

    pub fn unfold(&self, id: &str, _level: UnfoldLevel) -> Option<String> {
        self.get(id).map(|g| {
            let mut out = format!("Claim: {}\nSummary: {}\n", g.claim, g.summary);
            if !g.evidence.is_empty() {
                out.push_str("Evidence:\n");
                for (i, e) in g.evidence.iter().enumerate() {
                    out.push_str(&format!("  {}. {}\n", i + 1, e));
                }
            }
            out
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_gist(id: &str) -> Gist {
        Gist {
            schema_version: 1,
            id: id.to_string(),
            agent_id: "a1".to_string(),
            agent_name: "coder".to_string(),
            claim: "Use token sessions".to_string(),
            summary: "REST-first session design".to_string(),
            evidence: vec!["API uses JWT".to_string()],
            source_type: GistSource::ModelClaim,
            peer_rank: None,
            status: GistStatus::Verified,
            created_at: Instant::now(),
        }
    }

    #[test]
    fn board_adds_and_verifies() {
        let mut board = GistBoard::new();
        board.add(sample_gist("g1"));
        assert_eq!(board.verified().len(), 1);
        assert!(board.get("g1").is_some());
    }

    #[test]
    fn board_rejects() {
        let mut board = GistBoard::new();
        board.add(sample_gist("g1"));
        assert!(board.reject("g1"));
        assert!(!board.verified().iter().any(|g| g.id == "g1"));
    }
}
