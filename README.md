# Nova Agent

A high-performance coding agent built in Rust. Designed to be fast, lightweight, and extensible.

## Features

- **Blazing fast**: 4.4MB static binary, <100ms startup, minimal memory footprint
- **6 built-in tools**: read (mmap), write, edit (ropey), glob, grep (ripgrep), bash (async)
- **Protocol-driven**: Clean NACP (Nova Agent Communication Protocol) layer with NDJSON transport
- **LLM integration**: Anthropic Claude API adapter (extensible to OpenAI and others)
- **Interactive REPL**: Colored terminal interface with streaming output
- **Permission system**: Three modes — ask, auto-read, auto-all
- **Context management**: Automatic compaction for long conversations
- **Sandbox support**: Pluggable sandbox abstraction (none, process, landlock, docker)

## Quick Start

```bash
# Build
cargo build --release

# Set your API key
export ANTHROPIC_API_KEY=sk-ant-...

# Interactive mode
./target/release/nova

# Single prompt
nova --prompt "Read main.rs and fix the bug on line 42"

# With options
nova --model claude-sonnet-4-20250514 --permission auto-all --workdir /my/project
```

## Architecture

```
┌─────────────────────────────────────────────────┐
│              CLI / REPL (nova-cli)               │
├─────────────────────────────────────────────────┤
│         NACP Protocol (nova-protocol)            │
│      Request/Response │ Events │ Transport       │
├─────────────────────────────────────────────────┤
│            Core Engine (nova-core)               │
│   Agent Loop │ Context Manager │ Permissions     │
├─────────────────────────────────────────────────┤
│          Tools (nova-tools) │ Sandbox            │
│  read│write│edit│glob│grep│bash │ (nova-sandbox) │
├─────────────────────────────────────────────────┤
│          LLM Adapter (nova-core::llm)            │
│              Anthropic │ OpenAI                   │
└─────────────────────────────────────────────────┘
```

### Crates

| Crate | Purpose |
|-------|---------|
| `nova-protocol` | Message types, tool definitions, NDJSON transport |
| `nova-core` | Agent loop, LLM adapters, context management, permissions |
| `nova-tools` | Built-in tool implementations (read, write, edit, glob, grep, bash) |
| `nova-cli` | CLI binary with interactive REPL |
| `nova-sandbox` | Sandbox abstraction layer |

## CLI Options

```
nova [OPTIONS]

Options:
  -p, --prompt <PROMPT>          Run a single prompt (non-interactive)
  -m, --model <MODEL>            LLM model [default: claude-sonnet-4-20250514]
      --permission <MODE>        ask | auto-read | auto-all [default: auto-read]
  -d, --workdir <DIR>            Working directory
  -v, --verbose                  Enable debug logging
  -h, --help                     Print help
  -V, --version                  Print version
```

## REPL Commands

| Command | Description |
|---------|-------------|
| `/help` | Show available commands |
| `/quit` | Exit the REPL |
| `/clear` | Clear conversation context |
| `/tokens` | Show token usage statistics |

## Configuration

Optional config file at `~/.nova/config.toml`:

```toml
[llm]
provider = "anthropic"
api_key = "sk-ant-..."
model = "claude-sonnet-4-20250514"

[performance]
threads = 0        # 0 = auto-detect
cache_size = "1GB"
```

## Testing

```bash
cargo test          # Run all 29 tests
cargo test -p nova-cli -- --nocapture  # With output
```

## Performance

| Metric | Nova Agent | Typical Python agents |
|--------|-----------|----------------------|
| Binary size | 4.4 MB | 100+ MB (with deps) |
| Startup time | <100ms | 3-10s |
| Memory idle | ~5 MB | 200-500 MB |
| Tool call latency | <10ms | 50-100ms |

## License

MIT
