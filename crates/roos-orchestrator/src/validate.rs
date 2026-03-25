use std::fmt;

use jsonschema::JSONSchema;
use serde_json::Value;

/// Error returned when LLM-generated tool input fails JSON Schema validation.
#[derive(Debug)]
pub struct ValidationError {
    /// Name of the tool whose schema was violated.
    pub tool: String,
    /// Human-readable description of each violation.
    pub violations: Vec<String>,
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "tool '{}' input validation failed: {}",
            self.tool,
            self.violations.join("; ")
        )
    }
}

impl std::error::Error for ValidationError {}

/// Validate `input` against `schema` (JSON Schema draft-07) for `tool_name`.
///
/// Returns `Ok(())` if valid, or a [`ValidationError`] listing every
/// violation. Called by the orchestrator before [`Tool::execute`] to catch
/// LLM hallucinations early.
///
/// [`Tool::execute`]: roos_core::Tool::execute
pub fn validate_tool_input(
    tool_name: &str,
    schema: &Value,
    input: &Value,
) -> Result<(), ValidationError> {
    let compiled = JSONSchema::compile(schema).map_err(|e| ValidationError {
        tool: tool_name.to_owned(),
        violations: vec![format!("invalid schema: {e}")],
    })?;

    // Collect violations before `compiled` is dropped (borrow constraint).
    let violations: Vec<String> = compiled
        .validate(input)
        .err()
        .into_iter()
        .flatten()
        .map(|e| e.to_string())
        .collect();

    if violations.is_empty() {
        Ok(())
    } else {
        Err(ValidationError {
            tool: tool_name.to_owned(),
            violations,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn string_schema() -> Value {
        serde_json::json!({
            "type": "object",
            "properties": { "text": { "type": "string" } },
            "required": ["text"]
        })
    }

    #[test]
    fn valid_input_returns_ok() {
        let result = validate_tool_input(
            "echo",
            &string_schema(),
            &serde_json::json!({ "text": "hi" }),
        );
        assert!(result.is_ok());
    }

    #[test]
    fn missing_required_field_fails() {
        let err =
            validate_tool_input("echo", &string_schema(), &serde_json::json!({})).unwrap_err();
        assert_eq!(err.tool, "echo");
        assert!(!err.violations.is_empty());
    }

    #[test]
    fn wrong_type_fails() {
        let err = validate_tool_input("echo", &string_schema(), &serde_json::json!({ "text": 42 }))
            .unwrap_err();
        assert!(!err.violations.is_empty());
    }

    #[test]
    fn empty_schema_accepts_any_object() {
        let result = validate_tool_input(
            "noop",
            &serde_json::json!({ "type": "object" }),
            &serde_json::json!({ "anything": true }),
        );
        assert!(result.is_ok());
    }

    #[test]
    fn display_includes_tool_name_and_violations() {
        let err =
            validate_tool_input("echo", &string_schema(), &serde_json::json!({})).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("echo"));
        assert!(msg.contains("validation failed"));
    }
}
