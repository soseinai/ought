/// MAY generate helper functions or fixtures when multiple clauses in a section share setup
#[test]
fn test_generator__test_generation__may_generate_helper_functions_or_fixtures_when_multiple_clauses() {
    use ought_gen::context::GenerationContext;
    use ought_gen::generator::{ClauseGroup, Language};
    use ought_gen::providers::{build_batch_prompt, parse_batch_response};
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

    let c1 = mk("auth::login::must_return_jwt", "return a JWT on success");
    let c2 = mk("auth::login::must_set_expiry", "set token expiry to 1 hour");
    let c3 = mk("auth::login::must_reject_bad_password", "reject invalid passwords");

    let context = GenerationContext {
        spec_context: Some("Auth service validates credentials and issues JWTs.".to_string()),
        source_files: vec![],
        schema_files: vec![],
        target_language: Language::Rust,
        verbose: false,
    };
    let group = ClauseGroup {
        section_path: "Auth > Login".to_string(),
        clauses: vec![&c1, &c2, &c3],
        conditions: vec!["the request contains valid credentials".to_string()],
    };

    let prompt = build_batch_prompt(&group, &context);

    // All clause texts must be present so the LLM has full section context for shared fixtures
    for text in &["return a JWT on success", "set token expiry to 1 hour", "reject invalid passwords"] {
        assert!(
            prompt.contains(text),
            "may_generate_helper_functions: batch prompt must include all clause texts; missing: '{}'",
            text
        );
    }
    // Shared GIVEN condition must appear so the LLM can factor it into shared setup
    assert!(
        prompt.contains("valid credentials"),
        "may_generate_helper_functions: shared GIVEN condition must be in batch prompt \
         so the LLM can generate shared test fixtures"
    );

    // parse_batch_response must return one test per clause marker even when a shared helper
    // function appears before the first marker in the LLM output
    let response_with_helper = [
        "fn make_auth_client() -> AuthClient { AuthClient::default() }",
        "",
        "// === CLAUSE: auth::login::must_return_jwt ===",
        "#[test]",
        "fn test_auth__login__must_return_jwt() {",
        "    assert!(make_auth_client().login(\"u\", \"p\").is_ok(), \"auth::login::must_return_jwt\");",
        "}",
        "",
        "// === CLAUSE: auth::login::must_set_expiry ===",
        "#[test]",
        "fn test_auth__login__must_set_expiry() {",
        "    assert_eq!(make_auth_client().token_ttl_seconds(), 3600, \"auth::login::must_set_expiry\");",
        "}",
        "",
        "// === CLAUSE: auth::login::must_reject_bad_password ===",
        "#[test]",
        "fn test_auth__login__must_reject_bad_password() {",
        "    assert!(make_auth_client().login(\"u\", \"wrong\").is_err(), \"auth::login::must_reject_bad_password\");",
        "}",
    ]
    .join("\n");

    let tests = parse_batch_response(&response_with_helper, &group, Language::Rust);
    assert_eq!(
        tests.len(),
        3,
        "may_generate_helper_functions: parse_batch_response must yield exactly 3 tests \
         (one per clause marker) even when a shared helper precedes the first marker; got {}",
        tests.len()
    );
    assert_eq!(
        tests[0].clause_id,
        ClauseId("auth::login::must_return_jwt".to_string()),
        "may_generate_helper_functions: test[0] clause_id mismatch"
    );
    assert_eq!(
        tests[1].clause_id,
        ClauseId("auth::login::must_set_expiry".to_string()),
        "may_generate_helper_functions: test[1] clause_id mismatch"
    );
    assert_eq!(
        tests[2].clause_id,
        ClauseId("auth::login::must_reject_bad_password".to_string()),
        "may_generate_helper_functions: test[2] clause_id mismatch"
    );
}