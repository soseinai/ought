/// MUST use the LLM to correlate the source diff with the failure and produce a causal explanation
#[test]
fn test_analysis__blame__must_use_the_llm_to_correlate_the_source_diff_with_the_failure_an() {
    let llm_called = Arc::new(AtomicBool::new(false));

    struct SpyGenerator {
        called: Arc<AtomicBool>,
    }
    impl Generator for SpyGenerator {
        fn generate(&self, _: &Clause, _: &GenerationContext) -> anyhow::Result<GeneratedTest> {
            self.called.store(true, Ordering::SeqCst);
            Ok(GeneratedTest {
                clause_id: ClauseId("blame::analysis".to_string()),
                code: r#"{"narrative":"The diff shows the response code was changed from 401 to 200, directly causing the assertion failure.","suggested_fix":null}"#.to_string(),
                language: Language::Rust,
                file_path: PathBuf::from("_blame.json"),
            })
        }
    }

    let clause_id = ClauseId("auth::login::must_return_401".to_string());
    let base = std::env::temp_dir()
        .join(format!("ought_blame_llm_correlate_{}", std::process::id()));
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

    let gen = SpyGenerator { called: Arc::clone(&llm_called) };
    let res = blame(&clause_id, &specs, &run_result, &gen);
    assert!(res.is_ok(), "blame must succeed; err: {:?}", res.err());
    assert!(
        llm_called.load(Ordering::SeqCst),
        "blame must invoke the LLM to correlate the source diff with the failure and produce a causal explanation"
    );

    let _ = fs::remove_dir_all(&base);
}