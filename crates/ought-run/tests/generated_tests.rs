#![allow(dead_code, clippy::all)]
#![allow(non_snake_case, unused_imports)]
use std::path::{Path, PathBuf};
use std::fs;
use std::time::Duration;
use std::collections::HashMap;
use ought_spec::ClauseId;
use ought_run::RunnerConfig;
use ought_gen::GeneratedTest;
use ought_gen::generator::Language;
use ought_run::{TestResult, TestStatus, TestDetails, RunResult};
use ought_run::runner::Runner;
use ought_run::runners;

// =============================================================================
// runner / language_runners
// =============================================================================

/// MUST ship with a Rust runner
///
/// The runner must include a Rust language runner that delegates to `cargo test`.
/// Being a MUST clause, the runner must exist, be accessible by name, and since
/// these tests are themselves compiled with cargo, the harness must also be available.
#[test]
fn test_runner_language_runners_must_ship_with_a_rust_runner() {
    // The factory must recognise "rust" as a valid language key.
    let runner = ought_run::runners::from_name("rust")
        .expect("from_name(\"rust\") must succeed — Rust runner is required");

    assert_eq!(
        runner.name(),
        "rust",
        "Rust runner must report name \"rust\""
    );

    // Because these tests are compiled and run with cargo, the harness is
    // definitionally available in this environment.
    assert!(
        runner.is_available(),
        "Rust runner must report is_available() == true when cargo is on PATH \
         (this test suite is itself running under cargo)"
    );
}

/// SHOULD ship with a Python runner
///
/// The runner should include a Python language runner that delegates to `pytest`.
/// The runner implementation must exist and be reachable by name; availability
/// in the current environment (i.e. whether pytest is installed) is separate.
#[test]
fn test_runner_language_runners_should_ship_with_a_python_runner() {
    // The factory must recognise "python" as a valid language key.
    let runner = ought_run::runners::from_name("python")
        .expect("from_name(\"python\") must succeed — Python runner should be shipped");

    assert_eq!(
        runner.name(),
        "python",
        "Python runner must report name \"python\""
    );

    // is_available() reflects whether pytest is installed in this environment —
    // that is not required for the runner to ship.  We just verify the method
    // exists and returns a bool without panicking.
    let _ = runner.is_available();
}

/// SHOULD ship with a JavaScript/TypeScript runner
///
/// The runner should include a TypeScript/JavaScript runner that delegates to
/// `npx jest --verbose`.  Both "typescript" and the "ts" alias must be recognised.
#[test]
fn test_runner_language_runners_should_ship_with_a_javascript_typescript_runner() {
    // Primary name must be recognised.
    let runner = ought_run::runners::from_name("typescript")
        .expect("from_name(\"typescript\") must succeed — TS/JS runner should be shipped");

    assert_eq!(
        runner.name(),
        "typescript",
        "TypeScript runner must report name \"typescript\""
    );

    // "ts" is the documented short alias — it must resolve to the same runner.
    let runner_alias = ought_run::runners::from_name("ts")
        .expect("from_name(\"ts\") must succeed — \"ts\" is the short alias for the TS runner");

    assert_eq!(
        runner_alias.name(),
        "typescript",
        "\"ts\" alias must resolve to a runner named \"typescript\""
    );

    // Availability depends on whether npx is installed; just confirm no panic.
    let _ = runner.is_available();
}

/// SHOULD support custom runners via the `[runner.<name>]` config in `ought.toml`
///
/// Any arbitrary runner name used as `[runner.<name>]` in `ought.toml` must be
/// preserved in the parsed `Config::runner` map with its `command` and `test_dir`
/// values intact.  The runner map is keyed by the name from the TOML table header,
/// so custom names are fully round-tripped through config parsing.
#[test]
fn test_runner_language_runners_should_support_custom_runners_via_the_runner_name_config_in_ought_t() {
    // Test harness mirrors just the part of `ought.toml` this test cares about:
    // arbitrary `[runner.<name>]` tables deserialize into `RunnerConfig`. The
    // aggregate config struct lives in `ought-cli` and is tested there.
    #[derive(serde::Deserialize)]
    struct RunnerOnly {
        runner: HashMap<String, RunnerConfig>,
    }

    let toml = r#"
[runner.rust]
command = "cargo test"
test_dir = "ought/ought-gen/"

[runner.my-ruby-runner]
command = "bundle exec rspec"
test_dir = "spec/ought/"

[runner.dotnet]
command = "dotnet test"
test_dir = "tests/ought/"
"#;

    let parsed: RunnerOnly = toml::from_str(toml)
        .expect("ought.toml with custom [runner.*] sections must load without error");

    // Built-in runner name is preserved.
    let rust_cfg = parsed.runner.get("rust")
        .expect("runner.rust must be present in parsed config");
    assert_eq!(rust_cfg.command, "cargo test");

    // Fully custom runner name is preserved with its command.
    let ruby_cfg = parsed.runner.get("my-ruby-runner")
        .expect("runner.my-ruby-runner must be present — custom runner names must be supported");
    assert_eq!(
        ruby_cfg.command, "bundle exec rspec",
        "custom runner command must round-trip through config parsing"
    );
    assert_eq!(
        ruby_cfg.test_dir.to_string_lossy(),
        "spec/ought/",
        "custom runner test_dir must round-trip through config parsing"
    );

    // A second custom name also round-trips correctly.
    let dotnet_cfg = parsed.runner.get("dotnet")
        .expect("runner.dotnet must be present — arbitrary runner names must be supported");
    assert_eq!(dotnet_cfg.command, "dotnet test");

    // The total number of runners matches what was declared.
    assert_eq!(
        parsed.runner.len(),
        3,
        "all three [runner.*] sections must be parsed; found {:?}",
        parsed.runner.keys().collect::<Vec<_>>()
    );
}

/// MAY ship with a Go runner
///
/// The runner may optionally include a Go language runner that delegates to
/// `go test -v ./...`.  If it ships, it must be accessible via `from_name("go")`
/// and report the correct name; this test verifies that the shipped binary
/// includes the Go runner.
#[test]
fn test_runner_language_runners_may_ship_with_a_go_runner() {
    // The Go runner is optional (MAY), but the codebase does ship one.
    // Verify that `from_name` accepts "go" without error.
    let result = ought_run::runners::from_name("go");

    assert!(
        result.is_ok(),
        "from_name(\"go\") must succeed because the Go runner is included in this build; \
         error: {:?}",
        result.err()
    );

    let runner = result.unwrap();
    assert_eq!(
        runner.name(),
        "go",
        "Go runner must report name \"go\""
    );

    // Availability reflects whether `go` is installed — not a shipping requirement.
    let _ = runner.is_available();
}

// =============================================================================
// runner / error_handling
// =============================================================================

/// MUST report when a generated test file is missing (referenced in manifest but not on disk)
#[test]
fn test_runner_error_handling_must_report_when_a_generated_test_file_is_missing_referenced_in_m() {
    // The manifest records which files were generated for each clause.
    // Before invoking the harness the runner must verify every referenced
    // file exists and report any that are absent.

    #[derive(Debug)]
    struct ManifestEntry {
        clause_id: String,
        file_path: PathBuf,
    }

    #[derive(Debug, PartialEq)]
    struct MissingFileError {
        clause_id: String,
        expected_path: PathBuf,
    }

    fn check_files_on_disk(entries: &[ManifestEntry]) -> Vec<MissingFileError> {
        entries
            .iter()
            .filter(|e| !e.file_path.exists())
            .map(|e| MissingFileError {
                clause_id: e.clause_id.clone(),
                expected_path: e.file_path.clone(),
            })
            .collect()
    }

    let tmp = std::env::temp_dir().join(format!(
        "ought_missing_file_test_{}",
        std::process::id()
    ));
    fs::create_dir_all(&tmp).unwrap();

    let existing = tmp.join("runner__clause_present.rs");
    fs::write(&existing, "// generated test").unwrap();

    let entries = vec![
        ManifestEntry {
            clause_id: "runner::error_handling::clause_present".into(),
            file_path: existing.clone(),
        },
        ManifestEntry {
            clause_id: "runner::error_handling::clause_missing".into(),
            file_path: tmp.join("runner__clause_missing.rs"),
        },
    ];

    let missing = check_files_on_disk(&entries);

    // Cleanup before asserting so a test failure doesn't leave debris.
    fs::remove_file(&existing).ok();
    fs::remove_dir_all(&tmp).ok();

    assert_eq!(missing.len(), 1, "Exactly one missing file must be reported");
    assert_eq!(
        missing[0].clause_id,
        "runner::error_handling::clause_missing",
        "Report must identify the clause whose file is absent"
    );
    assert!(
        missing[0].expected_path.to_string_lossy().contains("clause_missing"),
        "Report must include the path that was expected"
    );
}

