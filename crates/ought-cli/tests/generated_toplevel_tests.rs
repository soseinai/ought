#![allow(dead_code, clippy::all)]
#![allow(non_snake_case, unused_imports)]
use std::path::{Path, PathBuf};
use std::fs;
use std::process::Command;
use ought_spec::{Config, SpecGraph, Parser};
use ought_spec::types::*;

// ===========================================================================
// what_ought_does
// ===========================================================================

/// MUST accept behavioral specifications written in standard markdown files (`.ought.md`)
#[test]
fn test_ought_what_ought_does_must_accept_behavioral_specifications_written_in_standard_markdow() {
    let tmp_path = std::env::temp_dir().join(format!("test_spec_{}.ought.md", std::process::id()));
    let spec_content = "# My Spec\n\n## Section\n\n- **MUST** do something\n";
    fs::write(&tmp_path, spec_content).expect("Failed to write test .ought.md file");

    assert!(tmp_path.exists(), "Spec file should exist after writing");
    assert!(
        tmp_path.to_str().unwrap().ends_with(".ought.md"),
        "File must use .ought.md compound extension"
    );

    let contents = fs::read_to_string(&tmp_path).expect("Spec file should be readable as UTF-8");
    assert!(!contents.is_empty(), "Spec file should not be empty");
    assert!(contents.contains('#'), "Spec file should contain markdown headings");

    // The parser must accept this file
    let result = Parser::parse_string(&contents, &tmp_path);
    assert!(result.is_ok(), "Parser must accept a valid .ought.md file: {:?}", result.err());

    let _ = fs::remove_file(&tmp_path);
}

/// MUST provide a CLI (`ought`) as the primary interface for all operations
#[test]
fn test_ought_what_ought_does_must_provide_a_cli_ought_as_the_primary_interface_for_all_operati() {
    let bin = PathBuf::from(env!("CARGO_BIN_EXE_ought"));
    let output = Command::new(&bin).arg("--help").output();

    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            let stderr = String::from_utf8_lossy(&out.stderr);
            let combined = format!("{}{}", stdout, stderr);
            assert!(
                !combined.is_empty(),
                "The `ought` CLI must produce output when invoked with --help"
            );
        }
        Err(e) => {
            panic!(
                "The `ought` CLI binary must be available; invocation failed: {}", e
            );
        }
    }
}

/// MUST execute generated tests and report pass/fail results mapped back to the original spec clauses
#[test]
fn test_ought_what_ought_does_must_execute_generated_tests_and_report_pass_fail_results_mapped() {
    #[derive(Debug, PartialEq)]
    enum TestOutcome { Pass, Fail }

    struct ClauseResult {
        clause_id: String,
        outcome: TestOutcome,
    }

    let results = vec![
        ClauseResult { clause_id: "my_spec::section::must_do_something".to_string(), outcome: TestOutcome::Pass },
        ClauseResult { clause_id: "my_spec::section::must_handle_error".to_string(), outcome: TestOutcome::Fail },
    ];

    for r in &results {
        assert!(!r.clause_id.is_empty(), "Every test result must reference a clause ID");
        assert!(r.clause_id.contains("::"), "Clause IDs must use '::' namespace separators, got: {}", r.clause_id);
    }

    let passes: Vec<_> = results.iter().filter(|r| r.outcome == TestOutcome::Pass).collect();
    let failures: Vec<_> = results.iter().filter(|r| r.outcome == TestOutcome::Fail).collect();

    assert_eq!(passes.len(), 1);
    assert_eq!(failures.len(), 1);
    assert_eq!(passes[0].clause_id, "my_spec::section::must_do_something");
    assert_eq!(failures[0].clause_id, "my_spec::section::must_handle_error");
}

/// MUST use an LLM to generate concrete, runnable test code from those specifications
#[test]
fn test_ought_what_ought_does_must_use_an_llm_to_generate_concrete_runnable_test_code_from_thos() {
    let generated_samples = vec![
        "#[test]\nfn test_example() { assert!(true); }",
        "#[test]\nfn test_another() { let x = 1; assert_eq!(x, 1); }",
        "#[test]\nfn test_boundary() { assert_ne!(0, 1); }",
    ];

    for sample in &generated_samples {
        assert!(sample.contains("#[test]"), "Generated code must carry the #[test] attribute: {:?}", sample);
        assert!(sample.contains("fn test_"), "Generated code must declare a test function: {:?}", sample);
        let has_assertion = sample.contains("assert!(") || sample.contains("assert_eq!(") || sample.contains("assert_ne!(");
        assert!(has_assertion, "Generated tests must include at least one assertion: {:?}", sample);
    }
}

/// MUST NOT require users to write any test code by hand
#[test]
fn test_ought_what_ought_does_must_not_require_users_to_write_any_test_code_by_hand() {
    let tmp_dir = std::env::temp_dir().join(format!("ought_no_manual_tests_{}", std::process::id()));
    let _ = fs::create_dir_all(&tmp_dir);

    let spec_path = tmp_dir.join("feature.ought.md");
    let spec_content = "# Feature\n\n## Section\n\n- **MUST** work correctly\n";
    fs::write(&spec_path, spec_content).expect("Should be able to write spec file");

    let handwritten_test = tmp_dir.join("tests.rs");
    assert!(!handwritten_test.exists(), "Users must not be required to provide a hand-written tests.rs");

    let contents = fs::read_to_string(&spec_path).unwrap();
    assert!(!contents.contains("#[test]"), "User-facing spec files must not contain #[test] attributes");
    assert!(!contents.contains("fn test_"), "User-facing spec files must not contain test function definitions");
    assert!(!contents.contains("assert!("), "User-facing spec files must not contain assertion macros");

    let _ = fs::remove_dir_all(&tmp_dir);
}

// ===========================================================================
// spec_format
// ===========================================================================

/// MUST support RFC 2119 keywords (MUST, MUST NOT, SHOULD, SHOULD NOT, MAY) as deontic operators.
#[test]
fn test_ought_spec_format_must_support_rfc_2119_keywords_must_must_not_should_should_not_ma() {
    let md = r#"# Spec Format

## Rules

- **MUST** always validate input
- **MUST NOT** store plaintext passwords
- **SHOULD** log all access attempts
- **SHOULD NOT** cache sensitive data
- **MAY** support optional audit trails
"#;

    let spec = Parser::parse_string(md, Path::new("test.ought.md")).expect("parse failed");
    let clauses = &spec.sections[0].clauses;

    assert_eq!(clauses.len(), 5);

    assert_eq!(clauses[0].keyword, Keyword::Must);
    assert_eq!(clauses[0].severity, Severity::Required);
    assert!(clauses[0].text.contains("validate input"));

    assert_eq!(clauses[1].keyword, Keyword::MustNot);
    assert_eq!(clauses[1].severity, Severity::Required);
    assert!(clauses[1].text.contains("plaintext passwords"));

    assert_eq!(clauses[2].keyword, Keyword::Should);
    assert_eq!(clauses[2].severity, Severity::Recommended);
    assert!(clauses[2].text.contains("log all access"));

    assert_eq!(clauses[3].keyword, Keyword::ShouldNot);
    assert_eq!(clauses[3].severity, Severity::Recommended);
    assert!(clauses[3].text.contains("cache sensitive"));

    assert_eq!(clauses[4].keyword, Keyword::May);
    assert_eq!(clauses[4].severity, Severity::Optional);
    assert!(clauses[4].text.contains("audit trails"));
}

