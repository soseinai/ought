/// MUST show the commit message, author, date, and diff summary for the breaking commit
#[test]
fn test_analysis__bisect__must_show_the_commit_message_author_date_and_diff_summary_for_the() {
    struct FileStatusRunner {
        sentinel: PathBuf,
        clause_id: ClauseId,
    }
    impl Runner for FileStatusRunner {
        fn run(&self, _: &[GeneratedTest], _: &std::path::Path) -> anyhow::Result<RunResult> {
            let content = fs::read_to_string(&self.sentinel).unwrap_or_default();
            let status = if content.trim() == "pass" { TestStatus::Passed } else { TestStatus::Failed };
            Ok(RunResult {
                results: vec![TestResult {
                    clause_id: self.clause_id.clone(),
                    status,
                    message: None,
                    duration: Duration::ZERO,
                    details: TestDetails { failure_message: None, stack_trace: None, iterations: None, measured_duration: None },
                }],
                total_duration: Duration::ZERO,
            })
        }
        fn is_available(&self) -> bool { true }
        fn name(&self) -> &str { "file-status" }
    }

    let base = std::env::temp_dir()
        .join(format!("ought_bisect_commit_info_{}", std::process::id()));
    let spec_dir = base.join("specs");
    let src_dir = base.join("src");
    fs::create_dir_all(&spec_dir).unwrap();
    fs::create_dir_all(&src_dir).unwrap();
    fs::write(
        spec_dir.join("auth.ought.md"),
        "# Auth\n\n## Login\n\n- **MUST** return 401 for invalid credentials\n",
    ).unwrap();

    for args in &[
        vec!["init"],
        vec!["config", "user.email", "alice@example.com"],
        vec!["config", "user.name", "Alice Developer"],
    ] {
        std::process::Command::new("git").args(args).current_dir(&base).output().unwrap();
    }

    let sentinel = src_dir.join("auth.rs");
    // Commit 1: passing state.
    fs::write(&sentinel, "pub fn status() -> u16 { 401 }\n").unwrap();
    fs::write(src_dir.join("status.txt"), "pass\n").unwrap();
    std::process::Command::new("git").args(["add", "."]).current_dir(&base).output().unwrap();
    std::process::Command::new("git")
        .args(["commit", "-m", "Initial: auth returns 401"])
        .current_dir(&base).output().unwrap();

    // Commit 2: breaking change — different author, distinct message, changes a file.
    for args in &[
        vec!["config", "user.email", "bob@example.com"],
        vec!["config", "user.name", "Bob Refactorer"],
    ] {
        std::process::Command::new("git").args(args).current_dir(&base).output().unwrap();
    }
    fs::write(&sentinel, "pub fn status() -> u16 { 200 }\n").unwrap();
    fs::write(src_dir.join("status.txt"), "fail\n").unwrap();
    std::process::Command::new("git").args(["add", "."]).current_dir(&base).output().unwrap();
    std::process::Command::new("git")
        .args(["commit", "-m", "refactor: simplify auth — always return 200"])
        .current_dir(&base).output().unwrap();

    let clause_id = ClauseId("auth::login::must_return_401_for_invalid_credentials".to_string());
    let runner = FileStatusRunner {
        sentinel: src_dir.join("status.txt"),
        clause_id: clause_id.clone(),
    };
    let specs = SpecGraph::from_roots(&[spec_dir.clone()]).expect("spec graph should parse");
    let options = BisectOptions { range: None, regenerate: false };

    let res = bisect(&clause_id, &specs, &runner, &options);
    assert!(res.is_ok(), "bisect must succeed; err: {:?}", res.err());
    let result = res.unwrap();
    let commit = &result.breaking_commit;

    assert!(
        !commit.message.is_empty(),
        "bisect must populate the breaking commit message; got empty string"
    );
    assert!(
        commit.message.contains("simplify auth") || commit.message.contains("200"),
        "breaking commit message must match the actual commit; got: {:?}",
        commit.message
    );
    assert!(
        !commit.author.is_empty(),
        "bisect must populate the breaking commit author; got empty string"
    );
    assert!(
        commit.author.contains("Bob") || commit.author.contains("bob@example.com"),
        "breaking commit author must match the committer; got: {:?}",
        commit.author
    );
    // date is a DateTime<Utc>; year must be plausible (not epoch zero).
    assert!(
        commit.date.timestamp() > 0,
        "bisect must populate a non-zero date for the breaking commit; got: {:?}",
        commit.date
    );
    assert!(
        !result.diff_summary.is_empty(),
        "bisect must populate diff_summary describing what changed; got empty string"
    );
    assert!(
        result.diff_summary.contains("auth.rs") || result.diff_summary.contains("status"),
        "diff summary must reference the changed file; got: {:?}",
        result.diff_summary
    );

    let _ = fs::remove_dir_all(&base);
}