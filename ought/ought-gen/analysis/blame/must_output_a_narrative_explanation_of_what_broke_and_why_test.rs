/// MUST output a narrative explanation of what broke and why
#[test]
fn test_analysis__blame__must_output_a_narrative_explanation_of_what_broke_and_why() {
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

    let blame_json = r#"{"narrative":"The auth::login::must_return_401 test broke because the authentication handler was refactored to return HTTP 200 for all requests, removing the 401 error response for invalid credentials.","suggested_fix":null}"#;

    let clause_id = ClauseId("auth::login::must_return_401".to_string());
    let base = std::env::temp_dir()
        .join(format!("ought_blame_narrative_{}", std::process::id()));
    let spec_dir = base.join("specs");
    fs::create_dir_all(&spec_dir).unwrap();
    fs::write(
        spec_dir.join("auth.ought.md"),
        "# Auth\n\n## Login\n\n- **MUST** return 401 for invalid credentials\n",
    )
    .unwrap();

    let specs = SpecGraph::from_roots(&[spec_dir.clone()]).expect("spec graph should parse");
    let run_result = RunResult {
        results: vec![TestResult {
            clause_id: clause_id.clone(),
            status: TestStatus::Failed,
            message: Some("expected 401 got 200".to_string()),
            duration: Duration::ZERO,
            details: TestDetails {
                failure_message: Some("expected 401 got 200".to_string()),
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
        !result.narrative.is_empty(),
        "blame must output a non-empty narrative explanation of what broke and why"
    );

    let _ = fs::remove_dir_all(&base);
}