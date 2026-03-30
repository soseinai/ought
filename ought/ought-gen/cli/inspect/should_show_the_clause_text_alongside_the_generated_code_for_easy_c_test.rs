/// SHOULD show the clause text alongside the generated code for easy comparison
#[test]
fn test_cli__inspect__should_show_the_clause_text_alongside_the_generated_code_for_easy_c() {
    let dir = std::env::temp_dir()
        .join(format!("ought_inspect_clause_text_{}", std::process::id()));
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

    // Write a spec file that contains the clause under test.
    let spec_dir = dir.join("ought");
    std::fs::create_dir_all(&spec_dir).unwrap();
    std::fs::write(
        spec_dir.join("auth.ought.md"),
        "# Auth\n\n## Login\n\n- **MUST** return a JWT token\n",
    )
    .unwrap();

    // Write the corresponding generated test file.
    let gen_dir = dir.join("ought/ought-gen/auth/login");
    std::fs::create_dir_all(&gen_dir).unwrap();
    let test_code =
        "#[test]\nfn test_auth__login__must_return_jwt() {\n    assert!(true);\n}\n";
    std::fs::write(gen_dir.join("must_return_jwt.rs"), test_code).unwrap();

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
        "ought inspect must exit 0; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );

    // The clause text ("return a JWT token") should appear alongside the code so
    // the developer can compare the spec intent with the generated test.
    assert!(
        combined.contains("return a JWT token"),
        "ought inspect should show the clause text alongside the generated code for \
         easy comparison; output was:\n{combined}"
    );

    // The generated code must also be present.
    assert!(
        combined.contains("test_auth__login__must_return_jwt"),
        "ought inspect must always include the generated test code; output was:\n{combined}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}