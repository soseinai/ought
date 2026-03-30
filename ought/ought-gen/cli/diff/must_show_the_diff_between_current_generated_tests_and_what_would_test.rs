/// MUST show the diff between current generated tests and what would be generated now
#[test]
fn test_cli__diff__must_show_the_diff_between_current_generated_tests_and_what_would() {
    let dir = std::env::temp_dir()
        .join(format!("ought_diff_show_{}", std::process::id()));
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

    std::fs::write(
        dir.join("ought/auth.ought.md"),
        "# Auth\n\ncontext: Authentication service\n\n## Login\n\n- **MUST** return a JWT on success\n",
    )
    .unwrap();

    // Write an existing generated test representing the "current" state on disk.
    let gen_dir = dir.join("ought/ought-gen/auth/login");
    std::fs::create_dir_all(&gen_dir).unwrap();
    std::fs::write(
        gen_dir.join("must_return_a_jwt_on_success.rs"),
        "#[test]\nfn test_auth__login__must_return_a_jwt_on_success() {\n    // generated from old clause text\n    assert!(false, \"placeholder\");\n}\n",
    )
    .unwrap();

    // Manifest with a stale hash so ought diff has a change to report.
    std::fs::create_dir_all(dir.join("ought/ought-gen")).unwrap();
    std::fs::write(
        dir.join("ought/ought-gen/manifest.toml"),
        "[\"auth::login::must_return_a_jwt_on_success\"]\n\
         clause_hash = \"old_stale_hash_does_not_match_current\"\n\
         source_hash = \"\"\n\
         generated_at = \"2020-01-01T00:00:00Z\"\n\
         model = \"test\"\n",
    )
    .unwrap();

    let bin = option_env!("CARGO_BIN_EXE_ought")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("ought"));

    let output = std::process::Command::new(&bin)
        .arg("diff")
        .current_dir(&dir)
        .output()
        .expect("failed to invoke ought diff");

    // Must not exit with a clap usage error.
    assert_ne!(
        output.status.code(),
        Some(2),
        "ought diff must not exit with a usage error (2); stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    // Must not be the unimplemented stub message.
    assert!(
        !combined.contains("not yet implemented"),
        "ought diff must be implemented and show actual diffs; got:\n{combined}"
    );

    // Must produce output when stale clauses exist.
    assert!(
        !combined.trim().is_empty(),
        "ought diff must produce output when generated tests are stale; got no output"
    );

    // Output must reference the affected clause or file.
    assert!(
        combined.contains("auth") || combined.contains("must_return_a_jwt_on_success"),
        "ought diff output must reference the stale clause or its spec; got:\n{combined}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}