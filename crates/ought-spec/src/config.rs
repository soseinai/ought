use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Project-level configuration from `ought.toml`.
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectConfig {
    pub name: String,
    #[serde(default = "default_version")]
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecsConfig {
    #[serde(default = "default_roots")]
    pub roots: Vec<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextConfig {
    #[serde(default)]
    pub search_paths: Vec<PathBuf>,
    #[serde(default)]
    pub exclude: Vec<String>,
    #[serde(default = "default_max_files")]
    pub max_files: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratorConfig {
    pub provider: String,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub tolerance: ToleranceConfig,
    #[serde(default = "default_parallelism")]
    pub parallelism: usize,
}

impl Default for GeneratorConfig {
    fn default() -> Self {
        Self {
            provider: "claude".to_string(),
            model: None,
            tolerance: ToleranceConfig::default(),
            parallelism: default_parallelism(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToleranceConfig {
    #[serde(default = "default_multiplier")]
    pub must_by_multiplier: f64,
}

impl Default for ToleranceConfig {
    fn default() -> Self {
        Self {
            must_by_multiplier: default_multiplier(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunnerConfig {
    pub command: String,
    pub test_dir: PathBuf,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct McpConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub transport: McpTransport,
}

/// Transport protocol used to expose the MCP server.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum McpTransport {
    /// Standard input/output — default for local IDE integration.
    #[default]
    Stdio,
    /// Server-Sent Events over HTTP — for remote clients.
    Sse,
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

impl Default for SpecsConfig {
    fn default() -> Self {
        Self {
            roots: default_roots(),
        }
    }
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

fn default_version() -> String {
    "0.1.0".into()
}
fn default_roots() -> Vec<PathBuf> {
    vec![PathBuf::from("ought/")]
}
fn default_max_files() -> usize {
    50
}
fn default_multiplier() -> f64 {
    1.0
}
fn default_parallelism() -> usize {
    1
}
