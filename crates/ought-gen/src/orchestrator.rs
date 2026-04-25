//! Orchestrates per-assignment in-process agent loops.
//!
//! For each assignment, builds a [`GenerateToolSet`] over the shared
//! manifest and an [`oharness_loop::Agent`] over a shared [`Llm`] client,
//! then runs the loop. Concurrency is bounded by a [`tokio::Semaphore`];
//! per-clause outcomes are read out of the tool set's tracker rather
//! than reconstructed from any model output.
//!
//! # Migration note
//!
//! The LLM + agent loop is now the sibling `open-harness` framework
//! (crates `oharness-llm` / `oharness-providers` / `oharness-loop` /
//! `oharness-tools`). `ought-llm` / `ought-agent` are deleted. Features
//! that ought-agent had and open-harness doesn't yet are regressions we
//! accept for this cutover — notably:
//!
//! - No automatic per-provider retry with exponential backoff
//!   (`ought-agent::Agent::complete_with_retry` is gone). A 503 surfaces
//!   as a single-turn failure. If this bites, wrap the `Arc<dyn Llm>`
//!   with a retry layer via `oharness-llm`'s middleware API.
//! - No tool-result eviction / context-budget guard
//!   (`ought-agent::evict_old_tool_results`, `ContextExhausted`). The
//!   run terminates with `Termination::Failed { RunErrorCategory::Llm }`
//!   when the provider 400s for oversize context. For long extraction /
//!   generation runs, lower `max_turns` or shrink `read_source_limit_bytes`.
//! - No `CompletionRequest.max_tokens` / `temperature` plumbing — the
//!   ReactLoop builds requests with neither set, so each turn uses the
//!   provider default (Anthropic 4096, OpenAI 4096). `config.model`
//!   determines the model on the provider constructor.
//!
//! Anthropic prompt caching is restored via the in-file [`CacheLastMessage`]
//! `RequestLayer`, wired only into the Anthropic branch of `build_llm`.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use tokio::sync::Semaphore;
use tokio::task::JoinSet;

use oharness_core::{
    CacheBreakpoint, CompletionReason, CompletionRequest, Task, Termination, TruncationLimit,
};
use oharness_llm::{Llm, LlmExt, RequestLayer};
use oharness_loop::{Agent, ReactLoop};

use crate::agent::{AgentAssignment, AgentReport, AgentRunStatus};
use crate::config::{GeneratorConfig, Provider};
use crate::manifest::Manifest;
use crate::tool_set::GenerateToolSet;

pub struct Orchestrator {
    config: GeneratorConfig,
    verbose: bool,
    manifest: Arc<Mutex<Manifest>>,
    manifest_path: PathBuf,
}

impl Orchestrator {
    pub fn new(
        config: GeneratorConfig,
        manifest: Arc<Mutex<Manifest>>,
        manifest_path: PathBuf,
        verbose: bool,
    ) -> Self {
        Self {
            config,
            verbose,
            manifest,
            manifest_path,
        }
    }

    /// Run all assignments and return per-assignment reports.
    ///
    /// Consumes `self` so the orchestrator's `Arc<Mutex<Manifest>>`
    /// reference is dropped on return; callers holding their own Arc
    /// can then `Arc::try_unwrap` to recover the manifest for final
    /// persistence.
    pub async fn run(
        self,
        assignments: Vec<AgentAssignment>,
    ) -> anyhow::Result<Vec<AgentReport>> {
        if assignments.is_empty() {
            return Ok(vec![]);
        }

        let llm = build_llm(&self.config)?;
        let max_turns = self.config.max_turns;
        let read_source_limit = self.config.read_source_limit_bytes;

        let parallelism = self.config.parallelism.max(1);
        let sem = Arc::new(Semaphore::new(parallelism));
        let mut tasks = JoinSet::new();

        for assignment in assignments {
            let permit = sem.clone().acquire_owned().await?;
            let llm = llm.clone();
            let manifest = self.manifest.clone();
            let manifest_path = self.manifest_path.clone();
            let verbose = self.verbose;

            tasks.spawn(async move {
                let _permit = permit;
                run_one_assignment(
                    assignment,
                    llm,
                    max_turns,
                    manifest,
                    manifest_path,
                    read_source_limit,
                    verbose,
                )
                .await
            });
        }

        let mut reports = Vec::new();
        while let Some(joined) = tasks.join_next().await {
            match joined {
                Ok(report) => reports.push(report),
                Err(e) => reports.push(AgentReport {
                    errors: vec![format!("agent task panicked: {}", e)],
                    status: AgentRunStatus::Errored,
                    ..AgentReport::default()
                }),
            }
        }
        Ok(reports)
    }
}

