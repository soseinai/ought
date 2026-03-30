/// MUST use the LLM to identify gaps — areas where related clauses exist but expected companion clauses are missing
#[test]
fn test_analysis__audit__must_use_the_llm_to_identify_gaps_areas_where_related_clauses_exi() {
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

    let base = std::env::temp_dir().join(format!("ought_audit_gaps_{}", std::process::id()));
    let spec_dir = base.join("specs");
    fs::create_dir_all(&spec_dir).unwrap();

    // Spec with a create clause but no corresponding delete — a classic companion-clause gap.
    fs::write(
        spec_dir.join("api.ought.md"),
        "# API\n\n## Users\n\n- **MUST** create a user account\n",
    )
    .unwrap();

    let findings_json = r#"[
      {"kind":"Gap","description":"MUST create user has no corresponding MUST delete user clause","clauses":["api::users::must_create_a_user_account"],"suggestion":"Add a MUST delete user clause to the API spec","confidence":0.85}
    ]"#;

    let specs = SpecGraph::from_roots(&[spec_dir.clone()]).expect("spec graph should parse");
    let res = audit(&specs, &MockJsonGenerator { json: findings_json });
    assert!(res.is_ok(), "audit must succeed");

    let result = res.unwrap();
    assert!(
        result.findings.iter().any(|f| f.kind == AuditFindingKind::Gap),
        "audit must identify gap findings where expected companion clauses are missing"
    );

    let _ = fs::remove_dir_all(&base);
}