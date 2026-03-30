/// MUST NOT require a running LLM if the clause has never passed (just report "never passed")
#[test]
fn test_analysis__blame__must_not_require_a_running_llm_if_the_clause_has_never_passed_just_re() {
    struct PanicGenerator;
    impl Generator for PanicGenerator {
        fn generate(&self, _: &Clause, _: &GenerationContext) -> anyhow::Result<GeneratedTest> {
            panic!("blame must NOT invoke the LLM when the clause has never passed");
        }
    }

    // No git repo — blame has no history, so this clause has never passed.
    let clause_id = ClauseId("auth::login::must_return_401".to_string());
    let base = std::env::temp_dir()
        .join(format!("ought_blame_never_passed_{}", std::process::id()));
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

    // PanicGenerator panics if called — blame must not invoke it when there is no passing history.
    let res = blame(&clause_id, &specs, &run_result, &PanicGenerator);
    assert!(
        res.is_ok(),
        "blame must return Ok even when the clause has never passed; err: {:?}",
        res.err()
    );
    let result = res.unwrap();
    assert!(
        result.last_passed.is_none(),
        "last_passed must be None when the clause has never passed"
    );
    assert!(
        result.narrative.to_lowercase().contains("never passed")
            || result.narrative.to_lowercase().contains("no passing"),
        "blame must report that the clause has never passed in the narrative; got: {:?}",
        result.narrative
    );

    let _ = fs::remove_dir_all(&base);
}