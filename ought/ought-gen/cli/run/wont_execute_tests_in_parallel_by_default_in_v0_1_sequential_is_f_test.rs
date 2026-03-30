/// WONT execute tests in parallel by default in v0.1 (sequential is fine to start).
/// Absence test: the CLI must not expose a `--parallel` flag.
#[test]
fn test_cli__run__wont_execute_tests_in_parallel_by_default_in_v0_1_sequential_is_f() {
    // Invoke without a project directory; clap argument parsing happens before
    // config is loaded, so an unknown flag is rejected immediately with exit 2.
    let out = std::process::Command::new(ought_bin())
        .arg("run")
        .arg("--parallel")
        .output()
        .expect("failed to invoke ought run --parallel");

    // clap returns exit code 2 for unrecognised flags.
    assert_eq!(
        out.status.code(),
        Some(2),
        "ought run must NOT expose a --parallel flag in v0.1 \
         (parallel test execution is intentionally absent); stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}