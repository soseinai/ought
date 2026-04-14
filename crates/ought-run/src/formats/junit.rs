//! JUnit XML parser.
//!
//! Consumes JUnit XML from the many test harnesses that emit it (pytest,
//! jest-junit, gotestsum, cargo-nextest, etc.) and produces `Vec<TestResult>`.
//!
//! Mapping of `<testcase>` children:
//! - `<failure>`  → `TestStatus::Failed`
//! - `<error>`    → `TestStatus::Errored`
//! - `<skipped>`  → `TestStatus::Skipped`
//! - otherwise    → `TestStatus::Passed`
//!
//! `<testcase name="..." classname="..." time="...">` — we resolve the
//! ClauseId from `name` (via [`super::resolve_clause_id`]).

use std::collections::HashMap;
use std::time::Duration;

use ought_spec::ClauseId;
use quick_xml::events::Event;
use quick_xml::Reader;

use crate::formats::resolve_clause_id;
use crate::types::{TestDetails, TestResult, TestStatus};

pub fn parse(xml: &str, name_to_clause: &HashMap<String, ClauseId>) -> anyhow::Result<Vec<TestResult>> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut results = Vec::new();

    // Fields of the currently-open <testcase>.
    let mut in_testcase = false;
    let mut case_name = String::new();
    let mut case_time = Duration::ZERO;
    let mut case_status = TestStatus::Passed;
    let mut failure_message: Option<String> = None;
    let mut stack_trace: Option<String> = None;
    // We buffer text content for the currently-open failure/error/skipped
    // element so we can use it as a stack trace.
    let mut in_diag_element: Option<DiagKind> = None;
    let mut diag_text = String::new();

    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Start(e) => match e.name().as_ref() {
                b"testcase" => {
                    in_testcase = true;
                    case_name.clear();
                    case_time = Duration::ZERO;
                    case_status = TestStatus::Passed;
                    failure_message = None;
                    stack_trace = None;
                    for attr in e.attributes().flatten() {
                        let key = attr.key.as_ref();
                        let val = attr.unescape_value().unwrap_or_default().to_string();
                        match key {
                            b"name" => case_name = val,
                            b"time" => {
                                case_time = val
                                    .parse::<f64>()
                                    .ok()
                                    .map(Duration::from_secs_f64)
                                    .unwrap_or(Duration::ZERO);
                            }
                            _ => {}
                        }
                    }
                }
                b"failure" | b"error" | b"skipped" if in_testcase => {
                    let kind = match e.name().as_ref() {
                        b"failure" => DiagKind::Failure,
                        b"error" => DiagKind::Error,
                        _ => DiagKind::Skipped,
                    };
                    case_status = kind.status();
                    in_diag_element = Some(kind);
                    diag_text.clear();
                    // The `message` attribute carries a short summary.
                    for attr in e.attributes().flatten() {
                        if attr.key.as_ref() == b"message" {
                            failure_message =
                                Some(attr.unescape_value().unwrap_or_default().to_string());
                        }
                    }
                }
                _ => {}
            },
            Event::Empty(e) => match e.name().as_ref() {
                // Self-closing <testcase .../> — handled by Start won't be emitted;
                // Empty is emitted for self-closing.
                b"testcase" => {
                    // A self-closing testcase: emit a Passed result directly.
                    let mut name = String::new();
                    let mut time = Duration::ZERO;
                    for attr in e.attributes().flatten() {
                        match attr.key.as_ref() {
                            b"name" => {
                                name = attr.unescape_value().unwrap_or_default().to_string();
                            }
                            b"time" => {
                                time = attr
                                    .unescape_value()
                                    .unwrap_or_default()
                                    .parse::<f64>()
                                    .ok()
                                    .map(Duration::from_secs_f64)
                                    .unwrap_or(Duration::ZERO);
                            }
                            _ => {}
                        }
                    }
                    if !name.is_empty() {
                        results.push(TestResult {
                            clause_id: resolve_clause_id(&name, name_to_clause),
                            status: TestStatus::Passed,
                            message: None,
                            duration: time,
                            details: TestDetails::default(),
                        });
                    }
                }
                b"failure" | b"error" | b"skipped" if in_testcase => {
                    let kind = match e.name().as_ref() {
                        b"failure" => DiagKind::Failure,
                        b"error" => DiagKind::Error,
                        _ => DiagKind::Skipped,
                    };
                    case_status = kind.status();
                    for attr in e.attributes().flatten() {
                        if attr.key.as_ref() == b"message" {
                            failure_message =
                                Some(attr.unescape_value().unwrap_or_default().to_string());
                        }
                    }
                }
                _ => {}
            },
            Event::Text(t) if in_diag_element.is_some() => {
                diag_text.push_str(&t.unescape().unwrap_or_default());
            }
            Event::CData(t) if in_diag_element.is_some() => {
                diag_text.push_str(&String::from_utf8_lossy(&t));
            }
            Event::End(e) => match e.name().as_ref() {
                b"testcase" if in_testcase => {
                    let message = match case_status {
                        TestStatus::Failed => failure_message
                            .clone()
                            .or_else(|| Some("test failed".to_string())),
                        TestStatus::Errored => failure_message
                            .clone()
                            .or_else(|| Some("test errored".to_string())),
                        TestStatus::Skipped => failure_message.clone(),
                        TestStatus::Passed => None,
                    };
                    results.push(TestResult {
                        clause_id: resolve_clause_id(&case_name, name_to_clause),
                        status: case_status,
                        message,
                        duration: case_time,
                        details: TestDetails {
                            failure_message: failure_message.clone(),
                            stack_trace: stack_trace.clone(),
                            iterations: None,
                            measured_duration: None,
                        },
                    });
                    in_testcase = false;
                }
                b"failure" | b"error" | b"skipped" if in_diag_element.is_some() => {
                    let trimmed = diag_text.trim().to_string();
                    if !trimmed.is_empty() {
                        // Prefer the body as the stack trace; keep the attribute
                        // message (if any) as the short failure_message.
                        stack_trace = Some(trimmed.clone());
                        if failure_message.is_none() {
                            failure_message = Some(trimmed);
                        }
                    }
                    in_diag_element = None;
                    diag_text.clear();
                }
                _ => {}
            },
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }

    Ok(results)
}

