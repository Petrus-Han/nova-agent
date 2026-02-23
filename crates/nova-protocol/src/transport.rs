use async_trait::async_trait;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use crate::error::{ProtocolError, Result};
use crate::message::{Request, Response};

/// Transport-agnostic trait for sending/receiving NACP messages.
#[async_trait]
pub trait Transport: Send + Sync {
    async fn send(&mut self, response: &Response) -> Result<()>;
    async fn recv(&mut self) -> Result<Request>;
}

/// Stdio-based transport using NDJSON (newline-delimited JSON).
pub struct StdioTransport {
    reader: BufReader<tokio::io::Stdin>,
    writer: tokio::io::Stdout,
}

impl StdioTransport {
    pub fn new() -> Self {
        Self {
            reader: BufReader::new(tokio::io::stdin()),
            writer: tokio::io::stdout(),
        }
    }
}

impl Default for StdioTransport {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Transport for StdioTransport {
    async fn send(&mut self, response: &Response) -> Result<()> {
        let json = serde_json::to_string(response)?;
        self.writer
            .write_all(json.as_bytes())
            .await
            .map_err(ProtocolError::Io)?;
        self.writer
            .write_all(b"\n")
            .await
            .map_err(ProtocolError::Io)?;
        self.writer.flush().await.map_err(ProtocolError::Io)?;
        Ok(())
    }

    async fn recv(&mut self) -> Result<Request> {
        let mut line = String::new();
        let bytes_read = self
            .reader
            .read_line(&mut line)
            .await
            .map_err(ProtocolError::Io)?;
        if bytes_read == 0 {
            return Err(ProtocolError::ConnectionClosed);
        }
        let request: Request = serde_json::from_str(line.trim())?;
        Ok(request)
    }
}
