/// SHOULD save progress so `ought bisect --continue` can resume
/// GIVEN: the bisect is interrupted (SIGINT, crash)
#[test]
fn test_analysis__bisect__should_save_progress_so_ought_bisect_continue_can_resume() {
    use std::sync::{Arc, Mutex};

    // Runner that fails after visiting a configurable number of commits, leaving bisect
    // mid-search so a progress file would be needed to resume.
    struct InterruptingRunner {
        clause_id: ClauseId,
        sentinel: PathBuf,
        calls_before_crash: usize,
        call_count: Arc<Mutex<usize>>,
    }
    impl Runner for InterruptingRunner {
        fn run(&self, _: &[GeneratedTest], _: &std::path::Path) -> anyhow::Result<RunResult> {
            let mut count = self.call_count.lock().unwrap();
            *count += 1;
            if *count >= self.calls_before_crash {
                anyhow::bail!("simulated SIGINT after {} calls", count);
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
        fn name(&self) -> &str { "interrupting" }
    }

    let base = std::env::temp_dir()
        .join(format!("ought_bisect_save_progress_{}", std::process::id()));
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
    for i in 1..=8usize {
        let status = if i <= 4 { "pass" } else { "fail" };
        fs::write(&sentinel, format!("{status}\n")).unwrap();
        std::process::Command::new("git").args(["add", "."]).current_dir(&base).output().unwrap();
        std::process::Command::new("git")
            .args(["commit", "-m", &format!("commit {i}")])
            .current_dir(&base).output().unwrap();
    }

    let call_count = Arc::new(Mutex::new(0usize));
    let clause_id = ClauseId("auth::login::must_return_401_for_invalid_credentials".to_string());
    let runner = InterruptingRunner {
        clause_id: clause_id.clone(),
        sentinel: sentinel.clone(),
        calls_before_crash: 2, // crash after 2 commits have been tested
        call_count: Arc::clone(&call_count),
    };
    let specs = SpecGraph::from_roots(&[spec_dir.clone()]).expect("spec graph should parse");
    let options = BisectOptions { range: None, regenerate: false };

    // Bisect is interrupted mid-run.
    let res = bisect(&clause_id, &specs, &runner, &options);
    assert!(res.is_err(), "bisect must return Err when interrupted; got Ok");

    // After interruption, a state/progress file must exist so `--continue` can resume.
    // Acceptable file names: .ought-bisect, ought-bisect-state.toml, etc.
    let progress_files = [
        base.join(".ought-bisect"),
        base.join("ought-bisect-state.toml"),
        base.join(".ought").join("bisect-state.toml"),
        base.join("ought-gen").join("bisect-state.toml"),
    ];
    let state_file_exists = progress_files.iter().any(|p| p.exists());
    assert!(
        state_file_exists,
        "GIVEN an interrupted bisect, a progress file must be saved so `ought bisect --continue` \
         can resume; checked paths: {:?}",
        progress_files
    );

    // The state file must reference the clause being bisected.
    let state_content = progress_files.iter()
        .find(|p| p.exists())
        .and_then(|p| fs::read_to_string(p).ok())
        .unwrap_or_default();
    assert!(
        state_content.contains("auth::login::must_return_401_for_invalid_credentials"),
        "bisect progress file must record the clause identifier for resumption; got: {state_content:?}"
    );

    let _ = fs::remove_dir_all(&base);
}