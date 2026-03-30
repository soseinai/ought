/// MUST ALWAYS restore the working tree to its original state after completion (never leave on detached HEAD)
/// Temporal: MUST ALWAYS (invariant). Generate property-based or fuzz-style tests.
#[test]
fn test_analysis__bisect__must_always_restore_the_working_tree_to_its_original_state_after_complet() {
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

    // Run the invariant across multiple scenarios to approximate property-based testing.
    let scenarios: &[(&str, &[(&str, &str)])] = &[
        // (scenario_name, &[(commit_msg, status_content)])
        ("all-fail",       &[("passing state", "pass"), ("breaking change", "fail")]),
        ("last-fails",     &[("c1 pass", "pass"), ("c2 pass", "pass"), ("c3 fail", "fail"), ("c4 fail", "fail")]),
        ("middle-breaks",  &[("c1 pass", "pass"), ("c2 pass", "pass"), ("c3 fail", "fail")]),
        ("first-fails",    &[("c1 fail", "fail"), ("c2 fail", "fail")]),
        ("many-commits",   &[("c1", "pass"), ("c2", "pass"), ("c3", "pass"), ("c4", "pass"), ("c5", "fail"), ("c6", "fail"), ("c7", "fail"), ("c8", "fail")]),
    ];

    for (scenario_idx, (scenario_name, commits)) in scenarios.iter().enumerate() {
        let base = std::env::temp_dir()
            .join(format!("ought_bisect_invariant_{}_{}_{}", scenario_idx, scenario_name, std::process::id()));
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
        for (msg, status) in *commits {
            fs::write(&sentinel, format!("{status}\n")).unwrap();
            std::process::Command::new("git").args(["add", "."]).current_dir(&base).output().unwrap();
            std::process::Command::new("git")
                .args(["commit", "-m", msg])
                .current_dir(&base).output().unwrap();
        }

        // Record initial HEAD hash (the current branch tip).
        let initial_head_out = std::process::Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(&base)
            .output()
            .unwrap();
        let initial_head = String::from_utf8_lossy(&initial_head_out.stdout).trim().to_string();

        let clause_id = ClauseId("auth::login::must_return_401".to_string());
        let runner = FileStatusRunner { sentinel: sentinel.clone(), clause_id: clause_id.clone() };
        let specs = SpecGraph::from_roots(&[spec_dir.clone()]).expect("spec graph should parse");
        let options = BisectOptions { range: None, regenerate: false };

        // Run bisect (it may succeed or produce an error; both must restore the tree).
        let _ = bisect(&clause_id, &specs, &runner, &options);

        // Verify HEAD is restored to the original commit.
        let after_head_out = std::process::Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(&base)
            .output()
            .unwrap();
        let after_head = String::from_utf8_lossy(&after_head_out.stdout).trim().to_string();
        assert_eq!(
            initial_head, after_head,
            "INVARIANT VIOLATED in scenario '{scenario_name}': bisect left HEAD at {after_head} but must restore to {initial_head}"
        );

        // Verify we are NOT in detached HEAD state.
        let symbolic_out = std::process::Command::new("git")
            .args(["symbolic-ref", "--short", "HEAD"])
            .current_dir(&base)
            .output()
            .unwrap();
        assert!(
            symbolic_out.status.success(),
            "INVARIANT VIOLATED in scenario '{scenario_name}': bisect left working tree in detached HEAD state"
        );

        let _ = fs::remove_dir_all(&base);
    }
}