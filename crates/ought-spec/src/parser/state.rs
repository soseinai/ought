//! The `.ought.md` parse state machine.
//!
//! Drives pulldown-cmark event iteration and mutates [`ParseState`]
//! — which tracks the current heading/list/code-block/metadata context —
//! to build up a [`Spec`] tree. The pure helpers this module relies on
//! live in sibling modules: [`super::ids`], [`super::keywords`],
//! [`super::metadata`].

use std::path::{Path, PathBuf};
use std::time::Duration;

use pulldown_cmark::{Event, HeadingLevel, Options, Parser as MdParser, Tag, TagEnd};

use crate::types::{Keyword, Metadata, ParseError, Section, Spec, SpecRef, Temporal};

use super::clauses::build_clauses;
use super::ids::{line_number_at_offset, slugify};
use super::keywords::{parse_keyword, ParsedKeyword};
use super::metadata::{parse_requires_line, split_metadata_values};

/// Entry point for parsing a spec from an in-memory string. The path is used
/// only for source-location labels in errors and `SourceLocation`s.
pub(super) fn parse_string(content: &str, path: &Path) -> Result<Spec, Vec<ParseError>> {
    let mut state = ParseState::new(path.to_path_buf(), content);
    state.parse();

    if state.errors.is_empty() {
        Ok(state.into_spec())
    } else {
        // When there are errors we surface them, even if a partial spec
        // was built — the signature returns Result, and callers rely on
        // Err to flag that at least some content could not be parsed.
        Err(state.errors)
    }
}

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

/// A clause-in-progress collected during list-item traversal. Promoted to
/// a public [`crate::types::Clause`] by [`super::clauses::build_clauses`]
/// once the enclosing list flushes. Visible to sibling submodules under
/// `parser/` but otherwise private to this crate.
#[derive(Debug, Clone)]
pub(super) struct PendingItem {
    pub(super) keyword: Keyword,
    pub(super) pending: bool,
    pub(super) text: String,
    pub(super) temporal: Option<Temporal>,
    pub(super) line: usize,
    pub(super) nested_items: Vec<PendingItem>,
    pub(super) hints: Vec<String>,
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
            let clauses = build_clauses(&self.file, &spec_name, &section_path, item, None);
            if let Some((_, section)) = self.section_stack.last_mut() {
                section.clauses.extend(clauses);
            }
        }

        // Note: we intentionally do NOT reset just_finished_clause here,
        // so that a code block right after the list can still be captured as a hint.
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
