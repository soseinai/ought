/// MUST reference the specific clauses involved in each finding (file, section, line)
#[test]
fn test_analysis__audit__must_reference_the_specific_clauses_involved_in_each_finding_file() {
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

    let findings_json = r#"[
      {"kind":"Contradiction","description":"Conflicting HTTP status obligations","clauses":["api::responses::must_return_200_on_all_successful_requests","api::responses::must_return_201_when_a_resource_is_created"],"suggestion":null,"confidence":null}
    ]"#;

    let base = std::env::temp_dir().join(format!("ought_audit_refs_{}", std::process::id()));
    let spec_dir = base.join("specs");
    fs::create_dir_all(&spec_dir).unwrap();
    fs::write(
        spec_dir.join("api.ought.md"),
        "# API\n\n## Responses\n\n\
         - **MUST** return 200 on all successful requests\n\
         - **MUST** return 201 when a resource is created\n",
    )
    .unwrap();

    let specs = SpecGraph::from_roots(&[spec_dir.clone()]).expect("spec graph should parse");
    let res = audit(&specs, &MockJsonGenerator { json: findings_json });
    assert!(res.is_ok(), "audit must succeed");

    let result = res.unwrap();
    assert!(!result.findings.is_empty(), "must report at least one finding");
    for finding in &result.findings {
        assert!(
            !finding.clauses.is_empty(),
            "each finding must reference at least one specific clause; finding: {:?}",
            finding.description
        );
        for clause_id in &finding.clauses {
            assert!(
                !clause_id.0.is_empty(),
                "each referenced clause ID must be non-empty"
            );
        }
    }

    let _ = fs::remove_dir_all(&base);
}