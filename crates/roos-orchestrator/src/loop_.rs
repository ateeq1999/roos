use std::collections::HashMap;
use std::sync::Arc;

use chrono::Utc;
use roos_core::error::AgentError;
use roos_core::memory::{ConversationMessage, Memory};
use roos_core::provider::{CompletionConfig, LLMProvider, Message, StopReason, ToolCall};
use roos_core::tool::Tool;
use roos_core::types::{AgentInput, AgentOutput, TokenUsage, ToolCallRecord};

use crate::state::AgentState;

/// Drives the Reasoning → Action → Observation loop for one agent run.
///
/// Construct with a provider, a set of tools, and a memory backend.
/// Call [`run`](ReasoningLoop::run) with the agent input and completion config.
pub struct ReasoningLoop {
    provider: Arc<dyn LLMProvider>,
    /// Tool registry keyed by tool name.
    tools: HashMap<String, Arc<dyn Tool>>,
    memory: Arc<dyn Memory>,
}

impl ReasoningLoop {
    pub fn new(
        provider: Arc<dyn LLMProvider>,
        tools: Vec<Arc<dyn Tool>>,
        memory: Arc<dyn Memory>,
    ) -> Self {
        let tools = tools
            .into_iter()
            .map(|t| (t.name().to_owned(), t))
            .collect();
        Self {
            provider,
            tools,
            memory,
        }
    }

    /// Execute one agent run from start to finish.
    pub async fn run(
        &self,
        input: AgentInput,
        config: &CompletionConfig,
    ) -> Result<AgentOutput, AgentError> {
        let max_steps = input.max_steps.unwrap_or(10);
        let run_id = input.run_id;

        // Build message history from persisted memory + this turn's input.
        let mut messages: Vec<Message> = Vec::new();
        if let Some(history) = self
            .memory
            .load(run_id)
            .await
            .map_err(|e| AgentError::MemoryError(e.to_string()))?
        {
            for m in &history.messages {
                messages.push(Message {
                    role: m.role.clone(),
                    content: m.content.clone(),
                });
            }
        }
        messages.push(Message::user(&input.content));

        let mut state = AgentState::Idle
            .start()
            .map_err(|e| AgentError::ConfigurationError(e.to_string()))?;
        let mut total_tokens = TokenUsage::default();
        let mut tools_called: Vec<ToolCallRecord> = Vec::new();

        loop {
            let step = state.step();

            if step > max_steps {
                return Err(AgentError::MaxStepsExceeded(max_steps));
            }

            // ── Reasoning: call the LLM ───────────────────────────────────────
            let response = self
                .provider
                .complete(&messages, config)
                .await
                .map_err(|e| AgentError::ProviderError(e.to_string()))?;

            total_tokens.add(&response.usage);

            if let Some(ref text) = response.content {
                messages.push(Message::assistant(text));
            }

            match response.stop_reason {
                // ── Finished ──────────────────────────────────────────────────
                StopReason::EndTurn | StopReason::StopSequence => {
                    let content = response.content.unwrap_or_default();
                    self.memory
                        .append(run_id, ConversationMessage::assistant(&content))
                        .await
                        .map_err(|e| AgentError::MemoryError(e.to_string()))?;
                    return Ok(AgentOutput {
                        content,
                        steps_taken: step,
                        tools_called,
                        total_tokens,
                        run_id,
                    });
                }

                // ── Action: execute tool calls (Observation appended inline) ──
                StopReason::ToolUse => {
                    state = state
                        .call_tool("batch")
                        .map_err(|e| AgentError::ConfigurationError(e.to_string()))?;

                    for tool_call in response.tool_calls {
                        let record = self.execute_tool(&tool_call, step).await;
                        let observation = match &record.output {
                            Some(out) => out.clone(),
                            None => {
                                format!("error: {}", record.error.as_deref().unwrap_or("unknown"))
                            }
                        };
                        messages.push(Message {
                            role: "tool".into(),
                            content: observation,
                        });
                        tools_called.push(record);
                    }

                    state = state
                        .tool_done()
                        .map_err(|e| AgentError::ConfigurationError(e.to_string()))?
                        .continue_reasoning()
                        .map_err(|e| AgentError::ConfigurationError(e.to_string()))?;
                }

                StopReason::MaxTokens => return Err(AgentError::ContextWindowExceeded),
            }
        }
    }

