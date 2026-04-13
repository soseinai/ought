use serde::{Deserialize, Serialize};

/// Configuration for the LLM test generator.
///
/// Composed into the aggregate `ought.toml` config by the CLI crate.
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

fn default_multiplier() -> f64 {
    1.0
}
fn default_parallelism() -> usize {
    1
}
