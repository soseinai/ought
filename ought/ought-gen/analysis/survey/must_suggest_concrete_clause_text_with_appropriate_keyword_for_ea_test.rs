/// MUST suggest concrete clause text (with appropriate keyword) for each uncovered behavior
#[test]
fn test_analysis__survey__must_suggest_concrete_clause_text_with_appropriate_keyword_for_ea() {
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
        .join(format!("ought_survey_clause_{}", std::process::id()));
    let src_dir = base.join("src");
    let spec_dir = base.join("specs");
    fs::create_dir_all(&src_dir).unwrap();
    fs::create_dir_all(&spec_dir).unwrap();

    let src_file = src_dir.join("service.rs");
    fs::write(&src_file, "pub fn process_payment(amount: u64) -> bool { true }\n").unwrap();
    fs::write(spec_dir.join("svc.ought.md"), "# Svc\n\n## Core\n\n- **MUST** do something\n")
        .unwrap();

    let behaviors_json = format!(
        r#"[
          {{"file":"{f}","line":1,"description":"process_payment uncovered","suggested_clause":"MUST process the payment and return success status","suggested_keyword":"Must","suggested_spec":"ought/svc.ought.md"}},
          {{"file":"{f}","line":1,"description":"payment error path","suggested_clause":"SHOULD return false when payment fails","suggested_keyword":"Should","suggested_spec":"ought/svc.ought.md"}}
        ]"#,
        f = src_file.display()
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

    // Appropriate deontic keywords for suggested clauses.
    const BEHAVIORAL_KEYWORDS: &[Keyword] = &[
        Keyword::Must,
        Keyword::MustNot,
        Keyword::Should,
        Keyword::ShouldNot,
        Keyword::May,
        Keyword::Wont,
        Keyword::MustAlways,
        Keyword::MustBy,
    ];

    for b in &result.uncovered {
        assert!(
            !b.suggested_clause.is_empty(),
            "suggested_clause must be non-empty for every uncovered behavior"
        );
        assert!(
            BEHAVIORAL_KEYWORDS.contains(&b.suggested_keyword),
            "suggested_keyword {:?} must be a deontic behavioral keyword, not structural",
            b.suggested_keyword
        );
    }

    let _ = fs::remove_dir_all(&base);
}