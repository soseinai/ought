/// MUST include the clause identifier in the test function name
#[test]
fn test_generator__test_generation__must_include_the_clause_identifier_in_the_test_function_name() {
    use ought_gen::context::GenerationContext;
    use ought_gen::generator::Language;
    use ought_gen::providers::{build_prompt, derive_file_path};
    use ought_spec::{Clause, ClauseId, Keyword, Severity, SourceLocation};
    use std::path::PathBuf;

    let clause_id_str = "auth::login::must_return_401_for_invalid_credentials";
    let clause = Clause {
        id: ClauseId(clause_id_str.to_string()),
        keyword: Keyword::Must,
        severity: Severity::Required,
        text: "return 401 for invalid credentials".to_string(),
        condition: None,
        otherwise: vec![],
        temporal: None,
        hints: vec![],
        source_location: SourceLocation { file: PathBuf::from("auth.ought.md"), line: 7 },
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
        prompt.contains(clause_id_str),
        "must_include_the_clause_identifier_in_the_test_function_name: \
         build_prompt must embed the full clause ID '{}' so the LLM can derive the function name",
        clause_id_str
    );
    let path = derive_file_path(&clause, Language::Rust);
    let path_str = path.to_string_lossy();
    assert!(
        path_str.contains("auth") && path_str.contains("login"),
        "must_include_the_clause_identifier_in_the_test_function_name: \
         derive_file_path must encode clause ID segments in the path; got '{}'",
        path_str
    );
    assert!(
        path_str.ends_with("_test.rs"),
        "must_include_the_clause_identifier_in_the_test_function_name: \
         Rust test file must end in '_test.rs'; got '{}'",
        path_str
    );
}