/// SHOULD generate descriptive assertion messages that reference the clause
#[test]
fn test_generator__test_generation__should_generate_descriptive_assertion_messages_that_reference_the_c() {
    use ought_gen::context::GenerationContext;
    use ought_gen::generator::Language;
    use ought_gen::providers::build_prompt;
    use ought_spec::{Clause, ClauseId, Keyword, Severity, SourceLocation};
    use std::path::PathBuf;

    let clause = Clause {
        id: ClauseId("payments::checkout::must_reject_expired_cards".to_string()),
        keyword: Keyword::Must,
        severity: Severity::Required,
        text: "reject expired cards at checkout".to_string(),
        condition: None,
        otherwise: vec![],
        temporal: None,
        hints: vec![],
        source_location: SourceLocation { file: PathBuf::from("payments.ought.md"), line: 10 },
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

    // The clause ID must appear in the prompt so the LLM can embed it in assertion messages
    assert!(
        prompt.contains("payments::checkout::must_reject_expired_cards"),
        "should_generate_descriptive_assertion_messages_that_reference_the_c: \
         build_prompt must include the full clause ID so the LLM can reference it in assertions"
    );
    // The clause text must appear so the LLM can quote it in descriptive failure messages
    assert!(
        prompt.contains("reject expired cards at checkout"),
        "should_generate_descriptive_assertion_messages_that_reference_the_c: \
         build_prompt must include the original clause text so the LLM can produce \
         descriptive, clause-referencing assertion messages"
    );
}