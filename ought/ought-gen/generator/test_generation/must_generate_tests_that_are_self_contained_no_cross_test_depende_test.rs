/// MUST generate tests that are self-contained (no cross-test dependencies)
#[test]
fn test_generator__test_generation__must_generate_tests_that_are_self_contained_no_cross_test_depende() {
    use ought_gen::context::GenerationContext;
    use ought_gen::generator::{ClauseGroup, Language};
    use ought_gen::providers::{build_prompt, parse_batch_response};
    use ought_spec::{Clause, ClauseId, Keyword, Severity, SourceLocation};
    use std::path::PathBuf;

    fn mk(id: &str, text: &str) -> Clause {
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
            content_hash: "x".to_string(),
        }
    }

    let lone = mk("gen::must_isolated", "be isolated from other tests");
    let context = GenerationContext {
        spec_context: None,
        source_files: vec![],
        schema_files: vec![],
        target_language: Language::Rust,
        verbose: false,
    };
    assert!(
        build_prompt(&lone, &context).contains("self-contained"),
        "must_generate_tests_that_are_self_contained: build_prompt must require self-contained tests"
    );

    // parse_batch_response must keep each test's code isolated between its markers
    let c1 = mk("gen::must_x", "x");
    let c2 = mk("gen::must_y", "y");
    let response = [
        "// === CLAUSE: gen::must_x ===",
        "fn test_x() { let v = 1; assert_eq!(v, 1, \"gen::must_x\"); }",
        "// === CLAUSE: gen::must_y ===",
        "fn test_y() { let v = 2; assert_eq!(v, 2, \"gen::must_y\"); }",
    ]
    .join("\n");
    let group = ClauseGroup {
        section_path: "Gen".to_string(),
        clauses: vec![&c1, &c2],
        conditions: vec![],
    };
    let tests = parse_batch_response(&response, &group, Language::Rust);
    assert_eq!(tests.len(), 2, "expected 2 isolated tests");
    assert!(
        !tests[0].code.contains("test_y"),
        "must_generate_tests_that_are_self_contained: test[0] code must not reference test_y; got: {:?}",
        tests[0].code
    );
    assert!(
        !tests[1].code.contains("test_x"),
        "must_generate_tests_that_are_self_contained: test[1] code must not reference test_x; got: {:?}",
        tests[1].code
    );
}