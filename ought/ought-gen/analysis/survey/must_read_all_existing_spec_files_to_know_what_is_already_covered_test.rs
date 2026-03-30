/// MUST read all existing spec files to know what is already covered
#[test]
fn test_analysis__survey__must_read_all_existing_spec_files_to_know_what_is_already_covered() {
    // The spy captures the spec_context text passed to the LLM so we can verify
    // that clause coverage from ALL spec files was included.
    let captured: Arc<std::sync::Mutex<Vec<String>>> =
        Arc::new(std::sync::Mutex::new(Vec::new()));

    struct SpyGenerator {
        captured: Arc<std::sync::Mutex<Vec<String>>>,
    }
    impl Generator for SpyGenerator {
        fn generate(&self, _: &Clause, ctx: &GenerationContext) -> anyhow::Result<GeneratedTest> {
            if let Some(spec_ctx) = &ctx.spec_context {
                self.captured.lock().unwrap().push(spec_ctx.clone());
            }
            // Return an empty behavior list — the test is only checking context, not output.
            Ok(GeneratedTest {
                clause_id: ClauseId("survey::analysis".to_string()),
                code: "[]".to_string(),
                language: Language::Rust,
                file_path: PathBuf::from("_survey.json"),
            })
        }
    }

    let base = std::env::temp_dir()
        .join(format!("ought_survey_allspecs_{}", std::process::id()));
    let src_dir = base.join("src");
    let spec_dir = base.join("specs");
    fs::create_dir_all(&src_dir).unwrap();
    fs::create_dir_all(&spec_dir).unwrap();

    fs::write(src_dir.join("lib.rs"), "pub fn process() {}\n").unwrap();

    // Two spec files with distinct clauses: auth and billing coverage.
    fs::write(
        spec_dir.join("auth.ought.md"),
        "# Auth\n\n## Login\n\n- **MUST** validate credentials\n",
    )
    .unwrap();
    fs::write(
        spec_dir.join("billing.ought.md"),
        "# Billing\n\n## Payments\n\n- **MUST** charge correct amount\n",
    )
    .unwrap();

    let specs = SpecGraph::from_roots(&[spec_dir.clone()]).expect("spec graph should parse");
    assert_eq!(specs.specs().len(), 2, "both spec files must be loaded");

    let gen = SpyGenerator {
        captured: Arc::clone(&captured),
    };
    let res = survey(&specs, &[src_dir.clone()], &gen);
    assert!(res.is_ok(), "survey must succeed");

    // The generator must have been called with context that includes coverage from
    // BOTH spec files, proving survey read all existing spec files.
    let contexts = captured.lock().unwrap();
    assert!(
        !contexts.is_empty(),
        "survey must invoke the LLM with existing clause coverage as context"
    );
    let all_context = contexts.join("\n");
    assert!(
        all_context.contains("validate credentials") || all_context.contains("auth"),
        "context must reflect auth spec coverage; got: {all_context:?}"
    );
    assert!(
        all_context.contains("charge correct amount") || all_context.contains("billing"),
        "context must reflect billing spec coverage; got: {all_context:?}"
    );

    let _ = fs::remove_dir_all(&base);
}