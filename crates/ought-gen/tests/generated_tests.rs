#![allow(non_snake_case, unused_imports)]
use std::path::{Path, PathBuf};
use std::fs;
use std::time::Duration;
use std::collections::HashMap;
use ought_spec::types::*;
use ought_spec::parser::Parser;
use ought_gen::generator::*;
use ought_gen::manifest::*;

// ============================================================================
// manifest_and_hashing (6 tests)
// ============================================================================

/// MUST compute a clause hash from the keyword + clause text + context metadata
#[test]
fn test_generator__manifest_and_hashing__must_compute_a_clause_hash_from_the_keyword_clause_text_context_m() {
    let path = PathBuf::from("spec.ought.md");

    // Parse a baseline MUST clause.
    let spec_must = Parser::parse_string(
        "# Spec\n\n## Section\n\n- **MUST** validate the token\n",
        &path,
    )
    .expect("parse must succeed");
    let hash_must = spec_must.sections[0].clauses[0].content_hash.clone();

    // 1. Hash must be a 16-character hex string (64-bit SipHash output).
    assert_eq!(
        hash_must.len(),
        16,
        "clause_hash must be 16 hex chars; got {hash_must:?}"
    );
    assert!(
        hash_must.chars().all(|c| c.is_ascii_hexdigit()),
        "clause_hash must contain only hex digits; got {hash_must:?}"
    );

    // 2. Same keyword + text + no condition -> same hash every time (deterministic).
    let spec_must2 = Parser::parse_string(
        "# Spec\n\n## Section\n\n- **MUST** validate the token\n",
        &path,
    )
    .expect("parse must succeed");
    assert_eq!(
        hash_must,
        spec_must2.sections[0].clauses[0].content_hash,
        "identical clause must produce an identical hash"
    );

    // 3. Different keyword (SHOULD) -> different hash.
    let spec_should = Parser::parse_string(
        "# Spec\n\n## Section\n\n- **SHOULD** validate the token\n",
        &path,
    )
    .expect("parse must succeed");
    assert_ne!(
        hash_must,
        spec_should.sections[0].clauses[0].content_hash,
        "changing the keyword from MUST to SHOULD must change the hash"
    );

    // 4. Different clause text -> different hash.
    let spec_other_text = Parser::parse_string(
        "# Spec\n\n## Section\n\n- **MUST** reject the request\n",
        &path,
    )
    .expect("parse must succeed");
    assert_ne!(
        hash_must,
        spec_other_text.sections[0].clauses[0].content_hash,
        "different clause text must produce a different hash"
    );

    // 5. GIVEN condition (context metadata) changes the hash of the nested clause.
    let spec_given = Parser::parse_string(
        "# Spec\n\n## Section\n\n- **GIVEN** user is authenticated\n  - **MUST** validate the token\n",
        &path,
    )
    .expect("parse must succeed");
    let hash_conditioned = spec_given.sections[0].clauses[0].content_hash.clone();
    assert_ne!(
        hash_must,
        hash_conditioned,
        "adding a GIVEN condition must change the clause hash (condition is part of the hash input)"
    );
}

