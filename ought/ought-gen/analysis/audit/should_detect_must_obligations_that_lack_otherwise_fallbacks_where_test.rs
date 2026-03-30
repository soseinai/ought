/// SHOULD detect MUST obligations that lack OTHERWISE fallbacks where degradation is likely (e.g. network-dependent operations)
#[test]
fn test_analysis__audit__should_detect_must_obligations_that_lack_otherwise_fallbacks_where() {
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

    // Network-dependent MUST with no OTHERWISE — leaves the system in an undefined state on failure.
    let findings_json = r#"[
      {"kind":"Gap","description":"MUST fetch configuration from the remote server has no OTHERWISE fallback — network failure leaves system in undefined state","clauses":["config::remote::must_fetch_configuration_from_the_remote_server_on_startup"],"suggestion":"Add an OTHERWISE clause specifying fallback behavior when the remote server is unreachable","confidence":0.87}
    ]"#;

    let base =
        std::env::temp_dir().join(format!("ought_audit_otherwise_{}", std::process::id()));
    let spec_dir = base.join("specs");
    fs::create_dir_all(&spec_dir).unwrap();

    fs::write(
        spec_dir.join("config.ought.md"),
        "# Config\n\n## Remote\n\n- **MUST** fetch configuration from the remote server on startup\n",
    )
    .unwrap();

    let specs = SpecGraph::from_roots(&[spec_dir.clone()]).expect("spec graph should parse");
    let res = audit(&specs, &MockJsonGenerator { json: findings_json });
    assert!(res.is_ok(), "audit should succeed");

    let result = res.unwrap();
    let missing_otherwise = result.findings.iter().any(|f| {
        f.kind == AuditFindingKind::Gap
            && (f.description.contains("OTHERWISE")
                || f.description.contains("fallback")
                || f.description.contains("network"))
    });
    assert!(
        missing_otherwise,
        "audit should detect MUST obligations on network-dependent operations that lack OTHERWISE fallbacks; findings: {:?}",
        result.findings.iter().map(|f| &f.description).collect::<Vec<_>>()
    );

    let _ = fs::remove_dir_all(&base);
}