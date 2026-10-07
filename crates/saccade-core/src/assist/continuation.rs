//! Offline campaign transport continuation and safety history.
use serde_json::{Value, json};

/// Declared transport safety valve, shared by both runners.
#[derive(Default)]
pub struct History {
    attempts: usize,
    failures: usize,
    consecutive: usize,
}
impl History {
    /// Record only physical settled outcomes, including restored campaign history.
    pub fn observe(&mut self, code: &str) -> Option<&'static str> {
        match code {
            "not_run_transport_failure" => {
                self.attempts += 1;
                self.failures += 1;
                self.consecutive += 1;
            }
            "completed" | "invalid_answer" => {
                self.attempts += 1;
                self.consecutive = 0;
            }
            _ => return None,
        }
        if self.consecutive > 5 {
            Some("consecutive_transport_failures")
        } else if self.attempts >= 20 && self.failures * 100 > self.attempts * 10 {
            Some("transport_failure_rate")
        } else {
            None
        }
    }
    /// Persist the declared policy alongside campaign outcomes.
    pub fn policy() -> Value {
        json!({"max_consecutive":5,"max_percent":10,"min_sample":20})
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn continuation_valves_include_history_and_ignore_unavailable_tail() {
        let mut history = History::default();
        for _ in 0..5 {
            assert_eq!(history.observe("not_run_transport_failure"), None);
        }
        assert_eq!(
            history.observe("not_run_transport_failure"),
            Some("consecutive_transport_failures")
        );
        let mut history = History::default();
        for i in 0..20 {
            let valve = history.observe(if i % 7 == 0 {
                "not_run_transport_failure"
            } else {
                "completed"
            });
            assert_eq!(
                valve,
                if i == 19 {
                    Some("transport_failure_rate")
                } else {
                    None
                }
            );
        }
        assert_eq!(history.observe("not_run"), None);
    }
}
