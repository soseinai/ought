/// MUST compose the conditions — the inner test setup includes both outer and inner preconditions
/// GIVEN: a clause has nested GIVEN blocks
#[test]
fn test_generator__given_block_generation__must_compose_the_conditions_the_inner_test_setup_includes_both_ou() {
    use ought_gen::context::GenerationContext;
    use ought_gen::generator::{ClauseGroup, Language};
    use ought_gen::providers::build_batch_prompt;
    use ought_spec::{Clause, ClauseId, Keyword, Severity, SourceLocation};
    use std::path::PathBuf;

    // A clause with its own inner GIVEN condition (nested GIVEN block)
    let inner_clause = Clause {
        id: ClauseId("gen::given::nested::must_forbid_delete".to_string()),
        keyword: Keyword::Must,
        severity: Severity::Required,
        text: "forbid deletion".to_string(),
        condition: Some("the item is marked as locked".to_string()),
        otherwise: vec![],
        temporal: None,
        hints: vec![],
        source_location: SourceLocation { file: PathBuf::from("spec.ought.md"), line: 20 },
        content_hash: "h3".to_string(),
    };

    // The outer GIVEN condition is carried by the ClauseGroup
    let outer_condition = "the user is an admin";
    let group = ClauseGroup {
        section_path: "Gen > Given > Nested".to_string(),
        clauses: vec![&inner_clause],
        conditions: vec![outer_condition.to_string()],
    };

    let context = GenerationContext {
        spec_context: None,
        source_files: vec![],
        schema_files: vec![],
        target_language: Language::Rust,
        verbose: false,
    };

    let prompt = build_batch_prompt(&group, &context);

    // Both the outer condition (from the group) and the inner condition
    // (from the clause itself) must appear in the prompt so the LLM
    // generates a test that composes both preconditions.
    assert!(
        prompt.contains(outer_condition),
        "must_compose_the_conditions: outer GIVEN condition '{}' must appear in the batch prompt \
         so tests include the outer precondition",
        outer_condition
    );
    assert!(
        prompt.contains("the item is marked as locked"),
        "must_compose_the_conditions: inner GIVEN condition 'the item is marked as locked' must \
         appear in the batch prompt so tests include the inner precondition"
    );

    // Both must be present together — not one or the other
    let outer_pos = prompt.find(outer_condition).expect("outer condition must be present");
    let inner_pos = prompt.find("the item is marked as locked").expect("inner condition must be present");
    assert_ne!(
        outer_pos, inner_pos,
        "outer and inner conditions must appear as distinct entries in the prompt"
    );
}