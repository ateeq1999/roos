use async_trait::async_trait;
use schemars::{schema_for, JsonSchema};
use serde::Deserialize;

use roos_core::tool::{Tool, ToolError};

// ── HttpGetTool ───────────────────────────────────────────────────────────────

#[derive(Deserialize, JsonSchema)]
struct HttpGetInput {
    /// URL to fetch.
    url: String,
}

/// Performs an HTTP GET request and returns the response body as text.
pub struct HttpGetTool {
    client: reqwest::Client,
}

impl HttpGetTool {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }
}

impl Default for HttpGetTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for HttpGetTool {
    fn name(&self) -> &str {
        "http_get"
    }

    fn description(&self) -> &str {
        "Perform an HTTP GET request and return the response body as text."
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::to_value(schema_for!(HttpGetInput)).unwrap_or_default()
    }

    async fn execute(&self, input: serde_json::Value) -> Result<String, ToolError> {
        let inp: HttpGetInput =
            serde_json::from_value(input).map_err(|e| ToolError::InvalidInput {
                tool: self.name().to_owned(),
                reason: e.to_string(),
            })?;
        self.client
            .get(&inp.url)
            .send()
            .await
            .map_err(|e| ToolError::ExecutionFailed {
                tool: self.name().to_owned(),
                source: e.into(),
            })?
            .text()
            .await
            .map_err(|e| ToolError::ExecutionFailed {
                tool: self.name().to_owned(),
                source: e.into(),
            })
    }
}

// ── HttpPostTool ──────────────────────────────────────────────────────────────

#[derive(Deserialize, JsonSchema)]
struct HttpPostInput {
    /// URL to POST to.
    url: String,
    /// Request body (sent as plain text with Content-Type: text/plain).
    body: String,
}

/// Performs an HTTP POST request with a text body and returns the response body.
pub struct HttpPostTool {
    client: reqwest::Client,
}

impl HttpPostTool {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }
}

impl Default for HttpPostTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for HttpPostTool {
    fn name(&self) -> &str {
        "http_post"
    }

    fn description(&self) -> &str {
        "Perform an HTTP POST request with a text body and return the response body."
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::to_value(schema_for!(HttpPostInput)).unwrap_or_default()
    }

    async fn execute(&self, input: serde_json::Value) -> Result<String, ToolError> {
        let inp: HttpPostInput =
            serde_json::from_value(input).map_err(|e| ToolError::InvalidInput {
                tool: self.name().to_owned(),
                reason: e.to_string(),
            })?;
        self.client
            .post(&inp.url)
            .body(inp.body)
            .send()
            .await
            .map_err(|e| ToolError::ExecutionFailed {
                tool: self.name().to_owned(),
                source: e.into(),
            })?
            .text()
            .await
            .map_err(|e| ToolError::ExecutionFailed {
                tool: self.name().to_owned(),
                source: e.into(),
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn tool_names_and_schemas() {
        assert_eq!(HttpGetTool::new().name(), "http_get");
        assert_eq!(HttpPostTool::new().name(), "http_post");

        let schema = HttpGetTool::new().schema();
        assert!(schema.get("properties").is_some());
    }

    #[tokio::test]
    async fn invalid_input_get() {
        let result = HttpGetTool::new().execute(json!({"wrong": "key"})).await;
        assert!(matches!(result, Err(ToolError::InvalidInput { .. })));
    }

    #[tokio::test]
    async fn invalid_input_post() {
        let result = HttpPostTool::new().execute(json!({"wrong": "key"})).await;
        assert!(matches!(result, Err(ToolError::InvalidInput { .. })));
    }

    #[tokio::test]
    async fn bad_url_returns_execution_failed() {
        let result = HttpGetTool::new()
            .execute(json!({"url": "http://127.0.0.1:1"}))
            .await;
        assert!(matches!(result, Err(ToolError::ExecutionFailed { .. })));
    }
}
