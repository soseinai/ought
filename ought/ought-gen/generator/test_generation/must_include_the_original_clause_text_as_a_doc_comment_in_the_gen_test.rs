/// MUST include the original clause text as a doc comment in the generated test
#[test]
fn test_generator__test_generation__must_include_the_original_clause_text_as_a_doc_comment_in_the_gen() {
    use ought_gen::context::GenerationContext;
    use ought_gen::generator::Language;
    use ought_gen::providers::build_prompt;
    use ought_spec::{Clause, ClauseId, Keyword, Severity, SourceLocation};
    use std::path::PathBuf;

    let clause = Clause {
        id: ClauseId("auth::login::must_return_jwt".to_string()),
        keyword: Keyword::Must,
        severity: Severity::Required,
        text: "return a JWT on successful authentication".to_string(),
        condition: None,
        otherwise: vec![],
        temporal: None,
        hints: vec![],
        source_location: SourceLocation { file: PathBuf::from("auth.ought.md"), line: 5 },
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
        prompt.contains("doc comment"),
        "must_include_the_original_clause_text_as_a_doc_comment_in_the_gen: \
         build_prompt must instruct the LLM to add the clause text as a doc comment"
    );
    assert!(
        prompt.contains("return a JWT on successful authentication"),
        "must_include_the_original_clause_text_as_a_doc_comment_in_the_gen: \
         build_prompt must embed the original clause text so it can be echoed verbatim"
    );
}