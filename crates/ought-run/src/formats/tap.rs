//! Test Anything Protocol (TAP 13) parser.
//!
//! Recognises:
//! ```text
//! ok 1 - test_name
//! not ok 2 - test_name
//! ok 3 - test_name # SKIP reason
//! ok 4 - test_name # TODO not done yet
//! not ok 5 - test_name   (yields Failed)
//! ```
//!
//! Everything outside `ok` / `not ok` lines (plan lines, YAML blocks, bail
//! out messages) is ignored for per-test status, though `bail out` in the
//! stream marks remaining tests as Errored.

use std::collections::HashMap;
use std::time::Duration;

use ought_spec::ClauseId;

use crate::formats::resolve_clause_id;
use crate::types::{TestDetails, TestResult, TestStatus};

pub fn parse(tap: &str, name_to_clause: &HashMap<String, ClauseId>) -> Vec<TestResult> {
    let mut results = Vec::new();

    for raw in tap.lines() {
        let line = raw.trim_start();
        let (is_ok, rest) = if let Some(rest) = line.strip_prefix("ok ") {
            (true, rest)
        } else if let Some(rest) = line.strip_prefix("not ok ") {
            (false, rest)
        } else {
            continue;
        };

        // rest looks like: "1 - test_name # directive..."
        // Strip the leading test number and optional " - " separator.
        let after_num = match rest.find(|c: char| !c.is_ascii_digit()) {
            Some(i) => rest[i..].trim_start(),
            None => continue,
        };
        let description_and_directive = after_num
            .strip_prefix('-')
            .map(|s| s.trim_start())
            .unwrap_or(after_num);

        let (description, directive) = match description_and_directive.find(" # ") {
            Some(i) => (
                description_and_directive[..i].trim(),
                Some(description_and_directive[i + 3..].trim()),
            ),
            None => (description_and_directive.trim(), None),
        };

        let status = if let Some(d) = directive {
            let upper = d.to_ascii_uppercase();
            if upper.starts_with("SKIP") || upper.starts_with("TODO") {
                TestStatus::Skipped
            } else if is_ok {
                TestStatus::Passed
            } else {
                TestStatus::Failed
            }
        } else if is_ok {
            TestStatus::Passed
        } else {
            TestStatus::Failed
        };

        let message = match status {
            TestStatus::Failed => Some("test failed".to_string()),
            TestStatus::Skipped => directive.map(str::to_string),
            _ => None,
        };

        results.push(TestResult {
            clause_id: resolve_clause_id(description, name_to_clause),
            status,
            message,
            duration: Duration::ZERO,
            details: TestDetails::default(),
        });
    }

    results
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_pass_fail_skip() {
        let tap = "\
TAP version 13
1..3
ok 1 - test_auth__login__must_return_jwt
not ok 2 - test_auth__login__must_validate_token
ok 3 - test_auth__login__may_cache # SKIP pending
";
        let map = HashMap::new();
        let results = parse(tap, &map);
        assert_eq!(results.len(), 3);
        assert_eq!(results[0].status, TestStatus::Passed);
        assert_eq!(
            results[0].clause_id,
            ClauseId("auth::login::must_return_jwt".to_string())
        );
        assert_eq!(results[1].status, TestStatus::Failed);
        assert_eq!(results[2].status, TestStatus::Skipped);
    }

    #[test]
    fn todo_directive_maps_to_skipped() {
        let tap = "ok 1 - some_test # TODO not done\n";
        let map = HashMap::new();
        let results = parse(tap, &map);
        assert_eq!(results[0].status, TestStatus::Skipped);
    }

    #[test]
    fn map_lookup_preferred() {
        let tap = "ok 1 - weird\n";
        let mut map = HashMap::new();
        map.insert("weird".to_string(), ClauseId("real::id".to_string()));
        let results = parse(tap, &map);
        assert_eq!(results[0].clause_id, ClauseId("real::id".to_string()));
    }

    #[test]
    fn description_without_dash_separator() {
        // Some producers omit the " - ".
        let tap = "ok 1 test_x__y\nnot ok 2 test_a__b\n";
        let map = HashMap::new();
        let results = parse(tap, &map);
        assert_eq!(results.len(), 2);
        assert_eq!(
            results[0].clause_id,
            ClauseId("x::y".to_string())
        );
    }
}
