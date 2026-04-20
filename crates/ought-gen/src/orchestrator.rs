//! Orchestrates per-assignment in-process agent loops.
//!
//! For each assignment, builds a [`GenerateToolSet`] over the shared
//! manifest and an [`ought_agent::Agent`] over a shared [`Llm`] client,
//! then runs the loop. Concurrency is bounded by a [`tokio::Semaphore`];
//! per-clause outcomes are read out of the tool set's tracker rather
//! than reconstructed from any model output.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use tokio::sync::Semaphore;
use tokio::task::JoinSet;

use ought_agent::{Agent, AgentConfig, AgentError, RunStatus};
use ought_llm::{AnthropicLlm, Llm, OpenAiLlm};

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
        let agent_cfg = AgentConfig {
            model: self.config.model.clone(),
            max_turns: self.config.max_turns,
            max_tokens_per_response: self.config.max_tokens_per_response,
            temperature: self.config.temperature,
            context_budget_tokens: self.config.context_budget_tokens,
            eviction_threshold_tokens: self.config.eviction_threshold_tokens,
            ..AgentConfig::default()
        };
        let read_source_limit = self.config.read_source_limit_bytes;

        let parallelism = self.config.parallelism.max(1);
        let sem = Arc::new(Semaphore::new(parallelism));
        let mut tasks = JoinSet::new();

        for assignment in assignments {
            let permit = sem.clone().acquire_owned().await?;
            let llm = llm.clone();
            let agent_cfg = agent_cfg.clone();
            let manifest = self.manifest.clone();
            let manifest_path = self.manifest_path.clone();
            let verbose = self.verbose;

            tasks.spawn(async move {
                let _permit = permit;
                run_one_assignment(
                    assignment,
                    llm,
                    agent_cfg,
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
    agent_cfg: AgentConfig,
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

    let tools = GenerateToolSet::with_limits(
        assignment.clone(),
        manifest,
        manifest_path,
        read_source_limit_bytes,
    );
    let system = build_system_prompt(&assignment);
    let initial = build_initial_user_message(&assignment);

    let agent = Agent::new(llm, agent_cfg);
    let result = agent.run(system, initial, &tools).await;
    let usage_snapshot = tools.usage();

    let mut report = AgentReport {
        assignment_id: assignment_id.clone(),
        generated: usage_snapshot.written,
        write_errors: usage_snapshot.write_errors,
        ..AgentReport::default()
    };

    match result {
        Ok(outcome) => {
            report.status = match outcome.status {
                RunStatus::Completed => AgentRunStatus::Completed,
                RunStatus::MaxTurnsExceeded => AgentRunStatus::MaxTurnsExceeded,
                RunStatus::Truncated => AgentRunStatus::Truncated,
                RunStatus::ContextExhausted => AgentRunStatus::ContextExhausted,
            };
            report.turns = outcome.turns;
            report.usage_input_tokens = outcome.usage.input_tokens;
            report.usage_output_tokens = outcome.usage.output_tokens;
            report.usage_cache_read_tokens = outcome.usage.cache_read_tokens;
            report.usage_cache_creation_tokens = outcome.usage.cache_creation_tokens;
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
        Err(AgentError::Llm { attempts, source }) => {
            report.status = AgentRunStatus::Errored;
            report.errors.push(format!(
                "agent loop failed after {} attempt(s): {}",
                attempts, source
            ));
            if verbose {
                eprintln!("  [agent {}] errored: {}", assignment_id, source);
            }
        }
    }

    report
}

pub(crate) fn build_llm(config: &GeneratorConfig) -> anyhow::Result<Arc<dyn Llm>> {
    fn require_env(var: &str) -> anyhow::Result<String> {
        std::env::var(var).map_err(|_| {
            anyhow::anyhow!("{} not set; export it or change provider in ought.toml", var)
        })
    }

    match config.provider {
        Provider::Anthropic => {
            let key = require_env(&config.anthropic.api_key_env)?;
            let llm = match &config.anthropic.base_url {
                Some(url) => AnthropicLlm::with_base_url(key, url.clone())?,
                None => AnthropicLlm::new(key)?,
            };
            Ok(Arc::new(llm))
        }
        Provider::Openai => {
            let key = require_env(&config.openai.api_key_env)?;
            let llm = match &config.openai.base_url {
                Some(url) => OpenAiLlm::custom("openai", Some(key), url.clone(), vec![])?,
                None => OpenAiLlm::openai(key)?,
            };
            Ok(Arc::new(llm))
        }
        Provider::Openrouter => {
            let key = require_env(&config.openrouter.api_key_env)?;
            let llm = OpenAiLlm::openrouter(
                key,
                config.openrouter.app_url.clone(),
                config.openrouter.app_title.clone(),
            )?;
            Ok(Arc::new(llm))
        }
        Provider::Ollama => {
            let llm = OpenAiLlm::ollama(config.ollama.base_url.clone())?;
            Ok(Arc::new(llm))
        }
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
