/// SHOULD rank uncovered behaviors by risk (public API > internal helper)
#[test]
fn test_analysis__survey__should_rank_uncovered_behaviors_by_risk_public_api_internal_helper() {
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
        .join(format!("ought_survey_ranking_{}", std::process::id()));
    let src_dir = base.join("src");
    let spec_dir = base.join("specs");
    fs::create_dir_all(&src_dir).unwrap();
    fs::create_dir_all(&spec_dir).unwrap();

    let src_file = src_dir.join("lib.rs");
    // Source has a public API function and a private helper.
    fs::write(
        &src_file,
        "pub fn public_process(data: &str) -> bool { private_validate(data) }\n\
         fn private_validate(data: &str) -> bool { !data.is_empty() }\n",
    )
    .unwrap();
    fs::write(spec_dir.join("svc.ought.md"), "# Svc\n\n## Core\n\n- **MUST** exist\n").unwrap();

    // The LLM returns the internal helper first (low risk) then the public API (high risk).
    // Survey must reorder: public API first.
    let behaviors_json = format!(
        r#"[
          {{"file":"{f}","line":2,"description":"private_validate (internal helper)","suggested_clause":"SHOULD validate non-empty data","suggested_keyword":"Should","suggested_spec":"ought/svc.ought.md","is_public":false}},
          {{"file":"{f}","line":1,"description":"public_process (public API)","suggested_clause":"MUST process data and return result","suggested_keyword":"Must","suggested_spec":"ought/svc.ought.md","is_public":true}}
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
    assert_eq!(result.uncovered.len(), 2, "both behaviors must be reported");

    // The public API behavior (line 1, public_process) must appear before the
    // internal helper (line 2, private_validate) regardless of the order the
    // LLM returned them.
    let first = &result.uncovered[0];
    let second = &result.uncovered[1];
    assert!(
        first.description.contains("public") || first.description.contains("public_process"),
        "public API behavior must rank first; got description: {:?}",
        first.description
    );
    assert!(
        second.description.contains("private") || second.description.contains("private_validate"),
        "internal helper must rank second; got description: {:?}",
        second.description
    );

    let _ = fs::remove_dir_all(&base);
}