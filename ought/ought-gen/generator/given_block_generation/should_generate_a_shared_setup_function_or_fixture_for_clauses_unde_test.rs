/// SHOULD generate a shared setup function or fixture for clauses under the same GIVEN block
#[test]
fn test_generator__given_block_generation__should_generate_a_shared_setup_function_or_fixture_for_clauses_unde() {
    use ought_gen::context::GenerationContext;
    use ought_gen::generator::{ClauseGroup, Language};
    use ought_gen::providers::build_batch_prompt;
    use ought_spec::{Clause, ClauseId, Keyword, Severity, SourceLocation};
    use std::path::PathBuf;

    fn mk_clause(id: &str, text: &str) -> Clause {
        Clause {
            id: ClauseId(id.to_string()),
            keyword: Keyword::Must,
            severity: Severity::Required,
            text: text.to_string(),
            condition: None,
            otherwise: vec![],
            temporal: None,
            hints: vec![],
            source_location: SourceLocation { file: PathBuf::from("spec.ought.md"), line: 1 },
            content_hash: "h".to_string(),
        }
    }

    let c1 = mk_clause("gen::given::must_list_items", "list all items");
    let c2 = mk_clause("gen::given::must_delete_item", "delete an item by id");

    let shared_condition = "the database contains at least one item";
    let group = ClauseGroup {
        section_path: "Gen > Given > DB".to_string(),
        clauses: vec![&c1, &c2],
        conditions: vec![shared_condition.to_string()],
    };

    let context = GenerationContext {
        spec_context: None,
        source_files: vec![],
        schema_files: vec![],
        target_language: Language::Rust,
        verbose: false,
    };

    let prompt = build_batch_prompt(&group, &context);

    // The prompt must contain the "Preconditions (GIVEN)" section that instructs
    // the LLM to set up a shared fixture/helper for all clauses in this group.
    assert!(
        prompt.contains("Preconditions (GIVEN)"),
        "should_generate_a_shared_setup: build_batch_prompt must emit a 'Preconditions (GIVEN)' \
         section so the LLM can generate a shared setup function or fixture; section not found in prompt"
    );
    assert!(
        prompt.contains(shared_condition),
        "should_generate_a_shared_setup: the shared GIVEN condition text must appear in the \
         Preconditions section so the LLM knows what state to establish"
    );
    // The prompt should tell the LLM to use the precondition for test setup
    assert!(
        prompt.contains("set up test preconditions"),
        "should_generate_a_shared_setup: prompt must instruct the LLM to use preconditions as \
         test setup, enabling generation of a shared fixture"
    );
}