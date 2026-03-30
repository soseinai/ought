/// MUST use the LLM to identify public behaviors, APIs, and logic branches in the source
/// that lack corresponding clauses
#[test]
fn test_analysis__survey__must_use_the_llm_to_identify_public_behaviors_apis_and_logic_bran() {
    let called = Arc::new(AtomicBool::new(false));

    struct SpyGenerator {
        called: Arc<AtomicBool>,
    }
    impl Generator for SpyGenerator {
        fn generate(&self, _: &Clause, _: &GenerationContext) -> anyhow::Result<GeneratedTest> {
            self.called.store(true, Ordering::SeqCst);
            Ok(GeneratedTest {
                clause_id: ClauseId("survey::analysis".to_string()),
                code: "[]".to_string(),
                language: Language::Rust,
                file_path: PathBuf::from("_survey.json"),
            })
        }
    }

    let base = std::env::temp_dir()
        .join(format!("ought_survey_llm_{}", std::process::id()));
    let src_dir = base.join("src");
    let spec_dir = base.join("specs");
    fs::create_dir_all(&src_dir).unwrap();
    fs::create_dir_all(&spec_dir).unwrap();

    // Source file with public API — the LLM must be consulted to find gaps.
    fs::write(
        src_dir.join("lib.rs"),
        "pub fn authenticate(user: &str, pass: &str) -> bool { true }\n\
         pub fn logout(session: &str) {}\n\
         fn internal_helper() {}\n",
    )
    .unwrap();
    fs::write(spec_dir.join("auth.ought.md"), "# Auth\n\n## Login\n\n- **MUST** do something\n")
        .unwrap();

    let specs = SpecGraph::from_roots(&[spec_dir.clone()]).expect("spec graph should parse");

    let gen = SpyGenerator {
        called: Arc::clone(&called),
    };
    let res = survey(&specs, &[src_dir.clone()], &gen);
    assert!(res.is_ok(), "survey must succeed");
    assert!(
        called.load(Ordering::SeqCst),
        "survey must invoke the LLM generator to identify uncovered behaviors"
    );

    let _ = fs::remove_dir_all(&base);
}