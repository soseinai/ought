/// SHOULD detect GIVEN blocks with overlapping conditions that impose contradictory obligations
#[test]
fn test_analysis__audit__should_detect_given_blocks_with_overlapping_conditions_that_impose() {
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

    // Two GIVEN conditions that overlap when a user is both authenticated and a guest (ambiguous role),
    // imposing contradictory MUST / MUST NOT obligations on write access.
    let findings_json = r#"[
      {"kind":"Contradiction","description":"GIVEN user is authenticated (MUST allow write) overlaps with GIVEN user is a guest (MUST NOT allow write) — contradictory obligations when role is ambiguous","clauses":["api::access::given_authenticated_must_allow_full_read_write","api::access::given_guest_must_not_allow_write"],"suggestion":"Make GIVEN conditions mutually exclusive or add explicit precedence rules","confidence":0.80}
    ]"#;

    let base =
        std::env::temp_dir().join(format!("ought_audit_given_overlap_{}", std::process::id()));
    let spec_dir = base.join("specs");
    fs::create_dir_all(&spec_dir).unwrap();

    fs::write(
        spec_dir.join("api.ought.md"),
        "# API\n\n## Access\n\n\
         - **GIVEN** user is authenticated: **MUST** allow full read-write access\n\
         - **GIVEN** user is a guest: **MUST NOT** allow write access\n",
    )
    .unwrap();

    let specs = SpecGraph::from_roots(&[spec_dir.clone()]).expect("spec graph should parse");
    let res = audit(&specs, &MockJsonGenerator { json: findings_json });
    assert!(res.is_ok(), "audit should succeed");

    let result = res.unwrap();
    let given_conflict = result.findings.iter().any(|f| {
        (f.kind == AuditFindingKind::Contradiction || f.kind == AuditFindingKind::Ambiguity)
            && (f.description.contains("GIVEN")
                || f.description.contains("overlap")
                || f.description.contains("condition"))
    });
    assert!(
        given_conflict,
        "audit should detect GIVEN blocks with overlapping conditions imposing contradictory obligations; findings: {:?}",
        result.findings.iter().map(|f| &f.description).collect::<Vec<_>>()
    );

    let _ = fs::remove_dir_all(&base);
}