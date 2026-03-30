/// MUST support `--config <path>` to specify an alternate ought.toml location
#[test]
fn test_cli__global_flags__must_support_config_path_to_specify_an_alternate_ought_toml_locat() {
    let base = std::env::temp_dir()
        .join(format!("ought_cfg_flag_{}", std::process::id()));
    let specs_dir = base.join("specs");
    let alt_config = base.join("alt_ought.toml");
    std::fs::create_dir_all(&specs_dir).unwrap();

    std::fs::write(
        specs_dir.join("test.ought.md"),
        "# Test\n\n## Section: Basics\n\n### MUST do something\n",
    )
    .unwrap();

    // Write a valid ought.toml at a non-default location.
    std::fs::write(
        &alt_config,
        format!(
            "[project]\nname = \"test\"\nversion = \"0.1.0\"\n\n\
             [specs]\nroots = [\"{specs}\"]\n\n\
             [context]\nsearch_paths = [\"{base}\"]\n\n\
             [generator]\nprovider = \"anthropic\"\n",
            specs = specs_dir.display(),
            base = base.display(),
        ),
    )
    .unwrap();

    let bin = option_env!("CARGO_BIN_EXE_ought")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("ought"));

    // Without --config and no ought.toml in `base`, config discovery must fail.
    let no_cfg = std::process::Command::new(&bin)
        .arg("check")
        .current_dir(&base)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .expect("ought check (no config)");
    assert_ne!(
        no_cfg.status.code(),
        Some(0),
        "check with no discoverable config must not succeed"
    );

    // With --config pointing to the alternate file, ought must load it and succeed.
    let with_cfg = std::process::Command::new(&bin)
        .args(["--config", alt_config.to_str().unwrap(), "check"])
        .current_dir(&base)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .expect("ought --config check");
    assert_eq!(
        with_cfg.status.code(),
        Some(0),
        "--config must load the alternate file; stderr: {}",
        String::from_utf8_lossy(&with_cfg.stderr)
    );

    let _ = std::fs::remove_dir_all(&base);
}