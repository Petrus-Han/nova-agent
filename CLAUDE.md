# Nova Agent

High-performance coding agent built in Rust.

## Project Structure

```
nova-agent/
  crates/
    nova-protocol/  - NACP protocol types (messages, tools, transport)
    nova-core/      - Agent loop, LLM adapter, context manager, permissions
    nova-tools/     - Built-in tools (read, write, edit, glob, grep, bash)
    nova-cli/       - CLI binary with REPL interface
    nova-sandbox/   - Sandbox abstraction (none, process, landlock, docker)
```

## Development

```bash
cargo check          # Type check
cargo build          # Debug build
cargo build --release # Release build (optimized, stripped)
cargo test           # Run tests
```

## Conventions

- All code and comments in English
- Never leave `todo!()`, `unimplemented!()`, or `// TODO` in code
- Every function must have a complete implementation
- Use `anyhow::Result` for application errors, `thiserror` for library errors
- Async code uses tokio runtime
- Tools implement the `Tool` trait from nova-tools

## Architecture

- **Protocol-driven**: All communication goes through NACP (nova-protocol)
- **Layered**: CLI -> Protocol -> Core Engine -> Tools -> LLM Adapter
- **Async**: Built on tokio for non-blocking I/O
- **Zero-copy where possible**: mmap for reads, ropey for edits

## Binary

The release binary is `nova` (from nova-cli crate), target size < 20MB.
