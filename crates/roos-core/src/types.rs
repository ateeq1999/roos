use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Token counts accumulated across a single agent run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct TokenUsage {
    /// Tokens sent to the LLM (prompt + history).
    pub input: usize,
    /// Tokens received from the LLM.
    pub output: usize,
    /// `input + output`.
    pub total: usize,
}

impl TokenUsage {
    /// Accumulate another [`TokenUsage`] into `self`.
    pub fn add(&mut self, other: &TokenUsage) {
        self.input += other.input;
        self.output += other.output;
        self.total += other.total;
    }
}

/// Audit record for one tool invocation within an agent run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallRecord {
    /// Name of the tool that was called.
    pub tool_name: String,
    /// JSON input supplied by the LLM.
    pub input: serde_json::Value,
    /// Stringified tool output, if successful.
    pub output: Option<String>,
    /// Error message, if the tool failed.
    pub error: Option<String>,
    /// Wall-clock duration of the tool execution in milliseconds.
    pub duration_ms: u64,
    /// Reasoning-loop step number during which the tool was called.
    pub step: usize,
    /// UTC timestamp at the start of tool execution (ISO 8601).
    pub timestamp: DateTime<Utc>,
}

/// Input supplied to an agent run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentInput {
    /// The human or system message that initiates the run.
    pub content: String,
    /// Arbitrary key-value context passed alongside the content.
    pub context: HashMap<String, serde_json::Value>,
    /// Unique identifier for this run (used for tracing correlation).
    pub run_id: Uuid,
    /// Override for the agent's `max_steps` setting. `None` uses the default.
    pub max_steps: Option<usize>,
}

impl AgentInput {
    /// Construct a minimal [`AgentInput`] with a generated `run_id`.
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            context: HashMap::new(),
            run_id: Uuid::new_v4(),
            max_steps: None,
        }
    }
}

/// Output produced by a completed agent run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentOutput {
    /// The agent's final response text.
    pub content: String,
    /// Number of Reasoning → Action → Observation cycles executed.
    pub steps_taken: usize,
    /// Ordered audit trail of every tool invocation during the run.
    pub tools_called: Vec<ToolCallRecord>,
    /// Aggregate token usage across all LLM calls in this run.
    pub total_tokens: TokenUsage,
    /// Echoed from [`AgentInput::run_id`] for end-to-end correlation.
    pub run_id: Uuid,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_input_new_has_unique_run_ids() {
        let a = AgentInput::new("hello");
        let b = AgentInput::new("hello");
        assert_ne!(a.run_id, b.run_id);
    }

    #[test]
    fn agent_input_new_empty_context() {
        let input = AgentInput::new("test");
        assert!(input.context.is_empty());
        assert!(input.max_steps.is_none());
    }

    #[test]
    fn token_usage_add() {
        let mut total = TokenUsage::default();
        total.add(&TokenUsage {
            input: 10,
            output: 5,
            total: 15,
        });
        total.add(&TokenUsage {
            input: 20,
            output: 10,
            total: 30,
        });
        assert_eq!(total.input, 30);
        assert_eq!(total.output, 15);
        assert_eq!(total.total, 45);
    }

    #[test]
    fn token_usage_default_is_zero() {
        let u = TokenUsage::default();
        assert_eq!(u.input, 0);
        assert_eq!(u.output, 0);
        assert_eq!(u.total, 0);
    }

    #[test]
    fn tool_call_record_roundtrips_json() {
        let record = ToolCallRecord {
            tool_name: "read_file".into(),
            input: serde_json::json!({ "path": "/tmp/x" }),
            output: Some("contents".into()),
            error: None,
            duration_ms: 42,
            step: 1,
            timestamp: Utc::now(),
        };
        let json = serde_json::to_string(&record).unwrap();
        let back: ToolCallRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(back.tool_name, "read_file");
        assert_eq!(back.duration_ms, 42);
    }
}
