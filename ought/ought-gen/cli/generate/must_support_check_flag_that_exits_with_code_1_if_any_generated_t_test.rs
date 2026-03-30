/// MUST support `--check` flag that exits with code 1 if any generated tests are stale (for CI)
#[test]
fn test_cli__generate__must_support_check_flag_that_exits_with_code_1_if_any_generated_t() {
    let dir = std::env::temp_dir()
        .join(format!("ought_gen_check_{}", std::process::id()));
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
        dir.join("ought/spec.ought.md"),
        "# Spec\n\ncontext: test\n\n## Section\n\n- **MUST** do the thing\n",
    )
    .unwrap();

    // No manifest: every clause is stale by definition.
    let bin = option_env!("CARGO_BIN_EXE_ought")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("ought"));

    let out = std::process::Command::new(&bin)
        .arg("generate")
        .arg("--check")
        .current_dir(&dir)
        .output()
        .expect("failed to invoke ought generate --check");

    // --check must be accepted by clap (not a usage error).
    assert_ne!(
        out.status.code(),
        Some(2),
        "--check must be a recognised flag; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    // With stale clauses --check must exit 1.
    assert_eq!(
        out.status.code(),
        Some(1),
        "ought generate --check must exit 1 when generated tests are stale; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let _ = std::fs::remove_dir_all(&dir);
}