//! Provider-agnostic agent loop.
//!
//! [`Agent::run`] takes a system prompt, an initial user message, and a
//! [`ToolSet`]. It calls the underlying [`ought_llm::Llm`] in a loop,
//! dispatching `tool_use` blocks to the [`ToolSet`] and feeding results
//! back to the model until the model emits an `end_turn` or a
//! configured limit (max turns) is reached.
//!
//! The loop knows nothing about ought-specific tools — those live in
//! `ought_gen::tools` and are wrapped in a [`ToolSet`] adapter by the
//! orchestrator.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use serde_json::Value;
use thiserror::Error;

use ought_llm::{
    CacheHints, CompletionRequest, Content, Llm, LlmError, Message, StopReason, ToolSpec, Usage,
};

// ── Public types ────────────────────────────────────────────────────────

/// A bundle of tools the agent may invoke.
///
/// Implementations describe the tools statically via [`Self::specs`] and
/// dispatch calls dynamically by tool name. The trait is async so
/// adapters can do real I/O without blocking the executor.
#[async_trait]
pub trait ToolSet: Send + Sync {
    /// Tool definitions sent to the model in every request.
    fn specs(&self) -> &[ToolSpec];

    /// Execute one tool by name with the given JSON input. The returned
    /// outcome is fed back to the model as a `tool_result` block.
    async fn execute(&self, name: &str, input: Value) -> ToolOutcome;
}

/// One tool execution result.
#[derive(Debug, Clone)]
pub struct ToolOutcome {
    /// Stringified result. Structured outputs should be JSON-encoded.
    pub content: String,
    /// True if the tool failed; the model treats this as an error and
    /// typically retries with different inputs.
    pub is_error: bool,
}

impl ToolOutcome {
    pub fn ok(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            is_error: false,
        }
    }

    pub fn err(message: impl Into<String>) -> Self {
        Self {
            content: message.into(),
            is_error: true,
        }
    }
}

/// Knobs for the agent loop.
#[derive(Debug, Clone)]
pub struct AgentConfig {
    /// Provider-specific model id.
    pub model: String,
    /// Maximum number of model turns before giving up.
    pub max_turns: u32,
    /// Token cap per individual model response.
    pub max_tokens_per_response: u32,
    /// Optional sampling temperature.
    pub temperature: Option<f32>,
    /// Maximum number of retries on retryable [`LlmError`]s.
    pub max_retries: u32,
    /// Initial backoff between retries; doubled each attempt up to
    /// [`Self::max_backoff`].
    pub initial_backoff: Duration,
    pub max_backoff: Duration,
    /// Cache hints sent on every request.
    pub cache_hints: CacheHints,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            model: String::new(),
            max_turns: 50,
            max_tokens_per_response: 8192,
            temperature: None,
            max_retries: 5,
            initial_backoff: Duration::from_secs(1),
            max_backoff: Duration::from_secs(60),
            cache_hints: CacheHints {
                cache_system: true,
                cache_tools: true,
                cache_messages_up_to: None,
            },
        }
    }
}

/// Result of [`Agent::run`].
#[derive(Debug)]
pub struct RunOutcome {
    pub status: RunStatus,
    /// Final assistant text (the last `Content::Text` block in the last
    /// response), if any.
    pub final_text: Option<String>,
    /// Number of model turns consumed.
    pub turns: u32,
    /// Token usage summed across all turns.
    pub usage: Usage,
}

/// Why the loop terminated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunStatus {
    /// Model emitted `end_turn`.
    Completed,
    /// Hit `max_turns` without the model finishing.
    MaxTurnsExceeded,
    /// `max_tokens` stop reason from the model on the final turn — model
    /// ran out of room mid-response.
    Truncated,
}

/// Agent-loop errors.
#[derive(Debug, Error)]
pub enum AgentError {
    #[error("LLM error after {attempts} attempt(s): {source}")]
    Llm {
        attempts: u32,
        #[source]
        source: LlmError,
    },
}

