use chrono::{DateTime, Utc};
use ought_gen::Generator;
use ought_run::{RunResult, TestStatus};
use ought_spec::{ClauseId, Section, SpecGraph};

use crate::types::{BlameResult, CommitInfo};

/// Explain why a clause is failing by correlating with git history.
///
/// Finds when the clause last passed, what commits changed since,
/// and produces a causal narrative. Uses structural analysis from git
/// history; the LLM generator is accepted for future narrative enrichment.
pub fn blame(
    clause_id: &ClauseId,
    specs: &SpecGraph,
    results: &RunResult,
    _generator: &dyn Generator,
) -> anyhow::Result<BlameResult> {
    // 1. Find the clause in the test results.
    let test_result = results
        .results
        .iter()
        .find(|r| r.clause_id == *clause_id);

    // If the clause isn't found in results at all.
    let test_result = match test_result {
        Some(r) => r,
        None => {
            return Ok(BlameResult {
                clause_id: clause_id.clone(),
                last_passed: None,
                first_failed: None,
                likely_commit: None,
                narrative: format!(
                    "Clause {} was not found in the test results. It may not have a generated test yet.",
                    clause_id
                ),
                suggested_fix: Some("Run `ought generate` to create tests for this clause".to_string()),
            });
        }
    };

    // If the clause is currently passing.
    if test_result.status == TestStatus::Passed {
        return Ok(BlameResult {
            clause_id: clause_id.clone(),
            last_passed: Some(Utc::now()),
            first_failed: None,
            likely_commit: None,
            narrative: format!("Clause {} is currently passing.", clause_id),
            suggested_fix: None,
        });
    }

    // 2. The clause is failing. Gather git history to find the likely cause.
    let source_files = collect_source_files_for_clause(clause_id, specs);
    let recent_commits = get_recent_commits(20);
    let recent_diff = get_recent_diff(&source_files, 5);

    // 3. Build the narrative.
    let failure_msg = test_result
        .details
        .failure_message
        .as_deref()
        .or(test_result.message.as_deref())
        .unwrap_or("(no failure message)");

    let mut narrative = format!(
        "Clause {} is failing with status {:?}.\n\nFailure: {}\n",
        clause_id, test_result.status, failure_msg
    );

    let likely_commit = if let Some(ref commits) = recent_commits {
        if !commits.is_empty() {
            narrative.push_str("\nRecent commits:\n");
            for commit in commits.iter().take(10) {
                narrative.push_str(&format!(
                    "  {} {} ({})\n",
                    &commit.hash[..7.min(commit.hash.len())],
                    commit.message,
                    commit.author
                ));
            }

            // The most recent commit is the most likely culprit.
            Some(commits[0].clone())
        } else {
            narrative.push_str("\nNo recent commits found.\n");
            None
        }
    } else {
        narrative.push_str("\nUnable to retrieve git history (not a git repository?).\n");
        None
    };

    if let Some(ref diff) = recent_diff
        && !diff.is_empty() {
            narrative.push_str(&format!("\nRecent changes to related source files:\n{}\n", diff));
        }

    // 4. Build suggested fix.
    let suggested_fix = likely_commit.as_ref().map(|commit| format!(
            "Investigate commit {} ({}) for changes that may have broken this clause",
            &commit.hash[..7.min(commit.hash.len())],
            commit.message
        ));

    Ok(BlameResult {
        clause_id: clause_id.clone(),
        last_passed: None, // Would need historical run data to populate.
        first_failed: Some(Utc::now()),
        likely_commit,
        narrative,
        suggested_fix,
    })
}

/// Collect source file paths that might be relevant to a clause.
fn collect_source_files_for_clause(clause_id: &ClauseId, specs: &SpecGraph) -> Vec<String> {
    let mut source_files = Vec::new();
    for spec in specs.specs() {
        // Check if this spec contains the clause.
        if section_contains_clause(&spec.sections, clause_id) {
            for src in &spec.metadata.sources {
                source_files.push(src.clone());
            }
        }
    }
    source_files
}

fn section_contains_clause(sections: &[Section], clause_id: &ClauseId) -> bool {
    for section in sections {
        for clause in &section.clauses {
            if clause.id == *clause_id {
                return true;
            }
            for ow in &clause.otherwise {
                if ow.id == *clause_id {
                    return true;
                }
            }
        }
        if section_contains_clause(&section.subsections, clause_id) {
            return true;
        }
    }
    false
}

/// Get recent git commits.
fn get_recent_commits(count: usize) -> Option<Vec<CommitInfo>> {
    let output = std::process::Command::new("git")
        .args([
            "log",
            &format!("--max-count={}", count),
            "--format=%H|%s|%an <%ae>|%aI",
        ])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let commits: Vec<CommitInfo> = stdout
        .lines()
        .filter_map(|line| {
            let parts: Vec<&str> = line.splitn(4, '|').collect();
            if parts.len() < 4 {
                return None;
            }
            let date: DateTime<Utc> = parts[3].parse().ok()?;
            Some(CommitInfo {
                hash: parts[0].to_string(),
                message: parts[1].to_string(),
                author: parts[2].to_string(),
                date,
            })
        })
        .collect();

    Some(commits)
}

/// Get recent diff for the specified source files.
fn get_recent_diff(source_files: &[String], depth: usize) -> Option<String> {
    if source_files.is_empty() {
        // If no source files specified, diff all files.
        let output = std::process::Command::new("git")
            .args([
                "diff",
                &format!("HEAD~{}..HEAD", depth),
                "--stat",
            ])
            .output()
            .ok()?;

        if !output.status.success() {
            return None;
        }
        return Some(String::from_utf8_lossy(&output.stdout).to_string());
    }

    let mut args = vec![
        "diff".to_string(),
        format!("HEAD~{}..HEAD", depth),
        "--stat".to_string(),
        "--".to_string(),
    ];
    args.extend(source_files.iter().cloned());

    let output = std::process::Command::new("git")
        .args(&args)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    Some(String::from_utf8_lossy(&output.stdout).to_string())
}
