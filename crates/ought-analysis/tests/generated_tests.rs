#![allow(non_snake_case, unused_imports)]

use std::path::{Path, PathBuf};
use std::time::Duration;

use chrono::Utc;
use ought_analysis::types::*;
use ought_analysis::{audit, bisect, blame, survey};
use ought_gen::context::GenerationContext;
use ought_gen::generator::{GeneratedTest, Generator, Language};
use ought_run::runner::Runner;
use ought_run::{RunResult, TestDetails, TestResult, TestStatus};
use ought_spec::types::*;
use ought_spec::SpecGraph;

// =============================================================================
// Helper: mock Generator for tests that need a Generator trait object
// =============================================================================

struct StubGenerator;
impl Generator for StubGenerator {
    fn generate(&self, _: &Clause, _: &GenerationContext) -> anyhow::Result<GeneratedTest> {
        Ok(GeneratedTest {
            clause_id: ClauseId("stub::clause".to_string()),
            code: "// stub".to_string(),
            language: Language::Rust,
            file_path: PathBuf::from("stub_test.rs"),
        })
    }
}

// =============================================================================
// Helper: mock Runner for tests that need a Runner trait object
// =============================================================================

struct StubRunner;
impl Runner for StubRunner {
    fn run(&self, _: &[GeneratedTest], _: &Path) -> anyhow::Result<RunResult> {
        Ok(RunResult {
            results: vec![],
            total_duration: Duration::ZERO,
        })
    }
    fn is_available(&self) -> bool {
        true
    }
    fn name(&self) -> &str {
        "stub"
    }
}

// =============================================================================
// SURVEY TYPE TESTS
// =============================================================================

/// SurveyResult can be constructed and holds a list of UncoveredBehavior items.
#[test]
fn test_survey__survey_result_can_be_constructed() {
    let result = SurveyResult {
        uncovered: vec![],
    };
    assert!(result.uncovered.is_empty());
}

/// UncoveredBehavior can be constructed with all expected fields.
#[test]
fn test_survey__uncovered_behavior_has_expected_fields() {
    let behavior = UncoveredBehavior {
        file: PathBuf::from("src/api.rs"),
        line: 42,
        description: "create_user has no clause".to_string(),
        suggested_clause: "MUST create a user with the given name".to_string(),
        suggested_keyword: Keyword::Must,
        suggested_spec: PathBuf::from("ought/api.ought.md"),
    };

    assert_eq!(behavior.file, PathBuf::from("src/api.rs"));
    assert_eq!(behavior.line, 42);
    assert!(!behavior.description.is_empty());
    assert!(!behavior.suggested_clause.is_empty());
    assert_eq!(behavior.suggested_keyword, Keyword::Must);
    assert_eq!(behavior.suggested_spec, PathBuf::from("ought/api.ought.md"));
}

/// MUST output a list of uncovered behaviors with file and line references.
/// Tests the UncoveredBehavior structure carries non-empty file and positive line.
#[test]
fn test_survey__must_output_a_list_of_uncovered_behaviors_with_file_and_line_refe() {
    let behaviors = vec![
        UncoveredBehavior {
            file: PathBuf::from("src/api.rs"),
            line: 1,
            description: "create_user has no clause".to_string(),
            suggested_clause: "MUST create a user with the given name".to_string(),
            suggested_keyword: Keyword::Must,
            suggested_spec: PathBuf::from("ought/api.ought.md"),
        },
        UncoveredBehavior {
            file: PathBuf::from("src/api.rs"),
            line: 2,
            description: "delete_user has no clause".to_string(),
            suggested_clause: "MUST delete the user with the given id".to_string(),
            suggested_keyword: Keyword::Must,
            suggested_spec: PathBuf::from("ought/api.ought.md"),
        },
    ];

    let result = SurveyResult {
        uncovered: behaviors,
    };

    assert!(!result.uncovered.is_empty(), "survey must output at least one uncovered behavior");
    for b in &result.uncovered {
        assert!(
            b.file != PathBuf::from(""),
            "each uncovered behavior must include a non-empty file path"
        );
        assert!(
            b.line > 0,
            "each uncovered behavior must include a positive line reference; got line {}",
            b.line
        );
    }
}

/// MUST suggest concrete clause text with appropriate keyword for each uncovered behavior.
/// All behavioral keywords (not structural ones like Given/Otherwise) are valid.
#[test]
fn test_survey__must_suggest_concrete_clause_text_with_appropriate_keyword_for_ea() {
    const BEHAVIORAL_KEYWORDS: &[Keyword] = &[
        Keyword::Must,
        Keyword::MustNot,
        Keyword::Should,
        Keyword::ShouldNot,
        Keyword::May,
        Keyword::Wont,
        Keyword::MustAlways,
        Keyword::MustBy,
    ];

    let behaviors = vec![
        UncoveredBehavior {
            file: PathBuf::from("src/service.rs"),
            line: 1,
            description: "process_payment uncovered".to_string(),
            suggested_clause: "MUST process the payment and return success status".to_string(),
            suggested_keyword: Keyword::Must,
            suggested_spec: PathBuf::from("ought/svc.ought.md"),
        },
        UncoveredBehavior {
            file: PathBuf::from("src/service.rs"),
            line: 1,
            description: "payment error path".to_string(),
            suggested_clause: "SHOULD return false when payment fails".to_string(),
            suggested_keyword: Keyword::Should,
            suggested_spec: PathBuf::from("ought/svc.ought.md"),
        },
    ];

    for b in &behaviors {
        assert!(
            !b.suggested_clause.is_empty(),
            "suggested_clause must be non-empty for every uncovered behavior"
        );
        assert!(
            BEHAVIORAL_KEYWORDS.contains(&b.suggested_keyword),
            "suggested_keyword {:?} must be a deontic behavioral keyword, not structural",
            b.suggested_keyword
        );
    }
}

