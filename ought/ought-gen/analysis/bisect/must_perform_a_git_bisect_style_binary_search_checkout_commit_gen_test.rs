/// MUST perform a git-bisect-style binary search: checkout commit, generate test for clause, run it, narrow range
#[test]
fn test_analysis__bisect__must_perform_a_git_bisect_style_binary_search_checkout_commit_gen() {
    use std::sync::{Arc, Mutex};

    // Runner reads a sentinel file from the working tree and reports pass/fail accordingly.
    struct FileStatusRunner {
        sentinel: PathBuf,
        clause_id: ClauseId,
        visited_commits: Arc<Mutex<Vec<String>>>,
    }
    impl Runner for FileStatusRunner {
        fn run(&self, _: &[GeneratedTest], _: &std::path::Path) -> anyhow::Result<RunResult> {
            // Record the HEAD at each call so we can verify multiple commits were visited.
            if let Ok(out) = std::process::Command::new("git")
                .args(["rev-parse", "HEAD"])
                .current_dir(self.sentinel.parent().unwrap())
                .output()
            {
                let hash = String::from_utf8_lossy(&out.stdout).trim().to_string();
                self.visited_commits.lock().unwrap().push(hash);
            }
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
        .join(format!("ought_bisect_bsearch_{}", std::process::id()));
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

    // Commits 1-4: passing; commits 5-8: failing. Breaking point is commit 5.
    let sentinel = src_dir.join("status.txt");
    for i in 1..=8usize {
        let content = if i < 5 { "pass\n" } else { "fail\n" };
        fs::write(&sentinel, content).unwrap();
        std::process::Command::new("git").args(["add", "."]).current_dir(&base).output().unwrap();
        std::process::Command::new("git")
            .args(["commit", "-m", &format!("commit {i}")])
            .current_dir(&base).output().unwrap();
    }

    let visited = Arc::new(Mutex::new(Vec::<String>::new()));
    let runner = FileStatusRunner {
        sentinel: sentinel.clone(),
        clause_id: ClauseId("auth::login::must_return_401_for_invalid_credentials".to_string()),
        visited_commits: Arc::clone(&visited),
    };

    let clause_id = ClauseId("auth::login::must_return_401_for_invalid_credentials".to_string());
    let specs = SpecGraph::from_roots(&[spec_dir.clone()]).expect("spec graph should parse");
    let options = BisectOptions { range: None, regenerate: false };

    let res = bisect(&clause_id, &specs, &runner, &options);
    assert!(res.is_ok(), "bisect must succeed; err: {:?}", res.err());

    // A binary search over 8 commits must visit strictly fewer than 8 commits.
    let call_count = visited.lock().unwrap().len();
    assert!(
        call_count > 0 && call_count < 8,
        "bisect must perform a binary search (visited {call_count} commits; expected 1..7 for 8-commit history)"
    );
    // Must identify the correct first-failing commit (commit 5 in the sequence).
    let result = res.unwrap();
    assert!(
        result.breaking_commit.message.contains("commit 5"),
        "bisect must narrow to commit 5 as the first failing commit; got: {:?}",
        result.breaking_commit.message
    );

    let _ = fs::remove_dir_all(&base);
}