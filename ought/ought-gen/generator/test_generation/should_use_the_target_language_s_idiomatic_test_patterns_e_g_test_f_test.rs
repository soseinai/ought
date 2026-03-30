/// SHOULD use the target language's idiomatic test patterns (e.g. `#[test]` for Rust, `test()` for Jest)
#[test]
fn test_generator__test_generation__should_use_the_target_language_s_idiomatic_test_patterns_e_g_test_f() {
    use ought_gen::context::GenerationContext;
    use ought_gen::generator::Language;
    use ought_gen::providers::build_prompt;
    use ought_spec::{Clause, ClauseId, Keyword, Severity, SourceLocation};
    use std::path::PathBuf;

    fn mk_clause() -> Clause {
        Clause {
            id: ClauseId("gen::test::must_thing".to_string()),
            keyword: Keyword::Must,
            severity: Severity::Required,
            text: "do the thing".to_string(),
            condition: None,
            otherwise: vec![],
            temporal: None,
            hints: vec![],
            source_location: SourceLocation { file: PathBuf::from("spec.ought.md"), line: 1 },
            content_hash: "x".to_string(),
        }
    }

    fn mk_ctx(lang: Language) -> GenerationContext {
        GenerationContext {
            spec_context: None,
            source_files: vec![],
            schema_files: vec![],
            target_language: lang,
            verbose: false,
        }
    }

    let clause = mk_clause();

    let rust_prompt = build_prompt(&clause, &mk_ctx(Language::Rust));
    assert!(
        rust_prompt.contains("#[test]"),
        "should_use_idiomatic_test_patterns: Rust prompt must mention '#[test]' attribute"
    );
    assert!(
        rust_prompt.contains("assert!"),
        "should_use_idiomatic_test_patterns: Rust prompt must mention 'assert!' macro"
    );

    let ts_prompt = build_prompt(&clause, &mk_ctx(Language::TypeScript));
    assert!(
        ts_prompt.contains("test()") || ts_prompt.contains("it()"),
        "should_use_idiomatic_test_patterns: TypeScript prompt must mention Jest-style 'test()' or 'it()'"
    );

    let go_prompt = build_prompt(&clause, &mk_ctx(Language::Go));
    assert!(
        go_prompt.contains("func Test") || go_prompt.contains("testing.T"),
        "should_use_idiomatic_test_patterns: Go prompt must mention 'func Test' or 'testing.T'"
    );

    let py_prompt = build_prompt(&clause, &mk_ctx(Language::Python));
    assert!(
        py_prompt.contains("def test"),
        "should_use_idiomatic_test_patterns: Python prompt must mention 'def test' naming convention"
    );
}