use async_trait::async_trait;
use nova_protocol::{ParameterType, ToolDefinition, ToolParameter, ToolResult};
use ropey::Rope;
use std::path::Path;

use crate::Tool;

/// Edit a file by performing exact string replacement. Uses the ropey crate
/// for O(log n) text manipulation on large files.
pub struct EditTool;

#[async_trait]
impl Tool for EditTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "edit".to_string(),
            description: "Edit a file by replacing an exact string match with new content. The old_string must be unique in the file.".to_string(),
            parameters: vec![
                ToolParameter {
                    name: "file_path".to_string(),
                    description: "Absolute path to the file to edit".to_string(),
                    param_type: ParameterType::String,
                    required: true,
                },
                ToolParameter {
                    name: "old_string".to_string(),
                    description: "The exact text to find and replace".to_string(),
                    param_type: ParameterType::String,
                    required: true,
                },
                ToolParameter {
                    name: "new_string".to_string(),
                    description: "The replacement text".to_string(),
                    param_type: ParameterType::String,
                    required: true,
                },
                ToolParameter {
                    name: "replace_all".to_string(),
                    description: "If true, replace all occurrences. Default: false".to_string(),
                    param_type: ParameterType::Boolean,
                    required: false,
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

        let old_string = match input.get("old_string").and_then(|v| v.as_str()) {
            Some(s) => s,
            None => {
                return ToolResult {
                    tool_use_id: id.to_string(),
                    output: "Error: old_string parameter is required".to_string(),
                    is_error: true,
                };
            }
        };

        let new_string = match input.get("new_string").and_then(|v| v.as_str()) {
            Some(s) => s,
            None => {
                return ToolResult {
                    tool_use_id: id.to_string(),
                    output: "Error: new_string parameter is required".to_string(),
                    is_error: true,
                };
            }
        };

        let replace_all = input
            .get("replace_all")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        match edit_file(file_path, old_string, new_string, replace_all) {
            Ok(count) => ToolResult {
                tool_use_id: id.to_string(),
                output: format!("Successfully replaced {count} occurrence(s) in {file_path}"),
                is_error: false,
            },
            Err(e) => ToolResult {
                tool_use_id: id.to_string(),
                output: format!("Error editing file: {e}"),
                is_error: true,
            },
        }
    }
}

fn edit_file(path: &str, old: &str, new: &str, replace_all: bool) -> anyhow::Result<usize> {
    let path = Path::new(path);
    if !path.exists() {
        anyhow::bail!("File not found: {}", path.display());
    }

    let content = std::fs::read_to_string(path)?;

    if !replace_all {
        let count = content.matches(old).count();
        if count == 0 {
            anyhow::bail!("old_string not found in file");
        }
        if count > 1 {
            anyhow::bail!(
                "old_string found {count} times in file. Provide more context to make it unique, or set replace_all=true"
            );
        }
    }

    // Use Rope for efficient editing on large files
    let rope = Rope::from_str(&content);
    let rope_str = rope.to_string();

    let (new_content, count) = if replace_all {
        let count = rope_str.matches(old).count();
        (rope_str.replace(old, new), count)
    } else {
        (rope_str.replacen(old, new, 1), 1)
    };

    if count == 0 {
        anyhow::bail!("old_string not found in file");
    }

    std::fs::write(path, new_content)?;
    Ok(count)
}