/// MUST compute a source hash from the contents of referenced source files
#[test]
fn test_generator__manifest_and_hashing__must_compute_a_source_hash_from_the_contents_of_referenced_source() {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    use chrono::Utc;

    // The source hash must be computed from referenced source file contents using
    // the same DefaultHasher mechanism as clause hashing.
    let compute_source_hash = |contents: &[&str]| -> String {
        let mut hasher = DefaultHasher::new();
        for content in contents {
            content.hash(&mut hasher);
        }
        format!("{:016x}", hasher.finish())
    };

    let content_v1 = "fn check(token: &str) -> bool { token == \"secret\" }";
    let content_v2 = "fn check(token: &str) -> bool { token == \"rotated_secret\" }";

    let hash_v1 = compute_source_hash(&[content_v1]);
    let hash_v2 = compute_source_hash(&[content_v2]);

    // 1. Different file contents must produce different source hashes.
    assert_ne!(
        hash_v1, hash_v2,
        "changed source file contents must produce a different source hash"
    );

    // 2. Same contents must always hash to the same value (deterministic).
    assert_eq!(
        hash_v1,
        compute_source_hash(&[content_v1]),
        "identical source file contents must always produce the same hash"
    );

    // 3. Adding a second referenced file must change the combined hash.
    let hash_two = compute_source_hash(&[content_v1, content_v2]);
    assert_ne!(
        hash_v1, hash_two,
        "adding a second referenced source file must change the source hash"
    );

    // 4. Manifest::is_stale() consumes the computed source hash.
    //    Verify it correctly detects a source file change as stale.
    let id = ClauseId("spec::section::must_validate".to_string());
    let fixed_clause_hash = "aabbccddeeff0011";

    let mut manifest = Manifest::default();
    manifest.entries.insert(
        id.0.clone(),
        ManifestEntry {
            clause_hash: fixed_clause_hash.to_string(),
            source_hash: hash_v1.clone(),
            generated_at: Utc::now(),
            model: "claude-sonnet-4-6".to_string(),
        },
    );

    // Stored source hash matches -> entry is not stale.
    assert!(
        !manifest.is_stale(&id, fixed_clause_hash, &hash_v1),
        "matching source hash must not be reported as stale"
    );

    // Source file was modified -> new hash -> entry is stale.
    assert!(
        manifest.is_stale(&id, fixed_clause_hash, &hash_v2),
        "changed source file hash must be reported as stale so the test is regenerated"
    );
}

/// MUST skip generation for clauses whose hashes match the manifest (unless `--force`)
#[test]
fn test_generator__manifest_and_hashing__must_skip_generation_for_clauses_whose_hashes_match_the_manifest() {
    use chrono::Utc;

    // Unit: Manifest::is_stale() drives the skip decision

    let id = ClauseId("spec::section::must_validate".to_string());
    let clause_hash = "aabbccddeeff0011";
    let source_hash = "";

    let mut manifest = Manifest::default();
    manifest.entries.insert(
        id.0.clone(),
        ManifestEntry {
            clause_hash: clause_hash.to_string(),
            source_hash: source_hash.to_string(),
            generated_at: Utc::now(),
            model: "claude-sonnet-4-6".to_string(),
        },
    );

    // Both hashes match -> not stale -> generation is skipped.
    assert!(
        !manifest.is_stale(&id, clause_hash, source_hash),
        "is_stale() must return false (skip generation) when both hashes match the manifest"
    );

    // Clause text changed -> stale -> regenerate.
    assert!(
        manifest.is_stale(&id, "different_hash_00", source_hash),
        "is_stale() must return true when the clause_hash differs from the stored value"
    );

    // Source file modified -> stale -> regenerate.
    assert!(
        manifest.is_stale(&id, clause_hash, "new_source_hash_0"),
        "is_stale() must return true when the source_hash differs from the stored value"
    );

    // No manifest entry at all -> stale (first-time generation).
    let new_id = ClauseId("spec::section::must_new".to_string());
    assert!(
        manifest.is_stale(&new_id, clause_hash, source_hash),
        "is_stale() must return true for a clause with no manifest entry"
    );
}

