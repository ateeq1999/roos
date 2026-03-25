use async_trait::async_trait;

use roos_core::provider::{
    CompletionConfig, CompletionResponse, LLMProvider, Message, ProviderError,
};

use crate::openai::complete_compat;

/// DashScope OpenAI-compatible endpoint for Qwen models.
const API_URL: &str = "https://dashscope.aliyuncs.com/compatible-mode/v1/chat/completions";

/// [`LLMProvider`] implementation for Alibaba's Qwen models via DashScope.
///
/// DashScope exposes an OpenAI-compatible Chat Completions endpoint.
/// Construct with [`QwenProvider::new`], passing your DashScope API key.
/// The key is typically loaded via `RoosConfig` from `${DASHSCOPE_API_KEY}`.
pub struct QwenProvider {
    api_key: String,
    client: reqwest::Client,
}

impl QwenProvider {
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl LLMProvider for QwenProvider {
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
        let p = QwenProvider::new("sk-dashscope-test");
        assert_eq!(p.api_key, "sk-dashscope-test");
    }
}
