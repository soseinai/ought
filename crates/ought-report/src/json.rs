use ought_run::RunResult;
use ought_spec::{Clause, Keyword, Spec};
use serde::Serialize;
use std::collections::HashMap;

/// A single clause result entry for JSON output.
#[derive(Serialize)]
struct JsonClauseResult {
    clause_id: String,
    keyword: String,
    severity: String,
    status: String,
    message: Option<String>,
    duration_ms: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    iterations: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    measured_duration_ms: Option<f64>,
    /// True when the clause is declared with a `PENDING` prefix.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pending: bool,
}

/// Top-level JSON report.
#[derive(Serialize)]
struct JsonReport {
    specs: Vec<JsonSpecReport>,
    summary: JsonSummary,
    total_duration_ms: f64,
}

#[derive(Serialize)]
struct JsonSpecReport {
    name: String,
    source_path: String,
    results: Vec<JsonClauseResult>,
}

#[derive(Serialize)]
struct JsonSummary {
    passed: usize,
    failed: usize,
    errored: usize,
    skipped: usize,
    pending: usize,
    must_total: usize,
    must_passed: usize,
    must_coverage_pct: f64,
}

fn keyword_str(kw: Keyword) -> &'static str {
    match kw {
        Keyword::Must => "MUST",
        Keyword::MustNot => "MUST NOT",
        Keyword::Should => "SHOULD",
        Keyword::ShouldNot => "SHOULD NOT",
        Keyword::May => "MAY",
        Keyword::Wont => "WONT",
        Keyword::Given => "GIVEN",
        Keyword::Otherwise => "OTHERWISE",
        Keyword::MustAlways => "MUST ALWAYS",
        Keyword::MustBy => "MUST BY",
    }
}

fn severity_str(kw: Keyword) -> &'static str {
    match kw.severity() {
        ought_spec::Severity::Required => "required",
        ought_spec::Severity::Recommended => "recommended",
        ought_spec::Severity::Optional => "optional",
        ought_spec::Severity::NegativeConfirmation => "negative_confirmation",
    }
}

fn status_str(status: ought_run::TestStatus) -> &'static str {
    match status {
        ought_run::TestStatus::Passed => "passed",
        ought_run::TestStatus::Failed => "failed",
        ought_run::TestStatus::Errored => "errored",
        ought_run::TestStatus::Skipped => "skipped",
    }
}

/// Collect all clause IDs from clauses and their otherwise chains.
/// Each entry carries `(id, keyword, pending)`.
fn collect_clauses(clauses: &[Clause], out: &mut Vec<(String, Keyword, bool)>) {
    for clause in clauses {
        out.push((clause.id.0.clone(), clause.keyword, clause.pending));
        if !clause.otherwise.is_empty() {
            collect_clauses(&clause.otherwise, out);
        }
    }
}

/// Render results as structured JSON to stdout.
pub fn report(results: &RunResult, specs: &[Spec]) -> anyhow::Result<String> {
    // Build a lookup from clause_id to TestResult
    let result_map: HashMap<&str, &ought_run::TestResult> = results
        .results
        .iter()
        .map(|r| (r.clause_id.0.as_str(), r))
        .collect();

    let mut total_passed = 0usize;
    let mut total_failed = 0usize;
    let mut total_errored = 0usize;
    let mut total_skipped = 0usize;
    let mut total_pending = 0usize;
    let mut must_total = 0usize;
    let mut must_passed = 0usize;

    let mut spec_reports = Vec::new();

    for spec in specs {
        let mut clause_infos = Vec::new();
        for section in &spec.sections {
            collect_clauses_from_section(section, &mut clause_infos);
        }

        let mut json_results = Vec::new();

        for (clause_id, keyword, pending) in &clause_infos {
            // Pending clauses are reported with status "pending" regardless of
            // whether a stray test result exists. They do NOT count toward the
            // MUST coverage denominator — the author has explicitly deferred
            // them, so measuring coverage against them would be misleading.
            if *pending {
                total_pending += 1;
                json_results.push(JsonClauseResult {
                    clause_id: clause_id.clone(),
                    keyword: keyword_str(*keyword).to_string(),
                    severity: severity_str(*keyword).to_string(),
                    status: "pending".to_string(),
                    message: None,
                    duration_ms: 0.0,
                    iterations: None,
                    measured_duration_ms: None,
                    pending: true,
                });
                continue;
            }

            if let Some(tr) = result_map.get(clause_id.as_str()) {
                let is_must = matches!(
                    keyword,
                    Keyword::Must | Keyword::MustNot | Keyword::MustAlways | Keyword::MustBy
                );
                if is_must {
                    must_total += 1;
                }

                match tr.status {
                    ought_run::TestStatus::Passed => {
                        total_passed += 1;
                        if is_must {
                            must_passed += 1;
                        }
                    }
                    ought_run::TestStatus::Failed => total_failed += 1,
                    ought_run::TestStatus::Errored => total_errored += 1,
                    ought_run::TestStatus::Skipped => total_skipped += 1,
                }

                json_results.push(JsonClauseResult {
                    clause_id: clause_id.clone(),
                    keyword: keyword_str(*keyword).to_string(),
                    severity: severity_str(*keyword).to_string(),
                    status: status_str(tr.status).to_string(),
                    message: tr.message.clone().or_else(|| tr.details.failure_message.clone()),
                    duration_ms: tr.duration.as_secs_f64() * 1000.0,
                    iterations: tr.details.iterations,
                    measured_duration_ms: tr.details.measured_duration.map(|d| d.as_secs_f64() * 1000.0),
                    pending: false,
                });
            }
        }

        spec_reports.push(JsonSpecReport {
            name: spec.name.clone(),
            source_path: spec.source_path.display().to_string(),
            results: json_results,
        });
    }

    let must_coverage_pct = if must_total > 0 {
        (must_passed as f64 / must_total as f64) * 100.0
    } else {
        100.0
    };

    let report = JsonReport {
        specs: spec_reports,
        summary: JsonSummary {
            passed: total_passed,
            failed: total_failed,
            errored: total_errored,
            skipped: total_skipped,
            pending: total_pending,
            must_total,
            must_passed,
            must_coverage_pct,
        },
        total_duration_ms: results.total_duration.as_secs_f64() * 1000.0,
    };

    let json = serde_json::to_string_pretty(&report)?;
    Ok(json)
}

fn collect_clauses_from_section(
    section: &ought_spec::Section,
    out: &mut Vec<(String, Keyword, bool)>,
) {
    collect_clauses(&section.clauses, out);
    for sub in &section.subsections {
        collect_clauses_from_section(sub, out);
    }
}
