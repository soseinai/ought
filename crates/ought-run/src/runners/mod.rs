//! Runner factory.
//!
//! As of the generic CLI runner refactor, every runner is a [`CliRunner`]
//! configured via `[runner.<name>]` in `ought.toml`. A small set of built-in
//! [`presets`](crate::presets) (rust, python, typescript, go) ship pre-filled
//! defaults; everything else is user-provided.

use std::path::Path;

use crate::cli_runner::CliRunner;
use crate::config::RunnerConfig;
use crate::runner::Runner;

/// Build a runner from a named config block.
///
/// `config_dir` is the directory containing `ought.toml`; relative paths in
/// `config.working_dir` resolve against it.
pub fn from_config(
    name: &str,
    config: &RunnerConfig,
    config_dir: &Path,
) -> anyhow::Result<Box<dyn Runner>> {
    let resolved = config.resolve(name)?;
    Ok(Box::new(CliRunner::new(
        name,
        resolved,
        config_dir.to_path_buf(),
    )))
}
