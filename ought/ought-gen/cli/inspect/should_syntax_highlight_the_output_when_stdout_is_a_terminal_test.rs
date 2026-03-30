/// SHOULD syntax-highlight the output when stdout is a terminal
#[test]
fn test_cli__inspect__should_syntax_highlight_the_output_when_stdout_is_a_terminal() {
    // When stdout is piped (non-terminal), the command must still produce plain
    // readable output.  Syntax highlighting, if supported, must be suppressed in
    // that case so downstream tools are not polluted with ANSI escape sequences.
    // When `--color=always` is requested the output MAY contain highlighting,
    // but must still contain the test code.
    let dir = std::env::temp_dir()
        .join(format!("ought_inspect_hl_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();

    std::fs::write(
        dir.join("ought.toml"),
        "[project]\nname = \"test\"\nversion = \"0.1.0\"\n\n\
         [specs]\nroots = [\"ought/\"]\n\n\
         [context]\nsearch_paths = [\"src/\"]\nexclude = []\n\n\
         [generator]\nprovider = \"anthropic\"\n\n\
         [runner.rust]\ncommand = \"cargo test\"\ntest_dir = \"ought/ought-gen/\"\n",
    )
    .unwrap();

    let gen_dir = dir.join("ought/ought-gen/auth/login");
    std::fs::create_dir_all(&gen_dir).unwrap();
    let test_code =
        "#[test]\nfn test_auth__login__must_return_jwt() {\n    assert!(true);\n}\n";
    std::fs::write(gen_dir.join("must_return_jwt.rs"), test_code).unwrap();

    let bin = option_env!("CARGO_BIN_EXE_ought")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("ought"));

    // Without a terminal (piped), output must not contain ANSI escape codes so
    // the raw test code is machine-readable.
    let plain_output = std::process::Command::new(&bin)
        .arg("--color=never")
        .arg("inspect")
        .arg("auth::login::must_return_jwt")
        .current_dir(&dir)
        .output()
        .expect("failed to invoke ought inspect --color=never");

    assert!(
        plain_output.status.success(),
        "ought inspect must succeed with --color=never; stderr: {}",
        String::from_utf8_lossy(&plain_output.stderr)
    );

    let plain_stdout = String::from_utf8_lossy(&plain_output.stdout);
    assert!(
        !plain_stdout.contains('\x1b'),
        "ought inspect must not emit ANSI escape sequences when --color=never is set; \
         got:\n{plain_stdout}"
    );
    assert!(
        plain_stdout.contains("test_auth__login__must_return_jwt"),
        "ought inspect must still print the test code when --color=never; got:\n{plain_stdout}"
    );

    // With `--color=always`, the test code must still be present in the output
    // (even if highlighting is not yet implemented).
    let color_output = std::process::Command::new(&bin)
        .arg("--color=always")
        .arg("inspect")
        .arg("auth::login::must_return_jwt")
        .current_dir(&dir)
        .output()
        .expect("failed to invoke ought inspect --color=always");

    assert!(
        color_output.status.success(),
        "ought inspect must succeed with --color=always; stderr: {}",
        String::from_utf8_lossy(&color_output.stderr)
    );

    let color_stdout = String::from_utf8_lossy(&color_output.stdout);
    assert!(
        color_stdout.contains("test_auth__login__must_return_jwt"),
        "ought inspect must include the test code in --color=always output; got:\n{color_stdout}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}