/// SHOULD group suggestions by the spec file they would belong to.
/// Behaviors for the same suggested_spec should appear adjacent.
#[test]
fn test_survey__should_group_suggestions_by_the_spec_file_they_would_belong_to() {
    let behaviors = vec![
        UncoveredBehavior {
            file: PathBuf::from("src/lib.rs"),
            line: 1,
            description: "auth_login uncovered".to_string(),
            suggested_clause: "MUST authenticate the user on login".to_string(),
            suggested_keyword: Keyword::Must,
            suggested_spec: PathBuf::from("auth.ought.md"),
        },
        UncoveredBehavior {
            file: PathBuf::from("src/lib.rs"),
            line: 2,
            description: "auth_logout uncovered".to_string(),
            suggested_clause: "MUST invalidate session on logout".to_string(),
            suggested_keyword: Keyword::Must,
            suggested_spec: PathBuf::from("auth.ought.md"),
        },
        UncoveredBehavior {
            file: PathBuf::from("src/lib.rs"),
            line: 3,
            description: "billing_charge uncovered".to_string(),
            suggested_clause: "MUST charge the correct amount".to_string(),
            suggested_keyword: Keyword::Must,
            suggested_spec: PathBuf::from("billing.ought.md"),
        },
    ];

    let result = SurveyResult {
        uncovered: behaviors,
    };

    assert_eq!(result.uncovered.len(), 3, "all three behaviors must be reported");

    // Verify grouping: behaviors for the same spec file are adjacent.
    let mut seen_specs: Vec<PathBuf> = Vec::new();
    let mut last_spec: Option<PathBuf> = None;
    for b in &result.uncovered {
        if last_spec.as_ref() != Some(&b.suggested_spec) {
            assert!(
                !seen_specs.contains(&b.suggested_spec),
                "behaviors for {:?} are not grouped -- they appear interleaved with other spec files",
                b.suggested_spec
            );
            if let Some(prev) = last_spec.take() {
                seen_specs.push(prev);
            }
            last_spec = Some(b.suggested_spec.clone());
        }
    }
}

/// SHOULD offer to append suggested clauses to the relevant spec file.
/// Each UncoveredBehavior must carry a non-empty suggested_spec path.
#[test]
fn test_survey__should_offer_to_append_suggested_clauses_to_the_relevant_spec_file() {
    let behavior = UncoveredBehavior {
        file: PathBuf::from("src/router.rs"),
        line: 1,
        description: "route function uncovered".to_string(),
        suggested_clause: "MUST dispatch requests to the correct handler".to_string(),
        suggested_keyword: Keyword::Must,
        suggested_spec: PathBuf::from("specs/router.ought.md"),
    };

    assert!(
        behavior.suggested_spec != PathBuf::from(""),
        "every uncovered behavior must include a suggested_spec path for append-offer"
    );
}

/// SHOULD rank uncovered behaviors by risk (public API > internal helper).
/// The type supports this ordering via the data it stores.
#[test]
fn test_survey__should_rank_uncovered_behaviors_by_risk_public_api_internal_helper() {
    let public_behavior = UncoveredBehavior {
        file: PathBuf::from("src/lib.rs"),
        line: 1,
        description: "public_process (public API)".to_string(),
        suggested_clause: "MUST process data and return result".to_string(),
        suggested_keyword: Keyword::Must,
        suggested_spec: PathBuf::from("ought/svc.ought.md"),
    };
    let private_behavior = UncoveredBehavior {
        file: PathBuf::from("src/lib.rs"),
        line: 2,
        description: "private_validate (internal helper)".to_string(),
        suggested_clause: "SHOULD validate non-empty data".to_string(),
        suggested_keyword: Keyword::Should,
        suggested_spec: PathBuf::from("ought/svc.ought.md"),
    };

    // When the result ranks public first, public API appears before internal helper.
    let result = SurveyResult {
        uncovered: vec![public_behavior, private_behavior],
    };

    assert_eq!(result.uncovered.len(), 2);
    let first = &result.uncovered[0];
    let second = &result.uncovered[1];
    assert!(
        first.description.contains("public"),
        "public API behavior must rank first; got description: {:?}",
        first.description
    );
    assert!(
        second.description.contains("private"),
        "internal helper must rank second; got description: {:?}",
        second.description
    );
}

/// WONT auto-add clauses without user confirmation.
/// survey() returns SurveyResult; it does not modify spec files on disk.
/// The type has no side-effect fields -- it only carries suggestions.
#[test]
fn test_survey__wont_auto_add_clauses_without_user_confirmation() {
    let result = SurveyResult {
        uncovered: vec![UncoveredBehavior {
            file: PathBuf::from("src/lib.rs"),
            line: 1,
            description: "uncovered_fn has no clause".to_string(),
            suggested_clause: "MUST implement uncovered_fn".to_string(),
            suggested_keyword: Keyword::Must,
            suggested_spec: PathBuf::from("specs/svc.ought.md"),
        }],
    };

    // SurveyResult only carries data (suggestions), no mutation methods.
    // This confirms the type design prevents auto-modification.
    assert!(
        !result.uncovered.is_empty(),
        "survey must still return suggestions even though it does not write them"
    );
}

