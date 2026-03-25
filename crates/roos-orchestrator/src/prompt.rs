use roos_core::provider::CompletionConfig;

/// Builds the system prompt injected into every LLM call for an agent.
///
/// Assembles agent identity, tool catalogue, and optional custom instructions
/// into a single string, then injects it into a [`CompletionConfig`] via
/// [`inject_into`](SystemPromptBuilder::inject_into).
pub struct SystemPromptBuilder {
    agent_name: String,
    agent_description: String,
    tools: Vec<ToolEntry>,
    custom: Option<String>,
}

struct ToolEntry {
    name: String,
    description: String,
    schema: serde_json::Value,
}

impl SystemPromptBuilder {
    /// Create a builder for an agent with the given name and description.
    pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            agent_name: name.into(),
            agent_description: description.into(),
            tools: Vec::new(),
            custom: None,
        }
    }

    /// Register a tool in the system prompt.
    pub fn with_tool(
        mut self,
        name: impl Into<String>,
        description: impl Into<String>,
        schema: serde_json::Value,
    ) -> Self {
        self.tools.push(ToolEntry {
            name: name.into(),
            description: description.into(),
            schema,
        });
        self
    }

    /// Override the default instructions section with custom text.
    pub fn with_custom(mut self, prompt: impl Into<String>) -> Self {
        self.custom = Some(prompt.into());
        self
    }

    /// Render the complete system prompt string.
    pub fn build(&self) -> String {
        let mut out = String::new();

        // Identity
        out.push_str(&format!(
            "You are **{}**, {}.\n",
            self.agent_name, self.agent_description
        ));

        // Tool catalogue
        if !self.tools.is_empty() {
            out.push_str("\n## Available Tools\n");
            for tool in &self.tools {
                out.push_str(&format!("\n### {}\n{}\n", tool.name, tool.description));
                if let Ok(schema_str) = serde_json::to_string_pretty(&tool.schema) {
                    out.push_str(&format!("```json\n{schema_str}\n```\n"));
                }
            }
        }

        // Instructions
        out.push_str("\n## Instructions\n");
        match &self.custom {
            Some(text) => out.push_str(text),
            None => out.push_str(
                "Think step-by-step. Use the available tools when needed. \
                 Respond concisely once you have a final answer.",
            ),
        }

        out
    }

    /// Inject the rendered system prompt into `config.system`.
    pub fn inject_into(&self, config: &mut CompletionConfig) {
        config.system = Some(self.build());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_contains_agent_identity() {
        let prompt = SystemPromptBuilder::new("Aria", "a helpful assistant").build();
        assert!(prompt.contains("Aria"));
        assert!(prompt.contains("a helpful assistant"));
    }

    #[test]
    fn no_tools_section_when_empty() {
        let prompt = SystemPromptBuilder::new("Bot", "desc").build();
        assert!(!prompt.contains("## Available Tools"));
    }

    #[test]
    fn tool_name_and_description_appear() {
        let prompt = SystemPromptBuilder::new("Bot", "desc")
            .with_tool("read_file", "Reads a file.", serde_json::json!({}))
            .build();
        assert!(prompt.contains("read_file"));
        assert!(prompt.contains("Reads a file."));
        assert!(prompt.contains("## Available Tools"));
    }

    #[test]
    fn schema_rendered_as_json_block() {
        let schema = serde_json::json!({ "type": "object" });
        let prompt = SystemPromptBuilder::new("Bot", "desc")
            .with_tool("t", "d", schema)
            .build();
        assert!(prompt.contains("```json"));
        assert!(prompt.contains("\"type\""));
    }

    #[test]
    fn custom_instructions_override_default() {
        let prompt = SystemPromptBuilder::new("Bot", "desc")
            .with_custom("Always respond in Spanish.")
            .build();
        assert!(prompt.contains("Always respond in Spanish."));
        assert!(!prompt.contains("Think step-by-step"));
    }

    #[test]
    fn default_instructions_present_without_custom() {
        let prompt = SystemPromptBuilder::new("Bot", "desc").build();
        assert!(prompt.contains("Think step-by-step"));
    }

    #[test]
    fn multiple_tools_all_appear() {
        let prompt = SystemPromptBuilder::new("Bot", "desc")
            .with_tool("alpha", "first", serde_json::json!({}))
            .with_tool("beta", "second", serde_json::json!({}))
            .build();
        assert!(prompt.contains("alpha"));
        assert!(prompt.contains("beta"));
    }

    #[test]
    fn inject_into_sets_system_on_config() {
        let builder = SystemPromptBuilder::new("Bot", "desc");
        let mut config = CompletionConfig::new("test-model");
        assert!(config.system.is_none());
        builder.inject_into(&mut config);
        assert!(config.system.is_some());
        assert!(config.system.unwrap().contains("Bot"));
    }
}
