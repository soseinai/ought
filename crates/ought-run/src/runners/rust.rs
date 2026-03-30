use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

use ought_gen::GeneratedTest;
use ought_spec::ClauseId;

use crate::runner::Runner;
use crate::types::{RunResult, TestDetails, TestResult, TestStatus};

pub struct RustRunner;

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

/// Convert a `ClauseId` like `auth::login::must_return_jwt` into the test function
/// name form used in generated code: `auth__login__must_return_jwt`.
fn clause_id_to_test_name(clause_id: &ClauseId) -> String {
    clause_id.0.replace("::", "__")
}

/// Reverse mapping: convert a test function name back into a `ClauseId`.
fn test_name_to_clause_id(test_name: &str) -> ClauseId {
    ClauseId(test_name.replace("__", "::"))
}

/// Parse cargo test stdout to extract per-test results and failure messages.
fn parse_cargo_test_output(
    output: &str,
    name_to_clause: &HashMap<String, ClauseId>,
) -> Vec<TestResult> {
    let mut results: Vec<TestResult> = Vec::new();
    let failure_messages = parse_failure_messages(output);

    for line in output.lines() {
        let line = line.trim();
        // Lines look like: "test test_name ... ok" or "test test_name ... FAILED"
        if let Some(rest) = line.strip_prefix("test ")
            && let Some((name_part, status_part)) = rest.rsplit_once(" ... ") {
                let test_name = name_part.trim();
                let status_str = status_part.trim();

                // The test name may be fully qualified like `module::test_name`.
                // We try the full name and also just the last segment.
                let clause_id = name_to_clause
                    .get(test_name)
                    .or_else(|| {
                        // Sometimes cargo test prints module paths; try the last segment.
                        let last = test_name.rsplit("::").next().unwrap_or(test_name);
                        name_to_clause.get(last)
                    })
                    .cloned();

                let clause_id = match clause_id {
                    Some(id) => id,
                    None => {
                        // If we can't map it back, try converting the test name directly.
                        test_name_to_clause_id(test_name)
                    }
                };

                let status = match status_str {
                    "ok" => TestStatus::Passed,
                    "FAILED" => TestStatus::Failed,
                    "ignored" => TestStatus::Skipped,
                    _ => TestStatus::Errored,
                };

                let failure_message = failure_messages.get(test_name).cloned();
                let message = match status {
                    TestStatus::Failed => failure_message.clone().or_else(|| Some("test failed".to_string())),
                    TestStatus::Errored => Some("test errored".to_string()),
                    _ => None,
                };

                results.push(TestResult {
                    clause_id,
                    status,
                    message,
                    duration: Duration::ZERO, // Individual timings not available from basic output
                    details: TestDetails {
                        failure_message,
                        stack_trace: None,
                        iterations: None,
                        measured_duration: None,
                    },
                });
            }
    }

    results
}

/// Parse the "failures:" section at the end of cargo test output to extract
/// per-test failure messages.
fn parse_failure_messages(output: &str) -> HashMap<String, String> {
    let mut messages = HashMap::new();

    // The failure details section looks like:
    // failures:
    //
    // ---- test_name stdout ----
    // <failure output>
    //
    // failures:
    //     test_name1
    //     test_name2

    let mut in_failures = false;
    let mut current_test: Option<String> = None;
    let mut current_message = String::new();

    for line in output.lines() {
        if line.trim() == "failures:" {
            // Flush any current test message
            if let Some(test_name) = current_test.take() {
                let msg = current_message.trim().to_string();
                if !msg.is_empty() {
                    messages.insert(test_name, msg);
                }
                current_message.clear();
            }
            in_failures = true;
            continue;
        }

        if !in_failures {
            continue;
        }

        // Check for "---- test_name stdout ----"
        if line.starts_with("---- ") && line.ends_with(" stdout ----") {
            // Flush previous
            if let Some(test_name) = current_test.take() {
                let msg = current_message.trim().to_string();
                if !msg.is_empty() {
                    messages.insert(test_name, msg);
                }
                current_message.clear();
            }
            let name = line
                .strip_prefix("---- ")
                .and_then(|s| s.strip_suffix(" stdout ----"))
                .unwrap_or("")
                .trim()
                .to_string();
            current_test = Some(name);
        } else if current_test.is_some() {
            // Check if we hit the summary section (list of failing test names)
            if line.starts_with("test result:") {
                break;
            }
            current_message.push_str(line);
            current_message.push('\n');
        }
    }

    // Flush last test
    if let Some(test_name) = current_test.take() {
        let msg = current_message.trim().to_string();
        if !msg.is_empty() {
            messages.insert(test_name, msg);
        }
    }

    messages
}

