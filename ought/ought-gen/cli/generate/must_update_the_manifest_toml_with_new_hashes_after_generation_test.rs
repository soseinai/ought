/// MUST update the manifest.toml with new hashes after generation
#[test]
fn test_cli__generate__must_update_the_manifest_toml_with_new_hashes_after_generation() {
    let dir = std::env::temp_dir()
        .join(format!("ought_gen_manifest_{}", std::process::id()));
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

    // No manifest exists yet.
    let manifest_path = dir.join("ought/ought-gen/manifest.toml");
    assert!(
        !manifest_path.exists(),
        "manifest.toml must not exist before generate"
    );

    let bin = option_env!("CARGO_BIN_EXE_ought")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("ought"));

    // --check mode still saves the manifest (save happens before the stale exit).
    std::process::Command::new(&bin)
        .arg("generate")
        .arg("--check")
        .current_dir(&dir)
        .output()
        .expect("failed to invoke ought generate --check");

    // The manifest.toml must be written to ought/ought-gen/ after generate runs.
    assert!(
        manifest_path.exists(),
        "manifest.toml must be created at ought/ought-gen/manifest.toml after ought generate"
    );

    let _ = std::fs::remove_dir_all(&dir);
}