use std::collections::HashMap;
use std::time::Duration;

use ought_gen::Generator;
use ought_spec::{Clause, Keyword, Section, SpecGraph, Temporal};

use crate::types::{AuditFinding, AuditFindingKind, AuditResult};

/// Analyze all specs for contradictions, gaps, and coherence issues.
///
/// Detects: contradictory clauses, MUST BY deadline conflicts,
/// MUST ALWAYS invariant conflicts, overlapping GIVEN conditions
/// with contradictory obligations, and missing OTHERWISE chains.
///
/// The generator parameter is accepted for future LLM-powered deep analysis
/// but structural checks work without it.
pub fn audit(specs: &SpecGraph, _generator: &dyn Generator) -> anyhow::Result<AuditResult> {
    let mut findings: Vec<AuditFinding> = Vec::new();

    // Collect all clauses from all specs with their section paths.
    let mut all_clauses: Vec<ClauseWithContext> = Vec::new();
    for spec in specs.specs() {
        collect_clauses_with_context(&spec.sections, &spec.name, &mut all_clauses);
    }

    // Check 1: MUST BY deadline conflicts.
    check_deadline_conflicts(&all_clauses, &mut findings);

    // Check 2: MUST ALWAYS invariant conflicts.
    check_invariant_conflicts(&all_clauses, &mut findings);

    // Check 3: Missing OTHERWISE on network-dependent MUST clauses.
    check_missing_otherwise(&all_clauses, &mut findings);

    // Check 4: Duplicate/near-duplicate clauses (redundancy).
    check_redundancy(&all_clauses, &mut findings);

    // Check 5: Overlapping GIVEN conditions with contradictory obligations.
    check_given_overlaps(&all_clauses, &mut findings);

    Ok(AuditResult { findings })
}

/// A clause with its section path context for analysis.
struct ClauseWithContext {
    clause: Clause,
    #[allow(dead_code)]
    section_path: String,
}

fn collect_clauses_with_context(
    sections: &[Section],
    parent_path: &str,
    out: &mut Vec<ClauseWithContext>,
) {
    for section in sections {
        let section_path = format!("{} > {}", parent_path, section.title);
        for clause in &section.clauses {
            out.push(ClauseWithContext {
                clause: clause.clone(),
                section_path: section_path.clone(),
            });
            // Also collect OTHERWISE children.
            for ow in &clause.otherwise {
                out.push(ClauseWithContext {
                    clause: ow.clone(),
                    section_path: section_path.clone(),
                });
            }
        }
        collect_clauses_with_context(&section.subsections, &section_path, out);
    }
}

/// Check for MUST BY deadline conflicts: if a parent operation has a deadline
/// shorter than a sub-operation it calls.
fn check_deadline_conflicts(clauses: &[ClauseWithContext], findings: &mut Vec<AuditFinding>) {
    let deadline_clauses: Vec<(&ClauseWithContext, Duration)> = clauses
        .iter()
        .filter_map(|c| {
            if let Some(Temporal::Deadline(dur)) = &c.clause.temporal {
                Some((c, *dur))
            } else {
                None
            }
        })
        .collect();

    // Compare each pair: if they share a section path prefix (implying nesting)
    // and the parent deadline is shorter, flag it.
    for i in 0..deadline_clauses.len() {
        for j in (i + 1)..deadline_clauses.len() {
            let (a, dur_a) = &deadline_clauses[i];
            let (b, dur_b) = &deadline_clauses[j];

            // Check if one could be a parent of the other by section path or text reference.
            let a_text_lower = a.clause.text.to_lowercase();
            let b_text_lower = b.clause.text.to_lowercase();

            // Heuristic: if clause A's text mentions something clause B specifies (or vice versa).
            let a_mentions_b = clause_text_overlaps(&a_text_lower, &b_text_lower);
            let b_mentions_a = clause_text_overlaps(&b_text_lower, &a_text_lower);

            if a_mentions_b && dur_a < dur_b {
                findings.push(AuditFinding {
                    kind: AuditFindingKind::Contradiction,
                    description: format!(
                        "{} MUST BY {:?} but references {} MUST BY {:?} -- sub-operation deadline exceeds parent deadline",
                        a.clause.text, dur_a, b.clause.text, dur_b
                    ),
                    clauses: vec![a.clause.id.clone(), b.clause.id.clone()],
                    suggestion: Some(format!(
                        "Reduce the sub-operation deadline below {:?} or increase the parent deadline",
                        dur_a
                    )),
                    confidence: Some(0.85),
                });
            } else if b_mentions_a && dur_b < dur_a {
                findings.push(AuditFinding {
                    kind: AuditFindingKind::Contradiction,
                    description: format!(
                        "{} MUST BY {:?} but references {} MUST BY {:?} -- sub-operation deadline exceeds parent deadline",
                        b.clause.text, dur_b, a.clause.text, dur_a
                    ),
                    clauses: vec![b.clause.id.clone(), a.clause.id.clone()],
                    suggestion: Some(format!(
                        "Reduce the sub-operation deadline below {:?} or increase the parent deadline",
                        dur_b
                    )),
                    confidence: Some(0.85),
                });
            }
        }
    }
}