async fn run_one_assignment(
    assignment: AgentAssignment,
    llm: Arc<dyn Llm>,
    max_turns: u32,
    manifest: Arc<Mutex<Manifest>>,
    manifest_path: PathBuf,
    read_source_limit_bytes: usize,
    verbose: bool,
) -> AgentReport {
    let assignment_id = assignment.id.clone();
    let group_count = assignment.groups.len();
    let clause_count: usize = assignment.groups.iter().map(|g| g.clauses.len()).sum();

    if verbose {
        eprintln!(
            "  [agent {}] starting: {} groups, {} clauses",
            assignment_id, group_count, clause_count
        );
    }

    // Keep a concrete Arc<GenerateToolSet> so we can read `.usage()` after
    // the loop terminates — the ToolSet trait object the agent holds is
    // opaque, but both Arcs point to the same allocation with shared
    // interior state.
    let tools_concrete = Arc::new(GenerateToolSet::with_limits(
        assignment.clone(),
        manifest,
        manifest_path,
        read_source_limit_bytes,
    ));
    let tools_for_agent: Arc<dyn oharness_tools::ToolSet> = tools_concrete.clone();

    let system = build_system_prompt(&assignment);
    let initial = build_initial_user_message(&assignment);

    let agent = match Agent::builder()
        .with_llm(llm)
        .with_tools(tools_for_agent)
        .with_loop(Box::new(ReactLoop::new().with_system_prompt(system)))
        .with_max_turns(max_turns)
        .build()
    {
        Ok(a) => a,
        Err(e) => {
            return AgentReport {
                assignment_id,
                status: AgentRunStatus::Errored,
                errors: vec![format!("agent build: {}", e)],
                ..AgentReport::default()
            };
        }
    };

    let task = Task::new(initial).with_id(assignment_id.clone());
    let result = agent.run(task).await;
    let usage_snapshot = tools_concrete.usage();

    let mut report = AgentReport {
        assignment_id: assignment_id.clone(),
        generated: usage_snapshot.written,
        write_errors: usage_snapshot.write_errors,
        ..AgentReport::default()
    };

    match result {
        Ok(outcome) => {
            report.status = map_termination(&outcome.termination);
            report.turns = outcome.usage.turns;
            report.usage_input_tokens = outcome.usage.tokens_input as u32;
            report.usage_output_tokens = outcome.usage.tokens_output as u32;
            report.usage_cache_read_tokens = outcome.usage.tokens_cache_read as u32;
            report.usage_cache_creation_tokens = outcome.usage.tokens_cache_create as u32;
            if matches!(report.status, AgentRunStatus::Errored)
                && let Termination::Failed { error, .. } = &outcome.termination
            {
                report.errors.push(error.message.clone());
            }
            if verbose {
                eprintln!(
                    "  [agent {}] finished: {} written, {} write errors, {} turns",
                    assignment_id,
                    report.generated.len(),
                    report.write_errors.len(),
                    report.turns
                );
            }
        }
        Err(e) => {
            report.status = AgentRunStatus::Errored;
            report.errors.push(format!("agent loop failed: {}", e));
            if verbose {
                eprintln!("  [agent {}] errored: {}", assignment_id, e);
            }
        }
    }

    report
}

