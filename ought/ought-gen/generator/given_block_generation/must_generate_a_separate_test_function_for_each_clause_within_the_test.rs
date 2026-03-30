/// MUST generate a separate test function for each clause within the GIVEN block, all sharing the same precondition setup
#[test]
fn test_generator__given_block_generation__must_generate_a_separate_test_function_for_each_clause_within_the() {
    use ought_gen::generator::{ClauseGroup, Language};
    use ought_gen::providers::parse_batch_response;
    use ought_spec::{Clause, ClauseId, Keyword, Severity, SourceLocation};
    use std::path::PathBuf;

    fn mk_clause(id: &str, text: &str, condition: &str) -> Clause {
        Clause {
            id: ClauseId(id.to_string()),
            keyword: Keyword::Must,
            severity: Severity::Required,
            text: text.to_string(),
            condition: Some(condition.to_string()),
            otherwise: vec![],
            temporal: None,
            hints: vec![],
            source_location: SourceLocation { file: PathBuf::from("spec.ought.md"), line: 1 },
            content_hash: "h".to_string(),
        }
    }

    let shared_condition = "the user is authenticated";
    let c1 = mk_clause("gen::given::must_allow_read", "allow read access", shared_condition);
    let c2 = mk_clause("gen::given::must_allow_write", "allow write access", shared_condition);

    let response = [
        "// === CLAUSE: gen::given::must_allow_read ===",
        "#[test]",
        "fn test_gen__given__must_allow_read() {",
        "    // setup: the user is authenticated",
        "    assert!(true);",
        "}",
        "",
        "// === CLAUSE: gen::given::must_allow_write ===",
        "#[test]",
        "fn test_gen__given__must_allow_write() {",
        "    // setup: the user is authenticated",
        "    assert!(true);",
        "}",
    ].join("\n");

    let group = ClauseGroup {
        section_path: "Gen > Given".to_string(),
        clauses: vec![&c1, &c2],
        conditions: vec![shared_condition.to_string()],
    };

    let tests = parse_batch_response(&response, &group, Language::Rust);

    assert_eq!(
        tests.len(),
        2,
        "must_generate_a_separate_test_function_for_each_clause: expected 2 GeneratedTests for 2 clauses under the same GIVEN block, got {}",
        tests.len()
    );
    assert_eq!(
        tests[0].clause_id,
        ClauseId("gen::given::must_allow_read".to_string()),
        "first test must map to the first clause id"
    );
    assert_eq!(
        tests[1].clause_id,
        ClauseId("gen::given::must_allow_write".to_string()),
        "second test must map to the second clause id"
    );
    assert_ne!(
        tests[0].clause_id, tests[1].clause_id,
        "each clause must produce a distinct test function"
    );
}