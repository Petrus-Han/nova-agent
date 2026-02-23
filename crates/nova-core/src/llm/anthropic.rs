use async_trait::async_trait;
use nova_protocol::{ContentBlock, Message, Role, ToolDefinition};
use reqwest::Client;
use serde_json::json;

use super::{LlmAdapter, LlmConfig, StreamChunk};

const ANTHROPIC_API_URL: &str = "https://api.anthropic.com/v1/messages";

pub struct AnthropicAdapter {
    client: Client,
    config: LlmConfig,
}

impl AnthropicAdapter {
    pub fn new(config: LlmConfig) -> Self {
        let client = Client::new();
        Self { client, config }
    }

    fn build_messages(&self, messages: &[Message]) -> Vec<serde_json::Value> {
        messages
            .iter()
            .filter(|m| m.role != Role::System)
            .map(|msg| {
                let content: Vec<serde_json::Value> = msg
                    .content
                    .iter()
                    .map(|block| match block {
                        ContentBlock::Text { text } => json!({
                            "type": "text",
                            "text": text,
                        }),
                        ContentBlock::ToolUse { id, name, input } => json!({
                            "type": "tool_use",
                            "id": id,
                            "name": name,
                            "input": input,
                        }),
                        ContentBlock::ToolResult {
                            tool_use_id,
                            content,
                            is_error,
                        } => json!({
                            "type": "tool_result",
                            "tool_use_id": tool_use_id,
                            "content": content,
                            "is_error": is_error,
                        }),
                    })
                    .collect();

                let role = match msg.role {
                    Role::User | Role::Tool => "user",
                    Role::Assistant => "assistant",
                    Role::System => unreachable!(),
                };

                json!({
                    "role": role,
                    "content": content,
                })
            })
            .collect()
    }

    fn build_tools(&self, tools: &[ToolDefinition]) -> Vec<serde_json::Value> {
        tools.iter().map(|t| t.to_api_schema()).collect()
    }
}

#[async_trait]
impl LlmAdapter for AnthropicAdapter {
    async fn chat(
        &self,
        system: &str,
        messages: &[Message],
        tools: &[ToolDefinition],
    ) -> anyhow::Result<Vec<StreamChunk>> {
        let api_messages = self.build_messages(messages);
        let api_tools = self.build_tools(tools);

        let mut body = json!({
            "model": self.config.model,
            "max_tokens": self.config.max_tokens,
            "system": system,
            "messages": api_messages,
        });

        if !api_tools.is_empty() {
            body["tools"] = json!(api_tools);
        }

        if self.config.temperature > 0.0 {
            body["temperature"] = json!(self.config.temperature);
        }

        let response = self
            .client
            .post(ANTHROPIC_API_URL)
            .header("x-api-key", &self.config.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_body = response.text().await.unwrap_or_default();
            anyhow::bail!("Anthropic API error ({}): {}", status, error_body);
        }

        let response_body: serde_json::Value = response.json().await?;

        let mut chunks = Vec::new();

        // Parse usage
        if let Some(usage) = response_body.get("usage") {
            if let Some(input) = usage.get("input_tokens").and_then(|v| v.as_u64()) {
                chunks.push(StreamChunk::InputTokens(input));
            }
            if let Some(output) = usage.get("output_tokens").and_then(|v| v.as_u64()) {
                chunks.push(StreamChunk::OutputTokens(output));
            }
        }

        // Parse content blocks
        if let Some(content) = response_body.get("content").and_then(|v| v.as_array()) {
            for block in content {
                match block.get("type").and_then(|v| v.as_str()) {
                    Some("text") => {
                        if let Some(text) = block.get("text").and_then(|v| v.as_str()) {
                            chunks.push(StreamChunk::TextDelta(text.to_string()));
                        }
                    }
                    Some("tool_use") => {
                        let id = block
                            .get("id")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        let name = block
                            .get("name")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        let input = block
                            .get("input")
                            .cloned()
                            .unwrap_or(json!({}));
                        chunks.push(StreamChunk::ToolUse { id, name, input });
                    }
                    _ => {}
                }
            }
        }

        chunks.push(StreamChunk::Done);
        Ok(chunks)
    }

    fn provider_name(&self) -> &str {
        "anthropic"
    }

    fn model_name(&self) -> &str {
        &self.config.model
    }
}
