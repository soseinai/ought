/// MUST accept clause identifiers in the form `file::section::clause`
/// (e.g. `auth::login::must_return_jwt`)
#[test]
fn test_cli__inspect__must_accept_clause_identifiers_in_the_form_file_section_clause_e() {
    let dir = std::env::temp_dir()
        .join(format!("ought_inspect_idform_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();

    std::fs::write(
        dir.join("ought.toml"),
        "[project]\nname = \"test\"\nversion = \"0.1.0\"\n\n\
         [specs]\nroots = [\"ought/\"]\n\n\
         [context]\nsearch_paths = [\"src/\"]\nexclude = []\n\n\
         [generator]\nprovider = \"anthropic\"\n\n\
         [runner.rust]\ncommand = \"cargo test\"\ntest_dir = \"ought/ought-gen/\"\n",
    )
    .unwrap();

    let gen_dir = dir.join("ought/ought-gen/auth/login");
    std::fs::create_dir_all(&gen_dir).unwrap();
    std::fs::write(
        gen_dir.join("must_return_jwt.rs"),
        "#[test]\nfn test_auth__login__must_return_jwt() {}\n",
    )
    .unwrap();

    let bin = option_env!("CARGO_BIN_EXE_ought")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("ought"));

    // Pass a three-part `file::section::clause` identifier — must not be rejected
    // with a usage error (exit code 2).
    let output = std::process::Command::new(&bin)
        .arg("inspect")
        .arg("auth::login::must_return_jwt")
        .current_dir(&dir)
        .output()
        .expect("failed to invoke ought inspect");

    assert_ne!(
        output.status.code(),
        Some(2),
        "ought inspect must not reject a `file::section::clause` identifier as a usage error; \
         stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.status.success(),
        "ought inspect must exit 0 for a well-formed `file::section::clause` identifier \
         when the generated file exists; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = std::fs::remove_dir_all(&dir);
}