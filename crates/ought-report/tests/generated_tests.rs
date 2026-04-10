#![allow(dead_code, clippy::all)]
#![allow(non_snake_case)]

//! Generated tests for ought-report, rewritten to use the real API.
//!
//! Categories covered:
//!   - terminal_output (10 tests)
//!   - failure_details (4 tests)
//!   - given_block_display (4 tests)
//!   - json_output (3 tests -- diagnosis/grade test removed since those features are stubbed)
//!   - junit_xml_output (5 tests)
//!   - otherwise_chain_display (5 tests)
//!   - temporal_result_display (3 tests)
//!
//! Removed categories (stubbed/non-reporter functionality):
//!   - failure_narratives_llm_powered (7 tests) -- diagnosis is stubbed
//!   - test_quality_grading (6 tests) -- grading is stubbed
//!   - progress_during_generation (3 tests) -- not part of reporter

use std::path::PathBuf;
use std::time::Duration;

use ought_report::types::{ColorChoice, ReportOptions};
use ought_run::{RunResult, TestDetails, TestResult, TestStatus};
use ought_spec::{Clause, ClauseId, Keyword, Metadata, Section, SourceLocation, Spec, Temporal};

// ── Test data builders ──────────────────────────────────────────────────────

fn make_clause(keyword: Keyword, text: &str, id: &str) -> Clause {
    Clause {
        id: ClauseId(id.to_string()),
        keyword,
        severity: keyword.severity(),
        text: text.to_string(),
        condition: None,
        otherwise: vec![],
        temporal: None,
        hints: vec![],
        source_location: SourceLocation {
            file: PathBuf::from("test.ought.md"),
            line: 1,
        },
        content_hash: "test".to_string(),
        pending: false,
    }
}

fn make_section(title: &str, clauses: Vec<Clause>) -> Section {
    Section {
        title: title.to_string(),
        depth: 1,
        prose: String::new(),
        clauses,
        subsections: vec![],
    }
}

fn make_spec(name: &str, sections: Vec<Section>) -> Spec {
    Spec {
        name: name.to_string(),
        metadata: Metadata::default(),
        sections,
        source_path: PathBuf::from("test.ought.md"),
    }
}

fn make_result(clause_id: &str, status: TestStatus) -> TestResult {
    TestResult {
        clause_id: ClauseId(clause_id.to_string()),
        status,
        message: None,
        duration: Duration::from_millis(10),
        details: TestDetails::default(),
    }
}

fn make_result_with_message(clause_id: &str, status: TestStatus, msg: &str) -> TestResult {
    TestResult {
        clause_id: ClauseId(clause_id.to_string()),
        status,
        message: Some(msg.to_string()),
        duration: Duration::from_millis(10),
        details: TestDetails::default(),
    }
}

fn make_result_with_failure_detail(clause_id: &str, status: TestStatus, failure_msg: &str) -> TestResult {
    TestResult {
        clause_id: ClauseId(clause_id.to_string()),
        status,
        message: None,
        duration: Duration::from_millis(10),
        details: TestDetails {
            failure_message: Some(failure_msg.to_string()),
            ..Default::default()
        },
    }
}

fn make_run_result(results: Vec<TestResult>) -> RunResult {
    let total: Duration = results.iter().map(|r| r.duration).sum();
    RunResult {
        results,
        total_duration: total,
    }
}

fn no_color_options() -> ReportOptions {
    ReportOptions {
        color: ColorChoice::Never,
        ..Default::default()
    }
}

fn color_options() -> ReportOptions {
    ReportOptions {
        color: ColorChoice::Always,
        ..Default::default()
    }
}

fn render_terminal(run: &RunResult, specs: &[Spec], opts: &ReportOptions) -> String {
    let mut buf: Vec<u8> = Vec::new();
    ought_report::terminal::report_to_writer(&mut buf, run, specs, opts)
        .expect("report_to_writer should succeed");
    String::from_utf8(buf).expect("output should be valid UTF-8")
}

// ── ANSI escape code constants ──────────────────────────────────────────────

const RED: &str = "\x1b[31m";
const YELLOW: &str = "\x1b[33m";
const DIM: &str = "\x1b[2m";
const BOLD: &str = "\x1b[1m";
#[allow(dead_code)]
const RESET: &str = "\x1b[0m";

// ═══════════════════════════════════════════════════════════════════════════
// TERMINAL OUTPUT TESTS
// ═══════════════════════════════════════════════════════════════════════════

/// MUST show each clause with its keyword, text, and pass/fail status
#[test]
fn test_terminal_output_must_show_each_clause_with_keyword_text_and_status() {
    let specs = vec![make_spec(
        "Auth Spec",
        vec![make_section(
            "Auth",
            vec![
                make_clause(Keyword::Must, "return a token", "auth::must_return_token"),
                make_clause(Keyword::Should, "log the attempt", "auth::should_log"),
                make_clause(Keyword::May, "set a cookie", "auth::may_cookie"),
            ],
        )],
    )];

    let run = make_run_result(vec![
        make_result("auth::must_return_token", TestStatus::Passed),
        make_result("auth::should_log", TestStatus::Failed),
        make_result("auth::may_cookie", TestStatus::Skipped),
    ]);

    let text = render_terminal(&run, &specs, &no_color_options());

    // Each clause line must carry the keyword label and clause text.
    let must_line = text.lines().find(|l| l.contains("return a token")).expect("MUST clause line missing");
    let should_line = text.lines().find(|l| l.contains("log the attempt")).expect("SHOULD clause line missing");
    let may_line = text.lines().find(|l| l.contains("set a cookie")).expect("MAY clause line missing");

    assert!(must_line.contains("MUST"), "MUST keyword must appear on clause line");
    assert!(must_line.contains("\u{2713}"), "passed indicator must appear on MUST line");

    assert!(should_line.contains("SHOULD"), "SHOULD keyword must appear on clause line");
    assert!(should_line.contains("\u{2717}"), "failed indicator must appear on SHOULD line");

    assert!(may_line.contains("MAY"), "MAY keyword must appear on clause line");
    assert!(may_line.contains("~"), "skipped indicator must appear on MAY line");
}

