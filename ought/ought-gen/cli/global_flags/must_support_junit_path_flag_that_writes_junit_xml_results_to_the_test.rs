/// MUST support `--junit <path>` flag that writes JUnit XML results to the given file
#[test]
fn test_cli__global_flags__must_support_junit_path_flag_that_writes_junit_xml_results_to_the() {
    let dir = std::env::temp_dir()
        .join(format!("ought_junit_flag_{}", std::process::id()));
    let junit_path = dir.join("results.xml");
    std::fs::create_dir_all(&dir).unwrap();

    let bin = option_env!("CARGO_BIN_EXE_ought")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("ought"));

    // Initialise a minimal project.
    let init = std::process::Command::new(&bin)
        .arg("init")
        .current_dir(&dir)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .expect("ought init");
    assert!(
        init.status.success(),
        "init must succeed to set up the test project; stderr: {}",
        String::from_utf8_lossy(&init.stderr)
    );

    // Run with --junit; the file must be created at the given path.
    let out = std::process::Command::new(&bin)
        .args(["--junit", junit_path.to_str().unwrap(), "run"])
        .current_dir(&dir)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .expect("ought --junit run");

    assert_ne!(
        out.status.code(),
        Some(2),
        "--junit must not produce a usage/parse error; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        junit_path.exists(),
        "--junit must create the output file at the specified path ({})",
        junit_path.display()
    );

    let xml_content = std::fs::read_to_string(&junit_path)
        .expect("--junit output file must be readable");
    assert!(
        xml_content.trim().starts_with('<'),
        "--junit file must contain XML; first 200 chars: {}",
        &xml_content[..xml_content.len().min(200)]
    );

    let _ = std::fs::remove_dir_all(&dir);
}