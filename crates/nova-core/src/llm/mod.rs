pub mod anthropic;

use async_trait::async_trait;
use nova_protocol::{Message, ToolDefinition};
use serde::{Deserialize, Serialize};

/// Configuration for the LLM provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfig {
    pub provider: LlmProvider,
    pub api_key: String,
    pub model: String,
    #[serde(default = "default_max_tokens")]
    pub max_tokens: u32,
    #[serde(default = "default_temperature")]
    pub temperature: f32,
}

fn default_max_tokens() -> u32 {
    16384
}

fn default_temperature() -> f32 {
    0.0
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum LlmProvider {
    Anthropic,
    OpenAI,
}

/// Streamed chunk from LLM.
#[derive(Debug, Clone)]
pub enum StreamChunk {
    TextDelta(String),
    ThinkingDelta(String),
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    InputTokens(u64),
    OutputTokens(u64),
    Done,
    Error(String),
}

/// Trait for LLM provider adapters.
#[async_trait]
pub trait LlmAdapter: Send + Sync {
    /// Send messages to the LLM and get a streaming response.
    async fn chat(
        &self,
        system: &str,
        messages: &[Message],
        tools: &[ToolDefinition],
    ) -> anyhow::Result<Vec<StreamChunk>>;

    /// Get the provider name.
    fn provider_name(&self) -> &str;

    /// Get the model name.
    fn model_name(&self) -> &str;
}

/// Create an LLM adapter from config.
pub fn create_adapter(config: &LlmConfig) -> anyhow::Result<Box<dyn LlmAdapter>> {
    match config.provider {
        LlmProvider::Anthropic => Ok(Box::new(anthropic::AnthropicAdapter::new(config.clone()))),
        LlmProvider::OpenAI => anyhow::bail!("OpenAI adapter not yet implemented"),
    }
}
