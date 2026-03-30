/// MUST validate that cross-file references (links to other .ought.md files) resolve
#[test]
fn test_cli__check__must_validate_that_cross_file_references_links_to_other_ought_md() {
    let base = std::env::temp_dir()
        .join(format!("ought_check_xref_{}", std::process::id()));
    let valid_dir = base.join("valid");
    let broken_dir = base.join("broken");
    std::fs::create_dir_all(&valid_dir).unwrap();
    std::fs::create_dir_all(&broken_dir).unwrap();

    // ── valid case: referenced file exists in the same roots ─────────────────
    std::fs::write(
        valid_dir.join("base.ought.md"),
        "# Base\n\n## Core\n\n- **MUST** exist\n",
    )
    .unwrap();
    std::fs::write(
        valid_dir.join("extension.ought.md"),
        "# Extension\n\nrequires: [Base](./base.ought.md)\n\n## Extra\n\n- **MUST** build on base\n",
    )
    .unwrap();

    let valid_result = ought_spec::SpecGraph::from_roots(&[valid_dir.clone()]);
    assert!(
        valid_result.is_ok(),
        "specs whose cross-file references resolve must pass check; errors: {:?}",
        valid_result.err()
    );
    assert_eq!(
        valid_result.unwrap().specs().len(),
        2,
        "both the referencing and referenced spec must be loaded"
    );

    // ── broken case: referenced file does not exist ───────────────────────────
    std::fs::write(
        broken_dir.join("consumer.ought.md"),
        "# Consumer\n\nrequires: [Ghost](./ghost.ought.md)\n\n## Usage\n\n- **MUST** depend on ghost\n",
    )
    .unwrap();
    // ghost.ought.md is intentionally absent.

    let broken_result = ought_spec::SpecGraph::from_roots(&[broken_dir.clone()]);
    assert!(
        broken_result.is_err(),
        "specs with unresolvable cross-file references must fail check"
    );
    let errs = broken_result.unwrap_err();
    let combined = errs
        .iter()
        .map(|e| e.to_string())
        .collect::<Vec<_>>()
        .join("; ")
        .to_lowercase();
    assert!(
        combined.contains("ghost")
            || combined.contains("not found")
            || combined.contains("resolve")
            || combined.contains("missing"),
        "error must reference the unresolved file; got: {:?}",
        combined
    );

    let _ = std::fs::remove_dir_all(&base);
}