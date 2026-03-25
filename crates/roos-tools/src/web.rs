use async_trait::async_trait;
use schemars::{schema_for, JsonSchema};
use serde::Deserialize;

use roos_core::tool::{Tool, ToolError};

// ── SearchWebTool ─────────────────────────────────────────────────────────────

#[derive(Deserialize, JsonSchema)]
struct SearchWebInput {
    /// Search query string.
    query: String,
}

/// Searches the web using the DuckDuckGo Instant Answer API and returns a
/// brief summary.  No API key is required.
///
/// For richer results, swap the implementation for a commercial search API
/// (e.g. SerpAPI, Brave Search) by replacing the `endpoint` URL.
pub struct SearchWebTool {
    client: reqwest::Client,
}

impl SearchWebTool {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }
}

impl Default for SearchWebTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for SearchWebTool {
    fn name(&self) -> &str {
        "search_web"
    }

    fn description(&self) -> &str {
        "Search the web for a query and return a brief summary of the top result."
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::to_value(schema_for!(SearchWebInput)).unwrap_or_default()
    }

    async fn execute(&self, input: serde_json::Value) -> Result<String, ToolError> {
        let inp: SearchWebInput =
            serde_json::from_value(input).map_err(|e| ToolError::InvalidInput {
                tool: self.name().to_owned(),
                reason: e.to_string(),
            })?;

        let resp: serde_json::Value = self
            .client
            .get("https://api.duckduckgo.com/")
            .query(&[
                ("q", &inp.query),
                ("format", &"json".to_owned()),
                ("no_html", &"1".to_owned()),
            ])
            .send()
            .await
            .map_err(|e| ToolError::ExecutionFailed {
                tool: self.name().to_owned(),
                source: e.into(),
            })?
            .json()
            .await
            .map_err(|e| ToolError::ExecutionFailed {
                tool: self.name().to_owned(),
                source: e.into(),
            })?;

        let summary = resp
            .get("AbstractText")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .or_else(|| {
                resp.get("RelatedTopics")
                    .and_then(|t| t.as_array())
                    .and_then(|arr| arr.first())
                    .and_then(|item| item.get("Text"))
                    .and_then(|v| v.as_str())
            })
            .unwrap_or("No results found.");

        Ok(summary.to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn tool_name_and_schema() {
        let tool = SearchWebTool::new();
        assert_eq!(tool.name(), "search_web");
        let schema = tool.schema();
        assert!(schema.get("properties").is_some());
    }

    #[tokio::test]
    async fn invalid_input_returns_error() {
        let result = SearchWebTool::new().execute(json!({"wrong": "key"})).await;
        assert!(matches!(result, Err(ToolError::InvalidInput { .. })));
    }
}
