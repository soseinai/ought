//! Pure primitives for building clause IDs, content hashes, and mapping
//! byte offsets to line numbers. No state, no `ParseState`.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use crate::types::Keyword;

/// Compute a line number (1-based) from byte offset in the source text.
pub(super) fn line_number_at_offset(source: &str, offset: usize) -> usize {
    let clamped = offset.min(source.len());
    source[..clamped].bytes().filter(|&b| b == b'\n').count() + 1
}

/// Slugify a string: lowercase, replace non-alphanumeric with underscore, collapse runs.
pub(super) fn slugify(s: &str) -> String {
    let mut result = String::new();
    let mut last_was_sep = true; // avoid leading underscore
    for c in s.chars() {
        if c.is_alphanumeric() {
            result.push(c.to_ascii_lowercase());
            last_was_sep = false;
        } else if !last_was_sep {
            result.push('_');
            last_was_sep = true;
        }
    }
    // Trim trailing underscore
    while result.ends_with('_') {
        result.pop();
    }
    // Truncate to reasonable length
    if result.len() > 60 {
        result.truncate(60);
        while result.ends_with('_') {
            result.pop();
        }
    }
    result
}

/// Generate a content hash from keyword, text, condition, and pending flag.
///
/// Pending is included in the hash so that promoting a clause from
/// `PENDING MUST` to `MUST` registers as a content change and forces the
/// generator to pick it up as stale.
pub(super) fn content_hash(
    keyword: Keyword,
    text: &str,
    condition: &Option<String>,
    pending: bool,
) -> String {
    let mut hasher = DefaultHasher::new();
    format!("{:?}", keyword).hash(&mut hasher);
    text.hash(&mut hasher);
    condition.hash(&mut hasher);
    pending.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}
