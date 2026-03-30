/// MUST use the GIVEN condition text to generate test setup/precondition code
#[test]
fn test_generator__given_block_generation__must_use_the_given_condition_text_to_generate_test_setup_precondi() {
    use ought_gen::context::GenerationContext;
    use ought_gen::generator::Language;
    use ought_gen::providers::build_prompt;
    use ought_spec::{Clause, ClauseId, Keyword, Severity, SourceLocation};
    use std::path::PathBuf;

    let clause = Clause {
        id: ClauseId("gen::given::must_validate_token".to_string()),
        keyword: Keyword::Must,
        severity: Severity::Required,
        text: "reject requests with expired tokens".to_string(),
        condition: Some("the user presents an expired token".to_string()),
        otherwise: vec![],
        temporal: None,
        hints: vec![],
        source_location: SourceLocation { file: PathBuf::from("spec.ought.md"), line: 5 },
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

    assert!(
        prompt.contains("the user presents an expired token"),
        "must_use_the_given_condition_text: prompt must contain the GIVEN condition text to inform test setup; prompt was:\n{}",
        prompt
    );
}