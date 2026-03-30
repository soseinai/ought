/// MUST accept a path argument to run a specific spec file:
/// `ought run ought/auth.ought.md`
#[test]
fn test_cli__run__must_accept_a_path_argument_to_run_a_specific_spec_file_ought_run() {
    let dir = unique_dir("path_arg");
    scaffold_project(&dir);
    std::fs::write(
        dir.join("ought/auth.ought.md"),
        "# Auth\n\ncontext: test\n\n## Login\n\n- **MUST** authenticate users\n",
    )
    .unwrap();
    write_test(&dir, "auth__login__must_authenticate_users", true);

    let out = std::process::Command::new(ought_bin())
        .arg("run")
        .arg("ought/auth.ought.md")
        .current_dir(&dir)
        .output()
        .expect("failed to invoke ought run with path argument");

    // clap exits 2 for unrecognised arguments; a path is a positional value and
    // must be accepted without triggering a usage error.
    assert_ne!(
        out.status.code(),
        Some(2),
        "ought run must accept a path argument without a usage error; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let _ = std::fs::remove_dir_all(&dir);
}