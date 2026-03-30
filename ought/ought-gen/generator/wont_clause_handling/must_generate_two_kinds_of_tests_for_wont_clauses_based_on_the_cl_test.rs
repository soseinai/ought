/// MUST generate two kinds of tests for WONT clauses based on the clause text:
///   Absence tests: verify the capability does not exist (e.g. no endpoint, no method, no feature flag)
///   Prevention tests: verify that attempting the behavior fails gracefully
#[test]
fn test_generator__wont_clause_handling__must_generate_two_kinds_of_tests_for_wont_clauses_based_on_the_cl() {
    use ought_gen::context::GenerationContext;
    use ought_gen::generator::{ClauseGroup, Language};
    use ought_gen::providers::{build_batch_prompt, build_prompt};
    use ought_spec::{Clause, ClauseId, Keyword, Severity, SourceLocation};
    use std::path::PathBuf;

    fn wont_clause(id: &str, text: &str) -> Clause {
        Clause {
            id: ClauseId(id.to_string()),
            keyword: Keyword::Wont,
            severity: Severity::NegativeConfirmation,
            text: text.to_string(),
            condition: None,
            otherwise: vec![],
            temporal: None,
            hints: vec![],
            source_location: SourceLocation {
                file: PathBuf::from("spec.ought.md"),
                line: 1,
            },
            content_hash: "h".to_string(),
        }
    }

    let ctx = GenerationContext {
        spec_context: None,
        source_files: vec![],
        schema_files: vec![],
        target_language: Language::Rust,
        verbose: false,
    };

    // --- Single-clause prompt must mention both kinds ---
    let absence_clause = wont_clause(
        "generator::wont_clause_handling::wont_expose_admin_api",
        "expose an /admin/reset endpoint",
    );
    let single_prompt = build_prompt(&absence_clause, &ctx);
    assert!(
        single_prompt.contains("absence"),
        "build_prompt for a WONT clause must mention absence tests so the LLM can \
         generate an absence-style test; got prompt:\n{single_prompt}"
    );
    assert!(
        single_prompt.contains("prevention"),
        "build_prompt for a WONT clause must mention prevention tests so the LLM can \
         generate a prevention-style test; got prompt:\n{single_prompt}"
    );

    // --- Batch prompt must also expose both options to the LLM ---
    let prevention_clause = wont_clause(
        "generator::wont_clause_handling::wont_swallow_write_errors",
        "silently swallow write errors — must surface them as Err",
    );
    let group = ClauseGroup {
        section_path: "Generator > WONT Clause Handling".to_string(),
        clauses: vec![&absence_clause, &prevention_clause],
        conditions: vec![],
    };
    let batch_prompt = build_batch_prompt(&group, &ctx);
    assert!(
        batch_prompt.contains("absence"),
        "build_batch_prompt must mention absence tests for WONT clauses; \
         got batch prompt:\n{batch_prompt}"
    );
    assert!(
        batch_prompt.contains("prevention"),
        "build_batch_prompt must mention prevention tests for WONT clauses; \
         got batch prompt:\n{batch_prompt}"
    );

    // --- Non-WONT clauses must NOT receive the absence/prevention instructions ---
    let must_clause = Clause {
        id: ClauseId("generator::wont_clause_handling::must_do_something".to_string()),
        keyword: Keyword::Must,
        severity: Severity::Required,
        text: "do something".to_string(),
        condition: None,
        otherwise: vec![],
        temporal: None,
        hints: vec![],
        source_location: SourceLocation {
            file: PathBuf::from("spec.ought.md"),
            line: 1,
        },
        content_hash: "h".to_string(),
    };
    let must_prompt = build_prompt(&must_clause, &ctx);
    assert!(
        !must_prompt.contains("absence test (verify the capability does not exist)"),
        "build_prompt for a non-WONT clause must not include WONT-specific absence/prevention \
         instructions; got prompt:\n{must_prompt}"
    );
}