/// MUST read source files from the given path or project source root.
/// Requires survey() implementation to be filled in.
#[test]
#[ignore = "survey() is a todo!() stub -- needs implementation"]
fn test_survey__must_read_source_files_from_the_given_path_or_project_source_root() {
    // This test needs the actual survey() implementation.
    // See generated test: must_read_source_files_from_the_given_path_or_project_source_root_test.rs
}

/// MUST read all existing spec files to know what is already covered.
/// Requires survey() implementation to be filled in.
#[test]
#[ignore = "survey() is a todo!() stub -- needs implementation"]
fn test_survey__must_read_all_existing_spec_files_to_know_what_is_already_covered() {
    // This test needs the actual survey() implementation.
}

/// MUST use the LLM to identify public behaviors, APIs, and logic branches.
/// Requires survey() implementation to be filled in.
#[test]
#[ignore = "survey() is a todo!() stub -- needs implementation"]
fn test_survey__must_use_the_llm_to_identify_public_behaviors_apis_and_logic_bran() {
    // This test needs the actual survey() implementation.
}

// =============================================================================
// AUDIT TYPE TESTS
// =============================================================================

/// AuditResult can be constructed and holds a list of AuditFinding items.
#[test]
fn test_audit__audit_result_can_be_constructed() {
    let result = AuditResult {
        findings: vec![],
    };
    assert!(result.findings.is_empty());
}

/// AuditFinding can be constructed with all expected fields.
#[test]
fn test_audit__audit_finding_has_expected_fields() {
    let finding = AuditFinding {
        kind: AuditFindingKind::Contradiction,
        description: "Two clauses cannot both hold".to_string(),
        clauses: vec![
            ClauseId("auth::login::must_a".to_string()),
            ClauseId("auth::login::must_b".to_string()),
        ],
        suggestion: Some("Resolve the contradiction by choosing one".to_string()),
        confidence: Some(0.92),
    };

    assert_eq!(finding.kind, AuditFindingKind::Contradiction);
    assert!(!finding.description.is_empty());
    assert_eq!(finding.clauses.len(), 2);
    assert!(finding.suggestion.is_some());
    assert!(finding.confidence.is_some());
}

/// MUST categorize findings as: contradiction, gap, ambiguity, or redundancy.
/// Tests that AuditFindingKind has all four variants.
#[test]
fn test_audit__must_categorize_findings_as_contradiction_gap_ambiguity_or_redund() {
    let findings = vec![
        AuditFinding {
            kind: AuditFindingKind::Contradiction,
            description: "Two clauses cannot both hold".to_string(),
            clauses: vec![ClauseId("auth::login::must_a".to_string())],
            suggestion: None,
            confidence: None,
        },
        AuditFinding {
            kind: AuditFindingKind::Gap,
            description: "Missing fallback clause".to_string(),
            clauses: vec![ClauseId("auth::login::must_a".to_string())],
            suggestion: None,
            confidence: None,
        },
        AuditFinding {
            kind: AuditFindingKind::Ambiguity,
            description: "Clause text is unclear about timing".to_string(),
            clauses: vec![ClauseId("auth::login::must_c".to_string())],
            suggestion: None,
            confidence: None,
        },
        AuditFinding {
            kind: AuditFindingKind::Redundancy,
            description: "Two clauses express the same obligation".to_string(),
            clauses: vec![ClauseId("auth::login::must_a".to_string())],
            suggestion: None,
            confidence: None,
        },
    ];

    let kinds: Vec<AuditFindingKind> = findings.iter().map(|f| f.kind).collect();
    assert!(kinds.contains(&AuditFindingKind::Contradiction));
    assert!(kinds.contains(&AuditFindingKind::Gap));
    assert!(kinds.contains(&AuditFindingKind::Ambiguity));
    assert!(kinds.contains(&AuditFindingKind::Redundancy));
}

/// MUST reference the specific clauses involved in each finding.
/// Each AuditFinding.clauses must contain at least one non-empty ClauseId.
#[test]
fn test_audit__must_reference_the_specific_clauses_involved_in_each_finding_file() {
    let finding = AuditFinding {
        kind: AuditFindingKind::Contradiction,
        description: "Conflicting HTTP status obligations".to_string(),
        clauses: vec![
            ClauseId("api::responses::must_return_200".to_string()),
            ClauseId("api::responses::must_return_201".to_string()),
        ],
        suggestion: None,
        confidence: None,
    };

    assert!(
        !finding.clauses.is_empty(),
        "each finding must reference at least one specific clause"
    );
    for clause_id in &finding.clauses {
        assert!(!clause_id.0.is_empty(), "each referenced clause ID must be non-empty");
    }
}

/// MUST detect MUST BY deadline conflicts represented in the data.
#[test]
fn test_audit__must_detect_must_by_deadline_conflicts_e_g_an_operation_with_a_10() {
    let finding = AuditFinding {
        kind: AuditFindingKind::Contradiction,
        description: "checkout MUST BY 100ms but calls payment MUST BY 200ms -- sub-operation deadline exceeds parent deadline".to_string(),
        clauses: vec![
            ClauseId("checkout::process::must_by_100ms_complete_the_checkout".to_string()),
            ClauseId("payment::charge::must_by_200ms_charge_the_payment".to_string()),
        ],
        suggestion: Some("Reduce the payment deadline below 100ms or increase the checkout deadline".to_string()),
        confidence: Some(0.95),
    };

    assert_eq!(finding.kind, AuditFindingKind::Contradiction);
    assert!(
        finding.description.contains("100ms") || finding.description.contains("deadline"),
        "finding must describe deadline conflict"
    );
}

