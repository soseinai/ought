/// MUST report parse errors with file, line number, and a human-readable message
#[test]
fn test_cli__check__must_report_parse_errors_with_file_line_number_and_a_human_readab() {
    let base = std::env::temp_dir()
        .join(format!("ought_check_errmsg_{}", std::process::id()));
    std::fs::create_dir_all(&base).unwrap();

    // A missing file is the canonical trigger for a file-level ParseError.
    let missing = base.join("nonexistent.ought.md");

    let errors = ought_spec::Parser::parse_file(&missing)
        .expect_err("parsing a missing file must return Vec<ParseError>");

    assert!(!errors.is_empty(), "at least one parse error must be reported");
    let err = &errors[0];

    // Must carry the correct file path.
    assert_eq!(err.file, missing, "error must name the problematic file");

    // The Display impl must produce the canonical `file:line: message` format.
    let displayed = err.to_string();
    assert!(
        displayed.contains(missing.to_str().unwrap()),
        "Display must embed the file path; got: {:?}",
        displayed
    );
    let rest = displayed.trim_start_matches(missing.to_str().unwrap());
    assert!(
        rest.starts_with(':'),
        "Display must follow `file:line: message` format; got: {:?}",
        displayed
    );

    // Message must be non-trivially human-readable.
    assert!(
        !err.message.is_empty(),
        "parse error message must not be empty"
    );
    assert!(
        err.message.split_whitespace().count() >= 2,
        "message must be human-readable (≥ 2 words); got: {:?}",
        err.message
    );

    let _ = std::fs::remove_dir_all(&base);
}