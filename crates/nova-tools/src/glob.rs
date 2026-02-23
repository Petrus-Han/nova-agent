use async_trait::async_trait;
use globset::{Glob, GlobSetBuilder};
use nova_protocol::{ParameterType, ToolDefinition, ToolParameter, ToolResult};
use std::path::Path;

use crate::Tool;

/// Find files matching glob patterns. Fast parallel directory traversal.
pub struct GlobTool;

#[async_trait]
impl Tool for GlobTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "glob".to_string(),
            description: "Find files matching a glob pattern (e.g., '**/*.rs', 'src/**/*.ts'). Returns matching file paths.".to_string(),
            parameters: vec![
                ToolParameter {
                    name: "pattern".to_string(),
                    description: "The glob pattern to match files against".to_string(),
                    param_type: ParameterType::String,
                    required: true,
                },
                ToolParameter {
                    name: "path".to_string(),
                    description: "The directory to search in. Defaults to current directory.".to_string(),
                    param_type: ParameterType::String,
                    required: false,
                },
            ],
        }
    }

    async fn execute(&self, id: &str, input: serde_json::Value) -> ToolResult {
        let pattern = match input.get("pattern").and_then(|v| v.as_str()) {
            Some(p) => p,
            None => {
                return ToolResult {
                    tool_use_id: id.to_string(),
                    output: "Error: pattern parameter is required".to_string(),
                    is_error: true,
                };
            }
        };

        let search_path = input
            .get("path")
            .and_then(|v| v.as_str())
            .unwrap_or(".");

        match glob_search(search_path, pattern) {
            Ok(files) => {
                let output = if files.is_empty() {
                    "No files matched the pattern".to_string()
                } else {
                    let count = files.len();
                    let listing = files.join("\n");
                    format!("{listing}\n\n({count} files matched)")
                };
                ToolResult {
                    tool_use_id: id.to_string(),
                    output,
                    is_error: false,
                }
            }
            Err(e) => ToolResult {
                tool_use_id: id.to_string(),
                output: format!("Error: {e}"),
                is_error: true,
            },
        }
    }
}

fn glob_search(base: &str, pattern: &str) -> anyhow::Result<Vec<String>> {
    let base_path = Path::new(base);
    if !base_path.exists() {
        anyhow::bail!("Directory not found: {}", base_path.display());
    }

    let glob = Glob::new(pattern)?;
    let mut builder = GlobSetBuilder::new();
    builder.add(glob);
    let globset = builder.build()?;

    let mut matches = Vec::new();

    // Use the ignore crate for fast directory walking (respects .gitignore)
    let walker = ignore::WalkBuilder::new(base_path)
        .hidden(false)
        .git_ignore(true)
        .build();

    for entry in walker {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() {
            let relative = path
                .strip_prefix(base_path)
                .unwrap_or(path);
            if globset.is_match(relative) {
                matches.push(path.display().to_string());
            }
        }
    }

    matches.sort();
    Ok(matches)
}
