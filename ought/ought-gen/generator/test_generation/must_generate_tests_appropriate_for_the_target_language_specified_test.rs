/// MUST generate tests appropriate for the target language specified in `ought.toml`
#[test]
fn test_generator__test_generation__must_generate_tests_appropriate_for_the_target_language_specified() {
    use ought_gen::context::GenerationContext;
    use ought_gen::generator::Language;
    use ought_gen::providers::{build_prompt, derive_file_path};
    use ought_spec::{Clause, ClauseId, Keyword, Severity, SourceLocation};
    use std::path::PathBuf;

    let clause = Clause {
        id: ClauseId("gen::must_thing".to_string()),
        keyword: Keyword::Must,
        severity: Severity::Required,
        text: "do the thing".to_string(),
        condition: None,
        otherwise: vec![],
        temporal: None,
        hints: vec![],
        source_location: SourceLocation { file: PathBuf::from("spec.ought.md"), line: 1 },
        content_hash: "x".to_string(),
    };

    // File extension must match the configured language
    for (lang, expected_suffix) in &[
        (Language::Rust,       "_test.rs"),
        (Language::Python,     "_test.py"),
        (Language::TypeScript, ".test.ts"),
        (Language::JavaScript, ".test.js"),
        (Language::Go,         "_test.go"),
    ] {
        let path = derive_file_path(&clause, *lang);
        assert!(
            path.to_string_lossy().ends_with(expected_suffix),
            "must_generate_tests_appropriate_for_the_target_language_specified: \
             {:?} must produce a file ending in '{}'; got '{}'",
            lang, expected_suffix, path.display()
        );
    }

    // build_prompt must name the language so the LLM generates language-appropriate tests
    for (lang, lang_name) in &[
        (Language::Rust,       "Rust"),
        (Language::Python,     "Python"),
        (Language::TypeScript, "TypeScript"),
        (Language::JavaScript, "JavaScript"),
        (Language::Go,         "Go"),
    ] {
        let ctx = GenerationContext {
            spec_context: None,
            source_files: vec![],
            schema_files: vec![],
            target_language: *lang,
            verbose: false,
        };
        let prompt = build_prompt(&clause, &ctx);
        assert!(
            prompt.contains(lang_name),
            "must_generate_tests_appropriate_for_the_target_language_specified: \
             build_prompt for {:?} must mention '{}' so the LLM targets the right language",
            lang, lang_name
        );
    }
}