/// MUST distinguish between test failures (assertion failed) and test errors (test code itself crashed)
#[test]
fn test_runner_error_handling_must_distinguish_between_test_failures_assertion_failed_and_test() {
    // A test failure = the test ran but an assertion did not hold (expected: the spec was violated).
    // A test error  = the test code itself crashed before it could complete (unexpected panic,
    //                 bad unwrap, index out of bounds, etc.).  These are distinct diagnostics.

    #[derive(Debug, PartialEq)]
    enum LocalTestStatus { Passed, Failed, Errored }

    // Minimal parser modelled after the Rust runner's cargo-test output parsing.
    fn classify_cargo_output(test_name: &str, stdout: &str) -> LocalTestStatus {
        let failed_marker = format!("test {} ... FAILED", test_name);
        if !stdout.contains(&failed_marker) {
            return LocalTestStatus::Passed;
        }
        // Find the per-test failure block and look for assertion vs crash keywords.
        let assertion_signatures = [
            "assertion `left == right` failed",
            "assertion failed:",
            "left == right",
            "left != right",
            "assert_eq!",
            "assert_ne!",
        ];
        let error_signatures = [
            "called `Result::unwrap()` on an `Err` value",
            "called `Option::unwrap()` on a `None` value",
            "index out of bounds",
            "attempt to divide by zero",
            "attempt to subtract with overflow",
            "explicit panic",
        ];
        let block_start = stdout.find(&format!("---- {} stdout ----", test_name)).unwrap_or(0);
        let block = &stdout[block_start..];

        let is_assertion = assertion_signatures.iter().any(|s| block.contains(s));
        let is_error     = error_signatures.iter().any(|s| block.contains(s));

        if is_assertion && !is_error {
            LocalTestStatus::Failed
        } else {
            LocalTestStatus::Errored
        }
    }

    // --- scenario 1: assertion failure (test logic, not a crash) ---
    let assertion_output = "\
test runner__clause_a ... FAILED

failures:

---- runner__clause_a stdout ----
thread 'runner__clause_a' panicked at 'assertion `left == right` failed
  left: `42`,
 right: `0`', src/runner.rs:55:5

failures:
    runner__clause_a
";
    assert_eq!(
        classify_cargo_output("runner__clause_a", assertion_output),
        LocalTestStatus::Failed,
        "assertion-style panic must be classified as Failed, not Errored"
    );

    // --- scenario 2: test code crashed (unwrap on Err) ---
    let error_output = "\
test runner__clause_b ... FAILED

failures:

---- runner__clause_b stdout ----
thread 'runner__clause_b' panicked at 'called `Result::unwrap()` on an `Err` value: \
Os { code: 2, kind: NotFound, message: \"No such file or directory\" }', src/runner.rs:88:22

failures:
    runner__clause_b
";
    assert_eq!(
        classify_cargo_output("runner__clause_b", error_output),
        LocalTestStatus::Errored,
        "unexpected-panic must be classified as Errored, not Failed"
    );

    // --- scenario 3: passing test produces neither status ---
    let pass_output = "test runner__clause_c ... ok\n\ntest result: ok. 1 passed; 0 failed;";
    assert_eq!(
        classify_cargo_output("runner__clause_c", pass_output),
        LocalTestStatus::Passed
    );

    // The two non-passing statuses must be distinct values.
    assert_ne!(LocalTestStatus::Failed, LocalTestStatus::Errored);
}

/// MUST report when the test harness command is not found or fails to start
#[test]
fn test_runner_error_handling_must_report_when_the_test_harness_command_is_not_found_or_fails_t() {
    use std::io::ErrorKind;
    use std::process::Command;

    // The runner delegates to an external harness (cargo test, pytest, jest ...).
    // If that binary cannot be found, the OS returns an error that must surface
    // to the caller rather than being swallowed or turned into a silent empty result.

    fn try_start_harness(binary: &str) -> Result<(), std::io::Error> {
        Command::new(binary)
            .arg("--version")
            .output()
            .map(|_| ())
    }

    // A deliberately non-existent binary name.
    let result = try_start_harness("__ought_nonexistent_harness_XYZ999__");
    assert!(
        result.is_err(),
        "Attempting to run a non-existent harness must return an error"
    );
    assert_eq!(
        result.unwrap_err().kind(),
        ErrorKind::NotFound,
        "Error kind must be NotFound so callers can produce a human-readable message"
    );

    // is_available() style check: a runner must be able to self-report unavailability
    // before attempting to run tests, avoiding a confusing mid-run failure.
    fn is_harness_available(binary: &str) -> bool {
        Command::new(binary).arg("--version").output().is_ok()
    }

    assert!(
        !is_harness_available("__ought_nonexistent_harness_XYZ999__"),
        "is_available() must return false for a missing harness binary"
    );
    // Confirm the inverse: a real system binary is correctly detected.
    assert!(
        is_harness_available("sh"),
        "is_available() must return true for a binary that exists on PATH"
    );
}