fn map_termination(t: &Termination) -> AgentRunStatus {
    match t {
        Termination::Completed {
            reason: CompletionReason::EndTurn | CompletionReason::StopSequence(_),
        } => AgentRunStatus::Completed,
        Termination::Truncated {
            limit: TruncationLimit::MaxTurns(_),
        } => AgentRunStatus::MaxTurnsExceeded,
        Termination::Truncated {
            limit: TruncationLimit::MaxTokens,
        } => AgentRunStatus::Truncated,
        Termination::Truncated {
            limit: TruncationLimit::Budget(_),
        } => AgentRunStatus::ContextExhausted,
        Termination::Truncated {
            limit: TruncationLimit::Timeout,
        } => AgentRunStatus::Errored,
        Termination::Failed { .. } | Termination::Interrupted { .. } => AgentRunStatus::Errored,
    }
}

pub(crate) fn build_llm(config: &GeneratorConfig) -> anyhow::Result<Arc<dyn Llm>> {
    fn require_env(var: &str) -> anyhow::Result<String> {
        std::env::var(var).map_err(|_| {
            anyhow::anyhow!("{} not set; export it or change provider in ought.toml", var)
        })
    }

    use oharness_providers::{AnthropicLlm, OpenAiLlm};

    match config.provider {
        Provider::Anthropic => {
            let key = require_env(&config.anthropic.api_key_env)?;
            let mut llm = AnthropicLlm::new(key, config.model.clone());
            if let Some(url) = &config.anthropic.base_url {
                llm = llm.with_base_url(url.clone());
            }
            Ok(Arc::new(llm.with_request_layer(CacheLastMessage)))
        }
        Provider::Openai => {
            let key = require_env(&config.openai.api_key_env)?;
            let mut llm = OpenAiLlm::new(key, config.model.clone());
            if let Some(url) = &config.openai.base_url {
                llm = llm.with_base_url(url.clone());
            }
            Ok(Arc::new(llm))
        }
        Provider::Openrouter => {
            let key = require_env(&config.openrouter.api_key_env)?;
            let mut llm = oharness_providers::OpenRouter::new(key, config.model.clone());
            if let Some(url) = &config.openrouter.app_url {
                llm = llm.with_extra_header("HTTP-Referer", url.clone());
            }
            if let Some(title) = &config.openrouter.app_title {
                llm = llm.with_extra_header("X-Title", title.clone());
            }
            Ok(Arc::new(llm))
        }
        Provider::Ollama => {
            let url = config
                .ollama
                .base_url
                .clone()
                .unwrap_or_else(|| "http://localhost:11434/v1/chat/completions".to_string());
            let llm = oharness_providers::Ollama::at(url, config.model.clone());
            Ok(Arc::new(llm))
        }
    }
}

/// Request layer that marks the last message of every request as a cache
/// breakpoint, so Anthropic caches the entire conversation prefix turn
/// after turn. Anthropic allows up to 4 active breakpoints and returns
/// cache-read tokens on any matching prefix — marking the most recent
/// message each turn keeps a rolling window of cached prefixes without
/// any bookkeeping. Non-caching providers ignore `cache_hints` entirely,
/// so this is only wired into the Anthropic branch in `build_llm`.
struct CacheLastMessage;

impl RequestLayer for CacheLastMessage {
    fn on_request(&self, req: &mut CompletionRequest) {
        if req.messages.is_empty() {
            return;
        }
        req.cache_hints.breakpoints.push(CacheBreakpoint {
            message_index: req.messages.len() - 1,
            ttl: None, // default 5m — long enough for turn-to-turn reuse.
        });
    }
}

