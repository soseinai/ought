/// SHOULD identify the specific commit and file change most likely responsible
#[test]
fn test_analysis__blame__should_identify_the_specific_commit_and_file_change_most_likely_res() {
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

    // LLM identifies a specific commit and the file that changed.
    let blame_json = r#"{"narrative":"Commit abc123def456 is most likely responsible. The change to src/auth.rs line 42 removed the 401 status code.","suggested_fix":null,"likely_commit_hash":"abc123def456","likely_commit_message":"refactor: simplify auth error responses","likely_commit_author":"Jane Developer <jane@example.com>","likely_commit_file":"src/auth.rs"}"#;

    let base = std::env::temp_dir()
        .join(format!("ought_blame_commit_id_{}", std::process::id()));
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
        .args(["config", "user.email", "jane@example.com"])
        .current_dir(&base)
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["config", "user.name", "Jane Developer"])
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
        .args(["commit", "-m", "Initial passing state"])
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
        .args(["commit", "-m", "refactor: simplify auth error responses"])
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
        result.likely_commit.is_some(),
        "blame should identify the specific commit most likely responsible for the failure"
    );
    let commit = result.likely_commit.unwrap();
    assert!(
        !commit.hash.is_empty(),
        "blame should populate the commit hash of the likely-responsible change; got empty hash"
    );

    let _ = fs::remove_dir_all(&base);
}