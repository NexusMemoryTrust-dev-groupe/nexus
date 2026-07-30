/// Embedded system rules for the AI copilot.
/// These rules are injected into EVERY AI request.
const SYSTEM_RULES: &str = include_str!("../../system_rules.md");

/// Max tokens for the full prompt (~4K tokens ≈ 16K chars).
/// Prevents context explosion for long conversations.
const MAX_PROMPT_CHARS: usize = 16000;

/// Max conversation messages to include (recent only).
const MAX_CONVERSATION_MESSAGES: usize = 20;

/// Get the system rules prompt. Loaded once, cached.
pub fn get_system_rules() -> &'static str {
    SYSTEM_RULES
}

/// Build the complete system prompt with rules + conversation context.
/// Truncates conversation history to prevent token explosion.
pub fn build_full_prompt(conversation: &[super::ai::ChatMessage]) -> String {
    let rules = get_system_rules();
    let mut parts = vec![rules.to_string()];

    // Take only the most recent N messages to prevent context explosion
    let start = conversation.len().saturating_sub(MAX_CONVERSATION_MESSAGES);
    let recent_messages = &conversation[start..];

    for msg in recent_messages {
        let label = if msg.role == "user" { "User" } else { "Assistant" };
        // Truncate individual messages to 500 chars each
        let content = if msg.content.len() > 500 {
            format!("{}...", &msg.content[..500])
        } else {
            msg.content.clone()
        };
        parts.push(format!("{}: {}", label, content));
    }

    let full = parts.join("\n\n");
    
    // Final safety: truncate entire prompt if still too long
    if full.len() > MAX_PROMPT_CHARS {
        format!("{}\n\n[Conversation truncated — {} chars total]", &full[..MAX_PROMPT_CHARS], full.len())
    } else {
        full
    }
}
