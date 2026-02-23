use async_trait::async_trait;
use nova_protocol::{ParameterType, ToolDefinition, ToolParameter, ToolResult};
use std::path::Path;

use crate::Tool;

/// Write content to a file, creating parent directories as needed.
pub struct WriteTool;

#[async_trait]
impl Tool for WriteTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "write".to_string(),
            description: "Write content to a file. Creates the file and parent directories if they don't exist. Overwrites existing content.".to_string(),
            parameters: vec![
                ToolParameter {
                    name: "file_path".to_string(),
                    description: "Absolute path to the file to write".to_string(),
                    param_type: ParameterType::String,
                    required: true,
                },
                ToolParameter {
                    name: "content".to_string(),
                    description: "The content to write to the file".to_string(),
                    param_type: ParameterType::String,
                    required: true,
                },
            ],
        }
    }

    async fn execute(&self, id: &str, input: serde_json::Value) -> ToolResult {
        let file_path = match input.get("file_path").and_then(|v| v.as_str()) {
            Some(p) => p,
            None => {
                return ToolResult {
                    tool_use_id: id.to_string(),
                    output: "Error: file_path parameter is required".to_string(),
                    is_error: true,
                };
            }
        };

        let content = match input.get("content").and_then(|v| v.as_str()) {
            Some(c) => c,
            None => {
                return ToolResult {
                    tool_use_id: id.to_string(),
                    output: "Error: content parameter is required".to_string(),
                    is_error: true,
                };
            }
        };

        match write_file(file_path, content) {
            Ok(bytes) => ToolResult {
                tool_use_id: id.to_string(),
                output: format!("Successfully wrote {bytes} bytes to {file_path}"),
                is_error: false,
            },
            Err(e) => ToolResult {
                tool_use_id: id.to_string(),
                output: format!("Error writing file: {e}"),
                is_error: true,
            },
        }
    }
}

fn write_file(path: &str, content: &str) -> anyhow::Result<usize> {
    let path = Path::new(path);

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    std::fs::write(path, content)?;
    Ok(content.len())
}
