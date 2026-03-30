/// MUST scaffold an `ought.toml`, an `ought/` directory, and an example spec file inside it
/// when run in a project directory.
#[test]
fn test_cli__init__must_scaffold_an_ought_toml_an_ought_directory_and_an_example_spe() {
    let dir = std::env::temp_dir()
        .join(format!("ought_init_scaffold_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();

    let bin = option_env!("CARGO_BIN_EXE_ought")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("ought"));

    let output = std::process::Command::new(&bin)
        .arg("init")
        .current_dir(&dir)
        .output()
        .expect("failed to run ought init");

    assert!(
        output.status.success(),
        "ought init should exit 0; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    assert!(
        dir.join("ought.toml").exists(),
        "ought.toml must be created by ought init"
    );

    assert!(
        dir.join("ought").is_dir(),
        "ought/ directory must be created by ought init"
    );

    let spec_files: Vec<_> = std::fs::read_dir(dir.join("ought"))
        .expect("ought/ must be readable")
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .map(|x| x == "md")
                .unwrap_or(false)
        })
        .collect();

    assert!(
        !spec_files.is_empty(),
        "at least one example spec file must exist inside ought/"
    );

    let _ = std::fs::remove_dir_all(&dir);
}