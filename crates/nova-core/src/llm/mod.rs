pub mod anthropic;
pub mod openai_compat;

use async_trait::async_trait;
use nova_protocol::{Message, ToolDefinition};
use serde::{Deserialize, Serialize};

/// Configuration for the LLM provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfig {
    pub provider: LlmProvider,
    pub api_key: String,
    pub model: String,
    #[serde(default)]
    pub base_url: Option<String>,
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
    Zhipu,
    DeepSeek,
    Custom,
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
    /// Send messages to the LLM and get a response.
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
        LlmProvider::Anthropic => {
            Ok(Box::new(anthropic::AnthropicAdapter::new(config.clone())))
        }
        LlmProvider::OpenAI | LlmProvider::Zhipu | LlmProvider::DeepSeek | LlmProvider::Custom => {
            Ok(Box::new(openai_compat::OpenAICompatAdapter::new(
                config.clone(),
                config.base_url.clone(),
                None,
            )))
        }
    }
}

/// Auto-detect provider from environment variables.
/// Checks ANTHROPIC_API_KEY, OPENAI_API_KEY, ZHIPU_API_KEY, DEEPSEEK_API_KEY.
pub fn detect_from_env() -> Option<(LlmProvider, String)> {
    if let Ok(key) = std::env::var("ANTHROPIC_API_KEY") {
        return Some((LlmProvider::Anthropic, key));
    }
    if let Ok(key) = std::env::var("OPENAI_API_KEY") {
        return Some((LlmProvider::OpenAI, key));
    }
    if let Ok(key) = std::env::var("ZHIPU_API_KEY") {
        return Some((LlmProvider::Zhipu, key));
    }
    if let Ok(key) = std::env::var("DEEPSEEK_API_KEY") {
        return Some((LlmProvider::DeepSeek, key));
    }
    None
}

/// Get default model for a provider.
pub fn default_model(provider: &LlmProvider) -> &'static str {
    match provider {
        LlmProvider::Anthropic => "claude-sonnet-4-20250514",
        LlmProvider::OpenAI => "gpt-4o",
        LlmProvider::Zhipu => "glm-4-plus",
        LlmProvider::DeepSeek => "deepseek-chat",
        LlmProvider::Custom => "default",
    }
}
