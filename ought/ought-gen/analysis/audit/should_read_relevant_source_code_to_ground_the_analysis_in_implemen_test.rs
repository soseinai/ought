/// SHOULD read relevant source code to ground the analysis in implementation reality
#[test]
fn test_analysis__audit__should_read_relevant_source_code_to_ground_the_analysis_in_implemen() {
    let source_files_seen: Arc<std::sync::Mutex<Vec<PathBuf>>> =
        Arc::new(std::sync::Mutex::new(Vec::new()));

    struct SpyGenerator {
        source_files_seen: Arc<std::sync::Mutex<Vec<PathBuf>>>,
    }
    impl Generator for SpyGenerator {
        fn generate(&self, _: &Clause, ctx: &GenerationContext) -> anyhow::Result<GeneratedTest> {
            let mut seen = self.source_files_seen.lock().unwrap();
            for sf in &ctx.source_files {
                seen.push(sf.path.clone());
            }
            Ok(GeneratedTest {
                clause_id: ClauseId("audit::analysis".to_string()),
                code: "[]".to_string(),
                language: Language::Rust,
                file_path: PathBuf::from("_audit.json"),
            })
        }
    }

    let base = std::env::temp_dir().join(format!("ought_audit_srccode_{}", std::process::id()));
    let src_dir = base.join("src");
    let spec_dir = base.join("specs");
    fs::create_dir_all(&src_dir).unwrap();
    fs::create_dir_all(&spec_dir).unwrap();

    let src_file = src_dir.join("auth.rs");
    fs::write(
        &src_file,
        "pub fn login(user: &str, pass: &str) -> bool { true }\n",
    )
    .unwrap();

    // Spec with an explicit source: directive so audit knows where to look.
    fs::write(
        spec_dir.join("auth.ought.md"),
        &format!(
            "# Auth\n\nsource: {}\n\n## Login\n\n- **MUST** authenticate the user\n",
            src_dir.display()
        ),
    )
    .unwrap();

    let specs = SpecGraph::from_roots(&[spec_dir.clone()]).expect("spec graph should parse");
    let gen = SpyGenerator { source_files_seen: Arc::clone(&source_files_seen) };
    let res = audit(&specs, &gen);
    assert!(res.is_ok(), "audit should succeed");

    let seen = source_files_seen.lock().unwrap();
    assert!(
        !seen.is_empty(),
        "audit should read source files and pass them to the LLM to ground findings in implementation reality"
    );

    let _ = fs::remove_dir_all(&base);
}