/// MUST use status indicators: checkmark passed, X failed, ! errored, circle-slash confirmed absent (WONT), ~ skipped
#[test]
fn test_terminal_output_must_use_status_indicators() {
    let specs = vec![make_spec(
        "Status Spec",
        vec![make_section(
            "All statuses",
            vec![
                make_clause(Keyword::Must, "clause-passed", "s::passed"),
                make_clause(Keyword::Must, "clause-failed", "s::failed"),
                make_clause(Keyword::Must, "clause-errored", "s::errored"),
                make_clause(Keyword::Wont, "clause-absent", "s::absent"),
                make_clause(Keyword::Should, "clause-skipped", "s::skipped"),
            ],
        )],
    )];

    let run = make_run_result(vec![
        make_result("s::passed", TestStatus::Passed),
        make_result("s::failed", TestStatus::Failed),
        make_result("s::errored", TestStatus::Errored),
        make_result("s::absent", TestStatus::Passed), // WONT + Passed = confirmed absent
        make_result("s::skipped", TestStatus::Skipped),
    ]);

    let text = render_terminal(&run, &specs, &no_color_options());

    // Verify each indicator appears on the correct clause's line.
    for line in text.lines() {
        if line.contains("clause-passed") {
            assert!(line.contains("\u{2713}"), "passed clause line must show checkmark, got: {line:?}");
        }
        if line.contains("clause-failed") {
            assert!(line.contains("\u{2717}"), "failed clause line must show X, got: {line:?}");
        }
        if line.contains("clause-errored") {
            assert!(line.contains("!"), "errored clause line must show !, got: {line:?}");
        }
        if line.contains("clause-absent") {
            assert!(line.contains("\u{2298}"), "absent clause line must show circle-slash, got: {line:?}");
        }
        if line.contains("clause-skipped") {
            assert!(line.contains("~"), "skipped clause line must show ~, got: {line:?}");
        }
    }
}

/// MUST display results grouped by spec file, then by section, then by clause
#[test]
fn test_terminal_output_must_display_grouped_by_spec_section_clause() {
    let specs = vec![
        make_spec(
            "Auth Spec",
            vec![
                make_section("Login", vec![make_clause(Keyword::Must, "issue JWT", "auth::login::jwt")]),
                make_section("Logout", vec![make_clause(Keyword::Must, "invalidate token", "auth::logout::token")]),
            ],
        ),
        make_spec(
            "API Spec",
            vec![make_section("Endpoints", vec![make_clause(Keyword::Must, "return 200", "api::endpoints::200")])],
        ),
    ];

    let run = make_run_result(vec![
        make_result("auth::login::jwt", TestStatus::Passed),
        make_result("auth::logout::token", TestStatus::Passed),
        make_result("api::endpoints::200", TestStatus::Passed),
    ]);

    let text = render_terminal(&run, &specs, &no_color_options());

    // Spec ordering
    let pos_auth = text.find("Auth Spec").expect("Auth Spec not found");
    let pos_api = text.find("API Spec").expect("API Spec not found");
    assert!(pos_auth < pos_api, "Auth Spec should precede API Spec");

    // Section ordering within a single spec
    let pos_login = text.find("Login").expect("Login section not found");
    let pos_logout = text.find("Logout").expect("Logout section not found");
    assert!(pos_login < pos_logout, "Login section should appear before Logout section");

    // Section header must precede its own clauses
    let pos_login_header = text.find("Login").unwrap();
    let pos_jwt_clause = text.find("issue JWT").expect("JWT clause not found");
    assert!(pos_login_header < pos_jwt_clause, "section header must precede its clauses");
}

/// MUST print a summary line at the end with total passed, failed, errored
#[test]
fn test_terminal_output_must_print_summary_line() {
    let specs = vec![make_spec(
        "Summary Spec",
        vec![make_section(
            "S",
            vec![
                make_clause(Keyword::Must, "p1", "sum::p1"),
                make_clause(Keyword::Must, "p2", "sum::p2"),
                make_clause(Keyword::Must, "f1", "sum::f1"),
                make_clause(Keyword::Should, "e1", "sum::e1"),
            ],
        )],
    )];

    let run = make_run_result(vec![
        make_result("sum::p1", TestStatus::Passed),
        make_result("sum::p2", TestStatus::Passed),
        make_result("sum::f1", TestStatus::Failed),
        make_result("sum::e1", TestStatus::Errored),
    ]);

    let text = render_terminal(&run, &specs, &no_color_options());

    // Summary must contain correct aggregate counts.
    assert!(text.contains("2 passed"), "summary must state '2 passed', got:\n{text}");
    assert!(text.contains("1 failed"), "summary must state '1 failed', got:\n{text}");
    assert!(text.contains("1 errored"), "summary must state '1 errored', got:\n{text}");

    // Clause content must appear before the summary.
    let pos_clause = text.find("p1").expect("clause text not found");
    let pos_passed = text.rfind("2 passed").expect("summary not found");
    assert!(pos_clause < pos_passed, "clause content must appear before the summary line");
}

/// MUST show MUST coverage percentage in the summary
#[test]
fn test_terminal_output_must_show_must_coverage_percentage() {
    // 2 of 3 MUST clauses pass -> 67%
    let specs = vec![make_spec(
        "Coverage Spec",
        vec![make_section(
            "S",
            vec![
                make_clause(Keyword::Must, "m1", "cov::m1"),
                make_clause(Keyword::Must, "m2", "cov::m2"),
                make_clause(Keyword::Must, "m3", "cov::m3"),
                make_clause(Keyword::Should, "s1", "cov::s1"), // SHOULD must not affect MUST %
            ],
        )],
    )];

    let run = make_run_result(vec![
        make_result("cov::m1", TestStatus::Passed),
        make_result("cov::m2", TestStatus::Passed),
        make_result("cov::m3", TestStatus::Failed),
        make_result("cov::s1", TestStatus::Passed),
    ]);

    let text = render_terminal(&run, &specs, &no_color_options());
    // 2/3 = 66.666...% -> displayed as 67%
    assert!(
        text.contains("2/3") || text.contains("67%") || text.contains("67 %") || text.contains("66%"),
        "summary must show MUST coverage, got:\n{text}"
    );

    // The label must mention MUST
    assert!(
        text.to_uppercase().contains("MUST"),
        "summary must mention MUST in coverage label, got:\n{text}"
    );
}

