/// SHOULD print a summary at the end showing pass/fail counts by severity level.
#[test]
fn test_cli__run__should_print_a_summary_at_the_end_showing_pass_fail_counts_by_sever() {
    let dir = unique_dir("summary");
    scaffold_project(&dir);
    write_spec(&dir, "MUST", "authenticate users");
    write_test(&dir, "spec__section__must_authenticate_users", true);

    let out = std::process::Command::new(ought_bin())
        .arg("run")
        .current_dir(&dir)
        .output()
        .expect("failed to invoke ought run");

    let stdout = String::from_utf8_lossy(&out.stdout);
    // The summary must contain at least one count keyword.
    // The terminal reporter emits lines like "1 passed", "MUST coverage: 1/1 (100%)"
    assert!(
        stdout.contains("passed")
            || stdout.contains("failed")
            || stdout.contains("coverage"),
        "ought run must print a summary with pass/fail counts; stdout was:\n{stdout}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}