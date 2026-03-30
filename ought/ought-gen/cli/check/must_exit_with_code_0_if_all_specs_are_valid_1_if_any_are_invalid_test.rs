/// MUST exit with code 0 if all specs are valid, 1 if any are invalid
#[test]
fn test_cli__check__must_exit_with_code_0_if_all_specs_are_valid_1_if_any_are_invalid() {
    let base = std::env::temp_dir()
        .join(format!("ought_check_exit_{}", std::process::id()));
    let valid_dir = base.join("valid");
    let invalid_dir = base.join("invalid");
    std::fs::create_dir_all(&valid_dir).unwrap();
    std::fs::create_dir_all(&invalid_dir).unwrap();

    // ── exit 0: all specs are valid ───────────────────────────────────────────
    std::fs::write(
        valid_dir.join("svc.ought.md"),
        "# Service\n\n## Auth\n\n- **MUST** authenticate every request\n",
    )
    .unwrap();

    let valid_result = ought_spec::SpecGraph::from_roots(&[valid_dir.clone()]);
    assert!(
        valid_result.is_ok(),
        "valid specs must not produce errors (ought check would exit 0); errors: {:?}",
        valid_result.err()
    );

    // ── exit 1: at least one spec is invalid (circular dependency) ────────────
    // Two specs that require each other form a cycle, which is a hard error.
    std::fs::write(
        invalid_dir.join("a.ought.md"),
        "# Spec A\n\nrequires: [B](./b.ought.md)\n\n## Section\n\n- **MUST** depend on B\n",
    )
    .unwrap();
    std::fs::write(
        invalid_dir.join("b.ought.md"),
        "# Spec B\n\nrequires: [A](./a.ought.md)\n\n## Section\n\n- **MUST** depend on A\n",
    )
    .unwrap();

    let invalid_result = ought_spec::SpecGraph::from_roots(&[invalid_dir.clone()]);
    assert!(
        invalid_result.is_err(),
        "specs with circular dependencies must produce errors (ought check would exit 1)"
    );
    let errors = invalid_result.unwrap_err();
    assert!(
        !errors.is_empty(),
        "ought check must report at least one error when specs are invalid"
    );
    let any_cycle_msg = errors
        .iter()
        .any(|e| e.message.contains("circular") || e.message.contains("cycle"));
    assert!(
        any_cycle_msg,
        "error must describe the circular dependency; got: {:?}",
        errors
            .iter()
            .map(|e| e.to_string())
            .collect::<Vec<_>>()
    );

    let _ = std::fs::remove_dir_all(&base);
}