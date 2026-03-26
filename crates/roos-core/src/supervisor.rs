use std::sync::Arc;

use async_trait::async_trait;
use serde::Deserialize;

use crate::{
    agent::Agent,
    bus::RoosAgentBus,
    error::AgentError,
    provider::{CompletionConfig, LLMProvider, Message},
    types::{AgentInput, AgentOutput},
};

/// Description of a worker agent the supervisor can dispatch tasks to.
#[derive(Debug, Clone)]
pub struct WorkerSpec {
    pub name: String,
    pub description: String,
}

/// Internal shape expected from the LLM decomposition response.
#[derive(Debug, Deserialize)]
struct Subtask {
    agent: String,
    task: String,
}

/// Orchestrator-worker supervisor that decomposes a task using an LLM,
/// dispatches subtasks to registered workers via [`RoosAgentBus`], and
/// aggregates their replies into a final output.
///
/// # Usage
/// ```ignore
/// let supervisor = SupervisorAgent::new("planner", "Plans and delegates", provider, bus, "gpt-4o")
///     .with_worker("researcher", "Searches and summarises information")
///     .with_worker("writer", "Writes polished prose from notes");
/// ```
pub struct SupervisorAgent {
    name: String,
    description: String,
    provider: Arc<dyn LLMProvider>,
    bus: RoosAgentBus,
    workers: Vec<WorkerSpec>,
    model: String,
}

impl SupervisorAgent {
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        provider: Arc<dyn LLMProvider>,
        bus: RoosAgentBus,
        model: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            provider,
            bus,
            workers: Vec::new(),
            model: model.into(),
        }
    }

    /// Register a worker agent that the supervisor may delegate to.
    pub fn with_worker(mut self, name: impl Into<String>, description: impl Into<String>) -> Self {
        self.workers.push(WorkerSpec {
            name: name.into(),
            description: description.into(),
        });
        self
    }

    fn system_prompt(&self) -> String {
        let worker_list = self
            .workers
            .iter()
            .map(|w| format!("- {}: {}", w.name, w.description))
            .collect::<Vec<_>>()
            .join("\n");

        format!(
            "You are a supervisor agent. Decompose the user's task into subtasks \
             and assign each to the most suitable worker.\n\n\
             Available workers:\n{worker_list}\n\n\
             Reply ONLY with a JSON array (no markdown, no prose):\n\
             [{{\"agent\":\"<worker_name>\",\"task\":\"<subtask_description>\"}}]"
        )
    }

    /// Extract and parse the JSON array from potentially noisy LLM output.
    fn parse_subtasks(content: &str) -> Result<Vec<Subtask>, AgentError> {
        let start = content.find('[').ok_or_else(|| {
            AgentError::ConfigurationError("supervisor LLM did not return a JSON array".into())
        })?;
        let end = content.rfind(']').ok_or_else(|| {
            AgentError::ConfigurationError("supervisor LLM response has unmatched '['".into())
        })?;
        serde_json::from_str(&content[start..=end]).map_err(|e| {
            AgentError::ConfigurationError(format!("failed to parse supervisor subtasks: {e}"))
        })
    }
}

