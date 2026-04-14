//! The generic CLI runner.
//!
//! Spawns a user-configured command, captures its output in a declared
//! format (JUnit XML, TAP, or native ought-json), and reconstructs
//! `RunResult`. All language-specific behavior lives in presets +
//! user-provided config, not in code — so users can plug in any CLI-driven
//! test harness via `ought.toml` without modifying ought source.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use ought_gen::GeneratedTest;
use ought_spec::ClauseId;

use crate::config::{OutputFormat, ResolvedRunnerConfig};
use crate::formats::{self, clause_id_to_test_name};
use crate::runner::Runner;
use crate::types::{RunResult, TestDetails, TestResult, TestStatus};

pub struct CliRunner {
    name: String,
    config: ResolvedRunnerConfig,
    /// Base directory for resolving relative paths (typically the directory
    /// containing `ought.toml`).
    config_dir: PathBuf,
}

impl CliRunner {
    pub fn new(name: impl Into<String>, config: ResolvedRunnerConfig, config_dir: PathBuf) -> Self {
        Self {
            name: name.into(),
            config,
            config_dir,
        }
    }

    pub fn config(&self) -> &ResolvedRunnerConfig {
        &self.config
    }

    /// Check if a command exists on PATH.
    fn command_exists(cmd: &str) -> bool {
        Command::new("which")
            .arg(cmd)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    fn resolved_working_dir(&self) -> PathBuf {
        match &self.config.working_dir {
            Some(p) if p.is_absolute() => p.clone(),
            Some(p) => self.config_dir.join(p),
            None => self.config_dir.clone(),
        }
    }

    /// Resolve the availability probe: user-provided command, or first token
    /// of `command`.
    fn availability_probe(&self) -> Option<String> {
        if let Some(cmd) = &self.config.available_check {
            return Some(cmd.clone());
        }
        shlex::split(&self.config.command)
            .and_then(|parts| parts.into_iter().next())
    }
}

/// Tokens substituted into `command` and `env` values.
struct Substitutions {
    test_dir: String,
    /// Present only when the format needs a file and user didn't pin one via
    /// `output_path`.
    junit_path: Option<String>,
    tap_path: Option<String>,
    json_path: Option<String>,
}

impl Substitutions {
    fn apply(&self, template: &str) -> String {
        let mut out = template.replace("{test_dir}", &self.test_dir);
        if let Some(p) = &self.junit_path {
            out = out.replace("{junit_path}", p);
        }
        if let Some(p) = &self.tap_path {
            out = out.replace("{tap_path}", p);
        }
        if let Some(p) = &self.json_path {
            out = out.replace("{json_path}", p);
        }
        out
    }
}

impl Runner for CliRunner {
    fn run(&self, tests: &[GeneratedTest], test_dir: &Path) -> anyhow::Result<RunResult> {
        if tests.is_empty() {
            return Ok(RunResult {
                results: Vec::new(),
                total_duration: Duration::ZERO,
            });
        }

        if !self.is_available() {
            anyhow::bail!(
                "runner {:?} is not available — check that the required tool is installed on PATH",
                self.name
            );
        }

        // Build name→ClauseId map up front.
        let mut name_to_clause: HashMap<String, ClauseId> = HashMap::new();
        for test in tests {
            name_to_clause.insert(clause_id_to_test_name(&test.clause_id), test.clause_id.clone());
        }

        // Write generated test files to the test directory.
        fs::create_dir_all(test_dir)?;
        for test in tests {
            let dest = test_dir.join(&test.file_path);
            if let Some(parent) = dest.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(&dest, &test.code)?;
        }

        // Build substitutions. For file-based formats we may allocate a
        // tempfile that the spawned command writes to.
        let working_dir = self.resolved_working_dir();
        let test_dir_str = test_dir.to_string_lossy().to_string();

        // Only allocate a tempfile for the substitution token matching our
        // format, and only if the user hasn't pinned a fixed `output_path`.
        let (junit_tmp, tap_tmp, json_tmp) = allocate_tempfiles(
            &self.config.format,
            self.config.output_path.is_some(),
            &self.config.command,
            &self.config.env,
        )?;

        let subs = Substitutions {
            test_dir: test_dir_str,
            junit_path: junit_tmp
                .as_ref()
                .map(|t| t.path().to_string_lossy().to_string()),
            tap_path: tap_tmp
                .as_ref()
                .map(|t| t.path().to_string_lossy().to_string()),
            json_path: json_tmp
                .as_ref()
                .map(|t| t.path().to_string_lossy().to_string()),
        };

        // Parse and substitute the command.
        let expanded = subs.apply(&self.config.command);
        let parts = shlex::split(&expanded).ok_or_else(|| {
            anyhow::anyhow!("failed to parse runner command {:?}", self.config.command)
        })?;
        let (program, args) = parts.split_first().ok_or_else(|| {
            anyhow::anyhow!("runner command {:?} is empty", self.config.command)
        })?;

        let mut cmd = Command::new(program);
        cmd.args(args);
        cmd.current_dir(&working_dir);

        // Merge env (with substitution).
        for (k, v) in &self.config.env {
            cmd.env(k, subs.apply(v));
        }

        let start = Instant::now();
        let output = cmd.output()?;
        let total_duration = start.elapsed();

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        // Resolve where to read format output from.
        let output_source = read_output(
            &self.config.format,
            &self.config.output_path,
            junit_tmp.as_ref().map(|t| t.path()),
            tap_tmp.as_ref().map(|t| t.path()),
            json_tmp.as_ref().map(|t| t.path()),
            &working_dir,
            &stdout,
        );

        // Parse per format.
        let mut results: Vec<TestResult> = match (self.config.format, output_source) {
            (OutputFormat::JunitXml, Ok(data)) => {
                formats::junit::parse(&data, &name_to_clause).unwrap_or_default()
            }
            (OutputFormat::Tap, Ok(data)) => formats::tap::parse(&data, &name_to_clause),
            (OutputFormat::CargoTest, Ok(data)) => {
                formats::cargo_test::parse(&data, &name_to_clause)
            }
            (OutputFormat::OughtJson, Ok(data)) => match formats::json::parse(&data) {
                Ok(run_result) => run_result.results,
                Err(_) => Vec::new(),
            },
            (_, Err(_)) => Vec::new(),
        };

        // Fallback: if parsing produced nothing but the command failed, mark
        // every generated test as Errored with the captured stderr.
        if results.is_empty() && !output.status.success() {
            let error_msg = if stderr.trim().is_empty() {
                stdout.clone()
            } else {
                stderr.clone()
            };
            let error_msg = error_msg.trim();
            for test in tests {
                results.push(TestResult {
                    clause_id: test.clause_id.clone(),
                    status: TestStatus::Errored,
                    message: Some(format!("{} failed: {}", self.name, error_msg)),
                    duration: Duration::ZERO,
                    details: TestDetails {
                        failure_message: Some(error_msg.to_string()),
                        stack_trace: None,
                        iterations: None,
                        measured_duration: None,
                    },
                });
            }
        }

        Ok(RunResult {
            results,
            total_duration,
        })
    }

