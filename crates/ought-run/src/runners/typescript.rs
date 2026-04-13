use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

use ought_gen::GeneratedTest;
use ought_spec::ClauseId;

use crate::runner::Runner;
use crate::types::{RunResult, TestDetails, TestResult, TestStatus};

pub struct TypeScriptRunner;

/// Check if a command exists on PATH using `which`.
fn command_exists(cmd: &str) -> bool {
    Command::new("which")
        .arg(cmd)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Convert a `ClauseId` like `auth::login::must_return_jwt` into the jest
/// test name: `test_auth__login__must_return_jwt`. Double-underscore preserves
/// section boundaries so the mapping is reversible by `test_name_to_clause_id`.
fn clause_id_to_test_name(clause_id: &ClauseId) -> String {
    format!("test_{}", clause_id.0.replace("::", "__"))
}

/// Recover a `ClauseId` from a test name produced by `clause_id_to_test_name`.
fn test_name_to_clause_id(test_name: &str) -> ClauseId {
    let stripped = test_name.strip_prefix("test_").unwrap_or(test_name);
    ClauseId(stripped.replace("__", "::"))
}

/// Parse jest --verbose output.
///
/// Jest verbose output looks like:
/// ```text
///   ✓ test_name (5 ms)
///   ✕ test_name (10 ms)
///   PASS test_file.test.ts
///   FAIL test_file.test.ts
/// ```
///
/// Or with `--verbose`:
/// ```text
///   ✓ auth_login_must_return_jwt (5 ms)
///   ✕ auth_login_must_validate_token (10 ms)
/// ```
fn parse_jest_output(
    output: &str,
    name_to_clause: &HashMap<String, ClauseId>,
) -> Vec<TestResult> {
    let mut results = Vec::new();

    for line in output.lines() {
        let trimmed = line.trim();

        // Jest verbose uses ✓ for pass and ✕ (or ×) for fail
        let (status, rest) = if let Some(rest) = trimmed.strip_prefix("✓ ").or_else(|| trimmed.strip_prefix("✓ ")) {
            (TestStatus::Passed, rest)
        } else if let Some(rest) = trimmed.strip_prefix("✕ ").or_else(|| trimmed.strip_prefix("× ")) {
            (TestStatus::Failed, rest)
        } else if let Some(rest) = trimmed.strip_prefix("○ ") {
            (TestStatus::Skipped, rest)
        } else if trimmed.starts_with("PASS ") || trimmed.starts_with("FAIL ") {
            // File-level summary, skip
            continue;
        } else {
            continue;
        };

        // Strip timing info: " (5 ms)" from the end
        let test_name = if let Some(idx) = rest.rfind(" (") {
            rest[..idx].trim()
        } else {
            rest.trim()
        };

        let clause_id = name_to_clause
            .get(test_name)
            .cloned()
            .unwrap_or_else(|| test_name_to_clause_id(test_name));

        let message = match status {
            TestStatus::Failed => Some("test failed".to_string()),
            TestStatus::Errored => Some("test errored".to_string()),
            _ => None,
        };

        results.push(TestResult {
            clause_id,
            status,
            message,
            duration: Duration::ZERO,
            details: TestDetails::default(),
        });
    }

    results
}

impl Runner for TypeScriptRunner {
    fn run(&self, tests: &[GeneratedTest], test_dir: &Path) -> anyhow::Result<RunResult> {
        if tests.is_empty() {
            return Ok(RunResult {
                results: Vec::new(),
                total_duration: Duration::ZERO,
            });
        }

        if !self.is_available() {
            anyhow::bail!("npx is not available on PATH");
        }

        // Build a mapping from test name -> ClauseId.
        let mut name_to_clause: HashMap<String, ClauseId> = HashMap::new();
        for test in tests {
            let test_name = clause_id_to_test_name(&test.clause_id);
            name_to_clause.insert(test_name, test.clause_id.clone());
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

        let start = Instant::now();

        let output = Command::new("npx")
            .arg("jest")
            .arg("--verbose")
            .arg(test_dir)
            .output()?;

        let total_duration = start.elapsed();

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        // Jest may output to stderr
        let combined = format!("{}\n{}", stdout, stderr);
        let mut results = parse_jest_output(&combined, &name_to_clause);

        if results.is_empty() && !output.status.success() {
            let error_msg = if stderr.is_empty() {
                stdout.to_string()
            } else {
                stderr.to_string()
            };

            for test in tests {
                results.push(TestResult {
                    clause_id: test.clause_id.clone(),
                    status: TestStatus::Errored,
                    message: Some(format!("jest failed: {}", error_msg.trim())),
                    duration: Duration::ZERO,
                    details: TestDetails {
                        failure_message: Some(error_msg.trim().to_string()),
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
        command_exists("npx")
    }

    fn name(&self) -> &str {
        "typescript"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_jest_output() {
        let output = "\
 PASS src/tests/auth.test.ts
  ✓ auth_login_must_return_jwt (5 ms)
  ✕ auth_login_must_validate_token (10 ms)
  ○ auth_login_may_cache

Test Suites: 1 passed, 1 total
Tests:       1 failed, 1 passed, 1 skipped, 3 total
";
        let mut map = HashMap::new();
        map.insert(
            "auth_login_must_return_jwt".to_string(),
            ClauseId("auth::login::must_return_jwt".to_string()),
        );
        map.insert(
            "auth_login_must_validate_token".to_string(),
            ClauseId("auth::login::must_validate_token".to_string()),
        );
        map.insert(
            "auth_login_may_cache".to_string(),
            ClauseId("auth::login::may_cache".to_string()),
        );

        let results = parse_jest_output(output, &map);
        assert_eq!(results.len(), 3);
        assert_eq!(results[0].status, TestStatus::Passed);
        assert_eq!(results[1].status, TestStatus::Failed);
        assert_eq!(results[2].status, TestStatus::Skipped);
    }
}
