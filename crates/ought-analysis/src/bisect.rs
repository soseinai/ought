use std::path::Path;

use chrono::{DateTime, Utc};
use ought_run::Runner;
use ought_spec::{ClauseId, SpecGraph};

use crate::types::{BisectResult, CommitInfo};

/// Options for the bisect command.
pub struct BisectOptions {
    /// Limit the search to a git revision range (e.g. "abc123..def456").
    pub range: Option<String>,
    /// Regenerate tests at each commit instead of using the current manifest.
    pub regenerate: bool,
}

/// Binary search through git history to find the commit that broke a clause.
///
/// Always restores the working tree to its original state after completion.
pub fn bisect(
    clause_id: &ClauseId,
    _specs: &SpecGraph,
    runner: &dyn Runner,
    options: &BisectOptions,
) -> anyhow::Result<BisectResult> {
    // 1. Record the current branch/HEAD so we can restore it.
    let original_ref = get_current_ref()?;

    // 2. Get the list of commits in the range.
    let commits = get_commit_range(options)?;

    if commits.is_empty() {
        anyhow::bail!("No commits found in the specified range");
    }

    if commits.len() == 1 {
        // Only one commit -- it must be the breaking one.
        restore_working_tree(&original_ref);
        return Ok(BisectResult {
            clause_id: clause_id.clone(),
            breaking_commit: commits.into_iter().next().unwrap(),
            diff_summary: "Only one commit in range".to_string(),
        });
    }

    // 3. Binary search: find the first commit where the test fails.
    let result = run_bisect(clause_id, &commits, runner, &original_ref);

    // 4. Always restore working tree.
    restore_working_tree(&original_ref);

    match result {
        Ok(breaking_idx) => {
            let breaking_commit = commits[breaking_idx].clone();
            let diff_summary = get_commit_diff_summary(&breaking_commit.hash);

            Ok(BisectResult {
                clause_id: clause_id.clone(),
                breaking_commit,
                diff_summary,
            })
        }
        Err(e) => Err(e),
    }
}

/// Run the binary search across commits.
fn run_bisect(
    clause_id: &ClauseId,
    commits: &[CommitInfo],
    runner: &dyn Runner,
    original_ref: &str,
) -> anyhow::Result<usize> {
    // Commits are ordered newest first. We want to find the first (oldest) failing commit.
    // So we reverse to get oldest first for binary search.
    let n = commits.len();

    // Binary search: lo is the oldest, hi is the newest.
    // We assume commits[n-1] (newest) fails, and search for the first failure.
    let mut lo: usize = 0;
    let mut hi: usize = n - 1;
    let mut last_fail: usize = hi;

    while lo < hi {
        let mid = lo + (hi - lo) / 2;

        // Checkout the commit at index `mid` (in reversed order, so commits[n-1-mid] is older).
        // Actually, commits are newest first from git log. So commits[0] is newest.
        let commit_idx = mid;
        let commit = &commits[commit_idx];

        match checkout_and_test(commit, clause_id, runner) {
            Ok(passed) => {
                if passed {
                    // This commit passes, so the break is between mid and last_fail.
                    // Move towards newer commits.
                    // Since commits[0] is newest: lower index = newer.
                    // If mid passes and hi fails, break is between lo..mid (newer side).
                    hi = mid;
                } else {
                    // This commit fails, search older.
                    last_fail = mid;
                    lo = mid + 1;
                }
            }
            Err(_) => {
                // If test execution fails, treat as failing.
                last_fail = mid;
                lo = mid + 1;
            }
        }

        // Restore to original ref between checkouts.
        restore_working_tree(original_ref);
    }

    Ok(last_fail)
}

/// Checkout a commit, run the test for the clause, and return whether it passed.
fn checkout_and_test(
    commit: &CommitInfo,
    clause_id: &ClauseId,
    runner: &dyn Runner,
) -> anyhow::Result<bool> {
    // Stash any uncommitted changes.
    let _ = std::process::Command::new("git")
        .args(["stash", "--include-untracked"])
        .output();

    // Checkout the target commit.
    let checkout = std::process::Command::new("git")
        .args(["checkout", &commit.hash])
        .output()?;

    if !checkout.status.success() {
        anyhow::bail!(
            "Failed to checkout commit {}: {}",
            commit.hash,
            String::from_utf8_lossy(&checkout.stderr)
        );
    }

    // Find test files in the current working directory.
    let test_dir = find_test_dir();

    // Run tests using the runner.
    let tests = collect_test_files_for_clause(clause_id, &test_dir);

    if tests.is_empty() {
        // No test file found for this clause at this commit.
        return Ok(true); // Assume passing if test doesn't exist yet.
    }

    let result = runner.run(&tests, &test_dir)?;

    // Check if the specific clause passed.
    let clause_passed = result
        .results
        .iter()
        .any(|r| r.clause_id == *clause_id && r.status == ought_run::TestStatus::Passed);

    // If no results matched the clause, consider it passing (test may not exist at this commit).
    let relevant_results: Vec<_> = result
        .results
        .iter()
        .filter(|r| r.clause_id == *clause_id)
        .collect();

    if relevant_results.is_empty() {
        return Ok(true);
    }

    Ok(clause_passed)
}

