/// SHOULD debounce rapid file changes (at least 500ms)
#[test]
fn test_cli__watch__should_debounce_rapid_file_changes_at_least_500ms() {
    let dir = unique_dir("watch_debounce");
    scaffold_project(&dir);
    write_spec(&dir, "MUST", "validate input");
    write_test(&dir, "spec__section__must_validate_input", true);

    let mut child = std::process::Command::new(ought_bin())
        .arg("watch")
        .current_dir(&dir)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("failed to spawn ought watch");

    // Let the watcher initialise.
    std::thread::sleep(std::time::Duration::from_millis(800));

    // Fire five rapid spec-file changes within ~100 ms — well inside the 500 ms
    // debounce window.  A correct implementation collapses these into one run.
    let spec_path = dir.join("ought/spec.ought.md");
    for i in 0..5u8 {
        let base = std::fs::read_to_string(&spec_path).unwrap_or_default();
        std::fs::write(&spec_path, format!("{}\n<!-- burst {} -->", base, i)).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(50));
    }

    // Wait long enough for exactly one debounced cycle to finish.
    std::thread::sleep(std::time::Duration::from_millis(1500));

    let _ = child.kill();
    let output = child.wait_with_output().expect("failed to collect watch output");

    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    // Count how many times a "running" / cycle-start marker appears.
    // A debounced watcher fires at most once for the burst.
    // We accept 1 or 2 cycles (initial + debounced); we must not see 5.
    let run_count = combined.matches("passed").count()
        + combined.matches("failed").count()
        + combined.matches("running").count();

    assert!(
        run_count <= 2,
        "ought watch should debounce rapid changes into at most 1 re-run cycle \
         (got {} apparent run markers); debounce window must be ≥500 ms\noutput:\n{combined}",
        run_count
    );

    let _ = std::fs::remove_dir_all(&dir);
}