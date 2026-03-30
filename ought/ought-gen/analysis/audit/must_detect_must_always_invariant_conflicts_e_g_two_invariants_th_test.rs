/// MUST detect MUST ALWAYS invariant conflicts (e.g. two invariants that cannot simultaneously hold)
#[test]
fn test_analysis__audit__must_detect_must_always_invariant_conflicts_e_g_two_invariants_th() {
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
      {"kind":"Contradiction","description":"MUST ALWAYS maintain exactly one active session conflicts with MUST ALWAYS allow multiple concurrent sessions — both cannot hold simultaneously","clauses":["auth::session::must_always_maintain_exactly_one_active_session","auth::session::must_always_support_multiple_concurrent_sessions"],"suggestion":"Reconcile session invariants by choosing a single-session or multi-session model","confidence":0.98}
    ]"#;

    let base = std::env::temp_dir().join(format!("ought_audit_invariant_{}", std::process::id()));
    let spec_dir = base.join("specs");
    fs::create_dir_all(&spec_dir).unwrap();

    // Two MUST ALWAYS invariants that logically cannot both be true.
    fs::write(
        spec_dir.join("auth.ought.md"),
        "# Auth\n\n## Session\n\n\
         - **MUST ALWAYS** maintain exactly one active session per user\n\
         - **MUST ALWAYS** support multiple concurrent sessions per user\n",
    )
    .unwrap();

    let specs = SpecGraph::from_roots(&[spec_dir.clone()]).expect("spec graph should parse");
    let res = audit(&specs, &MockJsonGenerator { json: findings_json });
    assert!(res.is_ok(), "audit must succeed");

    let result = res.unwrap();
    let invariant_conflict = result.findings.iter().any(|f| {
        f.kind == AuditFindingKind::Contradiction
            && (f.description.contains("MUST ALWAYS")
                || f.description.contains("invariant")
                || f.description.contains("session"))
    });
    assert!(
        invariant_conflict,
        "audit must detect MUST ALWAYS invariant conflicts that cannot simultaneously hold; findings: {:?}",
        result.findings.iter().map(|f| &f.description).collect::<Vec<_>>()
    );

    let _ = fs::remove_dir_all(&base);
}