/// Simple word overlap check between two clause texts.
fn clause_text_overlaps(a: &str, b: &str) -> bool {
    let a_words: Vec<&str> = a.split_whitespace().filter(|w| w.len() > 3).collect();
    let b_words: Vec<&str> = b.split_whitespace().filter(|w| w.len() > 3).collect();
    let overlap = a_words.iter().filter(|w| b_words.contains(w)).count();
    overlap >= 2
}

/// Check for MUST ALWAYS invariant conflicts: two invariants that cannot both hold.
fn check_invariant_conflicts(clauses: &[ClauseWithContext], findings: &mut Vec<AuditFinding>) {
    let invariant_clauses: Vec<&ClauseWithContext> = clauses
        .iter()
        .filter(|c| matches!(c.clause.temporal, Some(Temporal::Invariant)))
        .collect();

    for i in 0..invariant_clauses.len() {
        for j in (i + 1)..invariant_clauses.len() {
            let a = invariant_clauses[i];
            let b = invariant_clauses[j];

            // Check if they have contradictory keywords or opposing text.
            let contradicts = is_contradictory(&a.clause, &b.clause);
            if contradicts {
                findings.push(AuditFinding {
                    kind: AuditFindingKind::Contradiction,
                    description: format!(
                        "MUST ALWAYS {} conflicts with MUST ALWAYS {}",
                        a.clause.text, b.clause.text
                    ),
                    clauses: vec![a.clause.id.clone(), b.clause.id.clone()],
                    suggestion: Some(
                        "Reconcile invariants by choosing one consistent model".to_string(),
                    ),
                    confidence: Some(0.80),
                });
            }
        }
    }
}

/// Heuristic: check if two clauses are contradictory.
fn is_contradictory(a: &Clause, b: &Clause) -> bool {
    // Different polarity keywords on similar topics.
    let a_positive = matches!(
        a.keyword,
        Keyword::Must | Keyword::Should | Keyword::MustAlways | Keyword::MustBy
    );
    let b_positive = matches!(
        b.keyword,
        Keyword::Must | Keyword::Should | Keyword::MustAlways | Keyword::MustBy
    );
    let a_negative = matches!(a.keyword, Keyword::MustNot | Keyword::ShouldNot | Keyword::Wont);
    let b_negative = matches!(b.keyword, Keyword::MustNot | Keyword::ShouldNot | Keyword::Wont);

    // If one is positive and the other negative on similar text.
    if (a_positive && b_negative) || (a_negative && b_positive) {
        let overlap = clause_text_overlaps(
            &a.text.to_lowercase(),
            &b.text.to_lowercase(),
        );
        if overlap {
            return true;
        }
    }

    // Check for opposing terms in text.
    let a_lower = a.text.to_lowercase();
    let b_lower = b.text.to_lowercase();

    let opposites = [
        ("single", "multiple"),
        ("one", "many"),
        ("exactly one", "concurrent"),
        ("block", "allow"),
        ("deny", "permit"),
        ("reject", "accept"),
        ("disable", "enable"),
        ("synchronous", "asynchronous"),
    ];

    for (pos, neg) in &opposites {
        if (a_lower.contains(pos) && b_lower.contains(neg))
            || (a_lower.contains(neg) && b_lower.contains(pos))
        {
            // Check they're about the same topic.
            if clause_text_overlaps(&a_lower, &b_lower) {
                return true;
            }
        }
    }

    false
}

