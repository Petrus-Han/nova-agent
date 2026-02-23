use nova_protocol::*;

// ============================================================
// Protocol tests
// ============================================================

#[test]
fn test_message_user() {
    let msg = Message::user("Hello");
    assert_eq!(msg.role, Role::User);
    assert_eq!(msg.text_content(), "Hello");
}

#[test]
fn test_message_assistant() {
    let msg = Message::assistant("Hi there");
    assert_eq!(msg.role, Role::Assistant);
    assert_eq!(msg.text_content(), "Hi there");
}

#[test]
fn test_message_system() {
    let msg = Message::system("You are an agent");
    assert_eq!(msg.role, Role::System);
    assert_eq!(msg.text_content(), "You are an agent");
}

#[test]
fn test_message_tool_result() {
    let msg = Message::tool_result("tool_123", "file contents here", false);
    assert_eq!(msg.role, Role::Tool);
    match &msg.content[0] {
        ContentBlock::ToolResult {
            tool_use_id,
            content,
            is_error,
        } => {
            assert_eq!(tool_use_id, "tool_123");
            assert_eq!(content, "file contents here");
            assert!(!is_error);
        }
        _ => panic!("Expected ToolResult content block"),
    }
}

#[test]
fn test_request_serialization() {
    let req = Request::UserInput {
        id: uuid::Uuid::nil(),
        content: "test".to_string(),
        attachments: vec![],
    };
    let json = serde_json::to_string(&req).unwrap();
    assert!(json.contains("user_input"));
    assert!(json.contains("test"));

    // Round-trip
    let parsed: Request = serde_json::from_str(&json).unwrap();
    match parsed {
        Request::UserInput { content, .. } => assert_eq!(content, "test"),
        _ => panic!("Expected UserInput"),
    }
}

#[test]
fn test_response_serialization() {
    let resp = Response::Text {
        content: "Hello!".to_string(),
    };
    let json = serde_json::to_string(&resp).unwrap();
    assert!(json.contains("text"));
    assert!(json.contains("Hello!"));
}

#[test]
fn test_event_serialization() {
    let event = Event::TextDelta {
        delta: "tok".to_string(),
    };
    let json = serde_json::to_string(&event).unwrap();
    assert!(json.contains("text_delta"));
    assert!(json.contains("tok"));
}

#[test]
fn test_tool_definition_schema() {
    let def = ToolDefinition {
        name: "read".to_string(),
        description: "Read a file".to_string(),
        parameters: vec![ToolParameter {
            name: "file_path".to_string(),
            description: "Path to file".to_string(),
            param_type: ParameterType::String,
            required: true,
        }],
    };

    let schema = def.to_api_schema();
    assert_eq!(schema["name"], "read");
    assert_eq!(schema["input_schema"]["properties"]["file_path"]["type"], "string");
    assert_eq!(schema["input_schema"]["required"][0], "file_path");
}

// ============================================================
// Tool registry tests
// ============================================================

#[test]
fn test_tool_registry_builtins() {
    let registry = nova_tools::ToolRegistry::with_builtins();
    assert_eq!(registry.len(), 6);
    assert!(registry.get("read").is_some());
    assert!(registry.get("write").is_some());
    assert!(registry.get("edit").is_some());
    assert!(registry.get("glob").is_some());
    assert!(registry.get("grep").is_some());
    assert!(registry.get("bash").is_some());
    assert!(registry.get("nonexistent").is_none());
}

#[test]
fn test_tool_definitions() {
    let registry = nova_tools::ToolRegistry::with_builtins();
    let defs = registry.definitions();
    assert_eq!(defs.len(), 6);
    let names: Vec<&str> = defs.iter().map(|d| d.name.as_str()).collect();
    assert!(names.contains(&"read"));
    assert!(names.contains(&"bash"));
}

// ============================================================
// Tool execution tests
// ============================================================

#[tokio::test]
async fn test_read_tool_existing_file() {
    use nova_tools::Tool;
    let tool = nova_tools::read::ReadTool;

    // Read Cargo.toml which we know exists
    let result = tool
        .execute(
            "test_1",
            serde_json::json!({
                "file_path": concat!(env!("CARGO_MANIFEST_DIR"), "/../../Cargo.toml")
            }),
        )
        .await;

    assert!(!result.is_error, "Read should succeed: {}", result.output);
    assert!(result.output.contains("[workspace]"));
}