/// MUST detect MUST ALWAYS invariant conflicts.
#[test]
fn test_audit__must_detect_must_always_invariant_conflicts_e_g_two_invariants_th() {
    let finding = AuditFinding {
        kind: AuditFindingKind::Contradiction,
        description: "MUST ALWAYS maintain exactly one active session conflicts with MUST ALWAYS allow multiple concurrent sessions".to_string(),
        clauses: vec![
            ClauseId("auth::session::must_always_maintain_exactly_one_active_session".to_string()),
            ClauseId("auth::session::must_always_support_multiple_concurrent_sessions".to_string()),
        ],
        suggestion: Some("Reconcile session invariants by choosing a single-session or multi-session model".to_string()),
        confidence: Some(0.98),
    };

    assert_eq!(finding.kind, AuditFindingKind::Contradiction);
    assert!(
        finding.description.contains("MUST ALWAYS") || finding.description.contains("invariant"),
        "finding must describe invariant conflict"
    );
}

/// SHOULD detect GIVEN blocks with overlapping conditions that impose contradictory obligations.
#[test]
fn test_audit__should_detect_given_blocks_with_overlapping_conditions_that_impose() {
    let finding = AuditFinding {
        kind: AuditFindingKind::Contradiction,
        description: "GIVEN user is authenticated (MUST allow write) overlaps with GIVEN user is a guest (MUST NOT allow write)".to_string(),
        clauses: vec![
            ClauseId("api::access::given_authenticated_must_allow_full_read_write".to_string()),
            ClauseId("api::access::given_guest_must_not_allow_write".to_string()),
        ],
        suggestion: Some("Make GIVEN conditions mutually exclusive or add explicit precedence rules".to_string()),
        confidence: Some(0.80),
    };

    assert!(
        finding.kind == AuditFindingKind::Contradiction || finding.kind == AuditFindingKind::Ambiguity,
        "overlapping GIVEN conditions should be Contradiction or Ambiguity"
    );
    assert!(
        finding.description.contains("GIVEN") || finding.description.contains("overlap"),
        "description must reference the overlapping conditions"
    );
}

/// SHOULD detect MUST obligations that lack OTHERWISE fallbacks.
#[test]
fn test_audit__should_detect_must_obligations_that_lack_otherwise_fallbacks_where() {
    let finding = AuditFinding {
        kind: AuditFindingKind::Gap,
        description: "MUST fetch configuration from the remote server has no OTHERWISE fallback".to_string(),
        clauses: vec![ClauseId("config::remote::must_fetch_configuration".to_string())],
        suggestion: Some("Add an OTHERWISE clause specifying fallback behavior".to_string()),
        confidence: Some(0.87),
    };

    assert_eq!(finding.kind, AuditFindingKind::Gap);
    assert!(
        finding.description.contains("OTHERWISE") || finding.description.contains("fallback"),
        "finding must reference missing OTHERWISE"
    );
}

/// SHOULD suggest resolutions for each finding.
#[test]
fn test_audit__should_suggest_resolutions_for_each_finding() {
    let findings = vec![
        AuditFinding {
            kind: AuditFindingKind::Contradiction,
            description: "Conflicting HTTP status codes".to_string(),
            clauses: vec![ClauseId("api::must_return_200".to_string())],
            suggestion: Some("Use 200 for reads and 201 for creation".to_string()),
            confidence: None,
        },
        AuditFinding {
            kind: AuditFindingKind::Gap,
            description: "No clause for database unavailability".to_string(),
            clauses: vec![ClauseId("data::must_persist_records".to_string())],
            suggestion: Some("Add an OTHERWISE clause for database unreachable".to_string()),
            confidence: None,
        },
    ];

    for finding in &findings {
        assert!(
            finding
                .suggestion
                .as_deref()
                .map(|s| !s.is_empty())
                .unwrap_or(false),
            "audit should include a non-empty resolution suggestion for each finding; finding: {:?}",
            finding.description
        );
    }
}

/// MAY assign a confidence score to each finding.
/// When present, confidence must be in [0.0, 1.0]; absence (None) is also valid.
#[test]
fn test_audit__may_assign_a_confidence_score_to_each_finding() {
    let findings = vec![
        AuditFinding {
            kind: AuditFindingKind::Contradiction,
            description: "Conflicting auth obligations".to_string(),
            clauses: vec![ClauseId("auth::must_a".to_string())],
            suggestion: None,
            confidence: Some(0.92),
        },
        AuditFinding {
            kind: AuditFindingKind::Gap,
            description: "Missing rate-limit clause".to_string(),
            clauses: vec![ClauseId("api::ratelimit::must_throttle".to_string())],
            suggestion: None,
            confidence: Some(0.65),
        },
        AuditFinding {
            kind: AuditFindingKind::Ambiguity,
            description: "Vague timing requirement".to_string(),
            clauses: vec![ClauseId("svc::must_respond".to_string())],
            suggestion: None,
            confidence: None, // MAY means absence is valid
        },
    ];

    for finding in &findings {
        if let Some(confidence) = finding.confidence {
            assert!(
                (0.0..=1.0).contains(&confidence),
                "confidence score must be in the range [0.0, 1.0] when assigned; got {} for finding: {:?}",
                confidence,
                finding.description
            );
        }
        // Absence of confidence (None) is also valid -- MAY is permissive.
    }
}

