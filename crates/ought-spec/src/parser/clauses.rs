//! Transform a `PendingItem` tree (the parser's transient IR) into the
//! public `Clause` tree (ought's stable IR).
//!
//! Handles the three semantic moves that happen at list-flush time:
//!
//! 1. **GIVEN is a grouping construct** — it contributes its text as a
//!    `condition` to all nested children and never becomes a clause itself.
//! 2. **OTHERWISE is a fallback chain** — OTHERWISE children of an
//!    obligation become `parent.otherwise`, inheriting the parent's
//!    severity and its pending flag.
//! 3. **ID + content-hash construction** — deterministic from
//!    `spec_name::section1::section2::keyword_summary`, truncated to
//!    120 chars.
//!
//! Pure — no `ParseState`, no markdown events. Easy to unit-test in
//! isolation if we ever want to.

use std::path::Path;

use crate::types::{Clause, ClauseId, Keyword, SourceLocation};

use super::ids::{content_hash, slugify};
use super::state::PendingItem;

/// Convert one top-level [`PendingItem`] (optionally already carrying a
/// GIVEN-derived condition) into a list of [`Clause`]s, ready to be
/// attached to their enclosing section.
pub(super) fn build_clauses(
    file: &Path,
    spec_name: &str,
    section_path: &[String],
    item: PendingItem,
    given_condition: Option<String>,
) -> Vec<Clause> {
    let mut result = Vec::new();

    match item.keyword {
        Keyword::Given => {
            // GIVEN is a grouping construct. Its text is the condition.
            // All nested items inherit this condition.
            let condition = Some(item.text.clone());
            for nested in item.nested_items {
                let nested_clauses =
                    build_clauses(file, spec_name, section_path, nested, condition.clone());
                result.extend(nested_clauses);
            }
        }
        _ => {
            // Build the clause
            let condition = given_condition;
            let keyword_slug = match item.keyword {
                Keyword::Must => "must",
                Keyword::MustNot => "must_not",
                Keyword::Should => "should",
                Keyword::ShouldNot => "should_not",
                Keyword::May => "may",
                Keyword::Wont => "wont",
                Keyword::MustAlways => "must_always",
                Keyword::MustBy => "must_by",
                Keyword::Otherwise => "otherwise",
                Keyword::Given => unreachable!(),
            };

            let text_slug = slugify(&item.text);
            let summary = if text_slug.is_empty() {
                keyword_slug.to_string()
            } else {
                format!("{}_{}", keyword_slug, text_slug)
            };

            // Build ID: spec_name::section1::section2::keyword_summary
            let mut id_parts: Vec<&str> = Vec::new();
            id_parts.push(spec_name);
            for sp in section_path {
                id_parts.push(sp);
            }
            id_parts.push(&summary);
            let id_str = id_parts.join("::");

            // Truncate if too long
            let id_str = if id_str.len() > 120 {
                let mut s = id_str[..120].to_string();
                while s.ends_with('_') || s.ends_with(':') {
                    s.pop();
                }
                s
            } else {
                id_str
            };

            let hash = content_hash(item.keyword, &item.text, &condition, item.pending);

            // Build otherwise clauses from nested items that are OTHERWISE.
            // An OTHERWISE child is pending if either the parent is pending
            // (inheritance — a deferred obligation's fallback chain is also
            // deferred) OR the child was explicitly written as
            // `**PENDING OTHERWISE**`.
            let mut otherwise_clauses = Vec::new();
            let mut other_nested = Vec::new();

            for nested in item.nested_items {
                if nested.keyword == Keyword::Otherwise {
                    let ow_pending = item.pending || nested.pending;

                    // Build otherwise clause
                    let ow_summary = format!("otherwise_{}", slugify(&nested.text));
                    let mut ow_id_parts: Vec<&str> = Vec::new();
                    ow_id_parts.push(spec_name);
                    for sp in section_path {
                        ow_id_parts.push(sp);
                    }
                    ow_id_parts.push(&ow_summary);
                    let ow_id_str = ow_id_parts.join("::");

                    let ow_hash = content_hash(
                        Keyword::Otherwise,
                        &nested.text,
                        &condition,
                        ow_pending,
                    );

                    otherwise_clauses.push(Clause {
                        id: ClauseId(ow_id_str),
                        keyword: Keyword::Otherwise,
                        severity: item.keyword.severity(), // inherit parent severity
                        text: nested.text,
                        condition: condition.clone(),
                        otherwise: Vec::new(),
                        temporal: None,
                        hints: nested.hints,
                        source_location: SourceLocation {
                            file: file.to_path_buf(),
                            line: nested.line,
                        },
                        content_hash: ow_hash,
                        pending: ow_pending,
                    });
                } else {
                    other_nested.push(nested);
                }
            }

            let clause = Clause {
                id: ClauseId(id_str),
                keyword: item.keyword,
                severity: item.keyword.severity(),
                text: item.text,
                condition,
                otherwise: otherwise_clauses,
                temporal: item.temporal,
                hints: item.hints,
                source_location: SourceLocation {
                    file: file.to_path_buf(),
                    line: item.line,
                },
                content_hash: hash,
                pending: item.pending,
            };

            result.push(clause);

            // Any non-OTHERWISE nested items get turned into clauses too
            // (rare case, but handle gracefully)
            for nested in other_nested {
                let nested_clauses = build_clauses(file, spec_name, section_path, nested, None);
                result.extend(nested_clauses);
            }
        }
    }

    result
}