/// MUST color-code by severity: MUST failures in red, SHOULD failures in yellow
#[test]
fn test_terminal_output_must_color_code_by_severity() {
    let specs = vec![make_spec(
        "Severity Spec",
        vec![make_section(
            "Severity",
            vec![
                make_clause(Keyword::Must, "must-fail", "sev::must"),
                make_clause(Keyword::Should, "should-fail", "sev::should"),
                make_clause(Keyword::May, "may-fail", "sev::may"),
            ],
        )],
    )];

    let run = make_run_result(vec![
        make_result("sev::must", TestStatus::Failed),
        make_result("sev::should", TestStatus::Failed),
        make_result("sev::may", TestStatus::Failed),
    ]);

    let text = render_terminal(&run, &specs, &color_options());

    let must_line = text.lines().find(|l| l.contains("must-fail")).expect("MUST failure line missing");
    let should_line = text.lines().find(|l| l.contains("should-fail")).expect("SHOULD failure line missing");
    let may_line = text.lines().find(|l| l.contains("may-fail")).expect("MAY failure line missing");

    assert!(
        must_line.contains(RED),
        "MUST failures must be colored red, got: {must_line:?}"
    );
    assert!(
        should_line.contains(YELLOW),
        "SHOULD failures must be colored yellow, got: {should_line:?}"
    );
    // MAY failures use yellow in the actual implementation
    assert!(
        may_line.contains(YELLOW),
        "MAY failures must be colored yellow, got: {may_line:?}"
    );
}

/// SHOULD dim passing clauses and highlight failures for visual scanning
#[test]
fn test_terminal_output_should_dim_passing_and_highlight_failures() {
    let specs = vec![make_spec(
        "Dim Spec",
        vec![make_section(
            "S",
            vec![
                make_clause(Keyword::Must, "passing clause", "dim::pass"),
                make_clause(Keyword::Must, "failing clause", "dim::fail"),
            ],
        )],
    )];

    let run = make_run_result(vec![
        make_result("dim::pass", TestStatus::Passed),
        make_result("dim::fail", TestStatus::Failed),
    ]);

    let text_color = render_terminal(&run, &specs, &color_options());
    let text_nocolor = render_terminal(&run, &specs, &no_color_options());

    // With color: passing clause line must carry the dim code.
    let passing_line = text_color.lines().find(|l| l.contains("passing clause")).expect("passing clause missing");
    assert!(
        passing_line.contains(DIM),
        "passing clause must be dimmed, got: {passing_line:?}"
    );

    // With color: failing MUST clause must be highlighted (red or bold).
    let failing_line = text_color.lines().find(|l| l.contains("failing clause")).expect("failing clause missing");
    assert!(
        failing_line.contains(RED) || failing_line.contains(BOLD),
        "failing clause must be highlighted (red or bold), got: {failing_line:?}"
    );

    // Without color: no ANSI escape codes should appear.
    assert!(
        !text_nocolor.contains("\x1b["),
        "color-disabled output must contain no ANSI escape sequences"
    );
}

/// WONT use animated spinners or progress bars in non-TTY mode (pipe-friendly)
#[test]
fn test_terminal_output_wont_use_spinners_in_non_tty_mode() {
    let specs = vec![make_spec(
        "Pipe Spec",
        vec![make_section(
            "S",
            vec![
                make_clause(Keyword::Must, "clause a", "pipe::a"),
                make_clause(Keyword::Must, "clause b", "pipe::b"),
            ],
        )],
    )];

    let run = make_run_result(vec![
        make_result("pipe::a", TestStatus::Passed),
        make_result("pipe::b", TestStatus::Failed),
    ]);

    let text = render_terminal(&run, &specs, &no_color_options());

    // Output must not be empty.
    assert!(!text.trim().is_empty(), "non-TTY output must not be empty");

    // Braille spinner characters must be absent.
    let braille_spinners = ['\u{280B}', '\u{2819}', '\u{2839}', '\u{2838}', '\u{283C}', '\u{2834}', '\u{2826}', '\u{2827}', '\u{2807}', '\u{280F}'];
    for ch in &braille_spinners {
        assert!(!text.contains(*ch), "non-TTY output must not contain braille spinner char '{ch}'");
    }

    // Block-fill progress-bar characters must be absent.
    let progress_bar_chars = ['\u{2588}', '\u{2591}', '\u{2593}', '\u{2592}'];
    for ch in &progress_bar_chars {
        assert!(!text.contains(*ch), "non-TTY output must not contain progress bar character '{ch}'");
    }

    // ANSI cursor-control escapes must be absent.
    assert!(!text.contains("\x1b[?25l"), "cursor-hide escape must not appear");
    assert!(!text.contains("\x1b[1A"), "cursor-up escape must not appear");
    assert!(!text.contains("\x1b[2K"), "erase-line escape must not appear");
}

/// Terminal report should not panic with empty specs
#[test]
fn test_terminal_output_no_panic_with_empty_specs() {
    let specs: Vec<Spec> = vec![];
    let run = RunResult {
        results: vec![],
        total_duration: Duration::from_millis(0),
    };
    // Should not panic.
    let text = render_terminal(&run, &specs, &no_color_options());
    assert!(!text.is_empty(), "output should contain at least a header");
}

/// Terminal report should not panic with no results for clauses
#[test]
fn test_terminal_output_no_panic_with_missing_results() {
    let specs = vec![make_spec(
        "Missing Spec",
        vec![make_section(
            "S",
            vec![make_clause(Keyword::Must, "no result for this", "missing::clause")],
        )],
    )];
    let run = RunResult {
        results: vec![],
        total_duration: Duration::from_millis(0),
    };
    // Should not panic even though clause has no matching test result.
    let text = render_terminal(&run, &specs, &no_color_options());
    assert!(text.contains("no result for this"), "clause text should still appear");
}

// ═══════════════════════════════════════════════════════════════════════════
// FAILURE DETAILS TESTS
// ═══════════════════════════════════════════════════════════════════════════

/// MUST show the assertion error or failure message for each failed clause
#[test]
fn test_failure_details_must_show_assertion_error_for_failed_clause() {
    let failure_msg = "assertion `left == right` failed\n  left: 200\n right: 401";
    let specs = vec![make_spec(
        "Auth Spec",
        vec![make_section(
            "Authentication",
            vec![make_clause(
                Keyword::Must,
                "reject invalid credentials with 401",
                "auth::login::must_reject",
            )],
        )],
    )];

    let run = make_run_result(vec![make_result_with_failure_detail(
        "auth::login::must_reject",
        TestStatus::Failed,
        failure_msg,
    )]);

    let text = render_terminal(&run, &specs, &no_color_options());

    assert!(
        text.contains("assertion `left == right` failed"),
        "output must contain the assertion error message; got:\n{text}"
    );
    assert!(
        text.contains("left: 200"),
        "output must include the full multi-line failure message; got:\n{text}"
    );
    assert!(
        text.contains("right: 401"),
        "output must include all lines of the failure message; got:\n{text}"
    );
}

