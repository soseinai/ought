/// SHOULD cache test results per commit to avoid redundant runs
#[test]
fn test_analysis__bisect__should_cache_test_results_per_commit_to_avoid_redundant_runs() {
    use std::sync::{Arc, Mutex};

    struct CountingRunner {
        clause_id: ClauseId,
        call_count: Arc<Mutex<usize>>,
        sentinel: PathBuf,
    }
    impl Runner for CountingRunner {
        fn run(&self, _: &[GeneratedTest], _: &std::path::Path) -> anyhow::Result<RunResult> {
            *self.call_count.lock().unwrap() += 1;
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
        fn name(&self) -> &str { "counting" }
    }

    let base = std::env::temp_dir()
        .join(format!("ought_bisect_cache_{}", std::process::id()));
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

    // 16 commits: first 8 pass, last 8 fail — max binary-search steps = log2(16) = 4.
    let sentinel = src_dir.join("status.txt");
    for i in 1..=16usize {
        let status = if i <= 8 { "pass" } else { "fail" };
        fs::write(&sentinel, format!("{status}\n")).unwrap();
        std::process::Command::new("git").args(["add", "."]).current_dir(&base).output().unwrap();
        std::process::Command::new("git")
            .args(["commit", "-m", &format!("commit {i}")])
            .current_dir(&base).output().unwrap();
    }

    let call_count = Arc::new(Mutex::new(0usize));
    let clause_id = ClauseId("auth::login::must_return_401_for_invalid_credentials".to_string());
    let runner = CountingRunner { clause_id: clause_id.clone(), call_count: Arc::clone(&call_count), sentinel };
    let specs = SpecGraph::from_roots(&[spec_dir.clone()]).expect("spec graph should parse");
    let options = BisectOptions { range: None, regenerate: false };

    let res = bisect(&clause_id, &specs, &runner, &options);
    assert!(res.is_ok(), "bisect must succeed; err: {:?}", res.err());

    let calls = *call_count.lock().unwrap();
    // A binary search on 16 commits needs at most log2(16)+1 = 5 runner calls.
    // Caching ensures the same commit is never run twice, keeping it at this bound.
    assert!(
        calls <= 5,
        "bisect should cache results per commit; expected at most 5 runner calls for 16 commits, got {calls}"
    );

    let _ = fs::remove_dir_all(&base);
}