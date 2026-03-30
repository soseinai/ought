/// MUST parse all spec files, execute generated tests, and report results
/// mapped back to clauses.
#[test]
fn test_cli__run__must_parse_all_spec_files_execute_generated_tests_and_report_resu() {
    let dir = unique_dir("parse_all");
    scaffold_project(&dir);
    write_spec(&dir, "MUST", "authenticate users");
    write_test(&dir, "spec__section__must_authenticate_users", true);

    let out = std::process::Command::new(ought_bin())
        .arg("run")
        .current_dir(&dir)
        .output()
        .expect("failed to invoke ought run");

    assert_eq!(
        out.status.code(),
        Some(0),
        "ought run must exit 0 when all tests pass; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    // Some output must be produced reporting on the results
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        !combined.trim().is_empty(),
        "ought run must produce output reporting test results"
    );

    let _ = std::fs::remove_dir_all(&dir);
}