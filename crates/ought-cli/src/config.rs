//! Aggregate on-disk configuration for the `ought` CLI.
//!
//! `ought.toml` is a CLI concern — each domain crate owns the sub-config
//! struct(s) it cares about; this module composes them and handles loading
//! and discovery from the filesystem.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use ought_core::ContextConfig;
use ought_gen::GeneratorConfig;
use ought_mcp::McpConfig;
use ought_run::RunnerConfig;
use ought_spec::SpecsConfig;

/// Project-level configuration loaded from `ought.toml`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub project: ProjectConfig,
    #[serde(default)]
    pub specs: SpecsConfig,
    #[serde(default)]
    pub context: ContextConfig,
    pub generator: GeneratorConfig,
    #[serde(default)]
    pub runner: HashMap<String, RunnerConfig>,
    #[serde(default)]
    pub mcp: McpConfig,
}

/// Minimal project metadata used by the CLI for banners/version reporting.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectConfig {
    pub name: String,
    #[serde(default = "default_version")]
    pub version: String,
}

impl Config {
    /// Load config from an `ought.toml` file.
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| anyhow::anyhow!("failed to read {}: {}", path.display(), e))?;
        let config: Config = toml::from_str(&content)
            .map_err(|e| anyhow::anyhow!("failed to parse {}: {}", path.display(), e))?;
        Ok(config)
    }

    /// Discover `ought.toml` by walking up from the current directory.
    pub fn discover() -> anyhow::Result<(PathBuf, Self)> {
        let mut dir = std::env::current_dir()?;
        loop {
            let candidate = dir.join("ought.toml");
            if candidate.is_file() {
                let config = Self::load(&candidate)?;
                return Ok((candidate, config));
            }
            if !dir.pop() {
                anyhow::bail!("could not find ought.toml in any parent directory");
            }
        }
    }
}

fn default_version() -> String {
    "0.1.0".into()
}
