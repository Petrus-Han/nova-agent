use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Sandbox mode selection.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SandboxMode {
    /// No sandboxing (trust mode).
    None,
    /// Process-level isolation.
    Process,
    /// Linux Landlock (Linux only).
    Landlock,
    /// macOS Seatbelt (macOS only).
    Seatbelt,
    /// Docker container isolation.
    Docker,
}

impl Default for SandboxMode {
    fn default() -> Self {
        Self::None
    }
}

#[derive(Debug, Error)]
pub enum SandboxError {
    #[error("sandbox not available on this platform: {0}")]
    NotAvailable(String),
    #[error("sandbox execution error: {0}")]
    ExecutionError(String),
    #[error("permission denied: {0}")]
    PermissionDenied(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

/// Path access rule for sandbox restrictions.
#[derive(Debug, Clone)]
pub struct PathRule {
    pub path: String,
    pub read: bool,
    pub write: bool,
    pub execute: bool,
}

/// Output from a sandboxed command execution.
#[derive(Debug, Clone)]
pub struct SandboxOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

/// Trait for sandbox implementations.
#[async_trait]
pub trait Sandbox: Send + Sync {
    /// Execute a command within the sandbox.
    async fn execute(&self, command: &str) -> Result<SandboxOutput, SandboxError>;

    /// Restrict file system access.
    fn restrict_paths(&mut self, rules: &[PathRule]);

    /// Restrict network access.
    fn restrict_network(&mut self, allow: bool);

    /// Get the sandbox mode.
    fn mode(&self) -> SandboxMode;
}

/// No-op sandbox for trust mode.
pub struct NoSandbox;

#[async_trait]
impl Sandbox for NoSandbox {
    async fn execute(&self, command: &str) -> Result<SandboxOutput, SandboxError> {
        let output = tokio::process::Command::new("bash")
            .arg("-c")
            .arg(command)
            .output()
            .await?;

        Ok(SandboxOutput {
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            exit_code: output.status.code().unwrap_or(-1),
        })
    }

    fn restrict_paths(&mut self, _rules: &[PathRule]) {
        // No-op in trust mode
    }

    fn restrict_network(&mut self, _allow: bool) {
        // No-op in trust mode
    }

    fn mode(&self) -> SandboxMode {
        SandboxMode::None
    }
}

/// Create a sandbox based on the requested mode.
pub fn create_sandbox(mode: SandboxMode) -> Result<Box<dyn Sandbox>, SandboxError> {
    match mode {
        SandboxMode::None => Ok(Box::new(NoSandbox)),
        SandboxMode::Process => Ok(Box::new(NoSandbox)), // Placeholder: process isolation
        other => Err(SandboxError::NotAvailable(format!(
            "{other:?} sandbox not yet implemented"
        ))),
    }
}
