/// MUST read source files from the given path (or project source roots if no path given)
#[test]
fn test_analysis__survey__must_read_source_files_from_the_given_path_or_project_source_root() {
    struct StubGenerator;
    impl Generator for StubGenerator {
        fn generate(&self, _: &Clause, _: &GenerationContext) -> anyhow::Result<GeneratedTest> {
            unimplemented!("stub")
        }
    }

    let base = std::env::temp_dir()
        .join(format!("ought_survey_srcpath_{}", std::process::id()));
    let src_dir = base.join("src");
    let out_of_scope = base.join("other");
    let spec_dir = base.join("specs");
    fs::create_dir_all(&src_dir).unwrap();
    fs::create_dir_all(&out_of_scope).unwrap();
    fs::create_dir_all(&spec_dir).unwrap();

    fs::write(src_dir.join("lib.rs"), "pub fn add(a: i32, b: i32) -> i32 { a + b }\n").unwrap();
    fs::write(out_of_scope.join("other.rs"), "pub fn hidden() {}\n").unwrap();

    let spec_md = format!(
        "# Math\n\nsource: {}\n\n## Ops\n\n- **MUST** handle addition\n",
        src_dir.display()
    );
    fs::write(spec_dir.join("math.ought.md"), &spec_md).unwrap();

    let specs = SpecGraph::from_roots(&[spec_dir.clone()]).expect("spec graph should parse");

    // Explicit path: every reported file must be inside the given path.
    let res = survey(&specs, &[src_dir.clone()], &StubGenerator);
    assert!(res.is_ok(), "survey with explicit path must succeed");
    for b in &res.unwrap().uncovered {
        assert!(
            b.file.starts_with(&src_dir),
            "behavior {:?} must originate from the given path {:?}",
            b.file,
            src_dir
        );
    }

    // No path: survey must fall back to project source roots, not read arbitrary dirs.
    let res_default = survey(&specs, &[], &StubGenerator);
    assert!(
        res_default.is_ok(),
        "survey with empty paths must fall back to project source roots"
    );
    for b in &res_default.unwrap().uncovered {
        assert!(
            !b.file.starts_with(&out_of_scope),
            "must not read files outside project source roots; got {:?}",
            b.file
        );
    }

    let _ = fs::remove_dir_all(&base);
}