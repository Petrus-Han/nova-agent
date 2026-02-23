use serde::{Deserialize, Serialize};

/// Permission mode controlling tool execution.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PermissionMode {
    /// Ask before every tool call.
    Ask,
    /// Auto-approve read-only tools, ask for writes.
    AutoRead,
    /// Auto-approve all tools (trust mode).
    AutoAll,
}

impl Default for PermissionMode {
    fn default() -> Self {
        Self::AutoRead
    }
}

/// Permission engine that decides whether a tool call should proceed.
pub struct PermissionEngine {
    mode: PermissionMode,
}

impl PermissionEngine {
    pub fn new(mode: PermissionMode) -> Self {
        Self { mode }
    }

    /// Check if a tool call is auto-approved based on current mode.
    pub fn is_auto_approved(&self, tool_name: &str) -> bool {
        match self.mode {
            PermissionMode::Ask => false,
            PermissionMode::AutoRead => is_read_only_tool(tool_name),
            PermissionMode::AutoAll => true,
        }
    }

    pub fn mode(&self) -> &PermissionMode {
        &self.mode
    }

    pub fn set_mode(&mut self, mode: PermissionMode) {
        self.mode = mode;
    }
}

impl Default for PermissionEngine {
    fn default() -> Self {
        Self::new(PermissionMode::default())
    }
}

fn is_read_only_tool(name: &str) -> bool {
    matches!(name, "read" | "glob" | "grep")
}
