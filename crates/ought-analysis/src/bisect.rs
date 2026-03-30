use ought_run::Runner;
use ought_spec::{ClauseId, SpecGraph};

use crate::types::BisectResult;

/// Options for the bisect command.
pub struct BisectOptions {
    /// Limit the search to a git revision range (e.g. "abc123..def456").
    pub range: Option<String>,
    /// Regenerate tests at each commit instead of using the current manifest.
    pub regenerate: bool,
}

/// Binary search through git history to find the commit that broke a clause.
///
/// Always restores the working tree to its original state after completion.
pub fn bisect(
    _clause_id: &ClauseId,
    _specs: &SpecGraph,
    _runner: &dyn Runner,
    _options: &BisectOptions,
) -> anyhow::Result<BisectResult> {
    todo!()
}
