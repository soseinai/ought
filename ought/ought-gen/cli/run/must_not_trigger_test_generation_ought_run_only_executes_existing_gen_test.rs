/// MUST NOT trigger test generation — `ought run` only executes existing generated tests.
#[test]
fn test_cli__run__must_not_trigger_test_generation_ought_run_only_executes_existing_gen() {
    let dir = unique_dir("no_generate");
    scaffold_project(&dir);
    write_spec(&dir, "MUST", "return HTTP 200");
    write_test(&dir, "spec__section__must_return_http_200", true);

    // Snapshot the tests/ directory before running
    let before: std::collections::BTreeSet<_> = std::fs::read_dir(dir.join("tests"))
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name())
        .collect();

    // Run without any LLM API credentials; generation would fail loudly if attempted
    let out = std::process::Command::new(ought_bin())
        .arg("run")
        .current_dir(&dir)
        .env_remove("ANTHROPIC_API_KEY")
        .env_remove("OPENAI_API_KEY")
        .output()
        .expect("failed to invoke ought run");

    // Must not print an API-key complaint (which would only appear if generation ran)
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !stderr.to_lowercase().contains("api key"),
        "ought run must not attempt LLM generation; stderr contained an API key \
         reference: {stderr}"
    );

    // No new test files must have been created
    let after: std::collections::BTreeSet<_> = std::fs::read_dir(dir.join("tests"))
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name())
        .collect();
    assert_eq!(
        before, after,
        "ought run must not create new test files (generation must not be triggered)"
    );

    let _ = std::fs::remove_dir_all(&dir);
}