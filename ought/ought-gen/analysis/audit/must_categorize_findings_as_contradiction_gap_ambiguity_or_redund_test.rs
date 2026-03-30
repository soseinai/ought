/// MUST categorize findings as: contradiction, gap, ambiguity, or redundancy
#[test]
fn test_analysis__audit__must_categorize_findings_as_contradiction_gap_ambiguity_or_redund() {
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

    // LLM returns one finding of each category.
    let findings_json = r#"[
      {"kind":"Contradiction","description":"Two clauses cannot both hold","clauses":["auth::login::must_a","auth::login::must_b"],"suggestion":null,"confidence":null},
      {"kind":"Gap","description":"Missing fallback clause","clauses":["auth::login::must_a"],"suggestion":null,"confidence":null},
      {"kind":"Ambiguity","description":"Clause text is unclear about timing","clauses":["auth::login::must_c"],"suggestion":null,"confidence":null},
      {"kind":"Redundancy","description":"Two clauses express the same obligation","clauses":["auth::login::must_a","auth::login::must_d"],"suggestion":null,"confidence":null}
    ]"#;

    let base =
        std::env::temp_dir().join(format!("ought_audit_categories_{}", std::process::id()));
    let spec_dir = base.join("specs");
    fs::create_dir_all(&spec_dir).unwrap();
    fs::write(
        spec_dir.join("auth.ought.md"),
        "# Auth\n\n## Login\n\n- **MUST** validate credentials\n",
    )
    .unwrap();

    let specs = SpecGraph::from_roots(&[spec_dir.clone()]).expect("spec graph should parse");
    let res = audit(&specs, &MockJsonGenerator { json: findings_json });
    assert!(res.is_ok(), "audit must succeed");

    let result = res.unwrap();
    let kinds: Vec<AuditFindingKind> = result.findings.iter().map(|f| f.kind).collect();

    assert!(
        kinds.contains(&AuditFindingKind::Contradiction),
        "audit must categorize Contradiction findings; got kinds: {kinds:?}"
    );
    assert!(
        kinds.contains(&AuditFindingKind::Gap),
        "audit must categorize Gap findings; got kinds: {kinds:?}"
    );
    assert!(
        kinds.contains(&AuditFindingKind::Ambiguity),
        "audit must categorize Ambiguity findings; got kinds: {kinds:?}"
    );
    assert!(
        kinds.contains(&AuditFindingKind::Redundancy),
        "audit must categorize Redundancy findings; got kinds: {kinds:?}"
    );

    let _ = fs::remove_dir_all(&base);
}