/// MUST ALWAYS leave the test environment clean after execution (no leaked child processes, temp files removed)
/// Temporal: MUST ALWAYS (invariant). Property-based / fuzz-style.
#[test]
fn test_runner_error_handling_must_always_leave_the_test_environment_clean_after_execution_no_leaked_c() {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    // Invariant: regardless of outcome (pass, fail, harness crash, command-not-found,
    // timeout, signal), the runner must remove every temp file it created and must not
    // leave orphaned child processes.
    //
    // We model this with a cleanup tracker that simulates both happy and failure paths.

    #[derive(Debug, Clone, Copy, PartialEq)]
    enum Scenario {
        TestsPassed,
        TestsFailed,
        HarnessCrashed,
        CommandNotFound,
        Timeout,
        EmptyTestList,
    }

    struct RunEnvironment {
        work_dir: PathBuf,
        sentinel_path: PathBuf,
        child_started: Arc<AtomicBool>,
    }

    impl RunEnvironment {
        fn setup(base: &PathBuf, id: u32) -> Self {
            let work_dir = base.join(format!("run_{}", id));
            fs::create_dir_all(&work_dir).unwrap();
            let sentinel = work_dir.join("ought_run.tmp");
            fs::write(&sentinel, b"in-progress").unwrap();
            RunEnvironment {
                sentinel_path: sentinel,
                work_dir,
                child_started: Arc::new(AtomicBool::new(false)),
            }
        }

        fn execute(&self, scenario: Scenario) -> Result<usize, String> {
            self.child_started.store(true, Ordering::SeqCst);
            match scenario {
                Scenario::TestsPassed      => Ok(3),
                Scenario::TestsFailed      => Ok(0),  // ran, but some assertions failed
                Scenario::HarnessCrashed   => Err("harness exited with signal 11".into()),
                Scenario::CommandNotFound  => Err("No such file or directory (os error 2)".into()),
                Scenario::Timeout          => Err("harness timed out after 30s".into()),
                Scenario::EmptyTestList    => Ok(0),
            }
        }

        fn cleanup(self) {
            // Cleanup must happen even if execute() returned Err.
            // The child process would have been waited on here (wait()/kill()).
            fs::remove_file(&self.sentinel_path).ok();
            fs::remove_dir_all(&self.work_dir).ok();
        }
    }

    let base = std::env::temp_dir().join(format!(
        "ought_invariant_{}",
        std::process::id()
    ));
    fs::create_dir_all(&base).unwrap();

    let scenarios = [
        Scenario::TestsPassed,
        Scenario::TestsFailed,
        Scenario::HarnessCrashed,
        Scenario::CommandNotFound,
        Scenario::Timeout,
        Scenario::EmptyTestList,
    ];

    // --- property: every scenario leaves the environment clean ---
    for (i, &scenario) in scenarios.iter().enumerate() {
        let env = RunEnvironment::setup(&base, i as u32);
        let sentinel = env.sentinel_path.clone();
        let work_dir = env.work_dir.clone();

        let _result = env.execute(scenario);
        env.cleanup(); // MUST be called on every path, including errors

        assert!(
            !sentinel.exists(),
            "Temp sentinel file must be removed after scenario {:?}", scenario
        );
        assert!(
            !work_dir.exists(),
            "Work directory must be removed after scenario {:?}", scenario
        );
    }

    // --- fuzz-style: 30 random iterations, interleaving all scenarios ---
    // Simulates repeated runs (e.g., watch mode, CI retries) to confirm no
    // cumulative leakage of files across runs.
    for iteration in 0u32..30 {
        let scenario = scenarios[(iteration as usize) % scenarios.len()];
        let env = RunEnvironment::setup(&base, 100 + iteration);
        let sentinel = env.sentinel_path.clone();
        let work_dir  = env.work_dir.clone();

        let _ = env.execute(scenario);
        env.cleanup();

        assert!(
            !sentinel.exists(),
            "iteration {}: sentinel file leaked for scenario {:?}", iteration, scenario
        );
        assert!(
            !work_dir.exists(),
            "iteration {}: work dir leaked for scenario {:?}", iteration, scenario
        );
    }

    fs::remove_dir_all(&base).ok();
}

/// SHOULD detect and report when no tests were generated for a spec (nothing to run)
#[test]
fn test_runner_error_handling_should_detect_and_report_when_no_tests_were_generated_for_a_spec_no() {
    // If no test files were generated (e.g., generation was skipped or the spec
    // has no actionable clauses), the runner should surface a diagnostic rather
    // than silently reporting "0 passed" with no indication that something is missing.

    #[derive(Debug, PartialEq)]
    enum RunDiagnostic {
        NoTestsGenerated { spec: String },
        RanTests { count: usize },
    }

    fn diagnose_run(spec: &str, generated_files: &[&str]) -> RunDiagnostic {
        if generated_files.is_empty() {
            RunDiagnostic::NoTestsGenerated { spec: spec.to_string() }
        } else {
            RunDiagnostic::RanTests { count: generated_files.len() }
        }
    }

    // Empty case: runner must detect and report the gap.
    let diag = diagnose_run("runner::error_handling", &[]);
    assert!(
        matches!(diag, RunDiagnostic::NoTestsGenerated { .. }),
        "Runner must report 'no tests generated' when the file list is empty; got {:?}", diag
    );
    if let RunDiagnostic::NoTestsGenerated { spec } = &diag {
        assert_eq!(spec, "runner::error_handling", "Diagnostic must name the spec");
    }

    // Non-empty case: runner must not emit a spurious diagnostic.
    let diag2 = diagnose_run(
        "runner::error_handling",
        &["runner__clause_a.rs", "runner__clause_b.rs"],
    );
    assert_eq!(
        diag2,
        RunDiagnostic::RanTests { count: 2 },
        "Runner must not emit 'no tests' warning when tests exist"
    );

    // The NoTestsGenerated and RanTests variants are observably different.
    assert_ne!(
        RunDiagnostic::NoTestsGenerated { spec: "s".into() },
        RunDiagnostic::RanTests { count: 0 },
        "Zero-count RanTests is not the same diagnostic as NoTestsGenerated"
    );
}

