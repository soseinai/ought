use std::path::PathBuf;

use chrono::{DateTime, Utc};
use ought_spec::{ClauseId, Keyword};

// -- Survey --

/// Results from `ought analyze survey`.
#[derive(Debug, Clone)]
pub struct SurveyResult {
    pub uncovered: Vec<UncoveredBehavior>,
}

/// A behavior found in source code with no corresponding spec clause.
#[derive(Debug, Clone)]
pub struct UncoveredBehavior {
    pub file: PathBuf,
    pub line: usize,
    pub description: String,
    pub suggested_clause: String,
    pub suggested_keyword: Keyword,
    pub suggested_spec: PathBuf,
}

// -- Audit --

/// Results from `ought analyze audit`.
#[derive(Debug, Clone)]
pub struct AuditResult {
    pub findings: Vec<AuditFinding>,
}

/// A coherence issue found across specs.
#[derive(Debug, Clone)]
pub struct AuditFinding {
    pub kind: AuditFindingKind,
    pub description: String,
    pub clauses: Vec<ClauseId>,
    pub suggestion: Option<String>,
    pub confidence: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuditFindingKind {
    Contradiction,
    Gap,
    Ambiguity,
    Redundancy,
}

// -- Blame --

/// Results from `ought debug blame`.
#[derive(Debug, Clone)]
pub struct BlameResult {
    pub clause_id: ClauseId,
    pub last_passed: Option<DateTime<Utc>>,
    pub first_failed: Option<DateTime<Utc>>,
    pub likely_commit: Option<CommitInfo>,
    pub narrative: String,
    pub suggested_fix: Option<String>,
}

/// Information about a git commit.
#[derive(Debug, Clone)]
pub struct CommitInfo {
    pub hash: String,
    pub message: String,
    pub author: String,
    pub date: DateTime<Utc>,
}

// -- Bisect --

/// Results from `ought debug bisect`.
#[derive(Debug, Clone)]
pub struct BisectResult {
    pub clause_id: ClauseId,
    pub breaking_commit: CommitInfo,
    pub diff_summary: String,
}
