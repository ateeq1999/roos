use async_trait::async_trait;

use crate::error::AgentError;
use crate::types::{AgentInput, AgentOutput};

/// Top-level abstraction for a ROOS agent.
///
/// An `Agent` encapsulates a reasoning loop wired to an [`LLMProvider`],
/// zero or more [`Tool`]s, and a [`Memory`] backend. Callers interact only
/// through [`run`](Agent::run); internal coordination is an implementation
/// detail of each concrete agent (typically [`BaseAgent`] in
/// `roos-orchestrator`).
///
/// [`LLMProvider`]: crate::LLMProvider
/// [`Tool`]: crate::Tool
/// [`Memory`]: crate::Memory
///
/// # Object safety
/// `Agent` is object-safe: store behind `Arc<dyn Agent>` to share across
/// async tasks or expose via the HTTP trigger layer.
#[async_trait]
pub trait Agent: Send + Sync {
    /// Unique, stable identifier for this agent (e.g. `"summariser"`).
    /// Appears in logs, API responses, and the TUI dashboard.
    fn name(&self) -> &str;

    /// Plain-English description of what this agent does.
    fn description(&self) -> &str;

    /// Execute one agent run and return the final output.
    ///
    /// The implementation is responsible for the full
    /// Reasoning → Action → Observation loop, memory reads/writes,
    /// and populating [`AgentOutput`] with the audit trail.
    async fn run(&self, input: AgentInput) -> Result<AgentOutput, AgentError>;
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::*;
    use crate::types::TokenUsage;

    struct GreetAgent;

    #[async_trait]
    impl Agent for GreetAgent {
        fn name(&self) -> &str {
            "greet"
        }
        fn description(&self) -> &str {
            "Returns a greeting."
        }
        async fn run(&self, input: AgentInput) -> Result<AgentOutput, AgentError> {
            Ok(AgentOutput {
                content: format!("Hello, {}!", input.content),
                steps_taken: 1,
                tools_called: vec![],
                total_tokens: TokenUsage::default(),
                run_id: input.run_id,
            })
        }
    }

    #[tokio::test]
    async fn agent_run_returns_output() {
        let agent = GreetAgent;
        let input = AgentInput::new("world");
        let output = agent.run(input.clone()).await.unwrap();
        assert_eq!(output.content, "Hello, world!");
        assert_eq!(output.run_id, input.run_id);
        assert_eq!(output.steps_taken, 1);
    }

    #[tokio::test]
    async fn agent_is_object_safe() {
        let agent: Box<dyn Agent> = Box::new(GreetAgent);
        assert_eq!(agent.name(), "greet");
        let id = Uuid::new_v4();
        let input = AgentInput {
            content: "ROOS".into(),
            context: Default::default(),
            run_id: id,
            max_steps: None,
        };
        let out = agent.run(input).await.unwrap();
        assert_eq!(out.run_id, id);
    }
}
