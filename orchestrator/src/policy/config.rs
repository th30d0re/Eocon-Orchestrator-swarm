use std::path::Path;

use crate::policy::policy::{GlobalPolicy, UserPolicy};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AppConfig {
    pub user_policy: UserPolicy,
    pub pinned_verifier: Option<(String, String)>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            user_policy: UserPolicy::default(),
            pinned_verifier: None,
        }
    }
}

pub fn load_config() -> AppConfig {
    confy::load("eocon-swarm", None).unwrap_or_default()
}

pub fn save_config(config: &AppConfig) -> Result<(), String> {
    confy::store("eocon-swarm", None, config).map_err(|e| e.to_string())
}

pub fn load_global_policy(_project_dir: &Path) -> GlobalPolicy {
    let mut policy = GlobalPolicy::default();
    let path = _project_dir.join("eocon-policy.toml");
    if let Ok(content) = std::fs::read_to_string(&path) {
        if let Ok(augment) = toml::from_str::<GlobalPolicy>(&content) {
            // Augment only: add forbidden patterns, never weaken max_allowed_risk.
            for p in augment.forbidden_patterns {
                if !policy.forbidden_patterns.contains(&p) {
                    policy.forbidden_patterns.push(p);
                }
            }
        }
    }
    policy
}

pub fn load_user_policy() -> UserPolicy {
    let config = load_config();
    config.user_policy
}
