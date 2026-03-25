use std::fmt;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::types::TokenUsage;

// ── Message ──────────────────────────────────────────────────────────────────

/// A single turn in the conversation sent to the LLM.
///
/// Roles follow the OpenAI/Anthropic convention: `"user"`, `"assistant"`,
/// `"system"`, or `"tool"`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: String,
}

impl Message {
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: "user".into(),
            content: content.into(),
        }
    }
    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: "assistant".into(),
            content: content.into(),
        }
    }
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: "system".into(),
            content: content.into(),
        }
    }
}

// ── ToolSchema ───────────────────────────────────────────────────────────────

/// JSON Schema description of a tool exposed to the LLM.
///
/// Built from [`Tool::name`](crate::Tool::name),
/// [`Tool::description`](crate::Tool::description), and
/// [`Tool::schema`](crate::Tool::schema).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSchema {
    pub name: String,
    pub description: String,
    /// JSON Schema (draft-07) for the tool's input object.
    pub parameters: serde_json::Value,
}

// ── ToolCall ─────────────────────────────────────────────────────────────────

/// A tool invocation requested by the LLM in a `CompletionResponse`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    /// Provider-assigned call identifier (used to correlate results).
    pub id: String,
    pub name: String,
    pub input: serde_json::Value,
}

// ── StopReason ───────────────────────────────────────────────────────────────

/// Why the LLM stopped generating.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum StopReason {
    /// Model completed its response naturally.
    EndTurn,
    /// Model wants to invoke one or more tools.
    ToolUse,
    /// The response was cut off at the token limit.
    MaxTokens,
    /// A configured stop sequence was reached.
    StopSequence,
}

// ── CompletionConfig ─────────────────────────────────────────────────────────

/// Parameters for a single LLM completion call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionConfig {
    /// Model identifier (e.g. `"claude-sonnet-4-6"`, `"gpt-4o"`).
    pub model: String,
    /// Maximum tokens the model may generate. `None` uses the provider default.
    pub max_tokens: Option<u32>,
    /// Sampling temperature in `[0.0, 2.0]`. `None` uses the provider default.
    pub temperature: Option<f32>,
    /// Tools available to the model for this call.
    pub tools: Vec<ToolSchema>,
    /// Optional system prompt override for this call.
    pub system: Option<String>,
}

impl CompletionConfig {
    pub fn new(model: impl Into<String>) -> Self {
        Self {
            model: model.into(),
            max_tokens: None,
            temperature: None,
            tools: Vec::new(),
            system: None,
        }
    }
}

// ── CompletionResponse ───────────────────────────────────────────────────────

/// The result of a single LLM completion call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionResponse {
    /// Final text, if the model produced one (absent when `stop_reason == ToolUse`).
    pub content: Option<String>,
    /// Tool invocations requested by the model.
    pub tool_calls: Vec<ToolCall>,
    /// Token usage for this call.
    pub usage: TokenUsage,
    /// Model that produced the response.
    pub model: String,
    /// Reason the model stopped generating.
    pub stop_reason: StopReason,
}

// ── ProviderError ─────────────────────────────────────────────────────────────

/// Errors returned by [`LLMProvider`] implementations.
#[derive(Debug)]
pub enum ProviderError {
    /// API key missing or rejected (HTTP 401/403).
    Unauthorized { message: String },
    /// Request rate-limited (HTTP 429).
    RateLimited { retry_after_secs: Option<u64> },
    /// Provider returned a server error (HTTP 5xx).
    ServerError { status: u16, message: String },
    /// Network or transport failure before a response was received.
    NetworkError {
        source: Box<dyn std::error::Error + Send + Sync>,
    },
    /// Provider returned a response that could not be parsed.
    InvalidResponse { reason: String },
}

impl fmt::Display for ProviderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unauthorized { message } => write!(f, "unauthorized: {message}"),
            Self::RateLimited {
                retry_after_secs: Some(s),
            } => {
                write!(f, "rate limited — retry after {s}s")
            }
            Self::RateLimited {
                retry_after_secs: None,
            } => write!(f, "rate limited"),
            Self::ServerError { status, message } => {
                write!(f, "provider server error {status}: {message}")
            }
            Self::NetworkError { source } => write!(f, "network error: {source}"),
            Self::InvalidResponse { reason } => write!(f, "invalid provider response: {reason}"),
        }
    }
}

