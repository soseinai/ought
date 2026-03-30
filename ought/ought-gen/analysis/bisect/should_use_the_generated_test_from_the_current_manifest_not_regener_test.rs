/// SHOULD use the generated test from the current manifest (not regenerate per commit) unless `--regenerate` is passed
#[test]
fn test_analysis__bisect__should_use_the_generated_test_from_the_current_manifest_not_regener() {
    use std::sync::{Arc, Mutex};

    // Spy runner captures the code of every test it receives so we can verify it came from the manifest.
    struct SpyRunner {
        clause_id: ClauseId,
        received_codes: Arc<Mutex<Vec<String>>>,
    }
    impl Runner for SpyRunner {
        fn run(&self, tests: &[GeneratedTest], _: &std::path::Path) -> anyhow::Result<RunResult> {
            let codes: Vec<String> = tests.iter().map(|t| t.code.clone()).collect();
            self.received_codes.lock().unwrap().extend(codes);
            Ok(RunResult {
                results: vec![TestResult {
                    clause_id: self.clause_id.clone(),
                    status: TestStatus::Failed,
                    message: Some("intentional failure for bisect".to_string()),
                    duration: Duration::ZERO,
                    details: TestDetails { failure_message: None, stack_trace: None, iterations: None, measured_duration: None },
                }],
                total_duration: Duration::ZERO,
            })
        }
        fn is_available(&self) -> bool { true }
        fn name(&self) -> &str { "spy" }
    }

    let base = std::env::temp_dir()
        .join(format!("ought_bisect_manifest_{}", std::process::id()));
    let spec_dir = base.join("specs");
    let gen_dir = base.join("ought-gen");
    let src_dir = base.join("src");
    fs::create_dir_all(&spec_dir).unwrap();
    fs::create_dir_all(&gen_dir).unwrap();
    fs::create_dir_all(&src_dir).unwrap();
    fs::write(
        spec_dir.join("auth.ought.md"),
        "# Auth\n\n## Login\n\n- **MUST** return 401 for invalid credentials\n",
    ).unwrap();

    // Write an existing generated test to the gen directory (simulating the current manifest).
    let canonical_test_code = r#"/// canonical test from manifest
#[test]
fn test_auth__login__must_return_401() {
    assert_eq!(401u16, 401u16);
}"#;
    let test_file = gen_dir.join("auth").join("login");
    fs::create_dir_all(&test_file).unwrap();
    fs::write(
        test_file.join("must_return_401_for_invalid_credentials_test.rs"),
        canonical_test_code,
    ).unwrap();

    for git_args in &[
        vec!["init"],
        vec!["config", "user.email", "test@example.com"],
        vec!["config", "user.name", "Test Runner"],
    ] {
        std::process::Command::new("git").args(git_args).current_dir(&base).output().unwrap();
    }
    fs::write(src_dir.join("auth.rs"), "pub fn status() -> u16 { 401 }\n").unwrap();
    std::process::Command::new("git").args(["add", "."]).current_dir(&base).output().unwrap();
    std::process::Command::new("git")
        .args(["commit", "-m", "initial passing state"])
        .current_dir(&base).output().unwrap();
    fs::write(src_dir.join("auth.rs"), "pub fn status() -> u16 { 200 }\n").unwrap();
    std::process::Command::new("git").args(["add", "."]).current_dir(&base).output().unwrap();
    std::process::Command::new("git")
        .args(["commit", "-m", "breaking change"])
        .current_dir(&base).output().unwrap();

    let clause_id = ClauseId("auth::login::must_return_401_for_invalid_credentials".to_string());
    let received_codes = Arc::new(Mutex::new(Vec::<String>::new()));
    let runner = SpyRunner { clause_id: clause_id.clone(), received_codes: Arc::clone(&received_codes) };
    let specs = SpecGraph::from_roots(&[spec_dir.clone()]).expect("spec graph should parse");
    // Without --regenerate, bisect should reuse the manifest test.
    let options = BisectOptions { range: None, regenerate: false };

    let _ = bisect(&clause_id, &specs, &runner, &options);

    let codes = received_codes.lock().unwrap();
    assert!(
        !codes.is_empty(),
        "bisect must invoke the runner at least once"
    );
    assert!(
        codes.iter().all(|c| c.contains("canonical test from manifest")),
        "without --regenerate, bisect must reuse the manifest test for every commit; \
         got codes that differ from manifest: {:?}",
        &codes
    );

    let _ = fs::remove_dir_all(&base);
}