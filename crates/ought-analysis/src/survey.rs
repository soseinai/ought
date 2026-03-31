use std::collections::HashSet;
use std::path::PathBuf;

use ought_gen::Generator;
use ought_spec::{Keyword, Section, SpecGraph};

use crate::types::{SurveyResult, UncoveredBehavior};

/// Discover behaviors in source code not covered by any spec clause.
///
/// Reads source files, reads all specs, and compares public function/method
/// signatures against existing clause texts to find uncovered behaviors.
/// The LLM generator parameter is accepted for future enrichment but
/// structural analysis works without it.
pub fn survey(
    specs: &SpecGraph,
    paths: &[PathBuf],
    _generator: &dyn Generator,
) -> anyhow::Result<SurveyResult> {
    // 1. Collect all existing clause texts (lowercased) so we can check coverage.
    let mut covered_texts: HashSet<String> = HashSet::new();
    let mut spec_source_roots: Vec<PathBuf> = Vec::new();

    for spec in specs.specs() {
        collect_clause_texts(&spec.sections, &mut covered_texts);
        // Collect source roots from spec metadata for fallback path discovery.
        for src in &spec.metadata.sources {
            let base = spec
                .source_path
                .parent()
                .unwrap_or(std::path::Path::new("."));
            spec_source_roots.push(base.join(src));
        }
    }

    // 2. Determine which paths to scan.
    let scan_paths: Vec<PathBuf> = if paths.is_empty() {
        spec_source_roots
    } else {
        paths.to_vec()
    };

    // 3. Walk paths and read source files, extracting public function signatures.
    let mut uncovered: Vec<UncoveredBehavior> = Vec::new();

    for path in &scan_paths {
        if path.is_file() {
            if let Ok(content) = std::fs::read_to_string(path) {
                extract_uncovered_from_file(path, &content, &covered_texts, specs, &mut uncovered);
            }
        } else if path.is_dir() {
            walk_source_dir(path, &covered_texts, specs, &mut uncovered);
        }
    }

    // 4. Sort: public API behaviors first (MUST keyword), then helpers (SHOULD).
    uncovered.sort_by(|a, b| {
        let a_pub = a.description.contains("public") || a.suggested_keyword == Keyword::Must;
        let b_pub = b.description.contains("public") || b.suggested_keyword == Keyword::Must;
        b_pub.cmp(&a_pub)
    });

    // 5. Group by suggested_spec so that behaviors for the same file are adjacent.
    uncovered.sort_by(|a, b| a.suggested_spec.cmp(&b.suggested_spec));

    // Re-apply risk ranking within each group.
    let mut grouped: Vec<UncoveredBehavior> = Vec::new();
    let mut current_spec: Option<PathBuf> = None;
    let mut current_group: Vec<UncoveredBehavior> = Vec::new();

    for item in uncovered {
        if current_spec.as_ref() != Some(&item.suggested_spec) {
            // Flush previous group, sorted by risk.
            current_group.sort_by(|a, b| {
                let a_pub =
                    a.description.contains("public") || a.suggested_keyword == Keyword::Must;
                let b_pub =
                    b.description.contains("public") || b.suggested_keyword == Keyword::Must;
                b_pub.cmp(&a_pub)
            });
            grouped.append(&mut current_group);
            current_spec = Some(item.suggested_spec.clone());
        }
        current_group.push(item);
    }
    // Flush last group.
    current_group.sort_by(|a, b| {
        let a_pub = a.description.contains("public") || a.suggested_keyword == Keyword::Must;
        let b_pub = b.description.contains("public") || b.suggested_keyword == Keyword::Must;
        b_pub.cmp(&a_pub)
    });
    grouped.append(&mut current_group);

    Ok(SurveyResult {
        uncovered: grouped,
    })
}

/// Recursively collect clause texts from sections (lowercased for matching).
fn collect_clause_texts(sections: &[Section], texts: &mut HashSet<String>) {
    for section in sections {
        for clause in &section.clauses {
            texts.insert(clause.text.to_lowercase());
            // Also collect otherwise clause texts.
            for ow in &clause.otherwise {
                texts.insert(ow.text.to_lowercase());
            }
        }
        collect_clause_texts(&section.subsections, texts);
    }
}

/// Walk a directory recursively and process source files.
fn walk_source_dir(
    dir: &std::path::Path,
    covered_texts: &HashSet<String>,
    specs: &SpecGraph,
    uncovered: &mut Vec<UncoveredBehavior>,
) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            // Skip common non-source directories.
            if let Some(name) = path.file_name().and_then(|n| n.to_str())
                && matches!(
                    name,
                    "target"
                        | "node_modules"
                        | ".git"
                        | "__pycache__"
                        | "vendor"
                        | ".venv"
                        | "venv"
                ) {
                    continue;
                }
            walk_source_dir(&path, covered_texts, specs, uncovered);
        } else if is_source_file(&path)
            && let Ok(content) = std::fs::read_to_string(&path) {
                extract_uncovered_from_file(&path, &content, covered_texts, specs, uncovered);
            }
    }
}

