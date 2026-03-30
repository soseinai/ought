/// MUST accept a clause identifier
#[test]
fn test_analysis__bisect__must_accept_a_clause_identifier() {
    struct StubRunner;
    impl Runner for StubRunner {
        fn run(&self, _: &[GeneratedTest], _: &std::path::Path) -> anyhow::Result<RunResult> {
            Ok(RunResult { results: vec![], total_duration: Duration::ZERO })
        }
        fn is_available(&self) -> bool { true }
        fn name(&self) -> &str { "stub" }
    }

    let clause_id = ClauseId("auth::login::must_return_401".to_string());
    let base = std::env::temp_dir()
        .join(format!("ought_bisect_accept_id_{}", std::process::id()));
    let spec_dir = base.join("specs");
    let src_dir = base.join("src");
    fs::create_dir_all(&spec_dir).unwrap();
    fs::create_dir_all(&src_dir).unwrap();
    fs::write(
        spec_dir.join("auth.ought.md"),
        "# Auth\n\n## Login\n\n- **MUST** return 401 for invalid credentials\n",
    ).unwrap();
    fs::write(src_dir.join("status.txt"), "fail\n").unwrap();
    for args in &[
        vec!["init"],
        vec!["config", "user.email", "test@example.com"],
        vec!["config", "user.name", "Test Runner"],
    ] {
        std::process::Command::new("git").args(args).current_dir(&base).output().unwrap();
    }
    std::process::Command::new("git").args(["add", "."]).current_dir(&base).output().unwrap();
    std::process::Command::new("git")
        .args(["commit", "-m", "Passing state"])
        .current_dir(&base).output().unwrap();
    // Add a second commit so bisect has a range to search.
    fs::write(src_dir.join("status.txt"), "fail\n").unwrap();
    std::process::Command::new("git").args(["add", "."]).current_dir(&base).output().unwrap();
    std::process::Command::new("git")
        .args(["commit", "-m", "Breaking change"])
        .current_dir(&base).output().unwrap();

    let specs = SpecGraph::from_roots(&[spec_dir.clone()]).expect("spec graph should parse");
    let options = BisectOptions { range: None, regenerate: false };
    let res = bisect(&clause_id, &specs, &StubRunner, &options);
    assert!(
        res.is_ok(),
        "bisect must accept a ClauseId and return Ok; err: {:?}",
        res.err()
    );
    assert_eq!(
        res.unwrap().clause_id,
        clause_id,
        "bisect result must carry back the same clause_id that was passed in"
    );
    let _ = fs::remove_dir_all(&base);
}