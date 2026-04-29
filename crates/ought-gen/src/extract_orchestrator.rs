//! Per-assignment in-process agent loops for spec extraction.
//!
//! Mirrors [`crate::orchestrator::Orchestrator`] but drives the extraction
//! tool set and embeds the `.ought.md` grammar into the system prompt so
//! the agent drafts against the canonical grammar that matches this
//! binary's parser.

use std::sync::Arc;

use tokio::sync::Semaphore;
use tokio::task::JoinSet;

use oharness_core::{CompletionReason, Task, Termination, TruncationLimit};
use oharness_llm::Llm;
use oharness_loop::{Agent, ReactLoop};

use crate::config::GeneratorConfig;
use crate::extract::{ExtractAssignment, ExtractReport, ExtractRunStatus};
use crate::extract_tool_set::ExtractToolSet;

/// The canonical `.ought.md` grammar, embedded at compile time so agents
/// draft against the grammar that matches this binary's parser.
///
/// `grammar.md` is a symlink to `../../docs/grammar.md` — the symlink keeps
/// the path inside the crate so `cargo publish`'s tarball verification can
/// find the file (cargo dereferences symlinks during packaging, so the
/// published `.crate` ships the actual grammar contents).
const GRAMMAR_MD: &str = include_str!("../grammar.md");

pub struct ExtractOrchestrator {
    config: GeneratorConfig,
    verbose: bool,
}

impl ExtractOrchestrator {
    pub fn new(config: GeneratorConfig, verbose: bool) -> Self {
        Self { config, verbose }
    }

    /// Run all assignments and return per-assignment reports.
    pub async fn run(
        self,
        assignments: Vec<ExtractAssignment>,
    ) -> anyhow::Result<Vec<ExtractReport>> {
        if assignments.is_empty() {
            return Ok(vec![]);
        }

        let llm = crate::orchestrator::build_llm(&self.config).await?;
        let max_turns = self.config.max_turns;
        let read_source_limit = self.config.read_source_limit_bytes;

        let parallelism = self.config.parallelism.max(1);
        let sem = Arc::new(Semaphore::new(parallelism));
        let mut tasks = JoinSet::new();

        for assignment in assignments {
            let permit = sem.clone().acquire_owned().await?;
            let llm = llm.clone();
            let verbose = self.verbose;

            tasks.spawn(async move {
                let _permit = permit;
                run_one_assignment(assignment, llm, max_turns, read_source_limit, verbose).await
            });
        }

        let mut reports = Vec::new();
        while let Some(joined) = tasks.join_next().await {
            match joined {
                Ok(report) => reports.push(report),
                Err(e) => reports.push(ExtractReport {
                    errors: vec![format!("extract agent task panicked: {}", e)],
                    status: ExtractRunStatus::Errored,
                    ..ExtractReport::default()
                }),
            }
        }
        Ok(reports)
    }
}

async fn run_one_assignment(
    assignment: ExtractAssignment,
    llm: Arc<dyn Llm>,
    max_turns: u32,
    read_source_limit_bytes: usize,
    verbose: bool,
) -> ExtractReport {
    let assignment_id = assignment.id.clone();
    let group_count = assignment.groups.len();

    if verbose {
        eprintln!(
            "  [extract agent {}] starting: {} groups",
            assignment_id, group_count
        );
    }

    let tools_concrete = Arc::new(ExtractToolSet::with_limits(
        assignment.clone(),
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
            return ExtractReport {
                assignment_id,
                status: ExtractRunStatus::Errored,
                errors: vec![format!("agent build: {}", e)],
                ..ExtractReport::default()
            };
        }
    };

    let task = Task::new(initial).with_id(assignment_id.clone());
    let result = agent.run(task).await;
    let usage_snapshot = tools_concrete.usage();

    let mut report = ExtractReport {
        assignment_id: assignment_id.clone(),
        written: usage_snapshot.written,
        write_errors: usage_snapshot.write_errors,
        ..ExtractReport::default()
    };

    match result {
        Ok(outcome) => {
            report.status = map_termination(&outcome.termination);
            report.turns = outcome.usage.turns;
            report.usage_input_tokens = outcome.usage.tokens_input as u32;
            report.usage_output_tokens = outcome.usage.tokens_output as u32;
            report.usage_cache_read_tokens = outcome.usage.tokens_cache_read as u32;
            report.usage_cache_creation_tokens = outcome.usage.tokens_cache_create as u32;
            if matches!(report.status, ExtractRunStatus::Errored)
                && let Termination::Failed { error, .. } = &outcome.termination
            {
                report.errors.push(error.message.clone());
            }
            if verbose {
                eprintln!(
                    "  [extract agent {}] finished: {} written, {} write errors, {} turns",
                    assignment_id,
                    report.written.len(),
                    report.write_errors.len(),
                    report.turns
                );
            }
        }
        Err(e) => {
            report.status = ExtractRunStatus::Errored;
            report.errors.push(format!("agent loop failed: {}", e));
            if verbose {
                eprintln!("  [extract agent {}] errored: {}", assignment_id, e);
            }
        }
    }

    report
}

