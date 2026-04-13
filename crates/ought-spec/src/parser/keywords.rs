//! Pure recognition of deontic keywords, `PENDING` modifier, and
//! `MUST BY <duration>` literals. Stateless helpers — no `ParseState`.

use std::time::Duration;

use crate::types::Keyword;

/// Result of parsing a bold text span as a keyword.
pub(super) enum ParsedKeyword {
    /// A valid keyword, optionally with a PENDING prefix and/or a MUST BY duration.
    Ok {
        keyword: Keyword,
        duration: Option<Duration>,
        pending: bool,
    },
    /// Bold text that isn't a keyword at all — caller should treat it as prose.
    NotAKeyword,
    /// Malformed keyword (e.g. bare `PENDING`, `PENDING WONT`). The caller
    /// should record the message as a parse error and skip the clause.
    Invalid(String),
}

/// Try to parse a keyword from bold text. Handles an optional `PENDING` prefix,
/// MUST BY durations, and enforces the restriction that PENDING may only modify
/// obligation-style keywords.
pub(super) fn parse_keyword(bold_text: &str) -> ParsedKeyword {
    let trimmed = bold_text.trim();
    let upper = trimmed.to_uppercase();

    // Detect PENDING prefix. Bare "PENDING" (with nothing after) is an error —
    // the author must commit to an obligation strength.
    if upper == "PENDING" {
        return ParsedKeyword::Invalid(
            "PENDING must be followed by an obligation keyword (MUST, SHOULD, MAY, etc.)"
                .to_string(),
        );
    }
    let (is_pending, body) = if let Some(rest) = upper.strip_prefix("PENDING ") {
        // "PENDING " is 8 ASCII bytes, so the byte offset is the same in the
        // original (un-uppercased) string.
        let _ = rest;
        (true, trimmed[8..].trim_start())
    } else {
        (false, trimmed)
    };

    let body_upper = body.to_uppercase();
    let (kw, dur) = match parse_obligation(body, &body_upper) {
        Some(pair) => pair,
        None => {
            return if is_pending {
                ParsedKeyword::Invalid(format!(
                    "PENDING must be followed by a valid obligation keyword, found `{}`",
                    body
                ))
            } else {
                ParsedKeyword::NotAKeyword
            };
        }
    };

    // PENDING may prefix any keyword that produces a clause. GIVEN is the
    // sole exception: it's a grouping construct that contributes a condition
    // to its children but never becomes a clause itself, so there is no test
    // to defer.
    if is_pending && kw == Keyword::Given {
        return ParsedKeyword::Invalid(
            "PENDING cannot modify GIVEN — GIVEN is a grouping construct, not \
             a clause, so there is no test to defer"
                .to_string(),
        );
    }

    ParsedKeyword::Ok {
        keyword: kw,
        duration: dur,
        pending: is_pending,
    }
}

/// Match the obligation/permission/negative keywords without any PENDING
/// awareness. `trimmed` is the original-case text and `upper` is its
/// uppercase form — both are passed to avoid re-uppercasing for MUST BY.
fn parse_obligation(trimmed: &str, upper: &str) -> Option<(Keyword, Option<Duration>)> {
    match upper {
        "MUST" => Some((Keyword::Must, None)),
        "MUST NOT" => Some((Keyword::MustNot, None)),
        "SHOULD" => Some((Keyword::Should, None)),
        "SHOULD NOT" => Some((Keyword::ShouldNot, None)),
        "MAY" => Some((Keyword::May, None)),
        "WONT" => Some((Keyword::Wont, None)),
        "GIVEN" => Some((Keyword::Given, None)),
        "OTHERWISE" => Some((Keyword::Otherwise, None)),
        "MUST ALWAYS" => Some((Keyword::MustAlways, None)),
        _ => {
            // Check for MUST BY <duration>
            if upper.starts_with("MUST BY") {
                let after_must_by = trimmed[7..].trim();
                if after_must_by.is_empty() {
                    // "MUST BY" with no duration — return keyword but no duration
                    // so the caller can report a parse error
                    return Some((Keyword::MustBy, None));
                }
                if let Some(dur) = parse_duration(after_must_by) {
                    return Some((Keyword::MustBy, Some(dur)));
                }
                // Has text after MUST BY but it's not a valid duration — still
                // return the keyword so the caller can error
                return Some((Keyword::MustBy, None));
            }
            None
        }
    }
}

