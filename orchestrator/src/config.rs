use serde::{Deserialize, Serialize};

/// Execution backend for agent commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, clap::ValueEnum)]
pub enum RunnerBackend {
    /// Local PTY execution (default).
    #[default]
    Pty,
    /// Apple Container Machine sandboxing.
    Container,
}

impl RunnerBackend {
    /// Parse a backend string such as `pty` or `container` (case-insensitive).
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "pty" => Some(RunnerBackend::Pty),
            "container" => Some(RunnerBackend::Container),
            _ => None,
        }
    }
}

/// On-disk application configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub runner_backend: RunnerBackend,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            runner_backend: RunnerBackend::Pty,
        }
    }
}
