use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::LlmError;

/// Top-level configuration for the unified LLM client.
///
/// Supports TOML and JSON on disk. API keys are intentionally stored by
/// reference to an environment variable name rather than inline.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LlmConfig {
    /// Provider name used when a request does not specify one.
    #[serde(default)]
    pub default_provider: Option<String>,

    /// Per-provider settings keyed by provider name.
    #[serde(default)]
    pub providers: HashMap<String, ProviderConfig>,

    /// Defaults for the text tool-call harness.
    #[serde(default)]
    pub tool_harness: ToolHarnessConfig,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProviderConfig {
    /// Name of the environment variable holding the API key.
    /// Local/offline providers may omit this field.
    #[serde(default)]
    pub api_key_env: Option<String>,

    /// Base URL for the provider's HTTP API.
    #[serde(default)]
    pub base_url: Option<String>,

    /// Model identifier used when a request does not specify one.
    #[serde(default)]
    pub default_model: Option<String>,

    /// Provider-specific free-form parameters (e.g. Anthropic API version).
    #[serde(default)]
    pub extras: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolHarnessConfig {
    #[serde(default = "default_tool_harness_enabled")]
    pub enabled: bool,

    #[serde(default = "default_tool_names")]
    pub tool_names: Vec<String>,
}

impl Default for ToolHarnessConfig {
    fn default() -> Self {
        Self {
            enabled: default_tool_harness_enabled(),
            tool_names: default_tool_names(),
        }
    }
}

fn default_tool_harness_enabled() -> bool {
    true
}

fn default_tool_names() -> Vec<String> {
    vec![
        "Bash".to_string(),
        "Read".to_string(),
        "Write".to_string(),
        "Edit".to_string(),
        "Glob".to_string(),
        "Grep".to_string(),
        "Move".to_string(),
        "Copy".to_string(),
        "Delete".to_string(),
        "MkDir".to_string(),
    ]
}

impl LlmConfig {
    /// Load configuration from a TOML or JSON file.
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self, LlmError> {
        let path = path.as_ref();
        let text = std::fs::read_to_string(path)
            .map_err(|e| LlmError::Config(format!("cannot read '{}': {}", path.display(), e)))?;

        if Self::is_json(path) {
            serde_json::from_str(&text)
                .map_err(|e| LlmError::Config(format!("invalid JSON config: {}", e)))
        } else {
            toml::from_str(&text)
                .map_err(|e| LlmError::Config(format!("invalid TOML config: {}", e)))
        }
    }

    /// Load from the default user config path, or return an empty config.
    pub fn load_default() -> Self {
        let path = dirs::config_dir()
            .map(|p| p.join("eocon").join("llm.toml"))
            .unwrap_or_else(|| std::path::PathBuf::from(".eocon/llm.toml"));
        Self::load(&path).unwrap_or_default()
    }

    /// Merge another config into this one. Values in `other` take precedence.
    pub fn merge(&mut self, other: Self) {
        if other.default_provider.is_some() {
            self.default_provider = other.default_provider;
        }
        for (name, cfg) in other.providers {
            self.providers.insert(name, cfg);
        }
        if !other.tool_harness.tool_names.is_empty() {
            self.tool_harness.tool_names = other.tool_harness.tool_names;
        }
        self.tool_harness.enabled = other.tool_harness.enabled;
    }

    fn is_json(path: &Path) -> bool {
        path.extension()
            .and_then(|e| e.to_str())
            .map(|e| e.eq_ignore_ascii_case("json"))
            .unwrap_or(false)
    }

    /// Resolve an API key for the named provider.
    ///
    /// Returns `Ok(None)` when the provider has no `api_key_env` set
    /// (typical for local/offline providers).
    pub fn api_key(&self, provider: &str) -> Result<Option<String>, LlmError> {
        let cfg = self.providers.get(provider).ok_or_else(|| {
            LlmError::ProviderNotConfigured(provider.to_string())
        })?;

        match &cfg.api_key_env {
            Some(env_name) => std::env::var(env_name)
                .map(Some)
                .map_err(|_| LlmError::MissingApiKey(provider.to_string(), env_name.clone())),
            None => Ok(None),
        }
    }

    pub fn base_url(&self, provider: &str, fallback: &str) -> String {
        self.providers
            .get(provider)
            .and_then(|c| c.base_url.clone())
            .unwrap_or_else(|| fallback.to_string())
    }

    pub fn default_model(&self, provider: &str) -> Option<String> {
        self.providers.get(provider).and_then(|c| c.default_model.clone())
    }

    pub fn extra(&self, provider: &str, key: &str) -> Option<serde_json::Value> {
        self.providers
            .get(provider)
            .and_then(|c| c.extras.get(key).cloned())
    }

    pub fn tool_names(&self) -> Vec<String> {
        self.tool_harness.tool_names.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn load_default_is_empty() {
        let cfg = LlmConfig::load_default();
        assert!(cfg.default_provider.is_none());
        assert!(cfg.providers.is_empty());
        assert!(cfg.tool_harness.enabled);
    }

    #[test]
    fn load_toml() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("llm.toml");
        let mut f = std::fs::File::create(&path).unwrap();
        write!(
            f,
            r#"
default_provider = "anthropic"

[providers.anthropic]
api_key_env = "ANTHROPIC_API_KEY"
base_url = "https://api.anthropic.com/v1"
default_model = "claude-sonnet-4-6"

[providers.openai]
api_key_env = "OPENAI_API_KEY"
default_model = "gpt-4o"
"#
        )
        .unwrap();

        let cfg = LlmConfig::load(&path).unwrap();
        assert_eq!(cfg.default_provider.as_deref(), Some("anthropic"));
        assert_eq!(cfg.base_url("anthropic", ""), "https://api.anthropic.com/v1");
        assert_eq!(cfg.default_model("anthropic").as_deref(), Some("claude-sonnet-4-6"));
    }

    #[test]
    fn load_json() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("llm.json");
        std::fs::write(
            &path,
            r#"{"default_provider":"openai","providers":{"openai":{"api_key_env":"OPENAI_API_KEY"}}}"#,
        )
        .unwrap();

        let cfg = LlmConfig::load(&path).unwrap();
        assert_eq!(cfg.default_provider.as_deref(), Some("openai"));
    }

    #[test]
    fn missing_api_key_returns_error() {
        let cfg = LlmConfig {
            providers: {
                let mut m = HashMap::new();
                m.insert(
                    "anthropic".to_string(),
                    ProviderConfig {
                        api_key_env: Some("DEFINITELY_NOT_SET_KEY".to_string()),
                        base_url: None,
                        default_model: None,
                        extras: HashMap::new(),
                    },
                );
                m
            },
            ..Default::default()
        };

        let err = cfg.api_key("anthropic").unwrap_err();
        assert!(matches!(err, LlmError::MissingApiKey(_, _)));
    }
}
