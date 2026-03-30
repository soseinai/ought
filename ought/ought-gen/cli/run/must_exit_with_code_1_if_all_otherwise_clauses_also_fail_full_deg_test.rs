/// MUST exit with code 1 if all OTHERWISE clauses also fail
/// (full degradation chain exhausted).
/// GIVEN: a clause has an OTHERWISE chain and the primary obligation fails.
#[test]
fn test_cli__run__must_exit_with_code_1_if_all_otherwise_clauses_also_fail_full_deg() {
    let dir = unique_dir("otherwise_all_fail");
    scaffold_project(&dir);
    // Spec: MUST with an OTHERWISE fallback
    std::fs::write(
        dir.join("ought/spec.ought.md"),
        "# Spec\n\ncontext: test\n\n## Section\n\n\
         - **MUST** respond in under 100ms\n  \
         - **OTHERWISE** respond in under 1s\n",
    )
    .unwrap();

    // Both the primary and the OTHERWISE fallback fail — full chain exhausted
    write_test(
        &dir,
        "spec__section__must_respond_in_under_100ms",
        false,
    );
    write_test(
        &dir,
        "spec__section__must_respond_in_under_100ms__otherwise_respond_in_under_1s",
        false,
    );

    let out = std::process::Command::new(ought_bin())
        .arg("run")
        .current_dir(&dir)
        .output()
        .expect("failed to invoke ought run");

    assert_eq!(
        out.status.code(),
        Some(1),
        "ought run must exit 1 when the primary MUST and every OTHERWISE fallback \
         all fail (degradation chain exhausted); stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let _ = std::fs::remove_dir_all(&dir);
}