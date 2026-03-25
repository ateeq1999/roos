use std::path::Path;
use std::sync::Arc;

use roos_core::provider::{CompletionConfig, LLMProvider};
use roos_core::types::AgentInput;
use roos_core::RoosConfig;
use roos_memory::InMemoryStore;
use roos_orchestrator::ReasoningLoop;

pub async fn run(config_path: &str, input_text: &str) -> anyhow::Result<()> {
    let cfg = RoosConfig::from_file(Path::new(config_path))
        .map_err(|e| anyhow::anyhow!("Failed to load '{}': {e}", config_path))?;

    let api_key = cfg.provider.api_key.as_deref().unwrap_or("");
    let provider: Arc<dyn LLMProvider> = match cfg.provider.provider_type.as_str() {
        "anthropic" => Arc::new(roos_providers::AnthropicProvider::new(api_key)),
        "openai" => Arc::new(roos_providers::OpenAIProvider::new(api_key)),
        other => anyhow::bail!(
            "Unknown provider type '{}'. Supported: anthropic, openai",
            other
        ),
    };

    let memory = Arc::new(InMemoryStore::new());
    let loop_ = ReasoningLoop::new(provider, vec![], memory);

    let mut comp_config = CompletionConfig::new(&cfg.provider.model);
    comp_config.max_tokens = cfg.provider.max_tokens;
    if !cfg.agent.description.is_empty() {
        comp_config.system = Some(cfg.agent.description);
    }

    let input = AgentInput::new(input_text);
    let output = loop_
        .run(input, &comp_config)
        .await
        .map_err(|e| anyhow::anyhow!("Agent run failed: {e}"))?;

    println!("{}", output.content);
    if output.steps_taken > 0 || !output.tools_called.is_empty() {
        println!(
            "\n[steps: {}, tools: {}, tokens: {}]",
            output.steps_taken,
            output.tools_called.len(),
            output.total_tokens.total
        );
    }
    Ok(())
}
