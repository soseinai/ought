/// MUST use the LLM to identify contradictions between clauses (across files or within)
#[test]
fn test_analysis__audit__must_use_the_llm_to_identify_contradictions_between_clauses_acros() {
    let called = Arc::new(AtomicBool::new(false));

    struct SpyGenerator {
        called: Arc<AtomicBool>,
    }
    impl Generator for SpyGenerator {
        fn generate(&self, _: &Clause, _: &GenerationContext) -> anyhow::Result<GeneratedTest> {
            self.called.store(true, Ordering::SeqCst);
            Ok(GeneratedTest {
                clause_id: ClauseId("audit::analysis".to_string()),
                code: "[]".to_string(),
                language: Language::Rust,
                file_path: PathBuf::from("_audit.json"),
            })
        }
    }

    let base =
        std::env::temp_dir().join(format!("ought_audit_llm_contra_{}", std::process::id()));
    let spec_dir = base.join("specs");
    fs::create_dir_all(&spec_dir).unwrap();

    // Two clauses that could contradict each other — the LLM must be consulted to detect it.
    fs::write(
        spec_dir.join("api.ought.md"),
        "# API\n\n## Response\n\n\
         - **MUST** return HTTP 200 on all successful requests\n\
         - **MUST** return HTTP 201 when a resource is created\n",
    )
    .unwrap();

    let specs = SpecGraph::from_roots(&[spec_dir.clone()]).expect("spec graph should parse");
    let gen = SpyGenerator { called: Arc::clone(&called) };
    let res = audit(&specs, &gen);
    assert!(res.is_ok(), "audit must succeed");
    assert!(
        called.load(Ordering::SeqCst),
        "audit must invoke the LLM generator to identify contradictions between clauses"
    );

    let _ = fs::remove_dir_all(&base);
}