#[tokio::test]
async fn test_read_tool_nonexistent_file() {
    use nova_tools::Tool;
    let tool = nova_tools::read::ReadTool;

    let result = tool
        .execute(
            "test_2",
            serde_json::json!({
                "file_path": "/tmp/definitely_does_not_exist_nova_test.txt"
            }),
        )
        .await;

    assert!(result.is_error);
    assert!(result.output.contains("not found") || result.output.contains("Error"));
}

#[tokio::test]
async fn test_write_and_read_tool() {
    use nova_tools::Tool;
    let write_tool = nova_tools::write::WriteTool;
    let read_tool = nova_tools::read::ReadTool;

    let test_path = "/tmp/nova_test_write.txt";
    let test_content = "Hello from Nova Agent!\nLine 2\nLine 3";

    // Write
    let result = write_tool
        .execute(
            "w1",
            serde_json::json!({
                "file_path": test_path,
                "content": test_content
            }),
        )
        .await;
    assert!(!result.is_error, "Write failed: {}", result.output);

    // Read back
    let result = read_tool
        .execute("r1", serde_json::json!({"file_path": test_path}))
        .await;
    assert!(!result.is_error, "Read failed: {}", result.output);
    assert!(result.output.contains("Hello from Nova Agent!"));
    assert!(result.output.contains("Line 3"));

    // Cleanup
    std::fs::remove_file(test_path).ok();
}

#[tokio::test]
async fn test_edit_tool() {
    use nova_tools::Tool;
    let write_tool = nova_tools::write::WriteTool;
    let edit_tool = nova_tools::edit::EditTool;
    let read_tool = nova_tools::read::ReadTool;

    let test_path = "/tmp/nova_test_edit.txt";

    // Create file
    write_tool
        .execute(
            "w1",
            serde_json::json!({
                "file_path": test_path,
                "content": "fn main() {\n    println!(\"old\");\n}\n"
            }),
        )
        .await;

    // Edit
    let result = edit_tool
        .execute(
            "e1",
            serde_json::json!({
                "file_path": test_path,
                "old_string": "println!(\"old\")",
                "new_string": "println!(\"new\")"
            }),
        )
        .await;
    assert!(!result.is_error, "Edit failed: {}", result.output);

    // Verify
    let result = read_tool
        .execute("r1", serde_json::json!({"file_path": test_path}))
        .await;
    assert!(result.output.contains("println!(\"new\")"));
    assert!(!result.output.contains("println!(\"old\")"));

    std::fs::remove_file(test_path).ok();
}

#[tokio::test]
async fn test_glob_tool() {
    use nova_tools::Tool;
    let tool = nova_tools::glob::GlobTool;

    let result = tool
        .execute(
            "g1",
            serde_json::json!({
                "pattern": "**/*.rs",
                "path": concat!(env!("CARGO_MANIFEST_DIR"), "/../nova-protocol")
            }),
        )
        .await;

    assert!(!result.is_error, "Glob failed: {}", result.output);
    assert!(result.output.contains("lib.rs"));
    assert!(result.output.contains("message.rs"));
}

#[tokio::test]
async fn test_grep_tool() {
    use nova_tools::Tool;
    let tool = nova_tools::grep::GrepTool;

    let result = tool
        .execute(
            "g1",
            serde_json::json!({
                "pattern": "pub struct ToolDefinition",
                "path": concat!(env!("CARGO_MANIFEST_DIR"), "/../nova-protocol")
            }),
        )
        .await;

    assert!(!result.is_error, "Grep failed: {}", result.output);
    assert!(result.output.contains("ToolDefinition"));
}

#[tokio::test]
async fn test_bash_tool() {
    use nova_tools::Tool;
    let tool = nova_tools::bash::BashTool;

    let result = tool
        .execute(
            "b1",
            serde_json::json!({
                "command": "echo 'nova test' && echo '42'"
            }),
        )
        .await;

    assert!(!result.is_error, "Bash failed: {}", result.output);
    assert!(result.output.contains("nova test"));
    assert!(result.output.contains("42"));
}

#[tokio::test]
async fn test_bash_tool_failure() {
    use nova_tools::Tool;
    let tool = nova_tools::bash::BashTool;

    let result = tool
        .execute(
            "b2",
            serde_json::json!({
                "command": "exit 1"
            }),
        )
        .await;

    assert!(result.is_error);
}

