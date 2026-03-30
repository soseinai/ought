/// MUST detect the project language from existing config files (Cargo.toml, package.json,
/// pyproject.toml, go.mod) and set defaults accordingly.
#[test]
fn test_cli__init__must_detect_the_project_language_from_existing_config_files_cargo() {
    let bin = option_env!("CARGO_BIN_EXE_ought")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("ought"));

    // (marker file, expected language key, expected runner command substring)
    let cases: &[(&str, &str, &str)] = &[
        ("Cargo.toml", "rust", "cargo test"),
        ("package.json", "typescript", "jest"),
        ("pyproject.toml", "python", "pytest"),
        ("go.mod", "go", "go test"),
    ];

    for (marker, lang, cmd_hint) in cases {
        let dir = std::env::temp_dir().join(format!(
            "ought_init_lang_{}_{}_{}",
            lang,
            std::process::id(),
            // extra entropy so parallel tests on the same pid don't collide
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .subsec_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();

        // Write the language marker file with minimal valid content.
        let content = match *marker {
            "Cargo.toml" => "[package]\nname = \"proj\"\nversion = \"0.1.0\"\n",
            "package.json" => "{\"name\":\"proj\"}\n",
            "pyproject.toml" => "[tool.poetry]\nname = \"proj\"\n",
            "go.mod" => "module proj\n\ngo 1.21\n",
            _ => "",
        };
        std::fs::write(dir.join(marker), content).unwrap();

        let output = std::process::Command::new(&bin)
            .arg("init")
            .current_dir(&dir)
            .output()
            .unwrap_or_else(|e| panic!("failed to run ought init for {}: {}", lang, e));

        assert!(
            output.status.success(),
            "ought init should succeed for {} project; stderr: {}",
            lang,
            String::from_utf8_lossy(&output.stderr)
        );

        let config =
            std::fs::read_to_string(dir.join("ought.toml")).expect("ought.toml must be created");

        assert!(
            config.contains(&format!("[runner.{}]", lang)),
            "ought.toml must contain [runner.{}] for a {} project; got:\n{}",
            lang,
            lang,
            config
        );

        assert!(
            config.contains(cmd_hint),
            "runner command for {} must contain '{}'; got:\n{}",
            lang,
            cmd_hint,
            config
        );

        let _ = std::fs::remove_dir_all(&dir);
    }
}