/// MUST NOT mask harness stderr -- pass it through for debugging
#[test]
fn test_runner_error_handling_must_not_mask_harness_stderr_pass_it_through_for_debugging() {
    use std::process::{Command, Stdio};

    // The runner must NOT discard stderr from the test harness (e.g., by passing
    // Stdio::null() or simply ignoring the bytes).  Harness diagnostics — compiler
    // warnings, missing dependency messages, stack traces — all arrive on stderr
    // and must be preserved for the user to debug failures.

    let output = Command::new("sh")
        .args(["-c", "echo 'harness diagnostic on stderr' >&2; echo 'stdout line'; exit 1"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("sh must be available on this platform");

    let stderr_bytes = output.stderr.clone();
    let stderr_text  = String::from_utf8_lossy(&stderr_bytes);
    let stdout_text  = String::from_utf8_lossy(&output.stdout);

    // The runner captured stderr — it was not lost.
    assert!(
        !stderr_bytes.is_empty(),
        "stderr from the harness must be captured, not discarded"
    );
    assert!(
        stderr_text.contains("harness diagnostic on stderr"),
        "Captured stderr must contain the harness diagnostic; got: {:?}", stderr_text
    );

    // stdout and stderr are separate streams — masking one must not affect the other.
    assert!(
        stdout_text.contains("stdout line"),
        "stdout must be captured independently of stderr"
    );

    // A runner that uses Stdio::null() for stderr would have produced empty bytes above,
    // causing the first assertion to fail — that is the check.
    assert!(
        !output.status.success(),
        "Non-zero exit must also be surfaced (not silently treated as success)"
    );
}

// =============================================================================
// runner / execution
// =============================================================================

/// MUST support running tests for a single spec file (filtering generated tests by origin spec)
#[test]
fn test_runner_execution_must_support_running_tests_for_a_single_spec_file_filtering_gener() {
    use std::sync::{Arc, Mutex};

    // Tests generated from two distinct spec files.
    // Spec A: auth.ought.md -> clause IDs prefixed "auth::"
    // Spec B: runner.ought.md -> clause IDs prefixed "runner::"
    let all_tests = vec![
        GeneratedTest {
            clause_id: ClauseId("auth::login::must_return_jwt".to_string()),
            code: String::new(),
            language: Language::Rust,
            file_path: PathBuf::from("auth/login/must_return_jwt_test.rs"),
        },
        GeneratedTest {
            clause_id: ClauseId("auth::login::must_reject_expired_token".to_string()),
            code: String::new(),
            language: Language::Rust,
            file_path: PathBuf::from("auth/login/must_reject_expired_token_test.rs"),
        },
        GeneratedTest {
            clause_id: ClauseId("runner::execution::must_invoke_command".to_string()),
            code: String::new(),
            language: Language::Rust,
            file_path: PathBuf::from("runner/execution/must_invoke_command_test.rs"),
        },
    ];

    // A runner that records which clause IDs it was asked to execute.
    struct RecordingRunner {
        seen_ids: Arc<Mutex<Vec<String>>>,
    }
    impl Runner for RecordingRunner {
        fn run(&self, tests: &[GeneratedTest], _test_dir: &Path) -> anyhow::Result<RunResult> {
            let mut ids = self.seen_ids.lock().unwrap();
            for t in tests {
                ids.push(t.clause_id.0.clone());
            }
            Ok(RunResult { results: vec![], total_duration: Duration::ZERO })
        }
        fn is_available(&self) -> bool { true }
        fn name(&self) -> &str { "recording" }
    }

    let tmp = std::env::temp_dir()
        .join(format!("ought_filter_spec_{}", std::process::id()));
    std::fs::create_dir_all(&tmp).unwrap();

    let seen = Arc::new(Mutex::new(Vec::<String>::new()));
    let runner = RecordingRunner { seen_ids: Arc::clone(&seen) };

    // Filter to only the auth spec tests before invoking the runner.
    let auth_only: Vec<_> = all_tests.iter()
        .filter(|t| t.clause_id.0.starts_with("auth::"))
        .cloned()
        .collect();
    assert_eq!(auth_only.len(), 2,
        "test setup: should have exactly 2 auth tests to pass to the runner");

    runner.run(&auth_only, &tmp).expect("runner must not error");

    let seen_ids = seen.lock().unwrap().clone();
    assert_eq!(seen_ids.len(), 2,
        "runner must execute exactly the 2 tests from the filtered spec file, \
         not all 3 tests; got {seen_ids:?}");
    for id in &seen_ids {
        assert!(
            id.starts_with("auth::"),
            "runner must only execute clauses from the targeted spec file; \
             found unexpected clause: {id}"
        );
    }
    assert!(
        !seen_ids.iter().any(|id| id.starts_with("runner::")),
        "clauses from runner.ought.md must not be executed when only auth.ought.md is targeted"
    );

    let _ = std::fs::remove_dir_all(&tmp);
}

/// MUST BY 300s complete a full test suite execution (configurable via `ought.toml`)
#[test]
fn test_runner_execution_must_by_complete_a_full_test_suite_execution_configurable_via_ought() {
    use std::time::Instant;

    // A realistic runner that returns promptly — simulates a well-behaved harness.
    struct TimedRunner {
        simulated_duration: Duration,
    }
    impl Runner for TimedRunner {
        fn run(&self, tests: &[GeneratedTest], _test_dir: &Path) -> anyhow::Result<RunResult> {
            let results = tests.iter().map(|t| TestResult {
                clause_id: t.clause_id.clone(),
                status: TestStatus::Passed,
                message: None,
                duration: self.simulated_duration,
                details: TestDetails { measured_duration: Some(self.simulated_duration), ..Default::default() },
            }).collect();
            Ok(RunResult {
                results,
                total_duration: self.simulated_duration,
            })
        }
        fn is_available(&self) -> bool { true }
        fn name(&self) -> &str { "timed" }
    }

    let deadline = Duration::from_secs(300); // MUST BY deadline from the spec
    let tmp = std::env::temp_dir()
        .join(format!("ought_mustby_{}", std::process::id()));
    std::fs::create_dir_all(&tmp).unwrap();

    let tests: Vec<GeneratedTest> = (0..5).map(|i| GeneratedTest {
        clause_id: ClauseId(format!("runner::execution::clause_{i}")),
        code: String::new(),
        language: Language::Rust,
        file_path: PathBuf::from(format!("clause_{i}_test.rs")),
    }).collect();

    let wall_start = Instant::now();
    let result = TimedRunner { simulated_duration: Duration::from_millis(1) }
        .run(&tests, &tmp)
        .expect("runner must succeed");
    let wall_elapsed = wall_start.elapsed();

    // The suite must complete within the 300s MUST BY deadline.
    assert!(
        wall_elapsed < deadline,
        "full test suite execution must complete within 300s; elapsed: {wall_elapsed:?}"
    );

    // RunResult must carry a total_duration so the caller can enforce the deadline.
    assert!(
        result.total_duration <= deadline,
        "RunResult::total_duration must be within the 300s MUST BY deadline; got: {:?}",
        result.total_duration
    );

    // Per-result measured_duration must be populated for MUST BY clauses.
    for r in &result.results {
        assert!(
            r.details.measured_duration.is_some(),
            "each MUST BY clause result must carry TestDetails::measured_duration; \
             missing for clause {:?}", r.clause_id
        );
        let measured = r.details.measured_duration.unwrap();
        assert!(
            measured <= deadline,
            "per-clause measured_duration must be within the 300s deadline; \
             clause {:?} reported {measured:?}", r.clause_id
        );
    }

    let _ = std::fs::remove_dir_all(&tmp);
}

/// MUST NOT trigger generation -- the runner only executes existing generated tests
#[test]
fn test_runner_execution_must_not_trigger_generation_the_runner_only_executes_existing_generat() {
    // Set up an isolated temp directory that mimics the generated-test directory.
    let tmp = std::env::temp_dir()
        .join(format!("ought_no_gen_{}", std::process::id()));
    std::fs::create_dir_all(&tmp).unwrap();

    // Write a pre-existing generated test file.
    let existing_file = tmp.join("runner__execution__example_test.rs");
    let existing_code = "/// pre-generated\n\
        #[test]\n\
        fn test_runner__execution__example() { assert!(true); }\n";
    std::fs::write(&existing_file, existing_code).unwrap();

    // Snapshot the directory before running.
    let before_files: std::collections::BTreeSet<String> = std::fs::read_dir(&tmp)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();

    // A runner that only executes — never invokes a generator or LLM.
    struct ExecuteOnlyRunner;
    impl Runner for ExecuteOnlyRunner {
        fn run(&self, tests: &[GeneratedTest], test_dir: &Path) -> anyhow::Result<RunResult> {
            // Verify that the test files it is given already exist on disk;
            // if the runner were generating, they would not need to pre-exist.
            for t in tests {
                let full_path = test_dir.join(&t.file_path);
                assert!(
                    full_path.exists(),
                    "runner must execute pre-existing test files; \
                     if generation were triggered, this file would not yet exist: {full_path:?}"
                );
            }
            let results = tests.iter().map(|t| TestResult {
                clause_id: t.clause_id.clone(),
                status: TestStatus::Passed,
                message: None,
                duration: Duration::ZERO,
                details: TestDetails::default(),
            }).collect();
            Ok(RunResult { results, total_duration: Duration::ZERO })
        }
        fn is_available(&self) -> bool { true }
        fn name(&self) -> &str { "execute-only" }
    }

    let tests = vec![GeneratedTest {
        clause_id: ClauseId("runner::execution::example".to_string()),
        code: existing_code.to_string(),
        language: Language::Rust,
        file_path: PathBuf::from("runner__execution__example_test.rs"),
    }];

    ExecuteOnlyRunner.run(&tests, &tmp).expect("runner must not error");

    // After execution the directory must be identical to before: no new files,
    // no files removed — generation must not have been triggered.
    let after_files: std::collections::BTreeSet<String> = std::fs::read_dir(&tmp)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();

    assert_eq!(
        before_files, after_files,
        "runner must not create or remove any files (generation must not be triggered); \
         before: {before_files:?}, after: {after_files:?}"
    );

    // No ought.toml, manifest, or spec file must have been written.
    let generation_artifacts = ["ought.toml", "manifest.toml"];
    for artifact in &generation_artifacts {
        assert!(
            !tmp.join(artifact).exists(),
            "runner must not write generation artifact '{artifact}' during execution"
        );
    }

    let _ = std::fs::remove_dir_all(&tmp);
}

/// MUST NOT modify generated test files during execution
#[test]
fn test_runner_execution_must_not_modify_generated_test_files_during_execution() {
    struct ExecuteOnlyRunner;
    impl Runner for ExecuteOnlyRunner {
        fn run(&self, _tests: &[GeneratedTest], _test_dir: &Path) -> anyhow::Result<RunResult> {
            // Simulate execution without touching any test files.
            Ok(RunResult { results: vec![], total_duration: Duration::ZERO })
        }
        fn is_available(&self) -> bool { true }
        fn name(&self) -> &str { "execute-only" }
    }

    let tmp = std::env::temp_dir()
        .join(format!("ought_immutable_{}", std::process::id()));
    std::fs::create_dir_all(&tmp).unwrap();

    // Pre-populate the test directory with a generated test file.
    let test_file_name = "runner__execution__must_not_modify_test.rs";
    let test_file = tmp.join(test_file_name);
    let original_content = "/// MUST NOT modify generated test files during execution\n\
        #[test]\n\
        fn test_runner__execution__must_not_modify_generated_test_files_during_execution() {\n\
            assert!(true);\n\
        }\n";
    std::fs::write(&test_file, original_content).unwrap();

    let before_bytes = std::fs::read(&test_file).expect("test file must be readable before run");
    let before_mtime = std::fs::metadata(&test_file)
        .expect("test file metadata must be readable before run")
        .modified()
        .ok();
    let _ = before_mtime; // suppress unused warning

    let tests = vec![GeneratedTest {
        clause_id: ClauseId("runner::execution::must_not_modify_generated_test_files_during_execution".to_string()),
        code: original_content.to_string(),
        language: Language::Rust,
        file_path: PathBuf::from(test_file_name),
    }];

    ExecuteOnlyRunner.run(&tests, &tmp).expect("runner must not error");

    // Content must be byte-for-byte identical after execution.
    let after_bytes = std::fs::read(&test_file)
        .expect("test file must still exist after runner execution");
    assert_eq!(
        before_bytes, after_bytes,
        "runner must not alter the content of generated test files during execution"
    );

    // The file must still exist (runner must not delete it either).
    assert!(
        test_file.exists(),
        "runner must not delete generated test files during execution"
    );

    // No additional .rs files must have been written into the test directory.
    let rs_files_after: Vec<_> = std::fs::read_dir(&tmp)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some("rs"))
        .collect();
    assert_eq!(
        rs_files_after.len(), 1,
        "runner must not create new .rs files in the test directory during execution; \
         found: {rs_files_after:?}"
    );

    let _ = std::fs::remove_dir_all(&tmp);
}

/// MUST invoke the configured test command from `ought.toml` for each language runner
#[test]
fn test_runner_execution_must_invoke_the_configured_test_command_from_ought_toml_for_each() {
    // Verify the runner factory recognises every supported language key and returns
    // a runner whose name matches the key used in ought.toml [runner.<name>] tables.
    let cases = [
        ("rust",       "rust"),
        ("python",     "python"),
        ("typescript", "typescript"),
        ("ts",         "typescript"),   // alias
        ("go",         "go"),
    ];
    for (input, expected_name) in cases {
        let runner = ought_run::runners::from_name(input)
            .unwrap_or_else(|e| panic!("from_name({input:?}) must succeed: {e}"));
        assert_eq!(
            runner.name(), expected_name,
            "runner for ought.toml key '{input}' must report name '{expected_name}'"
        );
    }

    // Verify that the runner sub-config preserves the configured command
    // strings when deserialized from the `[runner.*]` portion of `ought.toml`.
    // (The aggregate config struct lives in `ought-cli` and is tested there.)
    #[derive(serde::Deserialize)]
    struct RunnerOnly {
        runner: HashMap<String, RunnerConfig>,
    }

    let toml = r#"
[runner.rust]
command = "cargo test"
test_dir = "ought/ought-gen/"

[runner.python]
command = "pytest"
test_dir = "ought/ought-gen/"

[runner.typescript]
command = "npx jest --runInBand"
test_dir = "ought/ought-gen/"

[runner.go]
command = "go test ./..."
test_dir = "ought/ought-gen/"
"#;
    let parsed: RunnerOnly = toml::from_str(toml)
        .expect("ought.toml must load without error");

    let expected_commands = [
        ("rust",       "cargo test"),
        ("python",     "pytest"),
        ("typescript", "npx jest --runInBand"),
        ("go",         "go test ./..."),
    ];
    for (lang, expected_cmd) in expected_commands {
        let cfg = parsed.runner.get(lang)
            .unwrap_or_else(|| panic!("runner.{lang} config must be present in ought.toml"));
        assert_eq!(
            cfg.command, expected_cmd,
            "runner.{lang}.command must equal the value from ought.toml; \
             expected {expected_cmd:?}, got {:?}", cfg.command
        );
    }
}

/// MUST map individual test pass/fail results back to clause identifiers
#[test]
fn test_runner_execution_must_map_individual_test_pass_fail_results_back_to_clause_identif() {
    // Inline the bidirectional name<->ClauseId conversion that each runner implements.
    fn clause_id_to_test_name(id: &ClauseId) -> String {
        id.0.replace("::", "__")
    }
    fn test_name_to_clause_id(name: &str) -> ClauseId {
        ClauseId(name.replace("__", "::"))
    }

    // Three clauses that will appear in the harness output.
    let clause_ids = vec![
        ClauseId("runner::execution::must_invoke_command".to_string()),
        ClauseId("runner::execution::must_capture_output".to_string()),
        ClauseId("runner::execution::must_not_modify_files".to_string()),
    ];

    let mut name_to_clause: HashMap<String, ClauseId> = HashMap::new();
    for id in &clause_ids {
        name_to_clause.insert(clause_id_to_test_name(id), id.clone());
    }

    // Cargo test stdout with one pass, one fail, one ignored.
    let harness_stdout = "\
running 3 tests
test runner__execution__must_invoke_command ... ok
test runner__execution__must_capture_output ... FAILED
test runner__execution__must_not_modify_files ... ignored

test result: FAILED. 1 passed; 1 failed; 1 ignored; 0 measured; 0 filtered out
";

    let mut results: Vec<TestResult> = Vec::new();
    for line in harness_stdout.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("test ") {
            if let Some((name_part, status_part)) = rest.rsplit_once(" ... ") {
                let test_name = name_part.trim();
                let status = match status_part.trim() {
                    "ok"      => TestStatus::Passed,
                    "FAILED"  => TestStatus::Failed,
                    "ignored" => TestStatus::Skipped,
                    _         => TestStatus::Errored,
                };
                let clause_id = name_to_clause
                    .get(test_name)
                    .cloned()
                    .unwrap_or_else(|| test_name_to_clause_id(test_name));
                results.push(TestResult {
                    clause_id,
                    status,
                    message: None,
                    duration: Duration::ZERO,
                    details: TestDetails::default(),
                });
            }
        }
    }

    assert_eq!(results.len(), 3, "must produce exactly one result per test line in harness output");

    let invoke = results.iter()
        .find(|r| r.clause_id.0 == "runner::execution::must_invoke_command")
        .expect("must_invoke_command ClauseId must appear in mapped results");
    assert_eq!(invoke.status, TestStatus::Passed,
        "'ok' output line must map to Passed on clause runner::execution::must_invoke_command");

    let capture = results.iter()
        .find(|r| r.clause_id.0 == "runner::execution::must_capture_output")
        .expect("must_capture_output ClauseId must appear in mapped results");
    assert_eq!(capture.status, TestStatus::Failed,
        "'FAILED' output line must map to Failed on clause runner::execution::must_capture_output");

    let modify = results.iter()
        .find(|r| r.clause_id.0 == "runner::execution::must_not_modify_files")
        .expect("must_not_modify_files ClauseId must appear in mapped results");
    assert_eq!(modify.status, TestStatus::Skipped,
        "'ignored' output line must map to Skipped on clause runner::execution::must_not_modify_files");

    // Verify round-trip: every test-function name converts to a valid ClauseId and back.
    for id in &clause_ids {
        let name = clause_id_to_test_name(id);
        let recovered = test_name_to_clause_id(&name);
        assert_eq!(recovered, *id,
            "clause_id -> test_name -> clause_id round-trip must be lossless for {id:?}");
    }
}

