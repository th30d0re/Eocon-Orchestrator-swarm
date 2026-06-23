use std::collections::HashMap;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, PartialEq)]
pub enum TerminalKind {
    Succeeded,
    Refusal,
    GuardrailViolation,
    NonRetryableExit(i32),
    ProviderMissing,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AgentOutcome {
    Progressing,
    Stalled,
    Terminal(TerminalKind),
}

#[derive(Debug, Clone, PartialEq)]
pub enum GenerationErrorKind {
    Refusal,
    GuardrailViolation,
    ExceededContextWindow,
}

#[derive(Debug, Clone)]
pub struct AgentSignal {
    pub agent_id: String,
    pub last_progress_at: Instant,
    pub idle_for: Duration,
    pub tokens_since_progress: u64,
    pub exit_code: Option<i32>,
    pub last_chunk: Option<String>,
    pub generation_error: Option<GenerationErrorKind>,
}

pub trait OutcomeClassifier: Send + Sync {
    fn classify(&self, signal: &AgentSignal) -> AgentOutcome;
}

pub struct PtyOutcomeClassifier {
    pub stall_timeout: Duration,
    pub max_tokens_without_progress: u64,
}

impl Default for PtyOutcomeClassifier {
    fn default() -> Self {
        Self {
            stall_timeout: Duration::from_secs(300),
            max_tokens_without_progress: 8192,
        }
    }
}

impl OutcomeClassifier for PtyOutcomeClassifier {
    fn classify(&self, signal: &AgentSignal) -> AgentOutcome {
        if let Some(ref err) = signal.generation_error {
            return match err {
                GenerationErrorKind::Refusal => AgentOutcome::Terminal(TerminalKind::Refusal),
                GenerationErrorKind::GuardrailViolation => {
                    AgentOutcome::Terminal(TerminalKind::GuardrailViolation)
                }
                GenerationErrorKind::ExceededContextWindow => AgentOutcome::Progressing,
            };
        }

        if let Some(code) = signal.exit_code {
            return match code {
                0 => AgentOutcome::Terminal(TerminalKind::Succeeded),
                11 => AgentOutcome::Terminal(TerminalKind::NonRetryableExit(code)),
                _ => AgentOutcome::Terminal(TerminalKind::NonRetryableExit(code)),
            };
        }

        if signal.idle_for > self.stall_timeout
            || signal.tokens_since_progress > self.max_tokens_without_progress
        {
            return AgentOutcome::Stalled;
        }

        AgentOutcome::Progressing
    }
}

pub struct WatchdogState {
    pub last_progress: HashMap<String, Instant>,
    pub tokens_since_progress: HashMap<String, u64>,
    pub stall_timeout: Duration,
    pub max_tokens_without_progress: u64,
    pub classifier: Box<dyn OutcomeClassifier>,
}

impl Default for WatchdogState {
    fn default() -> Self {
        Self {
            last_progress: HashMap::new(),
            tokens_since_progress: HashMap::new(),
            stall_timeout: Duration::from_secs(300),
            max_tokens_without_progress: 8192,
            classifier: Box::new(PtyOutcomeClassifier::default()),
        }
    }
}

impl WatchdogState {
    pub fn record_progress(&mut self, agent_id: &str, tokens: u64) {
        self.last_progress
            .insert(agent_id.to_string(), Instant::now());
        self.tokens_since_progress
            .insert(agent_id.to_string(), tokens);
    }

    pub fn check_stalls(&self) -> Vec<(String, AgentOutcome)> {
        let now = Instant::now();
        self.last_progress
            .iter()
            .map(|(id, last)| {
                let idle = now.duration_since(*last);
                let tokens = self.tokens_since_progress.get(id).copied().unwrap_or(0);
                let signal = AgentSignal {
                    agent_id: id.clone(),
                    last_progress_at: *last,
                    idle_for: idle,
                    tokens_since_progress: tokens,
                    exit_code: None,
                    last_chunk: None,
                    generation_error: None,
                };
                (id.clone(), self.classifier.classify(&signal))
            })
            .collect()
    }

    pub fn stalling_agents(&self) -> Vec<String> {
        self.check_stalls()
            .into_iter()
            .filter(|(_, outcome)| matches!(outcome, AgentOutcome::Stalled))
            .map(|(id, _)| id)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    #[test]
    fn classifier_detects_stall() {
        let classifier = PtyOutcomeClassifier {
            stall_timeout: Duration::from_millis(1),
            max_tokens_without_progress: 100,
        };
        let signal = AgentSignal {
            agent_id: "a1".to_string(),
            last_progress_at: Instant::now() - Duration::from_secs(10),
            idle_for: Duration::from_secs(10),
            tokens_since_progress: 0,
            exit_code: None,
            last_chunk: None,
            generation_error: None,
        };
        assert_eq!(classifier.classify(&signal), AgentOutcome::Stalled);
    }

    #[test]
    fn classifier_detects_success_exit() {
        let classifier = PtyOutcomeClassifier::default();
        let signal = AgentSignal {
            agent_id: "a1".to_string(),
            last_progress_at: Instant::now(),
            idle_for: Duration::ZERO,
            tokens_since_progress: 0,
            exit_code: Some(0),
            last_chunk: None,
            generation_error: None,
        };
        assert_eq!(
            classifier.classify(&signal),
            AgentOutcome::Terminal(TerminalKind::Succeeded)
        );
    }
}
