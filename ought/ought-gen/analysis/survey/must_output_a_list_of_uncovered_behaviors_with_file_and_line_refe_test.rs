/// MUST output a list of uncovered behaviors with file and line references
#[test]
fn test_analysis__survey__must_output_a_list_of_uncovered_behaviors_with_file_and_line_refe() {
    // MockJsonGenerator simulates the LLM returning a list of behaviors as JSON.
    // Survey is responsible for parsing this response into UncoveredBehavior entries.
    struct MockJsonGenerator {
        json: &'static str,
    }
    impl Generator for MockJsonGenerator {
        fn generate(&self, _: &Clause, _: &GenerationContext) -> anyhow::Result<GeneratedTest> {
            Ok(GeneratedTest {
                clause_id: ClauseId("survey::analysis".to_string()),
                code: self.json.to_string(),
                language: Language::Rust,
                file_path: PathBuf::from("_survey.json"),
            })
        }
    }

    let base = std::env::temp_dir()
        .join(format!("ought_survey_fileline_{}", std::process::id()));
    let src_dir = base.join("src");
    let spec_dir = base.join("specs");
    fs::create_dir_all(&src_dir).unwrap();
    fs::create_dir_all(&spec_dir).unwrap();

    let src_file = src_dir.join("api.rs");
    fs::write(&src_file, "pub fn create_user(name: &str) {}\npub fn delete_user(id: u64) {}\n")
        .unwrap();
    fs::write(spec_dir.join("api.ought.md"), "# API\n\n## Users\n\n- **MUST** handle something\n")
        .unwrap();

    let behaviors_json = format!(
        r#"[
          {{"file":"{f}","line":1,"description":"create_user has no clause","suggested_clause":"MUST create a user with the given name","suggested_keyword":"Must","suggested_spec":"ought/api.ought.md"}},
          {{"file":"{f}","line":2,"description":"delete_user has no clause","suggested_clause":"MUST delete the user with the given id","suggested_keyword":"Must","suggested_spec":"ought/api.ought.md"}}
        ]"#,
        f = src_file.display()
    );

    let gen = MockJsonGenerator {
        json: Box::leak(behaviors_json.into_boxed_str()),
    };

    let specs = SpecGraph::from_roots(&[spec_dir.clone()]).expect("spec graph should parse");
    let res = survey(&specs, &[src_dir.clone()], &gen);
    assert!(res.is_ok(), "survey must succeed");

    let result = res.unwrap();
    assert!(
        !result.uncovered.is_empty(),
        "survey must output at least one uncovered behavior"
    );
    for b in &result.uncovered {
        assert!(
            b.file != PathBuf::from(""),
            "each uncovered behavior must include a non-empty file path"
        );
        assert!(
            b.line > 0,
            "each uncovered behavior must include a positive line reference; got line {}",
            b.line
        );
    }

    let _ = fs::remove_dir_all(&base);
}