/// SHOULD support targeting a specific spec file: `ought generate ought/auth.ought.md`
#[test]
fn test_cli__generate__should_support_targeting_a_specific_spec_file_ought_generate_ought() {
    let dir = std::env::temp_dir()
        .join(format!("ought_gen_target_{}", std::process::id()));
    std::fs::create_dir_all(dir.join("ought")).unwrap();

    std::fs::write(
        dir.join("ought.toml"),
        "[project]\nname = \"test\"\nversion = \"0.1.0\"\n\n\
         [specs]\nroots = [\"ought/\"]\n\n\
         [context]\nsearch_paths = [\"src/\"]\nexclude = [\"target/\"]\n\n\
         [generator]\nprovider = \"anthropic\"\n\n\
         [runner.rust]\ncommand = \"cargo test\"\ntest_dir = \"ought/ought-gen/\"\n",
    )
    .unwrap();

    // Two spec files; the test targets only the auth one.
    std::fs::write(
        dir.join("ought/auth.ought.md"),
        "# Auth\n\ncontext: test\n\n## Login\n\n- **MUST** authenticate users\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("ought/billing.ought.md"),
        "# Billing\n\ncontext: test\n\n## Payments\n\n- **MUST** charge the card\n",
    )
    .unwrap();

    let bin = option_env!("CARGO_BIN_EXE_ought")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("ought"));

    let out = std::process::Command::new(&bin)
        .arg("generate")
        .arg("ought/auth.ought.md")
        .arg("--check")
        .current_dir(&dir)
        .output()
        .expect("failed to invoke ought generate with a specific spec path");

    // A path argument must not cause a clap usage error (exit 2).
    assert_ne!(
        out.status.code(),
        Some(2),
        "ought generate must accept a specific spec file path without a usage error; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let _ = std::fs::remove_dir_all(&dir);
}