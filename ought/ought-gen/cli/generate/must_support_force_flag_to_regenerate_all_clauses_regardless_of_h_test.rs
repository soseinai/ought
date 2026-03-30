/// MUST support `--force` flag to regenerate all clauses regardless of hash
#[test]
fn test_cli__generate__must_support_force_flag_to_regenerate_all_clauses_regardless_of_h() {
    let dir = std::env::temp_dir()
        .join(format!("ought_gen_force_{}", std::process::id()));
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

    let bin = option_env!("CARGO_BIN_EXE_ought")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("ought"));

    // --force must be a recognised flag (clap exit 2 = usage error).
    let out = std::process::Command::new(&bin)
        .arg("generate")
        .arg("--force")
        .arg("--check")
        .current_dir(&dir)
        .output()
        .expect("failed to invoke ought generate --force --check");

    assert_ne!(
        out.status.code(),
        Some(2),
        "--force must be a recognised flag; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    // With --force all clauses are treated as stale regardless of manifest state.
    // Combining with --check avoids an LLM call while still asserting forced staleness.
    assert_eq!(
        out.status.code(),
        Some(1),
        "ought generate --force --check must exit 1 because --force marks every clause stale; \
         stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let _ = std::fs::remove_dir_all(&dir);
}