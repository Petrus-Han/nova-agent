use async_trait::async_trait;
use nova_protocol::{ParameterType, ToolDefinition, ToolParameter, ToolResult};
use std::path::Path;
use std::process::Command;

use crate::Tool;

/// Search file contents using regex patterns. Delegates to ripgrep if available,
/// otherwise falls back to a built-in implementation.
pub struct GrepTool;

#[async_trait]
impl Tool for GrepTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "grep".to_string(),
            description: "Search for a regex pattern in files. Returns matching lines with file paths and line numbers.".to_string(),
            parameters: vec![
                ToolParameter {
                    name: "pattern".to_string(),
                    description: "The regex pattern to search for".to_string(),
                    param_type: ParameterType::String,
                    required: true,
                },
                ToolParameter {
                    name: "path".to_string(),
                    description: "File or directory to search in. Defaults to current directory.".to_string(),
                    param_type: ParameterType::String,
                    required: false,
                },
                ToolParameter {
                    name: "glob".to_string(),
                    description: "Glob pattern to filter files (e.g., '*.rs', '*.{ts,tsx}')".to_string(),
                    param_type: ParameterType::String,
                    required: false,
                },
                ToolParameter {
                    name: "case_insensitive".to_string(),
                    description: "Case insensitive search. Default: false".to_string(),
                    param_type: ParameterType::Boolean,
                    required: false,
                },
                ToolParameter {
                    name: "context".to_string(),
                    description: "Number of context lines to show around matches".to_string(),
                    param_type: ParameterType::Integer,
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
        let glob_filter = input.get("glob").and_then(|v| v.as_str());
        let case_insensitive = input
            .get("case_insensitive")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let context = input
            .get("context")
            .and_then(|v| v.as_u64())
            .map(|v| v as usize);

        match grep_search(pattern, search_path, glob_filter, case_insensitive, context) {
            Ok(output) => ToolResult {
                tool_use_id: id.to_string(),
                output,
                is_error: false,
            },
            Err(e) => ToolResult {
                tool_use_id: id.to_string(),
                output: format!("Error: {e}"),
                is_error: true,
            },
        }
    }
}

fn grep_search(
    pattern: &str,
    path: &str,
    glob_filter: Option<&str>,
    case_insensitive: bool,
    context: Option<usize>,
) -> anyhow::Result<String> {
    let search_path = Path::new(path);
    if !search_path.exists() {
        anyhow::bail!("Path not found: {}", search_path.display());
    }

    // Try ripgrep first (faster), fall back to built-in
    if let Ok(output) = rg_search(pattern, path, glob_filter, case_insensitive, context) {
        return Ok(output);
    }

    // Fallback: simple built-in grep using ignore walker
    builtin_grep(pattern, search_path, glob_filter, case_insensitive, context)
}

fn rg_search(
    pattern: &str,
    path: &str,
    glob_filter: Option<&str>,
    case_insensitive: bool,
    context: Option<usize>,
) -> anyhow::Result<String> {
    let mut cmd = Command::new("rg");
    cmd.arg("--line-number")
        .arg("--no-heading")
        .arg("--color=never");

    if case_insensitive {
        cmd.arg("--ignore-case");
    }
    if let Some(ctx) = context {
        cmd.arg(format!("--context={ctx}"));
    }
    if let Some(g) = glob_filter {
        cmd.arg("--glob").arg(g);
    }

    cmd.arg(pattern).arg(path);

    let output = cmd.output()?;
    if output.status.success() || output.status.code() == Some(1) {
        let stdout = String::from_utf8_lossy(&output.stdout);
        if stdout.is_empty() {
            Ok("No matches found".to_string())
        } else {
            // Truncate output if too large
            let result = stdout.to_string();
            if result.len() > 30000 {
                Ok(format!(
                    "{}\n\n... (output truncated, {} total bytes)",
                    &result[..30000],
                    result.len()
                ))
            } else {
                Ok(result)
            }
        }
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("rg failed: {stderr}")
    }
}

fn builtin_grep(
    pattern: &str,
    path: &Path,
    glob_filter: Option<&str>,
    case_insensitive: bool,
    _context: Option<usize>,
) -> anyhow::Result<String> {
    let regex = if case_insensitive {
        regex::RegexBuilder::new(pattern)
            .case_insensitive(true)
            .build()?
    } else {
        regex::Regex::new(pattern)?
    };

    let mut results = Vec::new();
    let mut walker = ignore::WalkBuilder::new(path);
    walker.hidden(false).git_ignore(true);

    if let Some(g) = glob_filter {
        let glob = globset::Glob::new(g)?.compile_matcher();
        for entry in walker.build() {
            let entry = entry?;
            let entry_path = entry.path();
            if entry_path.is_file() {
                let relative = entry_path.strip_prefix(path).unwrap_or(entry_path);
                if glob.is_match(relative) {
                    search_file(&regex, entry_path, &mut results)?;
                }
            }
        }
    } else {
        for entry in walker.build() {
            let entry = entry?;
            if entry.path().is_file() {
                search_file(&regex, entry.path(), &mut results)?;
            }
        }
    }

    if results.is_empty() {
        Ok("No matches found".to_string())
    } else {
        Ok(results.join("\n"))
    }
}

fn search_file(regex: &regex::Regex, path: &Path, results: &mut Vec<String>) -> anyhow::Result<()> {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return Ok(()), // Skip binary/unreadable files
    };

    for (line_num, line) in content.lines().enumerate() {
        if regex.is_match(line) {
            results.push(format!("{}:{}:{}", path.display(), line_num + 1, line));
        }
    }

    Ok(())
}
