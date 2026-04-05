//! Scan runner test directories for "proofs" — the generated tests that
//! implement each clause — and extract their source so the viewer can show
//! them alongside the spec.
//!
//! A clause id like `parser::keywords::must_recognize_rfc_2119_keywords_...`
//! maps to a test file at `<test_dir>/parser/keywords/must_recognize_rfc_2119_keywords_..._test.<ext>`.
//! Each file contains one or more test functions. For each function we
//! extract its name, the doc comment immediately preceding it (as a summary),
//! and the full source text (signature + body) as the proof's code.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use ought_spec::Config;

/// A single test function extracted from a proof file.
#[derive(Debug, Clone)]
pub struct Proof {
    /// Test function name, e.g. `test_parser__keywords__must_recognize_...`.
    pub name: String,
    /// One-line summary taken from the `///` doc comment above the test.
    pub summary: String,
    /// Full source text of the test — attribute(s), signature, and body.
    pub code: String,
    /// Language tag for syntax highlighting: `rust`, `python`, `typescript`, `go`.
    pub language: String,
}

/// Index mapping clause id → proofs + source file, built once at server start.
#[derive(Debug, Clone, Default)]
pub struct ProofIndex {
    /// Maps `ClauseId.0` → (relative file path, proofs).
    pub by_clause: HashMap<String, (PathBuf, Vec<Proof>)>,
}

impl ProofIndex {
    /// Build an index by walking every configured runner's `test_dir`.
    /// Missing or unreadable dirs are silently skipped — proofs are optional.
    pub fn build(config: &Config, config_dir: &Path) -> Self {
        let mut by_clause: HashMap<String, (PathBuf, Vec<Proof>)> = HashMap::new();

        for (runner_name, runner_cfg) in &config.runner {
            let language = runner_language(runner_name);
            let test_dir = config_dir.join(&runner_cfg.test_dir);
            if !test_dir.is_dir() {
                continue;
            }
            walk_test_dir(&test_dir, &test_dir, language, &mut by_clause);
        }

        Self { by_clause }
    }

    /// Number of clauses that have at least one proof.
    pub fn clause_count(&self) -> usize {
        self.by_clause.len()
    }

    /// Total number of proof functions across all clauses.
    pub fn proof_count(&self) -> usize {
        self.by_clause.values().map(|(_, p)| p.len()).sum()
    }
}

/// Map a runner key (from `[runner.<name>]`) to a highlight.js language tag.
fn runner_language(runner_name: &str) -> &'static str {
    match runner_name.to_lowercase().as_str() {
        "rust" => "rust",
        "python" | "py" => "python",
        "typescript" | "ts" | "javascript" | "js" => "typescript",
        "go" => "go",
        _ => "plaintext",
    }
}

/// File extension associated with each language.
fn language_extension(language: &str) -> &'static str {
    match language {
        "rust" => "rs",
        "python" => "py",
        "typescript" => "ts",
        "go" => "go",
        _ => "",
    }
}

/// Recursively walk a test directory, parsing every `*_test.<ext>` file.
fn walk_test_dir(
    root: &Path,
    dir: &Path,
    language: &str,
    out: &mut HashMap<String, (PathBuf, Vec<Proof>)>,
) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk_test_dir(root, &path, language, out);
            continue;
        }
        let ext = language_extension(language);
        if ext.is_empty() {
            continue;
        }
        let is_test_file = path
            .file_name()
            .and_then(|n| n.to_str())
            .map(|n| n.ends_with(&format!("_test.{}", ext)))
            .unwrap_or(false);
        if !is_test_file {
            continue;
        }

        let Some(clause_id) = clause_id_from_path(root, &path, ext) else {
            continue;
        };
        let Ok(source) = std::fs::read_to_string(&path) else {
            continue;
        };
        let proofs = extract_proofs(&source, language);
        if proofs.is_empty() {
            continue;
        }
        let rel = path.strip_prefix(root).unwrap_or(&path).to_path_buf();
        out.insert(clause_id, (rel, proofs));
    }
}

/// Convert a test-file path back into its clause id.
///
/// `<root>/parser/keywords/must_foo_test.rs` → `parser::keywords::must_foo`.
fn clause_id_from_path(root: &Path, file: &Path, ext: &str) -> Option<String> {
    let rel = file.strip_prefix(root).ok()?;
    let mut parts: Vec<String> = rel
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect();
    let last = parts.pop()?;
    let suffix = format!("_test.{}", ext);
    let stem = last.strip_suffix(&suffix)?;
    parts.push(stem.to_string());
    Some(parts.join("::"))
}