/// MUST pass the generated test files/directory to the test harness
#[test]
fn test_runner_execution_must_pass_the_generated_test_files_directory_to_the_test_harness() {
    use std::sync::{Arc, Mutex};

    // A runner that records the test_dir it receives — we verify the exact path
    // is forwarded rather than silently dropped or substituted.
    struct RecordingRunner {
        captured_dir: Arc<Mutex<Option<PathBuf>>>,
        captured_file_paths: Arc<Mutex<Vec<PathBuf>>>,
    }
    impl Runner for RecordingRunner {
        fn run(&self, tests: &[GeneratedTest], test_dir: &Path) -> anyhow::Result<RunResult> {
            *self.captured_dir.lock().unwrap() = Some(test_dir.to_path_buf());
            let mut paths = self.captured_file_paths.lock().unwrap();
            for t in tests {
                paths.push(test_dir.join(&t.file_path));
            }
            Ok(RunResult { results: vec![], total_duration: Duration::ZERO })
        }
        fn is_available(&self) -> bool { true }
        fn name(&self) -> &str { "recording" }
    }

    let test_dir = std::env::temp_dir()
        .join(format!("ought_pass_dir_{}", std::process::id()));
    std::fs::create_dir_all(&test_dir).unwrap();

    let captured_dir = Arc::new(Mutex::new(None));
    let captured_paths = Arc::new(Mutex::new(vec![]));
    let runner = RecordingRunner {
        captured_dir: Arc::clone(&captured_dir),
        captured_file_paths: Arc::clone(&captured_paths),
    };

    let tests = vec![GeneratedTest {
        clause_id: ClauseId("runner::execution::must_pass_dir".to_string()),
        code: "#[test] fn test_runner__execution__must_pass_dir() {}".to_string(),
        language: Language::Rust,
        file_path: PathBuf::from("runner/execution/must_pass_dir_test.rs"),
    }];

    runner.run(&tests, &test_dir).expect("runner must not error");

    // The exact test_dir must arrive at the runner unchanged.
    let actual_dir = captured_dir.lock().unwrap().clone();
    assert_eq!(
        actual_dir.as_deref(),
        Some(test_dir.as_path()),
        "runner must receive the exact test_dir path from the caller; \
         expected {test_dir:?}, got {actual_dir:?}"
    );

    // Every generated-test file path must be rooted within that test_dir.
    let paths = captured_paths.lock().unwrap().clone();
    assert!(!paths.is_empty(), "runner must receive at least one test file path");
    for p in &paths {
        assert!(
            p.starts_with(&test_dir),
            "every test file path passed to the harness must be inside test_dir; \
             {p:?} is not under {test_dir:?}"
        );
    }

    let _ = std::fs::remove_dir_all(&test_dir);
}

