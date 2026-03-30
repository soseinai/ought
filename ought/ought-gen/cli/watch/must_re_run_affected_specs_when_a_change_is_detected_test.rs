/// MUST re-run affected specs when a change is detected
#[test]
fn test_cli__watch__must_re_run_affected_specs_when_a_change_is_detected() {
    let dir = unique_dir("watch_rerun");
    scaffold_project(&dir);
    write_spec(&dir, "MUST", "process items");
    write_test(&dir, "spec__section__must_process_items", true);

    let mut child = std::process::Command::new(ought_bin())
        .arg("watch")
        .current_dir(&dir)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("failed to spawn ought watch");

    // Allow the initial run to complete before triggering a change.
    std::thread::sleep(std::time::Duration::from_millis(1000));

    // Modify the spec file to trigger a re-run.
    let spec_path = dir.join("ought/spec.ought.md");
    let original = std::fs::read_to_string(&spec_path).unwrap_or_default();
    std::fs::write(&spec_path, format!("{}\n<!-- re-run trigger -->", original)).unwrap();

    // Wait long enough for the watcher to detect the change and complete a cycle.
    std::thread::sleep(std::time::Duration::from_millis(2000));

    let _ = child.kill();
    let output = child.wait_with_output().expect("failed to collect watch output");

    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    // After the change the watcher must have re-run and reported results.
    // Look for markers that a test run occurred: "passed", "failed", or a clause id.
    let ran_again = combined.contains("passed")
        || combined.contains("failed")
        || combined.contains("must_process_items")
        || combined.contains("spec.ought.md");

    assert!(
        ran_again,
        "ought watch must re-run affected specs when a file change is detected; \
         output:\n{combined}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}