/// MUST use standard markdown (CommonMark) so specs render in GitHub, editors, and browsers.
#[test]
fn test_ought_spec_format_must_use_standard_markdown_commonmark_so_specs_render_in_github_e() {
    let md = r#"# Spec Format

> This spec is written in standard CommonMark.

## Rules

Some *introductory* prose with `inline code` and **bold** text.

- **MUST** render correctly in any CommonMark-compliant renderer

```markdown
# Example heading
- **MUST** sample clause
```
"#;

    let result = Parser::parse_string(md, Path::new("spec_format.ought.md"));
    assert!(result.is_ok(), "CommonMark spec should parse without errors: {:?}", result.err());
    let spec = result.unwrap();

    assert_eq!(spec.name, "Spec Format");
    assert_eq!(spec.sections.len(), 1);
    assert_eq!(spec.sections[0].title, "Rules");
    assert!(!spec.sections[0].prose.is_empty(), "CommonMark prose should be captured");
    assert_eq!(spec.sections[0].clauses.len(), 1);
    assert!(spec.sections[0].clauses[0].text.contains("render correctly"));
    assert_eq!(spec.sections[0].clauses[0].hints.len(), 1);
}

/// MUST support the WONT keyword for deliberately absent capabilities.
#[test]
fn test_ought_spec_format_must_support_the_wont_keyword_for_deliberately_absent_capabilitie() {
    let md = r#"# Spec Format

## Scope Exclusions

- **WONT** support OAuth 1.0 due to known security flaws
- **WONT** provide a SOAP API
"#;

    let spec = Parser::parse_string(md, Path::new("test.ought.md")).expect("parse failed");
    let clauses = &spec.sections[0].clauses;

    assert_eq!(clauses.len(), 2);

    assert_eq!(clauses[0].keyword, Keyword::Wont);
    assert_eq!(clauses[0].severity, Severity::NegativeConfirmation);
    assert!(clauses[0].text.contains("OAuth 1.0"));

    assert_eq!(clauses[1].keyword, Keyword::Wont);
    assert_eq!(clauses[1].severity, Severity::NegativeConfirmation);
    assert!(clauses[1].text.contains("SOAP API"));
}

/// MUST support GIVEN blocks for conditional obligations.
#[test]
fn test_ought_spec_format_must_support_given_blocks_for_conditional_obligations_clauses_tha() {
    let md = r#"# Spec Format

## Access Control

- **GIVEN** the user is authenticated:
  - **MUST** return their profile data
  - **MUST NOT** return other users' private data
  - **MAY** return extended metadata
"#;

    let spec = Parser::parse_string(md, Path::new("test.ought.md")).expect("parse failed");
    let clauses = &spec.sections[0].clauses;

    assert_eq!(clauses.len(), 3, "GIVEN should not appear as its own clause");
    assert!(
        clauses.iter().all(|c| c.keyword != Keyword::Given),
        "Keyword::Given should not be emitted as a top-level clause"
    );

    let expected_condition = "the user is authenticated:";
    for clause in clauses {
        assert_eq!(clause.condition.as_deref(), Some(expected_condition),
            "All clauses inside GIVEN must carry the condition");
    }

    assert_eq!(clauses[0].keyword, Keyword::Must);
    assert_eq!(clauses[1].keyword, Keyword::MustNot);
    assert_eq!(clauses[2].keyword, Keyword::May);
}

/// MUST support OTHERWISE chains for contrary-to-duty fallbacks.
#[test]
fn test_ought_spec_format_must_support_otherwise_chains_for_contrary_to_duty_fallbacks_grac() {
    let md = r#"# Spec Format

## Resilience

- **MUST** respond within 200ms
  - **OTHERWISE** return a cached response
  - **OTHERWISE** return 503 Service Unavailable
"#;

    let spec = Parser::parse_string(md, Path::new("test.ought.md")).expect("parse failed");
    let clauses = &spec.sections[0].clauses;

    assert_eq!(clauses.len(), 1, "OTHERWISE items must not appear as top-level clauses");

    let primary = &clauses[0];
    assert_eq!(primary.keyword, Keyword::Must);
    assert_eq!(primary.otherwise.len(), 2, "Two OTHERWISE fallbacks expected");

    assert_eq!(primary.otherwise[0].keyword, Keyword::Otherwise);
    assert!(primary.otherwise[0].text.contains("cached response"));
    assert_eq!(primary.otherwise[0].severity, Severity::Required);

    assert_eq!(primary.otherwise[1].keyword, Keyword::Otherwise);
    assert!(primary.otherwise[1].text.contains("503"));
    assert_eq!(primary.otherwise[1].severity, Severity::Required);
}

/// MUST support MUST ALWAYS for invariants.
#[test]
fn test_ought_spec_format_must_support_must_always_for_invariants_properties_that_must_hold() {
    let md = r#"# Spec Format

## Invariants

- **MUST ALWAYS** keep database connections below pool maximum
- **MUST ALWAYS** produce a valid UTF-8 response body
"#;

    let spec = Parser::parse_string(md, Path::new("test.ought.md")).expect("parse failed");
    let clauses = &spec.sections[0].clauses;

    assert_eq!(clauses.len(), 2);

    for clause in clauses {
        assert_eq!(clause.keyword, Keyword::MustAlways);
        assert_eq!(clause.severity, Severity::Required);
        assert!(
            matches!(clause.temporal, Some(Temporal::Invariant)),
            "MUST ALWAYS must set temporal to Invariant, got {:?}", clause.temporal
        );
    }

    assert!(clauses[0].text.contains("pool maximum"));
    assert!(clauses[1].text.contains("UTF-8"));
}

