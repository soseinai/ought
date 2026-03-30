/// SHOULD group suggestions by the spec file they would belong to
#[test]
fn test_analysis__survey__should_group_suggestions_by_the_spec_file_they_would_belong_to() {
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
        .join(format!("ought_survey_grouping_{}", std::process::id()));
    let src_dir = base.join("src");
    let spec_dir = base.join("specs");
    fs::create_dir_all(&src_dir).unwrap();
    fs::create_dir_all(&spec_dir).unwrap();

    let src_file = src_dir.join("lib.rs");
    fs::write(
        &src_file,
        "pub fn auth_login() {}\npub fn auth_logout() {}\npub fn billing_charge() {}\n",
    )
    .unwrap();
    fs::write(spec_dir.join("base.ought.md"), "# Base\n\n## Core\n\n- **MUST** exist\n").unwrap();

    let auth_spec = spec_dir.join("auth.ought.md");
    let billing_spec = spec_dir.join("billing.ought.md");

    // Three behaviors: two belong to auth.ought.md, one to billing.ought.md.
    // Survey should present them grouped (behaviors for the same spec are adjacent).
    let behaviors_json = format!(
        r#"[
          {{"file":"{f}","line":1,"description":"auth_login uncovered","suggested_clause":"MUST authenticate the user on login","suggested_keyword":"Must","suggested_spec":"{auth}"}},
          {{"file":"{f}","line":2,"description":"auth_logout uncovered","suggested_clause":"MUST invalidate session on logout","suggested_keyword":"Must","suggested_spec":"{auth}"}},
          {{"file":"{f}","line":3,"description":"billing_charge uncovered","suggested_clause":"MUST charge the correct amount","suggested_keyword":"Must","suggested_spec":"{billing}"}}
        ]"#,
        f = src_file.display(),
        auth = auth_spec.display(),
        billing = billing_spec.display()
    );

    let specs = SpecGraph::from_roots(&[spec_dir.clone()]).expect("spec graph should parse");
    let res = survey(
        &specs,
        &[src_dir.clone()],
        &MockJsonGenerator { json: behaviors_json },
    );
    assert!(res.is_ok(), "survey must succeed");

    let result = res.unwrap();
    assert_eq!(result.uncovered.len(), 3, "all three behaviors must be reported");

    // Verify grouping: behaviors for the same spec file must be adjacent.
    // Detect whether any spec file's behaviors are split (interleaved with another's).
    let mut seen_specs: Vec<PathBuf> = Vec::new();
    let mut last_spec: Option<PathBuf> = None;
    for b in &result.uncovered {
        if last_spec.as_ref() != Some(&b.suggested_spec) {
            assert!(
                !seen_specs.contains(&b.suggested_spec),
                "behaviors for {:?} are not grouped — they appear interleaved with other spec files",
                b.suggested_spec
            );
            if let Some(prev) = last_spec.take() {
                seen_specs.push(prev);
            }
            last_spec = Some(b.suggested_spec.clone());
        }
    }

    let _ = fs::remove_dir_all(&base);
}