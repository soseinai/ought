/// MUST NOT execute tests during generation (that is `run`'s job)
#[test]
fn test_cli__generate__must_not_execute_tests_during_generation_that_is_run_s_job() {
    let dir = std::env::temp_dir()
        .join(format!("ought_gen_norun_{}", std::process::id()));
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

    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );

    // cargo test output always contains "test result:" or "running N test".
    // Neither must appear during generation.
    assert!(
        !combined.contains("test result:"),
        "ought generate must not execute tests; found 'test result:' in output: {combined}"
    );
    assert!(
        !combined.contains("running 1 test") && !combined.contains("running 0 tests"),
        "ought generate must not invoke the test runner; found runner output: {combined}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}