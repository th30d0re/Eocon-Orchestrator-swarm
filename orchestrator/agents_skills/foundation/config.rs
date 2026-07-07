use std::path::{Path, PathBuf};

use crate::config::AppConfig;

/// Configuration for locating the Foundation Phase 5 agent scripts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FoundationConfig {
    /// Absolute path to the Eocon-Foundation-V1 checkout root.
    pub root: PathBuf,
}

impl FoundationConfig {
    /// Resolve the Foundation root from, in order of precedence:
    /// 1. The `EOCON_FOUNDATION_ROOT` environment variable.
    /// 2. The `eocon_foundation_root` field in the on-disk `AppConfig`.
    /// 3. The default `~/Dev/Tools/Eocon-Foundation-V1`.
    pub fn load() -> Self {
        if let Ok(root) = std::env::var("EOCON_FOUNDATION_ROOT") {
            return Self::new(root);
        }

        if let Ok(app_config) = confy::load::<AppConfig>("eocon-swarm", None) {
            if let Some(root) = app_config.eocon_foundation_root {
                return Self::new(root);
            }
        }

        Self::default()
    }

    /// Create a config from a path string, expanding a leading `~` to the
    /// user's home directory when possible.
    pub fn new(root: impl AsRef<Path>) -> Self {
        Self {
            root: expand_tilde(root.as_ref()),
        }
    }

    /// Path to the Phase 5 BuildAgent script.
    pub fn build_agent_script(&self) -> PathBuf {
        self.root
            .join(".Foundation")
            .join("Agents")
            .join("phases")
            .join("phase5")
            .join("BuildAgent.sh")
    }

    /// Path to the Phase 5 TestAgent script.
    pub fn test_agent_script(&self) -> PathBuf {
        self.root
            .join(".Foundation")
            .join("Agents")
            .join("phases")
            .join("phase5")
            .join("testAgent.sh")
    }
}

impl Default for FoundationConfig {
    fn default() -> Self {
        Self::new("~/Dev/Tools/Eocon-Foundation-V1")
    }
}

fn expand_tilde(path: &Path) -> PathBuf {
    let s = path.to_string_lossy();
    if let Some(rest) = s.strip_prefix("~/") {
        dirs::home_dir()
            .map(|home| home.join(rest))
            .unwrap_or_else(|| path.to_path_buf())
    } else if s == "~" {
        dirs::home_dir().unwrap_or_else(|| path.to_path_buf())
    } else {
        path.to_path_buf()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_root_expands_tilde() {
        let cfg = FoundationConfig::default();
        let home = dirs::home_dir().expect("home dir should be available");
        assert_eq!(cfg.root, home.join("Dev/Tools/Eocon-Foundation-V1"));
    }

    #[test]
    fn explicit_absolute_path_is_preserved() {
        let cfg = FoundationConfig::new("/opt/foundation");
        assert_eq!(cfg.root, PathBuf::from("/opt/foundation"));
    }

    #[test]
    fn build_agent_script_path() {
        let cfg = FoundationConfig::new("/opt/foundation");
        assert_eq!(
            cfg.build_agent_script(),
            PathBuf::from("/opt/foundation/.Foundation/Agents/phases/phase5/BuildAgent.sh")
        );
    }

    #[test]
    fn test_agent_script_path() {
        let cfg = FoundationConfig::new("/opt/foundation");
        assert_eq!(
            cfg.test_agent_script(),
            PathBuf::from("/opt/foundation/.Foundation/Agents/phases/phase5/testAgent.sh")
        );
    }
}
