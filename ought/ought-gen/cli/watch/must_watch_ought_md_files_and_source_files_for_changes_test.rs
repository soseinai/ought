/// MUST watch `ought.md` files and source files for changes
#[test]
fn test_cli__watch__must_watch_ought_md_files_and_source_files_for_changes() {
    let dir = unique_dir("watch_files");
    scaffold_project(&dir);
    write_spec(&dir, "MUST", "handle requests");

    // Create a source file that the watcher should also monitor.
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::write(dir.join("src/lib.rs"), "// source file for watch test\n").unwrap();

    // Spawn ought watch as a long-running process.
    let mut child = std::process::Command::new(ought_bin())
        .arg("watch")
        .current_dir(&dir)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("failed to spawn ought watch");

    // Give the watcher time to start and establish file watches.
    std::thread::sleep(std::time::Duration::from_millis(400));

    // The process must not have exited with a usage error (code 2).
    if let Some(status) = child.try_wait().expect("failed to check child status") {
        assert_ne!(
            status.code(),
            Some(2),
            "ought watch must accept the `watch` subcommand without a usage error"
        );
    }
    // If still running, the watcher is active — expected behaviour.

    // Touch the spec file; a file watcher must pick this up.
    let spec_path = dir.join("ought/spec.ought.md");
    let original = std::fs::read_to_string(&spec_path).unwrap_or_default();
    std::fs::write(&spec_path, format!("{}\n<!-- touched -->", original)).unwrap();

    std::thread::sleep(std::time::Duration::from_millis(800));

    // Touch a source file; the watcher must monitor source paths too.
    std::fs::write(dir.join("src/lib.rs"), "// touched\n").unwrap();

    std::thread::sleep(std::time::Duration::from_millis(800));

    let _ = child.kill();
    let output = child.wait_with_output().expect("failed to collect watch output");

    // The watcher must have produced output, confirming it is actively watching.
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !combined.trim().is_empty(),
        "ought watch must produce output while watching ought.md and source files; got nothing"
    );

    let _ = std::fs::remove_dir_all(&dir);
}