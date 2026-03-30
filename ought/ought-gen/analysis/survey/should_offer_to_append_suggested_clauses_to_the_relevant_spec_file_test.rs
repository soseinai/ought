/// SHOULD offer to append suggested clauses to the relevant spec file (or create a new one)
#[test]
fn test_analysis__survey__should_offer_to_append_suggested_clauses_to_the_relevant_spec_file() {
    struct MockJsonGenerator {
        json: String,
    }
    impl Generator for MockJsonGenerator {
        fn generate(&self, _: &Clause, _: &GenerationContext) -> anyhow::Result<GeneratedTest> {
            Ok(GeneratedTest {
                clause_id: ClauseId("survey::analysis".to_string()),
                code: self.json.clone(),
                language: Language::Rust,
                file_path: PathBuf::from("_survey.json"),
            })
        }
    }

    let base = std::env::temp_dir()
        .join(format!("ought_survey_appendoffer_{}", std::process::id()));
    let src_dir = base.join("src");
    let spec_dir = base.join("specs");
    fs::create_dir_all(&src_dir).unwrap();
    fs::create_dir_all(&spec_dir).unwrap();

    let src_file = src_dir.join("router.rs");
    fs::write(&src_file, "pub fn route(path: &str) {}\n").unwrap();
    fs::write(
        spec_dir.join("router.ought.md"),
        "# Router\n\n## Routing\n\n- **MUST** handle routes\n",
    )
    .unwrap();

    let target_spec = spec_dir.join("router.ought.md");
    let behaviors_json = format!(
        r#"[{{"file":"{f}","line":1,"description":"route function uncovered","suggested_clause":"MUST dispatch requests to the correct handler","suggested_keyword":"Must","suggested_spec":"{s}"}}]"#,
        f = src_file.display(),
        s = target_spec.display()
    );

    let specs = SpecGraph::from_roots(&[spec_dir.clone()]).expect("spec graph should parse");
    let res = survey(
        &specs,
        &[src_dir.clone()],
        &MockJsonGenerator { json: behaviors_json },
    );
    assert!(res.is_ok(), "survey must succeed");

    let result = res.unwrap();
    assert!(!result.uncovered.is_empty(), "must report uncovered behaviors");

    // Each uncovered behavior must carry a suggested_spec path so the user can be
    // offered the choice of appending clauses to that file.
    for b in &result.uncovered {
        assert!(
            b.suggested_spec != PathBuf::from(""),
            "every uncovered behavior must include a suggested_spec path for append-offer"
        );
    }

    let _ = fs::remove_dir_all(&base);
}