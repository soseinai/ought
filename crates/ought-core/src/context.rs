use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Where the project's source code lives and how it should be included as
/// context for LLM-driven commands (generate, survey, etc.).
///
/// This is cross-cutting because multiple subsystems care about the project's
/// source layout (generation for prompt context, the CLI's watch command for
/// file-system watching, future analysis passes, etc.).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextConfig {
    #[serde(default)]
    pub search_paths: Vec<PathBuf>,
    #[serde(default)]
    pub exclude: Vec<String>,
    #[serde(default = "default_max_files")]
    pub max_files: usize,
}

impl Default for ContextConfig {
    fn default() -> Self {
        Self {
            search_paths: vec![],
            exclude: vec![],
            max_files: default_max_files(),
        }
    }
}

fn default_max_files() -> usize {
    50
}
