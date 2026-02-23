pub mod bash;
pub mod edit;
pub mod glob;
pub mod grep;
pub mod read;
pub mod registry;
pub mod write;

use async_trait::async_trait;
use nova_protocol::{ToolDefinition, ToolResult};

pub use registry::ToolRegistry;

/// Trait that all tools must implement.
#[async_trait]
pub trait Tool: Send + Sync {
    /// Returns the tool definition for LLM API registration.
    fn definition(&self) -> ToolDefinition;

    /// Executes the tool with the given input parameters.
    async fn execute(&self, id: &str, input: serde_json::Value) -> ToolResult;
}
