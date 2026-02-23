use async_trait::async_trait;
use nova_protocol::{ContentBlock, Message, Role, ToolDefinition};
use reqwest::Client;
use serde_json::json;

use super::{LlmAdapter, LlmConfig, StreamChunk};

/// Known OpenAI-compatible API endpoints.
pub struct KnownProvider {
    pub name: &'static str,
    pub base_url: &'static str,
    pub auth_header: &'static str,
}

pub const KNOWN_PROVIDERS: &[KnownProvider] = &[
    KnownProvider {
        name: "openai",
        base_url: "https://api.openai.com/v1",
        auth_header: "Authorization",
    },
    KnownProvider {
        name: "zhipu",
        base_url: "https://open.bigmodel.cn/api/paas/v4",
        auth_header: "Authorization",
    },
    KnownProvider {
        name: "deepseek",
        base_url: "https://api.deepseek.com/v1",
        auth_header: "Authorization",
    },
    KnownProvider {
        name: "moonshot",
        base_url: "https://api.moonshot.cn/v1",
        auth_header: "Authorization",
    },
    KnownProvider {
        name: "ollama",
        base_url: "http://localhost:11434/v1",
        auth_header: "Authorization",
    },
];

/// Adapter for any OpenAI-compatible API (OpenAI, Zhipu GLM-4, DeepSeek, etc.)
pub struct OpenAICompatAdapter {
    client: Client,
    config: LlmConfig,
    base_url: String,
    provider_name: String,
}

impl OpenAICompatAdapter {
    pub fn new(config: LlmConfig, base_url: Option<String>, provider_name: Option<String>) -> Self {
        let (resolved_url, resolved_name) = if let Some(url) = base_url {
            let name = provider_name.unwrap_or_else(|| "custom".to_string());
            (url, name)
        } else {
            // Try to detect provider from model name
            detect_provider(&config.model)
        };

        let client = Client::new();
        Self {
            client,
            config,
            base_url: resolved_url,
            provider_name: resolved_name,
        }
    }

    fn build_messages(&self, system: &str, messages: &[Message]) -> Vec<serde_json::Value> {
        let mut api_messages = vec![json!({
            "role": "system",
            "content": system,
        })];

        for msg in messages {
            match msg.role {
                Role::System => continue,
                Role::User | Role::Tool => {
                    // Check if this contains tool results
                    let has_tool_results = msg.content.iter().any(|b| {
                        matches!(b, ContentBlock::ToolResult { .. })
                    });

                    if has_tool_results {
                        // Each tool result becomes a separate "tool" message
                        for block in &msg.content {
                            if let ContentBlock::ToolResult {
                                tool_use_id,
                                content,
                                ..
                            } = block
                            {
                                api_messages.push(json!({
                                    "role": "tool",
                                    "tool_call_id": tool_use_id,
                                    "content": content,
                                }));
                            }
                        }
                    } else {
                        let text = msg.text_content();
                        if !text.is_empty() {
                            api_messages.push(json!({
                                "role": "user",
                                "content": text,
                            }));
                        }
                    }
                }
                Role::Assistant => {
                    let mut assistant_msg = json!({
                        "role": "assistant",
                    });

                    let text = msg.text_content();
                    if !text.is_empty() {
                        assistant_msg["content"] = json!(text);
                    }

                    // Check for tool calls
                    let tool_calls: Vec<serde_json::Value> = msg
                        .content
                        .iter()
                        .filter_map(|block| {
                            if let ContentBlock::ToolUse { id, name, input } = block {
                                Some(json!({
                                    "id": id,
                                    "type": "function",
                                    "function": {
                                        "name": name,
                                        "arguments": input.to_string(),
                                    }
                                }))
                            } else {
                                None
                            }
                        })
                        .collect();

                    if !tool_calls.is_empty() {
                        assistant_msg["tool_calls"] = json!(tool_calls);
                    }

                    api_messages.push(assistant_msg);
                }
            }
        }

        api_messages
    }

