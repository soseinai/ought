use ought_gen::Generator;
use ought_spec::SpecGraph;

use crate::types::AuditResult;

/// Analyze all specs for contradictions, gaps, and coherence issues.
///
/// Detects: contradictory clauses, MUST BY deadline conflicts,
/// MUST ALWAYS invariant conflicts, overlapping GIVEN conditions
/// with contradictory obligations, and missing OTHERWISE chains.
pub fn audit(_specs: &SpecGraph, _generator: &dyn Generator) -> anyhow::Result<AuditResult> {
    todo!()
}
