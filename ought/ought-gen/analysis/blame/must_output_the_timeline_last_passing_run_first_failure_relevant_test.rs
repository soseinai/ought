/// MUST output the timeline: last passing run, first failure, relevant commits
#[test]
fn test_analysis__blame__must_output_the_timeline_last_passing_run_first_failure_relevant() {
    struct MockBlameGenerator {
        json: &'static str,
    }
    impl Generator for MockBlameGenerator {
        fn generate(&self, _: &Clause, _: &GenerationContext) -> anyhow::Result<GeneratedTest> {
            Ok(GeneratedTest {
                clause_id: ClauseId("blame::analysis".to_string()),
                code: self.json.to_string(),
                language: Language::Rust,
                file_path: PathBuf::from("_blame.json"),
            })
        }
    }

    let blame_json = r#"{"narrative":"Timeline: last passed before breaking commit, first failed after.","suggested_fix":null,"likely_commit_hash":"abc123def","likely_commit_message":"refactor: auth responses","likely_commit_author":"Dev <dev@example.com>"}"#;

    let base = std::env::temp_dir()
        .join(format!("ought_blame_timeline_{}", std::process::id()));
    let spec_dir = base.join("specs");
    let src_dir = base.join("src");
    fs::create_dir_all(&spec_dir).unwrap();
    fs::create_dir_all(&src_dir).unwrap();

    std::process::Command::new("git")
        .args(["init"])
        .current_dir(&base)
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["config", "user.email", "test@example.com"])
        .current_dir(&base)
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["config", "user.name", "Test Runner"])
        .current_dir(&base)
        .output()
        .unwrap();

    fs::write(
        spec_dir.join("auth.ought.md"),
        "# Auth\n\n## Login\n\n- **MUST** return 401 for invalid credentials\n",
    )
    .unwrap();
    fs::write(src_dir.join("auth.rs"), "pub fn status() -> u16 { 401 }\n").unwrap();
    std::process::Command::new("git")
        .args(["add", "."])
        .current_dir(&base)
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["commit", "-m", "Initial: auth returns 401"])
        .current_dir(&base)
        .output()
        .unwrap();

    fs::write(src_dir.join("auth.rs"), "pub fn status() -> u16 { 200 }\n").unwrap();
    std::process::Command::new("git")
        .args(["add", "."])
        .current_dir(&base)
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["commit", "-m", "refactor: simplify auth responses"])
        .current_dir(&base)
        .output()
        .unwrap();

    let clause_id = ClauseId("auth::login::must_return_401_for_invalid_credentials".to_string());
    let specs = SpecGraph::from_roots(&[spec_dir.clone()]).expect("spec graph should parse");
    let run_result = RunResult {
        results: vec![TestResult {
            clause_id: clause_id.clone(),
            status: TestStatus::Failed,
            message: Some("expected 401, got 200".to_string()),
            duration: Duration::ZERO,
            details: TestDetails {
                failure_message: Some("expected 401, got 200".to_string()),
                stack_trace: None,
                iterations: None,
                measured_duration: None,
            },
        }],
        total_duration: Duration::ZERO,
    };

    let res = blame(&clause_id, &specs, &run_result, &MockBlameGenerator { json: blame_json });
    assert!(res.is_ok(), "blame must succeed; err: {:?}", res.err());
    let result = res.unwrap();

    assert!(
        result.last_passed.is_some(),
        "blame must output the last passing run timestamp"
    );
    assert!(
        result.first_failed.is_some(),
        "blame must output the first failure timestamp"
    );
    assert!(
        result.likely_commit.is_some(),
        "blame must output the relevant commits in the timeline"
    );

    let _ = fs::remove_dir_all(&base);
}