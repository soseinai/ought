use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::presets;

/// Configuration for a single runner entry in `ought.toml`, keyed by runner
/// name (e.g. `rust`, `python`, or any user-chosen identifier).
///
/// Most fields are optional: if `preset` is set (or the section name matches
/// a known preset), the preset's defaults fill them in. User-provided fields
/// always override the preset.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RunnerConfig {
    /// Optional preset name (`rust` | `python` | `typescript` | `go`). When
    /// set, the preset's defaults fill in any unset field below.
    #[serde(default)]
    pub preset: Option<String>,

    /// Command template. Ought substitutes these tokens before spawning:
    /// - `{test_dir}`   — where generated tests are written
    /// - `{junit_path}` — a tempfile path for JUnit XML output (format = junit-xml)
    /// - `{tap_path}`   — a tempfile path for TAP output (format = tap)
    /// - `{json_path}`  — a tempfile path for ought-json output (format = ought-json)
    #[serde(default)]
    pub command: Option<String>,

    /// Directory where generated test files are written. Relative paths are
    /// resolved against the directory containing `ought.toml`.
    #[serde(default)]
    pub test_dir: Option<PathBuf>,

    /// Format ought parses to reconstruct per-test pass/fail.
    #[serde(default)]
    pub format: Option<OutputFormat>,

    /// Fixed path (relative to `working_dir`) where the test harness writes
    /// its formatted output. When unset, ought inspects `command` for
    /// `{junit_path}` / `{tap_path}` / `{json_path}` and allocates a tempfile
    /// for the substitution; if no token is found, ought reads stdout.
    #[serde(default)]
    pub output_path: Option<PathBuf>,

    /// File extensions ought discovers as generated tests (e.g. `["rs"]`,
    /// `["ts", "js"]`).
    #[serde(default)]
    pub file_extensions: Option<Vec<String>>,

    /// Working directory for the spawned command, relative to the
    /// `ought.toml` directory. Defaults to the `ought.toml` directory.
    #[serde(default)]
    pub working_dir: Option<PathBuf>,

    /// Environment variables merged into the child process env. Values may
    /// contain the same substitution tokens as `command`.
    #[serde(default)]
    pub env: HashMap<String, String>,

    /// Explicit availability-probe command (a string executed via the shell
    /// parser). Defaults to the first token of `command`.
    #[serde(default)]
    pub available_check: Option<String>,
}

/// Declares how ought captures per-test results from a runner's command.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OutputFormat {
    /// JUnit XML — near-universal across test harnesses.
    JunitXml,
    /// TAP 13 stream.
    Tap,
    /// Native ought `RunResult` JSON (the escape hatch for custom runners).
    OughtJson,
    /// `cargo test` default stdout (lines like `test name ... ok`). Lets the
    /// built-in Rust preset work without requiring a third-party reporter
    /// like nextest or cargo2junit.
    CargoTest,
}

/// A `RunnerConfig` with every required field resolved — used internally by
/// `CliRunner`. Produced by `RunnerConfig::resolve`.
#[derive(Debug, Clone)]
pub struct ResolvedRunnerConfig {
    pub command: String,
    pub test_dir: PathBuf,
    pub format: OutputFormat,
    pub output_path: Option<PathBuf>,
    pub file_extensions: Vec<String>,
    pub working_dir: Option<PathBuf>,
    pub env: HashMap<String, String>,
    pub available_check: Option<String>,
}

