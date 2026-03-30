/// MUST read all spec files and their cross-references
#[test]
fn test_analysis__audit__must_read_all_spec_files_and_their_cross_references() {
    let captured: Arc<std::sync::Mutex<Vec<String>>> = Arc::new(std::sync::Mutex::new(Vec::new()));

    struct SpyGenerator {
        captured: Arc<std::sync::Mutex<Vec<String>>>,
    }
    impl Generator for SpyGenerator {
        fn generate(&self, _: &Clause, ctx: &GenerationContext) -> anyhow::Result<GeneratedTest> {
            if let Some(spec_ctx) = &ctx.spec_context {
                self.captured.lock().unwrap().push(spec_ctx.clone());
            }
            Ok(GeneratedTest {
                clause_id: ClauseId("audit::analysis".to_string()),
                code: "[]".to_string(),
                language: Language::Rust,
                file_path: PathBuf::from("_audit.json"),
            })
        }
    }

    let base = std::env::temp_dir().join(format!("ought_audit_allspecs_{}", std::process::id()));
    let spec_dir = base.join("specs");
    fs::create_dir_all(&spec_dir).unwrap();

    fs::write(
        spec_dir.join("auth.ought.md"),
        "# Auth\n\n## Login\n\n- **MUST** validate user credentials before granting access\n",
    )
    .unwrap();
    fs::write(
        spec_dir.join("billing.ought.md"),
        "# Billing\n\n## Payments\n\n- **MUST** charge the correct invoice amount\n",
    )
    .unwrap();

    let specs = SpecGraph::from_roots(&[spec_dir.clone()]).expect("spec graph should parse");
    assert_eq!(specs.specs().len(), 2, "both spec files must be loaded");

    let gen = SpyGenerator { captured: Arc::clone(&captured) };
    let res = audit(&specs, &gen);
    assert!(res.is_ok(), "audit must succeed");

    let contexts = captured.lock().unwrap();
    assert!(
        !contexts.is_empty(),
        "audit must invoke the LLM with spec content as context"
    );
    let all_context = contexts.join("\n");
    assert!(
        all_context.contains("validate user credentials") || all_context.contains("auth"),
        "audit context must include content from auth.ought.md; got: {all_context:?}"
    );
    assert!(
        all_context.contains("correct invoice amount") || all_context.contains("billing"),
        "audit context must include content from billing.ought.md; got: {all_context:?}"
    );

    let _ = fs::remove_dir_all(&base);
}