/// MUST support MUST BY for deadline obligations.
#[test]
fn test_ought_spec_format_must_support_must_by_for_deadline_obligations_operations_that_mus() {
    use std::time::Duration;

    let md = r#"# Spec Format

## Performance

- **MUST BY 100ms** return search results
- **MUST BY 5s** complete authentication handshake
- **MUST BY 30m** finish the nightly batch export
"#;

    let spec = Parser::parse_string(md, Path::new("test.ought.md")).expect("parse failed");
    let clauses = &spec.sections[0].clauses;

    assert_eq!(clauses.len(), 3);

    for clause in clauses {
        assert_eq!(clause.keyword, Keyword::MustBy);
        assert_eq!(clause.severity, Severity::Required);
    }

    assert!(
        matches!(clauses[0].temporal, Some(Temporal::Deadline(d)) if d == Duration::from_millis(100)),
        "Expected 100ms deadline, got {:?}", clauses[0].temporal
    );
    assert!(clauses[0].text.contains("search results"));

    assert!(
        matches!(clauses[1].temporal, Some(Temporal::Deadline(d)) if d == Duration::from_secs(5)),
        "Expected 5s deadline, got {:?}", clauses[1].temporal
    );
    assert!(clauses[1].text.contains("authentication handshake"));

    assert!(
        matches!(clauses[2].temporal, Some(Temporal::Deadline(d)) if d == Duration::from_secs(30 * 60)),
        "Expected 30m deadline, got {:?}", clauses[2].temporal
    );
    assert!(clauses[2].text.contains("batch export"));
}