/// Check for MUST clauses that mention network/remote operations without OTHERWISE.
fn check_missing_otherwise(clauses: &[ClauseWithContext], findings: &mut Vec<AuditFinding>) {
    let network_hints = [
        "request", "api", "fetch", "remote", "server", "http", "endpoint", "network", "download",
        "upload", "connect", "socket", "tcp", "udp", "grpc", "webhook",
    ];

    for c in clauses {
        if c.clause.keyword != Keyword::Must && c.clause.keyword != Keyword::MustBy {
            continue;
        }
        if !c.clause.otherwise.is_empty() {
            continue;
        }
        let text_lower = c.clause.text.to_lowercase();
        let is_network = network_hints.iter().any(|h| text_lower.contains(h));
        if is_network {
            findings.push(AuditFinding {
                kind: AuditFindingKind::Gap,
                description: format!(
                    "MUST {} has no OTHERWISE fallback but appears to depend on network/remote operations",
                    c.clause.text
                ),
                clauses: vec![c.clause.id.clone()],
                suggestion: Some(
                    "Add an OTHERWISE clause specifying fallback behavior when the network operation fails"
                        .to_string(),
                ),
                confidence: Some(0.75),
            });
        }
    }
}

/// Check for redundant/near-duplicate clauses.
fn check_redundancy(clauses: &[ClauseWithContext], findings: &mut Vec<AuditFinding>) {
    for i in 0..clauses.len() {
        for j in (i + 1)..clauses.len() {
            let a = &clauses[i];
            let b = &clauses[j];

            // Skip Otherwise children compared to their parent.
            if a.clause.keyword == Keyword::Otherwise || b.clause.keyword == Keyword::Otherwise {
                continue;
            }

            let similarity = text_similarity(&a.clause.text, &b.clause.text);
            if similarity > 0.8 && a.clause.keyword == b.clause.keyword {
                findings.push(AuditFinding {
                    kind: AuditFindingKind::Redundancy,
                    description: format!(
                        "Clauses appear to express the same obligation: \"{}\" and \"{}\"",
                        a.clause.text, b.clause.text
                    ),
                    clauses: vec![a.clause.id.clone(), b.clause.id.clone()],
                    suggestion: Some("Consider merging these clauses into one".to_string()),
                    confidence: Some(similarity),
                });
            }
        }
    }
}

/// Simple Jaccard similarity on word sets.
fn text_similarity(a: &str, b: &str) -> f64 {
    let a_owned: std::collections::HashSet<String> = a
        .to_lowercase()
        .split_whitespace()
        .filter(|w| w.len() > 2)
        .map(|w| w.to_string())
        .collect();
    let b_owned: std::collections::HashSet<String> = b
        .to_lowercase()
        .split_whitespace()
        .filter(|w| w.len() > 2)
        .map(|w| w.to_string())
        .collect();

    if a_owned.is_empty() && b_owned.is_empty() {
        return 1.0;
    }

    let intersection = a_owned.intersection(&b_owned).count();
    let union = a_owned.union(&b_owned).count();

    if union == 0 {
        return 0.0;
    }

    intersection as f64 / union as f64
}

/// Check for overlapping GIVEN conditions with contradictory obligations.
fn check_given_overlaps(clauses: &[ClauseWithContext], findings: &mut Vec<AuditFinding>) {
    // Group clauses by their GIVEN condition.
    let mut given_groups: HashMap<String, Vec<&ClauseWithContext>> = HashMap::new();

    for c in clauses {
        if let Some(ref condition) = c.clause.condition {
            given_groups
                .entry(condition.to_lowercase())
                .or_default()
                .push(c);
        }
    }

    // For clauses under different GIVEN conditions, check if the conditions overlap
    // and the obligations contradict.
    let conditions: Vec<String> = given_groups.keys().cloned().collect();
    for i in 0..conditions.len() {
        for j in (i + 1)..conditions.len() {
            let cond_a = &conditions[i];
            let cond_b = &conditions[j];

            // Check if conditions might overlap (heuristic: shared significant words).
            if clause_text_overlaps(cond_a, cond_b) {
                let clauses_a = &given_groups[cond_a];
                let clauses_b = &given_groups[cond_b];

                for ca in clauses_a {
                    for cb in clauses_b {
                        if is_contradictory(&ca.clause, &cb.clause) {
                            findings.push(AuditFinding {
                                kind: AuditFindingKind::Contradiction,
                                description: format!(
                                    "GIVEN {} ({}) overlaps with GIVEN {} ({}) -- contradictory obligations",
                                    cond_a, ca.clause.text, cond_b, cb.clause.text
                                ),
                                clauses: vec![ca.clause.id.clone(), cb.clause.id.clone()],
                                suggestion: Some(
                                    "Make GIVEN conditions mutually exclusive or add explicit precedence rules"
                                        .to_string(),
                                ),
                                confidence: Some(0.70),
                            });
                        }
                    }
                }
            }
        }
    }
}
