//! Built-in runner presets.
//!
//! A preset is a canned [`RunnerConfig`] for a popular language+harness combo
//! so users can write `[runner.python]` with nothing else and ought still
//! knows how to spawn pytest and parse its output.
//!
//! Every preset emits JUnit XML because it's the one format essentially every
//! mainstream harness already knows how to write. Users wanting a different
//! harness for the same language can still set `preset = "python"` and
//! override `command` / `format` as needed.

use std::collections::HashMap;

use crate::config::{OutputFormat, RunnerConfig};

/// Return the built-in preset for `name`, if any.
pub fn preset(name: &str) -> Option<RunnerConfig> {
    match name.to_lowercase().as_str() {
        "rust" => Some(rust()),
        "python" => Some(python()),
        "typescript" | "ts" => Some(typescript()),
        "go" => Some(go()),
        _ => None,
    }
}

/// The Rust preset shells out to `cargo test` and parses its default stdout
/// (the `cargo-test` format). This works on a stock Rust toolchain — no
/// third-party reporter (nextest, cargo2junit, …) required. Users who prefer
/// nextest can override `command` and `format` via their `[runner.rust]`
/// section.
fn rust() -> RunnerConfig {
    RunnerConfig {
        command: Some("cargo test --no-fail-fast -- --test-threads=1".into()),
        test_dir: None,
        format: Some(OutputFormat::CargoTest),
        file_extensions: Some(vec!["rs".into()]),
        available_check: Some("cargo".into()),
        ..Default::default()
    }
}

fn python() -> RunnerConfig {
    RunnerConfig {
        command: Some("pytest --junit-xml={junit_path} -v {test_dir}".into()),
        test_dir: None,
        format: Some(OutputFormat::JunitXml),
        output_path: None, // uses {junit_path} tempfile
        file_extensions: Some(vec!["py".into()]),
        available_check: Some("pytest".into()),
        ..Default::default()
    }
}

fn typescript() -> RunnerConfig {
    let mut env = HashMap::new();
    env.insert("JEST_JUNIT_OUTPUT_FILE".into(), "{junit_path}".into());
    RunnerConfig {
        command: Some("npx jest --reporters=default --reporters=jest-junit {test_dir}".into()),
        test_dir: None,
        format: Some(OutputFormat::JunitXml),
        output_path: None, // jest-junit writes to JEST_JUNIT_OUTPUT_FILE
        file_extensions: Some(vec!["ts".into(), "js".into()]),
        env,
        available_check: Some("npx".into()),
        ..Default::default()
    }
}

fn go() -> RunnerConfig {
    RunnerConfig {
        command: Some("gotestsum --junitfile={junit_path} -- ./...".into()),
        test_dir: None,
        format: Some(OutputFormat::JunitXml),
        output_path: None, // uses {junit_path}
        file_extensions: Some(vec!["go".into()]),
        available_check: Some("gotestsum".into()),
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_presets_resolve() {
        for name in ["rust", "python", "typescript", "go"] {
            let p = preset(name).expect(name);
            // Must have all the fields that a resolve() without user overrides needs.
            assert!(p.command.is_some(), "{name} missing command");
            assert!(p.format.is_some(), "{name} missing format");
            assert!(p.file_extensions.is_some(), "{name} missing file_extensions");
        }
    }

    #[test]
    fn unknown_preset_returns_none() {
        assert!(preset("haskell").is_none());
    }

    #[test]
    fn typescript_alias() {
        assert!(preset("ts").is_some());
    }
}
