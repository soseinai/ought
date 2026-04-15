//! Provider-agnostic request and response types.
//!
//! Every provider adapter accepts these types and produces these types;
//! the wire-format differences (Anthropic's `tool_use`/`tool_result`
//! blocks vs OpenAI's `tool_calls` array, etc.) live entirely inside the
//! adapters.

use serde::{Deserialize, Serialize};

/// One request to the model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionRequest {
    /// Provider-specific model identifier (e.g. `"claude-sonnet-4-6"`).
    pub model: String,

    /// System prompt. Always sent as the system role; never appears in
    /// the message list.
    pub system: String,

    /// Conversation history in order, oldest first.
    pub messages: Vec<Message>,

    /// Tool definitions exposed to the model. Empty if no tools.
    pub tools: Vec<ToolSpec>,

    /// Maximum tokens the model may emit in this turn.
    pub max_tokens: u32,

    /// Optional sampling temperature (0.0–2.0 typically).
    pub temperature: Option<f32>,

    /// Hints for prompt caching. Providers that don't support caching
    /// (OpenAI / Ollama) ignore this.
    #[serde(default)]
    pub cache_hints: CacheHints,
}

/// Hints for which parts of the request the provider should cache.
///
/// Anthropic uses these to insert `cache_control` markers on the system
/// block, the last tool, and/or a specific message in the history.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CacheHints {
    /// Cache the system prompt.
    #[serde(default)]
    pub cache_system: bool,
    /// Cache the tool definitions.
    #[serde(default)]
    pub cache_tools: bool,
    /// Cache messages up to and including this index (0-based).
    #[serde(default)]
    pub cache_messages_up_to: Option<usize>,
}

/// One turn in the conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "role", rename_all = "snake_case")]
pub enum Message {
    User { content: Vec<Content> },
    Assistant { content: Vec<Content> },
}

impl Message {
    pub fn user_text(text: impl Into<String>) -> Self {
        Message::User {
            content: vec![Content::Text(text.into())],
        }
    }

    pub fn assistant_text(text: impl Into<String>) -> Self {
        Message::Assistant {
            content: vec![Content::Text(text.into())],
        }
    }
}

/// One content block within a [`Message`].
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Content {
    /// Plain text.
    Text(String),
    /// The model is requesting a tool invocation.
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    /// The result of a previously-requested tool invocation, sent back
    /// to the model. The `content` is the stringified result; structured
    /// outputs are typically JSON-encoded.
    ToolResult {
        tool_use_id: String,
        content: String,
        #[serde(default)]
        is_error: bool,
    },
}

/// Definition of a tool the model may invoke.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    /// JSON Schema for the tool's `input` parameter.
    pub input_schema: serde_json::Value,
}

/// The model's reply to one [`CompletionRequest`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionResponse {
    /// Content blocks emitted by the model. Typically a single `Text`,
    /// or any number of `ToolUse` blocks (possibly interleaved with
    /// explanatory `Text`).
    pub content: Vec<Content>,
    pub stop_reason: StopReason,
    pub usage: Usage,
}

/// Why the model stopped emitting tokens.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    /// Model finished its turn naturally.
    EndTurn,
    /// Model wants the caller to invoke one or more tools.
    ToolUse,
    /// Model hit `max_tokens`.
    MaxTokens,
    /// Model emitted a configured stop sequence.
    StopSequence,
    /// Anything else (provider-specific). The string is provider-supplied
    /// and meant for logging, not control flow.
    Other(String),
}

/// Token usage for one completion.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct Usage {
    pub input_tokens: u32,
    pub output_tokens: u32,
    /// Tokens read from cache (Anthropic). 0 if not supported / no hit.
    #[serde(default)]
    pub cache_read_tokens: u32,
    /// Tokens written to cache (Anthropic). 0 if not supported.
    #[serde(default)]
    pub cache_creation_tokens: u32,
}

impl std::ops::AddAssign for Usage {
    fn add_assign(&mut self, rhs: Self) {
        self.input_tokens += rhs.input_tokens;
        self.output_tokens += rhs.output_tokens;
        self.cache_read_tokens += rhs.cache_read_tokens;
        self.cache_creation_tokens += rhs.cache_creation_tokens;
    }
}
