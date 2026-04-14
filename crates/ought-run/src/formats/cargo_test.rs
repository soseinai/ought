//! Parser for `cargo test`'s default stdout.
//!
//! The format is lightly-structured lines like:
//! ```text
//! running 2 tests
//! test auth_login_must_return_jwt ... ok
//! test auth_login_must_validate_token ... FAILED
//!
//! failures:
//!
//! ---- auth_login_must_validate_token stdout ----
//! thread 'auth_login_must_validate_token' panicked at 'assertion failed'
//!
//! failures:
//!     auth_login_must_validate_token
//!
//! test result: FAILED. 1 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out
//! ```
//!
//! `cargo test` doesn't emit JUnit XML without a third-party reporter, so this
//! format is provided so that the generic `CliRunner` can drive `cargo test`
//! out of the box.

use std::collections::HashMap;
use std::time::Duration;

use ought_spec::ClauseId;

use crate::formats::resolve_clause_id;
use crate::types::{TestDetails, TestResult, TestStatus};

pub fn parse(output: &str, name_to_clause: &HashMap<String, ClauseId>) -> Vec<TestResult> {
    let failure_messages = parse_failure_messages(output);
    let mut results = Vec::new();

    for line in output.lines() {
        let line = line.trim();
        let Some(rest) = line.strip_prefix("test ") else {
            continue;
        };
        let Some((name_part, status_part)) = rest.rsplit_once(" ... ") else {
            continue;
        };
        let test_name = name_part.trim();
        let status_str = status_part.trim();

        // cargo test may print fully-qualified names like `mod::test_name`.
        let clause_id = resolve_clause_id(test_name, name_to_clause);

        let status = match status_str {
            "ok" => TestStatus::Passed,
            "FAILED" => TestStatus::Failed,
            "ignored" => TestStatus::Skipped,
            _ => TestStatus::Errored,
        };

        let failure_message = failure_messages.get(test_name).cloned();
        let message = match status {
            TestStatus::Failed => failure_message
                .clone()
                .or_else(|| Some("test failed".to_string())),
            TestStatus::Errored => Some("test errored".to_string()),
            _ => None,
        };

        results.push(TestResult {
            clause_id,
            status,
            message,
            duration: Duration::ZERO,
            details: TestDetails {
                failure_message,
                stack_trace: None,
                iterations: None,
                measured_duration: None,
            },
        });
    }

    results
}

/// Extract per-test failure output from cargo test's "failures:" section.
fn parse_failure_messages(output: &str) -> HashMap<String, String> {
    let mut messages = HashMap::new();
    let mut in_failures = false;
    let mut current_test: Option<String> = None;
    let mut current_message = String::new();

    for line in output.lines() {
        if line.trim() == "failures:" {
            if let Some(name) = current_test.take() {
                let msg = current_message.trim().to_string();
                if !msg.is_empty() {
                    messages.insert(name, msg);
                }
                current_message.clear();
            }
            in_failures = true;
            continue;
        }
        if !in_failures {
            continue;
        }
        if line.starts_with("---- ") && line.ends_with(" stdout ----") {
            if let Some(name) = current_test.take() {
                let msg = current_message.trim().to_string();
                if !msg.is_empty() {
                    messages.insert(name, msg);
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
            if line.starts_with("test result:") {
                break;
            }
            current_message.push_str(line);
            current_message.push('\n');
        }
    }

    if let Some(name) = current_test.take() {
        let msg = current_message.trim().to_string();
        if !msg.is_empty() {
            messages.insert(name, msg);
        }
    }

    messages
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_passing_output() {
        let output = "\
running 2 tests
test test_auth__login ... ok
test test_auth__logout ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
";
        let map = HashMap::new();
        let results = parse(output, &map);
        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|r| r.status == TestStatus::Passed));
        assert_eq!(
            results[0].clause_id,
            ClauseId("auth::login".to_string())
        );
    }

    #[test]
    fn parses_mixed_output_with_failure_messages() {
        let output = "\
running 2 tests
test test_auth__login ... ok
test test_auth__validate ... FAILED

failures:

---- test_auth__validate stdout ----
thread 'test_auth__validate' panicked at 'token was invalid'

failures:
    test_auth__validate

test result: FAILED. 1 passed; 1 failed
";
        let map = HashMap::new();
        let results = parse(output, &map);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].status, TestStatus::Passed);
        assert_eq!(results[1].status, TestStatus::Failed);
        assert!(
            results[1]
                .details
                .failure_message
                .as_deref()
                .unwrap_or("")
                .contains("token was invalid")
        );
    }

    #[test]
    fn ignored_tests_map_to_skipped() {
        let output = "test test_x__y ... ignored\n";
        let map = HashMap::new();
        let results = parse(output, &map);
        assert_eq!(results[0].status, TestStatus::Skipped);
    }
}
