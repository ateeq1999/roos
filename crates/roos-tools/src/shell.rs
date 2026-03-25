use async_trait::async_trait;
use schemars::{schema_for, JsonSchema};
use serde::Deserialize;

use roos_core::tool::{Tool, ToolError};

// ── ExecuteShellTool ──────────────────────────────────────────────────────────

#[derive(Deserialize, JsonSchema)]
struct ExecuteShellInput {
    /// The shell command to run.
    command: String,
}

/// Runs a shell command and returns its combined stdout + stderr output.
///
/// Commands are checked against an allowlist before execution.  If the
/// allowlist is empty every command is permitted (development mode).  In
/// production, populate the allowlist with the exact command prefixes that
/// are safe to execute.
pub struct ExecuteShellTool {
    /// Permitted command prefixes.  An empty list means *all* commands are
    /// allowed (useful for development / testing).
    allowlist: Vec<String>,
}

impl ExecuteShellTool {
    /// Create a tool with no allowlist restrictions (all commands allowed).
    pub fn new() -> Self {
        Self { allowlist: vec![] }
    }

    /// Create a tool that only permits commands whose first token matches one
    /// of the given prefixes.
    ///
    /// ```
    /// # use roos_tools::ExecuteShellTool;
    /// let tool = ExecuteShellTool::with_allowlist(vec!["echo".into(), "ls".into()]);
    /// ```
    pub fn with_allowlist(allowlist: Vec<String>) -> Self {
        Self { allowlist }
    }

    fn is_allowed(&self, command: &str) -> bool {
        if self.allowlist.is_empty() {
            return true;
        }
        let first_token = command.split_whitespace().next().unwrap_or("");
        self.allowlist.iter().any(|a| a == first_token)
    }
}

impl Default for ExecuteShellTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for ExecuteShellTool {
    fn name(&self) -> &str {
        "execute_shell"
    }

    fn description(&self) -> &str {
        "Execute a shell command and return its combined stdout and stderr output."
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::to_value(schema_for!(ExecuteShellInput)).unwrap_or_default()
    }

    async fn execute(&self, input: serde_json::Value) -> Result<String, ToolError> {
        let inp: ExecuteShellInput =
            serde_json::from_value(input).map_err(|e| ToolError::InvalidInput {
                tool: self.name().to_owned(),
                reason: e.to_string(),
            })?;

        if !self.is_allowed(&inp.command) {
            return Err(ToolError::NotAllowed {
                tool: self.name().to_owned(),
            });
        }

        let output = if cfg!(target_os = "windows") {
            std::process::Command::new("cmd")
                .args(["/C", &inp.command])
                .output()
        } else {
            std::process::Command::new("sh")
                .args(["-c", &inp.command])
                .output()
        }
        .map_err(|e| ToolError::ExecutionFailed {
            tool: self.name().to_owned(),
            source: e.into(),
        })?;

        let mut result = String::new();
        if !output.stdout.is_empty() {
            result.push_str(&String::from_utf8_lossy(&output.stdout));
        }
        if !output.stderr.is_empty() {
            result.push_str(&String::from_utf8_lossy(&output.stderr));
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn allowlist_empty_permits_everything() {
        let tool = ExecuteShellTool::new();
        assert!(tool.is_allowed("echo hello"));
        assert!(tool.is_allowed("ls -la"));
        assert!(tool.is_allowed("rm -rf /"));
    }

    #[test]
    fn allowlist_blocks_unlisted_commands() {
        let tool = ExecuteShellTool::with_allowlist(vec!["echo".into()]);
        assert!(tool.is_allowed("echo hello"));
        assert!(!tool.is_allowed("rm file"));
        assert!(!tool.is_allowed("ls -la"));
    }

    #[test]
    fn tool_name_and_schema() {
        let tool = ExecuteShellTool::new();
        assert_eq!(tool.name(), "execute_shell");
        let schema = tool.schema();
        assert!(schema.get("properties").is_some());
    }

    #[tokio::test]
    async fn blocked_command_returns_not_allowed() {
        let tool = ExecuteShellTool::with_allowlist(vec!["echo".into()]);
        let result = tool.execute(json!({"command": "ls"})).await;
        assert!(matches!(result, Err(ToolError::NotAllowed { .. })));
    }

    #[tokio::test]
    async fn invalid_input_returns_error() {
        let result = ExecuteShellTool::new()
            .execute(json!({"wrong": "key"}))
            .await;
        assert!(matches!(result, Err(ToolError::InvalidInput { .. })));
    }

    #[tokio::test]
    async fn echo_command_runs() {
        let tool = ExecuteShellTool::new();
        let result = tool
            .execute(json!({"command": "echo hello"}))
            .await
            .unwrap();
        assert!(result.trim().contains("hello"));
    }
}
