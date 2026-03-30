/// SHOULD support `--verbose` flag for debug-level output
#[test]
fn test_cli__global_flags__should_support_verbose_flag_for_debug_level_output() {
    let bin = option_env!("CARGO_BIN_EXE_ought")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("ought"));

    // Set up two identical projects so we can compare output side-by-side.
    let dir_normal = std::env::temp_dir()
        .join(format!("ought_verbose_normal_{}", std::process::id()));
    let dir_verbose = std::env::temp_dir()
        .join(format!("ought_verbose_verbose_{}", std::process::id()));
    std::fs::create_dir_all(&dir_normal).unwrap();
    std::fs::create_dir_all(&dir_verbose).unwrap();

    for d in [&dir_normal, &dir_verbose] {
        let init = std::process::Command::new(&bin)
            .arg("init")
            .current_dir(d)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .output()
            .expect("ought init");
        assert!(
            init.status.success(),
            "init must succeed; stderr: {}",
            String::from_utf8_lossy(&init.stderr)
        );
    }

    let normal = std::process::Command::new(&bin)
        .arg("check")
        .current_dir(&dir_normal)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .expect("ought check (normal)");

    let verbose = std::process::Command::new(&bin)
        .args(["--verbose", "check"])
        .current_dir(&dir_verbose)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .expect("ought --verbose check");

    // --verbose must be a recognised flag (not a usage error).
    assert_ne!(
        verbose.status.code(),
        Some(2),
        "--verbose must not produce a usage/parse error; stderr: {}",
        String::from_utf8_lossy(&verbose.stderr)
    );

    // --verbose must produce at least as much total output as the default invocation,
    // since debug-level lines are added on top of the normal output.
    let normal_total = normal.stdout.len() + normal.stderr.len();
    let verbose_total = verbose.stdout.len() + verbose.stderr.len();
    assert!(
        verbose_total >= normal_total,
        "--verbose must produce at least as much output as the default; \
         verbose={verbose_total} bytes, normal={normal_total} bytes"
    );

    let _ = std::fs::remove_dir_all(&dir_normal);
    let _ = std::fs::remove_dir_all(&dir_verbose);
}