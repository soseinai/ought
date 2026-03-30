/// SHOULD suggest a fix when the cause is clear
#[test]
fn test_analysis__blame__should_suggest_a_fix_when_the_cause_is_clear() {
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

    // LLM response includes a concrete fix because the cause is unambiguous.
    let blame_json = r#"{"narrative":"The test broke because the authentication handler was changed to return 200 instead of 401 for invalid credentials.","suggested_fix":"Restore the 401 status code in src/auth.rs line 42: change `StatusCode::OK` back to `StatusCode::UNAUTHORIZED`."}"#;

    let clause_id = ClauseId("auth::login::must_return_401".to_string());
    let base = std::env::temp_dir()
        .join(format!("ought_blame_suggest_fix_{}", std::process::id()));
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
        result.suggested_fix.is_some(),
        "blame should suggest a fix when the LLM determines the cause is clear; got None"
    );
    let fix = result.suggested_fix.unwrap();
    assert!(
        !fix.is_empty(),
        "suggested_fix must be a non-empty string describing the fix"
    );

    let _ = fs::remove_dir_all(&base);
}