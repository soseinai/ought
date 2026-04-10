use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};

/// A parsed `.ought.md` spec file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Spec {
    pub name: String,
    pub metadata: Metadata,
    pub sections: Vec<Section>,
    pub source_path: PathBuf,
}

/// Frontmatter metadata from the top of a spec file.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Metadata {
    pub context: Option<String>,
    pub sources: Vec<String>,
    pub schemas: Vec<String>,
    pub requires: Vec<SpecRef>,
}

/// A reference to another spec file (from `requires:` or inline links).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecRef {
    pub label: String,
    pub path: PathBuf,
    pub anchor: Option<String>,
}

/// A section within a spec (maps to markdown headings).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Section {
    pub title: String,
    pub depth: u8,
    pub prose: String,
    pub clauses: Vec<Clause>,
    pub subsections: Vec<Section>,
}

/// The core IR type — a single testable clause.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Clause {
    pub id: ClauseId,
    pub keyword: Keyword,
    pub severity: Severity,
    pub text: String,
    pub condition: Option<String>,
    pub otherwise: Vec<Clause>,
    pub temporal: Option<Temporal>,
    pub hints: Vec<String>,
    pub source_location: SourceLocation,
    pub content_hash: String,
    /// Clause is declared with a `PENDING` prefix: the author has committed to
    /// the obligation strength but the implementation is deferred. The
    /// generator must skip pending clauses and the runner reports them as
    /// `pending` rather than passed/failed/skipped.
    #[serde(default)]
    pub pending: bool,
}

/// Stable identifier for a clause, derived from section path + clause text.
/// e.g. `auth::login::must_return_jwt`
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ClauseId(pub String);

impl std::fmt::Display for ClauseId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Deontic keyword — the operator on a clause.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Keyword {
    Must,
    MustNot,
    Should,
    ShouldNot,
    May,
    Wont,
    Given,
    Otherwise,
    MustAlways,
    MustBy,
}

/// Severity level derived from the keyword.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Severity {
    Required,
    Recommended,
    Optional,
    NegativeConfirmation,
}

impl Keyword {
    pub fn severity(self) -> Severity {
        match self {
            Keyword::Must | Keyword::MustNot | Keyword::MustAlways | Keyword::MustBy => {
                Severity::Required
            }
            Keyword::Should | Keyword::ShouldNot => Severity::Recommended,
            Keyword::May => Severity::Optional,
            Keyword::Wont => Severity::NegativeConfirmation,
            Keyword::Given | Keyword::Otherwise => Severity::Required,
        }
    }
}

/// Temporal qualifier for MUST ALWAYS and MUST BY clauses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Temporal {
    /// Must hold across all states/inputs. Generates property-based tests.
    Invariant,
    /// Must complete within the given duration. Generates timed assertions.
    Deadline(Duration),
}

/// Location of a clause in a source file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceLocation {
    pub file: PathBuf,
    pub line: usize,
}

/// An error encountered during parsing.
#[derive(Debug, Clone, thiserror::Error)]
#[error("{file}:{line}: {message}")]
pub struct ParseError {
    pub file: PathBuf,
    pub line: usize,
    pub message: String,
}
