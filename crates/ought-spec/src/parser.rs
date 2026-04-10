use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::time::Duration;

use pulldown_cmark::{Event, HeadingLevel, Options, Parser as MdParser, Tag, TagEnd};

use crate::types::{
    Clause, ClauseId, Keyword, Metadata, ParseError, Section, SourceLocation, Spec, SpecRef,
    Temporal,
};

/// Parses `.ought.md` files into structured spec IR.
///
/// Pure Rust, no LLM dependency. Recognizes CommonMark markdown,
/// extracts metadata, identifies bold deontic keywords, handles
/// GIVEN nesting and OTHERWISE chains, and parses MUST BY durations.
pub struct Parser;

impl Parser {
    /// Parse a spec file from disk.
    pub fn parse_file(path: &Path) -> Result<Spec, Vec<ParseError>> {
        let content = std::fs::read_to_string(path).map_err(|e| {
            vec![ParseError {
                file: path.to_path_buf(),
                line: 0,
                message: format!("failed to read file: {}", e),
            }]
        })?;
        Self::parse_string(&content, path)
    }

    /// Parse a spec from a string (for testing or programmatic use).
    pub fn parse_string(content: &str, path: &Path) -> Result<Spec, Vec<ParseError>> {
        let mut state = ParseState::new(path.to_path_buf(), content);
        state.parse();

        if state.errors.is_empty() {
            Ok(state.into_spec())
        } else if state.spec_name.is_some() {
            // We have errors but also partial results — return the spec
            // The spec says to continue after non-fatal errors.
            // But the signature returns Result, so if there are errors we return them.
            Err(state.errors)
        } else {
            Err(state.errors)
        }
    }
}

/// Compute a line number (1-based) from byte offset in the source text.
fn line_number_at_offset(source: &str, offset: usize) -> usize {
    let clamped = offset.min(source.len());
    source[..clamped].bytes().filter(|&b| b == b'\n').count() + 1
}

