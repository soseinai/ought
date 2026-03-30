use std::path::PathBuf;

use ought_gen::Generator;
use ought_spec::SpecGraph;

use crate::types::SurveyResult;

/// Discover behaviors in source code not covered by any spec clause.
///
/// Reads source files, reads all specs, and uses the LLM to identify
/// public behaviors, APIs, and logic branches that lack corresponding clauses.
pub fn survey(
    _specs: &SpecGraph,
    _paths: &[PathBuf],
    _generator: &dyn Generator,
) -> anyhow::Result<SurveyResult> {
    todo!()
}
