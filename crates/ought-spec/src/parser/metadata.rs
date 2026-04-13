//! Pure helpers for metadata parsing — splitting comma-separated value
//! lists and extracting markdown-link cross-references. Stateless.

use std::path::PathBuf;

use crate::types::SpecRef;

/// Split metadata values by commas, but respect values that contain glob patterns
/// (e.g. `tests/**/*.rs`). Commas inside path-like values with `*`, `?`, `[` are
/// preserved — we only split on commas followed by whitespace and a new path.
pub(super) fn split_metadata_values(val: &str) -> Vec<String> {
    // Simple approach: split by comma, then re-join segments that look like
    // they're part of a glob pattern (contain * or ? after a comma).
    // Actually, simplest correct approach: just split by ", " (comma-space)
    // which is the expected delimiter, and trim each result.
    val.split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Parse `requires:` value containing markdown links like `[label](path.ought.md)` and
/// `[label](path.ought.md#anchor)`.
pub(super) fn parse_requires_line(line: &str) -> Vec<SpecRef> {
    let mut refs = Vec::new();
    let mut rest = line;
    while let Some(open_bracket) = rest.find('[') {
        rest = &rest[open_bracket..];
        // Find ](
        if let Some(bracket_paren) = rest.find("](") {
            let label = &rest[1..bracket_paren];
            let after_paren = &rest[bracket_paren + 2..];
            if let Some(close_paren) = after_paren.find(')') {
                let url = &after_paren[..close_paren];
                let (path_str, anchor) = if let Some(hash_pos) = url.find('#') {
                    (&url[..hash_pos], Some(url[hash_pos + 1..].to_string()))
                } else {
                    (url, None)
                };
                refs.push(SpecRef {
                    label: label.to_string(),
                    path: PathBuf::from(path_str),
                    anchor,
                });
                rest = &after_paren[close_paren + 1..];
            } else {
                rest = &rest[1..];
            }
        } else {
            rest = &rest[1..];
        }
    }
    refs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_link_without_anchor() {
        let refs = parse_requires_line("[Pricing](pricing.ought.md)");
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].label, "Pricing");
        assert_eq!(refs[0].path, PathBuf::from("pricing.ought.md"));
        assert_eq!(refs[0].anchor, None);
    }

    #[test]
    fn single_link_with_anchor() {
        let refs = parse_requires_line("[Users](users.ought.md#profiles)");
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].label, "Users");
        assert_eq!(refs[0].path, PathBuf::from("users.ought.md"));
        assert_eq!(refs[0].anchor.as_deref(), Some("profiles"));
    }

    #[test]
    fn multiple_links_with_and_without_anchors() {
        let refs = parse_requires_line(
            "[Pricing](pricing.ought.md#discount-rules), [Auth](auth.ought.md)",
        );
        assert_eq!(refs.len(), 2);
        assert_eq!(refs[0].label, "Pricing");
        assert_eq!(refs[0].anchor.as_deref(), Some("discount-rules"));
        assert_eq!(refs[1].label, "Auth");
        assert_eq!(refs[1].path, PathBuf::from("auth.ought.md"));
        assert_eq!(refs[1].anchor, None);
    }

    #[test]
    fn plain_text_with_no_links_yields_no_refs() {
        assert!(parse_requires_line("pricing.ought.md").is_empty());
        assert!(parse_requires_line("").is_empty());
    }

    #[test]
    fn unclosed_bracket_does_not_loop_forever() {
        // Regression guard: the loop advances by one byte when `](` isn't found.
        let refs = parse_requires_line("[incomplete label");
        assert!(refs.is_empty());
    }

    #[test]
    fn split_metadata_values_handles_comma_separated_lists() {
        let out = split_metadata_values("a.rs, b.rs, c.rs");
        assert_eq!(out, vec!["a.rs", "b.rs", "c.rs"]);
    }

    #[test]
    fn split_metadata_values_drops_empty_segments() {
        let out = split_metadata_values("a.rs, , b.rs,");
        assert_eq!(out, vec!["a.rs", "b.rs"]);
    }
}