// ── Agent ───────────────────────────────────────────────────────────────

pub struct Agent {
    llm: Arc<dyn Llm>,
    config: AgentConfig,
}

impl Agent {
    pub fn new(llm: Arc<dyn Llm>, config: AgentConfig) -> Self {
        Self { llm, config }
    }

    /// Run the agent loop to completion (or limit).
    pub async fn run<T: ToolSet + ?Sized>(
        &self,
        system: String,
        initial_user: String,
        tools: &T,
    ) -> Result<RunOutcome, AgentError> {
        let mut messages: Vec<Message> = vec![Message::user_text(initial_user)];
        let mut total_usage = Usage::default();
        let mut last_text: Option<String> = None;

        for turn in 0..self.config.max_turns {
            let req = CompletionRequest {
                model: self.config.model.clone(),
                system: system.clone(),
                messages: messages.clone(),
                tools: tools.specs().to_vec(),
                max_tokens: self.config.max_tokens_per_response,
                temperature: self.config.temperature,
                cache_hints: self.config.cache_hints.clone(),
            };

            let resp = self.complete_with_retry(req).await?;
            total_usage += resp.usage;

            // Capture last text if present.
            for block in &resp.content {
                if let Content::Text(t) = block
                    && !t.trim().is_empty()
                {
                    last_text = Some(t.clone());
                }
            }

            // Append the assistant turn.
            messages.push(Message::Assistant {
                content: resp.content.clone(),
            });

            match resp.stop_reason {
                StopReason::EndTurn => {
                    return Ok(RunOutcome {
                        status: RunStatus::Completed,
                        final_text: last_text,
                        turns: turn + 1,
                        usage: total_usage,
                    });
                }
                StopReason::MaxTokens => {
                    return Ok(RunOutcome {
                        status: RunStatus::Truncated,
                        final_text: last_text,
                        turns: turn + 1,
                        usage: total_usage,
                    });
                }
                StopReason::ToolUse => {
                    let tool_results = self.run_tools(&resp.content, tools).await;
                    messages.push(Message::User {
                        content: tool_results,
                    });
                    // Continue loop.
                }
                StopReason::StopSequence | StopReason::Other(_) => {
                    return Ok(RunOutcome {
                        status: RunStatus::Completed,
                        final_text: last_text,
                        turns: turn + 1,
                        usage: total_usage,
                    });
                }
            }
        }

        Ok(RunOutcome {
            status: RunStatus::MaxTurnsExceeded,
            final_text: last_text,
            turns: self.config.max_turns,
            usage: total_usage,
        })
    }

    /// Invoke the model with retry on retryable errors.
    async fn complete_with_retry(
        &self,
        req: CompletionRequest,
    ) -> Result<ought_llm::CompletionResponse, AgentError> {
        let mut backoff = self.config.initial_backoff;
        let mut attempt: u32 = 0;
        loop {
            attempt += 1;
            match self.llm.complete(req.clone()).await {
                Ok(resp) => return Ok(resp),
                Err(e) => {
                    if !e.is_retryable() || attempt > self.config.max_retries {
                        return Err(AgentError::Llm {
                            attempts: attempt,
                            source: e,
                        });
                    }
                    tokio::time::sleep(backoff).await;
                    backoff = std::cmp::min(backoff * 2, self.config.max_backoff);
                }
            }
        }
    }

    /// Dispatch every `tool_use` block in the response and return a
    /// matching list of `tool_result` blocks (in order).
    async fn run_tools<T: ToolSet + ?Sized>(
        &self,
        response_content: &[Content],
        tools: &T,
    ) -> Vec<Content> {
        let mut results = Vec::new();
        for block in response_content {
            if let Content::ToolUse { id, name, input } = block {
                let outcome = tools.execute(name, input.clone()).await;
                results.push(Content::ToolResult {
                    tool_use_id: id.clone(),
                    content: outcome.content,
                    is_error: outcome.is_error,
                });
            }
        }
        results
    }
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use ought_llm::{CompletionResponse, Content as LlmContent};
    use std::sync::Mutex as StdMutex;

