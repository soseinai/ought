/// SHOULD show a progress indicator during LLM generation
#[test]
fn test_cli__generate__should_show_a_progress_indicator_during_llm_generation() {
    let dir = std::env::temp_dir()
        .join(format!("ought_gen_progress_{}", std::process::id()));
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

    let out = std::process::Command::new(&bin)
        .arg("generate")
        .arg("--check")
        .current_dir(&dir)
        .output()
        .expect("failed to invoke ought generate --check");

    // The generate command must emit diagnostic output to stderr so the user
    // can see activity (stale clause IDs, section headers, and/or a summary line).
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !stderr.trim().is_empty(),
        "ought generate must produce diagnostic output on stderr as a progress indicator; \
         got empty stderr"
    );

    let _ = std::fs::remove_dir_all(&dir);
}