#[tokio::test]
async fn test_bash_tool_timeout() {
    use nova_tools::Tool;
    let tool = nova_tools::bash::BashTool;

    let result = tool
        .execute(
            "b3",
            serde_json::json!({
                "command": "sleep 10",
                "timeout": 500
            }),
        )
        .await;

    assert!(result.is_error);
    assert!(result.output.contains("timed out"));
}

// ============================================================
// Context manager tests
// ============================================================

#[test]
fn test_context_manager_basic() {
    let mut ctx = nova_core::context::ContextManager::new("System prompt".to_string());
    assert_eq!(ctx.system_prompt(), "System prompt");
    assert_eq!(ctx.message_count(), 0);

    ctx.add_message(Message::user("Hello"));
    assert_eq!(ctx.message_count(), 1);

    ctx.add_message(Message::assistant("Hi"));
    assert_eq!(ctx.message_count(), 2);
}

#[test]
fn test_context_compaction() {
    let mut ctx = nova_core::context::ContextManager::new("System".to_string());
    ctx.set_max_messages(5);

    for i in 0..10 {
        ctx.add_message(Message::user(format!("Message {i}")));
    }

    assert_eq!(ctx.message_count(), 10);
    assert!(ctx.needs_compaction());

    ctx.compact(3);

    // After compaction: 1 summary + 3 recent = 4
    assert_eq!(ctx.message_count(), 4);
    assert!(!ctx.needs_compaction());
}

// ============================================================
// Permission engine tests
// ============================================================

#[test]
fn test_permission_ask_mode() {
    let engine = nova_core::permission::PermissionEngine::new(
        nova_core::permission::PermissionMode::Ask,
    );
    assert!(!engine.is_auto_approved("read"));
    assert!(!engine.is_auto_approved("write"));
    assert!(!engine.is_auto_approved("bash"));
}

#[test]
fn test_permission_auto_read_mode() {
    let engine = nova_core::permission::PermissionEngine::new(
        nova_core::permission::PermissionMode::AutoRead,
    );
    assert!(engine.is_auto_approved("read"));
    assert!(engine.is_auto_approved("glob"));
    assert!(engine.is_auto_approved("grep"));
    assert!(!engine.is_auto_approved("write"));
    assert!(!engine.is_auto_approved("edit"));
    assert!(!engine.is_auto_approved("bash"));
}

#[test]
fn test_permission_auto_all_mode() {
    let engine = nova_core::permission::PermissionEngine::new(
        nova_core::permission::PermissionMode::AutoAll,
    );
    assert!(engine.is_auto_approved("read"));
    assert!(engine.is_auto_approved("write"));
    assert!(engine.is_auto_approved("bash"));
    assert!(engine.is_auto_approved("anything"));
}

// ============================================================
// CLI binary tests
// ============================================================

#[test]
fn test_cli_help() {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_nova"))
        .arg("--help")
        .output()
        .expect("Failed to run nova binary");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Nova Agent"));
    assert!(stdout.contains("--prompt"));
    assert!(stdout.contains("--model"));
}

#[test]
fn test_cli_version() {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_nova"))
        .arg("--version")
        .output()
        .expect("Failed to run nova binary");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("nova 0.1.0"));
}

#[test]
fn test_cli_missing_api_key() {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_nova"))
        .arg("--prompt")
        .arg("test")
        .env_remove("ANTHROPIC_API_KEY")
        .output()
        .expect("Failed to run nova binary");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("ANTHROPIC_API_KEY"));
}

// ============================================================
// Sandbox tests
// ============================================================

#[tokio::test]
async fn test_no_sandbox_execute() {
    use nova_sandbox::Sandbox;
    let sandbox = nova_sandbox::NoSandbox;

    let output = sandbox.execute("echo 'sandbox test'").await.unwrap();
    assert_eq!(output.exit_code, 0);
    assert!(output.stdout.contains("sandbox test"));
}

#[test]
fn test_sandbox_creation() {
    let sandbox = nova_sandbox::create_sandbox(nova_sandbox::SandboxMode::None);
    assert!(sandbox.is_ok());

    let sandbox = nova_sandbox::create_sandbox(nova_sandbox::SandboxMode::Docker);
    assert!(sandbox.is_err()); // Not implemented yet
}
