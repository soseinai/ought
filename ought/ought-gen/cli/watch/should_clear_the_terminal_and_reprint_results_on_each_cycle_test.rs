/// SHOULD clear the terminal and reprint results on each cycle
#[test]
fn test_cli__watch__should_clear_the_terminal_and_reprint_results_on_each_cycle() {
    let dir = unique_dir("watch_clear");
    scaffold_project(&dir);
    write_spec(&dir, "MUST", "return results");
    write_test(&dir, "spec__section__must_return_results", true);

    let mut child = std::process::Command::new(ought_bin())
        .arg("watch")
        .current_dir(&dir)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("failed to spawn ought watch");

    // Allow the first cycle to complete.
    std::thread::sleep(std::time::Duration::from_millis(1000));

    // Trigger a second cycle by modifying the spec.
    let spec_path = dir.join("ought/spec.ought.md");
    let original = std::fs::read_to_string(&spec_path).unwrap_or_default();
    std::fs::write(&spec_path, format!("{}\n<!-- cycle 2 -->", original)).unwrap();

    std::thread::sleep(std::time::Duration::from_millis(1500));

    let _ = child.kill();
    let output = child.wait_with_output().expect("failed to collect watch output");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{stdout}{stderr}");

    // A terminal clear is signalled by the ANSI escape sequence ESC[2J (erase
    // display) or the common ESC[H ESC[2J (move-home + erase) pattern.
    // Either form satisfies the "clear the terminal" requirement.
    let has_clear = combined.contains("\x1b[2J")
        || combined.contains("\x1b[H")
        || combined.contains("\x1bc"); // full terminal reset

    assert!(
        has_clear,
        "ought watch should emit an ANSI clear-screen sequence (ESC[2J or equivalent) \
         before reprinting results on each cycle; stdout was:\n{stdout}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}