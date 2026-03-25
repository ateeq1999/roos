use async_trait::async_trait;

use roos_core::provider::{
    CompletionConfig, CompletionResponse, LLMProvider, Message, ProviderError,
};

use crate::openai::complete_compat;

const API_URL: &str = "https://api.groq.com/openai/v1/chat/completions";

/// [`LLMProvider`] implementation for the Groq inference API.
///
/// Groq exposes an OpenAI-compatible Chat Completions endpoint.
/// Construct with [`GroqProvider::new`], passing your API key.
/// The key is typically loaded via `RoosConfig` from `${GROQ_API_KEY}`.
pub struct GroqProvider {
    api_key: String,
    client: reqwest::Client,
}

impl GroqProvider {
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl LLMProvider for GroqProvider {
    async fn complete(
        &self,
        messages: &[Message],
        config: &CompletionConfig,
    ) -> Result<CompletionResponse, ProviderError> {
        complete_compat(&self.client, &self.api_key, API_URL, messages, config).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_new_stores_key() {
        let p = GroqProvider::new("gsk-test");
        assert_eq!(p.api_key, "gsk-test");
    }
}