impl RunnerConfig {
    /// Merge this config over an optional preset lookup and validate that all
    /// required fields are set. The `section_name` is used as a fallback
    /// preset lookup when `self.preset` is `None`, preserving backward-compat
    /// for configs that just write `[runner.rust]` with no other fields.
    pub fn resolve(&self, section_name: &str) -> anyhow::Result<ResolvedRunnerConfig> {
        // Determine the preset to use.
        let preset_name = self.preset.clone().or_else(|| {
            // Fall back to section name if it matches a known preset and the
            // user hasn't provided enough to stand on their own.
            if presets::preset(section_name).is_some() {
                Some(section_name.to_string())
            } else {
                None
            }
        });

        let base = if let Some(ref name) = preset_name {
            presets::preset(name).ok_or_else(|| {
                anyhow::anyhow!(
                    "unknown runner preset {name:?}; known presets: rust, python, typescript, go"
                )
            })?
        } else {
            RunnerConfig::default()
        };

        let command = self
            .command
            .clone()
            .or(base.command)
            .ok_or_else(|| anyhow::anyhow!("runner {section_name:?}: `command` is required (or set `preset`)"))?;

        let test_dir = self
            .test_dir
            .clone()
            .or(base.test_dir)
            .ok_or_else(|| anyhow::anyhow!("runner {section_name:?}: `test_dir` is required"))?;

        let format = self
            .format
            .or(base.format)
            .ok_or_else(|| anyhow::anyhow!("runner {section_name:?}: `format` is required (or set `preset`)"))?;

        let file_extensions = self
            .file_extensions
            .clone()
            .or(base.file_extensions)
            .ok_or_else(|| {
                anyhow::anyhow!("runner {section_name:?}: `file_extensions` is required (or set `preset`)")
            })?;

        let output_path = self.output_path.clone().or(base.output_path);
        let working_dir = self.working_dir.clone().or(base.working_dir);
        let available_check = self.available_check.clone().or(base.available_check);

        // Merge env: preset first, then user overrides.
        let mut env = base.env;
        for (k, v) in &self.env {
            env.insert(k.clone(), v.clone());
        }

        Ok(ResolvedRunnerConfig {
            command,
            test_dir,
            format,
            output_path,
            file_extensions,
            working_dir,
            env,
            available_check,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_uses_section_name_as_preset_fallback() {
        // Bare `[runner.python]` with only test_dir → preset=python is applied.
        let cfg = RunnerConfig {
            test_dir: Some(PathBuf::from("ought/ought-gen")),
            ..Default::default()
        };
        let resolved = cfg.resolve("python").expect("resolve");
        assert_eq!(resolved.format, OutputFormat::JunitXml);
        assert_eq!(resolved.file_extensions, vec!["py"]);
        assert!(resolved.command.contains("pytest"));
    }

    #[test]
    fn resolve_user_command_overrides_preset() {
        let cfg = RunnerConfig {
            preset: Some("python".into()),
            command: Some("my-pytest-wrapper --junit-xml={junit_path} {test_dir}".into()),
            test_dir: Some(PathBuf::from("tests")),
            ..Default::default()
        };
        let resolved = cfg.resolve("python").expect("resolve");
        assert_eq!(resolved.command, "my-pytest-wrapper --junit-xml={junit_path} {test_dir}");
        // Format still comes from preset.
        assert_eq!(resolved.format, OutputFormat::JunitXml);
    }

    #[test]
    fn resolve_requires_command_when_no_preset() {
        let cfg = RunnerConfig {
            test_dir: Some(PathBuf::from("tests")),
            ..Default::default()
        };
        // Section name `custom` is not a known preset → must set command.
        let err = cfg.resolve("custom").unwrap_err();
        assert!(err.to_string().contains("command"), "got: {err}");
    }

    #[test]
    fn resolve_user_env_merges_over_preset() {
        let cfg = RunnerConfig {
            preset: Some("typescript".into()),
            test_dir: Some(PathBuf::from("ought/ought-gen")),
            env: HashMap::from([("EXTRA".to_string(), "1".to_string())]),
            ..Default::default()
        };
        let resolved = cfg.resolve("typescript").expect("resolve");
        // Preset sets JEST_JUNIT_OUTPUT_FILE; user adds EXTRA.
        assert!(resolved.env.contains_key("JEST_JUNIT_OUTPUT_FILE"));
        assert_eq!(resolved.env.get("EXTRA"), Some(&"1".to_string()));
    }

    #[test]
    fn output_format_parses_from_toml_kebab_case() {
        let toml = r#"
            command = "cmd"
            test_dir = "tests"
            format = "junit-xml"
            file_extensions = ["rs"]
        "#;
        let cfg: RunnerConfig = toml::from_str(toml).unwrap();
        assert_eq!(cfg.format, Some(OutputFormat::JunitXml));
    }

    #[test]
    fn output_format_ought_json_parses() {
        let toml = r#"
            command = "cmd"
            test_dir = "tests"
            format = "ought-json"
            file_extensions = ["sh"]
        "#;
        let cfg: RunnerConfig = toml::from_str(toml).unwrap();
        assert_eq!(cfg.format, Some(OutputFormat::OughtJson));
    }
}
