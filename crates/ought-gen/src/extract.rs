//! Reverse-engineer `.ought.md` specs from an existing codebase.
//!
//! Mirrors the shape of [`crate::agent`] but for spec extraction rather
//! than test generation: a single agent is handed a list of source files
//! and the relative path where the resulting spec should live.

use serde::{Deserialize, Serialize};

/// A unit of work assigned to a single extraction agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractAssignment {
    pub id: String,
    pub project_root: String,
    pub config_path: String,
    /// Absolute path to the directory where `.ought.md` files should land.
    pub specs_root: String,
    /// When true, `write_spec` prints to stdout instead of writing to disk.
    pub dry_run: bool,
    /// When true, existing `.ought.md` files may be overwritten. The CLI
    /// pre-flight already filters out pre-existing paths when `force=false`,
    /// but the tool primitive checks again defensively.
    pub force: bool,
    pub groups: Vec<ExtractGroup>,
}

/// One output `.ought.md` file's worth of work.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractGroup {
    /// Human-friendly title for the spec (becomes the `# H1` by convention).
    pub title: String,
    /// Path to the target `.ought.md` file, relative to `specs_root`.
    pub target_spec_path: String,
    /// Source files (relative to `project_root`) that contribute behaviors
    /// to this group.
    pub source_files: Vec<String>,
}

/// Results from one extraction agent's work on one assignment.
///
/// Populated from the `ExtractToolSet`'s recorded state and the agent
/// loop's outcome — never reconstructed from log scraping.
#[derive(Debug, Default, Clone)]
pub struct ExtractReport {
    pub assignment_id: String,
    /// Target paths (relative to `specs_root`) that were written or
    /// previewed under `--dry-run`.
    pub written: Vec<String>,
    /// Per-target write failures: (target_path, message).
    pub write_errors: Vec<(String, String)>,
    pub status: ExtractRunStatus,
    pub turns: u32,
    pub usage_input_tokens: u32,
    pub usage_output_tokens: u32,
    pub usage_cache_read_tokens: u32,
    pub usage_cache_creation_tokens: u32,
    /// Orchestrator-level errors (bad config, agent loop failure).
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ExtractRunStatus {
    #[default]
    NotRun,
    Completed,
    MaxTurnsExceeded,
    Truncated,
    ContextExhausted,
    Errored,
}