    /// A scripted Llm that returns a pre-baked response per call.
    struct ScriptedLlm {
        responses: StdMutex<Vec<Result<CompletionResponse, LlmError>>>,
        captured: StdMutex<Vec<CompletionRequest>>,
    }

    impl ScriptedLlm {
        fn new(responses: Vec<Result<CompletionResponse, LlmError>>) -> Self {
            Self {
                responses: StdMutex::new(responses),
                captured: StdMutex::new(vec![]),
            }
        }
    }

    #[async_trait]
    impl Llm for ScriptedLlm {
        fn name(&self) -> &'static str {
            "scripted"
        }
        async fn complete(
            &self,
            req: CompletionRequest,
        ) -> Result<CompletionResponse, LlmError> {
            self.captured.lock().unwrap().push(req);
            self.responses
                .lock()
                .unwrap()
                .remove(0)
        }
    }

    /// A trivial ToolSet that always returns "ok: <name>".
    struct EchoTools {
        specs: Vec<ToolSpec>,
    }

    impl EchoTools {
        fn new() -> Self {
            Self {
                specs: vec![ToolSpec {
                    name: "echo".into(),
                    description: "echo".into(),
                    input_schema: serde_json::json!({"type": "object"}),
                }],
            }
        }
    }

    #[async_trait]
    impl ToolSet for EchoTools {
        fn specs(&self) -> &[ToolSpec] {
            &self.specs
        }
        async fn execute(&self, name: &str, _input: Value) -> ToolOutcome {
            ToolOutcome::ok(format!("ok: {}", name))
        }
    }

    fn end_turn(text: &str) -> CompletionResponse {
        CompletionResponse {
            content: vec![LlmContent::Text(text.into())],
            stop_reason: StopReason::EndTurn,
            usage: Usage {
                input_tokens: 1,
                output_tokens: 1,
                ..Default::default()
            },
        }
    }

    fn tool_use_resp(id: &str, name: &str) -> CompletionResponse {
        CompletionResponse {
            content: vec![LlmContent::ToolUse {
                id: id.into(),
                name: name.into(),
                input: serde_json::json!({}),
            }],
            stop_reason: StopReason::ToolUse,
            usage: Usage {
                input_tokens: 1,
                output_tokens: 1,
                ..Default::default()
            },
        }
    }

    fn config() -> AgentConfig {
        AgentConfig {
            model: "test".into(),
            max_turns: 5,
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn end_turn_on_first_response_completes() {
        let llm = Arc::new(ScriptedLlm::new(vec![Ok(end_turn("hello"))]));
        let agent = Agent::new(llm.clone(), config());
        let out = agent
            .run("sys".into(), "go".into(), &EchoTools::new())
            .await
            .unwrap();
        assert_eq!(out.status, RunStatus::Completed);
        assert_eq!(out.turns, 1);
        assert_eq!(out.final_text.as_deref(), Some("hello"));
    }

    #[tokio::test]
    async fn tool_use_then_end_turn() {
        let llm = Arc::new(ScriptedLlm::new(vec![
            Ok(tool_use_resp("toolu_a", "echo")),
            Ok(end_turn("done")),
        ]));
        let agent = Agent::new(llm.clone(), config());
        let out = agent
            .run("sys".into(), "go".into(), &EchoTools::new())
            .await
            .unwrap();
        assert_eq!(out.status, RunStatus::Completed);
        assert_eq!(out.turns, 2);

        // Inspect what was sent on the second call: should include the
        // tool_result message.
        let captured = llm.captured.lock().unwrap();
        assert_eq!(captured.len(), 2);
        let second = &captured[1];
        // Conversation: user("go"), assistant(tool_use), user(tool_result)
        assert_eq!(second.messages.len(), 3);
        match &second.messages[2] {
            Message::User { content } => match &content[0] {
                Content::ToolResult {
                    tool_use_id,
                    content,
                    ..
                } => {
                    assert_eq!(tool_use_id, "toolu_a");
                    assert_eq!(content, "ok: echo");
                }
                _ => panic!("expected tool_result"),
            },
            _ => panic!("expected user message"),
        }
    }

    #[tokio::test]
    async fn max_turns_exceeded_returns_status() {
        // Always emit tool_use; loop will hit max_turns.
        let llm = Arc::new(ScriptedLlm::new(vec![
            Ok(tool_use_resp("a", "echo")),
            Ok(tool_use_resp("b", "echo")),
            Ok(tool_use_resp("c", "echo")),
        ]));
        let agent = Agent::new(
            llm.clone(),
            AgentConfig {
                model: "test".into(),
                max_turns: 3,
                ..Default::default()
            },
        );
        let out = agent
            .run("sys".into(), "go".into(), &EchoTools::new())
            .await
            .unwrap();
        assert_eq!(out.status, RunStatus::MaxTurnsExceeded);
        assert_eq!(out.turns, 3);
    }

    #[tokio::test]
    async fn truncated_when_max_tokens_stop() {
        let resp = CompletionResponse {
            content: vec![LlmContent::Text("partial".into())],
            stop_reason: StopReason::MaxTokens,
            usage: Usage::default(),
        };
        let llm = Arc::new(ScriptedLlm::new(vec![Ok(resp)]));
        let agent = Agent::new(llm.clone(), config());
        let out = agent
            .run("sys".into(), "go".into(), &EchoTools::new())
            .await
            .unwrap();
        assert_eq!(out.status, RunStatus::Truncated);
    }

    #[tokio::test]
    async fn non_retryable_error_propagates_immediately() {
        let llm = Arc::new(ScriptedLlm::new(vec![Err(LlmError::Auth(
            "no key".into(),
        ))]));
        let agent = Agent::new(llm.clone(), config());
        let res = agent
            .run("sys".into(), "go".into(), &EchoTools::new())
            .await;
        match res {
            Err(AgentError::Llm { attempts, source }) => {
                assert_eq!(attempts, 1);
                assert!(matches!(source, LlmError::Auth(_)));
            }
            other => panic!("expected Llm error, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn retryable_error_then_success() {
        let llm = Arc::new(ScriptedLlm::new(vec![
            Err(LlmError::Api {
                status: 503,
                message: "down".into(),
            }),
            Ok(end_turn("recovered")),
        ]));
        let agent = Agent::new(
            llm.clone(),
            AgentConfig {
                model: "test".into(),
                max_turns: 5,
                initial_backoff: Duration::from_millis(1),
                max_backoff: Duration::from_millis(2),
                ..Default::default()
            },
        );
        let out = agent
            .run("sys".into(), "go".into(), &EchoTools::new())
            .await
            .unwrap();
        assert_eq!(out.status, RunStatus::Completed);
        assert_eq!(out.final_text.as_deref(), Some("recovered"));
    }

    #[tokio::test]
    async fn usage_accumulates_across_turns() {
        let llm = Arc::new(ScriptedLlm::new(vec![
            Ok(CompletionResponse {
                content: vec![LlmContent::ToolUse {
                    id: "a".into(),
                    name: "echo".into(),
                    input: serde_json::json!({}),
                }],
                stop_reason: StopReason::ToolUse,
                usage: Usage {
                    input_tokens: 100,
                    output_tokens: 20,
                    ..Default::default()
                },
            }),
            Ok(CompletionResponse {
                content: vec![LlmContent::Text("done".into())],
                stop_reason: StopReason::EndTurn,
                usage: Usage {
                    input_tokens: 200,
                    output_tokens: 30,
                    ..Default::default()
                },
            }),
        ]));
        let agent = Agent::new(llm, config());
        let out = agent
            .run("sys".into(), "go".into(), &EchoTools::new())
            .await
            .unwrap();
        assert_eq!(out.usage.input_tokens, 300);
        assert_eq!(out.usage.output_tokens, 50);
    }
}