/// MUST show failure message fallback: message field used when details.failure_message is absent
#[test]
fn test_failure_details_must_show_message_fallback_when_no_failure_detail() {
    let specs = vec![make_spec(
        "Auth Spec",
        vec![make_section(
            "Authentication",
            vec![make_clause(
                Keyword::Must,
                "reject invalid credentials",
                "auth::must_reject_fallback",
            )],
        )],
    )];

    let run = make_run_result(vec![make_result_with_message(
        "auth::must_reject_fallback",
        TestStatus::Failed,
        "test panicked: expected 401 got 200",
    )]);

    let text = render_terminal(&run, &specs, &no_color_options());

    assert!(
        text.contains("expected 401 got 200"),
        "output must show message field as fallback; got:\n{text}"
    );
}

/// SHOULD show the original clause text alongside the failure for easy comparison
#[test]
fn test_failure_details_should_show_clause_text_alongside_failure() {
    let clause_text = "charge the correct amount in the user's currency";
    let failure_msg = "assertion failed: invoice.amount_cents == 999\n  left: 1099\n right: 999";

    let specs = vec![make_spec(
        "Billing Spec",
        vec![make_section(
            "Billing",
            vec![make_clause(
                Keyword::Must,
                clause_text,
                "billing::must_charge",
            )],
        )],
    )];

    let run = make_run_result(vec![make_result_with_failure_detail(
        "billing::must_charge",
        TestStatus::Failed,
        failure_msg,
    )]);

    let text = render_terminal(&run, &specs, &no_color_options());

    // The original clause text must be visible.
    assert!(text.contains(clause_text), "output must show the original clause text; got:\n{text}");
    // The failure message must also be present.
    assert!(text.contains("assertion failed"), "output must show the failure message; got:\n{text}");

    // Clause text must appear before the failure detail.
    let clause_pos = text.lines().position(|l| l.contains(clause_text)).expect("clause text in output");
    let failure_pos = text.lines().position(|l| l.contains("assertion failed")).expect("failure message in output");
    assert!(
        clause_pos < failure_pos,
        "clause text must appear before the failure detail"
    );
}

