use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use roos_core::provider::{
    CompletionConfig, CompletionResponse, LLMProvider, Message, ProviderError, StopReason,
    ToolCall, ToolSchema,
};
use roos_core::types::TokenUsage;

const API_URL: &str = "https://api.openai.com/v1/chat/completions";
const DEFAULT_MAX_TOKENS: u32 = 4096;

// ── OpenAI wire types ─────────────────────────────────────────────────────────

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
    #[serde(rename = "type")]
    kind: &'static str,
    function: ApiFunction,
}

#[derive(Serialize)]
struct ApiFunction {
    name: String,
    description: String,
    parameters: serde_json::Value,
}

#[derive(Deserialize)]
struct ApiResponse {
    choices: Vec<ApiChoice>,
    model: String,
    usage: ApiUsage,
}

#[derive(Deserialize)]
struct ApiChoice {
    message: ApiResponseMessage,
    finish_reason: String,
}

#[derive(Deserialize)]
struct ApiResponseMessage {
    content: Option<String>,
    #[serde(default)]
    tool_calls: Vec<ApiToolCall>,
}

#[derive(Deserialize)]
struct ApiToolCall {
    id: String,
    function: ApiToolCallFunction,
}

#[derive(Deserialize)]
struct ApiToolCallFunction {
    name: String,
    arguments: String,
}

#[derive(Deserialize)]
struct ApiUsage {
    prompt_tokens: u32,
    completion_tokens: u32,
}

#[derive(Deserialize)]
struct ApiError {
    error: ApiErrorDetail,
}

#[derive(Deserialize)]
struct ApiErrorDetail {
    message: String,
}

// ── OpenAIProvider ────────────────────────────────────────────────────────────

/// [`LLMProvider`] implementation for the OpenAI Chat Completions API.
///
/// Construct with [`OpenAIProvider::new`], passing your API key.
/// The key is typically loaded via `RoosConfig` from `${OPENAI_API_KEY}`.
pub struct OpenAIProvider {
    api_key: String,
    client: reqwest::Client,
}

impl OpenAIProvider {
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl LLMProvider for OpenAIProvider {
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
            .bearer_auth(&self.api_key)
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
        kind: "function",
        function: ApiFunction {
            name: t.name.clone(),
            description: t.description.clone(),
            parameters: t.parameters.clone(),
        },
    }
}

fn map_response(api: ApiResponse) -> CompletionResponse {
    let choice = match api.choices.into_iter().next() {
        Some(c) => c,
        None => {
            return CompletionResponse {
                content: None,
                tool_calls: vec![],
                usage: TokenUsage {
                    input: 0,
                    output: 0,
                    total: 0,
                },
                model: api.model,
                stop_reason: StopReason::EndTurn,
            }
        }
    };

    let stop_reason = match choice.finish_reason.as_str() {
        "tool_calls" => StopReason::ToolUse,
        "length" => StopReason::MaxTokens,
        "content_filter" => StopReason::StopSequence,
        _ => StopReason::EndTurn,
    };

    let tool_calls = choice
        .message
        .tool_calls
        .into_iter()
        .map(|tc| {
            let input =
                serde_json::from_str(&tc.function.arguments).unwrap_or(serde_json::Value::Null);
            ToolCall {
                id: tc.id,
                name: tc.function.name,
                input,
            }
        })
        .collect();

    let total = api.usage.prompt_tokens + api.usage.completion_tokens;
    let usage = TokenUsage {
        input: api.usage.prompt_tokens as usize,
        output: api.usage.completion_tokens as usize,
        total: total as usize,
    };

    CompletionResponse {
        content: choice.message.content,
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

    fn make_response(
        finish_reason: &str,
        content: Option<&str>,
        tool_calls: Vec<ApiToolCall>,
    ) -> ApiResponse {
        ApiResponse {
            choices: vec![ApiChoice {
                message: ApiResponseMessage {
                    content: content.map(|s| s.to_owned()),
                    tool_calls,
                },
                finish_reason: finish_reason.to_owned(),
            }],
            model: "gpt-4o".to_owned(),
            usage: ApiUsage {
                prompt_tokens: 10,
                completion_tokens: 5,
            },
        }
    }

    #[test]
    fn map_text_response() {
        let api = make_response("stop", Some("hello"), vec![]);
        let resp = map_response(api);
        assert_eq!(resp.content.as_deref(), Some("hello"));
        assert_eq!(resp.stop_reason, StopReason::EndTurn);
        assert_eq!(resp.usage.total, 15);
        assert!(resp.tool_calls.is_empty());
    }

    #[test]
    fn map_tool_use_response() {
        let tc = ApiToolCall {
            id: "call_1".into(),
            function: ApiToolCallFunction {
                name: "read_file".into(),
                arguments: r#"{"path":"/tmp/x"}"#.into(),
            },
        };
        let api = make_response("tool_calls", None, vec![tc]);
        let resp = map_response(api);
        assert!(resp.content.is_none());
        assert_eq!(resp.stop_reason, StopReason::ToolUse);
        assert_eq!(resp.tool_calls.len(), 1);
        assert_eq!(resp.tool_calls[0].name, "read_file");
    }

    #[test]
    fn stop_reason_mapping() {
        let make = |reason: &str| make_response(reason, None, vec![]);
        assert_eq!(
            map_response(make("tool_calls")).stop_reason,
            StopReason::ToolUse
        );
        assert_eq!(
            map_response(make("length")).stop_reason,
            StopReason::MaxTokens
        );
        assert_eq!(
            map_response(make("content_filter")).stop_reason,
            StopReason::StopSequence
        );
        assert_eq!(map_response(make("stop")).stop_reason, StopReason::EndTurn);
    }

    #[test]
    fn empty_choices_returns_end_turn() {
        let api = ApiResponse {
            choices: vec![],
            model: "gpt-4o".into(),
            usage: ApiUsage {
                prompt_tokens: 0,
                completion_tokens: 0,
            },
        };
        let resp = map_response(api);
        assert!(resp.content.is_none());
        assert_eq!(resp.stop_reason, StopReason::EndTurn);
    }

    #[test]
    fn provider_new_stores_key() {
        let p = OpenAIProvider::new("sk-test");
        assert_eq!(p.api_key, "sk-test");
    }
}
