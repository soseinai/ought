/// MUST ALWAYS write diagnostic messages to stderr, never stdout (stdout is reserved for
/// structured output and results). Invariant — verified across a range of invocations.
#[test]
fn test_cli__global_flags__must_always_write_diagnostic_messages_to_stderr_never_stdout_stdout_is_r(
) {
    let bin = option_env!("CARGO_BIN_EXE_ought")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("ought"));

    // Set up a valid project so most commands have something real to work with.
    let proj = std::env::temp_dir()
        .join(format!("ought_stderr_inv_{}", std::process::id()));
    std::fs::create_dir_all(&proj).unwrap();
    let init = std::process::Command::new(&bin)
        .arg("init")
        .current_dir(&proj)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .expect("ought init (stderr probe setup)");
    assert!(
        init.status.success(),
        "init must succeed for probe setup; stderr: {}",
        String::from_utf8_lossy(&init.stderr)
    );

    // Diagnostic keyword patterns that must never appear on stdout.
    // These are prefixes written by the application itself (not embedded in JSON payloads).
    let diagnostic_patterns: &[&str] = &[
        "error: ",
        "Error: ",
        "warning: ",
        "Warning: ",
        "fatal: ",
    ];

    // Probe a range of non-JSON invocations; for each, stdout must not contain raw diagnostics.
    let probes: &[(&[&str], &str)] = &[
        (&["check"], "check"),
        (&["--verbose", "check"], "--verbose check"),
        (&["--quiet", "check"], "--quiet check"),
        (&["run"], "run"),
        (&["--quiet", "run"], "--quiet run"),
    ];

    for (args, label) in probes {
        let out = std::process::Command::new(&bin)
            .args(*args)
            .current_dir(&proj)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .output()
            .unwrap_or_else(|e| panic!("failed to run ought {label}: {e}"));

        let stdout = String::from_utf8_lossy(&out.stdout);
        for pat in diagnostic_patterns {
            assert!(
                !stdout.contains(pat),
                "ought {label}: diagnostic pattern {pat:?} must not appear on stdout; \
                 stdout={stdout:?}"
            );
        }
    }

    // A known-error path (nonexistent --config) must write its error to stderr, not stdout.
    let bad = std::process::Command::new(&bin)
        .args(["--config", "/nonexistent/path/ought.toml", "check"])
        .current_dir(&proj)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .expect("ought --config /nonexistent check");

    assert_ne!(
        bad.status.code(),
        Some(0),
        "ought with a nonexistent --config must not succeed"
    );
    let bad_stdout = String::from_utf8_lossy(&bad.stdout);
    for pat in diagnostic_patterns {
        assert!(
            !bad_stdout.contains(pat),
            "error from bad --config must appear on stderr, not stdout; \
             stdout={bad_stdout:?}"
        );
    }
    let bad_stderr = String::from_utf8_lossy(&bad.stderr);
    assert!(
        !bad_stderr.is_empty(),
        "error from bad --config must produce output on stderr"
    );

    let _ = std::fs::remove_dir_all(&proj);
}