/// MUST report the first commit where the clause fails
#[test]
fn test_analysis__bisect__must_report_the_first_commit_where_the_clause_fails() {
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
        .join(format!("ought_bisect_first_fail_{}", std::process::id()));
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
        vec!["config", "user.email", "test@example.com"],
        vec!["config", "user.name", "Test Runner"],
    ] {
        std::process::Command::new("git").args(args).current_dir(&base).output().unwrap();
    }

    let sentinel = src_dir.join("status.txt");
    // Commit 1: pass; commit 2: pass; commit 3 (breaking): fail; commit 4: fail.
    for (i, content) in [(1, "pass\n"), (2, "pass\n"), (3, "fail\n"), (4, "fail\n")] {
        fs::write(&sentinel, content).unwrap();
        std::process::Command::new("git").args(["add", "."]).current_dir(&base).output().unwrap();
        std::process::Command::new("git")
            .args(["commit", "-m", &format!("commit {i}")])
            .current_dir(&base).output().unwrap();
    }

    // Capture the hash of commit 3 (the expected breaking commit).
    let log_out = std::process::Command::new("git")
        .args(["log", "--format=%H %s", "--reverse"])
        .current_dir(&base)
        .output()
        .unwrap();
    let commits: Vec<(String, String)> = String::from_utf8_lossy(&log_out.stdout)
        .lines()
        .filter(|l| !l.is_empty())
        .map(|l| {
            let mut parts = l.splitn(2, ' ');
            (parts.next().unwrap_or("").to_string(), parts.next().unwrap_or("").to_string())
        })
        .collect();
    assert_eq!(commits.len(), 4, "expected 4 commits in test repo");
    let breaking_hash = &commits[2].0; // commit 3 (0-indexed: index 2)

    let clause_id = ClauseId("auth::login::must_return_401_for_invalid_credentials".to_string());
    let runner = FileStatusRunner { sentinel: sentinel.clone(), clause_id: clause_id.clone() };
    let specs = SpecGraph::from_roots(&[spec_dir.clone()]).expect("spec graph should parse");
    let options = BisectOptions { range: None, regenerate: false };

    let res = bisect(&clause_id, &specs, &runner, &options);
    assert!(res.is_ok(), "bisect must succeed; err: {:?}", res.err());
    let result = res.unwrap();

    assert_eq!(
        result.breaking_commit.hash,
        *breaking_hash,
        "bisect must report the FIRST failing commit (commit 3); got hash {}",
        result.breaking_commit.hash
    );

    let _ = fs::remove_dir_all(&base);
}