mod config;
mod repl;

use anyhow::Result;
use clap::Parser;
use colored::Colorize;
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(name = "nova", version, about = "Nova Agent - High-Performance Coding Agent")]
struct Cli {
    /// Initial prompt to send (non-interactive mode).
    #[arg(short, long)]
    prompt: Option<String>,

    /// Model to use (default: claude-sonnet-4-20250514).
    #[arg(short, long, default_value = "claude-sonnet-4-20250514")]
    model: String,

    /// Permission mode: ask, auto-read, auto-all.
    #[arg(long, default_value = "auto-read")]
    permission: String,

    /// Working directory.
    #[arg(short = 'd', long)]
    workdir: Option<String>,

    /// Enable verbose logging.
    #[arg(short, long)]
    verbose: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize logging
    let filter = if cli.verbose {
        EnvFilter::new("debug")
    } else {
        EnvFilter::new("warn")
    };
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .init();

    // Set working directory if specified
    if let Some(ref workdir) = cli.workdir {
        std::env::set_current_dir(workdir)?;
    }

    // Load config
    let config = config::load_config()?;

    // Resolve API key
    let api_key = std::env::var("ANTHROPIC_API_KEY")
        .or_else(|_| {
            config
                .llm
                .as_ref()
                .and_then(|l| l.api_key.clone())
                .ok_or(std::env::VarError::NotPresent)
        })
        .map_err(|_| {
            anyhow::anyhow!(
                "ANTHROPIC_API_KEY not set. Set it in your environment or in ~/.nova/config.toml"
            )
        })?;

    // Build LLM config
    let llm_config = nova_core::LlmConfig {
        provider: nova_core::llm::LlmProvider::Anthropic,
        api_key,
        model: cli.model.clone(),
        max_tokens: 16384,
        temperature: 0.0,
    };

    // Parse permission mode
    let permission_mode = match cli.permission.as_str() {
        "ask" => nova_core::permission::PermissionMode::Ask,
        "auto-read" => nova_core::permission::PermissionMode::AutoRead,
        "auto-all" => nova_core::permission::PermissionMode::AutoAll,
        other => {
            eprintln!(
                "{} Unknown permission mode '{}', using auto-read",
                "Warning:".yellow(),
                other
            );
            nova_core::permission::PermissionMode::AutoRead
        }
    };

    // Print banner
    print_banner(&cli.model);

    if let Some(prompt) = cli.prompt {
        // Non-interactive mode: process single prompt and exit
        run_single(&llm_config, permission_mode, &prompt).await?;
    } else {
        // Interactive REPL mode
        repl::run_repl(llm_config, permission_mode).await?;
    }

    Ok(())
}

async fn run_single(
    llm_config: &nova_core::LlmConfig,
    permission_mode: nova_core::permission::PermissionMode,
    prompt: &str,
) -> Result<()> {
    let llm = nova_core::llm::create_adapter(llm_config)?;
    let tools = nova_tools::ToolRegistry::with_builtins();
    let permissions = nova_core::permission::PermissionEngine::new(permission_mode);

    let system_prompt = build_system_prompt();
    let mut agent = nova_core::AgentLoop::new(llm, tools, system_prompt, permissions);

    // Set up event handler for streaming output
    agent.on_event(|event| {
        repl::handle_event(event);
    });

    let _result = agent.process(prompt).await?;

    let (input_tokens, output_tokens) = agent.token_usage();
    eprintln!(
        "\n{}",
        format!("Tokens: {input_tokens} in / {output_tokens} out").dimmed()
    );

    Ok(())
}

fn build_system_prompt() -> String {
    let cwd = std::env::current_dir()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| ".".to_string());

    format!(
        r#"You are Nova Agent, a high-performance coding assistant.

You help users with software engineering tasks: writing code, debugging, refactoring, and more.

Working directory: {cwd}
Platform: {os}

Guidelines:
- Read files before modifying them
- Use the appropriate tool for each task
- Be concise and focused
- Write clean, idiomatic code
- Prefer editing existing files over creating new ones"#,
        os = std::env::consts::OS
    )
}

fn print_banner(model: &str) {
    let version = env!("CARGO_PKG_VERSION");
    eprintln!(
        "{}",
        format!("Nova Agent v{version} ({model})").cyan().bold()
    );
    eprintln!(
        "{}",
        "Type your request, or /help for commands. Ctrl+C to exit."
            .dimmed()
    );
    eprintln!();
}
