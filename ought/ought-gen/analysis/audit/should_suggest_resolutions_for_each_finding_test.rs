/// SHOULD suggest resolutions for each finding
#[test]
fn test_analysis__audit__should_suggest_resolutions_for_each_finding() {
    struct MockJsonGenerator {
        json: &'static str,
    }
    impl Generator for MockJsonGenerator {
        fn generate(&self, _: &Clause, _: &GenerationContext) -> anyhow::Result<GeneratedTest> {
            Ok(GeneratedTest {
                clause_id: ClauseId("audit::analysis".to_string()),
                code: self.json.to_string(),
                language: Language::Rust,
                file_path: PathBuf::from("_audit.json"),
            })
        }
    }

    // LLM returns findings that each carry a non-empty resolution suggestion.
    let findings_json = r#"[
      {"kind":"Contradiction","description":"Conflicting HTTP status codes for success responses","clauses":["api::responses::must_return_200","api::responses::must_return_201"],"suggestion":"Use 200 for reads and 201 specifically for resource creation to eliminate the ambiguity","confidence":null},
      {"kind":"Gap","description":"No clause specifying behavior when the database is unavailable","clauses":["data::storage::must_persist_records"],"suggestion":"Add an OTHERWISE clause describing the fallback when the database is unreachable","confidence":null}
    ]"#;

    let base = std::env::temp_dir().join(format!("ought_audit_suggest_{}", std::process::id()));
    let spec_dir = base.join("specs");
    fs::create_dir_all(&spec_dir).unwrap();
    fs::write(
        spec_dir.join("api.ought.md"),
        "# API\n\n## Responses\n\n- **MUST** return a success status\n",
    )
    .unwrap();

    let specs = SpecGraph::from_roots(&[spec_dir.clone()]).expect("spec graph should parse");
    let res = audit(&specs, &MockJsonGenerator { json: findings_json });
    assert!(res.is_ok(), "audit should succeed");

    let result = res.unwrap();
    assert!(!result.findings.is_empty(), "must have at least one finding");
    for finding in &result.findings {
        assert!(
            finding.suggestion.as_deref().map(|s| !s.is_empty()).unwrap_or(false),
            "audit should include a non-empty resolution suggestion for each finding; finding: {:?}",
            finding.description
        );
    }

    let _ = fs::remove_dir_all(&base);
}