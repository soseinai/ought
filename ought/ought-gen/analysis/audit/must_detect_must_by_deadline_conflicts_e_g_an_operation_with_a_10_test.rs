/// MUST detect MUST BY deadline conflicts (e.g. an operation with a 100ms deadline that calls a sub-operation with a 200ms deadline)
#[test]
fn test_analysis__audit__must_detect_must_by_deadline_conflicts_e_g_an_operation_with_a_10() {
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

    // Parent operation deadline (100ms) is tighter than sub-operation deadline (200ms) — impossible to satisfy both.
    let findings_json = r#"[
      {"kind":"Contradiction","description":"checkout MUST BY 100ms but calls payment MUST BY 200ms — sub-operation deadline exceeds parent deadline","clauses":["checkout::process::must_by_100ms_complete_the_checkout","payment::charge::must_by_200ms_charge_the_payment"],"suggestion":"Reduce the payment deadline below 100ms or increase the checkout deadline","confidence":0.95}
    ]"#;

    let base = std::env::temp_dir().join(format!("ought_audit_deadline_{}", std::process::id()));
    let spec_dir = base.join("specs");
    fs::create_dir_all(&spec_dir).unwrap();

    fs::write(
        spec_dir.join("checkout.ought.md"),
        "# Checkout\n\n## Process\n\n- **MUST BY** 100ms complete the checkout operation\n",
    )
    .unwrap();
    fs::write(
        spec_dir.join("payment.ought.md"),
        "# Payment\n\n## Charge\n\n- **MUST BY** 200ms charge the payment method\n",
    )
    .unwrap();

    let specs = SpecGraph::from_roots(&[spec_dir.clone()]).expect("spec graph should parse");
    let res = audit(&specs, &MockJsonGenerator { json: findings_json });
    assert!(res.is_ok(), "audit must succeed");

    let result = res.unwrap();
    let deadline_conflict = result.findings.iter().any(|f| {
        f.kind == AuditFindingKind::Contradiction
            && (f.description.contains("100ms")
                || f.description.contains("200ms")
                || f.description.contains("deadline"))
    });
    assert!(
        deadline_conflict,
        "audit must detect MUST BY deadline conflicts where a sub-operation deadline exceeds its parent; findings: {:?}",
        result.findings.iter().map(|f| &f.description).collect::<Vec<_>>()
    );

    let _ = fs::remove_dir_all(&base);
}