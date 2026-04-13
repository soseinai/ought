use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Where on disk to find `.ought.md` spec files.
///
/// Owned by the spec crate because it is directly about spec discovery —
/// the parser needs roots to walk. The aggregate `ought.toml` config lives
/// in the CLI crate and composes this struct.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecsConfig {
    #[serde(default = "default_roots")]
    pub roots: Vec<PathBuf>,
}

impl Default for SpecsConfig {
    fn default() -> Self {
        Self {
            roots: default_roots(),
        }
    }
}

fn default_roots() -> Vec<PathBuf> {
    vec![PathBuf::from("ought/")]
}
