/// SHOULD support `--fail-on-should` flag to also fail on SHOULD clause failures.
#[test]
fn test_cli__run__should_support_fail_on_should_flag_to_also_fail_on_should_clause_fa() {
    let dir = unique_dir("fail_on_should");
    scaffold_project(&dir);
    // Only a SHOULD clause — would normally be a soft failure
    write_spec(&dir, "SHOULD", "emit telemetry events");
    write_test(&dir, "spec__section__should_emit_telemetry_events", false);

    let out = std::process::Command::new(ought_bin())
        .arg("run")
        .arg("--fail-on-should")
        .current_dir(&dir)
        .output()
        .expect("failed to invoke ought run --fail-on-should");

    // Flag must be recognised by clap (not a usage error)
    assert_ne!(
        out.status.code(),
        Some(2),
        "--fail-on-should must be a recognised flag; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    // With the flag set, a failing SHOULD test must cause exit 1
    assert_eq!(
        out.status.code(),
        Some(1),
        "ought run --fail-on-should must exit 1 when a SHOULD-level test fails; \
         stderr: {} stdout: {}",
        String::from_utf8_lossy(&out.stderr),
        String::from_utf8_lossy(&out.stdout)
    );

    let _ = std::fs::remove_dir_all(&dir);
}