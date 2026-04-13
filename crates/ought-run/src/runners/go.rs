use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

use ought_gen::GeneratedTest;
use ought_spec::ClauseId;

use crate::runner::Runner;
use crate::types::{RunResult, TestDetails, TestResult, TestStatus};

pub struct GoRunner;

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

/// Convert a `ClauseId` like `auth::login::must_return_jwt` into the Go test
/// function name: `Testauth__login__must_return_jwt`. The `Test` prefix is
/// required by Go's test discovery; sections are separated with double
/// underscore so the mapping is reversible by `test_name_to_clause_id`.
///
/// Note: Go convention is CamelCase, but since generated code controls the name
/// we use snake_case after the `Test` prefix for consistency with other runners.
fn clause_id_to_test_name(clause_id: &ClauseId) -> String {
    format!("Test{}", clause_id.0.replace("::", "__"))
}

/// Best-effort fallback: wrap the test name as a `ClauseId` directly. This is only
/// used when the HashMap lookup fails; strips the `Test` prefix and maps
/// `__` → `::` to mirror `clause_id_to_test_name`.
fn test_name_to_clause_id(test_name: &str) -> ClauseId {
    let stripped = test_name.strip_prefix("Test").unwrap_or(test_name);
    ClauseId(stripped.replace("__", "::"))
}

/// Parse `go test -v` output.
///
/// Lines look like:
/// ```text
/// === RUN   TestName
/// --- PASS: TestName (0.00s)
/// --- FAIL: TestName (0.00s)
/// --- SKIP: TestName (0.00s)
/// ```
fn parse_go_test_output(
    output: &str,
    name_to_clause: &HashMap<String, ClauseId>,
) -> Vec<TestResult> {
    let mut results = Vec::new();
    let mut failure_messages: HashMap<String, String> = HashMap::new();
    let mut current_test: Option<String> = None;
    let mut current_output = String::new();

    for line in output.lines() {
        let trimmed = line.trim();

        // Track test output for failure messages
        if let Some(rest) = trimmed.strip_prefix("=== RUN") {
            // Flush previous
            if let Some(name) = current_test.take() {
                let msg = current_output.trim().to_string();
                if !msg.is_empty() {
                    failure_messages.insert(name, msg);
                }
                current_output.clear();
            }
            current_test = Some(rest.trim().to_string());
            continue;
        }

        // Parse result lines: "--- PASS: TestName (0.00s)"
        let (status, rest) = if let Some(rest) = trimmed.strip_prefix("--- PASS: ") {
            (TestStatus::Passed, rest)
        } else if let Some(rest) = trimmed.strip_prefix("--- FAIL: ") {
            (TestStatus::Failed, rest)
        } else if let Some(rest) = trimmed.strip_prefix("--- SKIP: ") {
            (TestStatus::Skipped, rest)
        } else {
            // Accumulate output for current test
            if current_test.is_some() {
                current_output.push_str(line);
                current_output.push('\n');
            }
            continue;
        };

        // Extract test name (before the timing parenthetical)
        let test_name = if let Some(idx) = rest.find(" (") {
            rest[..idx].trim()
        } else {
            rest.trim()
        };

        // Flush the current test output
        if let Some(name) = current_test.take() {
            let msg = current_output.trim().to_string();
            if !msg.is_empty() {
                failure_messages.insert(name, msg);
            }
            current_output.clear();
        }

        let clause_id = name_to_clause
            .get(test_name)
            .cloned()
            .unwrap_or_else(|| test_name_to_clause_id(test_name));

        let failure_msg = failure_messages.get(test_name).cloned();
        let message = match status {
            TestStatus::Failed => failure_msg.clone().or_else(|| Some("test failed".to_string())),
            TestStatus::Errored => Some("test errored".to_string()),
            _ => None,
        };

        results.push(TestResult {
            clause_id,
            status,
            message,
            duration: Duration::ZERO,
            details: TestDetails {
                failure_message: if status == TestStatus::Failed {
                    failure_msg
                } else {
                    None
                },
                stack_trace: None,
                iterations: None,
                measured_duration: None,
            },
        });
    }

    results
}

impl Runner for GoRunner {
    fn run(&self, tests: &[GeneratedTest], test_dir: &Path) -> anyhow::Result<RunResult> {
        if tests.is_empty() {
            return Ok(RunResult {
                results: Vec::new(),
                total_duration: Duration::ZERO,
            });
        }

        if !self.is_available() {
            anyhow::bail!("go is not available on PATH");
        }

        // Build a mapping from test function name -> ClauseId.
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

        let output = Command::new("go")
            .arg("test")
            .arg("-v")
            .arg("./...")
            .current_dir(test_dir)
            .output()?;

        let total_duration = start.elapsed();

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        let mut results = parse_go_test_output(&stdout, &name_to_clause);

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
                    message: Some(format!("go test failed: {}", error_msg.trim())),
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
        command_exists("go")
    }

    fn name(&self) -> &str {
        "go"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_go_test_output() {
        let output = "\
=== RUN   Testauth_login_must_return_jwt
--- PASS: Testauth_login_must_return_jwt (0.00s)
=== RUN   Testauth_login_must_validate_token
    auth_test.go:15: expected valid token
--- FAIL: Testauth_login_must_validate_token (0.01s)
=== RUN   Testauth_login_may_cache
--- SKIP: Testauth_login_may_cache (0.00s)
FAIL
";
        let mut map = HashMap::new();
        map.insert(
            "Testauth_login_must_return_jwt".to_string(),
            ClauseId("auth::login::must_return_jwt".to_string()),
        );
        map.insert(
            "Testauth_login_must_validate_token".to_string(),
            ClauseId("auth::login::must_validate_token".to_string()),
        );
        map.insert(
            "Testauth_login_may_cache".to_string(),
            ClauseId("auth::login::may_cache".to_string()),
        );

        let results = parse_go_test_output(output, &map);
        assert_eq!(results.len(), 3);
        assert_eq!(results[0].status, TestStatus::Passed);
        assert_eq!(results[1].status, TestStatus::Failed);
        assert!(results[1].details.failure_message.is_some());
        assert_eq!(results[2].status, TestStatus::Skipped);
    }
}
