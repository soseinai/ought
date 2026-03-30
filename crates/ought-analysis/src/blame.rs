use ought_gen::Generator;
use ought_run::RunResult;
use ought_spec::{ClauseId, SpecGraph};

use crate::types::BlameResult;

/// Explain why a clause is failing by correlating with git history.
///
/// Finds when the clause last passed, what commits changed since,
/// and uses the LLM to produce a causal narrative.
pub fn blame(
    _clause_id: &ClauseId,
    _specs: &SpecGraph,
    _results: &RunResult,
    _generator: &dyn Generator,
) -> anyhow::Result<BlameResult> {
    todo!()
}