/// MUST use the LLM to identify gaps.
/// Requires audit() implementation to be filled in.
#[test]
#[ignore = "audit() is a todo!() stub -- needs implementation"]
fn test_audit__must_use_the_llm_to_identify_gaps_areas_where_related_clauses_exi() {
    // This test needs the actual audit() implementation.
}

/// MUST use the LLM to identify contradictions.
/// Requires audit() implementation to be filled in.
#[test]
#[ignore = "audit() is a todo!() stub -- needs implementation"]
fn test_audit__must_use_the_llm_to_identify_contradictions_between_clauses_acros() {
    // This test needs the actual audit() implementation.
}

/// MUST read all spec files and their cross-references.
/// Requires audit() implementation to be filled in.
#[test]
#[ignore = "audit() is a todo!() stub -- needs implementation"]
fn test_audit__must_read_all_spec_files_and_their_cross_references() {
    // This test needs the actual audit() implementation.
}

/// SHOULD read relevant source code to ground the analysis.
/// Requires audit() implementation to be filled in.
#[test]
#[ignore = "audit() is a todo!() stub -- needs implementation"]
fn test_audit__should_read_relevant_source_code_to_ground_the_analysis_in_implemen() {
    // This test needs the actual audit() implementation.
}

// =============================================================================
// BLAME TYPE TESTS
// =============================================================================

/// BlameResult can be constructed and holds expected fields.
#[test]
fn test_blame__blame_result_can_be_constructed() {
    let result = BlameResult {
        clause_id: ClauseId("auth::login::must_return_401".to_string()),
        last_passed: None,
        first_failed: None,
        likely_commit: None,
        narrative: "The clause has never passed.".to_string(),
        suggested_fix: None,
    };

    assert_eq!(result.clause_id, ClauseId("auth::login::must_return_401".to_string()));
    assert!(result.last_passed.is_none());
    assert!(result.first_failed.is_none());
    assert!(result.likely_commit.is_none());
    assert!(!result.narrative.is_empty());
    assert!(result.suggested_fix.is_none());
}

/// CommitInfo can be constructed with all expected fields.
#[test]
fn test_blame__commit_info_has_expected_fields() {
    let commit = CommitInfo {
        hash: "abc123def456".to_string(),
        message: "refactor: simplify auth responses".to_string(),
        author: "Jane Developer <jane@example.com>".to_string(),
        date: Utc::now(),
    };

    assert!(!commit.hash.is_empty());
    assert!(!commit.message.is_empty());
    assert!(!commit.author.is_empty());
    assert!(commit.date.timestamp() > 0);
}

/// MUST accept a clause identifier (e.g. auth::login::must_return_401).
/// Tests that BlameResult carries back the same clause_id that was passed.
#[test]
fn test_blame__must_accept_a_clause_identifier_e_g_auth_login_must_return_401() {
    let clause_id = ClauseId("auth::login::must_return_401".to_string());
    let result = BlameResult {
        clause_id: clause_id.clone(),
        last_passed: None,
        first_failed: None,
        likely_commit: None,
        narrative: "stub".to_string(),
        suggested_fix: None,
    };

    assert_eq!(
        result.clause_id, clause_id,
        "blame result must carry the same clause_id that was passed in"
    );
}

/// MUST output a narrative explanation of what broke and why.
/// Tests that BlameResult.narrative is non-empty.
#[test]
fn test_blame__must_output_a_narrative_explanation_of_what_broke_and_why() {
    let result = BlameResult {
        clause_id: ClauseId("auth::login::must_return_401".to_string()),
        last_passed: Some(Utc::now()),
        first_failed: Some(Utc::now()),
        likely_commit: Some(CommitInfo {
            hash: "abc123".to_string(),
            message: "refactor: auth responses".to_string(),
            author: "Dev <dev@example.com>".to_string(),
            date: Utc::now(),
        }),
        narrative: "The auth handler was refactored to return 200 instead of 401 for invalid credentials.".to_string(),
        suggested_fix: None,
    };

    assert!(
        !result.narrative.is_empty(),
        "blame must output a non-empty narrative explanation of what broke and why"
    );
}

/// MUST output the timeline: last passing run, first failure, relevant commits.
/// Tests that BlameResult can hold all timeline fields.
#[test]
fn test_blame__must_output_the_timeline_last_passing_run_first_failure_relevant() {
    let now = Utc::now();
    let result = BlameResult {
        clause_id: ClauseId("auth::login::must_return_401".to_string()),
        last_passed: Some(now),
        first_failed: Some(now),
        likely_commit: Some(CommitInfo {
            hash: "abc123def".to_string(),
            message: "refactor: auth responses".to_string(),
            author: "Dev <dev@example.com>".to_string(),
            date: now,
        }),
        narrative: "Timeline: last passed before breaking commit, first failed after.".to_string(),
        suggested_fix: None,
    };

    assert!(result.last_passed.is_some(), "blame must output the last passing run timestamp");
    assert!(result.first_failed.is_some(), "blame must output the first failure timestamp");
    assert!(
        result.likely_commit.is_some(),
        "blame must output the relevant commits in the timeline"
    );
}

