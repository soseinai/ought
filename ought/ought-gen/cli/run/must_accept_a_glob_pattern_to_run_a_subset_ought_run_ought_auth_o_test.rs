/// MUST accept a glob pattern to run a subset:
/// `ought run "ought/auth*.ought.md"`
#[test]
fn test_cli__run__must_accept_a_glob_pattern_to_run_a_subset_ought_run_ought_auth_o() {
    let dir = unique_dir("glob_arg");
    scaffold_project(&dir);
    std::fs::write(
        dir.join("ought/auth.ought.md"),
        "# Auth\n\ncontext: test\n\n## Login\n\n- **MUST** authenticate users\n",
    )
    .unwrap();
    write_test(&dir, "auth__login__must_authenticate_users", true);

    let out = std::process::Command::new(ought_bin())
        .arg("run")
        .arg("ought/auth*.ought.md")
        .current_dir(&dir)
        .output()
        .expect("failed to invoke ought run with glob pattern");

    // The glob pattern is a string positional argument; must not produce a usage error.
    assert_ne!(
        out.status.code(),
        Some(2),
        "ought run must accept a glob pattern without a usage error; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let _ = std::fs::remove_dir_all(&dir);
}