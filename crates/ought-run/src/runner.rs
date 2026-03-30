use std::path::Path;

use ought_gen::GeneratedTest;

use crate::types::RunResult;

/// Trait implemented by each language-specific test runner.
///
/// Each runner collects results however is most natural for its ecosystem —
/// JUnit XML, structured JSON, harness APIs, or stdout parsing as last resort.
pub trait Runner: Send + Sync {
    /// Run a set of generated tests and collect results.
    fn run(&self, tests: &[GeneratedTest], test_dir: &Path) -> anyhow::Result<RunResult>;

    /// Check if the runner's test harness is available (e.g. `cargo` is installed).
    fn is_available(&self) -> bool;

    /// Human-readable name for this runner (e.g. "rust", "python").
    fn name(&self) -> &str;
}
