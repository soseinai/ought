/// SHOULD support `--range <from>..<to>` to limit the search space
#[test]
fn test_analysis__bisect__should_support_range_from_to_to_limit_the_search_space() {
    use std::sync::{Arc, Mutex};

    struct TrackingRunner {
        clause_id: ClauseId,
        sentinel: PathBuf,
        visited: Arc<Mutex<Vec<String>>>,
    }
    impl Runner for TrackingRunner {
        fn run(&self, _: &[GeneratedTest], _: &std::path::Path) -> anyhow::Result<RunResult> {
            // Record visited commit hash.
            if let Ok(out) = std::process::Command::new("git")
                .args(["rev-parse", "HEAD"])
                .current_dir(self.sentinel.parent().unwrap())
                .output()
            {
                let hash = String::from_utf8_lossy(&out.stdout).trim().to_string();
                self.visited.lock().unwrap().push(hash);
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
        fn name(&self) -> &str { "tracking" }
    }

    let base = std::env::temp_dir()
        .join(format!("ought_bisect_range_{}", std::process::id()));
    let spec_dir = base.join("specs");
    let src_dir = base.join("src");
    fs::create_dir_all(&spec_dir).unwrap();
    fs::create_dir_all(&src_dir).unwrap();
    fs::write(
        spec_dir.join("auth.ought.md"),
        "# Auth\n\n## Login\n\n- **MUST** return 401 for invalid credentials\n",
    ).unwrap();

    for git_args in &[
        vec!["init"],
        vec!["config", "user.email", "test@example.com"],
        vec!["config", "user.name", "Test Runner"],
    ] {
        std::process::Command::new("git").args(git_args).current_dir(&base).output().unwrap();
    }

    // 8 commits: commits 1-4 pass, commits 5-8 fail.
    let sentinel = src_dir.join("status.txt");
    for i in 1..=8usize {
        let status = if i <= 4 { "pass" } else { "fail" };
        fs::write(&sentinel, format!("{status}\n")).unwrap();
        std::process::Command::new("git").args(["add", "."]).current_dir(&base).output().unwrap();
        std::process::Command::new("git")
            .args(["commit", "-m", &format!("commit {i}")])
            .current_dir(&base).output().unwrap();
    }

    // Collect all commit hashes in chronological order.
    let log_out = std::process::Command::new("git")
        .args(["log", "--format=%H", "--reverse"])
        .current_dir(&base)
        .output()
        .unwrap();
    let all_hashes: Vec<String> = String::from_utf8_lossy(&log_out.stdout)
        .lines()
        .filter(|l| !l.is_empty())
        .map(|l| l.trim().to_string())
        .collect();
    assert_eq!(all_hashes.len(), 8);

    // Restrict the search to commits 3..8 (hashes[2]..hashes[7]).
    // The range excludes the "from" end, so commits 4-8 are searched.
    let range = format!("{}..{}", all_hashes[2], all_hashes[7]);
    let out_of_range: Vec<&str> = all_hashes[..2].iter().map(|s| s.as_str()).collect();

    let visited = Arc::new(Mutex::new(Vec::<String>::new()));
    let clause_id = ClauseId("auth::login::must_return_401_for_invalid_credentials".to_string());
    let runner = TrackingRunner { clause_id: clause_id.clone(), sentinel: sentinel.clone(), visited: Arc::clone(&visited) };
    let specs = SpecGraph::from_roots(&[spec_dir.clone()]).expect("spec graph should parse");
    let options = BisectOptions { range: Some(range), regenerate: false };

    let res = bisect(&clause_id, &specs, &runner, &options);
    assert!(res.is_ok(), "bisect with --range must succeed; err: {:?}", res.err());

    let seen = visited.lock().unwrap();
    for hash in &out_of_range {
        assert!(
            !seen.contains(&hash.to_string()),
            "bisect with --range must not visit commits outside the range; unexpectedly visited {hash}"
        );
    }

    let _ = fs::remove_dir_all(&base);
}