use async_trait::async_trait;
use nova_protocol::{ParameterType, ToolDefinition, ToolParameter, ToolResult};
use tokio::process::Command;

use crate::Tool;

const DEFAULT_TIMEOUT_MS: u64 = 120_000; // 2 minutes
const MAX_OUTPUT_BYTES: usize = 30_000;

/// Execute bash commands asynchronously using tokio::process.
pub struct BashTool;

#[async_trait]
impl Tool for BashTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "bash".to_string(),
            description: "Execute a bash command and return stdout/stderr. Commands run in a non-interactive shell.".to_string(),
            parameters: vec![
                ToolParameter {
                    name: "command".to_string(),
                    description: "The bash command to execute".to_string(),
                    param_type: ParameterType::String,
                    required: true,
                },
                ToolParameter {
                    name: "timeout".to_string(),
                    description: "Timeout in milliseconds (default: 120000, max: 600000)".to_string(),
                    param_type: ParameterType::Integer,
                    required: false,
                },
            ],
        }
    }

    async fn execute(&self, id: &str, input: serde_json::Value) -> ToolResult {
        let command = match input.get("command").and_then(|v| v.as_str()) {
            Some(c) => c,
            None => {
                return ToolResult {
                    tool_use_id: id.to_string(),
                    output: "Error: command parameter is required".to_string(),
                    is_error: true,
                };
            }
        };

        let timeout_ms = input
            .get("timeout")
            .and_then(|v| v.as_u64())
            .unwrap_or(DEFAULT_TIMEOUT_MS)
            .min(600_000);

        match execute_command(command, timeout_ms).await {
            Ok((stdout, stderr, code)) => {
                let mut output = String::new();
                if !stdout.is_empty() {
                    output.push_str(&stdout);
                }
                if !stderr.is_empty() {
                    if !output.is_empty() {
                        output.push('\n');
                    }
                    output.push_str(&stderr);
                }
                if output.is_empty() {
                    output = "(no output)".to_string();
                }

                // Truncate if too large
                if output.len() > MAX_OUTPUT_BYTES {
                    output = format!(
                        "{}\n\n... (output truncated, {} total bytes)",
                        &output[..MAX_OUTPUT_BYTES],
                        output.len()
                    );
                }

                ToolResult {
                    tool_use_id: id.to_string(),
                    output,
                    is_error: code != 0,
                }
            }
            Err(e) => ToolResult {
                tool_use_id: id.to_string(),
                output: format!("Error executing command: {e}"),
                is_error: true,
            },
        }
    }
}

async fn execute_command(command: &str, timeout_ms: u64) -> anyhow::Result<(String, String, i32)> {
    let timeout = std::time::Duration::from_millis(timeout_ms);

    let result = tokio::time::timeout(timeout, async {
        let output = Command::new("bash")
            .arg("-c")
            .arg(command)
            .output()
            .await?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let code = output.status.code().unwrap_or(-1);

        Ok::<_, anyhow::Error>((stdout, stderr, code))
    })
    .await;

    match result {
        Ok(inner) => inner,
        Err(_) => anyhow::bail!("Command timed out after {timeout_ms}ms"),
    }
}