fn build_system_prompt(assignment: &AgentAssignment) -> String {
    let mut p = String::from(
        "You are a test generation agent for the ought behavioral test framework.\n\n",
    );

    if !assignment.source_paths.is_empty() {
        p.push_str("Before generating tests, read the source files for the code under test ");
        p.push_str("with `read_source`:\n");
        for path in &assignment.source_paths {
            p.push_str(&format!("  - {}\n", path));
        }
        p.push_str("\nIf a source file doesn't exist yet, that's OK — write tests against ");
        p.push_str("the expected interface described in the spec clauses (TDD mode). Assume ");
        p.push_str("reasonable function signatures based on the clause text.\n\n");
    } else {
        p.push_str("You are in TDD mode. The source code may not exist yet. Write tests ");
        p.push_str("against the expected interface described in the spec clauses. Assume ");
        p.push_str("reasonable function signatures based on the clause text.\n\n");
    }

    p.push_str(
        "Your tools:\n\
         1. `get_assignment` — see the clauses you must generate tests for.\n\
         2. `read_source` / `list_source_files` — explore the project.\n\
         3. `write_test` / `write_tests_batch` — emit test code.\n\
         4. `check_compiles` — verify your tests compile; iterate on failures.\n\
         5. `report_progress` — emit human-visible progress lines.\n\n\
         Generate self-contained tests with the clause text as a doc comment.\n\n\
         TEST FILE LAYOUT (per-subsection):\n\
         All clauses under the same subsection share a single test file.\n\
         For a clause id `parser::clause_ir::must_generate_foo`, the file is\n\
         `<test_dir>/src/parser/clause_ir_test.rs` (Rust). Write only the\n\
         #[test] function plus its leading doc comment; write_test merges\n\
         it into the file, replacing any previous version with the same fn name.\n\n\
         Name each Rust test function as:\n\
         `fn test_<subsystem>__<subsection>__<clause_slug>()` — using DOUBLE\n\
         underscores to separate `::` boundaries, so the full clause path is\n\
         recoverable from the function name. The clause_slug is everything\n\
         after the subsection in the clause id.\n\n",
    );

    p.push_str(&format!(
        "Target language: {}. ",
        assignment.target_language
    ));
    match assignment.target_language.as_str() {
        "rust" => p.push_str("Use #[test] attribute and assert! macros.\n"),
        "python" => p.push_str("Use def test_... with assert statements.\n"),
        "typescript" | "ts" | "javascript" | "js" => {
            p.push_str("Use test() or it() with expect() assertions (Jest style).\n")
        }
        "go" => p.push_str("Use func Test...(t *testing.T) with t.Error/t.Fatal.\n"),
        _ => p.push_str("Use the language's standard test conventions.\n"),
    }

    p
}

fn build_initial_user_message(assignment: &AgentAssignment) -> String {
    format!(
        "Begin assignment {}. Call `get_assignment` first to see your work, then proceed.",
        assignment.id
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use oharness_core::Message;

    #[test]
    fn cache_last_message_marks_breakpoint_at_final_index() {
        let layer = CacheLastMessage;
        let mut req = CompletionRequest::new(vec![
            Message::user_text("first"),
            Message::assistant_text("reply"),
            Message::user_text("second"),
        ]);
        layer.on_request(&mut req);
        assert_eq!(req.cache_hints.breakpoints.len(), 1);
        assert_eq!(req.cache_hints.breakpoints[0].message_index, 2);
    }

    #[test]
    fn cache_last_message_skips_empty_messages() {
        let layer = CacheLastMessage;
        let mut req = CompletionRequest::new(vec![]);
        layer.on_request(&mut req);
        assert!(req.cache_hints.breakpoints.is_empty());
    }

    /// Regression: `run` must consume `self` so its clone of the shared
    /// manifest Arc is dropped before the caller tries to `try_unwrap`.
    /// An earlier version took `&self`, leaving the orchestrator's Arc
    /// alive and silently failing the recovery path in `ought generate`.
    #[tokio::test]
    async fn run_releases_manifest_reference() {
        let manifest = Arc::new(Mutex::new(Manifest::default()));
        let orch = Orchestrator::new(
            GeneratorConfig::default(),
            manifest.clone(),
            PathBuf::from("/tmp/ought_test_manifest.toml"),
            false,
        );
        // Empty assignments list short-circuits before any LLM call; the
        // test exercises only the ownership contract.
        let _ = orch.run(vec![]).await.unwrap();
        assert_eq!(
            Arc::strong_count(&manifest),
            1,
            "orchestrator leaked a manifest Arc after run"
        );
    }
}
