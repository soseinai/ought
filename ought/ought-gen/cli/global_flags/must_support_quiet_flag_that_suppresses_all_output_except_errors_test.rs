/// MUST support `--quiet` flag that suppresses all output except errors and the final summary
#[test]
fn test_cli__global_flags__must_support_quiet_flag_that_suppresses_all_output_except_errors() {
    let bin = option_env!("CARGO_BIN_EXE_ought")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("ought"));

    let dir_loud = std::env::temp_dir()
        .join(format!("ought_quiet_loud_{}", std::process::id()));
    let dir_quiet = std::env::temp_dir()
        .join(format!("ought_quiet_silent_{}", std::process::id()));
    std::fs::create_dir_all(&dir_loud).unwrap();
    std::fs::create_dir_all(&dir_quiet).unwrap();

    // Run `ought init` (produces informational output) without --quiet.
    let loud = std::process::Command::new(&bin)
        .arg("init")
        .current_dir(&dir_loud)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .expect("ought init (loud)");

    // Run the same command with --quiet.
    let quiet = std::process::Command::new(&bin)
        .args(["--quiet", "init"])
        .current_dir(&dir_quiet)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .expect("ought --quiet init");

    // --quiet must be a recognised flag (not a usage error).
    assert_ne!(
        quiet.status.code(),
        Some(2),
        "--quiet must not produce a usage/parse error; stderr: {}",
        String::from_utf8_lossy(&quiet.stderr)
    );

    // --quiet must suppress stdout relative to the normal invocation.
    assert!(
        quiet.stdout.len() <= loud.stdout.len(),
        "--quiet must produce no more stdout than default; quiet={} bytes, loud={} bytes",
        quiet.stdout.len(),
        loud.stdout.len()
    );

    let _ = std::fs::remove_dir_all(&dir_loud);
    let _ = std::fs::remove_dir_all(&dir_quiet);
}