/// Check if a file looks like source code.
fn is_source_file(path: &std::path::Path) -> bool {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    matches!(
        ext,
        "rs" | "py" | "ts" | "tsx" | "js" | "jsx" | "go" | "java" | "rb" | "kt" | "swift" | "c"
            | "cpp" | "h" | "hpp"
    )
}

/// Extract public function signatures from a source file and check coverage.
fn extract_uncovered_from_file(
    file: &std::path::Path,
    content: &str,
    covered_texts: &HashSet<String>,
    specs: &SpecGraph,
    uncovered: &mut Vec<UncoveredBehavior>,
) {
    let suggested_spec = infer_spec_path(file, specs);

    for (line_num_0, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        if let Some(fn_name) = extract_public_fn_name(trimmed) {
            // Check if any clause text mentions this function name.
            let fn_lower = fn_name.to_lowercase();
            let fn_words = fn_lower.replace('_', " ");
            let is_covered = covered_texts
                .iter()
                .any(|text| text.contains(&fn_lower) || text.contains(&fn_words));

            if !is_covered {
                let is_public = trimmed.starts_with("pub ")
                    || trimmed.starts_with("export ")
                    || trimmed.starts_with("def ")
                    || trimmed.starts_with("func ");
                let (keyword, desc_prefix) = if is_public {
                    (Keyword::Must, "public")
                } else {
                    (Keyword::Should, "private")
                };

                uncovered.push(UncoveredBehavior {
                    file: file.to_path_buf(),
                    line: line_num_0 + 1,
                    description: format!(
                        "{} function `{}` has no corresponding spec clause",
                        desc_prefix, fn_name
                    ),
                    suggested_clause: format!(
                        "{} handle {} correctly",
                        if keyword == Keyword::Must {
                            "MUST"
                        } else {
                            "SHOULD"
                        },
                        fn_name.replace('_', " ")
                    ),
                    suggested_keyword: keyword,
                    suggested_spec: suggested_spec.clone(),
                });
            }
        }
    }
}

/// Extract a function name from a line if it declares a public function/method.
fn extract_public_fn_name(line: &str) -> Option<String> {
    // Rust: pub fn name(...) or pub async fn name(...)
    if let Some(rest) = line
        .strip_prefix("pub fn ")
        .or_else(|| line.strip_prefix("pub async fn "))
        .or_else(|| line.strip_prefix("pub(crate) fn "))
        .or_else(|| line.strip_prefix("pub(super) fn "))
    {
        return extract_ident(rest);
    }
    // Also match `fn ` for private Rust functions (but we'll mark them differently).
    if line.starts_with("fn ")
        && let Some(rest) = line.strip_prefix("fn ") {
            return extract_ident(rest);
        }
    // Python: def name(
    if let Some(rest) = line.strip_prefix("def ") {
        let name = extract_ident(rest);
        // Skip dunder methods
        if let Some(ref n) = name
            && n.starts_with("__") && n.ends_with("__") {
                return None;
            }
        return name;
    }
    // TypeScript/JavaScript: export function name, function name, export const name
    if let Some(rest) = line
        .strip_prefix("export function ")
        .or_else(|| line.strip_prefix("export async function "))
        .or_else(|| line.strip_prefix("function "))
        .or_else(|| line.strip_prefix("async function "))
    {
        return extract_ident(rest);
    }
    // Go: func name(
    if let Some(rest) = line.strip_prefix("func ") {
        // Skip method receivers: func (r *Type) Name(...)
        if rest.starts_with('(') {
            // Method: find closing paren then extract name
            if let Some(idx) = rest.find(')') {
                let after = rest[idx + 1..].trim();
                return extract_ident(after);
            }
            return None;
        }
        return extract_ident(rest);
    }
    None
}

/// Extract an identifier (letters, digits, underscores) from the start of a string.
fn extract_ident(s: &str) -> Option<String> {
    let s = s.trim();
    let end = s
        .find(|c: char| !c.is_alphanumeric() && c != '_')
        .unwrap_or(s.len());
    if end == 0 {
        return None;
    }
    Some(s[..end].to_string())
}

/// Infer which spec file a source file would belong to.
fn infer_spec_path(source_file: &std::path::Path, specs: &SpecGraph) -> PathBuf {
    // Try to match by source metadata in specs.
    let source_str = source_file.to_string_lossy();
    for spec in specs.specs() {
        for src in &spec.metadata.sources {
            if source_str.contains(src) || source_str.contains(&src.replace("./", "")) {
                return spec.source_path.clone();
            }
        }
    }
    // Fallback: derive from the source file name.
    let stem = source_file
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown");
    PathBuf::from(format!("ought/{}.ought.md", stem))
}