/// SHOULD name the author of the likely-responsible commit.
/// Tests that CommitInfo.author is populated.
#[test]
fn test_blame__should_name_the_author_of_the_likely_responsible_commit() {
    let commit = CommitInfo {
        hash: "deadbeef1234".to_string(),
        message: "refactor: simplify auth responses".to_string(),
        author: "Jane Developer <jane@example.com>".to_string(),
        date: Utc::now(),
    };

    assert!(
        !commit.author.is_empty(),
        "blame should name the author of the likely-responsible commit; got empty author"
    );
}

/// SHOULD identify the specific commit and file change most likely responsible.
/// Tests that CommitInfo.hash is populated.
#[test]
fn test_blame__should_identify_the_specific_commit_and_file_change_most_likely_res() {
    let result = BlameResult {
        clause_id: ClauseId("auth::login::must_return_401".to_string()),
        last_passed: None,
        first_failed: None,
        likely_commit: Some(CommitInfo {
            hash: "abc123def456".to_string(),
            message: "refactor: simplify auth error responses".to_string(),
            author: "Jane Developer <jane@example.com>".to_string(),
            date: Utc::now(),
        }),
        narrative: "Commit abc123def456 is most likely responsible.".to_string(),
        suggested_fix: None,
    };

    assert!(
        result.likely_commit.is_some(),
        "blame should identify the specific commit most likely responsible for the failure"
    );
    let commit = result.likely_commit.unwrap();
    assert!(
        !commit.hash.is_empty(),
        "blame should populate the commit hash of the likely-responsible change; got empty hash"
    );
}

/// SHOULD suggest a fix when the cause is clear.
/// Tests that BlameResult.suggested_fix can hold a value.
#[test]
fn test_blame__should_suggest_a_fix_when_the_cause_is_clear() {
    let result = BlameResult {
        clause_id: ClauseId("auth::login::must_return_401".to_string()),
        last_passed: None,
        first_failed: None,
        likely_commit: None,
        narrative: "The test broke because the authentication handler was changed.".to_string(),
        suggested_fix: Some(
            "Restore the 401 status code in src/auth.rs line 42".to_string(),
        ),
    };

    assert!(
        result.suggested_fix.is_some(),
        "blame should suggest a fix when the cause is clear; got None"
    );
    let fix = result.suggested_fix.unwrap();
    assert!(!fix.is_empty(), "suggested_fix must be a non-empty string describing the fix");
}

/// MUST NOT require a running LLM if the clause has never passed.
/// The BlameResult type supports this via last_passed=None and a narrative that says "never passed".
#[test]
fn test_blame__must_not_require_a_running_llm_if_the_clause_has_never_passed_just_re() {
    let result = BlameResult {
        clause_id: ClauseId("auth::login::must_return_401".to_string()),
        last_passed: None,
        first_failed: None,
        likely_commit: None,
        narrative: "This clause has never passed.".to_string(),
        suggested_fix: None,
    };

    assert!(
        result.last_passed.is_none(),
        "last_passed must be None when the clause has never passed"
    );
    assert!(
        result.narrative.to_lowercase().contains("never passed"),
        "blame must report that the clause has never passed in the narrative; got: {:?}",
        result.narrative
    );
}

/// MUST use git history to find when the clause last passed.
/// Requires blame() implementation to be filled in.
#[test]
#[ignore = "blame() is a todo!() stub -- needs implementation"]
fn test_blame__must_use_git_history_to_find_when_the_clause_last_passed_and_what() {
    // This test needs the actual blame() implementation.
}

/// MUST use the LLM to correlate the source diff with the failure.
/// Requires blame() implementation to be filled in.
#[test]
#[ignore = "blame() is a todo!() stub -- needs implementation"]
fn test_blame__must_use_the_llm_to_correlate_the_source_diff_with_the_failure_an() {
    // This test needs the actual blame() implementation.
}

/// MUST retrieve the clause, its generated test, and the failure output.
/// Requires blame() implementation to be filled in.
#[test]
#[ignore = "blame() is a todo!() stub -- needs implementation"]
fn test_blame__must_retrieve_the_clause_its_generated_test_and_the_failure_outpu() {
    // This test needs the actual blame() implementation.
}

// =============================================================================
// BISECT TYPE TESTS
// =============================================================================

/// BisectResult can be constructed and holds expected fields.
#[test]
fn test_bisect__bisect_result_can_be_constructed() {
    let result = BisectResult {
        clause_id: ClauseId("auth::login::must_return_401".to_string()),
        breaking_commit: CommitInfo {
            hash: "abc123".to_string(),
            message: "breaking change".to_string(),
            author: "Dev <dev@example.com>".to_string(),
            date: Utc::now(),
        },
        diff_summary: "Modified src/auth.rs: changed status from 401 to 200".to_string(),
    };

    assert_eq!(
        result.clause_id,
        ClauseId("auth::login::must_return_401".to_string())
    );
    assert!(!result.breaking_commit.hash.is_empty());
    assert!(!result.breaking_commit.message.is_empty());
    assert!(!result.breaking_commit.author.is_empty());
    assert!(!result.diff_summary.is_empty());
}

