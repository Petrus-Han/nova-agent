use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::tool::{ToolCall, ToolResult};

/// Role in a conversation message.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
    Tool,
}

/// Content block within a message.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
    Text {
        text: String,
    },
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    ToolResult {
        tool_use_id: String,
        content: String,
        is_error: bool,
    },
}

/// A conversation message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: Vec<ContentBlock>,
}

impl Message {
    pub fn user(text: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: vec![ContentBlock::Text { text: text.into() }],
        }
    }

    pub fn assistant(text: impl Into<String>) -> Self {
        Self {
            role: Role::Assistant,
            content: vec![ContentBlock::Text { text: text.into() }],
        }
    }

    pub fn system(text: impl Into<String>) -> Self {
        Self {
            role: Role::System,
            content: vec![ContentBlock::Text { text: text.into() }],
        }
    }

    pub fn tool_result(tool_use_id: impl Into<String>, content: impl Into<String>, is_error: bool) -> Self {
        Self {
            role: Role::Tool,
            content: vec![ContentBlock::ToolResult {
                tool_use_id: tool_use_id.into(),
                content: content.into(),
                is_error,
            }],
        }
    }

    pub fn text_content(&self) -> String {
        self.content
            .iter()
            .filter_map(|block| match block {
                ContentBlock::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("")
    }
}

/// Inbound request from the user/client.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Request {
    UserInput {
        id: Uuid,
        content: String,
        #[serde(default)]
        attachments: Vec<String>,
    },
    ToolApproval {
        id: Uuid,
        tool_use_id: String,
        approved: bool,
    },
    Compact {
        #[serde(default = "default_threshold")]
        threshold: f32,
    },
    Cancel {
        id: Uuid,
    },
}

fn default_threshold() -> f32 {
    0.5
}

/// Outbound response/event from the agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Response {
    Thinking {
        content: String,
    },
    Text {
        content: String,
    },
    ToolCall {
        #[serde(flatten)]
        call: ToolCall,
    },
    ToolResult {
        #[serde(flatten)]
        result: ToolResult,
    },
    Error {
        code: ErrorCode,
        message: String,
    },
    Done {
        summary: String,
    },
}

/// Error codes for protocol-level errors.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    InvalidRequest,
    ToolNotFound,
    ToolExecutionFailed,
    LlmError,
    PermissionDenied,
    ContextOverflow,
    Internal,
}

/// Streaming events emitted during agent execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum Event {
    /// Agent is generating text (streamed token-by-token).
    TextDelta { delta: String },
    /// Agent started thinking.
    ThinkingStart,
    /// Agent thinking content delta.
    ThinkingDelta { delta: String },
    /// Agent finished thinking.
    ThinkingEnd,
    /// Agent is calling a tool.
    ToolStart { call: ToolCall },
    /// Tool execution completed.
    ToolEnd { result: ToolResult },
    /// Agent turn completed.
    TurnComplete { summary: String },
    /// Error occurred.
    Error { code: ErrorCode, message: String },
}