    fn is_available(&self) -> bool {
        match self.availability_probe() {
            Some(probe) => Self::command_exists(&probe),
            None => false,
        }
    }

    fn name(&self) -> &str {
        &self.name
    }
}

/// Allocate tempfiles for whichever `{junit_path}` / `{tap_path}` /
/// `{json_path}` tokens actually appear in `command` or `env` values, but
/// only when the format matches and `output_path` is not pinned.
fn allocate_tempfiles(
    format: &OutputFormat,
    output_pinned: bool,
    command: &str,
    env: &HashMap<String, String>,
) -> anyhow::Result<(
    Option<tempfile::NamedTempFile>,
    Option<tempfile::NamedTempFile>,
    Option<tempfile::NamedTempFile>,
)> {
    if output_pinned {
        return Ok((None, None, None));
    }

    let token_mentioned = |tok: &str| -> bool {
        command.contains(tok) || env.values().any(|v| v.contains(tok))
    };

    let junit = if matches!(format, OutputFormat::JunitXml) && token_mentioned("{junit_path}") {
        Some(tempfile::Builder::new().suffix(".xml").tempfile()?)
    } else {
        None
    };
    let tap = if matches!(format, OutputFormat::Tap) && token_mentioned("{tap_path}") {
        Some(tempfile::Builder::new().suffix(".tap").tempfile()?)
    } else {
        None
    };
    let json = if matches!(format, OutputFormat::OughtJson) && token_mentioned("{json_path}") {
        Some(tempfile::Builder::new().suffix(".json").tempfile()?)
    } else {
        None
    };
    Ok((junit, tap, json))
}

/// Return the string we should hand to the format parser.
///
/// Priority:
/// 1. `output_path` pinned in config (relative to `working_dir`)
/// 2. The tempfile associated with the format's substitution token (if any)
/// 3. Fall back to stdout
fn read_output(
    format: &OutputFormat,
    output_path: &Option<PathBuf>,
    junit_tmp: Option<&Path>,
    tap_tmp: Option<&Path>,
    json_tmp: Option<&Path>,
    working_dir: &Path,
    stdout: &str,
) -> std::io::Result<String> {
    if let Some(p) = output_path {
        let full = if p.is_absolute() {
            p.clone()
        } else {
            working_dir.join(p)
        };
        return fs::read_to_string(full);
    }
    let tmp = match format {
        OutputFormat::JunitXml => junit_tmp,
        OutputFormat::Tap => tap_tmp,
        OutputFormat::OughtJson => json_tmp,
        // cargo test writes directly to stdout and has no tempfile variant.
        OutputFormat::CargoTest => None,
    };
    if let Some(path) = tmp {
        return fs::read_to_string(path);
    }
    Ok(stdout.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::RunnerConfig;

    #[test]
    fn availability_probe_defaults_to_first_token() {
        let cfg = RunnerConfig {
            command: Some("pytest -v {test_dir}".into()),
            test_dir: Some(PathBuf::from("t")),
            format: Some(OutputFormat::JunitXml),
            file_extensions: Some(vec!["py".into()]),
            ..Default::default()
        };
        let resolved = cfg.resolve("custom").unwrap();
        let r = CliRunner::new("custom", resolved, PathBuf::from("."));
        assert_eq!(r.availability_probe().as_deref(), Some("pytest"));
    }

    #[test]
    fn substitutions_apply() {
        let subs = Substitutions {
            test_dir: "ought/gen".into(),
            junit_path: Some("/tmp/a.xml".into()),
            tap_path: None,
            json_path: None,
        };
        assert_eq!(
            subs.apply("pytest --junit-xml={junit_path} {test_dir}"),
            "pytest --junit-xml=/tmp/a.xml ought/gen"
        );
    }
}
