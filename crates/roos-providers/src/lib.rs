// roos-providers — LLM provider implementations.
// Populated in tasks 15 (Anthropic), 16 (OpenAI), 38 (Ollama).

#[cfg(feature = "provider-anthropic")]
pub mod anthropic;
#[cfg(feature = "provider-anthropic")]
pub use anthropic::AnthropicProvider;

#[cfg(feature = "provider-openai")]
pub mod openai;
#[cfg(feature = "provider-openai")]
pub use openai::OpenAIProvider;

#[cfg(feature = "provider-groq")]
pub mod groq;
#[cfg(feature = "provider-groq")]
pub use groq::GroqProvider;

#[cfg(feature = "provider-qwen")]
pub mod qwen;
#[cfg(feature = "provider-qwen")]
pub use qwen::QwenProvider;

#[cfg(feature = "provider-cohere")]
pub mod cohere;
#[cfg(feature = "provider-cohere")]
pub use cohere::CohereProvider;
