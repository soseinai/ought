/// MUST ALWAYS return a valid exit code (0, 1, or 2) — never crash without an exit code.
/// Invariant — verified across a wide fuzz-style range of invocations.
#[test]
fn test_cli__global_flags__must_always_return_a_valid_exit_code_0_1_or_2_never_crash_without_an_exi(
) {
    let bin = option_env!("CARGO_BIN_EXE_ought")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("ought"));

    let proj = std::env::temp_dir()
        .join(format!("ought_exitcode_inv_{}", std::process::id()));
    std::fs::create_dir_all(&proj).unwrap();

    // Pre-initialise so commands that need a project have one.
    let _ = std::process::Command::new(&bin)
        .arg("init")
        .current_dir(&proj)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output();

    // Fuzz corpus: valid commands, usage errors, missing args, bad flag values, edge cases.
    let invocations: &[&[&str]] = &[
        // ── Valid subcommands ────────────────────────────────────────────────
        &["check"],
        &["run"],
        &["diff"],
        &["--json", "run"],
        &["--quiet", "run"],
        &["--verbose", "check"],
        &["--color", "never", "check"],
        &["--color", "always", "check"],
        &["--color", "auto", "check"],
        &["--quiet", "--json", "run"],
        // ── Usage errors: unknown flags / subcommands (must exit 2) ─────────
        &["--nonexistent-flag"],
        &["nonexistent-subcommand"],
        &["--unknown-global", "check"],
        &["run", "--unknown-local"],
        // ── Missing required positional args (must exit 2) ──────────────────
        &["blame"],
        &["bisect"],
        &["inspect"],
        // ── Bad flag values (must exit 2) ────────────────────────────────────
        &["--color", "rainbow", "check"],
        // ── Config edge cases ────────────────────────────────────────────────
        &["--config", "/nonexistent/path/ought.toml", "check"],
        &["--config", "", "check"],
        // ── Re-init (project already exists) ─────────────────────────────────
        &["init"],
    ];

    for args in invocations {
        let out = std::process::Command::new(&bin)
            .args(*args)
            .current_dir(&proj)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .output()
            .unwrap_or_else(|e| panic!("process::Command failed for {args:?}: {e}"));

        let code = out.status.code();
        assert!(
            matches!(code, Some(0) | Some(1) | Some(2)),
            "ought {args:?} must exit 0, 1, or 2 — never crash or signal-terminate; \
             got {code:?}; stderr: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    let _ = std::fs::remove_dir_all(&proj);
}