/// Extract proof functions from a source file. Currently supports Rust;
/// other languages fall back to treating the whole file as a single proof.
fn extract_proofs(source: &str, language: &str) -> Vec<Proof> {
    match language {
        "rust" => extract_rust_proofs(source),
        _ => vec![Proof {
            name: "test".to_string(),
            summary: first_nonblank_line(source).unwrap_or_default(),
            code: source.to_string(),
            language: language.to_string(),
        }],
    }
}

fn first_nonblank_line(s: &str) -> Option<String> {
    s.lines()
        .map(|l| l.trim())
        .find(|l| !l.is_empty())
        .map(|l| l.to_string())
}

/// Parse a Rust source file and return one Proof per `fn` annotated with
/// `#[test]`. The summary comes from the contiguous `///` doc-comment block
/// directly above the attribute(s). The code is everything from the first
/// attribute line through the function's closing brace.
fn extract_rust_proofs(source: &str) -> Vec<Proof> {
    let lines: Vec<&str> = source.lines().collect();
    let mut proofs = Vec::new();
    let mut i = 0;

    while i < lines.len() {
        let trimmed = lines[i].trim_start();
        if trimmed.starts_with("#[test]") || trimmed == "#[test]" {
            // Walk upward over attribute lines and doc comments to find the
            // start of this test "block" (inclusive of the first attribute).
            let mut block_start = i;
            while block_start > 0 {
                let prev = lines[block_start - 1].trim_start();
                if prev.starts_with("#[") || prev.starts_with("#![") {
                    block_start -= 1;
                } else {
                    break;
                }
            }

            // Collect doc comments above the attributes.
            let mut doc_lines: Vec<String> = Vec::new();
            let mut d = block_start;
            while d > 0 {
                let prev = lines[d - 1].trim_start();
                if let Some(rest) = prev.strip_prefix("///") {
                    doc_lines.push(rest.trim().to_string());
                    d -= 1;
                } else if prev.is_empty() {
                    break;
                } else {
                    break;
                }
            }
            doc_lines.reverse();
            let summary = doc_lines
                .iter()
                .find(|l| !l.is_empty())
                .cloned()
                .unwrap_or_default();

            // Find the `fn ...` line after the attributes.
            let mut fn_line = i + 1;
            while fn_line < lines.len() && !lines[fn_line].trim_start().starts_with("fn ") {
                fn_line += 1;
            }
            if fn_line >= lines.len() {
                i += 1;
                continue;
            }

            // Extract the function name from `fn name(...) ...`.
            let name = parse_fn_name(lines[fn_line]).unwrap_or_else(|| "test".to_string());

            // Walk braces to find the end of the function body.
            let Some(body_end) = find_body_end(&lines, fn_line) else {
                i = fn_line + 1;
                continue;
            };

            let code = lines[block_start..=body_end].join("\n");
            proofs.push(Proof {
                name,
                summary,
                code,
                language: "rust".to_string(),
            });

            i = body_end + 1;
            continue;
        }
        i += 1;
    }

    proofs
}

fn parse_fn_name(line: &str) -> Option<String> {
    let after = line.trim_start().strip_prefix("fn ")?;
    let end = after
        .find(|c: char| c == '(' || c == '<' || c.is_whitespace())
        .unwrap_or(after.len());
    Some(after[..end].to_string())
}

/// Given the index of a line containing `fn ... {`, return the index of the
/// line with the matching closing `}`. Handles braces inside strings and
/// `//` line comments conservatively.
fn find_body_end(lines: &[&str], fn_line: usize) -> Option<usize> {
    let mut depth: i32 = 0;
    let mut started = false;
    for (i, line) in lines.iter().enumerate().skip(fn_line) {
        let mut in_string = false;
        let mut in_char = false;
        let mut prev = ' ';
        let bytes = line.as_bytes();
        let mut j = 0;
        while j < bytes.len() {
            let c = bytes[j] as char;
            // Skip `//` line comments.
            if !in_string && !in_char && c == '/' && j + 1 < bytes.len() && bytes[j + 1] == b'/' {
                break;
            }
            if !in_char && c == '"' && prev != '\\' {
                in_string = !in_string;
            } else if !in_string && c == '\'' && prev != '\\' {
                in_char = !in_char;
            } else if !in_string && !in_char {
                if c == '{' {
                    depth += 1;
                    started = true;
                } else if c == '}' {
                    depth -= 1;
                    if started && depth == 0 {
                        return Some(i);
                    }
                }
            }
            prev = c;
            j += 1;
        }
    }
    None
}
