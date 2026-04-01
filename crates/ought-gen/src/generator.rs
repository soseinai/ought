use std::path::PathBuf;

use ought_spec::ClauseId;

/// A test generated from a single clause.
#[derive(Debug, Clone)]
pub struct GeneratedTest {
    pub clause_id: ClauseId,
    pub code: String,
    pub language: Language,
    pub file_path: PathBuf,
}

/// Target language for test generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    Rust,
    Python,
    TypeScript,
    JavaScript,
    Go,
}

/// Convert a keyword enum to its display string.
pub fn keyword_str(kw: ought_spec::Keyword) -> &'static str {
    match kw {
        ought_spec::Keyword::Must => "MUST",
        ought_spec::Keyword::MustNot => "MUST NOT",
        ought_spec::Keyword::Should => "SHOULD",
        ought_spec::Keyword::ShouldNot => "SHOULD NOT",
        ought_spec::Keyword::May => "MAY",
        ought_spec::Keyword::Wont => "WONT",
        ought_spec::Keyword::Given => "GIVEN",
        ought_spec::Keyword::Otherwise => "OTHERWISE",
        ought_spec::Keyword::MustAlways => "MUST ALWAYS",
        ought_spec::Keyword::MustBy => "MUST BY",
    }
}
