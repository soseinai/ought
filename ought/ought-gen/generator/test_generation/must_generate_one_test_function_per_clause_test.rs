/// MUST generate one test function per clause
#[test]
fn test_generator__test_generation__must_generate_one_test_function_per_clause() {
    use ought_gen::generator::{ClauseGroup, Language};
    use ought_gen::providers::parse_batch_response;
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

    let c1 = mk("gen::test::must_a", "do a");
    let c2 = mk("gen::test::must_b", "do b");
    let c3 = mk("gen::test::must_c", "do c");
    let response = [
        "// === CLAUSE: gen::test::must_a ===",
        "#[test]",
        "fn test_gen__test__must_a() { assert!(true); }",
        "",
        "// === CLAUSE: gen::test::must_b ===",
        "#[test]",
        "fn test_gen__test__must_b() { assert!(true); }",
        "",
        "// === CLAUSE: gen::test::must_c ===",
        "#[test]",
        "fn test_gen__test__must_c() { assert!(true); }",
    ].join("\n");
    let group = ClauseGroup {
        section_path: "Gen > Test".to_string(),
        clauses: vec![&c1, &c2, &c3],
        conditions: vec![],
    };
    let tests = parse_batch_response(&response, &group, Language::Rust);
    assert_eq!(
        tests.len(),
        3,
        "must_generate_one_test_function_per_clause: expected 3 GeneratedTests for 3 clauses, got {}",
        tests.len()
    );
    for (i, expected_id) in ["gen::test::must_a", "gen::test::must_b", "gen::test::must_c"]
        .iter()
        .enumerate()
    {
        assert_eq!(
            tests[i].clause_id,
            ClauseId(expected_id.to_string()),
            "test[{}] must map to clause id '{}'",
            i,
            expected_id
        );
    }
}