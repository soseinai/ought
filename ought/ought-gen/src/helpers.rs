//! Shared test helpers for CLI integration tests.
//!
//! The CLI tests scaffold a temporary ought project on disk and shell out to
//! the `ought` binary, so they need a way to locate that binary and scaffold
//! repeatable project structures. These helpers live here (as opposed to
//! being duplicated into every test file) because the CLI tests, unlike the
//! per-subsystem generated tests, share a lot of setup boilerplate.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

/// Locate the built `ought` binary inside the workspace's target directory.
///
/// `CARGO_BIN_EXE_<name>` is set only for integration tests in the binary's
/// own package, so in `ought-dogfood` we walk up from our own manifest dir
/// to find `target/{debug,release}/ought`. Respects `CARGO_TARGET_DIR` for
/// custom layouts.
pub fn ought_bin() -> PathBuf {
    let exe_name = if cfg!(windows) { "ought.exe" } else { "ought" };

    // Workspace root = parent of parent of CARGO_MANIFEST_DIR
    // (which is `<workspace>/ought/ought-gen`).
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .map(Path::to_path_buf)
        .expect("CARGO_MANIFEST_DIR has fewer than 2 ancestors");

    let target_dir = std::env::var("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| workspace_root.join("target"));

    for profile in ["debug", "release"] {
        let candidate = target_dir.join(profile).join(exe_name);
        if candidate.exists() {
            return candidate;
        }
    }
    panic!(
        "ought binary not found under {}; run `cargo build -p ought` first",
        target_dir.display()
    );
}

/// Create a unique temporary directory with the given prefix. Returned dirs
/// are unique per process but not per test — tests should include their own
/// disambiguator in the prefix.
pub fn unique_dir(prefix: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("{}_{}", prefix, std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// Scaffold a minimal ought project in `dir`: `ought.toml`, an empty
/// `Cargo.toml` + `src/lib.rs` so `cargo test` works, and one trivial spec.
pub fn scaffold_project(dir: &Path) {
    let spec_dir = dir.join("ought");
    fs::create_dir_all(&spec_dir).unwrap();
    fs::write(
        dir.join("ought.toml"),
        "[project]\nname = \"test\"\n\n[specs]\nroots = [\"ought/\"]\n\n[generator]\nprovider = \"anthropic\"\n\n[runner.rust]\ntest_dir = \"ought/ought-gen/\"\n",
    )
    .unwrap();
    fs::write(
        dir.join("Cargo.toml"),
        "[package]\nname = \"ought-test-project\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .unwrap();
    fs::create_dir_all(dir.join("src")).unwrap();
    fs::write(dir.join("src/lib.rs"), "// placeholder\n").unwrap();
    fs::write(
        spec_dir.join("test.ought.md"),
        "# Test\n\n## Basic\n\n- **MUST** do something\n",
    )
    .unwrap();
}

/// Overwrite `ought/spec.ought.md` with a single-clause spec using `keyword`
/// (e.g. `"MUST"`, `"SHOULD"`) and free-form clause text.
pub fn write_spec(dir: &Path, keyword: &str, clause_text: &str) {
    let spec_dir = dir.join("ought");
    fs::create_dir_all(&spec_dir).unwrap();
    fs::write(
        spec_dir.join("spec.ought.md"),
        format!(
            "# Spec\n\ncontext: test\n\n## Section\n\n- **{}** {}\n",
            keyword, clause_text
        ),
    )
    .unwrap();
}

/// Recursively collect every file under `dir` into a BTreeSet (for
/// deterministic comparisons).
pub fn walkdir(dir: &Path) -> BTreeSet<PathBuf> {
    let mut result = BTreeSet::new();
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                result.extend(walkdir(&path));
            } else {
                result.insert(path);
            }
        }
    }
    result
}

/// Write a generated test file for `clause_id` (using `__` as the hierarchy
/// separator) and register it in the project's `Cargo.toml` so `cargo test`
/// picks it up. `pass` controls whether the generated body asserts true
/// (passing) or asserts false (deliberately failing).
pub fn write_test(dir: &Path, clause_id: &str, pass: bool) {
    let test_dir = dir.join("ought").join("ought-gen");
    let parts: Vec<&str> = clause_id.split("__").collect();
    let file_path = if parts.len() > 1 {
        let dir_parts = &parts[..parts.len() - 1];
        let file_name = parts[parts.len() - 1];
        let mut p = test_dir.clone();
        for part in dir_parts {
            p = p.join(part);
        }
        p.join(format!("{}_test.rs", file_name))
    } else {
        test_dir.join(format!("{}_test.rs", clause_id))
    };
    if let Some(parent) = file_path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    // The runner's `clause_id_to_test_name` produces `test_<path>` where `::`
    // → `__`. The clause_id passed in already uses `__`, so prefix with `test_`.
    let function_name = format!("test_{}", clause_id);
    let body = if pass {
        format!(
            "#[test]\nfn {}() {{\n    assert!(true);\n}}\n",
            function_name
        )
    } else {
        format!(
            "#[test]\nfn {}() {{\n    assert!(false, \"deliberately failing\");\n}}\n",
            function_name
        )
    };
    fs::write(&file_path, body).unwrap();

    let cargo_toml = dir.join("Cargo.toml");
    let rel_path = file_path
        .strip_prefix(dir)
        .unwrap()
        .to_string_lossy()
        .replace('\\', "/");
    let test_entry = format!(
        "\n[[test]]\nname = \"{}\"\npath = \"{}\"\nharness = true\n",
        clause_id, rel_path
    );
    let mut cargo_content = fs::read_to_string(&cargo_toml).unwrap();
    cargo_content.push_str(&test_entry);
    fs::write(&cargo_toml, cargo_content).unwrap();
}
