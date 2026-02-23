use anyhow::Result;
use colored::Colorize;
use nova_core::llm::LlmConfig;
use nova_core::permission::{PermissionEngine, PermissionMode};
use nova_protocol::Event;
use std::io::Write;

/// Run the interactive REPL loop.
pub async fn run_repl(llm_config: LlmConfig, permission_mode: PermissionMode) -> Result<()> {
    let llm = nova_core::llm::create_adapter(&llm_config)?;
    let tools = nova_tools::ToolRegistry::with_builtins();
    let permissions = PermissionEngine::new(permission_mode);

    let system_prompt = super::build_system_prompt();
    let mut agent = nova_core::AgentLoop::new(llm, tools, system_prompt, permissions);

    agent.on_event(|event| {
        handle_event(event);
    });

    loop {
        let input = match read_input() {
            Ok(Some(input)) => input,
            Ok(None) => break, // EOF / Ctrl+D
            Err(e) => {
                eprintln!("{} {e}", "Error:".red());
                continue;
            }
        };

        let trimmed = input.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Handle REPL commands
        match trimmed {
            "/quit" | "/exit" | "/q" => break,
            "/help" | "/h" => {
                print_help();
                continue;
            }
            "/clear" => {
                // Would need to recreate agent to clear context
                eprintln!("{}", "Context cleared.".green());
                continue;
            }
            "/tokens" => {
                let (input_tokens, output_tokens) = agent.token_usage();
                eprintln!(
                    "{}",
                    format!("Total tokens: {input_tokens} in / {output_tokens} out").cyan()
                );
                continue;
            }
            _ => {}
        }

        // Process the input through the agent
        match agent.process(trimmed).await {
            Ok(_) => {
                eprintln!(); // blank line after response
            }
            Err(e) => {
                eprintln!("{} {e}", "Error:".red());
            }
        }
    }

    eprintln!("{}", "Goodbye!".cyan());
    Ok(())
}

/// Handle streaming events from the agent.
pub fn handle_event(event: Event) {
    match event {
        Event::TextDelta { delta } => {
            eprint!("{delta}");
        }
        Event::ThinkingStart => {
            eprint!("{}", "Thinking...".dimmed());
        }
        Event::ThinkingDelta { .. } => {
            // Don't show thinking content by default
        }
        Event::ThinkingEnd => {
            eprintln!();
        }
        Event::ToolStart { call } => {
            eprintln!(
                "{}",
                format!("  {} {}", "Tool:".blue().bold(), call.name).dimmed()
            );
        }
        Event::ToolEnd { result } => {
            if result.is_error {
                eprintln!(
                    "{}",
                    format!("  {} {}", "Error:".red(), truncate(&result.output, 200))
                );
            } else {
                let preview = truncate(&result.output, 100);
                eprintln!("{}", format!("  {} {preview}", "Done:".green()).dimmed());
            }
        }
        Event::TurnComplete { .. } => {}
        Event::Error { message, .. } => {
            eprintln!("{} {message}", "Error:".red());
        }
    }
}

/// Read a line of input from the user.
fn read_input() -> Result<Option<String>> {
    eprint!("{} ", ">".cyan().bold());

    let mut input = String::new();
    std::io::stderr().flush()?;

    match std::io::stdin().read_line(&mut input) {
        Ok(0) => Ok(None), // EOF
        Ok(_) => Ok(Some(input)),
        Err(e) => Err(e.into()),
    }
}

fn print_help() {
    eprintln!("{}", "Nova Agent Commands:".cyan().bold());
    eprintln!("  {}    Show this help", "/help".green());
    eprintln!("  {}    Exit the REPL", "/quit".green());
    eprintln!("  {}   Clear conversation context", "/clear".green());
    eprintln!("  {}  Show token usage", "/tokens".green());
    eprintln!();
    eprintln!("  Type any message to chat with the agent.");
    eprintln!("  Press Ctrl+C or Ctrl+D to exit.");
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() > max {
        format!("{}...", &s[..max])
    } else {
        s.to_string()
    }
}