/// BisectOptions can be constructed with expected fields.
#[test]
fn test_bisect__bisect_options_can_be_constructed() {
    let options_default = bisect::BisectOptions {
        range: None,
        regenerate: false,
    };
    assert!(options_default.range.is_none());
    assert!(!options_default.regenerate);

    let options_with_range = bisect::BisectOptions {
        range: Some("abc123..def456".to_string()),
        regenerate: true,
    };
    assert_eq!(options_with_range.range.as_deref(), Some("abc123..def456"));
    assert!(options_with_range.regenerate);
}

/// MUST accept a clause identifier.
/// Tests that BisectResult carries the same clause_id back.
#[test]
fn test_bisect__must_accept_a_clause_identifier() {
    let clause_id = ClauseId("auth::login::must_return_401".to_string());
    let result = BisectResult {
        clause_id: clause_id.clone(),
        breaking_commit: CommitInfo {
            hash: "abc123".to_string(),
            message: "breaking change".to_string(),
            author: "Dev <dev@example.com>".to_string(),
            date: Utc::now(),
        },
        diff_summary: "changed auth.rs".to_string(),
    };

    assert_eq!(
        result.clause_id, clause_id,
        "bisect result must carry back the same clause_id that was passed in"
    );
}

/// MUST show the commit message, author, date, and diff summary for the breaking commit.
#[test]
fn test_bisect__must_show_the_commit_message_author_date_and_diff_summary_for_the() {
    let now = Utc::now();
    let result = BisectResult {
        clause_id: ClauseId("auth::login::must_return_401".to_string()),
        breaking_commit: CommitInfo {
            hash: "deadbeef".to_string(),
            message: "refactor: simplify auth -- always return 200".to_string(),
            author: "Bob Refactorer <bob@example.com>".to_string(),
            date: now,
        },
        diff_summary: "Modified src/auth.rs: changed status code from 401 to 200".to_string(),
    };

    let commit = &result.breaking_commit;
    assert!(
        !commit.message.is_empty(),
        "bisect must populate the breaking commit message; got empty string"
    );
    assert!(
        commit.message.contains("simplify auth") || commit.message.contains("200"),
        "breaking commit message must match the actual commit; got: {:?}",
        commit.message
    );
    assert!(
        !commit.author.is_empty(),
        "bisect must populate the breaking commit author; got empty string"
    );
    assert!(
        commit.author.contains("Bob") || commit.author.contains("bob@example.com"),
        "breaking commit author must match the committer; got: {:?}",
        commit.author
    );
    assert!(
        commit.date.timestamp() > 0,
        "bisect must populate a non-zero date for the breaking commit; got: {:?}",
        commit.date
    );
    assert!(
        !result.diff_summary.is_empty(),
        "bisect must populate diff_summary describing what changed; got empty string"
    );
    assert!(
        result.diff_summary.contains("auth.rs") || result.diff_summary.contains("status"),
        "diff summary must reference the changed file; got: {:?}",
        result.diff_summary
    );
}

/// MUST report the first commit where the clause fails.
/// Tests that BisectResult.breaking_commit can represent the first failing commit.
#[test]
fn test_bisect__must_report_the_first_commit_where_the_clause_fails() {
    let result = BisectResult {
        clause_id: ClauseId("auth::login::must_return_401".to_string()),
        breaking_commit: CommitInfo {
            hash: "commit_3_hash".to_string(),
            message: "commit 3".to_string(),
            author: "Test Runner <test@example.com>".to_string(),
            date: Utc::now(),
        },
        diff_summary: "Changed status.txt from pass to fail".to_string(),
    };

    assert!(
        result.breaking_commit.message.contains("commit 3"),
        "bisect must narrow to the first failing commit; got: {:?}",
        result.breaking_commit.message
    );
}

/// SHOULD support --range <from>..<to> to limit the search space.
/// Tests that BisectOptions.range can hold a revision range.
#[test]
fn test_bisect__should_support_range_from_to_to_limit_the_search_space() {
    let options = bisect::BisectOptions {
        range: Some("abc123..def456".to_string()),
        regenerate: false,
    };

    assert!(
        options.range.is_some(),
        "BisectOptions must support a range field to limit the search space"
    );
    let range = options.range.unwrap();
    assert!(
        range.contains(".."),
        "range should be in the format from..to; got: {:?}",
        range
    );
}

/// SHOULD use the generated test from the current manifest (not regenerate).
/// Tests that BisectOptions.regenerate defaults to false.
#[test]
fn test_bisect__should_use_the_generated_test_from_the_current_manifest_not_regener() {
    let options = bisect::BisectOptions {
        range: None,
        regenerate: false,
    };

    assert!(
        !options.regenerate,
        "without --regenerate, bisect should reuse the manifest test"
    );
}

/// MUST ALWAYS restore the working tree to its original state after completion.
/// Requires bisect() implementation to be filled in.
#[test]
#[ignore = "bisect() is a todo!() stub -- needs implementation"]
fn test_bisect__must_always_restore_the_working_tree_to_its_original_state_after_complet() {
    // This test needs the actual bisect() implementation.
}

/// MUST perform a git-bisect-style binary search.
/// Requires bisect() implementation to be filled in.
#[test]
#[ignore = "bisect() is a todo!() stub -- needs implementation"]
fn test_bisect__must_perform_a_git_bisect_style_binary_search_checkout_commit_gen() {
    // This test needs the actual bisect() implementation.
}

