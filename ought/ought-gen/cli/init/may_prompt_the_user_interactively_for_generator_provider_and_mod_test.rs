/// MAY prompt the user interactively for generator provider and model preferences.
/// When stdin is not a terminal (piped from /dev/null), the command must not hang
/// indefinitely; it must terminate with a defined exit code.
#[test]
fn test_cli__init__may_prompt_the_user_interactively_for_generator_provider_and_mod() {
    let dir = std::env::temp_dir()
        .join(format!("ought_init_noninteractive_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();

    let bin = option_env!("CARGO_BIN_EXE_ought")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("ought"));

    let output = std::process::Command::new(&bin)
        .arg("init")
        .current_dir(&dir)
        // Provide a closed stdin to simulate a non-interactive environment.
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .expect("ought init must not hang — process must return");

    // The command MAY prompt interactively, but must not block indefinitely on
    // null stdin. Any definite exit code is acceptable (success or graceful error).
    assert!(
        output.status.code().is_some(),
        "process must terminate with an exit code and not be killed by a signal"
    );

    let _ = std::fs::remove_dir_all(&dir);
}