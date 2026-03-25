use std::fmt;

use async_trait::async_trait;

/// Errors that a [`Tool`] implementation can return from [`Tool::execute`].
#[derive(Debug)]
pub enum ToolError {
    /// The JSON input supplied by the LLM failed validation or parsing.
    InvalidInput { tool: String, reason: String },
    /// The tool ran but encountered a runtime failure.
    ExecutionFailed {
        tool: String,
        source: Box<dyn std::error::Error + Send + Sync>,
    },
    /// The tool is blocked by the current security policy.
    NotAllowed { tool: String },
    /// The tool execution exceeded its time budget.
    Timeout { tool: String, elapsed_ms: u64 },
}

impl fmt::Display for ToolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput { tool, reason } => {
                write!(f, "tool '{tool}': invalid input — {reason}")
            }
            Self::ExecutionFailed { tool, source } => {
                write!(f, "tool '{tool}' execution failed: {source}")
            }
            Self::NotAllowed { tool } => {
                write!(f, "tool '{tool}' is not allowed by the security policy")
            }
            Self::Timeout { tool, elapsed_ms } => {
                write!(f, "tool '{tool}' timed out after {elapsed_ms}ms")
            }
        }
    }
}

impl std::error::Error for ToolError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ExecutionFailed { source, .. } => Some(source.as_ref()),
            _ => None,
        }
    }
}

/// A callable capability that an agent can invoke during its reasoning loop.
///
/// Implement this trait to expose any action — filesystem access, HTTP calls,
/// database queries, or custom integrations — to ROOS agents.
///
/// All implementations must be `Send + Sync` so they can be held behind
/// `Arc<dyn Tool>` and shared across async tasks.
#[async_trait]
pub trait Tool: Send + Sync {
    /// Unique snake_case identifier (e.g. `"read_file"`).
    /// Must be stable across restarts — it appears in audit logs.
    fn name(&self) -> &str;

    /// Plain-English description shown to the LLM in the system prompt.
    fn description(&self) -> &str;

    /// JSON Schema (draft-07) describing the tool's expected input object.
    ///
    /// The orchestrator validates LLM-generated inputs against this schema
    /// before calling [`execute`](Tool::execute) (ROOS-TOOL-004).
    fn schema(&self) -> serde_json::Value;

    /// Execute the tool with the given JSON input.
    ///
    /// The input has already been validated against [`schema`](Tool::schema)
    /// by the time this method is called.
    async fn execute(&self, input: serde_json::Value) -> Result<String, ToolError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── ToolError display ────────────────────────────────────────────────────

    #[test]
    fn invalid_input_display() {
        let e = ToolError::InvalidInput {
            tool: "read_file".into(),
            reason: "missing 'path' field".into(),
        };
        assert_eq!(
            e.to_string(),
            "tool 'read_file': invalid input — missing 'path' field"
        );
    }

    #[test]
    fn execution_failed_display() {
        let src: Box<dyn std::error::Error + Send + Sync> = "disk full".into();
        let e = ToolError::ExecutionFailed {
            tool: "write_file".into(),
            source: src,
        };
        assert_eq!(
            e.to_string(),
            "tool 'write_file' execution failed: disk full"
        );
    }

    #[test]
    fn not_allowed_display() {
        let e = ToolError::NotAllowed {
            tool: "execute_shell".into(),
        };
        assert_eq!(
            e.to_string(),
            "tool 'execute_shell' is not allowed by the security policy"
        );
    }

    #[test]
    fn timeout_display() {
        let e = ToolError::Timeout {
            tool: "http_get".into(),
            elapsed_ms: 5000,
        };
        assert_eq!(e.to_string(), "tool 'http_get' timed out after 5000ms");
    }

    #[test]
    fn execution_failed_source_is_some() {
        let src: Box<dyn std::error::Error + Send + Sync> = "boom".into();
        let e = ToolError::ExecutionFailed {
            tool: "shell".into(),
            source: src,
        };
        assert!(std::error::Error::source(&e).is_some());
    }

    #[test]
    fn other_variants_source_is_none() {
        let e = ToolError::NotAllowed {
            tool: "shell".into(),
        };
        assert!(std::error::Error::source(&e).is_none());
    }

    // ── Minimal Tool impl (compile-time object-safety check) ─────────────────

    struct Echo;

    #[async_trait]
    impl Tool for Echo {
        fn name(&self) -> &str {
            "echo"
        }
        fn description(&self) -> &str {
            "Returns the input text unchanged."
        }
        fn schema(&self) -> serde_json::Value {
            serde_json::json!({
                "type": "object",
                "properties": { "text": { "type": "string" } },
                "required": ["text"]
            })
        }
        async fn execute(&self, input: serde_json::Value) -> Result<String, ToolError> {
            let text = input["text"]
                .as_str()
                .ok_or_else(|| ToolError::InvalidInput {
                    tool: self.name().into(),
                    reason: "missing 'text' field".into(),
                })?;
            Ok(text.to_owned())
        }
    }

    #[tokio::test]
    async fn echo_tool_returns_input() {
        let tool = Echo;
        let result = tool
            .execute(serde_json::json!({ "text": "hello" }))
            .await
            .unwrap();
        assert_eq!(result, "hello");
    }

    #[tokio::test]
    async fn echo_tool_missing_field_returns_err() {
        let tool = Echo;
        let err = tool.execute(serde_json::json!({})).await.unwrap_err();
        assert!(matches!(err, ToolError::InvalidInput { .. }));
    }

    #[tokio::test]
    async fn tool_is_object_safe() {
        // Ensures Box<dyn Tool> compiles.
        let tool: Box<dyn Tool> = Box::new(Echo);
        let result = tool
            .execute(serde_json::json!({ "text": "world" }))
            .await
            .unwrap();
        assert_eq!(result, "world");
    }
}