impl std::error::Error for ProviderError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::NetworkError { source } => Some(source.as_ref()),
            _ => None,
        }
    }
}

// ── LLMProvider trait ─────────────────────────────────────────────────────────

/// Abstraction over any LLM backend (Anthropic, OpenAI, Ollama, etc.).
///
/// Implementations live in `roos-providers`. This trait is object-safe:
/// store behind `Arc<dyn LLMProvider>` to share across tasks.
#[async_trait]
pub trait LLMProvider: Send + Sync {
    /// Send `messages` to the model and return a completion.
    async fn complete(
        &self,
        messages: &[Message],
        config: &CompletionConfig,
    ) -> Result<CompletionResponse, ProviderError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_constructors() {
        assert_eq!(Message::user("hi").role, "user");
        assert_eq!(Message::assistant("ok").role, "assistant");
        assert_eq!(Message::system("be helpful").role, "system");
    }

    #[test]
    fn completion_config_new_defaults() {
        let cfg = CompletionConfig::new("claude-sonnet-4-6");
        assert_eq!(cfg.model, "claude-sonnet-4-6");
        assert!(cfg.max_tokens.is_none());
        assert!(cfg.temperature.is_none());
        assert!(cfg.tools.is_empty());
        assert!(cfg.system.is_none());
    }

    #[test]
    fn stop_reason_eq() {
        assert_eq!(StopReason::EndTurn, StopReason::EndTurn);
        assert_ne!(StopReason::ToolUse, StopReason::MaxTokens);
    }

    #[test]
    fn unauthorized_display() {
        let e = ProviderError::Unauthorized {
            message: "bad key".into(),
        };
        assert_eq!(e.to_string(), "unauthorized: bad key");
    }

    #[test]
    fn rate_limited_with_retry_display() {
        let e = ProviderError::RateLimited {
            retry_after_secs: Some(30),
        };
        assert_eq!(e.to_string(), "rate limited — retry after 30s");
    }

    #[test]
    fn rate_limited_no_retry_display() {
        let e = ProviderError::RateLimited {
            retry_after_secs: None,
        };
        assert_eq!(e.to_string(), "rate limited");
    }

    #[test]
    fn server_error_display() {
        let e = ProviderError::ServerError {
            status: 503,
            message: "overloaded".into(),
        };
        assert_eq!(e.to_string(), "provider server error 503: overloaded");
    }

    #[test]
    fn network_error_source_is_some() {
        let src: Box<dyn std::error::Error + Send + Sync> = "timeout".into();
        let e = ProviderError::NetworkError { source: src };
        assert!(std::error::Error::source(&e).is_some());
    }

    #[test]
    fn invalid_response_source_is_none() {
        let e = ProviderError::InvalidResponse {
            reason: "missing field".into(),
        };
        assert!(std::error::Error::source(&e).is_none());
    }

    // ── Object-safety ────────────────────────────────────────────────────────

    struct EchoProvider;

    #[async_trait]
    impl LLMProvider for EchoProvider {
        async fn complete(
            &self,
            messages: &[Message],
            config: &CompletionConfig,
        ) -> Result<CompletionResponse, ProviderError> {
            let last = messages
                .last()
                .map(|m| m.content.clone())
                .unwrap_or_default();
            Ok(CompletionResponse {
                content: Some(last),
                tool_calls: vec![],
                usage: TokenUsage::default(),
                model: config.model.clone(),
                stop_reason: StopReason::EndTurn,
            })
        }
    }

    #[tokio::test]
    async fn provider_is_object_safe() {
        let p: Box<dyn LLMProvider> = Box::new(EchoProvider);
        let msgs = vec![Message::user("hello")];
        let cfg = CompletionConfig::new("test-model");
        let resp = p.complete(&msgs, &cfg).await.unwrap();
        assert_eq!(resp.content.as_deref(), Some("hello"));
        assert_eq!(resp.stop_reason, StopReason::EndTurn);
    }
}
