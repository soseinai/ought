/// MUST accept a clause identifier (e.g. `auth::login::must_return_401`)
#[test]
fn test_analysis__blame__must_accept_a_clause_identifier_e_g_auth_login_must_return_401() {
    struct StubGenerator;
    impl Generator for StubGenerator {
        fn generate(&self, _: &Clause, _: &GenerationContext) -> anyhow::Result<GeneratedTest> {
            Ok(GeneratedTest {
                clause_id: ClauseId("blame::analysis".to_string()),
                code: r#"{"narrative":"stub"}"#.to_string(),
                language: Language::Rust,
                file_path: PathBuf::from("_blame.json"),
            })
        }
    }

    let clause_id = ClauseId("auth::login::must_return_401".to_string());
    let base = std::env::temp_dir()
        .join(format!("ought_blame_accept_id_{}", std::process::id()));
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

    let res = blame(&clause_id, &specs, &run_result, &StubGenerator);
    assert!(
        res.is_ok(),
        "blame must accept a ClauseId and return Ok; err: {:?}",
        res.err()
    );
    assert_eq!(
        res.unwrap().clause_id,
        clause_id,
        "blame result must carry the same clause_id that was passed in"
    );

    let _ = fs::remove_dir_all(&base);
}