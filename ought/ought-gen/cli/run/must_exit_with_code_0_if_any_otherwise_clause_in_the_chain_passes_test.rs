/// MUST exit with code 0 if any OTHERWISE clause in the chain passes
/// (graceful degradation accepted).
/// GIVEN: a clause has an OTHERWISE chain and the primary obligation fails.
#[test]
fn test_cli__run__must_exit_with_code_0_if_any_otherwise_clause_in_the_chain_passes() {
    let dir = unique_dir("otherwise_pass");
    scaffold_project(&dir);
    // Spec: MUST with an OTHERWISE fallback
    std::fs::write(
        dir.join("ought/spec.ought.md"),
        "# Spec\n\ncontext: test\n\n## Section\n\n\
         - **MUST** respond in under 100ms\n  \
         - **OTHERWISE** respond in under 1s\n",
    )
    .unwrap();

    // Primary obligation fails
    write_test(
        &dir,
        "spec__section__must_respond_in_under_100ms",
        false,
    );
    // OTHERWISE fallback passes — the degradation chain is satisfied
    write_test(
        &dir,
        "spec__section__must_respond_in_under_100ms__otherwise_respond_in_under_1s",
        true,
    );

    let out = std::process::Command::new(ought_bin())
        .arg("run")
        .current_dir(&dir)
        .output()
        .expect("failed to invoke ought run");

    assert_eq!(
        out.status.code(),
        Some(0),
        "ought run must exit 0 when the primary MUST fails but an OTHERWISE \
         fallback passes (graceful degradation is accepted); \
         stderr: {} stdout: {}",
        String::from_utf8_lossy(&out.stderr),
        String::from_utf8_lossy(&out.stdout)
    );

    let _ = std::fs::remove_dir_all(&dir);
}