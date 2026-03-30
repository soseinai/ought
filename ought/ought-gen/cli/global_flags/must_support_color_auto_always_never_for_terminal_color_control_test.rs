/// MUST support `--color <auto|always|never>` for terminal color control
#[test]
fn test_cli__global_flags__must_support_color_auto_always_never_for_terminal_color_control() {
    let bin = option_env!("CARGO_BIN_EXE_ought")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("ought"));

    // Each variant must be accepted by the argument parser (exit code must not be 2).
    for color_value in ["auto", "always", "never"] {
        let dir = std::env::temp_dir().join(format!(
            "ought_color_{}_{}_{}",
            color_value,
            std::process::id(),
            color_value.len() // extra salt to keep names distinct in the loop
        ));
        std::fs::create_dir_all(&dir).unwrap();

        let out = std::process::Command::new(&bin)
            .args(["--color", color_value, "init"])
            .current_dir(&dir)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .output()
            .unwrap_or_else(|e| {
                panic!("failed to run `ought --color {color_value} init`: {e}")
            });

        assert_ne!(
            out.status.code(),
            Some(2),
            "--color {color_value} must not produce a usage/parse error; stderr: {}",
            String::from_utf8_lossy(&out.stderr)
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    // An invalid value must be rejected with exit code 2.
    let dir_invalid = std::env::temp_dir()
        .join(format!("ought_color_invalid_{}", std::process::id()));
    std::fs::create_dir_all(&dir_invalid).unwrap();
    let bad = std::process::Command::new(&bin)
        .args(["--color", "rainbow", "init"])
        .current_dir(&dir_invalid)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .expect("ought --color rainbow init");
    assert_eq!(
        bad.status.code(),
        Some(2),
        "--color with an invalid value must produce a usage error (exit 2)"
    );
    let _ = std::fs::remove_dir_all(&dir_invalid);
}