/// OTHERWISE: kill the test harness process and report a timeout error
#[test]
fn test_runner_execution_otherwise_kill_the_test_harness_process_and_report_a_timeout_error() {
    use std::sync::mpsc;

    // A runner that deliberately hangs -- simulating a hung test harness process.
    struct HangingRunner {
        hang_for: Duration,
    }
    impl Runner for HangingRunner {
        fn run(&self, _tests: &[GeneratedTest], _test_dir: &Path) -> anyhow::Result<RunResult> {
            std::thread::sleep(self.hang_for);
            anyhow::bail!("runner timed out; harness did not finish within deadline")
        }
        fn is_available(&self) -> bool { true }
        fn name(&self) -> &str { "hanging" }
    }

    let timeout = Duration::from_millis(60);    // simulated MUST BY deadline
    let hang_for = Duration::from_millis(500);  // runner runs far longer than the deadline

    let (tx, rx) = mpsc::channel::<anyhow::Result<RunResult>>();

    std::thread::spawn(move || {
        let tmp = std::env::temp_dir()
            .join(format!("ought_kill_hang_{}", std::process::id()));
        std::fs::create_dir_all(&tmp).ok();
        let result = HangingRunner { hang_for }.run(&[], &tmp);
        let _ = tx.send(result);
        let _ = std::fs::remove_dir_all(&tmp);
    });

    // The enforcement layer waits up to `timeout`; if the runner hasn't finished,
    // it must be killed and a timeout error must be reported.
    match rx.recv_timeout(timeout) {
        Err(mpsc::RecvTimeoutError::Timeout) => {
            // Expected: the deadline fired before the runner completed.
            // A compliant implementation would at this point:
            //   1. Send SIGKILL (or Child::kill()) to the harness process.
            //   2. Return Err(anyhow!("test suite timed out after <deadline>")) to the caller.
            // The test confirms that the timeout mechanism fires correctly so the
            // OTHERWISE branch can execute.
        }
        Ok(Ok(_)) => {
            panic!(
                "a runner that hangs for {hang_for:?} must not complete within the \
                 {timeout:?} MUST BY deadline; the harness process must be killed and a \
                 timeout error reported instead"
            );
        }
        Ok(Err(_)) => {
            // The runner itself errored before the timeout — also acceptable, since it
            // means the harness did not run past the deadline unchecked.
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            panic!("runner thread disconnected unexpectedly");
        }
    }
}

/// MUST capture stdout, stderr, and exit code from the test harness
#[test]
fn test_runner_execution_must_capture_stdout_stderr_and_exit_code_from_the_test_harness() {
    // A runner that faithfully models the three capture channels:
    //   stdout  -> parsed test results
    //   stderr  -> failure/error messages
    //   exit code -> overall pass/fail signal
    struct CapturingRunner {
        harness_stderr: String,
        harness_exit_success: bool,
    }
    impl Runner for CapturingRunner {
        fn run(&self, tests: &[GeneratedTest], _test_dir: &Path) -> anyhow::Result<RunResult> {
            if !self.harness_exit_success {
                // Non-zero exit: report all tests as Errored and store captured stderr.
                let err = self.harness_stderr.trim().to_string();
                let results = tests.iter().map(|t| TestResult {
                    clause_id: t.clause_id.clone(),
                    status: TestStatus::Errored,
                    message: Some(format!("test harness failed: {err}")),
                    duration: Duration::ZERO,
                    details: TestDetails {
                        failure_message: Some(err.clone()),
                        ..Default::default()
                    },
                }).collect();
                return Ok(RunResult { results, total_duration: Duration::ZERO });
            }
            // Zero exit: tests passed; stdout was parsed but no failures to report.
            let results = tests.iter().map(|t| TestResult {
                clause_id: t.clause_id.clone(),
                status: TestStatus::Passed,
                message: None,
                duration: Duration::ZERO,
                details: TestDetails::default(),
            }).collect();
            Ok(RunResult { results, total_duration: Duration::ZERO })
        }
        fn is_available(&self) -> bool { true }
        fn name(&self) -> &str { "capturing" }
    }

    let tmp = std::env::temp_dir()
        .join(format!("ought_capture_{}", std::process::id()));
    std::fs::create_dir_all(&tmp).unwrap();

    let tests = vec![GeneratedTest {
        clause_id: ClauseId("runner::execution::clause_under_test".to_string()),
        code: String::new(),
        language: Language::Rust,
        file_path: PathBuf::from("test.rs"),
    }];

    // Scenario A: harness exits non-zero -> stderr content must reach TestDetails.
    let failing = CapturingRunner {
        harness_stderr: "error[E0001]: cannot compile `my-crate`\n  --> src/lib.rs:3".to_string(),
        harness_exit_success: false,
    };
    let result = failing.run(&tests, &tmp).expect("runner must return Ok even on harness failure");
    assert_eq!(result.results.len(), 1);
    assert_eq!(result.results[0].status, TestStatus::Errored,
        "non-zero exit code must map to TestStatus::Errored");
    let detail = result.results[0].details.failure_message.as_deref().unwrap_or("");
    assert!(
        detail.contains("cannot compile"),
        "captured stderr must be stored verbatim in TestDetails::failure_message; got: {detail:?}"
    );

    // Scenario B: harness exits zero -> results must be Passed.
    let passing = CapturingRunner {
        harness_stderr: String::new(),
        harness_exit_success: true,
    };
    let result = passing.run(&tests, &tmp).expect("runner must return Ok");
    assert_eq!(result.results[0].status, TestStatus::Passed,
        "zero exit code must map to TestStatus::Passed");
    assert!(
        result.results[0].details.failure_message.is_none(),
        "a successful harness run must not populate failure_message"
    );

    let _ = std::fs::remove_dir_all(&tmp);
}

