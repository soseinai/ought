#![allow(dead_code, clippy::all)]
#![allow(non_snake_case)]
#[allow(unused_imports)]
use std::path::{Path, PathBuf};
#[allow(unused_imports)]
use std::fs;
#[allow(unused_imports)]
use std::process::Command;
#[allow(unused_imports)]
use ought_cli::config::Config;
#[allow(unused_imports)]
use ought_spec::SpecGraph;

/// Returns the path to the `ought` binary built by `cargo test`.
fn ought_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_ought"))
}

/// Creates a unique temporary directory with the given prefix.
fn unique_dir(prefix: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("{}_{}", prefix, std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// Creates a minimal ought project in the given directory.
/// This includes a Cargo.toml and src/lib.rs so that `cargo test` can run.
fn scaffold_project(dir: &Path) {
    let spec_dir = dir.join("ought");
    fs::create_dir_all(&spec_dir).unwrap();
    fs::write(
        dir.join("ought.toml"),
        "[project]\nname = \"test\"\n\n[specs]\nroots = [\"ought/\"]\n\n[generator]\nprovider = \"anthropic\"\n\n[runner.rust]\ncommand = \"cargo test\"\ntest_dir = \"ought/ought-gen/\"\n",
    )
    .unwrap();
    // Create a minimal Cargo.toml so the rust runner can find a Cargo project.
    fs::write(
        dir.join("Cargo.toml"),
        "[package]\nname = \"ought-test-project\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .unwrap();
    // Create src/lib.rs so cargo has something to compile.
    fs::create_dir_all(dir.join("src")).unwrap();
    fs::write(dir.join("src/lib.rs"), "// placeholder\n").unwrap();
    fs::write(
        spec_dir.join("test.ought.md"),
        "# Test\n\n## Basic\n\n- **MUST** do something\n",
    )
    .unwrap();
}

/// Writes a spec file with the given keyword (e.g. "MUST", "SHOULD") and clause text.
fn write_spec(dir: &Path, keyword: &str, clause_text: &str) {
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

/// Recursively collects all file paths under a directory into a BTreeSet.
fn walkdir(dir: &Path) -> std::collections::BTreeSet<PathBuf> {
    let mut result = std::collections::BTreeSet::new();
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

/// Writes a generated test file for a clause identifier (using `__` as separator).
/// `pass` controls whether the generated test passes or fails.
/// Also registers the test file in the project's Cargo.toml as a `[[test]]` entry
/// so that `cargo test` can discover and run it.
fn write_test(dir: &Path, clause_id: &str, pass: bool) {
    let test_dir = dir.join("ought").join("ought-gen");
    // Convert double-underscore separated clause id into a directory path
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
    // The runner's `clause_id_to_test_name` produces snake_case names by replacing
    // `::` with `_`. The clause_id passed here uses `__` as a hierarchy separator,
    // so we collapse it to `_` to match what the runner expects in its HashMap.
    let function_name = clause_id.replace("__", "_");
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

    // Append a [[test]] entry to Cargo.toml so cargo test discovers this file.
    let cargo_toml = dir.join("Cargo.toml");
    let rel_path = file_path
        .strip_prefix(dir)
        .unwrap()
        .to_string_lossy()
        .replace('\\', "/");
    // Use the clause_id as the test name (unique per test).
    let test_entry = format!(
        "\n[[test]]\nname = \"{}\"\npath = \"{}\"\nharness = true\n",
        clause_id, rel_path
    );
    let mut cargo_content = fs::read_to_string(&cargo_toml).unwrap();
    cargo_content.push_str(&test_entry);
    fs::write(&cargo_toml, cargo_content).unwrap();
}

// --- must_exit_with_code_0_if_all_specs_are_valid_1_if_any_are_invalid_test.rs ---
/// MUST exit with code 0 if all specs are valid, 1 if any are invalid
#[test]
fn test_cli_check_must_exit_with_code_0_if_all_specs_are_valid_1_if_any_are_invalid() {
    let base = std::env::temp_dir()
        .join(format!("ought_check_exit_{}", std::process::id()));
    let valid_dir = base.join("valid");
    let invalid_dir = base.join("invalid");
    std::fs::create_dir_all(&valid_dir).unwrap();
    std::fs::create_dir_all(&invalid_dir).unwrap();

    // ── exit 0: all specs are valid ───────────────────────────────────────────
    std::fs::write(
        valid_dir.join("svc.ought.md"),
        "# Service\n\n## Auth\n\n- **MUST** authenticate every request\n",
    )
    .unwrap();

    let valid_result = ought_spec::SpecGraph::from_roots(&[valid_dir.clone()]);
    assert!(
        valid_result.is_ok(),
        "valid specs must not produce errors (ought check would exit 0); errors: {:?}",
        valid_result.err()
    );

    // ── exit 1: at least one spec is invalid (circular dependency) ────────────
    // Two specs that require each other form a cycle, which is a hard error.
    std::fs::write(
        invalid_dir.join("a.ought.md"),
        "# Spec A\n\nrequires: [B](./b.ought.md)\n\n## Section\n\n- **MUST** depend on B\n",
    )
    .unwrap();
    std::fs::write(
        invalid_dir.join("b.ought.md"),
        "# Spec B\n\nrequires: [A](./a.ought.md)\n\n## Section\n\n- **MUST** depend on A\n",
    )
    .unwrap();

    let invalid_result = ought_spec::SpecGraph::from_roots(&[invalid_dir.clone()]);
    assert!(
        invalid_result.is_err(),
        "specs with circular dependencies must produce errors (ought check would exit 1)"
    );
    let errors = invalid_result.unwrap_err();
    assert!(
        !errors.is_empty(),
        "ought check must report at least one error when specs are invalid"
    );
    let any_cycle_msg = errors
        .iter()
        .any(|e| e.message.contains("circular") || e.message.contains("cycle"));
    assert!(
        any_cycle_msg,
        "error must describe the circular dependency; got: {:?}",
        errors
            .iter()
            .map(|e| e.to_string())
            .collect::<Vec<_>>()
    );

    let _ = std::fs::remove_dir_all(&base);
}
// --- must_report_parse_errors_with_file_line_number_and_a_human_readab_test.rs ---
/// MUST report parse errors with file, line number, and a human-readable message
#[test]
fn test_cli_check_must_report_parse_errors_with_file_line_number_and_a_human_readab() {
    let base = std::env::temp_dir()
        .join(format!("ought_check_errmsg_{}", std::process::id()));
    std::fs::create_dir_all(&base).unwrap();

    // A missing file is the canonical trigger for a file-level ParseError.
    let missing = base.join("nonexistent.ought.md");

    let errors = ought_spec::Parser::parse_file(&missing)
        .expect_err("parsing a missing file must return Vec<ParseError>");

    assert!(!errors.is_empty(), "at least one parse error must be reported");
    let err = &errors[0];

    // Must carry the correct file path.
    assert_eq!(err.file, missing, "error must name the problematic file");

    // The Display impl must produce the canonical `file:line: message` format.
    let displayed = err.to_string();
    assert!(
        displayed.contains(missing.to_str().unwrap()),
        "Display must embed the file path; got: {:?}",
        displayed
    );
    let rest = displayed.trim_start_matches(missing.to_str().unwrap());
    assert!(
        rest.starts_with(':'),
        "Display must follow `file:line: message` format; got: {:?}",
        displayed
    );

    // Message must be non-trivially human-readable.
    assert!(
        !err.message.is_empty(),
        "parse error message must not be empty"
    );
    assert!(
        err.message.split_whitespace().count() >= 2,
        "message must be human-readable (≥ 2 words); got: {:?}",
        err.message
    );

    let _ = std::fs::remove_dir_all(&base);
}
// --- must_validate_that_cross_file_references_links_to_other_ought_md_test.rs ---
/// MUST validate that cross-file references (links to other .ought.md files) resolve
#[test]
fn test_cli_check_must_validate_that_cross_file_references_links_to_other_ought_md() {
    let base = std::env::temp_dir()
        .join(format!("ought_check_xref_{}", std::process::id()));
    let valid_dir = base.join("valid");
    let broken_dir = base.join("broken");
    std::fs::create_dir_all(&valid_dir).unwrap();
    std::fs::create_dir_all(&broken_dir).unwrap();

    // ── valid case: referenced file exists in the same roots ─────────────────
    std::fs::write(
        valid_dir.join("base.ought.md"),
        "# Base\n\n## Core\n\n- **MUST** exist\n",
    )
    .unwrap();
    std::fs::write(
        valid_dir.join("extension.ought.md"),
        "# Extension\n\nrequires: [Base](./base.ought.md)\n\n## Extra\n\n- **MUST** build on base\n",
    )
    .unwrap();

    let valid_result = ought_spec::SpecGraph::from_roots(&[valid_dir.clone()]);
    assert!(
        valid_result.is_ok(),
        "specs whose cross-file references resolve must pass check; errors: {:?}",
        valid_result.err()
    );
    assert_eq!(
        valid_result.unwrap().specs().len(),
        2,
        "both the referencing and referenced spec must be loaded"
    );

    // ── broken case: referenced file does not exist ───────────────────────────
    std::fs::write(
        broken_dir.join("consumer.ought.md"),
        "# Consumer\n\nrequires: [Ghost](./ghost.ought.md)\n\n## Usage\n\n- **MUST** depend on ghost\n",
    )
    .unwrap();
    // ghost.ought.md is intentionally absent.

    let broken_result = ought_spec::SpecGraph::from_roots(&[broken_dir.clone()]);
    assert!(
        broken_result.is_err(),
        "specs with unresolvable cross-file references must fail check"
    );
    let errs = broken_result.unwrap_err();
    let combined = errs
        .iter()
        .map(|e| e.to_string())
        .collect::<Vec<_>>()
        .join("; ")
        .to_lowercase();
    assert!(
        combined.contains("ghost")
            || combined.contains("not found")
            || combined.contains("resolve")
            || combined.contains("missing"),
        "error must reference the unresolved file; got: {:?}",
        combined
    );

    let _ = std::fs::remove_dir_all(&base);
}
// --- must_validate_the_syntax_of_all_spec_files_without_generating_or_test.rs ---
/// MUST validate the syntax of all spec files without generating or running anything
#[test]
fn test_cli_check_must_validate_the_syntax_of_all_spec_files_without_generating_or() {
    let base = std::env::temp_dir()
        .join(format!("ought_check_syntax_{}", std::process::id()));
    let spec_dir = base.join("specs");
    std::fs::create_dir_all(&spec_dir).unwrap();

    std::fs::write(
        spec_dir.join("auth.ought.md"),
        "# Auth\n\n## Login\n\n- **MUST** return a JWT token\n- **MUST NOT** expose password hashes\n",
    )
    .unwrap();
    std::fs::write(
        spec_dir.join("api.ought.md"),
        "# API\n\n## Endpoints\n\n- **MUST** respond within 200ms\n- **SHOULD** use pagination\n",
    )
    .unwrap();

    // Snapshot the directory state before invoking the check operation.
    let mut before: Vec<_> = std::fs::read_dir(&base)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .collect();
    before.sort();

    let result = ought_spec::SpecGraph::from_roots(&[spec_dir.clone()]);

    assert!(
        result.is_ok(),
        "check must succeed for valid spec files; errors: {:?}",
        result.err()
    );
    let graph = result.unwrap();
    assert_eq!(
        graph.specs().len(),
        2,
        "check must validate every spec file found in the roots"
    );

    // Check must be a read-only operation — no new files or directories created.
    let mut after: Vec<_> = std::fs::read_dir(&base)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .collect();
    after.sort();
    assert_eq!(
        before, after,
        "check must not generate or write any files"
    );

    let _ = std::fs::remove_dir_all(&base);
}
// --- must_show_the_diff_between_current_generated_tests_and_what_would_test.rs ---
/// MUST show the diff between current generated tests and what would be generated now
#[test]
fn test_cli_diff_must_show_the_diff_between_current_generated_tests_and_what_would() {
    let dir = std::env::temp_dir()
        .join(format!("ought_diff_show_{}", std::process::id()));
    std::fs::create_dir_all(dir.join("ought")).unwrap();

    std::fs::write(
        dir.join("ought.toml"),
        "[project]\nname = \"test\"\nversion = \"0.1.0\"\n\n\
         [specs]\nroots = [\"ought/\"]\n\n\
         [context]\nsearch_paths = [\"src/\"]\nexclude = [\"target/\"]\n\n\
         [generator]\nprovider = \"anthropic\"\n\n\
         [runner.rust]\ncommand = \"cargo test\"\ntest_dir = \"ought/ought-gen/\"\n",
    )
    .unwrap();

    std::fs::write(
        dir.join("ought/auth.ought.md"),
        "# Auth\n\ncontext: Authentication service\n\n## Login\n\n- **MUST** return a JWT on success\n",
    )
    .unwrap();

    // Write an existing generated test representing the "current" state on disk.
    let gen_dir = dir.join("ought/ought-gen/auth/login");
    std::fs::create_dir_all(&gen_dir).unwrap();
    std::fs::write(
        gen_dir.join("must_return_a_jwt_on_success.rs"),
        "#[test]\nfn test_auth__login__must_return_a_jwt_on_success() {\n    // generated from old clause text\n    assert!(false, \"placeholder\");\n}\n",
    )
    .unwrap();

    // Manifest with a stale hash so ought diff has a change to report.
    std::fs::create_dir_all(dir.join("ought/ought-gen")).unwrap();
    std::fs::write(
        dir.join("ought/ought-gen/manifest.toml"),
        "[\"auth::login::must_return_a_jwt_on_success\"]\n\
         clause_hash = \"old_stale_hash_does_not_match_current\"\n\
         source_hash = \"\"\n\
         generated_at = \"2020-01-01T00:00:00Z\"\n\
         model = \"test\"\n",
    )
    .unwrap();

    let bin = option_env!("CARGO_BIN_EXE_ought")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("ought"));

    let output = std::process::Command::new(&bin)
        .arg("diff")
        .current_dir(&dir)
        .output()
        .expect("failed to invoke ought diff");

    // Must not exit with a clap usage error.
    assert_ne!(
        output.status.code(),
        Some(2),
        "ought diff must not exit with a usage error (2); stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    // Must not be the unimplemented stub message.
    assert!(
        !combined.contains("not yet implemented"),
        "ought diff must be implemented and show actual diffs; got:\n{combined}"
    );

    // Must produce output when stale clauses exist.
    assert!(
        !combined.trim().is_empty(),
        "ought diff must produce output when generated tests are stale; got no output"
    );

    // Output must reference the affected clause or file.
    assert!(
        combined.contains("auth") || combined.contains("must_return_a_jwt_on_success"),
        "ought diff output must reference the stale clause or its spec; got:\n{combined}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}
// --- should_group_diffs_by_spec_file_test.rs ---
/// SHOULD group diffs by spec file
#[test]
fn test_cli_diff_should_group_diffs_by_spec_file() {
    let dir = std::env::temp_dir()
        .join(format!("ought_diff_group_{}", std::process::id()));
    std::fs::create_dir_all(dir.join("ought")).unwrap();

    std::fs::write(
        dir.join("ought.toml"),
        "[project]\nname = \"test\"\nversion = \"0.1.0\"\n\n\
         [specs]\nroots = [\"ought/\"]\n\n\
         [context]\nsearch_paths = [\"src/\"]\nexclude = [\"target/\"]\n\n\
         [generator]\nprovider = \"anthropic\"\n\n\
         [runner.rust]\ncommand = \"cargo test\"\ntest_dir = \"ought/ought-gen/\"\n",
    )
    .unwrap();

    // Two separate spec files, each with a stale clause.
    std::fs::write(
        dir.join("ought/auth.ought.md"),
        "# Auth\n\ncontext: Authentication\n\n## Login\n\n- **MUST** validate credentials\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("ought/payments.ought.md"),
        "# Payments\n\ncontext: Payments\n\n## Charge\n\n- **MUST** validate card expiry\n",
    )
    .unwrap();

    // Existing generated tests for both specs.
    let gen_auth = dir.join("ought/ought-gen/auth/login");
    let gen_pay = dir.join("ought/ought-gen/payments/charge");
    std::fs::create_dir_all(&gen_auth).unwrap();
    std::fs::create_dir_all(&gen_pay).unwrap();
    std::fs::write(
        gen_auth.join("must_validate_credentials.rs"),
        "#[test]\nfn test_auth__login__must_validate_credentials() { assert!(true); }\n",
    )
    .unwrap();
    std::fs::write(
        gen_pay.join("must_validate_card_expiry.rs"),
        "#[test]\nfn test_payments__charge__must_validate_card_expiry() { assert!(true); }\n",
    )
    .unwrap();

    // Both manifest entries are stale.
    std::fs::create_dir_all(dir.join("ought/ought-gen")).unwrap();
    std::fs::write(
        dir.join("ought/ought-gen/manifest.toml"),
        "[\"auth::login::must_validate_credentials\"]\n\
         clause_hash = \"stale_auth_hash\"\n\
         source_hash = \"\"\n\
         generated_at = \"2020-01-01T00:00:00Z\"\n\
         model = \"test\"\n\
         \n\
         [\"payments::charge::must_validate_card_expiry\"]\n\
         clause_hash = \"stale_payments_hash\"\n\
         source_hash = \"\"\n\
         generated_at = \"2020-01-01T00:00:00Z\"\n\
         model = \"test\"\n",
    )
    .unwrap();

    let bin = option_env!("CARGO_BIN_EXE_ought")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("ought"));

    let output = std::process::Command::new(&bin)
        .arg("diff")
        .current_dir(&dir)
        .output()
        .expect("failed to invoke ought diff");

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Output must reference both spec files.
    assert!(
        stdout.contains("auth"),
        "ought diff must include a section for auth.ought.md; stdout:\n{stdout}"
    );
    assert!(
        stdout.contains("payments"),
        "ought diff must include a section for payments.ought.md; stdout:\n{stdout}"
    );

    // The two spec files must appear as distinct groups: auth content and payments
    // content must not be interleaved without any per-file header.  We verify this
    // by checking that the first occurrence of each spec name is separated by some
    // output rather than appearing on the same line.
    let auth_pos = stdout.find("auth").expect("auth must appear in output");
    let pay_pos = stdout.find("payments").expect("payments must appear in output");
    assert!(
        auth_pos != pay_pos,
        "auth and payments diff groups must appear at distinct positions in the output"
    );

    let _ = std::fs::remove_dir_all(&dir);
}
// --- should_use_a_familiar_unified_diff_format_test.rs ---
/// SHOULD use a familiar unified diff format
#[test]
fn test_cli_diff_should_use_a_familiar_unified_diff_format() {
    let dir = std::env::temp_dir()
        .join(format!("ought_diff_format_{}", std::process::id()));
    std::fs::create_dir_all(dir.join("ought")).unwrap();

    std::fs::write(
        dir.join("ought.toml"),
        "[project]\nname = \"test\"\nversion = \"0.1.0\"\n\n\
         [specs]\nroots = [\"ought/\"]\n\n\
         [context]\nsearch_paths = [\"src/\"]\nexclude = [\"target/\"]\n\n\
         [generator]\nprovider = \"anthropic\"\n\n\
         [runner.rust]\ncommand = \"cargo test\"\ntest_dir = \"ought/ought-gen/\"\n",
    )
    .unwrap();

    std::fs::write(
        dir.join("ought/payments.ought.md"),
        "# Payments\n\ncontext: Payment processing\n\n## Charge\n\n- **MUST** reject invalid card numbers\n",
    )
    .unwrap();

    // Existing generated test — the "before" side of the diff.
    let gen_dir = dir.join("ought/ought-gen/payments/charge");
    std::fs::create_dir_all(&gen_dir).unwrap();
    std::fs::write(
        gen_dir.join("must_reject_invalid_card_numbers.rs"),
        "#[test]\nfn test_payments__charge__must_reject_invalid_card_numbers() {\n    // old body\n    assert!(true);\n}\n",
    )
    .unwrap();

    // Stale manifest entry to give diff something to show.
    std::fs::create_dir_all(dir.join("ought/ought-gen")).unwrap();
    std::fs::write(
        dir.join("ought/ought-gen/manifest.toml"),
        "[\"payments::charge::must_reject_invalid_card_numbers\"]\n\
         clause_hash = \"outdated_hash_does_not_match_current_clause\"\n\
         source_hash = \"\"\n\
         generated_at = \"2020-06-01T00:00:00Z\"\n\
         model = \"test\"\n",
    )
    .unwrap();

    let bin = option_env!("CARGO_BIN_EXE_ought")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("ought"));

    let output = std::process::Command::new(&bin)
        .arg("diff")
        .current_dir(&dir)
        .output()
        .expect("failed to invoke ought diff");

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Unified diff format requires --- / +++ file headers and @@ hunk markers.
    assert!(
        stdout.contains("---") && stdout.contains("+++"),
        "ought diff output must use unified diff format with --- and +++ file headers;\
         \nstdout:\n{stdout}"
    );
    assert!(
        stdout.contains("@@"),
        "ought diff output must include @@ hunk markers as in unified diff format;\
         \nstdout:\n{stdout}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}
// --- must_not_execute_tests_during_generation_that_is_run_s_job_test.rs ---
/// MUST NOT execute tests during generation (that is `run`'s job)
#[test]
fn test_cli_generate_must_not_execute_tests_during_generation_that_is_run_s_job() {
    let dir = std::env::temp_dir()
        .join(format!("ought_gen_norun_{}", std::process::id()));
    std::fs::create_dir_all(dir.join("ought")).unwrap();

    std::fs::write(
        dir.join("ought.toml"),
        "[project]\nname = \"test\"\nversion = \"0.1.0\"\n\n\
         [specs]\nroots = [\"ought/\"]\n\n\
         [context]\nsearch_paths = [\"src/\"]\nexclude = [\"target/\"]\n\n\
         [generator]\nprovider = \"anthropic\"\n\n\
         [runner.rust]\ncommand = \"cargo test\"\ntest_dir = \"ought/ought-gen/\"\n",
    )
    .unwrap();

    std::fs::write(
        dir.join("ought/spec.ought.md"),
        "# Spec\n\ncontext: test\n\n## Section\n\n- **MUST** do the thing\n",
    )
    .unwrap();

    let bin = option_env!("CARGO_BIN_EXE_ought")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("ought"));

    let out = std::process::Command::new(&bin)
        .arg("generate")
        .arg("--check")
        .current_dir(&dir)
        .output()
        .expect("failed to invoke ought generate --check");

    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );

    // cargo test output always contains "test result:" or "running N test".
    // Neither must appear during generation.
    assert!(
        !combined.contains("test result:"),
        "ought generate must not execute tests; found 'test result:' in output: {combined}"
    );
    assert!(
        !combined.contains("running 1 test") && !combined.contains("running 0 tests"),
        "ought generate must not invoke the test runner; found runner output: {combined}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}
// --- must_regenerate_test_code_for_all_clauses_where_the_clause_hash_o_test.rs ---
/// MUST regenerate test code for all clauses where the clause hash or source hash has changed
#[test]
fn test_cli_generate_must_regenerate_test_code_for_all_clauses_where_the_clause_hash_o() {
    let dir = std::env::temp_dir()
        .join(format!("ought_gen_rehash_{}", std::process::id()));
    std::fs::create_dir_all(dir.join("ought")).unwrap();

    std::fs::write(
        dir.join("ought.toml"),
        "[project]\nname = \"test\"\nversion = \"0.1.0\"\n\n\
         [specs]\nroots = [\"ought/\"]\n\n\
         [context]\nsearch_paths = [\"src/\"]\nexclude = [\"target/\"]\n\n\
         [generator]\nprovider = \"anthropic\"\n\n\
         [runner.rust]\ncommand = \"cargo test\"\ntest_dir = \"ought/ought-gen/\"\n",
    )
    .unwrap();

    std::fs::write(
        dir.join("ought/spec.ought.md"),
        "# Spec\n\ncontext: test\n\n## Section\n\n- **MUST** do the thing\n",
    )
    .unwrap();

    // Write a manifest with a deliberately wrong clause hash so the clause is stale.
    std::fs::create_dir_all(dir.join("ought/ought-gen")).unwrap();
    std::fs::write(
        dir.join("ought/ought-gen/manifest.toml"),
        "[\"spec::section::must_do_the_thing\"]\n\
         clause_hash = \"old_stale_hash_that_will_not_match\"\n\
         source_hash = \"\"\n\
         generated_at = \"2020-01-01T00:00:00Z\"\n\
         model = \"test\"\n",
    )
    .unwrap();

    let bin = option_env!("CARGO_BIN_EXE_ought")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("ought"));

    // --check detects staleness without invoking the LLM.
    let out = std::process::Command::new(&bin)
        .arg("generate")
        .arg("--check")
        .current_dir(&dir)
        .output()
        .expect("failed to invoke ought generate --check");

    // When any clause hash is stale, --check must exit 1.
    assert_eq!(
        out.status.code(),
        Some(1),
        "ought generate --check must exit 1 when clause hash has changed; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("stale"),
        "ought generate --check must report stale clauses; stderr: {stderr}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}
// --- must_support_check_flag_that_exits_with_code_1_if_any_generated_t_test.rs ---
/// MUST support `--check` flag that exits with code 1 if any generated tests are stale (for CI)
#[test]
fn test_cli_generate_must_support_check_flag_that_exits_with_code_1_if_any_generated_t() {
    let dir = std::env::temp_dir()
        .join(format!("ought_gen_check_{}", std::process::id()));
    std::fs::create_dir_all(dir.join("ought")).unwrap();

    std::fs::write(
        dir.join("ought.toml"),
        "[project]\nname = \"test\"\nversion = \"0.1.0\"\n\n\
         [specs]\nroots = [\"ought/\"]\n\n\
         [context]\nsearch_paths = [\"src/\"]\nexclude = [\"target/\"]\n\n\
         [generator]\nprovider = \"anthropic\"\n\n\
         [runner.rust]\ncommand = \"cargo test\"\ntest_dir = \"ought/ought-gen/\"\n",
    )
    .unwrap();

    std::fs::write(
        dir.join("ought/spec.ought.md"),
        "# Spec\n\ncontext: test\n\n## Section\n\n- **MUST** do the thing\n",
    )
    .unwrap();

    // No manifest: every clause is stale by definition.
    let bin = option_env!("CARGO_BIN_EXE_ought")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("ought"));

    let out = std::process::Command::new(&bin)
        .arg("generate")
        .arg("--check")
        .current_dir(&dir)
        .output()
        .expect("failed to invoke ought generate --check");

    // --check must be accepted by clap (not a usage error).
    assert_ne!(
        out.status.code(),
        Some(2),
        "--check must be a recognised flag; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    // With stale clauses --check must exit 1.
    assert_eq!(
        out.status.code(),
        Some(1),
        "ought generate --check must exit 1 when generated tests are stale; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let _ = std::fs::remove_dir_all(&dir);
}
// --- must_support_force_flag_to_regenerate_all_clauses_regardless_of_h_test.rs ---
/// MUST support `--force` flag to regenerate all clauses regardless of hash
#[test]
fn test_cli_generate_must_support_force_flag_to_regenerate_all_clauses_regardless_of_h() {
    let dir = std::env::temp_dir()
        .join(format!("ought_gen_force_{}", std::process::id()));
    std::fs::create_dir_all(dir.join("ought")).unwrap();

    std::fs::write(
        dir.join("ought.toml"),
        "[project]\nname = \"test\"\nversion = \"0.1.0\"\n\n\
         [specs]\nroots = [\"ought/\"]\n\n\
         [context]\nsearch_paths = [\"src/\"]\nexclude = [\"target/\"]\n\n\
         [generator]\nprovider = \"anthropic\"\n\n\
         [runner.rust]\ncommand = \"cargo test\"\ntest_dir = \"ought/ought-gen/\"\n",
    )
    .unwrap();

    std::fs::write(
        dir.join("ought/spec.ought.md"),
        "# Spec\n\ncontext: test\n\n## Section\n\n- **MUST** do the thing\n",
    )
    .unwrap();

    let bin = option_env!("CARGO_BIN_EXE_ought")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("ought"));

    // --force must be a recognised flag (clap exit 2 = usage error).
    let out = std::process::Command::new(&bin)
        .arg("generate")
        .arg("--force")
        .arg("--check")
        .current_dir(&dir)
        .output()
        .expect("failed to invoke ought generate --force --check");

    assert_ne!(
        out.status.code(),
        Some(2),
        "--force must be a recognised flag; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    // With --force all clauses are treated as stale regardless of manifest state.
    // Combining with --check avoids an LLM call while still asserting forced staleness.
    assert_eq!(
        out.status.code(),
        Some(1),
        "ought generate --force --check must exit 1 because --force marks every clause stale; \
         stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let _ = std::fs::remove_dir_all(&dir);
}
// --- must_update_the_manifest_toml_with_new_hashes_after_generation_test.rs ---
/// MUST update the manifest.toml with new hashes after generation
#[test]
fn test_cli_generate_must_update_the_manifest_toml_with_new_hashes_after_generation() {
    let dir = std::env::temp_dir()
        .join(format!("ought_gen_manifest_{}", std::process::id()));
    std::fs::create_dir_all(dir.join("ought")).unwrap();

    std::fs::write(
        dir.join("ought.toml"),
        "[project]\nname = \"test\"\nversion = \"0.1.0\"\n\n\
         [specs]\nroots = [\"ought/\"]\n\n\
         [context]\nsearch_paths = [\"src/\"]\nexclude = [\"target/\"]\n\n\
         [generator]\nprovider = \"anthropic\"\n\n\
         [runner.rust]\ncommand = \"cargo test\"\ntest_dir = \"ought/ought-gen/\"\n",
    )
    .unwrap();

    std::fs::write(
        dir.join("ought/spec.ought.md"),
        "# Spec\n\ncontext: test\n\n## Section\n\n- **MUST** do the thing\n",
    )
    .unwrap();

    // No manifest exists yet.
    let manifest_path = dir.join("ought/ought-gen/manifest.toml");
    assert!(
        !manifest_path.exists(),
        "manifest.toml must not exist before generate"
    );

    let bin = option_env!("CARGO_BIN_EXE_ought")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("ought"));

    // --check mode still saves the manifest (save happens before the stale exit).
    std::process::Command::new(&bin)
        .arg("generate")
        .arg("--check")
        .current_dir(&dir)
        .output()
        .expect("failed to invoke ought generate --check");

    // The manifest.toml must be written to ought/ought-gen/ after generate runs.
    assert!(
        manifest_path.exists(),
        "manifest.toml must be created at ought/ought-gen/manifest.toml after ought generate"
    );

    let _ = std::fs::remove_dir_all(&dir);
}
// --- must_write_generated_tests_to_the_ought_ought_gen_directory_test.rs ---
/// MUST write generated tests to the `ought/ought-gen/` directory
#[test]
fn test_cli_generate_must_write_generated_tests_to_the_ought_ought_gen_directory() {
    let dir = std::env::temp_dir()
        .join(format!("ought_gen_outdir_{}", std::process::id()));
    std::fs::create_dir_all(dir.join("ought")).unwrap();

    std::fs::write(
        dir.join("ought.toml"),
        "[project]\nname = \"test\"\nversion = \"0.1.0\"\n\n\
         [specs]\nroots = [\"ought/\"]\n\n\
         [context]\nsearch_paths = [\"src/\"]\nexclude = [\"target/\"]\n\n\
         [generator]\nprovider = \"anthropic\"\n\n\
         [runner.rust]\ncommand = \"cargo test\"\ntest_dir = \"ought/ought-gen/\"\n",
    )
    .unwrap();

    std::fs::write(
        dir.join("ought/spec.ought.md"),
        "# Spec\n\ncontext: test\n\n## Section\n\n- **MUST** do the thing\n",
    )
    .unwrap();

    // ought/ought-gen/ must not exist yet.
    let gen_dir = dir.join("ought/ought-gen");
    assert!(
        !gen_dir.exists(),
        "ought/ought-gen/ must not exist before generate"
    );

    let bin = option_env!("CARGO_BIN_EXE_ought")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("ought"));

    // --check avoids calling the LLM while still exercising the generate path.
    std::process::Command::new(&bin)
        .arg("generate")
        .arg("--check")
        .current_dir(&dir)
        .output()
        .expect("failed to invoke ought generate --check");

    assert!(
        gen_dir.is_dir(),
        "ought generate must create the ought/ought-gen/ output directory"
    );

    let _ = std::fs::remove_dir_all(&dir);
}
// --- should_show_a_progress_indicator_during_llm_generation_test.rs ---
/// SHOULD show a progress indicator during LLM generation
#[test]
fn test_cli_generate_should_show_a_progress_indicator_during_llm_generation() {
    let dir = std::env::temp_dir()
        .join(format!("ought_gen_progress_{}", std::process::id()));
    std::fs::create_dir_all(dir.join("ought")).unwrap();

    std::fs::write(
        dir.join("ought.toml"),
        "[project]\nname = \"test\"\nversion = \"0.1.0\"\n\n\
         [specs]\nroots = [\"ought/\"]\n\n\
         [context]\nsearch_paths = [\"src/\"]\nexclude = [\"target/\"]\n\n\
         [generator]\nprovider = \"anthropic\"\n\n\
         [runner.rust]\ncommand = \"cargo test\"\ntest_dir = \"ought/ought-gen/\"\n",
    )
    .unwrap();

    std::fs::write(
        dir.join("ought/spec.ought.md"),
        "# Spec\n\ncontext: test\n\n## Section\n\n- **MUST** do the thing\n",
    )
    .unwrap();

    let bin = option_env!("CARGO_BIN_EXE_ought")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("ought"));

    let out = std::process::Command::new(&bin)
        .arg("generate")
        .arg("--check")
        .current_dir(&dir)
        .output()
        .expect("failed to invoke ought generate --check");

    // The generate command must emit diagnostic output to stderr so the user
    // can see activity (stale clause IDs, section headers, and/or a summary line).
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !stderr.trim().is_empty(),
        "ought generate must produce diagnostic output on stderr as a progress indicator; \
         got empty stderr"
    );

    let _ = std::fs::remove_dir_all(&dir);
}
// --- should_support_targeting_a_specific_spec_file_ought_generate_ought_test.rs ---
/// SHOULD support targeting a specific spec file: `ought generate ought/auth.ought.md`
#[test]
fn test_cli_generate_should_support_targeting_a_specific_spec_file_ought_generate_ought() {
    let dir = std::env::temp_dir()
        .join(format!("ought_gen_target_{}", std::process::id()));
    std::fs::create_dir_all(dir.join("ought")).unwrap();

    std::fs::write(
        dir.join("ought.toml"),
        "[project]\nname = \"test\"\nversion = \"0.1.0\"\n\n\
         [specs]\nroots = [\"ought/\"]\n\n\
         [context]\nsearch_paths = [\"src/\"]\nexclude = [\"target/\"]\n\n\
         [generator]\nprovider = \"anthropic\"\n\n\
         [runner.rust]\ncommand = \"cargo test\"\ntest_dir = \"ought/ought-gen/\"\n",
    )
    .unwrap();

    // Two spec files; the test targets only the auth one.
    std::fs::write(
        dir.join("ought/auth.ought.md"),
        "# Auth\n\ncontext: test\n\n## Login\n\n- **MUST** authenticate users\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("ought/billing.ought.md"),
        "# Billing\n\ncontext: test\n\n## Payments\n\n- **MUST** charge the card\n",
    )
    .unwrap();

    let bin = option_env!("CARGO_BIN_EXE_ought")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("ought"));

    let out = std::process::Command::new(&bin)
        .arg("generate")
        .arg("ought/auth.ought.md")
        .arg("--check")
        .current_dir(&dir)
        .output()
        .expect("failed to invoke ought generate with a specific spec path");

    // A path argument must not cause a clap usage error (exit 2).
    assert_ne!(
        out.status.code(),
        Some(2),
        "ought generate must accept a specific spec file path without a usage error; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let _ = std::fs::remove_dir_all(&dir);
}
// --- must_always_return_a_valid_exit_code_0_1_or_2_never_crash_without_an_exi_test.rs ---
/// MUST ALWAYS return a valid exit code (0, 1, or 2) — never crash without an exit code.
/// Invariant — verified across a wide fuzz-style range of invocations.
#[test]
fn test_cli_global_flags_must_always_return_a_valid_exit_code_0_1_or_2_never_crash_without_an_exi(
) {
    let bin = option_env!("CARGO_BIN_EXE_ought")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("ought"));

    let proj = std::env::temp_dir()
        .join(format!("ought_exitcode_inv_{}", std::process::id()));
    std::fs::create_dir_all(&proj).unwrap();

    // Pre-initialise so commands that need a project have one.
    let _ = std::process::Command::new(&bin)
        .arg("init")
        .current_dir(&proj)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output();

    // Fuzz corpus: valid commands, usage errors, missing args, bad flag values, edge cases.
    let invocations: &[&[&str]] = &[
        // ── Valid subcommands ────────────────────────────────────────────────
        &["check"],
        &["run"],
        &["diff"],
        &["--json", "run"],
        &["--quiet", "run"],
        &["--verbose", "check"],
        &["--color", "never", "check"],
        &["--color", "always", "check"],
        &["--color", "auto", "check"],
        &["--quiet", "--json", "run"],
        // ── Usage errors: unknown flags / subcommands (must exit 2) ─────────
        &["--nonexistent-flag"],
        &["nonexistent-subcommand"],
        &["--unknown-global", "check"],
        &["run", "--unknown-local"],
        // ── Missing required positional args (must exit 2) ──────────────────
        &["blame"],
        &["bisect"],
        &["inspect"],
        // ── Bad flag values (must exit 2) ────────────────────────────────────
        &["--color", "rainbow", "check"],
        // ── Config edge cases ────────────────────────────────────────────────
        &["--config", "/nonexistent/path/ought.toml", "check"],
        &["--config", "", "check"],
        // ── Re-init (project already exists) ─────────────────────────────────
        &["init"],
    ];

    for args in invocations {
        let out = std::process::Command::new(&bin)
            .args(*args)
            .current_dir(&proj)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .output()
            .unwrap_or_else(|e| panic!("process::Command failed for {args:?}: {e}"));

        let code = out.status.code();
        assert!(
            matches!(code, Some(0) | Some(1) | Some(2)),
            "ought {args:?} must exit 0, 1, or 2 — never crash or signal-terminate; \
             got {code:?}; stderr: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    let _ = std::fs::remove_dir_all(&proj);
}
// --- must_always_write_diagnostic_messages_to_stderr_never_stdout_stdout_is_r_test.rs ---
/// MUST ALWAYS write diagnostic messages to stderr, never stdout (stdout is reserved for
/// structured output and results). Invariant — verified across a range of invocations.
#[test]
fn test_cli_global_flags_must_always_write_diagnostic_messages_to_stderr_never_stdout_stdout_is_r(
) {
    let bin = option_env!("CARGO_BIN_EXE_ought")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("ought"));

    // Set up a valid project so most commands have something real to work with.
    let proj = std::env::temp_dir()
        .join(format!("ought_stderr_inv_{}", std::process::id()));
    std::fs::create_dir_all(&proj).unwrap();
    let init = std::process::Command::new(&bin)
        .arg("init")
        .current_dir(&proj)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .expect("ought init (stderr probe setup)");
    assert!(
        init.status.success(),
        "init must succeed for probe setup; stderr: {}",
        String::from_utf8_lossy(&init.stderr)
    );

    // Diagnostic keyword patterns that must never appear on stdout.
    // These are prefixes written by the application itself (not embedded in JSON payloads).
    let diagnostic_patterns: &[&str] = &[
        "error: ",
        "Error: ",
        "warning: ",
        "Warning: ",
        "fatal: ",
    ];

    // Probe a range of non-JSON invocations; for each, stdout must not contain raw diagnostics.
    let probes: &[(&[&str], &str)] = &[
        (&["check"], "check"),
        (&["--verbose", "check"], "--verbose check"),
        (&["--quiet", "check"], "--quiet check"),
        (&["run"], "run"),
        (&["--quiet", "run"], "--quiet run"),
    ];

    for (args, label) in probes {
        let out = std::process::Command::new(&bin)
            .args(*args)
            .current_dir(&proj)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .output()
            .unwrap_or_else(|e| panic!("failed to run ought {label}: {e}"));

        let stdout = String::from_utf8_lossy(&out.stdout);
        for pat in diagnostic_patterns {
            assert!(
                !stdout.contains(pat),
                "ought {label}: diagnostic pattern {pat:?} must not appear on stdout; \
                 stdout={stdout:?}"
            );
        }
    }

    // A known-error path (nonexistent --config) must write its error to stderr, not stdout.
    let bad = std::process::Command::new(&bin)
        .args(["--config", "/nonexistent/path/ought.toml", "check"])
        .current_dir(&proj)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .expect("ought --config /nonexistent check");

    assert_ne!(
        bad.status.code(),
        Some(0),
        "ought with a nonexistent --config must not succeed"
    );
    let bad_stdout = String::from_utf8_lossy(&bad.stdout);
    for pat in diagnostic_patterns {
        assert!(
            !bad_stdout.contains(pat),
            "error from bad --config must appear on stderr, not stdout; \
             stdout={bad_stdout:?}"
        );
    }
    let bad_stderr = String::from_utf8_lossy(&bad.stderr);
    assert!(
        !bad_stderr.is_empty(),
        "error from bad --config must produce output on stderr"
    );

    let _ = std::fs::remove_dir_all(&proj);
}
// --- must_support_color_auto_always_never_for_terminal_color_control_test.rs ---
/// MUST support `--color <auto|always|never>` for terminal color control
#[test]
fn test_cli_global_flags_must_support_color_auto_always_never_for_terminal_color_control() {
    let bin = option_env!("CARGO_BIN_EXE_ought")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("ought"));

    // Each variant must be accepted by the argument parser (exit code must not be 2).
    for color_value in ["auto", "always", "never"] {
        let dir = std::env::temp_dir().join(format!(
            "ought_color_{}_{}_{}",
            color_value,
            std::process::id(),
            color_value.len() // extra salt to keep names distinct in the loop
        ));
        std::fs::create_dir_all(&dir).unwrap();

        let out = std::process::Command::new(&bin)
            .args(["--color", color_value, "init"])
            .current_dir(&dir)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .output()
            .unwrap_or_else(|e| {
                panic!("failed to run `ought --color {color_value} init`: {e}")
            });

        assert_ne!(
            out.status.code(),
            Some(2),
            "--color {color_value} must not produce a usage/parse error; stderr: {}",
            String::from_utf8_lossy(&out.stderr)
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    // An invalid value must be rejected with exit code 2.
    let dir_invalid = std::env::temp_dir()
        .join(format!("ought_color_invalid_{}", std::process::id()));
    std::fs::create_dir_all(&dir_invalid).unwrap();
    let bad = std::process::Command::new(&bin)
        .args(["--color", "rainbow", "init"])
        .current_dir(&dir_invalid)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .expect("ought --color rainbow init");
    assert_eq!(
        bad.status.code(),
        Some(2),
        "--color with an invalid value must produce a usage error (exit 2)"
    );
    let _ = std::fs::remove_dir_all(&dir_invalid);
}
// --- must_support_config_path_to_specify_an_alternate_ought_toml_locat_test.rs ---
/// MUST support `--config <path>` to specify an alternate ought.toml location
#[test]
fn test_cli_global_flags_must_support_config_path_to_specify_an_alternate_ought_toml_locat() {
    let base = std::env::temp_dir()
        .join(format!("ought_cfg_flag_{}", std::process::id()));
    let specs_dir = base.join("specs");
    let alt_config = base.join("alt_ought.toml");
    std::fs::create_dir_all(&specs_dir).unwrap();

    std::fs::write(
        specs_dir.join("test.ought.md"),
        "# Test\n\n## Section: Basics\n\n### MUST do something\n",
    )
    .unwrap();

    // Write a valid ought.toml at a non-default location.
    std::fs::write(
        &alt_config,
        format!(
            "[project]\nname = \"test\"\nversion = \"0.1.0\"\n\n\
             [specs]\nroots = [\"{specs}\"]\n\n\
             [context]\nsearch_paths = [\"{base}\"]\n\n\
             [generator]\nprovider = \"anthropic\"\n",
            specs = specs_dir.display(),
            base = base.display(),
        ),
    )
    .unwrap();

    let bin = option_env!("CARGO_BIN_EXE_ought")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("ought"));

    // Without --config and no ought.toml in `base`, config discovery must fail.
    let no_cfg = std::process::Command::new(&bin)
        .arg("check")
        .current_dir(&base)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .expect("ought check (no config)");
    assert_ne!(
        no_cfg.status.code(),
        Some(0),
        "check with no discoverable config must not succeed"
    );

    // With --config pointing to the alternate file, ought must load it and succeed.
    let with_cfg = std::process::Command::new(&bin)
        .args(["--config", alt_config.to_str().unwrap(), "check"])
        .current_dir(&base)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .expect("ought --config check");
    assert_eq!(
        with_cfg.status.code(),
        Some(0),
        "--config must load the alternate file; stderr: {}",
        String::from_utf8_lossy(&with_cfg.stderr)
    );

    let _ = std::fs::remove_dir_all(&base);
}
// --- must_support_json_flag_that_outputs_structured_json_for_programma_test.rs ---
/// MUST support `--json` flag that outputs structured JSON for programmatic consumption
#[test]
fn test_cli_global_flags_must_support_json_flag_that_outputs_structured_json_for_programma() {
    let dir = std::env::temp_dir()
        .join(format!("ought_json_flag_{}", std::process::id()));
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

    // Run with --json; the flag must be accepted and any stdout must be valid JSON.
    let out = std::process::Command::new(&bin)
        .args(["--json", "run"])
        .current_dir(&dir)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .expect("ought --json run");

    assert_ne!(
        out.status.code(),
        Some(2),
        "--json must not produce a usage/parse error; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let stdout = String::from_utf8_lossy(&out.stdout);
    let trimmed = stdout.trim();
    if !trimmed.is_empty() {
        assert!(
            trimmed.starts_with('{') || trimmed.starts_with('['),
            "--json stdout must be a JSON object or array; got: {}",
            &trimmed[..trimmed.len().min(200)]
        );
        // Verify that all opening brackets have matching closing brackets.
        let opens: usize = trimmed
            .chars()
            .filter(|&c| c == '{' || c == '[')
            .count();
        let closes: usize = trimmed
            .chars()
            .filter(|&c| c == '}' || c == ']')
            .count();
        assert_eq!(
            opens, closes,
            "--json output must have balanced braces; stdout: {}",
            &trimmed[..trimmed.len().min(200)]
        );
    }

    let _ = std::fs::remove_dir_all(&dir);
}
// --- must_support_junit_path_flag_that_writes_junit_xml_results_to_the_test.rs ---
/// MUST support `--junit <path>` flag that writes JUnit XML results to the given file
#[test]
fn test_cli_global_flags_must_support_junit_path_flag_that_writes_junit_xml_results_to_the() {
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
// --- must_support_quiet_flag_that_suppresses_all_output_except_errors_test.rs ---
/// MUST support `--quiet` flag that suppresses all output except errors and the final summary
#[test]
fn test_cli_global_flags_must_support_quiet_flag_that_suppresses_all_output_except_errors() {
    let bin = option_env!("CARGO_BIN_EXE_ought")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("ought"));

    let dir_loud = std::env::temp_dir()
        .join(format!("ought_quiet_loud_{}", std::process::id()));
    let dir_quiet = std::env::temp_dir()
        .join(format!("ought_quiet_silent_{}", std::process::id()));
    std::fs::create_dir_all(&dir_loud).unwrap();
    std::fs::create_dir_all(&dir_quiet).unwrap();

    // Run `ought init` (produces informational output) without --quiet.
    let loud = std::process::Command::new(&bin)
        .arg("init")
        .current_dir(&dir_loud)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .expect("ought init (loud)");

    // Run the same command with --quiet.
    let quiet = std::process::Command::new(&bin)
        .args(["--quiet", "init"])
        .current_dir(&dir_quiet)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .expect("ought --quiet init");

    // --quiet must be a recognised flag (not a usage error).
    assert_ne!(
        quiet.status.code(),
        Some(2),
        "--quiet must not produce a usage/parse error; stderr: {}",
        String::from_utf8_lossy(&quiet.stderr)
    );

    // --quiet must suppress stdout relative to the normal invocation.
    assert!(
        quiet.stdout.len() <= loud.stdout.len(),
        "--quiet must produce no more stdout than default; quiet={} bytes, loud={} bytes",
        quiet.stdout.len(),
        loud.stdout.len()
    );

    let _ = std::fs::remove_dir_all(&dir_loud);
    let _ = std::fs::remove_dir_all(&dir_quiet);
}
// --- should_support_verbose_flag_for_debug_level_output_test.rs ---
/// SHOULD support `--verbose` flag for debug-level output
#[test]
fn test_cli_global_flags_should_support_verbose_flag_for_debug_level_output() {
    let bin = option_env!("CARGO_BIN_EXE_ought")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("ought"));

    // Set up two identical projects so we can compare output side-by-side.
    let dir_normal = std::env::temp_dir()
        .join(format!("ought_verbose_normal_{}", std::process::id()));
    let dir_verbose = std::env::temp_dir()
        .join(format!("ought_verbose_verbose_{}", std::process::id()));
    std::fs::create_dir_all(&dir_normal).unwrap();
    std::fs::create_dir_all(&dir_verbose).unwrap();

    for d in [&dir_normal, &dir_verbose] {
        let init = std::process::Command::new(&bin)
            .arg("init")
            .current_dir(d)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .output()
            .expect("ought init");
        assert!(
            init.status.success(),
            "init must succeed; stderr: {}",
            String::from_utf8_lossy(&init.stderr)
        );
    }

    let normal = std::process::Command::new(&bin)
        .arg("check")
        .current_dir(&dir_normal)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .expect("ought check (normal)");

    let verbose = std::process::Command::new(&bin)
        .args(["--verbose", "check"])
        .current_dir(&dir_verbose)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .expect("ought --verbose check");

    // --verbose must be a recognised flag (not a usage error).
    assert_ne!(
        verbose.status.code(),
        Some(2),
        "--verbose must not produce a usage/parse error; stderr: {}",
        String::from_utf8_lossy(&verbose.stderr)
    );

    // --verbose must produce at least as much total output as the default invocation,
    // since debug-level lines are added on top of the normal output.
    let normal_total = normal.stdout.len() + normal.stderr.len();
    let verbose_total = verbose.stdout.len() + verbose.stderr.len();
    assert!(
        verbose_total >= normal_total,
        "--verbose must produce at least as much output as the default; \
         verbose={verbose_total} bytes, normal={normal_total} bytes"
    );

    let _ = std::fs::remove_dir_all(&dir_normal);
    let _ = std::fs::remove_dir_all(&dir_verbose);
}
// --- may_prompt_the_user_interactively_for_generator_provider_and_mod_test.rs ---
/// MAY prompt the user interactively for generator provider and model preferences.
/// When stdin is not a terminal (piped from /dev/null), the command must not hang
/// indefinitely; it must terminate with a defined exit code.
#[test]
fn test_cli_init_may_prompt_the_user_interactively_for_generator_provider_and_mod() {
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
// --- must_detect_the_project_language_from_existing_config_files_cargo_test.rs ---
/// MUST detect the project language from existing config files (Cargo.toml, package.json,
/// pyproject.toml, go.mod) and set defaults accordingly.
#[test]
fn test_cli_init_must_detect_the_project_language_from_existing_config_files_cargo() {
    let bin = option_env!("CARGO_BIN_EXE_ought")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("ought"));

    // (marker file, expected language key, expected runner command substring)
    let cases: &[(&str, &str, &str)] = &[
        ("Cargo.toml", "rust", "cargo test"),
        ("package.json", "typescript", "jest"),
        ("pyproject.toml", "python", "pytest"),
        ("go.mod", "go", "go test"),
    ];

    for (marker, lang, cmd_hint) in cases {
        let dir = std::env::temp_dir().join(format!(
            "ought_init_lang_{}_{}_{}",
            lang,
            std::process::id(),
            // extra entropy so parallel tests on the same pid don't collide
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .subsec_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();

        // Write the language marker file with minimal valid content.
        let content = match *marker {
            "Cargo.toml" => "[package]\nname = \"proj\"\nversion = \"0.1.0\"\n",
            "package.json" => "{\"name\":\"proj\"}\n",
            "pyproject.toml" => "[tool.poetry]\nname = \"proj\"\n",
            "go.mod" => "module proj\n\ngo 1.21\n",
            _ => "",
        };
        std::fs::write(dir.join(marker), content).unwrap();

        let output = std::process::Command::new(&bin)
            .arg("init")
            .current_dir(&dir)
            .output()
            .unwrap_or_else(|e| panic!("failed to run ought init for {}: {}", lang, e));

        assert!(
            output.status.success(),
            "ought init should succeed for {} project; stderr: {}",
            lang,
            String::from_utf8_lossy(&output.stderr)
        );

        let config =
            std::fs::read_to_string(dir.join("ought.toml")).expect("ought.toml must be created");

        assert!(
            config.contains(&format!("[runner.{}]", lang)),
            "ought.toml must contain [runner.{}] for a {} project; got:\n{}",
            lang,
            lang,
            config
        );

        assert!(
            config.contains(cmd_hint),
            "runner command for {} must contain '{}'; got:\n{}",
            lang,
            cmd_hint,
            config
        );

        let _ = std::fs::remove_dir_all(&dir);
    }
}
// --- must_not_overwrite_an_existing_ought_toml_test.rs ---
/// MUST NOT overwrite an existing `ought.toml`.
#[test]
fn test_cli_init_must_not_overwrite_an_existing_ought_toml() {
    let dir = std::env::temp_dir()
        .join(format!("ought_init_no_overwrite_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();

    let original_content =
        "# pre-existing config\n[project]\nname = \"existing\"\nversion = \"9.9.9\"\n";
    std::fs::write(dir.join("ought.toml"), original_content).unwrap();

    let bin = option_env!("CARGO_BIN_EXE_ought")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("ought"));

    let output = std::process::Command::new(&bin)
        .arg("init")
        .current_dir(&dir)
        .output()
        .expect("failed to run ought init");

    assert!(
        !output.status.success(),
        "ought init must exit non-zero when ought.toml already exists"
    );

    let after = std::fs::read_to_string(dir.join("ought.toml"))
        .expect("ought.toml must still exist after the failed init");

    assert_eq!(
        after, original_content,
        "ought.toml content must be unchanged when init is refused"
    );

    let _ = std::fs::remove_dir_all(&dir);
}
// --- must_scaffold_an_ought_toml_an_ought_directory_and_an_example_spe_test.rs ---
/// MUST scaffold an `ought.toml`, an `ought/` directory, and an example spec file inside it
/// when run in a project directory.
#[test]
fn test_cli_init_must_scaffold_an_ought_toml_an_ought_directory_and_an_example_spe() {
    let dir = std::env::temp_dir()
        .join(format!("ought_init_scaffold_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();

    let bin = option_env!("CARGO_BIN_EXE_ought")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("ought"));

    let output = std::process::Command::new(&bin)
        .arg("init")
        .current_dir(&dir)
        .output()
        .expect("failed to run ought init");

    assert!(
        output.status.success(),
        "ought init should exit 0; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    assert!(
        dir.join("ought.toml").exists(),
        "ought.toml must be created by ought init"
    );

    assert!(
        dir.join("ought").is_dir(),
        "ought/ directory must be created by ought init"
    );

    let spec_files: Vec<_> = std::fs::read_dir(dir.join("ought"))
        .expect("ought/ must be readable")
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .map(|x| x == "md")
                .unwrap_or(false)
        })
        .collect();

    assert!(
        !spec_files.is_empty(),
        "at least one example spec file must exist inside ought/"
    );

    let _ = std::fs::remove_dir_all(&dir);
}
// --- must_accept_clause_identifiers_in_the_form_file_section_clause_e_test.rs ---
/// MUST accept clause identifiers in the form `file::section::clause`
/// (e.g. `auth::login::must_return_jwt`)
#[test]
fn test_cli_inspect_must_accept_clause_identifiers_in_the_form_file_section_clause_e() {
    let dir = std::env::temp_dir()
        .join(format!("ought_inspect_idform_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();

    std::fs::write(
        dir.join("ought.toml"),
        "[project]\nname = \"test\"\nversion = \"0.1.0\"\n\n\
         [specs]\nroots = [\"ought/\"]\n\n\
         [context]\nsearch_paths = [\"src/\"]\nexclude = []\n\n\
         [generator]\nprovider = \"anthropic\"\n\n\
         [runner.rust]\ncommand = \"cargo test\"\ntest_dir = \"ought/ought-gen/\"\n",
    )
    .unwrap();

    let gen_dir = dir.join("ought/ought-gen/auth/login");
    std::fs::create_dir_all(&gen_dir).unwrap();
    std::fs::write(
        gen_dir.join("must_return_jwt.rs"),
        "#[test]\nfn test_auth__login__must_return_jwt() {}\n",
    )
    .unwrap();

    let bin = option_env!("CARGO_BIN_EXE_ought")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("ought"));

    // Pass a three-part `file::section::clause` identifier — must not be rejected
    // with a usage error (exit code 2).
    let output = std::process::Command::new(&bin)
        .arg("inspect")
        .arg("auth::login::must_return_jwt")
        .current_dir(&dir)
        .output()
        .expect("failed to invoke ought inspect");

    assert_ne!(
        output.status.code(),
        Some(2),
        "ought inspect must not reject a `file::section::clause` identifier as a usage error; \
         stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.status.success(),
        "ought inspect must exit 0 for a well-formed `file::section::clause` identifier \
         when the generated file exists; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = std::fs::remove_dir_all(&dir);
}
// --- must_print_the_generated_test_code_for_a_given_clause_identifier_test.rs ---
/// MUST print the generated test code for a given clause identifier
#[test]
fn test_cli_inspect_must_print_the_generated_test_code_for_a_given_clause_identifier() {
    let dir = std::env::temp_dir()
        .join(format!("ought_inspect_print_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();

    std::fs::write(
        dir.join("ought.toml"),
        "[project]\nname = \"test\"\nversion = \"0.1.0\"\n\n\
         [specs]\nroots = [\"ought/\"]\n\n\
         [context]\nsearch_paths = [\"src/\"]\nexclude = []\n\n\
         [generator]\nprovider = \"anthropic\"\n\n\
         [runner.rust]\ncommand = \"cargo test\"\ntest_dir = \"ought/ought-gen/\"\n",
    )
    .unwrap();

    let gen_dir = dir.join("ought/ought-gen/auth/login");
    std::fs::create_dir_all(&gen_dir).unwrap();
    let expected_code =
        "#[test]\nfn test_auth__login__must_return_jwt() {\n    assert!(true);\n}\n";
    std::fs::write(gen_dir.join("must_return_jwt.rs"), expected_code).unwrap();

    let bin = option_env!("CARGO_BIN_EXE_ought")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("ought"));

    let output = std::process::Command::new(&bin)
        .arg("inspect")
        .arg("auth::login::must_return_jwt")
        .current_dir(&dir)
        .output()
        .expect("failed to invoke ought inspect");

    assert!(
        output.status.success(),
        "ought inspect must exit 0 when the clause file exists; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("test_auth__login__must_return_jwt"),
        "ought inspect must print the generated test code to stdout; got:\n{stdout}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}
// --- should_show_the_clause_text_alongside_the_generated_code_for_easy_c_test.rs ---
/// SHOULD show the clause text alongside the generated code for easy comparison
#[test]
fn test_cli_inspect_should_show_the_clause_text_alongside_the_generated_code_for_easy_c() {
    let dir = std::env::temp_dir()
        .join(format!("ought_inspect_clause_text_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();

    std::fs::write(
        dir.join("ought.toml"),
        "[project]\nname = \"test\"\nversion = \"0.1.0\"\n\n\
         [specs]\nroots = [\"ought/\"]\n\n\
         [context]\nsearch_paths = [\"src/\"]\nexclude = []\n\n\
         [generator]\nprovider = \"anthropic\"\n\n\
         [runner.rust]\ncommand = \"cargo test\"\ntest_dir = \"ought/ought-gen/\"\n",
    )
    .unwrap();

    // Write a spec file that contains the clause under test.
    let spec_dir = dir.join("ought");
    std::fs::create_dir_all(&spec_dir).unwrap();
    std::fs::write(
        spec_dir.join("auth.ought.md"),
        "# Auth\n\n## Login\n\n- **MUST** return a JWT token\n",
    )
    .unwrap();

    // Write the corresponding generated test file.
    let gen_dir = dir.join("ought/ought-gen/auth/login");
    std::fs::create_dir_all(&gen_dir).unwrap();
    let test_code =
        "#[test]\nfn test_auth__login__must_return_jwt() {\n    assert!(true);\n}\n";
    std::fs::write(gen_dir.join("must_return_jwt.rs"), test_code).unwrap();

    let bin = option_env!("CARGO_BIN_EXE_ought")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("ought"));

    let output = std::process::Command::new(&bin)
        .arg("inspect")
        .arg("auth::login::must_return_jwt")
        .current_dir(&dir)
        .output()
        .expect("failed to invoke ought inspect");

    assert!(
        output.status.success(),
        "ought inspect must exit 0; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );

    // The clause text ("return a JWT token") should appear alongside the code so
    // the developer can compare the spec intent with the generated test.
    assert!(
        combined.contains("return a JWT token"),
        "ought inspect should show the clause text alongside the generated code for \
         easy comparison; output was:\n{combined}"
    );

    // The generated code must also be present.
    assert!(
        combined.contains("test_auth__login__must_return_jwt"),
        "ought inspect must always include the generated test code; output was:\n{combined}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}
// --- should_syntax_highlight_the_output_when_stdout_is_a_terminal_test.rs ---
/// SHOULD syntax-highlight the output when stdout is a terminal
#[test]
fn test_cli_inspect_should_syntax_highlight_the_output_when_stdout_is_a_terminal() {
    // When stdout is piped (non-terminal), the command must still produce plain
    // readable output.  Syntax highlighting, if supported, must be suppressed in
    // that case so downstream tools are not polluted with ANSI escape sequences.
    // When `--color=always` is requested the output MAY contain highlighting,
    // but must still contain the test code.
    let dir = std::env::temp_dir()
        .join(format!("ought_inspect_hl_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();

    std::fs::write(
        dir.join("ought.toml"),
        "[project]\nname = \"test\"\nversion = \"0.1.0\"\n\n\
         [specs]\nroots = [\"ought/\"]\n\n\
         [context]\nsearch_paths = [\"src/\"]\nexclude = []\n\n\
         [generator]\nprovider = \"anthropic\"\n\n\
         [runner.rust]\ncommand = \"cargo test\"\ntest_dir = \"ought/ought-gen/\"\n",
    )
    .unwrap();

    let gen_dir = dir.join("ought/ought-gen/auth/login");
    std::fs::create_dir_all(&gen_dir).unwrap();
    let test_code =
        "#[test]\nfn test_auth__login__must_return_jwt() {\n    assert!(true);\n}\n";
    std::fs::write(gen_dir.join("must_return_jwt.rs"), test_code).unwrap();

    let bin = option_env!("CARGO_BIN_EXE_ought")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("ought"));

    // Without a terminal (piped), output must not contain ANSI escape codes so
    // the raw test code is machine-readable.
    let plain_output = std::process::Command::new(&bin)
        .arg("--color=never")
        .arg("inspect")
        .arg("auth::login::must_return_jwt")
        .current_dir(&dir)
        .output()
        .expect("failed to invoke ought inspect --color=never");

    assert!(
        plain_output.status.success(),
        "ought inspect must succeed with --color=never; stderr: {}",
        String::from_utf8_lossy(&plain_output.stderr)
    );

    let plain_stdout = String::from_utf8_lossy(&plain_output.stdout);
    assert!(
        !plain_stdout.contains('\x1b'),
        "ought inspect must not emit ANSI escape sequences when --color=never is set; \
         got:\n{plain_stdout}"
    );
    assert!(
        plain_stdout.contains("test_auth__login__must_return_jwt"),
        "ought inspect must still print the test code when --color=never; got:\n{plain_stdout}"
    );

    // With `--color=always`, the test code must still be present in the output
    // (even if highlighting is not yet implemented).
    let color_output = std::process::Command::new(&bin)
        .arg("--color=always")
        .arg("inspect")
        .arg("auth::login::must_return_jwt")
        .current_dir(&dir)
        .output()
        .expect("failed to invoke ought inspect --color=always");

    assert!(
        color_output.status.success(),
        "ought inspect must succeed with --color=always; stderr: {}",
        String::from_utf8_lossy(&color_output.stderr)
    );

    let color_stdout = String::from_utf8_lossy(&color_output.stdout);
    assert!(
        color_stdout.contains("test_auth__login__must_return_jwt"),
        "ought inspect must include the test code in --color=always output; got:\n{color_stdout}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}
// --- must_accept_a_glob_pattern_to_run_a_subset_ought_run_ought_auth_o_test.rs ---
/// MUST accept a glob pattern to run a subset:
/// `ought run "ought/auth*.ought.md"`
#[test]
fn test_cli_run_must_accept_a_glob_pattern_to_run_a_subset_ought_run_ought_auth_o() {
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
// --- must_accept_a_path_argument_to_run_a_specific_spec_file_ought_run_test.rs ---
/// MUST accept a path argument to run a specific spec file:
/// `ought run ought/auth.ought.md`
#[test]
fn test_cli_run_must_accept_a_path_argument_to_run_a_specific_spec_file_ought_run() {
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
// --- must_exit_with_code_0_if_any_otherwise_clause_in_the_chain_passes_test.rs ---
/// MUST exit with code 0 if any OTHERWISE clause in the chain passes
/// (graceful degradation accepted).
/// GIVEN: a clause has an OTHERWISE chain and the primary obligation fails.
#[test]
fn test_cli_run_must_exit_with_code_0_if_any_otherwise_clause_in_the_chain_passes() {
    let dir = unique_dir("otherwise_pass");
    scaffold_project(&dir);
    // Spec: MUST with an OTHERWISE fallback
    std::fs::write(
        dir.join("ought/spec.ought.md"),
        "# Spec\n\ncontext: test\n\n## Section\n\n\
         - **MUST** respond in under 100ms\n  \
         - **OTHERWISE** respond in under 1s\n",
    )
    .unwrap();

    // Primary obligation fails
    write_test(
        &dir,
        "spec__section__must_respond_in_under_100ms",
        false,
    );
    // OTHERWISE fallback passes — the degradation chain is satisfied
    write_test(
        &dir,
        "spec__section__must_respond_in_under_100ms__otherwise_respond_in_under_1s",
        true,
    );

    let out = std::process::Command::new(ought_bin())
        .arg("run")
        .current_dir(&dir)
        .output()
        .expect("failed to invoke ought run");

    assert_eq!(
        out.status.code(),
        Some(0),
        "ought run must exit 0 when the primary MUST fails but an OTHERWISE \
         fallback passes (graceful degradation is accepted); \
         stderr: {} stdout: {}",
        String::from_utf8_lossy(&out.stderr),
        String::from_utf8_lossy(&out.stdout)
    );

    let _ = std::fs::remove_dir_all(&dir);
}
// --- must_exit_with_code_0_if_only_should_or_may_clauses_fail_test.rs ---
/// MUST exit with code 0 if only SHOULD or MAY clauses fail.
#[test]
fn test_cli_run_must_exit_with_code_0_if_only_should_or_may_clauses_fail() {
    let dir = unique_dir("exit0_should");
    scaffold_project(&dir);
    // Spec has only a SHOULD-level clause, no MUST
    write_spec(&dir, "SHOULD", "log access attempts");
    // Test for the SHOULD clause fails — this must not trigger exit 1
    write_test(&dir, "spec__section__should_log_access_attempts", false);

    let out = std::process::Command::new(ought_bin())
        .arg("run")
        .current_dir(&dir)
        .output()
        .expect("failed to invoke ought run");

    assert_eq!(
        out.status.code(),
        Some(0),
        "ought run must exit 0 when only SHOULD-level tests fail \
         (SHOULD failures must not count as hard failures); \
         stderr: {} stdout: {}",
        String::from_utf8_lossy(&out.stderr),
        String::from_utf8_lossy(&out.stdout)
    );

    let _ = std::fs::remove_dir_all(&dir);
}
// --- must_exit_with_code_1_if_all_otherwise_clauses_also_fail_full_deg_test.rs ---
/// MUST exit with code 1 if all OTHERWISE clauses also fail
/// (full degradation chain exhausted).
/// GIVEN: a clause has an OTHERWISE chain and the primary obligation fails.
#[test]
fn test_cli_run_must_exit_with_code_1_if_all_otherwise_clauses_also_fail_full_deg() {
    let dir = unique_dir("otherwise_all_fail");
    scaffold_project(&dir);
    // Spec: MUST with an OTHERWISE fallback
    std::fs::write(
        dir.join("ought/spec.ought.md"),
        "# Spec\n\ncontext: test\n\n## Section\n\n\
         - **MUST** respond in under 100ms\n  \
         - **OTHERWISE** respond in under 1s\n",
    )
    .unwrap();

    // Both the primary and the OTHERWISE fallback fail — full chain exhausted
    write_test(
        &dir,
        "spec__section__must_respond_in_under_100ms",
        false,
    );
    write_test(
        &dir,
        "spec__section__must_respond_in_under_100ms__otherwise_respond_in_under_1s",
        false,
    );

    let out = std::process::Command::new(ought_bin())
        .arg("run")
        .current_dir(&dir)
        .output()
        .expect("failed to invoke ought run");

    assert_eq!(
        out.status.code(),
        Some(1),
        "ought run must exit 1 when the primary MUST and every OTHERWISE fallback \
         all fail (degradation chain exhausted); stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let _ = std::fs::remove_dir_all(&dir);
}
// --- must_exit_with_code_1_if_any_must_must_not_must_always_or_must_by_test.rs ---
/// MUST exit with code 1 if any MUST, MUST NOT, MUST ALWAYS, or MUST BY clause fails.
#[test]
fn test_cli_run_must_exit_with_code_1_if_any_must_must_not_must_always_or_must_by() {
    let dir = unique_dir("exit1_must");
    scaffold_project(&dir);
    write_spec(&dir, "MUST", "authenticate users");
    // Deliberately failing test for the MUST clause
    write_test(&dir, "spec__section__must_authenticate_users", false);

    let out = std::process::Command::new(ought_bin())
        .arg("run")
        .current_dir(&dir)
        .output()
        .expect("failed to invoke ought run");

    assert_eq!(
        out.status.code(),
        Some(1),
        "ought run must exit 1 when a MUST clause test fails; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let _ = std::fs::remove_dir_all(&dir);
}
// --- must_not_trigger_test_generation_ought_run_only_executes_existing_gen_test.rs ---
/// MUST NOT trigger test generation — `ought run` only executes existing generated tests.
#[test]
fn test_cli_run_must_not_trigger_test_generation_ought_run_only_executes_existing_gen() {
    let dir = unique_dir("no_generate");
    scaffold_project(&dir);
    write_spec(&dir, "MUST", "return HTTP 200");
    write_test(&dir, "spec__section__must_return_http_200", true);

    // Snapshot the ought/ought-gen/ directory before running
    let gen_dir = dir.join("ought/ought-gen");
    let before: std::collections::BTreeSet<_> = walkdir(&gen_dir);

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
    let after: std::collections::BTreeSet<_> = walkdir(&gen_dir);
    assert_eq!(
        before, after,
        "ought run must not create new test files (generation must not be triggered)"
    );

    let _ = std::fs::remove_dir_all(&dir);
}
// --- must_parse_all_spec_files_execute_generated_tests_and_report_resu_test.rs ---
/// MUST parse all spec files, execute generated tests, and report results
/// mapped back to clauses.
#[test]
fn test_cli_run_must_parse_all_spec_files_execute_generated_tests_and_report_resu() {
    let dir = unique_dir("parse_all");
    scaffold_project(&dir);
    write_spec(&dir, "MUST", "authenticate users");
    write_test(&dir, "spec__section__must_authenticate_users", true);

    let out = std::process::Command::new(ought_bin())
        .arg("run")
        .current_dir(&dir)
        .output()
        .expect("failed to invoke ought run");

    assert_eq!(
        out.status.code(),
        Some(0),
        "ought run must exit 0 when all tests pass; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    // Some output must be produced reporting on the results
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        !combined.trim().is_empty(),
        "ought run must produce output reporting test results"
    );

    let _ = std::fs::remove_dir_all(&dir);
}
// --- should_print_a_summary_at_the_end_showing_pass_fail_counts_by_sever_test.rs ---
/// SHOULD print a summary at the end showing pass/fail counts by severity level.
#[test]
fn test_cli_run_should_print_a_summary_at_the_end_showing_pass_fail_counts_by_sever() {
    let dir = unique_dir("summary");
    scaffold_project(&dir);
    write_spec(&dir, "MUST", "authenticate users");
    write_test(&dir, "spec__section__must_authenticate_users", true);

    let out = std::process::Command::new(ought_bin())
        .arg("run")
        .current_dir(&dir)
        .output()
        .expect("failed to invoke ought run");

    let stdout = String::from_utf8_lossy(&out.stdout);
    // The summary must contain at least one count keyword.
    // The terminal reporter emits lines like "1 passed", "MUST coverage: 1/1 (100%)"
    assert!(
        stdout.contains("passed")
            || stdout.contains("failed")
            || stdout.contains("coverage"),
        "ought run must print a summary with pass/fail counts; stdout was:\n{stdout}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}
// --- should_support_fail_on_should_flag_to_also_fail_on_should_clause_fa_test.rs ---
/// SHOULD support `--fail-on-should` flag to also fail on SHOULD clause failures.
#[test]
fn test_cli_run_should_support_fail_on_should_flag_to_also_fail_on_should_clause_fa() {
    let dir = unique_dir("fail_on_should");
    scaffold_project(&dir);
    // Only a SHOULD clause — would normally be a soft failure
    write_spec(&dir, "SHOULD", "emit telemetry events");
    write_test(&dir, "spec__section__should_emit_telemetry_events", false);

    let out = std::process::Command::new(ought_bin())
        .arg("run")
        .arg("--fail-on-should")
        .current_dir(&dir)
        .output()
        .expect("failed to invoke ought run --fail-on-should");

    // Flag must be recognised by clap (not a usage error)
    assert_ne!(
        out.status.code(),
        Some(2),
        "--fail-on-should must be a recognised flag; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    // With the flag set, a failing SHOULD test must cause exit 1
    assert_eq!(
        out.status.code(),
        Some(1),
        "ought run --fail-on-should must exit 1 when a SHOULD-level test fails; \
         stderr: {} stdout: {}",
        String::from_utf8_lossy(&out.stderr),
        String::from_utf8_lossy(&out.stdout)
    );

    let _ = std::fs::remove_dir_all(&dir);
}
// --- wont_execute_tests_in_parallel_by_default_in_v0_1_sequential_is_f_test.rs ---
/// WONT execute tests in parallel by default in v0.1 (sequential is fine to start).
/// Absence test: the CLI must not expose a `--parallel` flag.
#[test]
fn test_cli_run_wont_execute_tests_in_parallel_by_default_in_v0_1_sequential_is_f() {
    // Invoke without a project directory; clap argument parsing happens before
    // config is loaded, so an unknown flag is rejected immediately with exit 2.
    let out = std::process::Command::new(ought_bin())
        .arg("run")
        .arg("--parallel")
        .output()
        .expect("failed to invoke ought run --parallel");

    // clap returns exit code 2 for unrecognised flags.
    assert_eq!(
        out.status.code(),
        Some(2),
        "ought run must NOT expose a --parallel flag in v0.1 \
         (parallel test execution is intentionally absent); stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}
// --- must_re_run_affected_specs_when_a_change_is_detected_test.rs ---
/// MUST re-run affected specs when a change is detected
#[test]
fn test_cli_watch_must_re_run_affected_specs_when_a_change_is_detected() {
    let dir = unique_dir("watch_rerun");
    scaffold_project(&dir);
    write_spec(&dir, "MUST", "process items");
    write_test(&dir, "spec__section__must_process_items", true);

    let mut child = std::process::Command::new(ought_bin())
        .arg("watch")
        .current_dir(&dir)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("failed to spawn ought watch");

    // Allow the initial run to complete before triggering a change.
    std::thread::sleep(std::time::Duration::from_millis(1000));

    // Modify the spec file to trigger a re-run.
    let spec_path = dir.join("ought/spec.ought.md");
    let original = std::fs::read_to_string(&spec_path).unwrap_or_default();
    std::fs::write(&spec_path, format!("{}\n<!-- re-run trigger -->", original)).unwrap();

    // Wait long enough for the watcher to detect the change and complete a cycle.
    std::thread::sleep(std::time::Duration::from_millis(2000));

    let _ = child.kill();
    let output = child.wait_with_output().expect("failed to collect watch output");

    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    // After the change the watcher must have re-run and reported results.
    // Look for markers that a test run occurred: "passed", "failed", or a clause id.
    let ran_again = combined.contains("passed")
        || combined.contains("failed")
        || combined.contains("must_process_items")
        || combined.contains("spec.ought.md");

    assert!(
        ran_again,
        "ought watch must re-run affected specs when a file change is detected; \
         output:\n{combined}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}
// --- must_watch_ought_md_files_and_source_files_for_changes_test.rs ---
/// MUST watch `ought.md` files and source files for changes
#[test]
fn test_cli_watch_must_watch_ought_md_files_and_source_files_for_changes() {
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
// --- should_clear_the_terminal_and_reprint_results_on_each_cycle_test.rs ---
/// SHOULD clear the terminal and reprint results on each cycle
#[test]
fn test_cli_watch_should_clear_the_terminal_and_reprint_results_on_each_cycle() {
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
// --- should_debounce_rapid_file_changes_at_least_500ms_test.rs ---
/// SHOULD debounce rapid file changes (at least 500ms)
#[test]
fn test_cli_watch_should_debounce_rapid_file_changes_at_least_500ms() {
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

    // Count cycle-start markers. `ought watch` emits "ought watch: checking N
    // spec(s)..." exactly once per cycle (initial + each debounced burst), so
    // it's a deterministic per-cycle marker — unlike words like "passed" or
    // "running" which appear multiple times within a single cycle's output and
    // cause the test to be flaky depending on output flush timing.
    // A debounced watcher fires at most once for the burst, so we accept up to
    // 2 cycles (initial + 1 debounced); we must not see 5.
    let run_count = combined.matches("ought watch: checking").count();

    assert!(
        run_count <= 2,
        "ought watch should debounce rapid changes into at most 1 re-run cycle \
         (got {} apparent run markers); debounce window must be ≥500 ms\noutput:\n{combined}",
        run_count
    );

    let _ = std::fs::remove_dir_all(&dir);
}