/// MUST write generated tests to the `ought/ought-gen/` directory
#[test]
fn test_cli__generate__must_write_generated_tests_to_the_ought_ought_gen_directory() {
    let dir = std::env::temp_dir()
        .join(format!("ought_gen_outdir_{}", std::process::id()));
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

    // ought/ought-gen/ must not exist yet.
    let gen_dir = dir.join("ought/ought-gen");
    assert!(
        !gen_dir.exists(),
        "ought/ought-gen/ must not exist before generate"
    );

    let bin = option_env!("CARGO_BIN_EXE_ought")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("ought"));

    // --check avoids calling the LLM while still exercising the generate path.
    std::process::Command::new(&bin)
        .arg("generate")
        .arg("--check")
        .current_dir(&dir)
        .output()
        .expect("failed to invoke ought generate --check");

    assert!(
        gen_dir.is_dir(),
        "ought generate must create the ought/ought-gen/ output directory"
    );

    let _ = std::fs::remove_dir_all(&dir);
}