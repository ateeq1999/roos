use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use roos_core::provider::{
    CompletionConfig, CompletionResponse, LLMProvider, Message, ProviderError, StopReason,
    ToolCall, ToolSchema,
};
use roos_core::types::TokenUsage;

const API_URL: &str = "https://api.anthropic.com/v1/messages";
const API_VERSION: &str = "2023-06-01";
const DEFAULT_MAX_TOKENS: u32 = 4096;

// ── Anthropic wire types ──────────────────────────────────────────────────────

#[derive(Serialize)]
struct ApiRequest<'a> {
    model: &'a str,
    max_tokens: u32,
    messages: Vec<ApiMessage>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tools: Vec<ApiTool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<&'a str>,
}

#[derive(Serialize)]
struct ApiMessage {
    role: String,
    content: String,
}

#[derive(Serialize)]
struct ApiTool {
    name: String,
    description: String,
    input_schema: serde_json::Value,
}

#[derive(Deserialize)]
struct ApiResponse {
    content: Vec<ApiContent>,
    model: String,
    stop_reason: String,
    usage: ApiUsage,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ApiContent {
    Text {
        text: String,
    },
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
}

#[derive(Deserialize)]
struct ApiUsage {
    input_tokens: u32,
    output_tokens: u32,
}

#[derive(Deserialize)]
struct ApiError {
    error: ApiErrorDetail,
}

#[derive(Deserialize)]
struct ApiErrorDetail {
    message: String,
}

// ── AnthropicProvider ─────────────────────────────────────────────────────────

/// [`LLMProvider`] implementation for the Anthropic Messages API.
///
/// Construct with [`AnthropicProvider::new`], passing your API key.
/// The key is typically loaded via `RoosConfig` from `${ANTHROPIC_API_KEY}`.
pub struct AnthropicProvider {
    api_key: String,
    client: reqwest::Client,
}

impl AnthropicProvider {
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl LLMProvider for AnthropicProvider {
    async fn complete(
        &self,
        messages: &[Message],
        config: &CompletionConfig,
    ) -> Result<CompletionResponse, ProviderError> {
        let body = ApiRequest {
            model: &config.model,
            max_tokens: config.max_tokens.unwrap_or(DEFAULT_MAX_TOKENS),
            messages: messages
                .iter()
                .map(|m| ApiMessage {
                    role: m.role.clone(),
                    content: m.content.clone(),
                })
                .collect(),
            tools: config.tools.iter().map(tool_schema_to_api).collect(),
            system: config.system.as_deref(),
        };

        let resp = self
            .client
            .post(API_URL)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", API_VERSION)
            .json(&body)
            .send()
            .await
            .map_err(|e| ProviderError::NetworkError { source: e.into() })?;

        let status = resp.status();

        if status == reqwest::StatusCode::UNAUTHORIZED {
            let msg = extract_error(resp).await;
            return Err(ProviderError::Unauthorized { message: msg });
        }
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return Err(ProviderError::RateLimited {
                retry_after_secs: None,
            });
        }
        if status.is_server_error() {
            let msg = extract_error(resp).await;
            return Err(ProviderError::ServerError {
                status: status.as_u16(),
                message: msg,
            });
        }
        if !status.is_success() {
            let msg = extract_error(resp).await;
            return Err(ProviderError::InvalidResponse { reason: msg });
        }

        let api: ApiResponse = resp
            .json()
            .await
            .map_err(|e| ProviderError::InvalidResponse {
                reason: e.to_string(),
            })?;

        Ok(map_response(api))
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn tool_schema_to_api(t: &ToolSchema) -> ApiTool {
    ApiTool {
        name: t.name.clone(),
        description: t.description.clone(),
        input_schema: t.parameters.clone(),
    }
}

fn map_response(api: ApiResponse) -> CompletionResponse {
    let mut text_parts: Vec<String> = Vec::new();
    let mut tool_calls: Vec<ToolCall> = Vec::new();

    for block in api.content {
        match block {
            ApiContent::Text { text } => text_parts.push(text),
            ApiContent::ToolUse { id, name, input } => {
                tool_calls.push(ToolCall { id, name, input });
            }
        }
    }

    let content = if text_parts.is_empty() {
        None
    } else {
        Some(text_parts.join(""))
    };

    let stop_reason = match api.stop_reason.as_str() {
        "tool_use" => StopReason::ToolUse,
        "max_tokens" => StopReason::MaxTokens,
        "stop_sequence" => StopReason::StopSequence,
        _ => StopReason::EndTurn,
    };

    let total = api.usage.input_tokens + api.usage.output_tokens;
    let usage = TokenUsage {
        input: api.usage.input_tokens as usize,
        output: api.usage.output_tokens as usize,
        total: total as usize,
    };

    CompletionResponse {
        content,
        tool_calls,
        usage,
        model: api.model,
        stop_reason,
    }
}

async fn extract_error(resp: reqwest::Response) -> String {
    resp.json::<ApiError>()
        .await
        .map(|e| e.error.message)
        .unwrap_or_else(|_| "unknown error".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_text_response() {
        let api = ApiResponse {
            content: vec![ApiContent::Text {
                text: "hello".into(),
            }],
            model: "claude-sonnet-4-6".into(),
            stop_reason: "end_turn".into(),
            usage: ApiUsage {
                input_tokens: 10,
                output_tokens: 5,
            },
        };
        let resp = map_response(api);
        assert_eq!(resp.content.as_deref(), Some("hello"));
        assert_eq!(resp.stop_reason, StopReason::EndTurn);
        assert_eq!(resp.usage.total, 15);
        assert!(resp.tool_calls.is_empty());
    }

    #[test]
    fn map_tool_use_response() {
        let api = ApiResponse {
            content: vec![ApiContent::ToolUse {
                id: "call_1".into(),
                name: "read_file".into(),
                input: serde_json::json!({ "path": "/tmp/x" }),
            }],
            model: "claude-sonnet-4-6".into(),
            stop_reason: "tool_use".into(),
            usage: ApiUsage {
                input_tokens: 20,
                output_tokens: 8,
            },
        };
        let resp = map_response(api);
        assert!(resp.content.is_none());
        assert_eq!(resp.stop_reason, StopReason::ToolUse);
        assert_eq!(resp.tool_calls.len(), 1);
        assert_eq!(resp.tool_calls[0].name, "read_file");
    }

    #[test]
    fn stop_reason_mapping() {
        let make = |reason: &str| ApiResponse {
            content: vec![],
            model: "m".into(),
            stop_reason: reason.into(),
            usage: ApiUsage {
                input_tokens: 0,
                output_tokens: 0,
            },
        };
        assert_eq!(
            map_response(make("tool_use")).stop_reason,
            StopReason::ToolUse
        );
        assert_eq!(
            map_response(make("max_tokens")).stop_reason,
            StopReason::MaxTokens
        );
        assert_eq!(
            map_response(make("stop_sequence")).stop_reason,
            StopReason::StopSequence
        );
        assert_eq!(
            map_response(make("end_turn")).stop_reason,
            StopReason::EndTurn
        );
    }

    #[test]
    fn provider_new_stores_key() {
        let p = AnthropicProvider::new("sk-test");
        assert_eq!(p.api_key, "sk-test");
    }
}
