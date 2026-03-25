use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use roos_core::provider::{
    CompletionConfig, CompletionResponse, LLMProvider, Message, ProviderError, StopReason,
    ToolCall, ToolSchema,
};
use roos_core::types::TokenUsage;

const API_URL: &str = "https://api.cohere.com/v2/chat";
const DEFAULT_MAX_TOKENS: u32 = 4096;

// ── Cohere v2 wire types ──────────────────────────────────────────────────────

#[derive(Serialize)]
struct ApiRequest<'a> {
    model: &'a str,
    messages: Vec<ApiMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tools: Vec<ApiTool>,
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
    function: ApiToolDef,
}

#[derive(Serialize)]
struct ApiToolDef {
    name: String,
    description: String,
    parameters: serde_json::Value,
}

#[derive(Deserialize)]
struct ApiResponse {
    message: ApiResponseMessage,
    finish_reason: String,
    usage: ApiUsage,
}

#[derive(Deserialize)]
struct ApiResponseMessage {
    #[serde(default)]
    content: Vec<ApiContentBlock>,
    #[serde(default)]
    tool_calls: Vec<ApiToolCall>,
}

#[derive(Deserialize)]
struct ApiContentBlock {
    text: String,
}

#[derive(Deserialize)]
struct ApiToolCall {
    id: String,
    function: ApiToolCallFn,
}

#[derive(Deserialize)]
struct ApiToolCallFn {
    name: String,
    arguments: serde_json::Value,
}

#[derive(Deserialize)]
struct ApiUsage {
    tokens: ApiTokens,
}

#[derive(Deserialize)]
struct ApiTokens {
    input_tokens: u32,
    output_tokens: u32,
}

#[derive(Deserialize)]
struct ApiError {
    message: String,
}

// ── CohereProvider ────────────────────────────────────────────────────────────

/// [`LLMProvider`] implementation for the Cohere v2 Chat API.
///
/// Construct with [`CohereProvider::new`], passing your API key.
/// The key is typically loaded via `RoosConfig` from `${COHERE_API_KEY}`.
pub struct CohereProvider {
    api_key: String,
    client: reqwest::Client,
}

impl CohereProvider {
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl LLMProvider for CohereProvider {
    async fn complete(
        &self,
        messages: &[Message],
        config: &CompletionConfig,
    ) -> Result<CompletionResponse, ProviderError> {
        let mut api_messages: Vec<ApiMessage> = Vec::new();
        if let Some(sys) = config.system.as_deref() {
            api_messages.push(ApiMessage {
                role: "system".into(),
                content: sys.to_owned(),
            });
        }
        for m in messages {
            api_messages.push(ApiMessage {
                role: m.role.clone(),
                content: m.content.clone(),
            });
        }

        let body = ApiRequest {
            model: &config.model,
            messages: api_messages,
            max_tokens: Some(config.max_tokens.unwrap_or(DEFAULT_MAX_TOKENS)),
            tools: config.tools.iter().map(tool_to_api).collect(),
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
            let msg = extract_err(resp).await;
            return Err(ProviderError::Unauthorized { message: msg });
        }
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return Err(ProviderError::RateLimited {
                retry_after_secs: None,
            });
        }
        if status.is_server_error() {
            let msg = extract_err(resp).await;
            return Err(ProviderError::ServerError {
                status: status.as_u16(),
                message: msg,
            });
        }
        if !status.is_success() {
            let msg = extract_err(resp).await;
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

fn tool_to_api(t: &ToolSchema) -> ApiTool {
    ApiTool {
        kind: "function",
        function: ApiToolDef {
            name: t.name.clone(),
            description: t.description.clone(),
            parameters: t.parameters.clone(),
        },
    }
}

fn map_response(api: ApiResponse) -> CompletionResponse {
    let content = api
        .message
        .content
        .into_iter()
        .map(|b| b.text)
        .collect::<Vec<_>>()
        .join("");
    let content = if content.is_empty() {
        None
    } else {
        Some(content)
    };

    let tool_calls = api
        .message
        .tool_calls
        .into_iter()
        .map(|tc| ToolCall {
            id: tc.id,
            name: tc.function.name,
            input: tc.function.arguments,
        })
        .collect();

    let stop_reason = match api.finish_reason.as_str() {
        "TOOL_CALL" => StopReason::ToolUse,
        "MAX_TOKENS" => StopReason::MaxTokens,
        _ => StopReason::EndTurn,
    };

    let total = api.usage.tokens.input_tokens + api.usage.tokens.output_tokens;
    let usage = TokenUsage {
        input: api.usage.tokens.input_tokens as usize,
        output: api.usage.tokens.output_tokens as usize,
        total: total as usize,
    };

    CompletionResponse {
        content,
        tool_calls,
        usage,
        model: String::new(),
        stop_reason,
    }
}

async fn extract_err(resp: reqwest::Response) -> String {
    resp.json::<ApiError>()
        .await
        .map(|e| e.message)
        .unwrap_or_else(|_| "unknown error".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_new_stores_key() {
        let p = CohereProvider::new("co-test");
        assert_eq!(p.api_key, "co-test");
    }

    #[test]
    fn map_text_response() {
        let api = ApiResponse {
            message: ApiResponseMessage {
                content: vec![ApiContentBlock {
                    text: "hello".into(),
                }],
                tool_calls: vec![],
            },
            finish_reason: "COMPLETE".into(),
            usage: ApiUsage {
                tokens: ApiTokens {
                    input_tokens: 10,
                    output_tokens: 5,
                },
            },
        };
        let resp = map_response(api);
        assert_eq!(resp.content.as_deref(), Some("hello"));
        assert_eq!(resp.stop_reason, StopReason::EndTurn);
        assert_eq!(resp.usage.total, 15);
    }

    #[test]
    fn stop_reason_tool_call() {
        let api = ApiResponse {
            message: ApiResponseMessage {
                content: vec![],
                tool_calls: vec![],
            },
            finish_reason: "TOOL_CALL".into(),
            usage: ApiUsage {
                tokens: ApiTokens {
                    input_tokens: 0,
                    output_tokens: 0,
                },
            },
        };
        assert_eq!(map_response(api).stop_reason, StopReason::ToolUse);
    }
}
