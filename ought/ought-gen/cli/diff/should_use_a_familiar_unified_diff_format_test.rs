/// SHOULD use a familiar unified diff format
#[test]
fn test_cli__diff__should_use_a_familiar_unified_diff_format() {
    let dir = std::env::temp_dir()
        .join(format!("ought_diff_format_{}", std::process::id()));
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
        dir.join("ought/payments.ought.md"),
        "# Payments\n\ncontext: Payment processing\n\n## Charge\n\n- **MUST** reject invalid card numbers\n",
    )
    .unwrap();

    // Existing generated test — the "before" side of the diff.
    let gen_dir = dir.join("ought/ought-gen/payments/charge");
    std::fs::create_dir_all(&gen_dir).unwrap();
    std::fs::write(
        gen_dir.join("must_reject_invalid_card_numbers.rs"),
        "#[test]\nfn test_payments__charge__must_reject_invalid_card_numbers() {\n    // old body\n    assert!(true);\n}\n",
    )
    .unwrap();

    // Stale manifest entry to give diff something to show.
    std::fs::create_dir_all(dir.join("ought/ought-gen")).unwrap();
    std::fs::write(
        dir.join("ought/ought-gen/manifest.toml"),
        "[\"payments::charge::must_reject_invalid_card_numbers\"]\n\
         clause_hash = \"outdated_hash_does_not_match_current_clause\"\n\
         source_hash = \"\"\n\
         generated_at = \"2020-06-01T00:00:00Z\"\n\
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

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Unified diff format requires --- / +++ file headers and @@ hunk markers.
    assert!(
        stdout.contains("---") && stdout.contains("+++"),
        "ought diff output must use unified diff format with --- and +++ file headers;\
         \nstdout:\n{stdout}"
    );
    assert!(
        stdout.contains("@@"),
        "ought diff output must include @@ hunk markers as in unified diff format;\
         \nstdout:\n{stdout}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}