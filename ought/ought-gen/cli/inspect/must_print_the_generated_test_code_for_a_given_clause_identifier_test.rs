/// MUST print the generated test code for a given clause identifier
#[test]
fn test_cli__inspect__must_print_the_generated_test_code_for_a_given_clause_identifier() {
    let dir = std::env::temp_dir()
        .join(format!("ought_inspect_print_{}", std::process::id()));
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
    let expected_code =
        "#[test]\nfn test_auth__login__must_return_jwt() {\n    assert!(true);\n}\n";
    std::fs::write(gen_dir.join("must_return_jwt.rs"), expected_code).unwrap();

    let bin = option_env!("CARGO_BIN_EXE_ought")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("ought"));

    let output = std::process::Command::new(&bin)
        .arg("inspect")
        .arg("auth::login::must_return_jwt")
        .current_dir(&dir)
        .output()
        .expect("failed to invoke ought inspect");

    assert!(
        output.status.success(),
        "ought inspect must exit 0 when the clause file exists; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("test_auth__login__must_return_jwt"),
        "ought inspect must print the generated test code to stdout; got:\n{stdout}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}