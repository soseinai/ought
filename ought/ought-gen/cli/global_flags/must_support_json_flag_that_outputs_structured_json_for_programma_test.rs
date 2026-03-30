/// MUST support `--json` flag that outputs structured JSON for programmatic consumption
#[test]
fn test_cli__global_flags__must_support_json_flag_that_outputs_structured_json_for_programma() {
    let dir = std::env::temp_dir()
        .join(format!("ought_json_flag_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();

    let bin = option_env!("CARGO_BIN_EXE_ought")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("ought"));

    // Initialise a minimal project.
    let init = std::process::Command::new(&bin)
        .arg("init")
        .current_dir(&dir)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .expect("ought init");
    assert!(
        init.status.success(),
        "init must succeed to set up the test project; stderr: {}",
        String::from_utf8_lossy(&init.stderr)
    );

    // Run with --json; the flag must be accepted and any stdout must be valid JSON.
    let out = std::process::Command::new(&bin)
        .args(["--json", "run"])
        .current_dir(&dir)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .expect("ought --json run");

    assert_ne!(
        out.status.code(),
        Some(2),
        "--json must not produce a usage/parse error; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let stdout = String::from_utf8_lossy(&out.stdout);
    let trimmed = stdout.trim();
    if !trimmed.is_empty() {
        assert!(
            trimmed.starts_with('{') || trimmed.starts_with('['),
            "--json stdout must be a JSON object or array; got: {}",
            &trimmed[..trimmed.len().min(200)]
        );
        // Verify that all opening brackets have matching closing brackets.
        let opens: usize = trimmed
            .chars()
            .filter(|&c| c == '{' || c == '[')
            .count();
        let closes: usize = trimmed
            .chars()
            .filter(|&c| c == '}' || c == ']')
            .count();
        assert_eq!(
            opens, closes,
            "--json output must have balanced braces; stdout: {}",
            &trimmed[..trimmed.len().min(200)]
        );
    }

    let _ = std::fs::remove_dir_all(&dir);
}