fn map_termination(t: &Termination) -> ExtractRunStatus {
    match t {
        Termination::Completed {
            reason: CompletionReason::EndTurn | CompletionReason::StopSequence(_),
        } => ExtractRunStatus::Completed,
        Termination::Truncated {
            limit: TruncationLimit::MaxTurns(_),
        } => ExtractRunStatus::MaxTurnsExceeded,
        Termination::Truncated {
            limit: TruncationLimit::MaxTokens,
        } => ExtractRunStatus::Truncated,
        Termination::Truncated {
            limit: TruncationLimit::Budget(_),
        } => ExtractRunStatus::ContextExhausted,
        Termination::Truncated {
            limit: TruncationLimit::Timeout,
        } => ExtractRunStatus::Errored,
        Termination::Failed { .. } | Termination::Interrupted { .. } => ExtractRunStatus::Errored,
    }
}

fn build_system_prompt(assignment: &ExtractAssignment) -> String {
    let mut p = String::from(
        "You are a spec-extraction agent for the ought behavioral test framework.\n\n\
         Your job: read source code and produce one or more draft `.ought.md` specs that \
         describe the behaviors the code implements, so a human can refine them into the \
         project's source-of-truth requirements.\n\n",
    );

    p.push_str("## Workflow\n\n");
    p.push_str(
        "1. Call get_assignment to see your assigned groups (each group = one spec file).\n\
         2. For each group, call read_source on every file in source_files.\n\
         3. Draft the spec content as `.ought.md` markdown.\n\
         4. Call validate_spec with the draft. If it reports errors, revise and revalidate.\n\
         5. Call write_spec with the validated draft. The server re-validates before writing.\n\
         6. Call report_progress at the end.\n\n",
    );

    p.push_str("## Spec style\n\n");
    p.push_str(
        "- Start with `# <Title>` matching the group's title.\n\
         - Add a `context:` line describing what this spec covers in one sentence.\n\
         - Add a `source:` line listing the source file(s) or directory this spec describes.\n\
         - Group clauses under `##` headings by coherent behavior, not by source file.\n\
         - Use the strongest keyword justified by what the code actually enforces:\n\
           * **MUST** / **MUST NOT** for invariants the code enforces unconditionally\n\
           * **SHOULD** for best-effort behaviors (fallbacks, performance hints)\n\
           * **MAY** for optional features you see in the code\n\
           * **GIVEN** to scope clauses that only apply under a precondition\n\
         - If you're unsure whether something is a real requirement vs. an implementation \
           detail, mark it `**PENDING MUST**` so a human can decide.\n\
         - Prefer fewer, sharper clauses over many vague ones. One clause per observable \
           behavior.\n\
         - Do NOT describe internal helpers or private implementation mechanics. Describe \
           what the module's public surface promises its callers.\n\n",
    );

    p.push_str(&format!(
        "## Output paths\n\nWrite each group's spec to `<specs_root>/<target_spec_path>`. \
         specs_root for this run is `{}`.\n\n",
        assignment.specs_root
    ));
    if assignment.dry_run {
        p.push_str(
            "DRY RUN is active: write_spec will print the spec to stdout and NOT touch \
             disk. You should still call validate_spec first.\n\n",
        );
    }

    p.push_str("## Grammar reference\n\n");
    p.push_str(
        "The `.ought.md` format is a strict superset of markdown. validate_spec parses \
         with the canonical parser and refuses anything it cannot understand. Below is \
         the full EBNF grammar and an example — consult it while drafting.\n\n",
    );
    p.push_str("```\n");
    p.push_str(GRAMMAR_MD);
    p.push_str("\n```\n");

    p
}

fn build_initial_user_message(assignment: &ExtractAssignment) -> String {
    format!(
        "Begin extraction assignment {}. Call `get_assignment` first to see your work, \
         then proceed.",
        assignment.id
    )
}
