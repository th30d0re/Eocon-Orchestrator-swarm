#[derive(Debug, Clone, Default)]
pub struct SwarmMetrics {
    pub successes: u64,
    pub failures: u64,
    pub compliance_events: u64,
    pub violations: u64,
    pub total_tokens_consumed: u64,
    pub delegation_latency_ms: u64,
}

impl SwarmMetrics {
    pub fn record_success(&mut self) {
        self.successes += 1;
    }

    pub fn record_failure(&mut self) {
        self.failures += 1;
    }

    pub fn record_compliance(&mut self) {
        self.compliance_events += 1;
    }

    pub fn record_violation(&mut self) {
        self.violations += 1;
    }

    pub fn record_tokens(&mut self, tokens: u64) {
        self.total_tokens_consumed += tokens;
    }

    pub fn record_latency(&mut self, ms: u64) {
        if self.delegation_latency_ms == 0 {
            self.delegation_latency_ms = ms;
        } else {
            self.delegation_latency_ms = (self.delegation_latency_ms + ms) / 2;
        }
    }

    pub fn success_rate(&self) -> f64 {
        let total = self.successes + self.failures;
        if total == 0 {
            0.0
        } else {
            self.successes as f64 / total as f64
        }
    }

    pub fn compliance_rate(&self) -> f64 {
        let total = self.compliance_events + self.violations;
        if total == 0 {
            1.0
        } else {
            self.compliance_events as f64 / total as f64
        }
    }

    /// Compliance-weighted success rate.
    pub fn csr(&self, alpha: f64) -> f64 {
        let sr = self.success_rate();
        let cr = self.compliance_rate();
        sr * (1.0 - alpha * (1.0 - cr))
    }

    /// Machiavellian Gap placeholder.
    pub fn machiavellian_gap(&self) -> f64 {
        (1.0 - self.compliance_rate()) * self.success_rate()
    }
}