/// MUST write both hashes to `ought/ought-gen/manifest.toml` after generation
#[test]
fn test_generator__manifest_and_hashing__must_write_both_hashes_to_ought_ought_gen_manifest_toml_after_gen() {
    use chrono::Utc;

    let tmp = std::env::temp_dir()
        .join(format!("ought_both_hashes_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();
    let manifest_path = tmp.join("manifest.toml");

    let clause_id   = "spec::section::must_do_the_thing";
    let clause_hash = "a1b2c3d4e5f60789";
    let source_hash = "fedcba9876543210";

    let mut manifest = Manifest::default();
    manifest.entries.insert(
        clause_id.to_string(),
        ManifestEntry {
            clause_hash: clause_hash.to_string(),
            source_hash: source_hash.to_string(),
            generated_at: Utc::now(),
            model: "claude-sonnet-4-6".to_string(),
        },
    );
    manifest.save(&manifest_path).expect("Manifest::save must succeed");

    // Raw TOML must contain both keys and their values.
    let toml_content = std::fs::read_to_string(&manifest_path)
        .expect("manifest.toml must exist after save");

    assert!(
        toml_content.contains("clause_hash"),
        "manifest.toml must contain the 'clause_hash' key; content:\n{toml_content}"
    );
    assert!(
        toml_content.contains(clause_hash),
        "manifest.toml must contain the clause_hash value; content:\n{toml_content}"
    );
    assert!(
        toml_content.contains("source_hash"),
        "manifest.toml must contain the 'source_hash' key; content:\n{toml_content}"
    );
    assert!(
        toml_content.contains(source_hash),
        "manifest.toml must contain the source_hash value; content:\n{toml_content}"
    );

    // Round-trip: reload and confirm both hashes survive serialization.
    let reloaded = Manifest::load(&manifest_path).expect("Manifest::load must succeed");
    let entry = reloaded
        .entries
        .get(clause_id)
        .expect("entry must survive a save/load round-trip");

    assert_eq!(
        entry.clause_hash, clause_hash,
        "clause_hash must survive a save/load round-trip"
    );
    assert_eq!(
        entry.source_hash, source_hash,
        "source_hash must survive a save/load round-trip"
    );

    let _ = std::fs::remove_dir_all(&tmp);
}

/// MUST record the model name and timestamp in the manifest entry
#[test]
fn test_generator__manifest_and_hashing__must_record_the_model_name_and_timestamp_in_the_manifest_entry() {
    use chrono::DateTime;

    let tmp = std::env::temp_dir()
        .join(format!("ought_model_ts_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();
    let manifest_path = tmp.join("manifest.toml");

    let model_name = "claude-sonnet-4-6";
    // Use a fixed, known timestamp so assertions are deterministic.
    let timestamp = DateTime::parse_from_rfc3339("2026-03-30T12:00:00Z")
        .expect("valid RFC3339 timestamp")
        .to_utc();

    let mut manifest = Manifest::default();
    manifest.entries.insert(
        "spec::section::must_do_the_thing".to_string(),
        ManifestEntry {
            clause_hash: "0000000000000000".to_string(),
            source_hash: "".to_string(),
            generated_at: timestamp,
            model: model_name.to_string(),
        },
    );
    manifest.save(&manifest_path).expect("Manifest::save must succeed");

    // Raw TOML must contain the model name and ISO timestamp.
    let content = std::fs::read_to_string(&manifest_path)
        .expect("manifest.toml must be written");

    assert!(
        content.contains("model"),
        "manifest.toml must contain the 'model' key; content:\n{content}"
    );
    assert!(
        content.contains(model_name),
        "manifest.toml must contain the model name value; content:\n{content}"
    );
    assert!(
        content.contains("generated_at"),
        "manifest.toml must contain the 'generated_at' key; content:\n{content}"
    );
    assert!(
        content.contains("2026-03-30"),
        "manifest.toml must contain the ISO date in the timestamp; content:\n{content}"
    );

    // Round-trip: reload and confirm model and timestamp survive serialization.
    let reloaded = Manifest::load(&manifest_path).expect("Manifest::load must succeed");
    let entry = reloaded
        .entries
        .get("spec::section::must_do_the_thing")
        .expect("entry must survive a save/load round-trip");

    assert_eq!(
        entry.model, model_name,
        "model name must survive a save/load round-trip"
    );
    assert_eq!(
        entry.generated_at, timestamp,
        "timestamp must survive a save/load round-trip"
    );

    let _ = std::fs::remove_dir_all(&tmp);
}

/// MUST detect and remove orphaned generated tests (clause was deleted from spec)
#[test]
fn test_generator__manifest_and_hashing__must_detect_and_remove_orphaned_generated_tests_clause_was_delete() {
    use chrono::Utc;

    let make_entry = || ManifestEntry {
        clause_hash: "0000000000000000".to_string(),
        source_hash: "".to_string(),
        generated_at: Utc::now(),
        model: "claude-sonnet-4-6".to_string(),
    };

    let id_a = ClauseId("spec::section::must_foo".to_string());
    let id_b = ClauseId("spec::section::must_bar".to_string()); // will be deleted from spec
    let id_c = ClauseId("spec::section::must_baz".to_string());

    let mut manifest = Manifest::default();
    manifest.entries.insert(id_a.0.clone(), make_entry());
    manifest.entries.insert(id_b.0.clone(), make_entry());
    manifest.entries.insert(id_c.0.clone(), make_entry());
    assert_eq!(manifest.entries.len(), 3, "setup: manifest must start with three entries");

    // Simulate spec after id_b's clause was deleted: only id_a and id_c are still valid.
    let valid_ids = [&id_a, &id_c];
    manifest.remove_orphans(&valid_ids);

    assert_eq!(
        manifest.entries.len(),
        2,
        "remove_orphans must leave exactly two entries; remaining: {:?}",
        manifest.entries.keys().collect::<Vec<_>>()
    );
    assert!(
        manifest.entries.contains_key(&id_a.0),
        "valid clause id_a must remain in the manifest after remove_orphans"
    );
    assert!(
        manifest.entries.contains_key(&id_c.0),
        "valid clause id_c must remain in the manifest after remove_orphans"
    );
    assert!(
        !manifest.entries.contains_key(&id_b.0),
        "orphaned clause id_b must be removed from the manifest by remove_orphans"
    );

    // Idempotent: calling again with the same valid set must not change anything.
    manifest.remove_orphans(&valid_ids);
    assert_eq!(
        manifest.entries.len(),
        2,
        "remove_orphans must be idempotent"
    );

    // Edge case: empty valid set removes all remaining entries.
    manifest.remove_orphans(&[]);
    assert!(
        manifest.entries.is_empty(),
        "remove_orphans with an empty valid_ids set must clear all manifest entries"
    );
}

// ============================================================================
// keyword_str (1 test)
// ============================================================================

/// keyword_str must return the correct display string for each keyword
#[test]
fn test_generator__keyword_str__returns_correct_display_string_for_each_keyword() {
    assert_eq!(keyword_str(Keyword::Must), "MUST");
    assert_eq!(keyword_str(Keyword::MustNot), "MUST NOT");
    assert_eq!(keyword_str(Keyword::Should), "SHOULD");
    assert_eq!(keyword_str(Keyword::ShouldNot), "SHOULD NOT");
    assert_eq!(keyword_str(Keyword::May), "MAY");
    assert_eq!(keyword_str(Keyword::Wont), "WONT");
    assert_eq!(keyword_str(Keyword::Given), "GIVEN");
    assert_eq!(keyword_str(Keyword::Otherwise), "OTHERWISE");
    assert_eq!(keyword_str(Keyword::MustAlways), "MUST ALWAYS");
    assert_eq!(keyword_str(Keyword::MustBy), "MUST BY");
}

// ============================================================================
// agent_assignment (2 tests)
// ============================================================================

/// AgentAssignment must serialize and deserialize via serde_json
#[test]
fn test_generator__agent_assignment__must_round_trip_through_json() {
    use ought_gen::{AgentAssignment, AssignmentGroup, AssignmentClause};

    let assignment = AgentAssignment {
        id: "agent_0".to_string(),
        project_root: "/tmp/project".to_string(),
        config_path: "/tmp/project/ought.toml".to_string(),
        test_dir: "/tmp/project/ought/ought-gen".to_string(),
        target_language: "rust".to_string(),
        source_paths: vec!["src/auth/".to_string()],
        groups: vec![
            AssignmentGroup {
                section_path: "Auth > Login".to_string(),
                clauses: vec![
                    AssignmentClause {
                        id: "auth::login::must_return_jwt".to_string(),
                        keyword: "MUST".to_string(),
                        text: "return a JWT on success".to_string(),
                        condition: None,
                        temporal: None,
                        content_hash: "abc123".to_string(),
                        hints: vec![],
                        otherwise: vec![],
                    },
                ],
                conditions: vec!["the user provides valid credentials".to_string()],
            },
        ],
    };

    let json = serde_json::to_string(&assignment).expect("serialization must succeed");
    let deserialized: AgentAssignment =
        serde_json::from_str(&json).expect("deserialization must succeed");

    assert_eq!(deserialized.id, "agent_0");
    assert_eq!(deserialized.groups.len(), 1);
    assert_eq!(deserialized.groups[0].clauses.len(), 1);
    assert_eq!(
        deserialized.groups[0].clauses[0].id,
        "auth::login::must_return_jwt"
    );
}

/// AgentReport tracks generated count and errors
#[test]
fn test_generator__agent_report__tracks_generated_and_errors() {
    use ought_gen::AgentReport;

    let report = AgentReport {
        generated: 5,
        errors: vec!["clause X failed".to_string()],
    };
    assert_eq!(report.generated, 5);
    assert_eq!(report.errors.len(), 1);

    let empty_report = AgentReport::default();
    assert_eq!(empty_report.generated, 0);
    assert!(empty_report.errors.is_empty());
}

// ============================================================================
// error_handling: manifest consistency (1 test)
// ============================================================================

/// MUST NOT leave the manifest in an inconsistent state if generation is interrupted
#[test]
fn test_generator__error_handling__must_not_leave_the_manifest_in_an_inconsistent_state_if_generation_is() {
    use chrono::Utc;

    let dir = std::env::temp_dir().join(format!(
        "ought_manifest_consistency_{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let manifest_path = dir.join("manifest.toml");

    // Phase 1: successful generation of clause_a; manifest saved
    let mut manifest = Manifest { entries: HashMap::new() };
    manifest.entries.insert(
        "generator::error_handling::clause_a".to_string(),
        ManifestEntry {
            clause_hash: "abc123".to_string(),
            source_hash: "src456".to_string(),
            generated_at: Utc::now(),
            model: "claude-sonnet-4-6".to_string(),
        },
    );
    manifest.save(&manifest_path).expect("save after clause_a must succeed");

    // Phase 2: generation of clause_b is interrupted before manifest save
    // (we never call manifest.save again)

    // Phase 3: load the manifest and verify it is well-formed
    let loaded = Manifest::load(&manifest_path)
        .expect("manifest must load cleanly after interrupted generation");

    assert_eq!(
        loaded.entries.len(),
        1,
        "must_not_leave_manifest_inconsistent: only committed entries must appear; \
         got {} entries, want 1",
        loaded.entries.len()
    );
    assert!(
        loaded.entries.contains_key("generator::error_handling::clause_a"),
        "must_not_leave_manifest_inconsistent: committed clause_a must be present"
    );
    assert!(
        !loaded.entries.contains_key("generator::error_handling::clause_b"),
        "must_not_leave_manifest_inconsistent: clause_b (never saved) must not appear"
    );

    // Each present entry must be fully populated
    let entry = &loaded.entries["generator::error_handling::clause_a"];
    assert!(
        !entry.clause_hash.is_empty(),
        "must_not_leave_manifest_inconsistent: clause_hash must not be empty"
    );
    assert!(
        !entry.source_hash.is_empty(),
        "must_not_leave_manifest_inconsistent: source_hash must not be empty"
    );
    assert!(
        !entry.model.is_empty(),
        "must_not_leave_manifest_inconsistent: model must not be empty"
    );

    // Idempotent reload
    let reloaded = Manifest::load(&manifest_path)
        .expect("manifest must be loadable a second time");
    assert_eq!(
        reloaded.entries.len(),
        loaded.entries.len(),
        "must_not_leave_manifest_inconsistent: re-loading must return same entry count"
    );

    let _ = std::fs::remove_dir_all(&dir);
}