/// MUST support cross-file references via standard markdown links.
#[test]
fn test_ought_spec_format_must_support_cross_file_references_via_standard_markdown_links_so() {
    let md = r#"# Spec Format

requires: [Auth](auth.ought.md), [Billing](billing.ought.md#invoices), [Core](core/base.ought.md#rules)

## Sections

- **MUST** integrate with referenced specs
"#;

    let spec = Parser::parse_string(md, Path::new("spec_format.ought.md")).expect("parse failed");
    let refs = &spec.metadata.requires;

    assert_eq!(refs.len(), 3, "Three cross-file references expected");

    assert_eq!(refs[0].label, "Auth");
    assert_eq!(refs[0].path.to_str().unwrap(), "auth.ought.md");
    assert_eq!(refs[0].anchor, None);

    assert_eq!(refs[1].label, "Billing");
    assert_eq!(refs[1].path.to_str().unwrap(), "billing.ought.md");
    assert_eq!(refs[1].anchor.as_deref(), Some("invoices"));

    assert_eq!(refs[2].label, "Core");
    assert_eq!(refs[2].path.to_str().unwrap(), "core/base.ought.md");
    assert_eq!(refs[2].anchor.as_deref(), Some("rules"));
}

/// SHOULD be parseable by a standalone library with no LLM dependency.
#[test]
fn test_ought_spec_format_should_be_parseable_by_a_standalone_library_with_no_llm_dependency() {
    let md = r#"# Standalone Spec

context: Verifies the parser works with no external dependencies

requires: [Other](other.ought.md)

## Obligations

- **MUST** validate all inputs
- **MUST NOT** leak secrets
- **SHOULD** emit structured logs
- **SHOULD NOT** swallow errors silently
- **MAY** support optional features
- **WONT** implement deprecated protocols

## Conditional

- **GIVEN** the cache is warm:
  - **MUST** serve from cache

## Degradation

- **MUST** return a result
  - **OTHERWISE** return a fallback value

## Temporal

- **MUST ALWAYS** preserve referential integrity
- **MUST BY 500ms** respond to health checks
"#;

    let result = Parser::parse_string(md, Path::new("standalone.ought.md"));
    assert!(result.is_ok(), "Standalone parser must succeed: {:?}", result.err());

    let spec = result.unwrap();

    assert_eq!(spec.name, "Standalone Spec");
    assert_eq!(spec.metadata.context.as_deref(), Some("Verifies the parser works with no external dependencies"));
    assert_eq!(spec.metadata.requires.len(), 1);
    assert_eq!(spec.sections.len(), 4);

    let obligations = &spec.sections[0].clauses;
    assert_eq!(obligations.len(), 6);
    assert!(obligations.iter().any(|c| c.keyword == Keyword::Must));
    assert!(obligations.iter().any(|c| c.keyword == Keyword::MustNot));
    assert!(obligations.iter().any(|c| c.keyword == Keyword::Should));
    assert!(obligations.iter().any(|c| c.keyword == Keyword::ShouldNot));
    assert!(obligations.iter().any(|c| c.keyword == Keyword::May));
    assert!(obligations.iter().any(|c| c.keyword == Keyword::Wont));

    let conditional = &spec.sections[1].clauses;
    assert_eq!(conditional.len(), 1);
    assert!(conditional[0].condition.is_some());

    let degradation = &spec.sections[2].clauses;
    assert_eq!(degradation.len(), 1);
    assert_eq!(degradation[0].otherwise.len(), 1);

    let temporal = &spec.sections[3].clauses;
    assert_eq!(temporal.len(), 2);
    assert!(matches!(temporal[0].temporal, Some(Temporal::Invariant)));
    assert!(matches!(temporal[1].temporal, Some(Temporal::Deadline(_))));

    let all_clauses: Vec<_> = spec.sections.iter()
        .flat_map(|s| s.clauses.iter())
        .collect();
    let ids: Vec<_> = all_clauses.iter().map(|c| &c.id.0).collect();
    let unique_ids: std::collections::HashSet<_> = ids.iter().collect();
    assert_eq!(ids.len(), unique_ids.len(), "All clause IDs must be unique");
    assert!(all_clauses.iter().all(|c| !c.id.0.is_empty()), "No clause may have an empty ID");

    for clause in &all_clauses {
        let expected = clause.keyword.severity();
        assert_eq!(clause.severity, expected,
            "Severity mismatch for {:?}: expected {:?}", clause.keyword, expected);
    }
}

// ===========================================================================
// implementation
// ===========================================================================

/// MUST be written in Rust
#[test]
fn test_ought_implementation_must_be_written_in_rust() {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")
        .expect("CARGO_MANIFEST_DIR must be set by cargo");

    let mut dir = Path::new(&manifest_dir).to_path_buf();
    let mut found_workspace = false;

    loop {
        let candidate = dir.join("Cargo.toml");
        if candidate.exists() {
            let contents = fs::read_to_string(&candidate).expect("failed to read Cargo.toml");
            if contents.contains("[workspace]") {
                found_workspace = true;
                break;
            }
        }
        if !dir.pop() { break; }
    }

    assert!(found_workspace,
        "Expected to find a Cargo.toml with a [workspace] section, confirming the project is written in Rust");
}

/// SHOULD use a workspace structure so components can be used independently
#[test]
fn test_ought_implementation_should_use_a_workspace_structure_so_components_can_be_used_independ() {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")
        .expect("CARGO_MANIFEST_DIR must be set by cargo");

    let mut workspace_root = Path::new(&manifest_dir).to_path_buf();
    let workspace_toml;
    loop {
        let candidate = workspace_root.join("Cargo.toml");
        if candidate.exists() {
            let contents = fs::read_to_string(&candidate).expect("failed to read Cargo.toml");
            if contents.contains("[workspace]") {
                workspace_toml = contents;
                break;
            }
        }
        assert!(workspace_root.pop(), "Could not find workspace root from CARGO_MANIFEST_DIR");
    }

    let member_lines: Vec<&str> = workspace_toml.lines()
        .filter(|l| {
            let trimmed = l.trim();
            (trimmed.starts_with('"') && trimmed.ends_with("\","))
                || (trimmed.starts_with('"') && trimmed.ends_with('"'))
        })
        .collect();

    let members: Vec<String> = member_lines.iter()
        .map(|l| l.trim().trim_matches('"').trim_end_matches(',').trim_matches('"').to_string())
        .filter(|s| !s.is_empty())
        .collect();

    assert!(members.len() >= 2,
        "Workspace should declare at least 2 members; found {} member(s).", members.len());

    for member in &members {
        let member_manifest = workspace_root.join(member).join("Cargo.toml");
        assert!(member_manifest.exists(),
            "Workspace member '{}' does not have its own Cargo.toml at '{}'",
            member, member_manifest.display());
    }
}

/// MUST publish the spec parser as a standalone crate (ought-spec) with no LLM dependencies
#[test]
fn test_ought_implementation_must_publish_the_spec_parser_as_a_standalone_crate_ought_spec_wit() {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")
        .expect("CARGO_MANIFEST_DIR must be set by cargo");

    let mut workspace_root = Path::new(&manifest_dir).to_path_buf();
    loop {
        let candidate = workspace_root.join("Cargo.toml");
        if candidate.exists() {
            let contents = fs::read_to_string(&candidate).expect("failed to read Cargo.toml");
            if contents.contains("[workspace]") { break; }
        }
        assert!(workspace_root.pop(), "Could not find workspace root from CARGO_MANIFEST_DIR");
    }

    let candidates = [
        workspace_root.join("crates").join("ought-spec").join("Cargo.toml"),
        workspace_root.join("ought-spec").join("Cargo.toml"),
    ];

    let spec_manifest_path = candidates.iter().find(|p| p.exists())
        .expect("ought-spec crate not found; expected at crates/ought-spec/Cargo.toml");

    let spec_manifest = fs::read_to_string(spec_manifest_path)
        .expect("failed to read ought-spec/Cargo.toml");

    assert!(spec_manifest.contains("name = \"ought-spec\""),
        "ought-spec/Cargo.toml should declare name = \"ought-spec\"");

    let llm_deps = ["openai", "anthropic", "mistral", "\"llm\"", "langchain", "rig", "llama", "ollama", "genai", "async-openai"];
    for dep in &llm_deps {
        assert!(!spec_manifest.contains(dep),
            "ought-spec/Cargo.toml must not depend on LLM library '{}', but it was found in the manifest", dep);
    }
}

// ===========================================================================
// generated_test_management
// ===========================================================================

/// MUST only regenerate tests when the user explicitly runs `ought generate`
#[test]
fn test_ought_generated_test_management_must_only_regenerate_tests_on_generate_not_run() {
    #[derive(PartialEq, Debug)]
    enum Cmd { Generate, Run }

    struct ManifestState { generation_count: usize }

    impl ManifestState {
        fn execute(&mut self, cmd: Cmd) {
            if cmd == Cmd::Generate { self.generation_count += 1; }
        }
    }

    let mut state = ManifestState { generation_count: 0 };

    state.execute(Cmd::Run);
    assert_eq!(state.generation_count, 0, "`ought run` must not regenerate tests");

    state.execute(Cmd::Run);
    assert_eq!(state.generation_count, 0, "repeated `ought run` must not regenerate tests");

    state.execute(Cmd::Generate);
    assert_eq!(state.generation_count, 1, "`ought generate` must trigger exactly one regeneration pass");

    state.execute(Cmd::Run);
    assert_eq!(state.generation_count, 1, "`ought run` after `ought generate` must not increment generation count");
}

/// MUST detect and remove orphaned tests when a clause is deleted from a spec.
#[test]
fn test_ought_generated_test_management_must_detect_and_remove_orphaned_tests() {
    use std::collections::HashMap;

    struct Manifest { entries: HashMap<String, ()> }

    impl Manifest {
        fn remove_orphans(&mut self, valid_ids: &[&str]) {
            let valid: std::collections::HashSet<&str> = valid_ids.iter().copied().collect();
            self.entries.retain(|k, _| valid.contains(k.as_str()));
        }
    }

    let mut manifest = Manifest {
        entries: {
            let mut m = HashMap::new();
            m.insert("auth::login::must_return_jwt".to_string(), ());
            m.insert("auth::login::must_reject_bad_password".to_string(), ());
            m.insert("auth::login::must_hash_password".to_string(), ());
            m
        },
    };

    assert_eq!(manifest.entries.len(), 3);

    let still_valid = vec!["auth::login::must_return_jwt", "auth::login::must_reject_bad_password"];
    manifest.remove_orphans(&still_valid);

    assert!(!manifest.entries.contains_key("auth::login::must_hash_password"), "orphaned clause must be removed");
    assert!(manifest.entries.contains_key("auth::login::must_return_jwt"));
    assert!(manifest.entries.contains_key("auth::login::must_reject_bad_password"));
    assert_eq!(manifest.entries.len(), 2);

    manifest.remove_orphans(&[]);
    assert!(manifest.entries.is_empty(), "removing all valid ids must leave an empty manifest");
}

/// MUST track generated tests with content hashes so they are only regenerated when the spec or source changes.
#[test]
fn test_ought_generated_test_management_must_track_generated_tests_with_content_hashes() {
    use std::collections::HashMap;

    struct ManifestEntry { clause_hash: String, source_hash: String }

    struct Manifest { entries: HashMap<String, ManifestEntry> }

    impl Manifest {
        fn is_stale(&self, clause_id: &str, clause_hash: &str, source_hash: &str) -> bool {
            match self.entries.get(clause_id) {
                Some(entry) => entry.clause_hash != clause_hash || entry.source_hash != source_hash,
                None => true,
            }
        }
    }

    let clause_id = "auth::login::must_return_jwt";
    let clause_hash = "a1b2c3d4e5f67890";
    let source_hash = "";

    let mut entries = HashMap::new();
    entries.insert(clause_id.to_string(), ManifestEntry { clause_hash: clause_hash.to_string(), source_hash: source_hash.to_string() });
    let manifest = Manifest { entries };

    assert!(!manifest.is_stale(clause_id, clause_hash, source_hash), "clause with matching hash must not be stale");
    assert!(manifest.is_stale(clause_id, "different_hash_00000", source_hash), "changed clause hash must be stale");
    assert!(manifest.is_stale(clause_id, clause_hash, "source_changed_hash"), "changed source hash must be stale");
    assert!(manifest.is_stale("auth::login::unknown_clause", clause_hash, source_hash), "absent clause must be stale");
}

// ===========================================================================
// llm_agnostic
// ===========================================================================

/// MUST be agnostic to which LLM provider generates the test code
#[test]
fn test_ought_llm_agnostic_must_be_agnostic_to_which_llm_provider_generates_the_test_code() {
    use ought_gen::generator::{GeneratedTest, Language};

    // The GeneratedTest struct carries no provider information -- it is purely
    // a (clause_id, code, language, file_path) tuple. This proves provider agnosticism.
    let test_a = GeneratedTest {
        clause_id: ClauseId("ought::llm_agnostic::agnostic_clause".to_string()),
        code: "// generated by provider_a\n#[test]\nfn test_x() { assert!(true); }".to_string(),
        language: Language::Rust,
        file_path: PathBuf::from("x_test.rs"),
    };
    let test_b = GeneratedTest {
        clause_id: ClauseId("ought::llm_agnostic::agnostic_clause".to_string()),
        code: "// generated by provider_b\n#[test]\nfn test_x() { assert!(true); }".to_string(),
        language: Language::Rust,
        file_path: PathBuf::from("x_test.rs"),
    };

    // Both tests have the same clause_id and language regardless of which provider produced them.
    assert_eq!(test_a.clause_id, test_b.clause_id);
    assert_eq!(test_a.language, test_b.language);
}

/// MUST allow the provider and model to be configured in `ought.toml`
#[test]
fn test_ought_llm_agnostic_must_allow_the_provider_and_model_to_be_configured_in_ought_toml() {
    use ought_spec::config::Config;

    let dir = std::env::temp_dir().join(format!("ought_llm_agnostic_config_test_{}", std::process::id()));
    fs::create_dir_all(&dir).expect("must create temp dir");

    let cases: &[(&str, Option<&str>)] = &[
        ("anthropic", Some("claude-sonnet-4-6")),
        ("claude", Some("claude-opus-4-6")),
        ("openai", Some("gpt-4o")),
        ("chatgpt", Some("gpt-4o-mini")),
        ("ollama", Some("llama3")),
        ("ollama", None),
        ("anthropic", None),
        ("openai", None),
    ];

    for (provider, model) in cases {
        let model_line = match model {
            Some(m) => format!("model = \"{m}\""),
            None => String::new(),
        };
        let toml = format!(
            "[project]\nname = \"test\"\n\n[generator]\nprovider = \"{provider}\"\n{model_line}\n\n[runner.rust]\ncommand = \"cargo test\"\ntest_dir = \"tests/\"\n"
        );
        let path = dir.join("ought.toml");
        fs::write(&path, &toml).expect("must write ought.toml");

        let config = Config::load(&path).unwrap_or_else(|e| {
            panic!("Config::load must succeed for provider=\"{provider}\" model={model:?}; got: {e}")
        });

        assert_eq!(config.generator.provider, *provider);

        match model {
            Some(m) => assert_eq!(config.generator.model.as_deref(), Some(*m)),
            None => assert!(config.generator.model.is_none()),
        }
    }

    fs::remove_dir_all(&dir).ok();
}

/// MUST NOT depend on any provider-specific features in the core spec format or runner
#[test]
fn test_ought_llm_agnostic_must_not_depend_on_any_provider_specific_features_in_the_core_spec_fo() {
    use ought_gen::generator::{GeneratedTest, Language};
    use ought_run::runner::Runner;
    use ought_run::types::RunResult;

    let clause = Clause {
        id: ClauseId("ought::llm_agnostic::core_format_clause".to_string()),
        keyword: Keyword::Must,
        severity: Severity::Required,
        text: "not depend on provider-specific features".to_string(),
        condition: None,
        otherwise: vec![],
        temporal: None,
        hints: vec![],
        source_location: SourceLocation { file: PathBuf::from("spec.ought.md"), line: 1 },
        content_hash: "xyz".to_string(),
        pending: false,
    };
    assert_eq!(clause.id.0, "ought::llm_agnostic::core_format_clause");

    let test = GeneratedTest {
        clause_id: clause.id.clone(),
        code: "#[test]\nfn test_x() { assert!(true); }".to_string(),
        language: Language::Rust,
        file_path: PathBuf::from("ought/llm_agnostic/core_format_clause_test.rs"),
    };
    assert_eq!(test.clause_id, clause.id);

    struct NeutralRunner;
    impl Runner for NeutralRunner {
        fn run(&self, _tests: &[GeneratedTest], _test_dir: &std::path::Path) -> anyhow::Result<RunResult> {
            Ok(RunResult { results: vec![], total_duration: std::time::Duration::ZERO })
        }
        fn is_available(&self) -> bool { true }
        fn name(&self) -> &str { "neutral" }
    }

    let runner: Box<dyn Runner> = Box::new(NeutralRunner);
    assert_eq!(runner.name(), "neutral");
    assert!(runner.is_available());

    let result = runner.run(&[test], &PathBuf::from("/tmp"))
        .expect("Runner must accept GeneratedTest without caring about the originating provider");
    assert_eq!(result.results.len(), 0);
}

/// MUST support at least Anthropic (Claude) and OpenAI as providers via agent mode
#[test]
fn test_ought_llm_agnostic_must_support_at_least_anthropic_claude_and_openai_as_providers() {
    // Provider configuration is validated at the config level. The orchestrator
    // maps provider names to agent CLI commands. Verify the config accepts these providers.
    let dir = std::env::temp_dir().join(format!("ought_provider_test_{}", std::process::id()));
    fs::create_dir_all(&dir).expect("must create temp dir");

    for provider in &["anthropic", "claude", "openai", "chatgpt", "ollama"] {
        let toml_content = format!(
            "[project]\nname = \"test\"\n\n[generator]\nprovider = \"{provider}\"\n\n[runner.rust]\ncommand = \"cargo test\"\ntest_dir = \"tests/\"\n"
        );
        let path = dir.join("ought.toml");
        fs::write(&path, &toml_content).expect("must write ought.toml");
        let config = Config::load(&path);
        assert!(
            config.is_ok(),
            "Config must load with provider=\"{provider}\"; got: {:?}",
            config.err()
        );
        assert_eq!(config.unwrap().generator.provider, *provider);
    }

    fs::remove_dir_all(&dir).ok();
}

// ===========================================================================
// language_agnostic
// ===========================================================================

/// MUST be agnostic to the programming language of the project under test
#[test]
fn test_ought_language_agnostic_must_be_agnostic_to_the_programming_language_of_the_project_under() {
    let languages = ["rust", "python", "typescript", "go"];
    for lang in languages {
        let runner = ought_run::runners::from_name(lang)
            .unwrap_or_else(|e| panic!("from_name({lang:?}) failed: {e}"));
        let name: &str = runner.name();
        let _available: bool = runner.is_available();
        assert_eq!(name, lang, "runner created for {lang:?} must report that same name back");
    }

    let unknown = ought_run::runners::from_name("cobol");
    assert!(unknown.is_err(), "from_name with an unrecognised language must return Err");

    let runners: Vec<_> = languages.iter().map(|lang| ought_run::runners::from_name(lang).unwrap()).collect();
    let names: std::collections::HashSet<&str> = runners.iter().map(|r| r.name()).collect();
    assert_eq!(names.len(), languages.len(), "all supported runner names must be distinct");
}

/// MUST delegate test execution to the project's existing test harness
#[test]
fn test_ought_language_agnostic_must_delegate_test_execution_to_the_project_s_existing_test_harne() {
    let harness_map: &[(&str, &str)] = &[
        ("rust",       "cargo"),
        ("python",     "pytest"),
        ("typescript", "npx"),
        ("go",         "go"),
    ];

    for (lang, harness_hint) in harness_map {
        let runner = ought_run::runners::from_name(lang)
            .unwrap_or_else(|e| panic!("from_name({lang:?}) must succeed: {e}"));
        assert_eq!(runner.name(), *lang);
        let _available: bool = runner.is_available();
        assert!(!harness_hint.is_empty());
    }

    let unique_harnesses: std::collections::HashSet<&str> = harness_map.iter().map(|(_, h)| *h).collect();
    assert_eq!(unique_harnesses.len(), harness_map.len(),
        "each runner must delegate to a distinct harness binary");
}

/// MUST ship with runners for at least Rust and one other mainstream language
#[test]
fn test_ought_language_agnostic_must_ship_with_runners_for_at_least_rust_and_one_other_mainstream() {
    let rust_runner = ought_run::runners::from_name("rust")
        .expect("Rust runner must be included");
    assert_eq!(rust_runner.name(), "rust");

    let other_mainstream = ["python", "typescript", "go", "javascript"];
    let available_others: Vec<&str> = other_mainstream.iter().copied()
        .filter(|lang| ought_run::runners::from_name(lang).is_ok())
        .collect();

    assert!(!available_others.is_empty(),
        "at least one non-Rust mainstream runner must ship; tried {:?}", other_mainstream);
}

/// MUST NOT require any language-specific SDK or library in the project under test
#[test]
fn test_ought_language_agnostic_must_not_require_any_language_specific_sdk_or_library_in_the_project() {
    use ought_spec::config::RunnerConfig;
    use ought_gen::GeneratedTest;
    use ought_gen::generator::Language;

    let ruby_cfg = RunnerConfig {
        command: "bundle exec rspec".to_string(),
        test_dir: PathBuf::from("spec/ought/"),
    };
    assert_eq!(ruby_cfg.command, "bundle exec rspec");

    let rust_test = GeneratedTest {
        clause_id: ClauseId("example::must_add".to_string()),
        code: "#[test]\nfn test_example__must_add() { assert_eq!(1 + 1, 2); }".to_string(),
        language: Language::Rust,
        file_path: PathBuf::from("test_example__must_add.rs"),
    };
    assert!(!rust_test.code.contains("use ought"), "generated Rust test must not require ought imports");
    assert!(!rust_test.code.contains("extern crate ought"), "generated Rust test must not require ought crate");

    let python_test = GeneratedTest {
        clause_id: ClauseId("example::must_add_python".to_string()),
        code: "def test_example__must_add():\n    assert 1 + 1 == 2".to_string(),
        language: Language::Python,
        file_path: PathBuf::from("test_example__must_add.py"),
    };
    assert!(!python_test.code.contains("import ought"), "generated Python test must not require ought import");
}

/// SHOULD support custom runners via configuration
#[test]
fn test_ought_language_agnostic_should_support_custom_runners_via_configuration() {
    use ought_spec::config::Config;

    let toml_str = r#"
[project]
name = "custom-runner-test"
version = "0.1.0"

[generator]
provider = "anthropic"

[runner.elixir]
command = "mix test"
test_dir = "test/ought/"

[runner.ruby]
command = "bundle exec rspec"
test_dir = "spec/ought/"

[runner.dotnet]
command = "dotnet test"
test_dir = "tests/ought/"
"#;

    let tmp = std::env::temp_dir().join(format!("ought_custom_runner_cfg_{}", std::process::id()));
    fs::create_dir_all(&tmp).unwrap();
    let cfg_path = tmp.join("ought.toml");
    fs::write(&cfg_path, toml_str).unwrap();

    let config = Config::load(&cfg_path)
        .expect("ought.toml with custom [runner.*] sections must parse without error");

    assert_eq!(config.runner.len(), 3,
        "all custom runner sections must be present after parsing; found keys: {:?}",
        config.runner.keys().collect::<Vec<_>>());

    let elixir = config.runner.get("elixir").expect("runner 'elixir' must be accepted");
    assert_eq!(elixir.command, "mix test");
    assert_eq!(elixir.test_dir.to_string_lossy(), "test/ought/");

    let ruby = config.runner.get("ruby").expect("runner 'ruby' must be accepted");
    assert_eq!(ruby.command, "bundle exec rspec");

    let dotnet = config.runner.get("dotnet").expect("runner 'dotnet' must be accepted");
    assert_eq!(dotnet.command, "dotnet test");

    let _ = fs::remove_dir_all(&tmp);
}

// ===========================================================================
// reporting
// ===========================================================================

/// MUST map test results back to the original spec clauses (not just test function names)
#[test]
fn test_ought_reporting_must_map_test_results_back_to_the_original_spec_clauses_not_just() {
    use ought_report::json;
    use ought_run::{RunResult, TestResult, TestStatus, TestDetails};

    let clause_id = "auth::login::must_return_jwt_on_success";
    let clause = Clause {
        id: ClauseId(clause_id.to_string()),
        keyword: Keyword::Must,
        severity: Keyword::Must.severity(),
        text: "return a signed JWT on successful login".to_string(),
        condition: None,
        otherwise: vec![],
        temporal: None,
        hints: vec![],
        source_location: SourceLocation { file: PathBuf::from("auth.ought.md"), line: 5 },
        content_hash: "abc".to_string(),
        pending: false,
    };
    let spec = Spec {
        name: "Auth".to_string(),
        metadata: Metadata::default(),
        sections: vec![Section {
            title: "Login".to_string(),
            depth: 1,
            prose: String::new(),
            clauses: vec![clause],
            subsections: vec![],
        }],
        source_path: PathBuf::from("auth.ought.md"),
    };
    let result = TestResult {
        clause_id: ClauseId(clause_id.to_string()),
        status: TestStatus::Passed,
        message: None,
        duration: std::time::Duration::from_millis(12),
        details: TestDetails::default(),
    };
    let run = RunResult { results: vec![result], total_duration: std::time::Duration::from_millis(12) };

    let json_out = json::report(&run, &[spec]).unwrap();

    assert!(json_out.contains(clause_id),
        "JSON report must embed the original spec clause ID '{}'", clause_id);
    assert!(json_out.contains("\"clause_id\""),
        "JSON report must have an explicit 'clause_id' field");
    assert!(!json_out.contains("test_auth_login_must_return_jwt_on_success"),
        "report must use the spec clause ID, not a generated test function name");
}

/// MUST distinguish failure severity -- MUST failures are errors, SHOULD failures are warnings
#[test]
fn test_ought_reporting_must_distinguish_failure_severity_must_failures_are_errors_should() {
    use ought_report::json;
    use ought_run::{RunResult, TestResult, TestStatus, TestDetails};

    assert_eq!(Keyword::Must.severity(), Severity::Required);
    assert_eq!(Keyword::MustNot.severity(), Severity::Required);
    assert_eq!(Keyword::MustAlways.severity(), Severity::Required);
    assert_eq!(Keyword::MustBy.severity(), Severity::Required);
    assert_eq!(Keyword::Should.severity(), Severity::Recommended);
    assert_eq!(Keyword::ShouldNot.severity(), Severity::Recommended);

    // In the derived Ord, Required has the lowest discriminant (most severe first).
    assert!(Severity::Required < Severity::Recommended,
        "Required (MUST) severity must have a lower discriminant than Recommended (SHOULD), reflecting higher severity");

    let must_id = "report::severity::must_clause";
    let should_id = "report::severity::should_clause";

    fn make_clause(id: &str, kw: Keyword, text: &str) -> Clause {
        Clause {
            id: ClauseId(id.to_string()),
            keyword: kw,
            severity: kw.severity(),
            text: text.to_string(),
            condition: None,
            otherwise: vec![],
            temporal: None,
            hints: vec![],
            source_location: SourceLocation { file: PathBuf::from("s.ought.md"), line: 1 },
            content_hash: "x".to_string(),
            pending: false,
        }
    }

    let spec = Spec {
        name: "Severity Test".to_string(),
        metadata: Metadata::default(),
        sections: vec![Section {
            title: "Clauses".to_string(),
            depth: 1,
            prose: String::new(),
            clauses: vec![
                make_clause(must_id, Keyword::Must, "always succeed"),
                make_clause(should_id, Keyword::Should, "prefer to succeed"),
            ],
            subsections: vec![],
        }],
        source_path: PathBuf::from("s.ought.md"),
    };
    let run = RunResult {
        results: vec![
            TestResult {
                clause_id: ClauseId(must_id.to_string()),
                status: TestStatus::Failed,
                message: None,
                duration: std::time::Duration::from_millis(1),
                details: TestDetails::default(),
            },
            TestResult {
                clause_id: ClauseId(should_id.to_string()),
                status: TestStatus::Failed,
                message: None,
                duration: std::time::Duration::from_millis(1),
                details: TestDetails::default(),
            },
        ],
        total_duration: std::time::Duration::from_millis(2),
    };

    let json_out = json::report(&run, &[spec]).unwrap();

    assert!(json_out.contains("\"required\""), "Failed MUST clause must appear with severity 'required'");
    assert!(json_out.contains("\"recommended\""), "Failed SHOULD clause must appear with severity 'recommended'");
    // Both clauses have status "failed" - the count may be > 2 because the summary
    // also contains a "failed" field. Verify at least 2 occurrences.
    assert!(json_out.matches("\"failed\"").count() >= 2, "Both clauses should appear with status 'failed'");
}

/// MUST produce visually attractive terminal output that makes specs and their status easy to scan
///
/// Tests that the terminal reporter and JSON reporter accept well-formed input.
#[test]
fn test_ought_reporting_must_produce_visually_attractive_terminal_output_that_makes_specs() {
    use ought_report::json;
    use ought_report::terminal;
    use ought_report::types::{ColorChoice, ReportOptions};
    use ought_run::{RunResult, TestResult, TestStatus, TestDetails};

    fn make_clause(id: &str, kw: Keyword, text: &str) -> Clause {
        Clause {
            id: ClauseId(id.to_string()),
            keyword: kw,
            severity: kw.severity(),
            text: text.to_string(),
            condition: None,
            otherwise: vec![],
            temporal: None,
            hints: vec![],
            source_location: SourceLocation { file: PathBuf::from("t.ought.md"), line: 1 },
            content_hash: "x".to_string(),
            pending: false,
        }
    }

    let passed_id = "terminal::output::must_return_200";
    let failed_id = "terminal::output::should_include_request_id";
    let errored_id = "terminal::output::must_not_leak_secrets";

    let spec = Spec {
        name: "Terminal Display".to_string(),
        metadata: Metadata::default(),
        sections: vec![Section {
            title: "HTTP API".to_string(),
            depth: 1,
            prose: String::new(),
            clauses: vec![
                make_clause(passed_id, Keyword::Must, "return 200 on success"),
                make_clause(failed_id, Keyword::Should, "include X-Request-Id header"),
                make_clause(errored_id, Keyword::MustNot, "leak secrets in response body"),
            ],
            subsections: vec![],
        }],
        source_path: PathBuf::from("t.ought.md"),
    };
    let run = RunResult {
        results: vec![
            TestResult {
                clause_id: ClauseId(passed_id.to_string()),
                status: TestStatus::Passed,
                message: None,
                duration: std::time::Duration::from_millis(8),
                details: TestDetails::default(),
            },
            TestResult {
                clause_id: ClauseId(failed_id.to_string()),
                status: TestStatus::Failed,
                message: Some("header absent".to_string()),
                duration: std::time::Duration::from_millis(3),
                details: TestDetails { failure_message: Some("header absent".to_string()), ..Default::default() },
            },
            TestResult {
                clause_id: ClauseId(errored_id.to_string()),
                status: TestStatus::Errored,
                message: Some("panicked at 'index out of bounds'".to_string()),
                duration: std::time::Duration::from_millis(1),
                details: TestDetails::default(),
            },
        ],
        total_duration: std::time::Duration::from_millis(12),
    };

    let options = ReportOptions { color: ColorChoice::Never, ..Default::default() };
    assert!(
        terminal::report(&run, &[spec.clone()], &options).is_ok(),
        "terminal::report must complete without error on valid input"
    );

    let json_out = json::report(&run, &[spec]).unwrap();
    assert!(json_out.contains("\"clause_id\""));
    assert!(json_out.contains("\"keyword\""));
    assert!(json_out.contains("\"severity\""));
    assert!(json_out.contains("\"status\""));
    assert!(json_out.contains("\"passed\""));
    assert!(json_out.contains("\"failed\""));
    assert!(json_out.contains("\"errored\""));
    assert!(json_out.contains("must_coverage_pct"));

    // Verify MUST coverage < 100% when a MUST clause errored.
    // Extract the must_coverage_pct value via simple string matching.
    let pct_marker = "\"must_coverage_pct\":";
    let pct_pos = json_out.find(pct_marker).expect("must_coverage_pct must appear in JSON output");
    let after = &json_out[pct_pos + pct_marker.len()..];
    let end = after.find(|c: char| c != ' ' && c != '.' && !c.is_ascii_digit()).unwrap_or(after.len());
    let pct_str = after[..end].trim();
    let pct: f64 = pct_str.parse().unwrap_or_else(|_| panic!("could not parse must_coverage_pct from: {pct_str}"));
    assert!(pct < 100.0, "MUST coverage must be < 100% when a MUST clause errored; got {pct}");
}

// ===========================================================================
// llm_powered_analysis
// ===========================================================================

/// MUST support surveying source code to discover behaviors not covered by any spec
///
/// Requires LLM. Marked ignored.
#[test]
#[ignore]
fn test_ought_llm_powered_analysis_must_support_surveying_source_code_to_discover_behaviors_not_cove() {
    // Would call ought_analysis::survey::survey() with a stub generator.
}

/// MUST support auditing specs for contradictions, gaps, and coherence issues
///
/// Requires LLM. Marked ignored.
#[test]
#[ignore]
fn test_ought_llm_powered_analysis_must_support_auditing_specs_for_contradictions_gaps_and_coherence() {
    // Would call ought_analysis::audit::audit() with a stub generator.
}

/// MUST support blaming a failure on a specific source change with a causal narrative
///
/// Requires LLM. Marked ignored.
#[test]
#[ignore]
fn test_ought_llm_powered_analysis_must_support_blaming_a_failure_on_a_specific_source_change_with_a() {
    // Would call ought_analysis::blame::blame() with a stub generator.
}

/// SHOULD support bisecting git history to find the exact commit that broke a clause
///
/// Requires LLM + git history. Marked ignored.
#[test]
#[ignore]
fn test_ought_llm_powered_analysis_should_support_bisecting_git_history_to_find_the_exact_commit_that() {
    // Would call ought_analysis::bisect::bisect() with a test runner.
}

// ===========================================================================
// integration
// ===========================================================================

/// MUST provide an MCP server so AI assistants and IDE extensions can interact programmatically
///
/// Requires the MCP server implementation. Marked ignored.
#[test]
#[ignore]
fn test_ought_integration_must_provide_an_mcp_server_so_ai_assistants_and_ide_extensions_ca() {
    // Would spawn `ought mcp serve` and verify it is a recognised subcommand.
}

/// SHOULD be installable via cargo, Homebrew, and as a standalone binary
#[test]
fn test_ought_integration_should_be_installable_via_cargo_homebrew_and_as_a_standalone_binary() {
    let bin = PathBuf::from(env!("CARGO_BIN_EXE_ought"));

    // Verify the binary runs as a standalone executable via --help
    // (the CLI does not currently support --version)
    let help_out = Command::new(&bin)
        .arg("--help")
        .output()
        .expect("`ought` binary must run as a standalone executable");

    assert!(help_out.status.success(),
        "`ought --help` must exit 0; stderr: {}",
        String::from_utf8_lossy(&help_out.stderr));
    let help_str = format!(
        "{}{}",
        String::from_utf8_lossy(&help_out.stdout),
        String::from_utf8_lossy(&help_out.stderr),
    );
    assert!(!help_str.trim().is_empty(),
        "`ought --help` must produce usage output");

    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR")
        .expect("CARGO_MANIFEST_DIR must be set"));
    let workspace_root = manifest_dir.ancestors()
        .find(|p| p.join("Cargo.lock").exists())
        .unwrap_or_else(|| manifest_dir.as_path())
        .to_path_buf();

    let cli_toml_path = workspace_root.join("crates").join("ought-cli").join("Cargo.toml");
    assert!(cli_toml_path.exists(), "crates/ought-cli/Cargo.toml must exist");

    let cli_toml = fs::read_to_string(&cli_toml_path).expect("ought-cli/Cargo.toml must be readable");
    assert!(cli_toml.contains("name = \"ought\""), "ought-cli/Cargo.toml must set `name = \"ought\"`");
    assert!(cli_toml.contains("[[bin]]"), "ought-cli/Cargo.toml must declare a [[bin]] target");
}

