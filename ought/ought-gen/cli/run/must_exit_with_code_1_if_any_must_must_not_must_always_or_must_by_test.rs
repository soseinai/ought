/// MUST exit with code 1 if any MUST, MUST NOT, MUST ALWAYS, or MUST BY clause fails.
#[test]
fn test_cli__run__must_exit_with_code_1_if_any_must_must_not_must_always_or_must_by() {
    let dir = unique_dir("exit1_must");
    scaffold_project(&dir);
    write_spec(&dir, "MUST", "authenticate users");
    // Deliberately failing test for the MUST clause
    write_test(&dir, "spec__section__must_authenticate_users", false);

    let out = std::process::Command::new(ought_bin())
        .arg("run")
        .current_dir(&dir)
        .output()
        .expect("failed to invoke ought run");

    assert_eq!(
        out.status.code(),
        Some(1),
        "ought run must exit 1 when a MUST clause test fails; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let _ = std::fs::remove_dir_all(&dir);
}