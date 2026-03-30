/// WONT auto-add clauses without user confirmation —
/// survey must never modify spec files on its own.
#[test]
fn test_analysis__survey__wont_auto_add_clauses_without_user_confirmation() {
    struct MockJsonGenerator {
        json: String,
    }
    impl Generator for MockJsonGenerator {
        fn generate(&self, _: &Clause, _: &GenerationContext) -> anyhow::Result<GeneratedTest> {
            Ok(GeneratedTest {
                clause_id: ClauseId("survey::analysis".to_string()),
                code: self.json.clone(),
                language: Language::Rust,
                file_path: PathBuf::from("_survey.json"),
            })
        }
    }

    let base = std::env::temp_dir()
        .join(format!("ought_survey_noautoadd_{}", std::process::id()));
    let src_dir = base.join("src");
    let spec_dir = base.join("specs");
    fs::create_dir_all(&src_dir).unwrap();
    fs::create_dir_all(&spec_dir).unwrap();

    let src_file = src_dir.join("lib.rs");
    fs::write(&src_file, "pub fn uncovered_fn() {}\n").unwrap();

    let spec_path = spec_dir.join("svc.ought.md");
    let original_spec = "# Svc\n\n## Core\n\n- **MUST** do one thing\n";
    fs::write(&spec_path, original_spec).unwrap();

    let behaviors_json = format!(
        r#"[{{"file":"{f}","line":1,"description":"uncovered_fn has no clause","suggested_clause":"MUST implement uncovered_fn","suggested_keyword":"Must","suggested_spec":"{s}"}}]"#,
        f = src_file.display(),
        s = spec_path.display()
    );

    let specs = SpecGraph::from_roots(&[spec_dir.clone()]).expect("spec graph should parse");

    // Snapshot every spec file's content and mtime before calling survey.
    let spec_content_before = fs::read_to_string(&spec_path).unwrap();
    let spec_mtime_before = fs::metadata(&spec_path).unwrap().modified().unwrap();

    let res = survey(
        &specs,
        &[src_dir.clone()],
        &MockJsonGenerator { json: behaviors_json },
    );
    assert!(res.is_ok(), "survey must succeed");

    // After survey completes, the spec file must be identical — no auto-appended clauses.
    let spec_content_after = fs::read_to_string(&spec_path).unwrap();
    let spec_mtime_after = fs::metadata(&spec_path).unwrap().modified().unwrap();

    assert_eq!(
        spec_content_before, spec_content_after,
        "survey must not modify spec files without user confirmation"
    );
    assert_eq!(
        spec_mtime_before, spec_mtime_after,
        "spec file mtime must be unchanged — survey must not write to it"
    );

    // Verify that the suggestion IS present in the result (survey found it)
    // but did NOT write it to disk.
    let result = res.unwrap();
    assert!(
        !result.uncovered.is_empty(),
        "survey must still return suggestions even though it did not write them"
    );

    let _ = fs::remove_dir_all(&base);
}
```