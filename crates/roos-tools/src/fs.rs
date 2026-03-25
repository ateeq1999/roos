use async_trait::async_trait;
use schemars::{schema_for, JsonSchema};
use serde::Deserialize;

use roos_core::tool::{Tool, ToolError};

// ── ReadFileTool ──────────────────────────────────────────────────────────────

#[derive(Deserialize, JsonSchema)]
struct ReadFileInput {
    /// Absolute or relative path of the file to read.
    path: String,
}

/// Reads the full text content of a file and returns it as a string.
pub struct ReadFileTool;

#[async_trait]
impl Tool for ReadFileTool {
    fn name(&self) -> &str {
        "read_file"
    }

    fn description(&self) -> &str {
        "Read the entire text content of a file at the given path."
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::to_value(schema_for!(ReadFileInput)).unwrap_or_default()
    }

    async fn execute(&self, input: serde_json::Value) -> Result<String, ToolError> {
        let inp: ReadFileInput =
            serde_json::from_value(input).map_err(|e| ToolError::InvalidInput {
                tool: self.name().to_owned(),
                reason: e.to_string(),
            })?;
        std::fs::read_to_string(&inp.path).map_err(|e| ToolError::ExecutionFailed {
            tool: self.name().to_owned(),
            source: e.into(),
        })
    }
}

// ── WriteFileTool ─────────────────────────────────────────────────────────────

#[derive(Deserialize, JsonSchema)]
struct WriteFileInput {
    /// Path of the file to write (created or overwritten).
    path: String,
    /// Text content to write.
    content: String,
}

/// Writes text content to a file, creating it if necessary.
pub struct WriteFileTool;

#[async_trait]
impl Tool for WriteFileTool {
    fn name(&self) -> &str {
        "write_file"
    }

    fn description(&self) -> &str {
        "Write text content to a file, creating or overwriting it."
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::to_value(schema_for!(WriteFileInput)).unwrap_or_default()
    }

    async fn execute(&self, input: serde_json::Value) -> Result<String, ToolError> {
        let inp: WriteFileInput =
            serde_json::from_value(input).map_err(|e| ToolError::InvalidInput {
                tool: self.name().to_owned(),
                reason: e.to_string(),
            })?;
        std::fs::write(&inp.path, &inp.content).map_err(|e| ToolError::ExecutionFailed {
            tool: self.name().to_owned(),
            source: e.into(),
        })?;
        Ok(format!("Wrote {} bytes to {}", inp.content.len(), inp.path))
    }
}

// ── ListDirectoryTool ─────────────────────────────────────────────────────────

#[derive(Deserialize, JsonSchema)]
struct ListDirectoryInput {
    /// Path of the directory to list.
    path: String,
}

/// Lists the entries in a directory, one per line.
pub struct ListDirectoryTool;

#[async_trait]
impl Tool for ListDirectoryTool {
    fn name(&self) -> &str {
        "list_directory"
    }

    fn description(&self) -> &str {
        "List the files and directories at the given path, one entry per line."
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::to_value(schema_for!(ListDirectoryInput)).unwrap_or_default()
    }

    async fn execute(&self, input: serde_json::Value) -> Result<String, ToolError> {
        let inp: ListDirectoryInput =
            serde_json::from_value(input).map_err(|e| ToolError::InvalidInput {
                tool: self.name().to_owned(),
                reason: e.to_string(),
            })?;
        let entries = std::fs::read_dir(&inp.path).map_err(|e| ToolError::ExecutionFailed {
            tool: self.name().to_owned(),
            source: e.into(),
        })?;
        let mut names: Vec<String> = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|e| ToolError::ExecutionFailed {
                tool: self.name().to_owned(),
                source: e.into(),
            })?;
            names.push(entry.file_name().to_string_lossy().into_owned());
        }
        names.sort();
        Ok(names.join("\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::TempDir;

    #[tokio::test]
    async fn read_file_returns_content() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("hello.txt");
        std::fs::write(&path, "hello world").unwrap();

        let result = ReadFileTool
            .execute(json!({"path": path.to_str().unwrap()}))
            .await
            .unwrap();
        assert_eq!(result, "hello world");
    }

    #[tokio::test]
    async fn read_file_missing_path_returns_error() {
        let result = ReadFileTool
            .execute(json!({"path": "/nonexistent/file.txt"}))
            .await;
        assert!(matches!(result, Err(ToolError::ExecutionFailed { .. })));
    }

    #[tokio::test]
    async fn write_file_creates_file() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("out.txt");

        WriteFileTool
            .execute(json!({"path": path.to_str().unwrap(), "content": "test"}))
            .await
            .unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "test");
    }

    #[tokio::test]
    async fn list_directory_returns_sorted_entries() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("b.txt"), "").unwrap();
        std::fs::write(tmp.path().join("a.txt"), "").unwrap();

        let result = ListDirectoryTool
            .execute(json!({"path": tmp.path().to_str().unwrap()}))
            .await
            .unwrap();
        let lines: Vec<&str> = result.lines().collect();
        assert_eq!(lines, ["a.txt", "b.txt"]);
    }

    #[tokio::test]
    async fn invalid_input_returns_invalid_input_error() {
        let result = ReadFileTool.execute(json!({"wrong": "key"})).await;
        assert!(matches!(result, Err(ToolError::InvalidInput { .. })));
    }

    #[test]
    fn tool_names_and_schemas() {
        assert_eq!(ReadFileTool.name(), "read_file");
        assert_eq!(WriteFileTool.name(), "write_file");
        assert_eq!(ListDirectoryTool.name(), "list_directory");

        let schema = ReadFileTool.schema();
        assert!(schema.get("properties").is_some());
    }
}
