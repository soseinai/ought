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
    /// Cache hints sent on every request. The loop overrides
    /// `cache_messages_up_to` per turn so the conversation prefix is
    /// always cached.
    pub cache_hints: CacheHints,
    /// Hard cap on per-request input tokens (system + tools +
    /// conversation, including cached). When the most recent turn's
    /// reported input tokens exceed this, the loop terminates with
    /// [`RunStatus::ContextExhausted`] rather than letting the next
    /// request hit a 400 from the provider.
    pub context_budget_tokens: u32,
    /// Soft threshold for tool-result eviction. When per-request input
    /// tokens cross this, older tool_result blocks get rewritten as
    /// short placeholders so the next request fits.
    pub eviction_threshold_tokens: u32,
    /// Number of most-recent tool_result blocks to never evict.
    pub preserve_recent_tool_results: usize,
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
                // Overridden per-turn by Agent::run.
                cache_messages_up_to: None,
            },
            // Calibrated for Claude Sonnet 4.6 (200K). Lower for smaller
            // models via config.
            context_budget_tokens: 180_000,
            eviction_threshold_tokens: 130_000,
            preserve_recent_tool_results: 4,
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
    /// Per-request input tokens crossed [`AgentConfig::context_budget_tokens`].
    /// The loop bailed pre-emptively rather than letting the next
    /// request hit a 400.
    ContextExhausted,
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
            // Step 1 (caching): mark the last message with cache_control
            // so the entire conversation prefix is cacheable. The
            // adapter only honors this for providers that support it
            // (Anthropic today); others ignore the hint.
            let mut hints = self.config.cache_hints.clone();
            if !messages.is_empty() {
                hints.cache_messages_up_to = Some(messages.len() - 1);
            }

            let req = CompletionRequest {
                model: self.config.model.clone(),
                system: system.clone(),
                messages: messages.clone(),
                tools: tools.specs().to_vec(),
                max_tokens: self.config.max_tokens_per_response,
                temperature: self.config.temperature,
                cache_hints: hints,
            };

            let resp = self.complete_with_retry(req).await?;
            total_usage += resp.usage;

            // Step 4 (context budget): the most recent input token count
            // tells us how big the conversation has grown. If it's over
            // the hard cap, stop before the next request would 400.
            let last_input = approx_context_size(&resp.usage);
            if last_input >= self.config.context_budget_tokens {
                // Still record the assistant turn we just got, so
                // partial work isn't invisible to callers.
                messages.push(Message::Assistant {
                    content: resp.content.clone(),
                });
                for block in &resp.content {
                    if let Content::Text(t) = block
                        && !t.trim().is_empty()
                    {
                        last_text = Some(t.clone());
                    }
                }
                return Ok(RunOutcome {
                    status: RunStatus::ContextExhausted,
                    final_text: last_text,
                    turns: turn + 1,
                    usage: total_usage,
                });
            }

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

                    // Step 3 (eviction): if we crossed the soft threshold,
                    // rewrite older tool_result content as short
                    // placeholders. Pairing (tool_use ↔ tool_result IDs)
                    // is preserved; only the result body shrinks.
                    if last_input >= self.config.eviction_threshold_tokens {
                        evict_old_tool_results(
                            &mut messages,
                            self.config.preserve_recent_tool_results,
                        );
                    }
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

// ── Memory helpers ──────────────────────────────────────────────────────

/// Approximate the per-request input token count from a usage report.
///
/// Anthropic reports `input_tokens` for new (non-cached) tokens and
/// `cache_read_tokens` separately; the actual context size is their sum.
/// OpenAI reports just `input_tokens` (cache fields are zero).
fn approx_context_size(usage: &Usage) -> u32 {
    usage.input_tokens.saturating_add(usage.cache_read_tokens)
}