/// Get the current git ref (branch name or HEAD hash).
fn get_current_ref() -> anyhow::Result<String> {
    // Try to get branch name.
    let output = std::process::Command::new("git")
        .args(["symbolic-ref", "--short", "HEAD"])
        .output();

    if let Ok(output) = output
        && output.status.success() {
            return Ok(String::from_utf8_lossy(&output.stdout).trim().to_string());
        }

    // Fall back to HEAD hash.
    let output = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()?;

    if !output.status.success() {
        anyhow::bail!("Not in a git repository");
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Get the list of commits in the given range.
fn get_commit_range(options: &BisectOptions) -> anyhow::Result<Vec<CommitInfo>> {
    let range = options
        .range
        .as_deref()
        .unwrap_or("HEAD~20..HEAD");

    let output = std::process::Command::new("git")
        .args([
            "log",
            range,
            "--format=%H|%s|%an <%ae>|%aI",
            "--reverse",
        ])
        .output()?;

    if !output.status.success() {
        // If the range is invalid (e.g., not enough history), try a smaller range.
        let output = std::process::Command::new("git")
            .args([
                "log",
                "--max-count=20",
                "--format=%H|%s|%an <%ae>|%aI",
                "--reverse",
            ])
            .output()?;

        if !output.status.success() {
            anyhow::bail!("Failed to get git log");
        }

        return parse_git_log(&String::from_utf8_lossy(&output.stdout));
    }

    parse_git_log(&String::from_utf8_lossy(&output.stdout))
}

fn parse_git_log(output: &str) -> anyhow::Result<Vec<CommitInfo>> {
    let commits: Vec<CommitInfo> = output
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

    Ok(commits)
}

/// Restore the working tree to the given ref.
fn restore_working_tree(ref_name: &str) {
    let _ = std::process::Command::new("git")
        .args(["checkout", ref_name])
        .output();

    // Pop stash if there was one.
    let _ = std::process::Command::new("git")
        .args(["stash", "pop"])
        .output();
}

/// Get a diff summary for a specific commit.
fn get_commit_diff_summary(hash: &str) -> String {
    let output = std::process::Command::new("git")
        .args(["diff-tree", "--no-commit-id", "-r", "--stat", hash])
        .output();

    match output {
        Ok(output) if output.status.success() => {
            String::from_utf8_lossy(&output.stdout).trim().to_string()
        }
        _ => "Unable to retrieve diff summary".to_string(),
    }
}

/// Find the test directory in the current working directory.
fn find_test_dir() -> std::path::PathBuf {
    // Try common locations.
    for candidate in &["ought/ought-gen", "tests", "test", "ought-gen"] {
        let path = std::path::PathBuf::from(candidate);
        if path.is_dir() {
            return path;
        }
    }
    std::path::PathBuf::from(".")
}

/// Collect generated test files for a specific clause.
fn collect_test_files_for_clause(
    clause_id: &ClauseId,
    test_dir: &Path,
) -> Vec<ought_gen::GeneratedTest> {
    // Derive the expected file path from the clause ID.
    let path_component = clause_id.0.replace("::", "/");

    // Look for test files matching common patterns.
    let extensions = ["_test.rs", "_test.py", ".test.ts", ".test.js", "_test.go"];

    for ext in &extensions {
        let file_path = test_dir.join(format!("{}{}", path_component, ext));
        if file_path.is_file()
            && let Ok(code) = std::fs::read_to_string(&file_path) {
                let language = match *ext {
                    "_test.rs" => ought_gen::generator::Language::Rust,
                    "_test.py" => ought_gen::generator::Language::Python,
                    ".test.ts" => ought_gen::generator::Language::TypeScript,
                    ".test.js" => ought_gen::generator::Language::JavaScript,
                    "_test.go" => ought_gen::generator::Language::Go,
                    _ => ought_gen::generator::Language::Rust,
                };
                return vec![ought_gen::GeneratedTest {
                    clause_id: clause_id.clone(),
                    code,
                    language,
                    file_path,
                }];
            }
    }

    Vec::new()
}