    /// Execute a single tool call and return an audit record.
    ///
    /// Tool failures are captured in `ToolCallRecord.error` rather than
    /// aborting the run, so the LLM can observe and recover from them.
    async fn execute_tool(&self, call: &ToolCall, step: usize) -> ToolCallRecord {
        let start = std::time::Instant::now();
        let timestamp = Utc::now();

        let (output, error) = match self.tools.get(&call.name) {
            None => (None, Some(format!("no tool named '{}'", call.name))),
            Some(tool) => match tool.execute(call.input.clone()).await {
                Ok(out) => (Some(out), None),
                Err(e) => (None, Some(e.to_string())),
            },
        };

        ToolCallRecord {
            tool_name: call.name.clone(),
            input: call.input.clone(),
            output,
            error,
            duration_ms: start.elapsed().as_millis() as u64,
            step,
            timestamp,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use async_trait::async_trait;
    use roos_core::provider::{CompletionResponse, ProviderError, ToolCall};
    use roos_core::types::TokenUsage;
    use roos_core::{AgentInput, CompletionConfig, LLMProvider, Message, StopReason};
    use roos_memory::InMemoryStore;

    use super::*;

    // ── Stub: always answers with fixed text ──────────────────────────────────

    struct StaticProvider(CompletionResponse);

    impl StaticProvider {
        fn answering(text: &str) -> Self {
            Self(CompletionResponse {
                content: Some(text.to_owned()),
                tool_calls: vec![],
                usage: TokenUsage {
                    input: 5,
                    output: 5,
                    total: 10,
                },
                model: "test".into(),
                stop_reason: StopReason::EndTurn,
            })
        }
    }

    #[async_trait]
    impl LLMProvider for StaticProvider {
        async fn complete(
            &self,
            _msgs: &[Message],
            _cfg: &CompletionConfig,
        ) -> Result<CompletionResponse, ProviderError> {
            Ok(self.0.clone())
        }
    }

    // ── Stub: always requests a (non-existent) tool ───────────────────────────

    struct LoopingProvider;

    #[async_trait]
    impl LLMProvider for LoopingProvider {
        async fn complete(
            &self,
            _: &[Message],
            _: &CompletionConfig,
        ) -> Result<CompletionResponse, ProviderError> {
            Ok(CompletionResponse {
                content: None,
                tool_calls: vec![ToolCall {
                    id: "1".into(),
                    name: "noop".into(),
                    input: serde_json::json!({}),
                }],
                usage: TokenUsage::default(),
                model: "test".into(),
                stop_reason: StopReason::ToolUse,
            })
        }
    }

    fn memory() -> Arc<InMemoryStore> {
        Arc::new(InMemoryStore::new())
    }

    // ── Tests ─────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn returns_provider_content() {
        let rl = ReasoningLoop::new(
            Arc::new(StaticProvider::answering("pong")),
            vec![],
            memory(),
        );
        let out = rl
            .run(AgentInput::new("ping"), &CompletionConfig::new("t"))
            .await
            .unwrap();
        assert_eq!(out.content, "pong");
        assert_eq!(out.steps_taken, 1);
        assert_eq!(out.total_tokens.total, 10);
    }

    #[tokio::test]
    async fn exceeds_max_steps_returns_error() {
        let rl = ReasoningLoop::new(Arc::new(LoopingProvider), vec![], memory());
        let mut input = AgentInput::new("go");
        input.max_steps = Some(2);
        let err = rl
            .run(input, &CompletionConfig::new("t"))
            .await
            .unwrap_err();
        assert!(matches!(err, AgentError::MaxStepsExceeded(2)));
    }

    #[tokio::test]
    async fn unknown_tool_recorded_not_fatal() {
        // LoopingProvider requests "noop" (not registered). The run should
        // record the error in ToolCallRecord and continue until max_steps.
        let rl = ReasoningLoop::new(Arc::new(LoopingProvider), vec![], memory());
        let mut input = AgentInput::new("go");
        input.max_steps = Some(1);
        // step 1 → ToolUse → tool error recorded → step 2 > 1 → MaxStepsExceeded
        let err = rl
            .run(input, &CompletionConfig::new("t"))
            .await
            .unwrap_err();
        assert!(matches!(err, AgentError::MaxStepsExceeded(1)));
    }
}