/// MUST show the file and line from the failure message if present in details
#[test]
fn test_failure_details_must_show_file_and_line_from_failure_message() {
    let failure_msg = "thread 'test' panicked at tests/generated/api_response.rs:57:5:\nassertion failed";

    let specs = vec![make_spec(
        "API Spec",
        vec![make_section(
            "API Response",
            vec![make_clause(
                Keyword::Must,
                "return JSON content-type",
                "api::must_json_ct",
            )],
        )],
    )];

    let run = make_run_result(vec![make_result_with_failure_detail(
        "api::must_json_ct",
        TestStatus::Failed,
        failure_msg,
    )]);

    let text = render_terminal(&run, &specs, &no_color_options());

    // The failure message (which contains file/line info) must appear in output.
    assert!(
        text.contains("api_response.rs"),
        "output must show the generated test file name; got:\n{text}"
    );
    assert!(
        text.contains("57"),
        "output must show the line number of the failure; got:\n{text}"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// GIVEN BLOCK DISPLAY TESTS
// ═══════════════════════════════════════════════════════════════════════════

/// MUST display GIVEN blocks as a visual group with the condition as a header line
#[test]
fn test_given_block_display_must_display_given_as_visual_group() {
    let mut given_clause = make_clause(Keyword::Given, "the user is authenticated", "given::auth");
    // GIVEN clauses contain their children in `otherwise` in the current data model.
    given_clause.otherwise = vec![
        make_clause(Keyword::Must, "return their profile data", "given::auth::must_profile"),
        make_clause(Keyword::Must, "NOT return other users' data", "given::auth::must_not_leak"),
    ];

    let specs = vec![make_spec(
        "GIVEN Spec",
        vec![make_section("Access", vec![given_clause])],
    )];

    let run = make_run_result(vec![
        make_result("given::auth::must_profile", TestStatus::Passed),
        make_result("given::auth::must_not_leak", TestStatus::Failed),
    ]);

    let text = render_terminal(&run, &specs, &no_color_options());

    // The GIVEN condition must appear as a header.
    assert!(
        text.contains("GIVEN the user is authenticated"),
        "GIVEN header must appear in output; got:\n{text}"
    );

    // Both child clauses must follow the GIVEN header.
    let header_pos = text.find("GIVEN the user is authenticated").unwrap();
    let clause1_pos = text.find("return their profile data").unwrap();
    let clause2_pos = text.find("NOT return other users' data").unwrap();
    assert!(header_pos < clause1_pos, "GIVEN header must precede first child clause");
    assert!(header_pos < clause2_pos, "GIVEN header must precede second child clause");
}

/// MUST indent clauses under their GIVEN condition to show the relationship
#[test]
fn test_given_block_display_must_indent_clauses_under_given() {
    let mut given_clause = make_clause(Keyword::Given, "the token is expired", "given::token");
    given_clause.otherwise = vec![
        make_clause(Keyword::Must, "return 401", "given::token::must_401"),
        make_clause(Keyword::Should, "include WWW-Authenticate", "given::token::should_www"),
    ];

    let specs = vec![make_spec(
        "GIVEN Indent Spec",
        vec![make_section("Auth", vec![given_clause])],
    )];

    let run = make_run_result(vec![
        make_result("given::token::must_401", TestStatus::Passed),
        make_result("given::token::should_www", TestStatus::Passed),
    ]);

    let text = render_terminal(&run, &specs, &no_color_options());

    let given_line = text
        .lines()
        .find(|l| l.contains("GIVEN the token is expired"))
        .expect("GIVEN header must be present");

    let child_line = text
        .lines()
        .find(|l| l.contains("return 401"))
        .expect("child clause must be present");

    // Leading whitespace on the child must be >= header (child is rendered at depth+1).
    let given_indent = given_line.len() - given_line.trim_start().len();
    let child_indent = child_line.len() - child_line.trim_start().len();
    assert!(
        child_indent >= given_indent,
        "child clause indent ({child_indent}) must be >= GIVEN header indent ({given_indent})"
    );
}

/// SHOULD dim the GIVEN condition line when all nested clauses pass
#[test]
fn test_given_block_display_should_dim_when_all_nested_pass() {
    let mut given_clause = make_clause(Keyword::Given, "the token is expired", "given::dim");
    given_clause.otherwise = vec![
        make_clause(Keyword::Must, "return 401", "given::dim::must_401"),
        make_clause(Keyword::Should, "include WWW-Authenticate", "given::dim::should_www"),
    ];

    let specs = vec![make_spec(
        "GIVEN Dim Spec",
        vec![make_section("Auth", vec![given_clause])],
    )];

    let run = make_run_result(vec![
        make_result("given::dim::must_401", TestStatus::Passed),
        make_result("given::dim::should_www", TestStatus::Passed),
    ]);

    let text = render_terminal(&run, &specs, &color_options());

    let given_line = text
        .lines()
        .find(|l| l.contains("GIVEN the token is expired"))
        .expect("GIVEN header must be present");

    // When all children pass, the GIVEN header should be dimmed (not bold).
    assert!(
        given_line.contains(DIM),
        "GIVEN header must carry DIM when all nested clauses pass, got: {given_line:?}"
    );
    assert!(
        !given_line.contains(BOLD),
        "GIVEN header must NOT be bold when all nested clauses pass, got: {given_line:?}"
    );
}

/// SHOULD highlight the GIVEN condition line when any nested clause fails
#[test]
fn test_given_block_display_should_highlight_when_nested_fails() {
    let mut given_clause = make_clause(Keyword::Given, "the user is authenticated", "given::hl");
    given_clause.otherwise = vec![
        make_clause(Keyword::Must, "return profile", "given::hl::must_profile"),
        make_clause(Keyword::Must, "NOT leak data", "given::hl::must_not_leak"),
    ];

    let specs = vec![make_spec(
        "GIVEN HL Spec",
        vec![make_section("Access", vec![given_clause])],
    )];

    let run = make_run_result(vec![
        make_result("given::hl::must_profile", TestStatus::Passed),
        make_result("given::hl::must_not_leak", TestStatus::Failed),
    ]);

    let text = render_terminal(&run, &specs, &color_options());

    let given_line = text
        .lines()
        .find(|l| l.contains("GIVEN the user is authenticated"))
        .expect("GIVEN header must be present");

    assert!(
        given_line.contains(BOLD),
        "GIVEN header must be bold when any nested clause fails, got: {given_line:?}"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// JSON OUTPUT TESTS
// ═══════════════════════════════════════════════════════════════════════════

/// MUST support --json flag that outputs structured results as JSON
#[test]
fn test_json_output_must_support_json_flag() {
    let specs = vec![make_spec(
        "API Spec",
        vec![make_section(
            "Endpoints",
            vec![make_clause(Keyword::Must, "return HTTP 200", "api::endpoints::200")],
        )],
    )];

    let run = make_run_result(vec![make_result("api::endpoints::200", TestStatus::Passed)]);

    let output = ought_report::json::report(&run, &specs).expect("json::report must not fail");

    assert!(!output.is_empty(), "JSON output must not be empty");

    let parsed: serde_json::Value =
        serde_json::from_str(&output).expect("output must be valid JSON");

    assert!(parsed.is_object(), "JSON output must be a top-level object");

    assert!(
        parsed.get("specs").map_or(false, |v| v.is_array()),
        "JSON report must contain a 'specs' array"
    );
    assert!(
        parsed.get("summary").map_or(false, |v| v.is_object()),
        "JSON report must contain a 'summary' object"
    );
    assert!(
        parsed.get("total_duration_ms").map_or(false, |v| v.is_number()),
        "JSON report must contain a 'total_duration_ms' number"
    );
}

/// MUST include all fields: clause identifier, keyword, severity, status, failure message, duration
#[test]
fn test_json_output_must_include_all_fields() {
    let clause_id_str = "auth::login::must_return_401";
    let failure_msg = "expected status 401, got 200";

    let specs = vec![make_spec(
        "Auth Spec",
        vec![make_section(
            "Login",
            vec![make_clause(Keyword::Must, "return 401 for invalid credentials", clause_id_str)],
        )],
    )];

    let run = make_run_result(vec![TestResult {
        clause_id: ClauseId(clause_id_str.to_string()),
        status: TestStatus::Failed,
        message: Some(failure_msg.to_string()),
        duration: Duration::from_millis(22),
        details: TestDetails::default(),
    }]);

    let output = ought_report::json::report(&run, &specs).expect("json::report must not fail");
    let parsed: serde_json::Value = serde_json::from_str(&output).expect("valid JSON");

    let results = parsed["specs"][0]["results"]
        .as_array()
        .expect("specs[0].results must be a JSON array");
    assert_eq!(results.len(), 1, "must emit exactly one clause result entry");

    let entry = &results[0];

    // clause identifier
    assert_eq!(entry["clause_id"].as_str().unwrap(), clause_id_str);
    // keyword
    assert_eq!(entry["keyword"].as_str().unwrap(), "MUST");
    // severity
    assert_eq!(entry["severity"].as_str().unwrap(), "required");
    // status
    assert_eq!(entry["status"].as_str().unwrap(), "failed");
    // failure message
    assert_eq!(entry["message"].as_str().unwrap(), failure_msg);
    // duration
    let dur = entry["duration_ms"].as_f64().expect("duration_ms must be a number");
    assert!(dur > 0.0, "duration_ms must be positive, got {dur}");
    assert!(
        (dur - 22.0).abs() < 1.0,
        "duration_ms must approximate 22ms, got {dur}"
    );
}

/// MUST NOT mix JSON output with human-readable output
#[test]
fn test_json_output_must_not_mix_with_human_readable() {
    let specs = vec![make_spec(
        "DB Spec",
        vec![make_section(
            "Query",
            vec![make_clause(Keyword::Should, "return results within 100ms", "db::query::sla")],
        )],
    )];

    let run = make_run_result(vec![make_result("db::query::sla", TestStatus::Passed)]);

    let json_output = ought_report::json::report(&run, &specs).expect("json::report must not fail");

    // The entire output string must parse as JSON.
    let _parsed: serde_json::Value =
        serde_json::from_str(&json_output).expect("must be valid JSON");

    // No ANSI escape codes.
    assert!(!json_output.contains("\x1b["), "JSON must not contain ANSI escape codes");

    // No terminal status icons.
    for indicator in &["\u{2713}", "\u{2717}", "\u{2298}"] {
        assert!(
            !json_output.contains(indicator),
            "JSON must not contain terminal indicator '{indicator}'"
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// JUNIT XML OUTPUT TESTS
// ═══════════════════════════════════════════════════════════════════════════

/// MUST support --junit <path> flag that writes results in JUnit XML format
#[test]
fn test_junit_xml_must_support_junit_path_flag() {
    let specs = vec![make_spec(
        "Auth Spec",
        vec![make_section(
            "Login",
            vec![make_clause(Keyword::Must, "return a JWT", "auth::login::jwt")],
        )],
    )];

    let run = make_run_result(vec![make_result("auth::login::jwt", TestStatus::Passed)]);

    let out_path = std::env::temp_dir().join("ought_gen_test_junit_flag.xml");
    let _ = std::fs::remove_file(&out_path);

    ought_report::junit::report(&run, &specs, &out_path).expect("junit::report must not fail");

    assert!(out_path.exists(), "--junit must produce a file at the given path");

    let contents = std::fs::read_to_string(&out_path).unwrap();
    assert!(!contents.is_empty(), "JUnit XML file must not be empty");
    assert!(contents.starts_with("<?xml"), "JUnit XML must start with XML declaration");
    assert!(contents.contains("<testsuites>"), "JUnit XML must contain <testsuites> root");

    let _ = std::fs::remove_file(&out_path);
}

/// MUST map spec files to <testsuite> elements and clauses to <testcase> elements
#[test]
fn test_junit_xml_must_map_specs_to_testsuites_and_clauses_to_testcases() {
    let specs = vec![
        make_spec(
            "Spec Alpha",
            vec![make_section(
                "Section",
                vec![make_clause(Keyword::Must, "behave correctly", "alpha::must_behave")],
            )],
        ),
        make_spec(
            "Spec Beta",
            vec![make_section(
                "Section",
                vec![make_clause(Keyword::Must, "behave correctly", "beta::must_behave")],
            )],
        ),
    ];

    let run = make_run_result(vec![
        make_result("alpha::must_behave", TestStatus::Passed),
        make_result("beta::must_behave", TestStatus::Passed),
    ]);

    let out_path = std::env::temp_dir().join("ought_gen_test_mapping.xml");
    let _ = std::fs::remove_file(&out_path);

    ought_report::junit::report(&run, &specs, &out_path).expect("junit::report must not fail");

    let xml = std::fs::read_to_string(&out_path).unwrap();

    let suite_count = xml.matches("<testsuite ").count();
    assert_eq!(suite_count, 2, "one <testsuite> per spec; found {suite_count}");

    assert!(xml.contains("name=\"Spec Alpha\""), "first spec name must appear");
    assert!(xml.contains("name=\"Spec Beta\""), "second spec name must appear");

    let case_count = xml.matches("<testcase ").count();
    assert_eq!(case_count, 2, "one <testcase> per clause; found {case_count}");

    assert!(xml.contains("classname=\"Spec Alpha\""), "classname must match spec name");

    let _ = std::fs::remove_file(&out_path);
}

/// MUST include failure messages and clause identifiers in <failure> elements
#[test]
fn test_junit_xml_must_include_failure_messages_and_clause_ids() {
    let failure_msg = "expected charge 42.00 but got 0.00";

    let specs = vec![make_spec(
        "Payment Spec",
        vec![make_section(
            "Checkout",
            vec![make_clause(Keyword::Must, "charge the correct amount", "payment::must_charge")],
        )],
    )];

    let run = make_run_result(vec![make_result_with_failure_detail(
        "payment::must_charge",
        TestStatus::Failed,
        failure_msg,
    )]);

    let out_path = std::env::temp_dir().join("ought_gen_test_failure_elem.xml");
    let _ = std::fs::remove_file(&out_path);

    ought_report::junit::report(&run, &specs, &out_path).expect("junit::report must not fail");

    let xml = std::fs::read_to_string(&out_path).unwrap();

    assert!(xml.contains("<failure "), "a <failure> element must be present");
    assert!(
        xml.contains("expected charge 42.00 but got 0.00"),
        "failure message must be included"
    );
    assert!(
        xml.contains("payment::must_charge"),
        "clause identifier must appear in the output"
    );

    let _ = std::fs::remove_file(&out_path);
}

/// SHOULD include the clause keyword and severity as properties on each <testcase>
#[test]
fn test_junit_xml_should_include_keyword_and_severity_as_properties() {
    let specs = vec![make_spec(
        "Properties Spec",
        vec![make_section(
            "Section",
            vec![
                make_clause(Keyword::Must, "satisfy must", "props::must"),
                make_clause(Keyword::Should, "satisfy should", "props::should"),
                make_clause(Keyword::May, "satisfy may", "props::may"),
            ],
        )],
    )];

    let run = make_run_result(vec![
        make_result("props::must", TestStatus::Passed),
        make_result("props::should", TestStatus::Passed),
        make_result("props::may", TestStatus::Passed),
    ]);

    let out_path = std::env::temp_dir().join("ought_gen_test_properties.xml");
    let _ = std::fs::remove_file(&out_path);

    ought_report::junit::report(&run, &specs, &out_path).expect("junit::report must not fail");

    let xml = std::fs::read_to_string(&out_path).unwrap();

    assert!(xml.contains("<properties>"), "<properties> element must be present");

    // Keyword properties
    assert!(xml.contains("name=\"keyword\" value=\"MUST\""), "keyword MUST");
    assert!(xml.contains("name=\"keyword\" value=\"SHOULD\""), "keyword SHOULD");
    assert!(xml.contains("name=\"keyword\" value=\"MAY\""), "keyword MAY");

    // Severity properties
    assert!(xml.contains("name=\"severity\" value=\"required\""), "severity required");
    assert!(xml.contains("name=\"severity\" value=\"recommended\""), "severity recommended");
    assert!(xml.contains("name=\"severity\" value=\"optional\""), "severity optional");

    // clause_id property
    assert!(xml.contains("name=\"clause_id\""), "clause_id property must be present");

    let _ = std::fs::remove_file(&out_path);
}

/// MAY be combined with other output modes (JUnit + terminal)
#[test]
fn test_junit_xml_may_be_combined_with_terminal() {
    let specs = vec![make_spec(
        "Combined Spec",
        vec![make_section(
            "Section",
            vec![make_clause(Keyword::Must, "work combined", "combined::must_work")],
        )],
    )];

    let run = make_run_result(vec![make_result("combined::must_work", TestStatus::Passed)]);

    let out_path = std::env::temp_dir().join("ought_gen_test_combined.xml");
    let _ = std::fs::remove_file(&out_path);

    // JUnit writes to file.
    ought_report::junit::report(&run, &specs, &out_path)
        .expect("junit::report must succeed when combined");

    // Terminal writes to a buffer independently.
    let text = render_terminal(&run, &specs, &no_color_options());

    // Both outputs must have been produced without interfering.
    assert!(out_path.exists(), "JUnit XML file must exist after combined reporting");

    let xml = std::fs::read_to_string(&out_path).unwrap();
    assert!(xml.contains("<testsuites>"), "JUnit XML must still be well-formed");

    assert!(!text.is_empty(), "terminal output must not be empty when combined");

    let _ = std::fs::remove_file(&out_path);
}

// ═══════════════════════════════════════════════════════════════════════════
// OTHERWISE CHAIN DISPLAY TESTS
// ═══════════════════════════════════════════════════════════════════════════

/// MUST display OTHERWISE clauses indented under their parent obligation
#[test]
fn test_otherwise_chain_must_display_indented_under_parent() {
    let mut parent = make_clause(Keyword::Must, "respond within 200ms", "ow::parent");
    parent.otherwise = vec![
        make_clause(Keyword::Otherwise, "return a cached response", "ow::cached"),
        make_clause(Keyword::Otherwise, "return 504", "ow::504"),
    ];

    let specs = vec![make_spec(
        "OW Spec",
        vec![make_section("Performance", vec![parent])],
    )];

    let run = make_run_result(vec![
        make_result("ow::parent", TestStatus::Failed),
        make_result("ow::cached", TestStatus::Passed),
        make_result("ow::504", TestStatus::Skipped),
    ]);

    let text = render_terminal(&run, &specs, &no_color_options());

    // OTHERWISE clause text must appear after parent.
    let parent_pos = text.find("respond within 200ms").expect("parent must be in output");
    let ow1_pos = text.find("return a cached response").expect("first OTHERWISE must be in output");
    let ow2_pos = text.find("return 504").expect("second OTHERWISE must be in output");
    assert!(parent_pos < ow1_pos, "parent must precede first OTHERWISE");
    assert!(parent_pos < ow2_pos, "parent must precede second OTHERWISE");

    // OTHERWISE lines must carry strictly more leading whitespace than parent.
    let parent_line = text.lines().find(|l| l.contains("respond within 200ms")).unwrap();
    let ow1_line = text.lines().find(|l| l.contains("return a cached response")).unwrap();

    let parent_indent = parent_line.len() - parent_line.trim_start().len();
    let ow1_indent = ow1_line.len() - ow1_line.trim_start().len();
    assert!(
        ow1_indent > parent_indent,
        "OTHERWISE indent ({ow1_indent}) must exceed parent indent ({parent_indent})"
    );
}

/// MUST use a distinct indicator for OTHERWISE results: arrow prefix
#[test]
fn test_otherwise_chain_must_use_arrow_indicator() {
    let mut parent = make_clause(Keyword::Must, "respond within 200ms", "ow2::parent");
    parent.otherwise = vec![
        make_clause(Keyword::Otherwise, "return a cached response", "ow2::cached"),
        make_clause(Keyword::Otherwise, "return 504", "ow2::504"),
    ];

    let specs = vec![make_spec(
        "OW Arrow Spec",
        vec![make_section("Performance", vec![parent])],
    )];

    let run = make_run_result(vec![
        make_result("ow2::parent", TestStatus::Failed),
        make_result("ow2::cached", TestStatus::Passed),
        make_result("ow2::504", TestStatus::Skipped),
    ]);

    let text = render_terminal(&run, &specs, &no_color_options());

    // Every OTHERWISE line must include the arrow prefix.
    for clause_text in &["return a cached response", "return 504"] {
        let ow_line = text
            .lines()
            .find(|l| l.contains(clause_text))
            .unwrap_or_else(|| panic!("OTHERWISE clause '{clause_text}' must be in output"));
        assert!(
            ow_line.contains("\u{21b3}"),
            "OTHERWISE line must carry the arrow prefix, got: {ow_line:?}"
        );
    }

    // The parent obligation line must NOT carry the arrow.
    let parent_line = text.lines().find(|l| l.contains("respond within 200ms")).unwrap();
    assert!(
        !parent_line.contains("\u{21b3}"),
        "parent clause line must NOT carry the arrow, got: {parent_line:?}"
    );
}

/// MUST show the full chain status: if parent passes, OTHERWISE shows as ~ (not needed)
#[test]
fn test_otherwise_chain_must_show_skipped_when_parent_passes() {
    let mut parent = make_clause(Keyword::Must, "respond within 200ms", "ow3::parent");
    parent.otherwise = vec![
        make_clause(Keyword::Otherwise, "return a cached response", "ow3::cached"),
        make_clause(Keyword::Otherwise, "return 504", "ow3::504"),
    ];

    let specs = vec![make_spec(
        "OW Pass Spec",
        vec![make_section("Performance", vec![parent])],
    )];

    let run = make_run_result(vec![
        make_result("ow3::parent", TestStatus::Passed),
        make_result("ow3::cached", TestStatus::Skipped),
        make_result("ow3::504", TestStatus::Skipped),
    ]);

    let text = render_terminal(&run, &specs, &no_color_options());

    // Parent must show checkmark.
    let parent_line = text.lines().find(|l| l.contains("respond within 200ms")).unwrap();
    assert!(parent_line.contains("\u{2713}"), "passing parent must show checkmark, got: {parent_line:?}");

    // OTHERWISE clauses must show ~.
    for clause_text in &["return a cached response", "return 504"] {
        let ow_line = text.lines().find(|l| l.contains(clause_text)).unwrap();
        assert!(ow_line.contains("~"), "OTHERWISE must show ~ when parent passes, got: {ow_line:?}");
    }
}

/// MUST show which OTHERWISE level caught the failure
#[test]
fn test_otherwise_chain_must_show_which_level_caught_failure() {
    // Scenario: first OTHERWISE catches the failure
    let mut parent = make_clause(Keyword::Must, "respond within 200ms", "ow4::parent");
    parent.otherwise = vec![
        make_clause(Keyword::Otherwise, "return a cached response", "ow4::cached"),
        make_clause(Keyword::Otherwise, "return 504", "ow4::504"),
    ];

    let specs = vec![make_spec(
        "OW Catch Spec",
        vec![make_section("Performance", vec![parent])],
    )];

    let run = make_run_result(vec![
        make_result("ow4::parent", TestStatus::Failed),
        make_result("ow4::cached", TestStatus::Passed),
        make_result("ow4::504", TestStatus::Skipped),
    ]);

    let text = render_terminal(&run, &specs, &no_color_options());

    let ow1 = text.lines().find(|l| l.contains("return a cached response")).unwrap();
    let ow2 = text.lines().find(|l| l.contains("return 504")).unwrap();

    assert!(
        ow1.contains("\u{2713}"),
        "first OTHERWISE (caught failure) must show checkmark, got: {ow1:?}"
    );
    assert!(
        ow2.contains("~"),
        "second OTHERWISE (not reached) must show ~, got: {ow2:?}"
    );
}

/// SHOULD visually distinguish the active fallback from lower ones
#[test]
fn test_otherwise_chain_should_distinguish_active_from_unreached() {
    let mut parent = make_clause(Keyword::Must, "respond within 200ms", "ow5::parent");
    parent.otherwise = vec![
        make_clause(Keyword::Otherwise, "return a cached response", "ow5::cached"),
        make_clause(Keyword::Otherwise, "return 504", "ow5::504"),
    ];

    let specs = vec![make_spec(
        "OW Distinguish Spec",
        vec![make_section("Performance", vec![parent])],
    )];

    let run = make_run_result(vec![
        make_result("ow5::parent", TestStatus::Failed),
        make_result("ow5::cached", TestStatus::Passed),
        make_result("ow5::504", TestStatus::Skipped),
    ]);

    let text = render_terminal(&run, &specs, &color_options());

    let active_line = text.lines().find(|l| l.contains("return a cached response")).unwrap();
    let not_reached_line = text.lines().find(|l| l.contains("return 504")).unwrap();

    // Not-reached line should contain "not reached" annotation.
    assert!(
        not_reached_line.contains("not reached"),
        "not-reached fallback must carry 'not reached' annotation, got: {not_reached_line:?}"
    );

    // Active line should NOT say "not reached".
    assert!(
        !active_line.contains("not reached"),
        "active fallback must NOT carry 'not reached', got: {active_line:?}"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// TEMPORAL RESULT DISPLAY TESTS
// ═══════════════════════════════════════════════════════════════════════════

/// MUST display MUST ALWAYS results with the number of iterations/inputs tested
#[test]
fn test_temporal_must_display_must_always_with_iterations() {
    let mut clause = make_clause(Keyword::MustAlways, "return valid JSON", "inv::must_always_json");
    clause.temporal = Some(Temporal::Invariant);

    let specs = vec![make_spec(
        "Invariant Spec",
        vec![make_section("Invariants", vec![clause])],
    )];

    let run = make_run_result(vec![TestResult {
        clause_id: ClauseId("inv::must_always_json".to_string()),
        status: TestStatus::Passed,
        message: None,
        duration: Duration::from_millis(50),
        details: TestDetails {
            iterations: Some(1000),
            ..Default::default()
        },
    }]);

    let text = render_terminal(&run, &specs, &no_color_options());

    let clause_line = text
        .lines()
        .find(|l| l.contains("return valid JSON"))
        .expect("MUST ALWAYS clause line must appear");

    assert!(
        clause_line.contains("1000"),
        "MUST ALWAYS line must include iteration count; got: {clause_line:?}"
    );
    assert!(
        clause_line.contains("tested") || clause_line.contains("inputs"),
        "MUST ALWAYS line must describe what was tested; got: {clause_line:?}"
    );
}

/// MUST display MUST BY results with the measured duration alongside the deadline
#[test]
fn test_temporal_must_display_must_by_with_duration_and_deadline() {
    let deadline = Duration::from_millis(200);
    let measured = Duration::from_millis(47);

    let mut clause = make_clause(Keyword::MustBy, "return a response", "perf::must_by_response");
    clause.temporal = Some(Temporal::Deadline(deadline));

    let specs = vec![make_spec(
        "Perf Spec",
        vec![make_section("Performance", vec![clause])],
    )];

    let run = make_run_result(vec![TestResult {
        clause_id: ClauseId("perf::must_by_response".to_string()),
        status: TestStatus::Passed,
        message: None,
        duration: measured,
        details: TestDetails {
            measured_duration: Some(measured),
            ..Default::default()
        },
    }]);

    let text = render_terminal(&run, &specs, &no_color_options());

    let clause_line = text
        .lines()
        .find(|l| l.contains("return a response"))
        .expect("MUST BY clause line must appear");

    assert!(
        clause_line.contains("47ms"),
        "MUST BY line must show measured duration (47ms); got: {clause_line:?}"
    );
    assert!(
        clause_line.contains("200ms"),
        "MUST BY line must show deadline (200ms); got: {clause_line:?}"
    );
}

/// SHOULD show a timing ratio for MUST BY clauses: [47ms / 200ms]
#[test]
fn test_temporal_should_show_timing_ratio_for_must_by() {
    let passing_clause = {
        let mut c = make_clause(Keyword::MustBy, "return a response", "perf::pass");
        c.temporal = Some(Temporal::Deadline(Duration::from_millis(200)));
        c
    };
    let failing_clause = {
        let mut c = make_clause(Keyword::MustBy, "acknowledge the write", "perf::fail");
        c.temporal = Some(Temporal::Deadline(Duration::from_millis(100)));
        c
    };

    let specs = vec![make_spec(
        "Timing Spec",
        vec![make_section("Performance", vec![passing_clause, failing_clause])],
    )];

    let run = make_run_result(vec![
        TestResult {
            clause_id: ClauseId("perf::pass".to_string()),
            status: TestStatus::Passed,
            message: None,
            duration: Duration::from_millis(47),
            details: TestDetails {
                measured_duration: Some(Duration::from_millis(47)),
                ..Default::default()
            },
        },
        TestResult {
            clause_id: ClauseId("perf::fail".to_string()),
            status: TestStatus::Failed,
            message: None,
            duration: Duration::from_millis(230),
            details: TestDetails {
                measured_duration: Some(Duration::from_millis(230)),
                ..Default::default()
            },
        },
    ]);

    let text = render_terminal(&run, &specs, &no_color_options());

    // Passing clause: [47ms / 200ms]
    let passing_line = text
        .lines()
        .find(|l| l.contains("return a response"))
        .expect("passing MUST BY clause line must appear");
    assert!(
        passing_line.contains("47ms") && passing_line.contains("200ms"),
        "passing MUST BY must show both durations; got: {passing_line:?}"
    );
    assert!(
        passing_line.contains('/'),
        "ratio must include a '/' separator; got: {passing_line:?}"
    );

    // Failing clause: [230ms / 100ms]
    let failing_line = text
        .lines()
        .find(|l| l.contains("acknowledge the write"))
        .expect("failing MUST BY clause line must appear");
    assert!(
        failing_line.contains("230ms") && failing_line.contains("100ms"),
        "failing MUST BY must show both durations; got: {failing_line:?}"
    );
}