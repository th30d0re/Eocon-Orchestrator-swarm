use serde::{Deserialize, Serialize};

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub enum RiskLabel {
    Safe,
    Low,
    Medium,
    High,
    Critical,
}

impl RiskLabel {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "safe" | "tier0" => RiskLabel::Safe,
            "low" | "tier1" => RiskLabel::Low,
            "medium" | "tier2" => RiskLabel::Medium,
            "high" | "tier3" => RiskLabel::High,
            "critical" => RiskLabel::Critical,
            _ => RiskLabel::Low,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ActionMode {
    Comply,
    Guide,
    Reject,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalPolicy {
    pub forbidden_patterns: Vec<String>,
    pub max_allowed_risk: RiskLabel,
}

impl Default for GlobalPolicy {
    fn default() -> Self {
        Self {
            forbidden_patterns: vec![
                "export PII".to_string(),
                "bypass 2FA".to_string(),
                "disable security".to_string(),
                "rm -rf /".to_string(),
                "DROP TABLE".to_string(),
            ],
            max_allowed_risk: RiskLabel::High,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserPolicy {
    pub domain: String,
    pub permitted_overrides: Vec<(RiskLabel, String)>,
    pub max_autonomous_risk: RiskLabel,
}

impl Default for UserPolicy {
    fn default() -> Self {
        Self {
            domain: "default".to_string(),
            permitted_overrides: Vec::new(),
            max_autonomous_risk: RiskLabel::Medium,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafetyDecision {
    pub action: ActionMode,
    pub risk_label: RiskLabel,
    pub reasoning: String,
    pub guided_alternative: Option<String>,
}

impl SafetyDecision {
    pub fn reject(reason: &str) -> Self {
        Self {
            action: ActionMode::Reject,
            risk_label: RiskLabel::Critical,
            reasoning: reason.to_string(),
            guided_alternative: None,
        }
    }

    pub fn comply() -> Self {
        Self {
            action: ActionMode::Comply,
            risk_label: RiskLabel::Safe,
            reasoning: "No policy concerns.".to_string(),
            guided_alternative: None,
        }
    }
}
