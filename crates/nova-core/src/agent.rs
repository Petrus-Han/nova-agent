use nova_protocol::{ContentBlock, Event, Message, Role, ToolCall};
use nova_tools::ToolRegistry;
use tracing::{debug, info, warn};

use crate::context::ContextManager;
use crate::llm::{LlmAdapter, StreamChunk};
use crate::permission::PermissionEngine;

const MAX_TOOL_ROUNDS: usize = 50;

/// The core agent loop: receives user input, calls LLM, executes tools, repeats.
pub struct AgentLoop {
    llm: Box<dyn LlmAdapter>,
    tools: ToolRegistry,
    context: ContextManager,
    permissions: PermissionEngine,
    event_handler: Option<Box<dyn Fn(Event) + Send + Sync>>,
    total_input_tokens: u64,
    total_output_tokens: u64,
}

impl AgentLoop {
    pub fn new(
        llm: Box<dyn LlmAdapter>,
        tools: ToolRegistry,
        system_prompt: String,
        permissions: PermissionEngine,
    ) -> Self {
        Self {
            llm,
            tools,
            context: ContextManager::new(system_prompt),
            permissions,
            event_handler: None,
            total_input_tokens: 0,
            total_output_tokens: 0,
        }
    }

    /// Set an event handler for streaming events to the UI.
    pub fn on_event<F>(&mut self, handler: F)
    where
        F: Fn(Event) + Send + Sync + 'static,
    {
        self.event_handler = Some(Box::new(handler));
    }

    fn emit(&self, event: Event) {
        if let Some(ref handler) = self.event_handler {
            handler(event);
        }
    }

    /// Process a single user message through the agent loop.
    pub async fn process(&mut self, user_input: &str) -> anyhow::Result<String> {
        info!(input_len = user_input.len(), "Processing user input");

        self.context.add_message(Message::user(user_input));

        let mut final_text = String::new();

        for round in 0..MAX_TOOL_ROUNDS {
            debug!(round, "Agent loop round");

            // Check if compaction is needed
            if self.context.needs_compaction() {
                info!("Compacting context");
                self.context.compact(20);
            }

            // Call the LLM
            let chunks = self
                .llm
                .chat(
                    self.context.system_prompt(),
                    self.context.messages(),
                    &self.tools.definitions(),
                )
                .await?;

            // Process response chunks
            let mut text_parts = Vec::new();
            let mut tool_calls: Vec<ToolCall> = Vec::new();

            for chunk in chunks {
                match chunk {
                    StreamChunk::TextDelta(text) => {
                        self.emit(Event::TextDelta {
                            delta: text.clone(),
                        });
                        text_parts.push(text);
                    }
                    StreamChunk::ThinkingDelta(text) => {
                        self.emit(Event::ThinkingDelta { delta: text });
                    }
                    StreamChunk::ToolUse { id, name, input } => {
                        let call = ToolCall { id, name, input };
                        self.emit(Event::ToolStart { call: call.clone() });
                        tool_calls.push(call);
                    }
                    StreamChunk::InputTokens(n) => {
                        self.total_input_tokens += n;
                    }
                    StreamChunk::OutputTokens(n) => {
                        self.total_output_tokens += n;
                    }
                    StreamChunk::Done => {}
                    StreamChunk::Error(e) => {
                        warn!(error = %e, "LLM stream error");
                    }
                }
            }

            // Build assistant message
            let assistant_text = text_parts.join("");
            let mut content_blocks = Vec::new();

            if !assistant_text.is_empty() {
                content_blocks.push(ContentBlock::Text {
                    text: assistant_text.clone(),
                });
            }

            for call in &tool_calls {
                content_blocks.push(ContentBlock::ToolUse {
                    id: call.id.clone(),
                    name: call.name.clone(),
                    input: call.input.clone(),
                });
            }

            if !content_blocks.is_empty() {
                self.context.add_message(Message {
                    role: Role::Assistant,
                    content: content_blocks,
                });
            }

            // If no tool calls, we're done
            if tool_calls.is_empty() {
                final_text = assistant_text;
                break;
            }

            // Execute tool calls
            let mut tool_results_content = Vec::new();

            for call in &tool_calls {
                // Check permissions
                if !self.permissions.is_auto_approved(&call.name) {
                    warn!(tool = %call.name, "Tool requires approval (auto-approving in agent mode)");
                }

                // Execute the tool
                let result = if let Some(tool) = self.tools.get(&call.name) {
                    tool.execute(&call.id, call.input.clone()).await
                } else {
                    nova_protocol::ToolResult {
                        tool_use_id: call.id.clone(),
                        output: format!("Error: tool '{}' not found", call.name),
                        is_error: true,
                    }
                };

                self.emit(Event::ToolEnd {
                    result: result.clone(),
                });

                tool_results_content.push(ContentBlock::ToolResult {
                    tool_use_id: result.tool_use_id,
                    content: result.output,
                    is_error: result.is_error,
                });
            }

            // Add tool results as a user message (Anthropic API format)
            self.context.add_message(Message {
                role: Role::User,
                content: tool_results_content,
            });

            // If we're at the last round, break to avoid infinite loops
            if round == MAX_TOOL_ROUNDS - 1 {
                final_text = "Reached maximum tool call rounds. Stopping.".to_string();
                warn!("Reached maximum tool call rounds");
            }
        }

        self.emit(Event::TurnComplete {
            summary: if final_text.len() > 100 {
                format!("{}...", &final_text[..100])
            } else {
                final_text.clone()
            },
        });

        Ok(final_text)
    }

    /// Get token usage statistics.
    pub fn token_usage(&self) -> (u64, u64) {
        (self.total_input_tokens, self.total_output_tokens)
    }

    /// Get the number of messages in context.
    pub fn context_length(&self) -> usize {
        self.context.message_count()
    }
}
