use nova_protocol::Message;

/// Manages conversation context with compaction support.
pub struct ContextManager {
    system_prompt: String,
    messages: Vec<Message>,
    max_messages: usize,
}

impl ContextManager {
    pub fn new(system_prompt: String) -> Self {
        Self {
            system_prompt,
            messages: Vec::new(),
            max_messages: 200,
        }
    }

    pub fn system_prompt(&self) -> &str {
        &self.system_prompt
    }

    pub fn messages(&self) -> &[Message] {
        &self.messages
    }

    pub fn add_message(&mut self, message: Message) {
        self.messages.push(message);
    }

    pub fn message_count(&self) -> usize {
        self.messages.len()
    }

    /// Compact the context by summarizing older messages.
    /// Keeps the most recent messages and summarizes the rest.
    pub fn compact(&mut self, keep_recent: usize) {
        if self.messages.len() <= keep_recent {
            return;
        }

        let split_point = self.messages.len() - keep_recent;
        let old_messages = &self.messages[..split_point];

        // Build a summary of old messages
        let mut summary_parts = Vec::new();
        for msg in old_messages {
            let text = msg.text_content();
            if !text.is_empty() {
                let role = match msg.role {
                    nova_protocol::Role::User => "User",
                    nova_protocol::Role::Assistant => "Assistant",
                    nova_protocol::Role::System => "System",
                    nova_protocol::Role::Tool => "Tool",
                };
                // Take first 200 chars of each message for summary
                let truncated = if text.len() > 200 {
                    format!("{}...", &text[..200])
                } else {
                    text
                };
                summary_parts.push(format!("[{role}]: {truncated}"));
            }
        }

        let summary = format!(
            "[Context compacted: {} messages summarized]\n{}",
            old_messages.len(),
            summary_parts.join("\n")
        );

        let mut new_messages = vec![Message::user(summary)];
        new_messages.extend(self.messages[split_point..].to_vec());
        self.messages = new_messages;
    }

    /// Check if compaction is needed.
    pub fn needs_compaction(&self) -> bool {
        self.messages.len() > self.max_messages
    }

    /// Set the maximum number of messages before compaction triggers.
    pub fn set_max_messages(&mut self, max: usize) {
        self.max_messages = max;
    }

    /// Clear all messages.
    pub fn clear(&mut self) {
        self.messages.clear();
    }
}
