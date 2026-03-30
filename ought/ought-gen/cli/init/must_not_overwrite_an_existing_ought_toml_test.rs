/// MUST NOT overwrite an existing `ought.toml`.
#[test]
fn test_cli__init__must_not_overwrite_an_existing_ought_toml() {
    let dir = std::env::temp_dir()
        .join(format!("ought_init_no_overwrite_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();

    let original_content =
        "# pre-existing config\n[project]\nname = \"existing\"\nversion = \"9.9.9\"\n";
    std::fs::write(dir.join("ought.toml"), original_content).unwrap();

    let bin = option_env!("CARGO_BIN_EXE_ought")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("ought"));

    let output = std::process::Command::new(&bin)
        .arg("init")
        .current_dir(&dir)
        .output()
        .expect("failed to run ought init");

    assert!(
        !output.status.success(),
        "ought init must exit non-zero when ought.toml already exists"
    );

    let after = std::fs::read_to_string(dir.join("ought.toml"))
        .expect("ought.toml must still exist after the failed init");

    assert_eq!(
        after, original_content,
        "ought.toml content must be unchanged when init is refused"
    );

    let _ = std::fs::remove_dir_all(&dir);
}