/// Rewrite older `tool_result` blocks as short placeholders so the
/// conversation prefix shrinks before the next request.
///
/// Walks the message list in order and elides every `tool_result`
/// except the most recent `preserve_recent`. Tool-use ↔ tool-result
/// pairing is preserved (only the `content` string shrinks; the
/// `tool_use_id` and the block's position in the message are
/// untouched).
///
/// Returns the number of bytes freed across all rewritten blocks.
pub(crate) fn evict_old_tool_results(messages: &mut [Message], preserve_recent: usize) -> usize {
    // Locate every tool_result block.
    let mut positions: Vec<(usize, usize)> = Vec::new();
    for (mi, m) in messages.iter().enumerate() {
        let content = match m {
            Message::User { content } => content,
            Message::Assistant { content } => content,
        };
        for (ci, block) in content.iter().enumerate() {
            if matches!(block, Content::ToolResult { .. }) {
                positions.push((mi, ci));
            }
        }
    }

    let cutoff = positions.len().saturating_sub(preserve_recent);
    let mut bytes_freed = 0usize;
    for &(mi, ci) in &positions[..cutoff] {
        let content = match &mut messages[mi] {
            Message::User { content } => content,
            Message::Assistant { content } => content,
        };
        if let Content::ToolResult {
            content: text,
            is_error,
            ..
        } = &mut content[ci]
        {
            // Already elided once — leave it alone.
            if text.starts_with("[elided") {
                continue;
            }
            let original_len = text.len();
            let placeholder = if *is_error {
                format!(
                    "[elided: {} chars from earlier failed tool call; call the tool again if needed]",
                    original_len
                )
            } else {
                format!(
                    "[elided: {} chars from earlier tool call; call the tool again if needed]",
                    original_len
                )
            };
            bytes_freed += original_len.saturating_sub(placeholder.len());
            *text = placeholder;
        }
    }
    bytes_freed
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

    // ── Memory tests ────────────────────────────────────────────────

    #[tokio::test]
    async fn cache_messages_up_to_marks_last_message_each_turn() {
        // Two-turn run; verify that on each request the cache hint
        // pointed at the last message in the conversation at that time.
        let llm = Arc::new(ScriptedLlm::new(vec![
            Ok(tool_use_resp("a", "echo")),
            Ok(end_turn("done")),
        ]));
        let agent = Agent::new(llm.clone(), config());
        agent
            .run("sys".into(), "go".into(), &EchoTools::new())
            .await
            .unwrap();

        let captured = llm.captured.lock().unwrap();
        // Turn 1 sent [user("go")] — last index is 0.
        assert_eq!(captured[0].cache_hints.cache_messages_up_to, Some(0));
        // Turn 2 sent [user, assistant, user(tool_result)] — last index is 2.
        assert_eq!(captured[1].cache_hints.cache_messages_up_to, Some(2));
    }

    fn resp_with_tokens(input_tokens: u32, stop_reason: StopReason) -> CompletionResponse {
        CompletionResponse {
            content: vec![LlmContent::ToolUse {
                id: "x".into(),
                name: "echo".into(),
                input: serde_json::json!({}),
            }],
            stop_reason,
            usage: Usage {
                input_tokens,
                output_tokens: 1,
                ..Default::default()
            },
        }
    }

    #[tokio::test]
    async fn context_exhausted_when_input_tokens_over_budget() {
        let llm = Arc::new(ScriptedLlm::new(vec![Ok(resp_with_tokens(
            10_000,
            StopReason::ToolUse,
        ))]));
        let agent = Agent::new(
            llm,
            AgentConfig {
                model: "test".into(),
                max_turns: 5,
                context_budget_tokens: 5_000,
                eviction_threshold_tokens: 4_000,
                ..Default::default()
            },
        );
        let out = agent
            .run("sys".into(), "go".into(), &EchoTools::new())
            .await
            .unwrap();
        assert_eq!(out.status, RunStatus::ContextExhausted);
        assert_eq!(out.turns, 1);
    }

    #[tokio::test]
    async fn eviction_runs_when_threshold_crossed() {
        // Turn 1: tool_use, reports 5K input tokens (above eviction
        // threshold 4K). The orchestrator dispatches the tool and
        // appends a tool_result. Eviction should rewrite earlier
        // tool_results — but since this is the first one, nothing to
        // evict. Add a second turn so we have an older tool_result.
        let llm = Arc::new(ScriptedLlm::new(vec![
            Ok(resp_with_tokens(5_000, StopReason::ToolUse)),
            Ok(resp_with_tokens(6_000, StopReason::ToolUse)),
            Ok(end_turn("done")),
        ]));

        // EchoTools returns a long string so the tool_result is big.
        struct BigEchoTools {
            specs: Vec<ToolSpec>,
        }
        #[async_trait]
        impl ToolSet for BigEchoTools {
            fn specs(&self) -> &[ToolSpec] {
                &self.specs
            }
            async fn execute(&self, _name: &str, _input: Value) -> ToolOutcome {
                ToolOutcome::ok("X".repeat(1_000))
            }
        }

        let agent = Agent::new(
            llm.clone(),
            AgentConfig {
                model: "test".into(),
                max_turns: 5,
                context_budget_tokens: 100_000,
                eviction_threshold_tokens: 4_000,
                preserve_recent_tool_results: 1,
                ..Default::default()
            },
        );
        let tools = BigEchoTools {
            specs: vec![ToolSpec {
                name: "echo".into(),
                description: "echo".into(),
                input_schema: serde_json::json!({"type": "object"}),
            }],
        };
        agent.run("sys".into(), "go".into(), &tools).await.unwrap();

        // Turn 3 was sent after turn 2 dispatched its tool. By that
        // point the first tool_result should have been elided
        // (preserve_recent_tool_results = 1 means only the most recent
        // is kept).
        let captured = llm.captured.lock().unwrap();
        let final_msgs = &captured[2].messages;
        // Find all tool_result content strings.
        let mut texts = vec![];
        for m in final_msgs {
            let blocks = match m {
                Message::User { content } => content,
                Message::Assistant { content } => content,
            };
            for b in blocks {
                if let Content::ToolResult { content, .. } = b {
                    texts.push(content.as_str());
                }
            }
        }
        assert_eq!(texts.len(), 2, "expected two tool_result blocks");
        // Older one elided, newer one intact.
        assert!(texts[0].starts_with("[elided"));
        assert_eq!(texts[1].len(), 1_000);
    }

    #[test]
    fn evict_preserves_tool_use_pairing() {
        // Build a small conversation with two tool_use/tool_result pairs.
        let mut messages = vec![
            Message::user_text("go"),
            Message::Assistant {
                content: vec![LlmContent::ToolUse {
                    id: "id_1".into(),
                    name: "echo".into(),
                    input: serde_json::json!({}),
                }],
            },
            Message::User {
                content: vec![LlmContent::ToolResult {
                    tool_use_id: "id_1".into(),
                    content: "Y".repeat(500),
                    is_error: false,
                }],
            },
            Message::Assistant {
                content: vec![LlmContent::ToolUse {
                    id: "id_2".into(),
                    name: "echo".into(),
                    input: serde_json::json!({}),
                }],
            },
            Message::User {
                content: vec![LlmContent::ToolResult {
                    tool_use_id: "id_2".into(),
                    content: "Z".repeat(500),
                    is_error: false,
                }],
            },
        ];

        let freed = evict_old_tool_results(&mut messages, 1);
        assert!(freed > 0);

        // Both tool_use_ids must still be present, in order.
        let mut tool_use_ids = vec![];
        let mut tool_result_ids = vec![];
        for m in &messages {
            let blocks = match m {
                Message::User { content } => content,
                Message::Assistant { content } => content,
            };
            for b in blocks {
                match b {
                    LlmContent::ToolUse { id, .. } => tool_use_ids.push(id.as_str()),
                    LlmContent::ToolResult { tool_use_id, .. } => {
                        tool_result_ids.push(tool_use_id.as_str())
                    }
                    _ => {}
                }
            }
        }
        assert_eq!(tool_use_ids, vec!["id_1", "id_2"]);
        assert_eq!(tool_result_ids, vec!["id_1", "id_2"]);

        // First result body should now be a placeholder; second intact.
        let r1 = match &messages[2] {
            Message::User { content } => match &content[0] {
                LlmContent::ToolResult { content, .. } => content.clone(),
                _ => panic!(),
            },
            _ => panic!(),
        };
        let r2 = match &messages[4] {
            Message::User { content } => match &content[0] {
                LlmContent::ToolResult { content, .. } => content.clone(),
                _ => panic!(),
            },
            _ => panic!(),
        };
        assert!(r1.starts_with("[elided"));
        assert_eq!(r2.len(), 500);
    }

    #[test]
    fn evict_idempotent_on_already_elided() {
        // Calling evict twice shouldn't double-wrap the placeholder.
        let mut messages = vec![
            Message::Assistant {
                content: vec![LlmContent::ToolUse {
                    id: "id_1".into(),
                    name: "echo".into(),
                    input: serde_json::json!({}),
                }],
            },
            Message::User {
                content: vec![LlmContent::ToolResult {
                    tool_use_id: "id_1".into(),
                    content: "Y".repeat(500),
                    is_error: false,
                }],
            },
        ];
        let _ = evict_old_tool_results(&mut messages, 0);
        let snap1 = match &messages[1] {
            Message::User { content } => match &content[0] {
                LlmContent::ToolResult { content, .. } => content.clone(),
                _ => panic!(),
            },
            _ => panic!(),
        };
        let _ = evict_old_tool_results(&mut messages, 0);
        let snap2 = match &messages[1] {
            Message::User { content } => match &content[0] {
                LlmContent::ToolResult { content, .. } => content.clone(),
                _ => panic!(),
            },
            _ => panic!(),
        };
        assert_eq!(snap1, snap2);
    }

    // ── Original tests continue ─────────────────────────────────────

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
