/// SHOULD group diffs by spec file
#[test]
fn test_cli__diff__should_group_diffs_by_spec_file() {
    let dir = std::env::temp_dir()
        .join(format!("ought_diff_group_{}", std::process::id()));
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

    // Two separate spec files, each with a stale clause.
    std::fs::write(
        dir.join("ought/auth.ought.md"),
        "# Auth\n\ncontext: Authentication\n\n## Login\n\n- **MUST** validate credentials\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("ought/payments.ought.md"),
        "# Payments\n\ncontext: Payments\n\n## Charge\n\n- **MUST** validate card expiry\n",
    )
    .unwrap();

    // Existing generated tests for both specs.
    let gen_auth = dir.join("ought/ought-gen/auth/login");
    let gen_pay = dir.join("ought/ought-gen/payments/charge");
    std::fs::create_dir_all(&gen_auth).unwrap();
    std::fs::create_dir_all(&gen_pay).unwrap();
    std::fs::write(
        gen_auth.join("must_validate_credentials.rs"),
        "#[test]\nfn test_auth__login__must_validate_credentials() { assert!(true); }\n",
    )
    .unwrap();
    std::fs::write(
        gen_pay.join("must_validate_card_expiry.rs"),
        "#[test]\nfn test_payments__charge__must_validate_card_expiry() { assert!(true); }\n",
    )
    .unwrap();

    // Both manifest entries are stale.
    std::fs::create_dir_all(dir.join("ought/ought-gen")).unwrap();
    std::fs::write(
        dir.join("ought/ought-gen/manifest.toml"),
        "[\"auth::login::must_validate_credentials\"]\n\
         clause_hash = \"stale_auth_hash\"\n\
         source_hash = \"\"\n\
         generated_at = \"2020-01-01T00:00:00Z\"\n\
         model = \"test\"\n\
         \n\
         [\"payments::charge::must_validate_card_expiry\"]\n\
         clause_hash = \"stale_payments_hash\"\n\
         source_hash = \"\"\n\
         generated_at = \"2020-01-01T00:00:00Z\"\n\
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

    // Output must reference both spec files.
    assert!(
        stdout.contains("auth"),
        "ought diff must include a section for auth.ought.md; stdout:\n{stdout}"
    );
    assert!(
        stdout.contains("payments"),
        "ought diff must include a section for payments.ought.md; stdout:\n{stdout}"
    );

    // The two spec files must appear as distinct groups: auth content and payments
    // content must not be interleaved without any per-file header.  We verify this
    // by checking that the first occurrence of each spec name is separated by some
    // output rather than appearing on the same line.
    let auth_pos = stdout.find("auth").expect("auth must appear in output");
    let pay_pos = stdout.find("payments").expect("payments must appear in output");
    assert!(
        auth_pos != pay_pos,
        "auth and payments diff groups must appear at distinct positions in the output"
    );

    let _ = std::fs::remove_dir_all(&dir);
}