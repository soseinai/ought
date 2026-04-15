//! Provider-agnostic LLM client used by ought.
//!
//! The [`Llm`] trait defines a single async `complete` method that takes
//! a [`CompletionRequest`] (system prompt, conversation, available tools)
//! and returns a [`CompletionResponse`] (the model's reply, which may
//! include tool-use blocks the caller is expected to fulfil before
//! calling `complete` again).
//!
//! The trait knows nothing about the agent loop, retry policy, or any
//! ought-specific concept. Provider adapters in [`providers`] translate
//! between the unified types in [`types`] and each provider's wire
//! format.

pub mod error;
pub mod providers;
pub mod types;

pub use error::LlmError;
pub use types::{
    CacheHints, CompletionRequest, CompletionResponse, Content, Message, StopReason, ToolSpec,
    Usage,
};

pub use providers::anthropic::AnthropicLlm;
pub use providers::openai::OpenAiLlm;

use async_trait::async_trait;

/// A model-agnostic chat-completion client.
///
/// Implementations translate the unified [`CompletionRequest`] to the
/// provider's native wire format, perform the call, and translate the
/// response back to a [`CompletionResponse`]. They are responsible for
/// auth (typically reading an API key from an env var at construction
/// time) but **not** for retries — the caller (the agent loop) decides
/// when an error is retryable and waits accordingly.
#[async_trait]
pub trait Llm: Send + Sync {
    /// Short identifier for logs and error messages (e.g. `"anthropic"`).
    fn name(&self) -> &'static str;

    /// Send one request, await one response.
    async fn complete(&self, req: CompletionRequest) -> Result<CompletionResponse, LlmError>;
}