    fn build_tools(&self, tools: &[ToolDefinition]) -> Vec<serde_json::Value> {
        tools
            .iter()
            .map(|t| {
                let schema = t.to_api_schema();
                json!({
                    "type": "function",
                    "function": {
                        "name": schema["name"],
                        "description": schema["description"],
                        "parameters": schema["input_schema"],
                    }
                })
            })
            .collect()
    }
}

#[async_trait]
impl LlmAdapter for OpenAICompatAdapter {
    async fn chat(
        &self,
        system: &str,
        messages: &[Message],
        tools: &[ToolDefinition],
    ) -> anyhow::Result<Vec<StreamChunk>> {
        let api_messages = self.build_messages(system, messages);
        let api_tools = self.build_tools(tools);

        let url = format!("{}/chat/completions", self.base_url);

        let mut body = json!({
            "model": self.config.model,
            "messages": api_messages,
            "max_tokens": self.config.max_tokens,
        });

        if !api_tools.is_empty() {
            body["tools"] = json!(api_tools);
            body["tool_choice"] = json!("auto");
        }

        if self.config.temperature > 0.0 {
            body["temperature"] = json!(self.config.temperature);
        }

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_body = response.text().await.unwrap_or_default();
            anyhow::bail!(
                "{} API error ({}): {}",
                self.provider_name,
                status,
                error_body
            );
        }

        let response_body: serde_json::Value = response.json().await?;

        let mut chunks = Vec::new();

        // Parse usage
        if let Some(usage) = response_body.get("usage") {
            if let Some(input) = usage.get("prompt_tokens").and_then(|v| v.as_u64()) {
                chunks.push(StreamChunk::InputTokens(input));
            }
            if let Some(output) = usage.get("completion_tokens").and_then(|v| v.as_u64()) {
                chunks.push(StreamChunk::OutputTokens(output));
            }
        }

        // Parse choices
        if let Some(choices) = response_body.get("choices").and_then(|v| v.as_array()) {
            if let Some(choice) = choices.first() {
                let message = &choice["message"];

                // Text content
                if let Some(content) = message.get("content").and_then(|v| v.as_str()) {
                    if !content.is_empty() {
                        chunks.push(StreamChunk::TextDelta(content.to_string()));
                    }
                }

                // Tool calls
                if let Some(tool_calls) = message.get("tool_calls").and_then(|v| v.as_array()) {
                    for tc in tool_calls {
                        let id = tc
                            .get("id")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        let function = &tc["function"];
                        let name = function
                            .get("name")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        let arguments = function
                            .get("arguments")
                            .and_then(|v| v.as_str())
                            .unwrap_or("{}");
                        let input: serde_json::Value =
                            serde_json::from_str(arguments).unwrap_or(json!({}));

                        chunks.push(StreamChunk::ToolUse { id, name, input });
                    }
                }
            }
        }

        chunks.push(StreamChunk::Done);
        Ok(chunks)
    }

    fn provider_name(&self) -> &str {
        &self.provider_name
    }

    fn model_name(&self) -> &str {
        &self.config.model
    }
}

/// Auto-detect provider and base URL from model name.
fn detect_provider(model: &str) -> (String, String) {
    let model_lower = model.to_lowercase();

    if model_lower.starts_with("glm") || model_lower.contains("zhipu") {
        (
            "https://open.bigmodel.cn/api/paas/v4".to_string(),
            "zhipu".to_string(),
        )
    } else if model_lower.starts_with("deepseek") {
        (
            "https://api.deepseek.com/v1".to_string(),
            "deepseek".to_string(),
        )
    } else if model_lower.starts_with("moonshot") {
        (
            "https://api.moonshot.cn/v1".to_string(),
            "moonshot".to_string(),
        )
    } else {
        // Default to OpenAI
        (
            "https://api.openai.com/v1".to_string(),
            "openai".to_string(),
        )
    }
}
