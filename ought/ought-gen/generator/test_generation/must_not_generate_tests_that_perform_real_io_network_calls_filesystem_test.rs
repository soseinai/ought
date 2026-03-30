/// MUST NOT generate tests that perform real IO (network calls, filesystem writes, database operations) unless the clause explicitly describes integration behavior
#[test]
fn test_generator__test_generation__must_not_generate_tests_that_perform_real_io_network_calls_filesystem() {
    use ought_gen::context::GenerationContext;
    use ought_gen::generator::Language;
    use ought_gen::providers::build_prompt;
    use ought_spec::{Clause, ClauseId, Keyword, Severity, SourceLocation};
    use std::path::PathBuf;

    // A plain logic clause — no integration context
    let clause = Clause {
        id: ClauseId("auth::hash::must_use_bcrypt".to_string()),
        keyword: Keyword::Must,
        severity: Severity::Required,
        text: "hash passwords with bcrypt before storing".to_string(),
        condition: None,
        otherwise: vec![],
        temporal: None,
        hints: vec![],
        source_location: SourceLocation { file: PathBuf::from("auth.ought.md"), line: 3 },
        content_hash: "abc".to_string(),
    };
    let context = GenerationContext {
        spec_context: None,
        source_files: vec![],
        schema_files: vec![],
        target_language: Language::Rust,
        verbose: false,
    };
    let prompt = build_prompt(&clause, &context);

    // Self-containment is the primary guard against real IO
    assert!(
        prompt.contains("self-contained"),
        "must_not_generate_tests_that_perform_real_io: build_prompt must require self-contained \
         tests — the mechanism that prevents real IO for non-integration clauses"
    );

    // The prompt must not instruct the LLM to do real IO for a pure logic clause
    let lower = prompt.to_lowercase();
    assert!(
        !lower.contains("connect to database")
            && !lower.contains("http request")
            && !lower.contains("write to disk"),
        "must_not_generate_tests_that_perform_real_io: build_prompt for a logic clause must not \
         contain IO instructions; prompt starts with: {:?}",
        &prompt[..prompt.len().min(200)]
    );
}