/// MUST exit with code 0 if only SHOULD or MAY clauses fail.
#[test]
fn test_cli__run__must_exit_with_code_0_if_only_should_or_may_clauses_fail() {
    let dir = unique_dir("exit0_should");
    scaffold_project(&dir);
    // Spec has only a SHOULD-level clause, no MUST
    write_spec(&dir, "SHOULD", "log access attempts");
    // Test for the SHOULD clause fails — this must not trigger exit 1
    write_test(&dir, "spec__section__should_log_access_attempts", false);

    let out = std::process::Command::new(ought_bin())
        .arg("run")
        .current_dir(&dir)
        .output()
        .expect("failed to invoke ought run");

    assert_eq!(
        out.status.code(),
        Some(0),
        "ought run must exit 0 when only SHOULD-level tests fail \
         (SHOULD failures must not count as hard failures); \
         stderr: {} stdout: {}",
        String::from_utf8_lossy(&out.stderr),
        String::from_utf8_lossy(&out.stdout)
    );

    let _ = std::fs::remove_dir_all(&dir);
}