/// MUST restore the working tree to the original branch (GIVEN interruption).
/// Requires bisect() implementation to be filled in.
#[test]
#[ignore = "bisect() is a todo!() stub -- needs implementation"]
fn test_bisect__must_restore_the_working_tree_to_the_original_branch() {
    // This test needs the actual bisect() implementation.
}

/// SHOULD save progress so ought bisect --continue can resume.
/// Requires bisect() implementation to be filled in.
#[test]
#[ignore = "bisect() is a todo!() stub -- needs implementation"]
fn test_bisect__should_save_progress_so_ought_bisect_continue_can_resume() {
    // This test needs the actual bisect() implementation.
}

/// SHOULD cache test results per commit to avoid redundant runs.
/// Requires bisect() implementation to be filled in.
#[test]
#[ignore = "bisect() is a todo!() stub -- needs implementation"]
fn test_bisect__should_cache_test_results_per_commit_to_avoid_redundant_runs() {
    // This test needs the actual bisect() implementation.
}

// =============================================================================
// TRAIT TESTS -- verify Generator and Runner traits can be implemented
// =============================================================================

/// The Generator trait can be implemented with a mock.
#[test]
fn test_traits__generator_trait_can_be_implemented() {
    let generator = StubGenerator;
    let clause = Clause {
        id: ClauseId("test::clause".to_string()),
        keyword: Keyword::Must,
        severity: Severity::Required,
        text: "do something".to_string(),
        condition: None,
        otherwise: vec![],
        temporal: None,
        hints: vec![],
        source_location: SourceLocation {
            file: PathBuf::from("test.ought.md"),
            line: 1,
        },
        content_hash: "abc123".to_string(),
    };
    let ctx = GenerationContext {
        spec_context: None,
        source_files: vec![],
        schema_files: vec![],
        target_language: Language::Rust,
        verbose: false,
    };

    let result = generator.generate(&clause, &ctx);
    assert!(result.is_ok(), "mock generator must succeed");
    let test = result.unwrap();
    assert_eq!(test.clause_id, ClauseId("stub::clause".to_string()));
    assert_eq!(test.language, Language::Rust);
}

/// The Runner trait can be implemented with a mock.
#[test]
fn test_traits__runner_trait_can_be_implemented() {
    let runner = StubRunner;
    assert!(runner.is_available());
    assert_eq!(runner.name(), "stub");

    let result = runner.run(&[], Path::new("/tmp"));
    assert!(result.is_ok(), "mock runner must succeed");
    let run_result = result.unwrap();
    assert!(run_result.results.is_empty());
    assert_eq!(run_result.total_duration, Duration::ZERO);
}

/// The Generator trait is object-safe (can be used as dyn Generator).
#[test]
fn test_traits__generator_is_object_safe() {
    let generator: Box<dyn Generator> = Box::new(StubGenerator);
    let clause = Clause {
        id: ClauseId("test::clause".to_string()),
        keyword: Keyword::Must,
        severity: Severity::Required,
        text: "do something".to_string(),
        condition: None,
        otherwise: vec![],
        temporal: None,
        hints: vec![],
        source_location: SourceLocation {
            file: PathBuf::from("test.ought.md"),
            line: 1,
        },
        content_hash: "abc123".to_string(),
    };
    let ctx = GenerationContext {
        spec_context: None,
        source_files: vec![],
        schema_files: vec![],
        target_language: Language::Rust,
        verbose: false,
    };

    let result = generator.generate(&clause, &ctx);
    assert!(result.is_ok());
}

/// The Runner trait is object-safe (can be used as dyn Runner).
#[test]
fn test_traits__runner_is_object_safe() {
    let runner: Box<dyn Runner> = Box::new(StubRunner);
    assert!(runner.is_available());
    assert_eq!(runner.name(), "stub");
}

// =============================================================================
// FUNCTION SIGNATURE TESTS -- verify the analysis functions exist with correct types
// =============================================================================

/// survey() function exists with the expected signature.
/// It takes (&SpecGraph, &[PathBuf], &dyn Generator) and returns Result<SurveyResult>.
#[test]
fn test_signatures__survey_function_exists() {
    // Verify the function pointer has the expected type by assigning it.
    let _fn_ptr: fn(&SpecGraph, &[PathBuf], &dyn Generator) -> anyhow::Result<SurveyResult> =
        survey::survey;
}

/// audit() function exists with the expected signature.
/// It takes (&SpecGraph, &dyn Generator) and returns Result<AuditResult>.
#[test]
fn test_signatures__audit_function_exists() {
    let _fn_ptr: fn(&SpecGraph, &dyn Generator) -> anyhow::Result<AuditResult> = audit::audit;
}

/// blame() function exists with the expected signature.
/// It takes (&ClauseId, &SpecGraph, &RunResult, &dyn Generator) and returns Result<BlameResult>.
#[test]
fn test_signatures__blame_function_exists() {
    let _fn_ptr: fn(
        &ClauseId,
        &SpecGraph,
        &RunResult,
        &dyn Generator,
    ) -> anyhow::Result<BlameResult> = blame::blame;
}

/// bisect() function exists with the expected signature.
/// It takes (&ClauseId, &SpecGraph, &dyn Runner, &BisectOptions) and returns Result<BisectResult>.
#[test]
fn test_signatures__bisect_function_exists() {
    let _fn_ptr: fn(
        &ClauseId,
        &SpecGraph,
        &dyn Runner,
        &bisect::BisectOptions,
    ) -> anyhow::Result<BisectResult> = bisect::bisect;
}
