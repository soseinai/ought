//! Public parsing interface for ought specs.
//!
//! The [`Parser`] trait is the abstraction boundary; [`OughtMdParser`] is
//! the canonical implementation for `.ought.md` files. The state-machine
//! driving the actual parse, plus all pure helpers, live in sibling
//! submodules below this one.

use std::path::Path;

use crate::types::{ParseError, Spec};

mod clauses;
mod ids;
mod keywords;
mod metadata;
mod state;

/// The public interface for parsing spec files into the ought IR.
///
/// Mirrors `ought_run::Runner`: one trait, one concrete implementation today
/// (`OughtMdParser`), and room to add more spec formats without breaking
/// callers. Most consumers should take `&dyn Parser` (or `impl Parser`)
/// rather than naming a concrete type.
pub trait Parser: Send + Sync {
    /// Parse a spec file from disk. The default implementation reads the
    /// file and delegates to [`Parser::parse_string`], so format-specific
    /// parsers usually only need to implement `parse_string`.
    fn parse_file(&self, path: &Path) -> Result<Spec, Vec<ParseError>> {
        let content = std::fs::read_to_string(path).map_err(|e| {
            vec![ParseError {
                file: path.to_path_buf(),
                line: 0,
                message: format!("failed to read file: {}", e),
            }]
        })?;
        self.parse_string(&content, path)
    }

    /// Parse a spec from an in-memory string, using `path` only as the
    /// source-location label for error messages and source locations.
    fn parse_string(&self, content: &str, path: &Path) -> Result<Spec, Vec<ParseError>>;

    /// Short, stable name for this parser (e.g. `"ought.md"`). Used for
    /// diagnostics and, eventually, format dispatch.
    fn name(&self) -> &str;
}

/// Canonical parser for `.ought.md` files: CommonMark markdown with bold
/// deontic keywords (`**MUST**`, `**SHOULD**`, …), GIVEN nesting,
/// OTHERWISE chains, and MUST BY duration literals.
///
/// Pure Rust, no LLM dependency.
#[derive(Debug, Default, Clone, Copy)]
pub struct OughtMdParser;

impl Parser for OughtMdParser {
    fn parse_string(&self, content: &str, path: &Path) -> Result<Spec, Vec<ParseError>> {
        state::parse_string(content, path)
    }

    fn name(&self) -> &str {
        "ought.md"
    }
}
