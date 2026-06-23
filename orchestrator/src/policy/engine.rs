use crate::policy::policy::{ActionMode, GlobalPolicy, RiskLabel, SafetyDecision, UserPolicy};
use crate::swarm::gist::Gist;

#[derive(Debug, Clone, Default)]
pub struct PolicyEngine {
    pub global: GlobalPolicy,
    pub user: UserPolicy,
}

impl PolicyEngine {
    pub fn new(global: GlobalPolicy, user: UserPolicy) -> Self {
        Self { global, user }
    }

    pub fn classify_and_act(&self, gist: &Gist) -> SafetyDecision {
        let text = format!("{} {}", gist.claim, gist.summary).to_lowercase();

        // Global deny list check.
        for pattern in &self.global.forbidden_patterns {
            if text.contains(&pattern.to_lowercase()) {
                return SafetyDecision::reject(&format!("Global policy violation: {}", pattern));
            }
        }

        // Estimate risk from source type.
        let risk = match gist.source_type {
            crate::swarm::gist::GistSource::ToolOutput => RiskLabel::Safe,
            crate::swarm::gist::GistSource::FileRead => RiskLabel::Low,
            crate::swarm::gist::GistSource::UserProvided => RiskLabel::Low,
            crate::swarm::gist::GistSource::ModelClaim => RiskLabel::Medium,
        };

        if risk > self.global.max_allowed_risk {
            return SafetyDecision {
                action: ActionMode::Reject,
                risk_label: risk,
                reasoning: "Risk exceeds global maximum allowed risk.".to_string(),
                guided_alternative: Some("Reduce scope or add concrete evidence.".to_string()),
            };
        }

        if risk > self.user.max_autonomous_risk {
            return SafetyDecision {
                action: ActionMode::Guide,
                risk_label: risk,
                reasoning: "Risk exceeds user-configured autonomous threshold; requires approval."
                    .to_string(),
                guided_alternative: Some(
                    "Ask the user for explicit approval before acting.".to_string(),
                ),
            };
        }

        SafetyDecision {
            action: ActionMode::Comply,
            risk_label: risk,
            reasoning: "Classified as safe under current policy.".to_string(),
            guided_alternative: None,
        }
    }

    pub fn pre_check(&self, action_description: &str) -> SafetyDecision {
        let text = action_description.to_lowercase();
        for pattern in &self.global.forbidden_patterns {
            if text.contains(&pattern.to_lowercase()) {
                return SafetyDecision::reject(&format!("Pre-check global violation: {}", pattern));
            }
        }
        SafetyDecision::comply()
    }

    pub fn post_check(&self, _outcome: &str) -> SafetyDecision {
        SafetyDecision::comply()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::swarm::gist::{Gist, GistSource};
    use std::time::Instant;

    fn gist_with_claim(claim: &str) -> Gist {
        Gist {
            schema_version: 1,
            id: "g1".to_string(),
            agent_id: "a1".to_string(),
            agent_name: "coder".to_string(),
            claim: claim.to_string(),
            summary: claim.to_string(),
            evidence: vec!["evidence".to_string()],
            source_type: GistSource::ModelClaim,
            peer_rank: None,
            status: crate::swarm::gist::GistStatus::Verifying,
            created_at: Instant::now(),
        }
    }

    #[test]
    fn policy_rejects_forbidden_pattern() {
        let engine = PolicyEngine::default();
        let gist = gist_with_claim("We should export PII to analytics");
        let decision = engine.classify_and_act(&gist);
        assert_eq!(decision.action, ActionMode::Reject);
    }

    #[test]
    fn policy_complies_for_safe_gist() {
        let engine = PolicyEngine::default();
        let gist = gist_with_claim("Use token-based sessions");
        let decision = engine.classify_and_act(&gist);
        assert_eq!(decision.action, ActionMode::Comply);
    }
}
