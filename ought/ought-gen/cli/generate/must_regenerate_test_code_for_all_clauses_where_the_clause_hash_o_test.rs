/// MUST regenerate test code for all clauses where the clause hash or source hash has changed
#[test]
fn test_cli__generate__must_regenerate_test_code_for_all_clauses_where_the_clause_hash_o() {
    let dir = std::env::temp_dir()
        .join(format!("ought_gen_rehash_{}", std::process::id()));
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

    // Write a manifest with a deliberately wrong clause hash so the clause is stale.
    std::fs::create_dir_all(dir.join("ought/ought-gen")).unwrap();
    std::fs::write(
        dir.join("ought/ought-gen/manifest.toml"),
        "[\"spec::section::must_do_the_thing\"]\n\
         clause_hash = \"old_stale_hash_that_will_not_match\"\n\
         source_hash = \"\"\n\
         generated_at = \"2020-01-01T00:00:00Z\"\n\
         model = \"test\"\n",
    )
    .unwrap();

    let bin = option_env!("CARGO_BIN_EXE_ought")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("ought"));

    // --check detects staleness without invoking the LLM.
    let out = std::process::Command::new(&bin)
        .arg("generate")
        .arg("--check")
        .current_dir(&dir)
        .output()
        .expect("failed to invoke ought generate --check");

    // When any clause hash is stale, --check must exit 1.
    assert_eq!(
        out.status.code(),
        Some(1),
        "ought generate --check must exit 1 when clause hash has changed; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("stale"),
        "ought generate --check must report stale clauses; stderr: {stderr}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}