/// Parse a duration string like "200ms", "5s", "30m".
fn parse_duration(s: &str) -> Option<Duration> {
    let s = s.trim();
    if let Some(num_str) = s.strip_suffix("ms") {
        let num = num_str.trim().parse::<u64>().ok()?;
        Some(Duration::from_millis(num))
    } else if let Some(num_str) = s.strip_suffix('m') {
        let num = num_str.trim().parse::<u64>().ok()?;
        Some(Duration::from_secs(num * 60))
    } else if let Some(num_str) = s.strip_suffix('s') {
        let num = num_str.trim().parse::<u64>().ok()?;
        Some(Duration::from_secs(num))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Assert `bold` parses to `Ok(keyword, duration, pending)`.
    fn assert_ok(
        bold: &str,
        expected_kw: Keyword,
        expected_dur: Option<Duration>,
        expected_pending: bool,
    ) {
        match parse_keyword(bold) {
            ParsedKeyword::Ok {
                keyword,
                duration,
                pending,
            } => {
                assert_eq!(keyword, expected_kw, "keyword mismatch for {bold:?}");
                assert_eq!(duration, expected_dur, "duration mismatch for {bold:?}");
                assert_eq!(pending, expected_pending, "pending mismatch for {bold:?}");
            }
            ParsedKeyword::NotAKeyword => panic!("expected Ok for {bold:?}, got NotAKeyword"),
            ParsedKeyword::Invalid(msg) => panic!("expected Ok for {bold:?}, got Invalid({msg:?})"),
        }
    }

    #[test]
    fn plain_obligation_keywords() {
        assert_ok("MUST", Keyword::Must, None, false);
        assert_ok("MUST NOT", Keyword::MustNot, None, false);
        assert_ok("SHOULD", Keyword::Should, None, false);
        assert_ok("SHOULD NOT", Keyword::ShouldNot, None, false);
        assert_ok("MAY", Keyword::May, None, false);
        assert_ok("WONT", Keyword::Wont, None, false);
        assert_ok("GIVEN", Keyword::Given, None, false);
        assert_ok("OTHERWISE", Keyword::Otherwise, None, false);
        assert_ok("MUST ALWAYS", Keyword::MustAlways, None, false);
    }

    #[test]
    fn keywords_are_case_insensitive_and_trim_whitespace() {
        assert_ok("must", Keyword::Must, None, false);
        assert_ok("  Must  ", Keyword::Must, None, false);
        assert_ok("must not", Keyword::MustNot, None, false);
    }

    #[test]
    fn must_by_with_valid_duration() {
        assert_ok(
            "MUST BY 200ms",
            Keyword::MustBy,
            Some(Duration::from_millis(200)),
            false,
        );
        assert_ok(
            "MUST BY 5s",
            Keyword::MustBy,
            Some(Duration::from_secs(5)),
            false,
        );
        assert_ok(
            "MUST BY 30m",
            Keyword::MustBy,
            Some(Duration::from_secs(30 * 60)),
            false,
        );
    }

    #[test]
    fn must_by_without_duration_returns_keyword_but_no_duration() {
        // Caller (the state machine) is responsible for emitting an error.
        assert_ok("MUST BY", Keyword::MustBy, None, false);
    }

    #[test]
    fn must_by_with_invalid_duration_returns_keyword_but_no_duration() {
        assert_ok("MUST BY soon", Keyword::MustBy, None, false);
        assert_ok("MUST BY 5 hours", Keyword::MustBy, None, false);
    }

    #[test]
    fn pending_accepts_every_obligation_keyword() {
        assert_ok("PENDING MUST", Keyword::Must, None, true);
        assert_ok("PENDING SHOULD NOT", Keyword::ShouldNot, None, true);
        assert_ok("PENDING MAY", Keyword::May, None, true);
        assert_ok("PENDING OTHERWISE", Keyword::Otherwise, None, true);
        assert_ok(
            "PENDING MUST BY 5s",
            Keyword::MustBy,
            Some(Duration::from_secs(5)),
            true,
        );
    }

    #[test]
    fn bare_pending_is_invalid() {
        match parse_keyword("PENDING") {
            ParsedKeyword::Invalid(msg) => {
                assert!(
                    msg.contains("PENDING must be followed"),
                    "unexpected message: {msg}"
                );
            }
            other => panic!("expected Invalid, got {other:?}", other = variant_name(&other)),
        }
    }

    #[test]
    fn pending_followed_by_non_keyword_is_invalid() {
        match parse_keyword("PENDING FOOBAR") {
            ParsedKeyword::Invalid(msg) => {
                assert!(msg.contains("FOOBAR"), "unexpected message: {msg}");
            }
            other => panic!("expected Invalid, got {other:?}", other = variant_name(&other)),
        }
    }

    #[test]
    fn pending_given_is_forbidden() {
        // GIVEN is a grouping construct with no clause to defer, so PENDING
        // is nonsense here.
        match parse_keyword("PENDING GIVEN") {
            ParsedKeyword::Invalid(msg) => {
                assert!(
                    msg.contains("GIVEN"),
                    "unexpected message: {msg}"
                );
            }
            other => panic!("expected Invalid, got {other:?}", other = variant_name(&other)),
        }
    }

    #[test]
    fn plain_prose_is_not_a_keyword() {
        assert!(matches!(
            parse_keyword("Hello, world"),
            ParsedKeyword::NotAKeyword
        ));
        assert!(matches!(
            parse_keyword("note on durability"),
            ParsedKeyword::NotAKeyword
        ));
    }

    #[test]
    fn empty_string_is_not_a_keyword() {
        assert!(matches!(parse_keyword(""), ParsedKeyword::NotAKeyword));
        assert!(matches!(parse_keyword("   "), ParsedKeyword::NotAKeyword));
    }

    fn variant_name(k: &ParsedKeyword) -> &'static str {
        match k {
            ParsedKeyword::Ok { .. } => "Ok",
            ParsedKeyword::NotAKeyword => "NotAKeyword",
            ParsedKeyword::Invalid(_) => "Invalid",
        }
    }
}
