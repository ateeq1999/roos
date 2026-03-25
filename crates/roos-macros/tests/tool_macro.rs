use roos_core::Tool;
use roos_macros::tool;
use schemars::JsonSchema;
use serde::Deserialize;

// ── Test fixture ─────────────────────────────────────────────────────────────

#[derive(Deserialize, JsonSchema)]
struct EchoInput {
    text: String,
}

#[tool(name = "echo", description = "Returns the input text unchanged.")]
async fn echo(input: EchoInput) -> Result<String, roos_core::ToolError> {
    Ok(input.text.clone())
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[test]
fn tool_name_and_description() {
    let t = EchoTool;
    assert_eq!(t.name(), "echo");
    assert_eq!(t.description(), "Returns the input text unchanged.");
}

#[test]
fn schema_is_valid_json_object() {
    let schema = EchoTool.schema();
    assert!(schema.is_object(), "schema must be a JSON object");
}

#[test]
fn schema_contains_text_property() {
    let schema = EchoTool.schema();
    // schemars puts properties under "properties" in the schema
    let props = &schema["properties"];
    assert!(props.is_object());
    assert!(
        props.get("text").is_some(),
        "expected 'text' in schema properties"
    );
}

#[tokio::test]
async fn execute_valid_input() {
    let result = EchoTool
        .execute(serde_json::json!({ "text": "hello" }))
        .await
        .unwrap();
    assert_eq!(result, "hello");
}

#[tokio::test]
async fn execute_invalid_input_returns_tool_error() {
    let err = EchoTool
        .execute(serde_json::json!({ "wrong_field": 42 }))
        .await
        .unwrap_err();
    assert!(matches!(err, roos_core::ToolError::InvalidInput { .. }));
}

#[tokio::test]
async fn tool_is_object_safe() {
    let t: Box<dyn Tool> = Box::new(EchoTool);
    let out = t
        .execute(serde_json::json!({ "text": "world" }))
        .await
        .unwrap();
    assert_eq!(out, "world");
}