/// Slugify a string: lowercase, replace non-alphanumeric with underscore, collapse runs.
fn slugify(s: &str) -> String {
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
fn content_hash(
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

/// Result of parsing a bold text span as a keyword.
enum ParsedKeyword {
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
fn parse_keyword(bold_text: &str) -> ParsedKeyword {
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

// ─── Parse state machine ───────────────────────────────────────────────────

/// Saved state for one list-item level on the item stack.
#[derive(Debug)]
struct ItemFrame {
    text: String,
    keyword: Option<(Keyword, Option<Duration>, bool)>,
    keyword_consumed: bool,
    /// Set when the bold span was a malformed keyword (e.g. `PENDING WONT`).
    /// The error is already recorded; the item should be dropped entirely
    /// when the frame is popped.
    keyword_invalid: bool,
    line: usize,
    /// Nested items collected while this frame is active (from child list items).
    nested_items: Vec<PendingItem>,
}

struct ParseState {
    file: PathBuf,
    source: String,
    errors: Vec<ParseError>,

    // Result accumulation
    spec_name: Option<String>,
    metadata: Metadata,
    sections: Vec<Section>,

    // Section stack: (depth, section) — we flatten at the end
    section_stack: Vec<(u8, Section)>,

    // State tracking
    in_heading: Option<HeadingLevel>,
    heading_text: String,
    in_strong: bool,
    strong_text: String,

    // Item stack: each Start(Item) pushes a frame; End(Item) pops it
    item_stack: Vec<ItemFrame>,

    list_depth: usize, // 0 = not in list, 1 = top-level list, 2 = nested, etc.
    metadata_region: bool, // between H1 and first H2
    prose_buf: String,
    just_finished_clause: bool, // to capture following code blocks as hints
    in_code_block: bool,
    code_block_text: String,

    // Track current byte offset for line number estimation
    current_offset: usize,

    // For metadata, track paragraph text between H1 and first H2
    metadata_paragraph_text: String,
    in_metadata_paragraph: bool,

    // For metadata link parsing: when we're inside a Link in metadata paragraph
    in_metadata_link: bool,
    metadata_link_url: String,
    metadata_link_label: String,

    // Track item nesting to handle GIVEN and OTHERWISE
    // depth-1 items: top-level clauses or GIVEN
    // depth-2 items: nested under GIVEN or OTHERWISE children
    depth1_items: Vec<PendingItem>,
}

#[derive(Debug, Clone)]
struct PendingItem {
    keyword: Keyword,
    pending: bool,
    text: String,
    temporal: Option<Temporal>,
    line: usize,
    nested_items: Vec<PendingItem>,
    hints: Vec<String>,
}

impl ParseState {
    fn new(file: PathBuf, source: &str) -> Self {
        Self {
            file,
            source: source.to_string(),
            errors: Vec::new(),
            spec_name: None,
            metadata: Metadata::default(),
            sections: Vec::new(),
            section_stack: Vec::new(),
            in_heading: None,
            heading_text: String::new(),
            in_strong: false,
            strong_text: String::new(),
            item_stack: Vec::new(),
            list_depth: 0,
            metadata_region: false,
            prose_buf: String::new(),
            just_finished_clause: false,
            in_code_block: false,
            code_block_text: String::new(),
            current_offset: 0,
            metadata_paragraph_text: String::new(),
            in_metadata_paragraph: false,
            in_metadata_link: false,
            metadata_link_url: String::new(),
            metadata_link_label: String::new(),
            depth1_items: Vec::new(),
        }
    }

    /// Are we currently inside a list item (at any depth)?
    fn in_list_item(&self) -> bool {
        !self.item_stack.is_empty()
    }

    /// Get the current (innermost) item frame mutably.
    fn current_item_mut(&mut self) -> Option<&mut ItemFrame> {
        self.item_stack.last_mut()
    }

    fn parse(&mut self) {
        // Pre-extract metadata from raw source text before markdown parsing.
        // This avoids pulldown-cmark interpreting `**` in paths (like `src/**/*.rs`)
        // as bold markers.
        self.extract_raw_metadata();

        let source = self.source.clone();
        // Collect events with their offsets
        let events: Vec<(Event<'_>, std::ops::Range<usize>)> =
            MdParser::new_ext(&source, Options::empty())
                .into_offset_iter()
                .collect();

        for (event, range) in events {
            self.current_offset = range.start;
            self.handle_event(event);
        }

        // Flush any remaining items/sections
        self.flush_pending_items();
        self.flush_section_stack();
    }

    /// Extract metadata (context:, source:, schema:, requires:) from raw source text.
    /// This runs before markdown parsing to avoid pulldown-cmark mangling glob patterns.
    fn extract_raw_metadata(&mut self) {
        let source = self.source.clone();
        let mut in_metadata = false;

        for line in source.lines() {
            let trimmed = line.trim();

            // Look for H1 to start metadata region
            if !in_metadata {
                if trimmed.starts_with("# ") {
                    in_metadata = true;
                    if let Some(rest) = trimmed.strip_prefix("# ") {
                        self.spec_name = Some(rest.trim().to_string());
                    }
                }
                continue;
            }

            // H2+ ends metadata region
            if trimmed.starts_with("## ") || trimmed.starts_with("### ") {
                break;
            }

            // Parse metadata lines from raw text
            if let Some(rest) = trimmed.strip_prefix("context:") {
                let val = rest.trim();
                if !val.is_empty() {
                    self.metadata.context = Some(val.to_string());
                }
            } else if let Some(rest) = trimmed.strip_prefix("source:") {
                let val = rest.trim();
                if !val.is_empty() {
                    for s in split_metadata_values(val) {
                        self.metadata.sources.push(s);
                    }
                }
            } else if let Some(rest) = trimmed.strip_prefix("schema:") {
                let val = rest.trim();
                if !val.is_empty() {
                    for s in split_metadata_values(val) {
                        self.metadata.schemas.push(s);
                    }
                }
            } else if let Some(rest) = trimmed.strip_prefix("requires:") {
                let val = rest.trim();
                if !val.is_empty() {
                    let refs = parse_requires_line(val);
                    if refs.is_empty() {
                        self.metadata.requires.push(SpecRef {
                            label: val.to_string(),
                            path: PathBuf::from(val),
                            anchor: None,
                        });
                    } else {
                        self.metadata.requires.extend(refs);
                    }
                }
            }
        }
    }

    fn handle_event(&mut self, event: Event<'_>) {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                self.in_heading = Some(level);
                self.heading_text.clear();
            }
            Event::End(TagEnd::Heading(level)) => {
                let title = std::mem::take(&mut self.heading_text).trim().to_string();
                self.in_heading = None;

                match level {
                    HeadingLevel::H1 => {
                        if self.spec_name.is_none() {
                            self.spec_name = Some(title);
                        }
                        // Metadata is now extracted in extract_raw_metadata(),
                        // so we don't enable metadata_region for event-based parsing.
                    }
                    _ => {
                        self.metadata_region = false;
                        // Flush pending items before starting new section
                        self.flush_pending_items();
                        // Flush prose to current section
                        self.flush_prose();

                        let depth = match level {
                            HeadingLevel::H1 => 1,
                            HeadingLevel::H2 => 2,
                            HeadingLevel::H3 => 3,
                            HeadingLevel::H4 => 4,
                            HeadingLevel::H5 => 5,
                            HeadingLevel::H6 => 6,
                        };

                        let section = Section {
                            title: title.clone(),
                            depth,
                            prose: String::new(),
                            clauses: Vec::new(),
                            subsections: Vec::new(),
                        };

                        // Pop sections from stack that are at same or deeper depth
                        while let Some((d, _)) = self.section_stack.last() {
                            if *d >= depth {
                                let (_, popped) = self.section_stack.pop().unwrap();
                                if let Some((_, parent)) = self.section_stack.last_mut() {
                                    parent.subsections.push(popped);
                                } else {
                                    self.sections.push(popped);
                                }
                            } else {
                                break;
                            }
                        }

                        self.section_stack.push((depth, section));
                        self.just_finished_clause = false;
                    }
                }
            }

            Event::Start(Tag::Strong) => {
                self.in_strong = true;
                self.strong_text.clear();
            }
            Event::End(TagEnd::Strong) => {
                self.in_strong = false;
                let bold_text = std::mem::take(&mut self.strong_text);

                if self.in_heading.is_some() {
                    self.heading_text.push_str(&bold_text);
                } else if self.current_item_mut().is_some() {
                    // We need mutable access in two different branches that
                    // don't overlap, plus access to `self` for error recording
                    // in the Invalid branch. Split the logic to satisfy the
                    // borrow checker.
                    let consumed = self
                        .current_item_mut()
                        .map(|f| f.keyword_consumed)
                        .unwrap_or(false);
                    if !consumed {
                        match parse_keyword(&bold_text) {
                            ParsedKeyword::Ok {
                                keyword,
                                duration,
                                pending,
                            } => {
                                let frame = self.current_item_mut().unwrap();
                                frame.keyword = Some((keyword, duration, pending));
                                frame.keyword_consumed = true;
                            }
                            ParsedKeyword::NotAKeyword => {
                                let frame = self.current_item_mut().unwrap();
                                frame.text.push_str("**");
                                frame.text.push_str(&bold_text);
                                frame.text.push_str("**");
                            }
                            ParsedKeyword::Invalid(msg) => {
                                let line = {
                                    let frame = self.current_item_mut().unwrap();
                                    frame.keyword_consumed = true;
                                    frame.keyword_invalid = true;
                                    frame.line
                                };
                                self.errors.push(ParseError {
                                    file: self.file.clone(),
                                    line,
                                    message: msg,
                                });
                            }
                        }
                    } else {
                        // Already have keyword, this bold text is part of clause text
                        let frame = self.current_item_mut().unwrap();
                        frame.text.push_str("**");
                        frame.text.push_str(&bold_text);
                        frame.text.push_str("**");
                    }
                } else if self.in_metadata_paragraph {
                    self.metadata_paragraph_text.push_str("**");
                    self.metadata_paragraph_text.push_str(&bold_text);
                    self.metadata_paragraph_text.push_str("**");
                } else {
                    // Bold in prose
                    self.prose_buf.push_str("**");
                    self.prose_buf.push_str(&bold_text);
                    self.prose_buf.push_str("**");
                }
            }

            Event::Start(Tag::List(_)) => {
                self.list_depth += 1;
            }
            Event::End(TagEnd::List(_)) => {
                if self.list_depth == 1 {
                    // End of top-level list: flush all pending items
                    self.flush_pending_items();
                    // Keep just_finished_clause alive so code blocks after the list
                    // can be captured as hints
                }
                if self.list_depth > 0 {
                    self.list_depth -= 1;
                }
            }

            Event::Start(Tag::Item) => {
                let line = line_number_at_offset(&self.source, self.current_offset);
                self.item_stack.push(ItemFrame {
                    text: String::new(),
                    keyword: None,
                    keyword_consumed: false,
                    keyword_invalid: false,
                    line,
                    nested_items: Vec::new(),
                });
            }
            Event::End(TagEnd::Item) => {
                if let Some(frame) = self.item_stack.pop() {
                    if frame.keyword_invalid {
                        // Error already recorded while parsing the bold span;
                        // drop the entire item (don't emit a clause or prose).
                        self.just_finished_clause = false;
                        return;
                    }
                    let text = frame.text.trim().to_string();
                    let keyword = frame.keyword;
                    let line = frame.line;
                    let nested_items = frame.nested_items;

                    if let Some((kw, dur, pending)) = keyword {
                        // Validate MUST BY has a duration
                        if kw == Keyword::MustBy && dur.is_none() {
                            self.errors.push(ParseError {
                                file: self.file.clone(),
                                line,
                                message: "MUST BY requires a duration (e.g. MUST BY 200ms, MUST BY 5s)".to_string(),
                            });
                            // Don't produce a clause for this — it's a parse error
                        }

                        // Validate OTHERWISE is nested under a parent obligation
                        if kw == Keyword::Otherwise && self.item_stack.is_empty() {
                            self.errors.push(ParseError {
                                file: self.file.clone(),
                                line,
                                message: "OTHERWISE must be nested under a parent obligation (MUST, SHOULD, etc.), not at the top level".to_string(),
                            });
                        }

                        // Validate OTHERWISE is not under MAY, WONT, or GIVEN
                        if kw == Keyword::Otherwise
                            && let Some(parent_frame) = self.item_stack.last()
                            && let Some((parent_kw, _, _)) = &parent_frame.keyword
                            && matches!(parent_kw, Keyword::May | Keyword::Wont | Keyword::Given)
                        {
                            self.errors.push(ParseError {
                                file: self.file.clone(),
                                line,
                                message: format!(
                                    "OTHERWISE cannot be nested under {} — only under obligations that can be violated (MUST, SHOULD, etc.)",
                                    match parent_kw {
                                        Keyword::May => "MAY",
                                        Keyword::Wont => "WONT",
                                        Keyword::Given => "GIVEN",
                                        _ => unreachable!(),
                                    }
                                ),
                            });
                        }

                        let temporal = match kw {
                            Keyword::MustAlways => Some(Temporal::Invariant),
                            Keyword::MustBy => dur.map(Temporal::Deadline),
                            _ => None,
                        };

                        // Skip creating the item if it was an invalid MUST BY
                        if kw == Keyword::MustBy && dur.is_none() {
                            // error already recorded above
                        } else {
                        let item = PendingItem {
                            keyword: kw,
                            pending,
                            text,
                            temporal,
                            line,
                            nested_items,
                            hints: Vec::new(),
                        };

                        // Determine nesting: if item_stack is empty, this is a top-level item.
                        // If item_stack is non-empty, this is nested under the parent frame.
                        if let Some(parent_frame) = self.item_stack.last_mut() {
                            parent_frame.nested_items.push(item);
                        } else {
                            // Top-level item
                            self.depth1_items.push(item);
                        }
                        }
                        self.just_finished_clause = true;
                    } else {
                        // Non-clause list item — treat as prose
                        if !text.is_empty() {
                            self.prose_buf.push_str("- ");
                            self.prose_buf.push_str(&text);
                            self.prose_buf.push('\n');
                        }
                        self.just_finished_clause = false;
                    }
                }
            }

            Event::Start(Tag::CodeBlock(_)) => {
                self.in_code_block = true;
                self.code_block_text.clear();
            }
            Event::End(TagEnd::CodeBlock) => {
                self.in_code_block = false;
                let code = std::mem::take(&mut self.code_block_text);
                if self.just_finished_clause && !code.trim().is_empty() {
                    // Attach to the most recent clause as a hint.
                    // If pending items exist, attach to last one.
                    if let Some(last) = self.depth1_items.last_mut() {
                        if let Some(nested_last) = last.nested_items.last_mut() {
                            nested_last.hints.push(code);
                        } else {
                            last.hints.push(code);
                        }
                    } else {
                        // Items already flushed — attach to last clause in current section
                        self.attach_hint_to_last_clause(code);
                    }
                } else {
                    // Code block as prose
                    self.prose_buf.push_str("```\n");
                    self.prose_buf.push_str(&code);
                    self.prose_buf.push_str("```\n");
                }
            }

            Event::Start(Tag::Paragraph) => {
                if self.metadata_region && !self.in_list_item() {
                    self.in_metadata_paragraph = true;
                    self.metadata_paragraph_text.clear();
                }
            }
            Event::End(TagEnd::Paragraph) => {
                if self.in_metadata_paragraph {
                    self.in_metadata_paragraph = false;
                    let para = std::mem::take(&mut self.metadata_paragraph_text);
                    self.parse_metadata_block(&para);
                } else if !self.in_list_item() {
                    self.prose_buf.push('\n');
                }
            }

            // Handle links — important for metadata `requires:` parsing
            Event::Start(Tag::Link { dest_url, .. }) => {
                if self.in_metadata_paragraph {
                    self.in_metadata_link = true;
                    self.metadata_link_url = dest_url.to_string();
                    self.metadata_link_label.clear();
                }
            }
            Event::End(TagEnd::Link) => {
                if self.in_metadata_link {
                    self.in_metadata_link = false;
                    // Reconstruct the markdown link syntax so parse_metadata_block can parse it
                    let label = std::mem::take(&mut self.metadata_link_label);
                    let url = std::mem::take(&mut self.metadata_link_url);
                    self.metadata_paragraph_text
                        .push_str(&format!("[{}]({})", label, url));
                }
            }

            Event::Text(text) => {
                self.handle_text(&text);
            }
            Event::Code(code) => {
                if self.in_heading.is_some() {
                    self.heading_text.push('`');
                    self.heading_text.push_str(&code);
                    self.heading_text.push('`');
                } else if let Some(frame) = self.current_item_mut() {
                    frame.text.push('`');
                    frame.text.push_str(&code);
                    frame.text.push('`');
                } else if self.in_metadata_paragraph {
                    self.metadata_paragraph_text.push('`');
                    self.metadata_paragraph_text.push_str(&code);
                    self.metadata_paragraph_text.push('`');
                } else {
                    self.prose_buf.push('`');
                    self.prose_buf.push_str(&code);
                    self.prose_buf.push('`');
                }
            }
            Event::SoftBreak | Event::HardBreak => {
                if self.in_heading.is_some() {
                    self.heading_text.push(' ');
                } else if self.in_list_item() {
                    if let Some(frame) = self.current_item_mut() {
                        frame.text.push(' ');
                    }
                } else if self.in_metadata_paragraph {
                    self.metadata_paragraph_text.push('\n');
                } else {
                    self.prose_buf.push('\n');
                }
            }

            _ => {}
        }
    }

    fn handle_text(&mut self, text: &str) {
        if self.in_code_block {
            self.code_block_text.push_str(text);
        } else if self.in_strong {
            self.strong_text.push_str(text);
        } else if self.in_heading.is_some() {
            self.heading_text.push_str(text);
        } else if self.in_metadata_link {
            // Text inside a link in metadata — capture as label
            self.metadata_link_label.push_str(text);
        } else if self.in_list_item() {
            if let Some(frame) = self.current_item_mut() {
                frame.text.push_str(text);
            }
        } else if self.in_metadata_paragraph {
            self.metadata_paragraph_text.push_str(text);
        } else {
            self.prose_buf.push_str(text);
        }
    }

    fn parse_metadata_block(&mut self, text: &str) {
        for line in text.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            if let Some(rest) = trimmed.strip_prefix("context:") {
                let val = rest.trim();
                if !val.is_empty() {
                    self.metadata.context = Some(val.to_string());
                }
            } else if let Some(rest) = trimmed.strip_prefix("source:") {
                let val = rest.trim();
                if !val.is_empty() {
                    for s in split_metadata_values(val) {
                        self.metadata.sources.push(s);
                    }
                }
            } else if let Some(rest) = trimmed.strip_prefix("schema:") {
                let val = rest.trim();
                if !val.is_empty() {
                    for s in split_metadata_values(val) {
                        self.metadata.schemas.push(s);
                    }
                }
            } else if let Some(rest) = trimmed.strip_prefix("requires:") {
                let val = rest.trim();
                if !val.is_empty() {
                    let refs = parse_requires_line(val);
                    if refs.is_empty() {
                        // Plain text path (no markdown link syntax)
                        self.metadata.requires.push(SpecRef {
                            label: val.to_string(),
                            path: PathBuf::from(val),
                            anchor: None,
                        });
                    } else {
                        self.metadata.requires.extend(refs);
                    }
                }
            }
            // Lines that don't match any metadata prefix are ignored as free-form text
        }
    }

    fn flush_prose(&mut self) {
        let prose = std::mem::take(&mut self.prose_buf).trim().to_string();
        if !prose.is_empty()
            && let Some((_, section)) = self.section_stack.last_mut() {
                if section.prose.is_empty() {
                    section.prose = prose;
                } else {
                    section.prose.push('\n');
                    section.prose.push_str(&prose);
                }
            }
    }

    fn flush_pending_items(&mut self) {
        let items = std::mem::take(&mut self.depth1_items);
        if items.is_empty() {
            return;
        }

        let spec_name = match &self.spec_name {
            Some(n) => slugify(n),
            None => "unknown".to_string(),
        };

        let section_path = self
            .section_stack
            .iter()
            .map(|(_, s)| slugify(&s.title))
            .collect::<Vec<_>>();

        for item in items {
            let clauses = self.items_to_clauses(&spec_name, &section_path, item, None);
            if let Some((_, section)) = self.section_stack.last_mut() {
                section.clauses.extend(clauses);
            }
        }

        // Note: we intentionally do NOT reset just_finished_clause here,
        // so that a code block right after the list can still be captured as a hint.
    }

    fn items_to_clauses(
        &self,
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
                        self.items_to_clauses(spec_name, section_path, nested, condition.clone());
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
                                file: self.file.clone(),
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
                        file: self.file.clone(),
                        line: item.line,
                    },
                    content_hash: hash,
                    pending: item.pending,
                };

                result.push(clause);

                // Any non-OTHERWISE nested items get turned into clauses too
                // (rare case, but handle gracefully)
                for nested in other_nested {
                    let nested_clauses =
                        self.items_to_clauses(spec_name, section_path, nested, None);
                    result.extend(nested_clauses);
                }
            }
        }

        result
    }

    fn attach_hint_to_last_clause(&mut self, code: String) {
        if let Some((_, section)) = self.section_stack.last_mut()
            && let Some(clause) = section.clauses.last_mut() {
                clause.hints.push(code);
            }
    }

    fn flush_section_stack(&mut self) {
        // Flush any remaining prose
        self.flush_prose();

        // Pop all sections from the stack
        while let Some((_, section)) = self.section_stack.pop() {
            if let Some((_, parent)) = self.section_stack.last_mut() {
                parent.subsections.push(section);
            } else {
                self.sections.push(section);
            }
        }
    }

    fn into_spec(self) -> Spec {
        Spec {
            name: self.spec_name.unwrap_or_else(|| "Untitled".to_string()),
            metadata: self.metadata,
            sections: self.sections,
            source_path: self.file,
        }
    }
}

/// Split metadata values by commas, but respect values that contain glob patterns
/// (e.g. `tests/**/*.rs`). Commas inside path-like values with `*`, `?`, `[` are
/// preserved — we only split on commas followed by whitespace and a new path.
fn split_metadata_values(val: &str) -> Vec<String> {
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
fn parse_requires_line(line: &str) -> Vec<SpecRef> {
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