#[async_trait]
impl Agent for SupervisorAgent {
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        &self.description
    }

    async fn run(&self, input: AgentInput) -> Result<AgentOutput, AgentError> {
        // Step 1 — ask the LLM to decompose the task.
        let mut config = CompletionConfig::new(&self.model);
        config.system = Some(self.system_prompt());

        let messages = vec![Message::user(&input.content)];
        let response = self
            .provider
            .complete(&messages, &config)
            .await
            .map_err(|e| AgentError::ProviderError(e.to_string()))?;

        let text = response.content.ok_or_else(|| {
            AgentError::ConfigurationError("supervisor LLM returned no text content".into())
        })?;
        let subtasks = Self::parse_subtasks(&text)?;

        // Step 2 — dispatch each subtask to its assigned worker via the bus.
        let mut results = Vec::with_capacity(subtasks.len());
        for st in &subtasks {
            let reply = self
                .bus
                .send(&st.agent, &st.task)
                .await
                .map_err(|e| AgentError::ProviderError(e.to_string()))?;
            results.push(format!("[{}]: {}", st.agent, reply));
        }

        Ok(AgentOutput {
            content: results.join("\n"),
            steps_taken: subtasks.len(),
            tools_called: vec![],
            total_tokens: response.usage,
            run_id: input.run_id,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        bus::RoosAgentBus,
        provider::{CompletionResponse, ProviderError, StopReason},
        types::TokenUsage,
    };

    // ── Stub LLM provider ─────────────────────────────────────────────────────

    struct StubProvider {
        response: String,
    }

    #[async_trait]
    impl LLMProvider for StubProvider {
        async fn complete(
            &self,
            _messages: &[Message],
            _config: &CompletionConfig,
        ) -> Result<CompletionResponse, ProviderError> {
            Ok(CompletionResponse {
                content: Some(self.response.clone()),
                stop_reason: StopReason::EndTurn,
                usage: TokenUsage {
                    input: 10,
                    output: 20,
                    total: 30,
                },
                tool_calls: vec![],
                model: "stub".into(),
            })
        }
    }

    fn stub(response: &str) -> Arc<StubProvider> {
        Arc::new(StubProvider {
            response: response.to_owned(),
        })
    }

    // ── Tests ─────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn supervisor_dispatches_subtasks_and_aggregates() {
        let bus = RoosAgentBus::new();

        // Register two workers that echo their input.
        let mut rx_a = bus.register("alpha");
        let mut rx_b = bus.register("beta");
        tokio::spawn(async move {
            if let Some(m) = rx_a.recv().await {
                let _ = m.reply_tx.send(format!("alpha-done: {}", m.input));
            }
        });
        tokio::spawn(async move {
            if let Some(m) = rx_b.recv().await {
                let _ = m.reply_tx.send(format!("beta-done: {}", m.input));
            }
        });

        let llm_json = r#"[{"agent":"alpha","task":"task A"},{"agent":"beta","task":"task B"}]"#;
        let supervisor =
            SupervisorAgent::new("sup", "test supervisor", stub(llm_json), bus, "test-model")
                .with_worker("alpha", "does A")
                .with_worker("beta", "does B");

        let out = supervisor.run(AgentInput::new("do stuff")).await.unwrap();
        assert!(out.content.contains("[alpha]: alpha-done: task A"));
        assert!(out.content.contains("[beta]: beta-done: task B"));
        assert_eq!(out.steps_taken, 2);
        assert_eq!(out.total_tokens.total, 30);
    }

    #[tokio::test]
    async fn invalid_llm_response_returns_configuration_error() {
        let bus = RoosAgentBus::new();
        let supervisor = SupervisorAgent::new("sup", "test", stub("not json at all"), bus, "m");
        let err = supervisor.run(AgentInput::new("task")).await.unwrap_err();
        assert!(matches!(err, AgentError::ConfigurationError(_)));
    }

    #[tokio::test]
    async fn unregistered_worker_returns_error() {
        let bus = RoosAgentBus::new();
        let llm_json = r#"[{"agent":"ghost","task":"haunt"}]"#;
        let supervisor = SupervisorAgent::new("sup", "test", stub(llm_json), bus, "m");
        let err = supervisor.run(AgentInput::new("task")).await.unwrap_err();
        assert!(matches!(err, AgentError::ProviderError(_)));
    }

    #[test]
    fn parse_subtasks_strips_surrounding_text() {
        let noisy = "Sure! Here you go:\n[{\"agent\":\"w\",\"task\":\"t\"}]\nDone.";
        let subtasks = SupervisorAgent::parse_subtasks(noisy).unwrap();
        assert_eq!(subtasks.len(), 1);
        assert_eq!(subtasks[0].agent, "w");
        assert_eq!(subtasks[0].task, "t");
    }

    #[test]
    fn with_worker_is_chainable() {
        let bus = RoosAgentBus::new();
        let sup = SupervisorAgent::new("s", "d", stub("[]"), bus, "m")
            .with_worker("w1", "desc1")
            .with_worker("w2", "desc2");
        assert_eq!(sup.workers.len(), 2);
        assert_eq!(sup.workers[0].name, "w1");
        assert_eq!(sup.workers[1].name, "w2");
    }
}