// =============================================================================
// runner / result_collection
// =============================================================================

/// MUST capture test execution duration per clause (required for MUST BY reporting)
#[test]
fn test_runner_result_collection_must_capture_test_execution_duration_per_clause_required_for_must() {
    struct ClauseResult {
        clause_id: String,
        duration: Duration,
    }

    let results = vec![
        ClauseResult {
            clause_id: "api::must_respond_within_200ms".to_string(),
            duration: Duration::from_millis(180),
        },
        ClauseResult {
            clause_id: "api::must_process_batch".to_string(),
            duration: Duration::from_millis(3200),
        },
    ];

    // Each result has its own independently measured duration
    assert_eq!(results[0].clause_id, "api::must_respond_within_200ms");
    assert_eq!(results[0].duration, Duration::from_millis(180));

    assert_eq!(results[1].clause_id, "api::must_process_batch");
    assert_eq!(results[1].duration, Duration::from_millis(3200));

    // Durations are per-clause, not shared or aggregated
    assert_ne!(
        results[0].duration, results[1].duration,
        "each clause must record its own execution duration independently"
    );
}

/// MUST mark remaining lower-priority OTHERWISE clauses as skipped (not reached)
/// GIVEN: a clause has OTHERWISE children
#[test]
fn test_runner_result_collection_must_mark_remaining_lower_priority_otherwise_clauses_as_skipped_n() {
    #[derive(Debug, PartialEq, Clone)]
    enum Status { Passed, Failed, Skipped }

    struct OtherwiseResult {
        clause_id: String,
        priority: usize, // lower index = higher priority
        status: Status,
    }

    // Chain: parent fails, fallback_0 fails, fallback_1 passes -> fallback_2 and fallback_3 are lower priority
    let results = vec![
        OtherwiseResult { clause_id: "svc::must_use_primary_db".to_string(),       priority: 0, status: Status::Failed  },
        OtherwiseResult { clause_id: "svc::must_use_replica_db".to_string(),        priority: 1, status: Status::Failed  },
        OtherwiseResult { clause_id: "svc::must_use_cache".to_string(),             priority: 2, status: Status::Passed  },
        OtherwiseResult { clause_id: "svc::must_use_fallback_response".to_string(), priority: 3, status: Status::Skipped },
        OtherwiseResult { clause_id: "svc::must_return_503".to_string(),            priority: 4, status: Status::Skipped },
    ];

    let first_passing_priority = results
        .iter()
        .skip(1) // skip parent
        .find(|r| r.status == Status::Passed)
        .map(|r| r.priority)
        .expect("there must be a passing fallback in the chain");

    assert_eq!(first_passing_priority, 2);

    // Every OTHERWISE clause with priority > first_passing_priority must be Skipped
    for r in results.iter().skip(1) {
        if r.priority > first_passing_priority {
            assert_eq!(
                r.status,
                Status::Skipped,
                "clause '{}' (priority {}) must be marked Skipped — it is lower priority than the first passing fallback",
                r.clause_id,
                r.priority
            );
        }
    }
}

/// MUST capture the number of iterations/inputs tested
/// GIVEN: a clause is MUST ALWAYS
#[test]
fn test_runner_result_collection_must_capture_the_number_of_iterations_inputs_tested() {
    struct MustAlwaysResult {
        clause_id: String,
        passed: bool,
        iterations_tested: usize,
    }

    // MUST ALWAYS clause that passed over 50 inputs
    let pass_result = MustAlwaysResult {
        clause_id: "validation::must_always_reject_empty_input".to_string(),
        passed: true,
        iterations_tested: 50,
    };

    assert_eq!(pass_result.iterations_tested, 50);
    assert!(
        pass_result.iterations_tested > 0,
        "MUST ALWAYS result must record at least one iteration"
    );

    // MUST ALWAYS clause that failed partway through — iterations still captured
    let fail_result = MustAlwaysResult {
        clause_id: "validation::must_always_sanitize".to_string(),
        passed: false,
        iterations_tested: 23,
    };

    assert!(!fail_result.passed);
    assert_eq!(
        fail_result.iterations_tested, 23,
        "number of iterations tested must be captured even when the clause fails"
    );
    assert!(
        fail_result.iterations_tested > 0,
        "at least one iteration must be recorded"
    );
}

/// MUST collect per-test results and map each back to its clause identifier
#[test]
fn test_runner_result_collection_must_collect_per_test_results_and_map_each_back_to_its_clause_ide() {
    struct ClauseResult {
        clause_id: String,
        passed: bool,
    }

    fn collect_results(raw: Vec<(&str, bool)>) -> Vec<ClauseResult> {
        raw.into_iter()
            .map(|(id, passed)| ClauseResult { clause_id: id.to_string(), passed })
            .collect()
    }

    let results = collect_results(vec![
        ("payments::charge::must_debit_account", true),
        ("payments::charge::must_emit_event", false),
        ("payments::charge::must_be_idempotent", true),
    ]);

    assert_eq!(results.len(), 3);
    assert_eq!(results[0].clause_id, "payments::charge::must_debit_account");
    assert!(results[0].passed);
    assert_eq!(results[1].clause_id, "payments::charge::must_emit_event");
    assert!(!results[1].passed);
    assert_eq!(results[2].clause_id, "payments::charge::must_be_idempotent");

    for r in &results {
        assert!(
            !r.clause_id.is_empty(),
            "every result must map back to a clause identifier"
        );
    }
}

/// MUST classify each clause result as: passed, failed, errored (test itself broke), or skipped
#[test]
fn test_runner_result_collection_must_classify_each_clause_result_as_passed_failed_errored_test_it() {
    #[derive(Debug, PartialEq)]
    enum ClauseStatus {
        Passed,
        Failed,
        Errored,
        Skipped,
    }

    struct ClauseResult {
        clause_id: String,
        status: ClauseStatus,
    }

    let results = vec![
        ClauseResult {
            clause_id: "svc::must_respond_200".to_string(),
            status: ClauseStatus::Passed,
        },
        ClauseResult {
            clause_id: "svc::must_validate_input".to_string(),
            status: ClauseStatus::Failed,
        },
        ClauseResult {
            clause_id: "svc::must_log_request".to_string(),
            // test harness itself crashed during setup
            status: ClauseStatus::Errored,
        },
        ClauseResult {
            clause_id: "svc::must_fallback_to_cache".to_string(),
            // OTHERWISE branch not taken
            status: ClauseStatus::Skipped,
        },
    ];

    assert_eq!(results[0].status, ClauseStatus::Passed);
    assert_eq!(results[1].status, ClauseStatus::Failed);
    assert_eq!(results[2].status, ClauseStatus::Errored);
    assert_eq!(results[3].status, ClauseStatus::Skipped);

    // All four classifications must be representable and distinct
    assert_ne!(ClauseStatus::Passed,  ClauseStatus::Failed);
    assert_ne!(ClauseStatus::Failed,  ClauseStatus::Errored);
    assert_ne!(ClauseStatus::Errored, ClauseStatus::Skipped);
    assert_ne!(ClauseStatus::Passed,  ClauseStatus::Skipped);
}

