use serde::{Deserialize, Serialize};

/// A unit of work assigned to a single agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentAssignment {
    pub id: String,
    pub project_root: String,
    pub config_path: String,
    pub test_dir: String,
    pub target_language: String,
    /// Source file/directory paths from spec metadata that the agent should read
    /// to understand the code under test.
    pub source_paths: Vec<String>,
    pub groups: Vec<AssignmentGroup>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssignmentGroup {
    pub section_path: String,
    pub clauses: Vec<AssignmentClause>,
    pub conditions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssignmentClause {
    pub id: String,
    pub keyword: String,
    pub text: String,
    pub condition: Option<String>,
    pub temporal: Option<String>,
    pub content_hash: String,
    pub hints: Vec<String>,
    pub otherwise: Vec<AssignmentClause>,
}

/// Results from one agent's work on one assignment.
///
/// Populated from the `GenerateToolSet`'s recorded state (what tests it
/// actually wrote, what failed) plus the agent loop's outcome — never
/// reconstructed from log scraping.
#[derive(Debug, Default, Clone)]
pub struct AgentReport {
    /// Identifier of the assignment this report covers.
    pub assignment_id: String,
    /// Clause ids that had test code written for them.
    pub generated: Vec<String>,
    /// Per-clause write failures: (clause_id, message).
    pub write_errors: Vec<(String, String)>,
    /// How the agent loop terminated.
    pub status: AgentRunStatus,
    /// Number of model turns consumed.
    pub turns: u32,
    /// Token usage summed across all turns.
    pub usage_input_tokens: u32,
    pub usage_output_tokens: u32,
    pub usage_cache_read_tokens: u32,
    pub usage_cache_creation_tokens: u32,
    /// Free-form errors from the orchestrator itself (bad config, agent
    /// loop failure surfaced as error, etc.). Per-clause failures live
    /// in `write_errors`.
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum AgentRunStatus {
    /// Agent loop hadn't been run yet (only when constructing default
    /// reports for skipped or pre-failed assignments).
    #[default]
    NotRun,
    /// Model emitted `end_turn`.
    Completed,
    /// Hit `max_turns` without completing.
    MaxTurnsExceeded,
    /// Model truncated due to `max_tokens` on its final turn.
    Truncated,
    /// Per-request input tokens crossed the configured budget; the loop
    /// terminated pre-emptively.
    ContextExhausted,
    /// Agent loop returned an error (LLM auth/rate-limit/etc.).
    Errored,
}
