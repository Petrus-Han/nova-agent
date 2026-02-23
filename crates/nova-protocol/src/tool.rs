use serde::{Deserialize, Serialize};

/// Definition of a tool that the agent can use.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: Vec<ToolParameter>,
}

/// A parameter in a tool definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolParameter {
    pub name: String,
    pub description: String,
    pub param_type: ParameterType,
    pub required: bool,
}

/// Supported parameter types.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ParameterType {
    String,
    Integer,
    Float,
    Boolean,
    Array,
    Object,
}

/// A tool call made by the agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub input: serde_json::Value,
}

/// Result of a tool execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub tool_use_id: String,
    pub output: String,
    pub is_error: bool,
}

impl ToolDefinition {
    /// Convert to the JSON schema format expected by LLM APIs.
    pub fn to_api_schema(&self) -> serde_json::Value {
        let mut properties = serde_json::Map::new();
        let mut required = Vec::new();

        for param in &self.parameters {
            let type_str = match param.param_type {
                ParameterType::String => "string",
                ParameterType::Integer => "integer",
                ParameterType::Float => "number",
                ParameterType::Boolean => "boolean",
                ParameterType::Array => "array",
                ParameterType::Object => "object",
            };

            properties.insert(
                param.name.clone(),
                serde_json::json!({
                    "type": type_str,
                    "description": param.description,
                }),
            );

            if param.required {
                required.push(serde_json::Value::String(param.name.clone()));
            }
        }

        serde_json::json!({
            "name": self.name,
            "description": self.description,
            "input_schema": {
                "type": "object",
                "properties": properties,
                "required": required,
            }
        })
    }
}
