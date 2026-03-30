/// MUST validate the syntax of all spec files without generating or running anything
#[test]
fn test_cli__check__must_validate_the_syntax_of_all_spec_files_without_generating_or() {
    let base = std::env::temp_dir()
        .join(format!("ought_check_syntax_{}", std::process::id()));
    let spec_dir = base.join("specs");
    std::fs::create_dir_all(&spec_dir).unwrap();

    std::fs::write(
        spec_dir.join("auth.ought.md"),
        "# Auth\n\n## Login\n\n- **MUST** return a JWT token\n- **MUST NOT** expose password hashes\n",
    )
    .unwrap();
    std::fs::write(
        spec_dir.join("api.ought.md"),
        "# API\n\n## Endpoints\n\n- **MUST** respond within 200ms\n- **SHOULD** use pagination\n",
    )
    .unwrap();

    // Snapshot the directory state before invoking the check operation.
    let mut before: Vec<_> = std::fs::read_dir(&base)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .collect();
    before.sort();

    let result = ought_spec::SpecGraph::from_roots(&[spec_dir.clone()]);

    assert!(
        result.is_ok(),
        "check must succeed for valid spec files; errors: {:?}",
        result.err()
    );
    let graph = result.unwrap();
    assert_eq!(
        graph.specs().len(),
        2,
        "check must validate every spec file found in the roots"
    );

    // Check must be a read-only operation — no new files or directories created.
    let mut after: Vec<_> = std::fs::read_dir(&base)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .collect();
    after.sort();
    assert_eq!(
        before, after,
        "check must not generate or write any files"
    );

    let _ = std::fs::remove_dir_all(&base);
}