/// MUST include the GIVEN condition in the LLM prompt so it understands the precondition context
#[test]
fn test_generator__given_block_generation__must_include_the_given_condition_in_the_llm_prompt_so_it_understa() {
    use ought_gen::context::GenerationContext;
    use ought_gen::generator::{ClauseGroup, Language};
    use ought_gen::providers::{build_batch_prompt, build_prompt};
    use ought_spec::{Clause, ClauseId, Keyword, Severity, SourceLocation};
    use std::path::PathBuf;

    let context = GenerationContext {
        spec_context: None,
        source_files: vec![],
        schema_files: vec![],
        target_language: Language::Rust,
        verbose: false,
    };

    // Single-clause path: condition on clause itself
    let clause = Clause {
        id: ClauseId("gen::given::must_return_profile".to_string()),
        keyword: Keyword::Must,
        severity: Severity::Required,
        text: "return the user profile".to_string(),
        condition: Some("a valid session token is provided".to_string()),
        otherwise: vec![],
        temporal: None,
        hints: vec![],
        source_location: SourceLocation { file: PathBuf::from("spec.ought.md"), line: 10 },
        content_hash: "h2".to_string(),
    };

    let single_prompt = build_prompt(&clause, &context);
    assert!(
        single_prompt.contains("a valid session token is provided"),
        "build_prompt must include the GIVEN condition so the LLM understands the precondition; \
         condition text not found in prompt"
    );

    // Batch path: conditions on the group
    let group = ClauseGroup {
        section_path: "Gen > Given".to_string(),
        clauses: vec![&clause],
        conditions: vec!["a valid session token is provided".to_string()],
    };
    let batch_prompt = build_batch_prompt(&group, &context);
    assert!(
        batch_prompt.contains("a valid session token is provided"),
        "build_batch_prompt must include group GIVEN conditions so the LLM understands the precondition context; \
         condition text not found in batch prompt"
    );
}