// roos-providers — LLM provider implementations.
// Populated in tasks 15 (Anthropic), 16 (OpenAI), 38 (Ollama).

#[cfg(feature = "provider-anthropic")]
pub mod anthropic;
#[cfg(feature = "provider-anthropic")]
pub use anthropic::AnthropicProvider;