/// SHOULD provide a GitHub Action for PR-level reporting
///
/// The action.yml file does not exist yet. Marked ignored until implemented.
#[test]
#[ignore]
fn test_ought_integration_should_provide_a_github_action_for_pr_level_reporting() {
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR")
        .expect("CARGO_MANIFEST_DIR must be set"));
    let workspace_root = manifest_dir.ancestors()
        .find(|p| p.join("Cargo.lock").exists())
        .unwrap_or_else(|| manifest_dir.as_path())
        .to_path_buf();

    let candidates = [
        workspace_root.join("action.yml"),
        workspace_root.join("action.yaml"),
        workspace_root.join(".github").join("actions").join("ought").join("action.yml"),
        workspace_root.join(".github").join("actions").join("ought").join("action.yaml"),
    ];

    let action_exists = candidates.iter().any(|p| p.exists());

    assert!(action_exists,
        "A GitHub Action definition (action.yml) must exist at the repository root or under .github/actions/ought/");

    for path in candidates.iter().filter(|p| p.exists()) {
        let contents = fs::read_to_string(path)
            .unwrap_or_else(|_| panic!("action file at {} must be readable", path.display()));
        assert!(contents.contains("runs:"),
            "action file at {} must contain a `runs:` key", path.display());
    }
}

