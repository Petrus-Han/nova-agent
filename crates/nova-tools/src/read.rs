use async_trait::async_trait;
use memmap2::Mmap;
use nova_protocol::{ToolDefinition, ToolParameter, ToolResult, ParameterType};
use std::fs::File;
use std::path::Path;

use crate::Tool;

/// Read file contents using memory-mapped I/O for performance.
pub struct ReadTool;

#[async_trait]
impl Tool for ReadTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "read".to_string(),
            description: "Read the contents of a file. Uses memory-mapped I/O for fast large file reads.".to_string(),
            parameters: vec![
                ToolParameter {
                    name: "file_path".to_string(),
                    description: "Absolute path to the file to read".to_string(),
                    param_type: ParameterType::String,
                    required: true,
                },
                ToolParameter {
                    name: "offset".to_string(),
                    description: "Line number to start reading from (1-based)".to_string(),
                    param_type: ParameterType::Integer,
                    required: false,
                },
                ToolParameter {
                    name: "limit".to_string(),
                    description: "Maximum number of lines to read".to_string(),
                    param_type: ParameterType::Integer,
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

        let offset = input
            .get("offset")
            .and_then(|v| v.as_u64())
            .map(|v| v as usize)
            .unwrap_or(1);
        let limit = input
            .get("limit")
            .and_then(|v| v.as_u64())
            .map(|v| v as usize)
            .unwrap_or(2000);

        match read_file(file_path, offset, limit) {
            Ok(content) => ToolResult {
                tool_use_id: id.to_string(),
                output: content,
                is_error: false,
            },
            Err(e) => ToolResult {
                tool_use_id: id.to_string(),
                output: format!("Error reading file: {e}"),
                is_error: true,
            },
        }
    }
}

fn read_file(path: &str, offset: usize, limit: usize) -> anyhow::Result<String> {
    let path = Path::new(path);
    if !path.exists() {
        anyhow::bail!("File not found: {}", path.display());
    }

    let metadata = std::fs::metadata(path)?;
    if metadata.len() == 0 {
        return Ok("(empty file)".to_string());
    }

    // Use mmap for files > 64KB, regular read for smaller files
    let content = if metadata.len() > 65536 {
        let file = File::open(path)?;
        let mmap = unsafe { Mmap::map(&file)? };
        String::from_utf8_lossy(&mmap).into_owned()
    } else {
        std::fs::read_to_string(path)?
    };

    let lines: Vec<&str> = content.lines().collect();
    let start = (offset.saturating_sub(1)).min(lines.len());
    let end = (start + limit).min(lines.len());

    let mut output = String::new();
    for (i, line) in lines[start..end].iter().enumerate() {
        let line_num = start + i + 1;
        let truncated = if line.len() > 2000 {
            &line[..2000]
        } else {
            line
        };
        output.push_str(&format!("{line_num:>6}\t{truncated}\n"));
    }

    if end < lines.len() {
        output.push_str(&format!(
            "\n... ({} more lines not shown)\n",
            lines.len() - end
        ));
    }

    Ok(output)
}
