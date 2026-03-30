/// MAY assign a confidence score to each finding
#[test]
fn test_analysis__audit__may_assign_a_confidence_score_to_each_finding() {
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

    // LLM returns findings with explicit confidence scores in [0.0, 1.0].
    let findings_json = r#"[
      {"kind":"Contradiction","description":"Conflicting auth obligations","clauses":["auth::login::must_a","auth::login::must_b"],"suggestion":null,"confidence":0.92},
      {"kind":"Gap","description":"Missing rate-limit clause","clauses":["api::ratelimit::must_throttle"],"suggestion":null,"confidence":0.65}
    ]"#;

    let base =
        std::env::temp_dir().join(format!("ought_audit_confidence_{}", std::process::id()));
    let spec_dir = base.join("specs");
    fs::create_dir_all(&spec_dir).unwrap();
    fs::write(
        spec_dir.join("auth.ought.md"),
        "# Auth\n\n## Login\n\n- **MUST** validate credentials\n",
    )
    .unwrap();

    let specs = SpecGraph::from_roots(&[spec_dir.clone()]).expect("spec graph should parse");
    let res = audit(&specs, &MockJsonGenerator { json: findings_json });
    assert!(res.is_ok(), "audit may succeed with confidence scores present");

    let result = res.unwrap();
    // MAY means confidence is optional — only validate range when it is present.
    for finding in &result.findings {
        if let Some(confidence) = finding.confidence {
            assert!(
                (0.0..=1.0).contains(&confidence),
                "confidence score must be in the range [0.0, 1.0] when assigned; got {} for finding: {:?}",
                confidence,
                finding.description
            );
        }
        // Absence of confidence (None) is also valid — MAY is permissive.
    }

    let _ = fs::remove_dir_all(&base);
}