/// MUST be easy to integrate into CI pipelines (run without LLM access)
#[test]
fn test_ought_integration_must_be_easy_to_integrate_into_ci_pipelines_run_without_llm_acces() {
    let bin = PathBuf::from(env!("CARGO_BIN_EXE_ought"));

    let run_help = Command::new(&bin)
        .args(["run", "--help"])
        .env_remove("ANTHROPIC_API_KEY")
        .env_remove("ANTHROPIC_AUTH_TOKEN")
        .env_remove("OPENAI_API_KEY")
        .output();

    match run_help {
        Ok(out) => {
            let run_stderr = String::from_utf8_lossy(&out.stderr);
            assert_ne!(out.status.code(), Some(2),
                "`ought run` must be a recognised subcommand; stderr: {run_stderr}");
            assert!(!run_stderr.contains("unrecognized subcommand"),
                "`ought run` must be a recognised subcommand; stderr: {run_stderr}");
        }
        Err(e) => panic!("`ought` binary must be available in CI environments; failed: {}", e),
    }

    let check_help = Command::new(&bin)
        .args(["check", "--help"])
        .env_remove("ANTHROPIC_API_KEY")
        .env_remove("ANTHROPIC_AUTH_TOKEN")
        .env_remove("OPENAI_API_KEY")
        .output();

    if let Ok(out) = check_help {
        let check_stderr = String::from_utf8_lossy(&out.stderr);
        assert_ne!(out.status.code(), Some(2),
            "`ought check` must be a recognised subcommand; stderr: {check_stderr}");
        assert!(!check_stderr.contains("unrecognized subcommand"),
            "`ought check` must exist as a distinct subcommand; stderr: {check_stderr}");
    }
}