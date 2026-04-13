use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Configuration for a single language's test runner, keyed by runner name
/// (e.g. `rust`, `python`) in the aggregate `ought.toml` config.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunnerConfig {
    pub command: String,
    pub test_dir: PathBuf,
}