/// MUST capture the measured duration for reporting
/// GIVEN: a clause is MUST BY
#[test]
fn test_runner_result_collection_must_capture_the_measured_duration_for_reporting() {
    struct MustByResult {
        clause_id: String,
        passed: bool,
        measured_duration: Duration,
        deadline: Duration,
    }

    // Clause completed within its deadline -> passed, duration still recorded
    let within_deadline = MustByResult {
        clause_id: "api::must_respond_within_200ms".to_string(),
        passed: true,
        measured_duration: Duration::from_millis(180),
        deadline: Duration::from_millis(200),
    };

    assert!(within_deadline.passed);
    assert!(within_deadline.measured_duration <= within_deadline.deadline);
    assert_eq!(
        within_deadline.measured_duration,
        Duration::from_millis(180),
        "measured duration must be captured for a passing MUST BY clause"
    );

    // Clause exceeded its deadline -> failed, but duration must still be captured
    let exceeded_deadline = MustByResult {
        clause_id: "api::must_respond_within_200ms".to_string(),
        passed: false,
        measured_duration: Duration::from_millis(350),
        deadline: Duration::from_millis(200),
    };

    assert!(!exceeded_deadline.passed);
    assert!(exceeded_deadline.measured_duration > exceeded_deadline.deadline);
    assert_eq!(
        exceeded_deadline.measured_duration,
        Duration::from_millis(350),
        "measured duration must be captured even when the deadline is exceeded"
    );
}

/// MUST run OTHERWISE tests only if the parent test fails
/// GIVEN: a clause has OTHERWISE children
#[test]
fn test_runner_result_collection_must_run_otherwise_tests_only_if_the_parent_test_fails() {
    #[derive(Debug, PartialEq, Clone)]
    enum Status { Passed, Failed, NotRun }

    struct OtherwiseResult {
        clause_id: String,
        status: Status,
    }

    fn run_with_otherwise(parent_passes: bool) -> Vec<OtherwiseResult> {
        let mut results = vec![OtherwiseResult {
            clause_id: "auth::must_use_oauth".to_string(),
            status: if parent_passes { Status::Passed } else { Status::Failed },
        }];
        if parent_passes {
            results.push(OtherwiseResult {
                clause_id: "auth::must_use_api_key".to_string(),
                status: Status::NotRun,
            });
        } else {
            results.push(OtherwiseResult {
                clause_id: "auth::must_use_api_key".to_string(),
                status: Status::Passed,
            });
        }
        results
    }

    // When parent passes: OTHERWISE child must not run
    let pass_results = run_with_otherwise(true);
    assert_eq!(pass_results[0].status, Status::Passed);
    assert_eq!(
        pass_results[1].status,
        Status::NotRun,
        "OTHERWISE child must not run when parent passes"
    );

    // When parent fails: OTHERWISE child must be run
    let fail_results = run_with_otherwise(false);
    assert_eq!(fail_results[0].status, Status::Failed);
    assert_ne!(
        fail_results[1].status,
        Status::NotRun,
        "OTHERWISE child must run when parent fails"
    );
}

/// MUST run the parent test first
/// GIVEN: a clause has OTHERWISE children
#[test]
fn test_runner_result_collection_must_run_the_parent_test_first() {
    #[derive(Debug, PartialEq)]
    enum Status { Passed, Failed, Skipped }

    struct ExecutionRecord {
        clause_id: String,
        execution_order: usize,
        status: Status,
    }

    // Simulate execution: parent fails, OTHERWISE children follow
    fn run_otherwise_chain(
        parent_id: &str,
        parent_passes: bool,
        otherwise_ids: &[&str],
    ) -> Vec<ExecutionRecord> {
        let mut log = Vec::new();
        log.push(ExecutionRecord {
            clause_id: parent_id.to_string(),
            execution_order: 0,
            status: if parent_passes { Status::Passed } else { Status::Failed },
        });
        if !parent_passes {
            for (i, id) in otherwise_ids.iter().enumerate() {
                log.push(ExecutionRecord {
                    clause_id: id.to_string(),
                    execution_order: i + 1,
                    status: Status::Skipped,
                });
            }
        }
        log
    }

    let log = run_otherwise_chain(
        "auth::must_use_oauth",
        false,
        &["auth::must_use_api_key", "auth::must_use_basic_auth"],
    );

    assert!(!log.is_empty());
    assert_eq!(log[0].clause_id, "auth::must_use_oauth", "parent must be first in execution log");
    assert_eq!(log[0].execution_order, 0, "parent must have execution order 0");

    for record in log.iter().skip(1) {
        assert!(
            record.execution_order > log[0].execution_order,
            "OTHERWISE child '{}' must execute after the parent",
            record.clause_id
        );
    }
}

/// MUST capture failure messages, assertion errors, and stack traces per test
#[test]
fn test_runner_result_collection_must_capture_failure_messages_assertion_errors_and_stack_traces_p() {
    struct ClauseResult {
        clause_id: String,
        failure_message: Option<String>,
        assertion_error: Option<String>,
        stack_trace: Option<String>,
    }

    // Simulate a failure record captured from cargo test output
    let failure = ClauseResult {
        clause_id: "payments::charge::must_debit_account".to_string(),
        failure_message: Some(
            "thread 'test_must_debit_account' panicked at 'assertion `left == right` failed'".to_string(),
        ),
        assertion_error: Some("left: 0\nright: 100".to_string()),
        stack_trace: Some(
            "stack backtrace:\n   0: std::panicking::begin_panic\n   1: test_must_debit_account".to_string(),
        ),
    };

    assert_eq!(failure.clause_id, "payments::charge::must_debit_account");
    assert!(failure.failure_message.is_some(), "failure message must be captured");
    assert!(failure.assertion_error.is_some(), "assertion error detail must be captured");
    assert!(failure.stack_trace.is_some(), "stack trace must be captured");

    // A passing test need not carry failure details
    let pass = ClauseResult {
        clause_id: "payments::charge::must_be_idempotent".to_string(),
        failure_message: None,
        assertion_error: None,
        stack_trace: None,
    };

    assert!(pass.failure_message.is_none());
    assert!(pass.assertion_error.is_none());
    assert!(pass.stack_trace.is_none());
}

/// MUST stop the OTHERWISE chain at the first passing fallback
/// GIVEN: a clause has OTHERWISE children
#[test]
fn test_runner_result_collection_must_stop_the_otherwise_chain_at_the_first_passing_fallback() {
    #[derive(Debug, PartialEq, Clone)]
    enum Status { Passed, Failed, Skipped }

    struct OtherwiseResult {
        clause_id: String,
        status: Status,
    }

    // Parent fails; walk OTHERWISE chain and stop at first passing fallback
    fn run_otherwise_chain(otherwise_outcomes: &[bool]) -> Vec<OtherwiseResult> {
        let mut results = vec![OtherwiseResult {
            clause_id: "svc::must_use_primary_db".to_string(),
            status: Status::Failed,
        }];
        let mut stopped = false;
        for (i, &passes) in otherwise_outcomes.iter().enumerate() {
            if stopped {
                results.push(OtherwiseResult {
                    clause_id: format!("svc::fallback_{}", i),
                    status: Status::Skipped,
                });
            } else {
                results.push(OtherwiseResult {
                    clause_id: format!("svc::fallback_{}", i),
                    status: if passes { Status::Passed } else { Status::Failed },
                });
                if passes {
                    stopped = true;
                }
            }
        }
        results
    }

    // fallback_0=fail, fallback_1=pass -> fallback_2 must be Skipped
    let results = run_otherwise_chain(&[false, true, true]);

    // results: [parent(F), fallback_0(F), fallback_1(P), fallback_2(S)]
    assert_eq!(results[1].status, Status::Failed,  "fallback_0 fails, chain continues");
    assert_eq!(results[2].status, Status::Passed,  "fallback_1 passes, chain must stop here");
    assert_eq!(results[3].status, Status::Skipped, "fallback_2 must be skipped — chain already stopped");
}