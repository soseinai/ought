/// SHOULD use the clause text to determine which kind of WONT test to generate
#[test]
fn test_generator__wont_clause_handling__should_use_the_clause_text_to_determine_which_kind_of_wont_test_to() {
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

    // Absence-appropriate: the feature simply must not exist at all.
    let absence_text = "expose a GraphQL introspection endpoint in production";
    let absence_clause = wont_clause(
        "generator::wont_clause_handling::wont_expose_graphql_introspection",
        absence_text,
    );

    // Prevention-appropriate: attempting the operation must fail gracefully.
    let prevention_text = "allow concurrent writes without acquiring a lock — callers must receive a conflict error";
    let prevention_clause = wont_clause(
        "generator::wont_clause_handling::wont_allow_concurrent_writes_without_lock",
        prevention_text,
    );

    // Each clause text must appear verbatim in its single-clause prompt so
    // the LLM can read it and pick the right test kind.
    let absence_prompt = build_prompt(&absence_clause, &ctx);
    assert!(
        absence_prompt.contains(absence_text),
        "build_prompt must embed the WONT clause text verbatim; expected '{}' \
         in prompt:\n{absence_prompt}",
        absence_text
    );

    let prevention_prompt = build_prompt(&prevention_clause, &ctx);
    assert!(
        prevention_prompt.contains(prevention_text),
        "build_prompt must embed the WONT clause text verbatim; expected '{}' \
         in prompt:\n{prevention_prompt}",
        prevention_text
    );

    // Distinct clause texts must produce distinct prompts so the LLM
    // receives different signals and can make different per-clause decisions.
    assert_ne!(
        absence_prompt, prevention_prompt,
        "prompts for WONT clauses with different texts must differ so the LLM \
         can determine the appropriate test kind from the clause text"
    );

    // Both texts must appear in the batch prompt so the LLM can make
    // per-clause decisions within a single generation call.
    let group = ClauseGroup {
        section_path: "Generator > WONT Clause Handling".to_string(),
        clauses: vec![&absence_clause, &prevention_clause],
        conditions: vec![],
    };
    let batch_prompt = build_batch_prompt(&group, &ctx);
    assert!(
        batch_prompt.contains(absence_text),
        "build_batch_prompt must include the absence-style clause text so the LLM \
         can determine it should emit an absence test; expected '{}' in:\n{batch_prompt}",
        absence_text
    );
    assert!(
        batch_prompt.contains(prevention_text),
        "build_batch_prompt must include the prevention-style clause text so the LLM \
         can determine it should emit a prevention test; expected '{}' in:\n{batch_prompt}",
        prevention_text
    );
}