#[derive(Clone, Copy)]
enum DiagKind {
    Failure,
    Error,
    Skipped,
}

impl DiagKind {
    fn status(self) -> TestStatus {
        match self {
            DiagKind::Failure => TestStatus::Failed,
            DiagKind::Error => TestStatus::Errored,
            DiagKind::Skipped => TestStatus::Skipped,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pytest_style_junit() {
        let xml = r#"<?xml version="1.0" encoding="utf-8"?>
<testsuites>
  <testsuite name="pytest" tests="3" failures="1" errors="0" skipped="1" time="0.123">
    <testcase classname="test_auth" name="test_auth__login__must_return_jwt" time="0.050"/>
    <testcase classname="test_auth" name="test_auth__login__must_validate_token" time="0.060">
      <failure message="AssertionError">token was invalid</failure>
    </testcase>
    <testcase classname="test_auth" name="test_auth__login__may_cache" time="0.010">
      <skipped/>
    </testcase>
  </testsuite>
</testsuites>"#;
        let map = HashMap::new();
        let results = parse(xml, &map).expect("parse");
        assert_eq!(results.len(), 3);
        assert_eq!(results[0].status, TestStatus::Passed);
        assert_eq!(
            results[0].clause_id,
            ClauseId("auth::login::must_return_jwt".to_string())
        );
        assert_eq!(results[1].status, TestStatus::Failed);
        assert_eq!(
            results[1].details.failure_message.as_deref(),
            Some("AssertionError")
        );
        assert_eq!(results[1].details.stack_trace.as_deref(), Some("token was invalid"));
        assert_eq!(results[2].status, TestStatus::Skipped);
    }

    #[test]
    fn jest_junit_style() {
        let xml = r#"<testsuites>
  <testsuite name="auth suite">
    <testcase classname="auth" name="test_auth__login" time="0.5"/>
    <testcase classname="auth" name="test_auth__logout" time="0.2">
      <failure message="expected true"><![CDATA[Error: expected true
    at line 42]]></failure>
    </testcase>
  </testsuite>
</testsuites>"#;
        let map = HashMap::new();
        let results = parse(xml, &map).expect("parse");
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].status, TestStatus::Passed);
        assert_eq!(results[1].status, TestStatus::Failed);
        assert!(
            results[1]
                .details
                .stack_trace
                .as_deref()
                .unwrap_or("")
                .contains("line 42")
        );
    }

    #[test]
    fn gotestsum_style() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<testsuites>
  <testsuite name="pkg/auth" tests="2">
    <testcase name="Testauth__login" time="0.001"/>
    <testcase name="Testauth__logout" time="0.002">
      <error message="panic: boom">panic trace here</error>
    </testcase>
  </testsuite>
</testsuites>"#;
        let map = HashMap::new();
        let results = parse(xml, &map).expect("parse");
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].status, TestStatus::Passed);
        assert_eq!(
            results[0].clause_id,
            ClauseId("auth::login".to_string())
        );
        assert_eq!(results[1].status, TestStatus::Errored);
    }

    #[test]
    fn map_lookup_preferred_over_convention() {
        let xml = r#"<testsuites><testsuite><testcase name="weird_name" time="0"/></testsuite></testsuites>"#;
        let mut map = HashMap::new();
        map.insert(
            "weird_name".to_string(),
            ClauseId("explicit::id".to_string()),
        );
        let results = parse(xml, &map).expect("parse");
        assert_eq!(results[0].clause_id, ClauseId("explicit::id".to_string()));
    }
}