impl Runner for RustRunner {
    fn run(&self, tests: &[GeneratedTest], test_dir: &Path) -> anyhow::Result<RunResult> {
        if tests.is_empty() {
            return Ok(RunResult {
                results: Vec::new(),
                total_duration: Duration::ZERO,
            });
        }

        if !self.is_available() {
            anyhow::bail!("cargo is not available on PATH");
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

        // Find the project root: walk up from test_dir to find a Cargo.toml.
        let project_dir = find_cargo_project(test_dir)?;

        let start = Instant::now();

        let output = Command::new("cargo")
            .arg("test")
            .arg("--test-threads=1")
            .current_dir(&project_dir)
            .output()?;

        let total_duration = start.elapsed();

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        // If cargo itself failed to start the test harness (compilation error, etc.)
        // we still try to parse whatever output we got. But if there's nothing to
        // parse, report the error.
        let mut results = parse_cargo_test_output(&stdout, &name_to_clause);

        if results.is_empty() && !output.status.success() {
            // The test harness itself failed (likely a compilation error).
            // Report all tests as errored.
            let error_msg = if stderr.is_empty() {
                stdout.to_string()
            } else {
                stderr.to_string()
            };

            for test in tests {
                results.push(TestResult {
                    clause_id: test.clause_id.clone(),
                    status: TestStatus::Errored,
                    message: Some(format!("test harness failed: {}", error_msg.trim())),
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
        command_exists("cargo")
    }

    fn name(&self) -> &str {
        "rust"
    }
}

/// Walk up from the given directory to find a Cargo.toml.
fn find_cargo_project(start: &Path) -> anyhow::Result<std::path::PathBuf> {
    let mut dir = start.to_path_buf();
    loop {
        if dir.join("Cargo.toml").exists() {
            return Ok(dir);
        }
        if !dir.pop() {
            anyhow::bail!(
                "could not find Cargo.toml in any parent of {}",
                start.display()
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clause_id_roundtrip() {
        let id = ClauseId("auth::login::must_return_jwt".to_string());
        let name = clause_id_to_test_name(&id);
        assert_eq!(name, "auth__login__must_return_jwt");
        let back = test_name_to_clause_id(&name);
        assert_eq!(back, id);
    }

    #[test]
    fn test_parse_passing_output() {
        let output = "\
running 2 tests
test auth__login__must_return_jwt ... ok
test auth__login__must_validate_token ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
";
        let mut map = HashMap::new();
        map.insert(
            "auth__login__must_return_jwt".to_string(),
            ClauseId("auth::login::must_return_jwt".to_string()),
        );
        map.insert(
            "auth__login__must_validate_token".to_string(),
            ClauseId("auth::login::must_validate_token".to_string()),
        );

        let results = parse_cargo_test_output(output, &map);
        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|r| r.status == TestStatus::Passed));
    }

    #[test]
    fn test_parse_mixed_output() {
        let output = "\
running 2 tests
test auth__login__must_return_jwt ... ok
test auth__login__must_validate_token ... FAILED

failures:

---- auth__login__must_validate_token stdout ----
thread 'auth__login__must_validate_token' panicked at 'assertion failed: token.is_valid()'

failures:
    auth__login__must_validate_token

test result: FAILED. 1 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out
";
        let mut map = HashMap::new();
        map.insert(
            "auth__login__must_return_jwt".to_string(),
            ClauseId("auth::login::must_return_jwt".to_string()),
        );
        map.insert(
            "auth__login__must_validate_token".to_string(),
            ClauseId("auth::login::must_validate_token".to_string()),
        );

        let results = parse_cargo_test_output(output, &map);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].status, TestStatus::Passed);
        assert_eq!(results[1].status, TestStatus::Failed);
        assert!(results[1].details.failure_message.is_some());
    }
}
