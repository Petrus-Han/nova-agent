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

    /// LLM provider: anthropic, openai, zhipu, deepseek, custom.
    /// Auto-detected from environment if not specified.
    #[arg(long)]
    provider: Option<String>,

    /// Model to use. Defaults depend on provider.
    #[arg(short, long)]
    model: Option<String>,

    /// Custom API base URL (for self-hosted or custom endpoints).
    #[arg(long)]
    base_url: Option<String>,

    /// API key. Can also be set via ANTHROPIC_API_KEY, OPENAI_API_KEY,
    /// ZHIPU_API_KEY, or DEEPSEEK_API_KEY environment variables.
    #[arg(long)]
    api_key: Option<String>,

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
    let file_config = config::load_config()?;

    // Resolve provider and API key
    let (provider, api_key) = resolve_provider_and_key(&cli, &file_config)?;

    // Resolve model
    let model = cli
        .model
        .unwrap_or_else(|| nova_core::llm::default_model(&provider).to_string());

    // Build LLM config
    let llm_config = nova_core::LlmConfig {
        provider: provider.clone(),
        api_key,
        model: model.clone(),
        base_url: cli.base_url.clone(),
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
    let provider_label = format!("{:?}", provider).to_lowercase();
    print_banner(&model, &provider_label);

    if let Some(prompt) = cli.prompt {
        // Non-interactive mode: process single prompt and exit
        run_single(&llm_config, permission_mode, &prompt).await?;
    } else {
        // Interactive REPL mode
        repl::run_repl(llm_config, permission_mode).await?;
    }

    Ok(())
}

fn resolve_provider_and_key(
    cli: &Cli,
    file_config: &config::Config,
) -> Result<(nova_core::llm::LlmProvider, String)> {
    // 1. Explicit --api-key flag
    if let Some(ref key) = cli.api_key {
        let provider = parse_provider(cli.provider.as_deref().unwrap_or("anthropic"));
        return Ok((provider, key.clone()));
    }

    // 2. Explicit --provider flag -> look for corresponding env var
    if let Some(ref provider_str) = cli.provider {
        let provider = parse_provider(provider_str);
        let env_var = match provider {
            nova_core::llm::LlmProvider::Anthropic => "ANTHROPIC_API_KEY",
            nova_core::llm::LlmProvider::OpenAI => "OPENAI_API_KEY",
            nova_core::llm::LlmProvider::Zhipu => "ZHIPU_API_KEY",
            nova_core::llm::LlmProvider::DeepSeek => "DEEPSEEK_API_KEY",
            nova_core::llm::LlmProvider::Custom => "API_KEY",
        };
        if let Ok(key) = std::env::var(env_var) {
            return Ok((provider, key));
        }
        // Also try config file
        if let Some(ref llm) = file_config.llm {
            if let Some(ref key) = llm.api_key {
                return Ok((provider, key.clone()));
            }
        }
        anyhow::bail!(
            "{env_var} not set. Set it in your environment, use --api-key, or add it to ~/.nova/config.toml"
        );
    }

    // 3. Auto-detect from environment
    if let Some((provider, key)) = nova_core::llm::detect_from_env() {
        return Ok((provider, key));
    }

    // 4. Config file
    if let Some(ref llm) = file_config.llm {
        if let Some(ref key) = llm.api_key {
            let provider = llm
                .provider
                .as_deref()
                .map(parse_provider)
                .unwrap_or(nova_core::llm::LlmProvider::Anthropic);
            return Ok((provider, key.clone()));
        }
    }

    anyhow::bail!(
        "No API key found. Set one of: ANTHROPIC_API_KEY, OPENAI_API_KEY, ZHIPU_API_KEY, DEEPSEEK_API_KEY\n\
         Or use --api-key flag, or add to ~/.nova/config.toml"
    );
}

fn parse_provider(s: &str) -> nova_core::llm::LlmProvider {
    match s.to_lowercase().as_str() {
        "anthropic" | "claude" => nova_core::llm::LlmProvider::Anthropic,
        "openai" | "gpt" => nova_core::llm::LlmProvider::OpenAI,
        "zhipu" | "glm" | "chatglm" => nova_core::llm::LlmProvider::Zhipu,
        "deepseek" => nova_core::llm::LlmProvider::DeepSeek,
        _ => nova_core::llm::LlmProvider::Custom,
    }
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

fn print_banner(model: &str, provider: &str) {
    let version = env!("CARGO_PKG_VERSION");
    eprintln!(
        "{}",
        format!("Nova Agent v{version} ({provider}/{model})").cyan().bold()
    );
    eprintln!(
        "{}",
        "Type your request, or /help for commands. Ctrl+C to exit."
            .dimmed()
    );
    eprintln!();
}
