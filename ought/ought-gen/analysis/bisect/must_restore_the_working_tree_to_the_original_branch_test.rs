/// MUST restore the working tree to the original branch
/// GIVEN: the bisect is interrupted (SIGINT, crash)
#[test]
fn test_analysis__bisect__must_restore_the_working_tree_to_the_original_branch() {
    // Simulate a bisect that crashes mid-run (runner returns Err on the first call).
    struct CrashingRunner {
        clause_id: ClauseId,
        sentinel: PathBuf,
        calls: std::cell::Cell<usize>,
    }
    impl Runner for CrashingRunner {
        fn run(&self, _: &[GeneratedTest], _: &std::path::Path) -> anyhow::Result<RunResult> {
            let n = self.calls.get();
            self.calls.set(n + 1);
            if n == 0 {
                // First call simulates a crash/interruption.
                anyhow::bail!("simulated crash during bisect (SIGINT equivalent)");
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
        fn name(&self) -> &str { "crashing" }
    }

    let base = std::env::temp_dir()
        .join(format!("ought_bisect_interrupt_restore_{}", std::process::id()));
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

    let sentinel = src_dir.join("status.txt");
    for (i, status) in [(1, "pass"), (2, "pass"), (3, "fail"), (4, "fail")] {
        fs::write(&sentinel, format!("{status}\n")).unwrap();
        std::process::Command::new("git").args(["add", "."]).current_dir(&base).output().unwrap();
        std::process::Command::new("git")
            .args(["commit", "-m", &format!("commit {i}")])
            .current_dir(&base).output().unwrap();
    }

    // Capture starting branch/HEAD.
    let initial_head_out = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(&base)
        .output()
        .unwrap();
    let initial_head = String::from_utf8_lossy(&initial_head_out.stdout).trim().to_string();
    let initial_branch_out = std::process::Command::new("git")
        .args(["symbolic-ref", "--short", "HEAD"])
        .current_dir(&base)
        .output()
        .unwrap();
    let initial_branch = String::from_utf8_lossy(&initial_branch_out.stdout).trim().to_string();

    let clause_id = ClauseId("auth::login::must_return_401_for_invalid_credentials".to_string());
    let runner = CrashingRunner { clause_id: clause_id.clone(), sentinel: sentinel.clone(), calls: std::cell::Cell::new(0) };
    let specs = SpecGraph::from_roots(&[spec_dir.clone()]).expect("spec graph should parse");
    let options = BisectOptions { range: None, regenerate: false };

    // Bisect should return Err due to the simulated crash.
    let res = bisect(&clause_id, &specs, &runner, &options);
    assert!(
        res.is_err(),
        "bisect must propagate the runner error; expected Err but got Ok"
    );

    // Even after the crash, the working tree must be on the original branch.
    let after_branch_out = std::process::Command::new("git")
        .args(["symbolic-ref", "--short", "HEAD"])
        .current_dir(&base)
        .output()
        .unwrap();
    let after_branch = String::from_utf8_lossy(&after_branch_out.stdout).trim().to_string();
    assert_eq!(
        initial_branch, after_branch,
        "GIVEN a crash during bisect, the working tree must be restored to the original branch \
         (was: {initial_branch}); got: {after_branch}"
    );

    let after_head_out = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(&base)
        .output()
        .unwrap();
    let after_head = String::from_utf8_lossy(&after_head_out.stdout).trim().to_string();
    assert_eq!(
        initial_head, after_head,
        "GIVEN a crash during bisect, HEAD must be restored to {initial_head}; got {after_head}"
    );

    let _ = fs::remove_dir_all(&base);
}