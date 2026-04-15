//! Tool primitives for the test generator.
//!
//! These are the operations an LLM agent invokes during test generation:
//! reading the assignment, reading and listing source files, writing test
//! files, and compile-checking what was written. They are plain functions
//! over plain Rust types — no MCP, no JSON-RPC, no subprocess plumbing —
//! so they can be used both by ought's in-process agent loop and by the
//! MCP server (which thin-wraps them as JSON-RPC handlers).
//!
//! The semantics here are load-bearing. The agent prompts assume:
//!
//! * `read_source` rejects paths outside the project root.
//! * `write_test` derives a per-subsection file path from the clause id
//!   and merges the new test in place if a function with the same name
//!   already exists.
//! * `write_test` updates the manifest and persists it to disk
//!   immediately, so a Ctrl-C never leaves the manifest out of sync with
//!   the test files on disk.
//! * `check_compiles` invokes the language-appropriate compiler.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use crate::agent::AgentAssignment;
use crate::manifest::{Manifest, ManifestEntry};

// ── Output types ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadSourceOutput {
    pub path: String,
    pub content: String,
    /// 1-based line of the first line returned. `None` when reading from
    /// the start of the file (the common case).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_line: Option<usize>,
    /// 1-based line of the last line returned. `None` when reading to
    /// EOF and not truncated.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_line: Option<usize>,
    /// True when the requested read was clipped at the size limit. The
    /// agent should call again with a `start_line`/`end_line` range to
    /// see the rest if it cares.
    #[serde(default, skip_serializing_if = "is_false")]
    pub truncated: bool,
    /// Size of the underlying file in bytes. Helps the agent decide how
    /// to chunk subsequent reads.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_bytes: Option<u64>,
}

fn is_false(b: &bool) -> bool {
    !b
}

/// Default per-call cap on the bytes returned by `read_source`. Calibrated
/// at ~16K Anthropic tokens, which is large enough for nearly every source
/// file in a typical Rust/Python/TS project but small enough that an agent
/// reading 5+ files won't blow past a 200K context window.
pub const DEFAULT_READ_SOURCE_LIMIT_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListSourceFilesOutput {
    pub pattern: String,
    pub files: Vec<String>,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WriteTestOutput {
    pub clause_id: String,
    pub file_path: String,
    pub status: WriteStatus,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WriteStatus {
    Written,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WriteTestsBatchOutput {
    pub results: Vec<WriteTestResult>,
    pub total: usize,
}

/// Outcome of writing a single test inside a batch.
///
/// Serialized untagged so the MCP JSON shape matches today's API:
/// successful writes carry `status: "written"`, failures carry
/// `status: "error"` and an `error` field.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum WriteTestResult {
    Ok {
        clause_id: String,
        file_path: String,
        status: WriteStatus,
    },
    Err {
        clause_id: String,
        status: ErrorStatus,
        error: String,
    },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ErrorStatus {
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckCompilesOutput {
    pub results: Vec<CompileResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum CompileResult {
    Ok { clause_id: String },
    Error { clause_id: String, error: String },
    Missing { clause_id: String, error: String },
}

// ── Tool primitives ─────────────────────────────────────────────────────

/// Return the agent's assignment.
///
/// Trivial today, but kept as a primitive so the MCP layer and the
/// in-process loop go through the same accessor.
pub fn get_assignment(assignment: &AgentAssignment) -> AgentAssignment {
    assignment.clone()
}

/// Read a source file relative to the project root, with the default
/// size cap and no range restriction.
///
/// Convenience wrapper over [`read_source_with`] for the common case.
pub fn read_source(project_root: &Path, path: &str) -> anyhow::Result<ReadSourceOutput> {
    read_source_with(project_root, path, None, None, DEFAULT_READ_SOURCE_LIMIT_BYTES)
}

/// Read a source file with optional 1-based line range and an explicit
/// size cap.
///
/// Returns whatever fits in `max_bytes`, marking `truncated: true` if
/// the read was clipped. The agent recovers by calling again with a
/// narrower line range.
pub fn read_source_with(
    project_root: &Path,
    path: &str,
    start_line: Option<usize>,
    end_line: Option<usize>,
    max_bytes: usize,
) -> anyhow::Result<ReadSourceOutput> {
    let resolved = project_root.join(path);
    let canonical_root = project_root
        .canonicalize()
        .unwrap_or_else(|_| project_root.to_path_buf());
    let canonical_path = resolved
        .canonicalize()
        .map_err(|e| anyhow::anyhow!("cannot resolve path '{}': {}", path, e))?;
    if !canonical_path.starts_with(&canonical_root) {
        anyhow::bail!("path '{}' is outside the project root", path);
    }

    let total_bytes = std::fs::metadata(&canonical_path).ok().map(|m| m.len());
    let raw = std::fs::read_to_string(&canonical_path)
        .map_err(|e| anyhow::anyhow!("failed to read '{}': {}", path, e))?;

    let lines: Vec<&str> = raw.lines().collect();
    let total_lines = lines.len();

    let start = start_line.map(|s| s.saturating_sub(1)).unwrap_or(0);
    let end_exclusive = end_line.map(|e| e.min(total_lines)).unwrap_or(total_lines);
    if start >= end_exclusive && total_lines > 0 {
        anyhow::bail!(
            "empty range: start_line {:?} > end_line {:?} (file has {} lines)",
            start_line,
            end_line,
            total_lines
        );
    }

    // Build the requested slice with line endings preserved.
    let mut content = String::new();
    let mut last_line_returned = start;
    let mut truncated = false;
    for (i, line) in lines[start..end_exclusive].iter().enumerate() {
        // +1 for the newline we'll re-add below.
        if content.len() + line.len() + 1 > max_bytes {
            truncated = true;
            break;
        }
        content.push_str(line);
        content.push('\n');
        last_line_returned = start + i;
    }

    // Edge case: even the first requested line overruns max_bytes.
    if content.is_empty() && !lines.is_empty() && start < end_exclusive {
        truncated = true;
        let first = lines[start];
        // Byte-index slicing must land on a UTF-8 char boundary or &str
        // slicing panics. Walk back from the requested cut if needed.
        let mut take = first.len().min(max_bytes.saturating_sub(1));
        while take > 0 && !first.is_char_boundary(take) {
            take -= 1;
        }
        content.push_str(&first[..take]);
        content.push('\n');
        last_line_returned = start;
    }

    let returned_start = if start_line.is_some() { Some(start + 1) } else { None };
    let returned_end = if start_line.is_some() || truncated {
        Some(last_line_returned + 1)
    } else {
        None
    };

    Ok(ReadSourceOutput {
        path: path.to_string(),
        content,
        start_line: returned_start,
        end_line: returned_end,
        truncated,
        total_bytes,
    })
}

/// List source files matching a simple glob pattern.
pub fn list_source_files(project_root: &Path, pattern: &str) -> ListSourceFilesOutput {
    let mut files = Vec::new();
    collect_files_matching(project_root, pattern, &mut files);
    let relative: Vec<String> = files
        .iter()
        .filter_map(|p| {
            p.strip_prefix(project_root)
                .ok()
                .map(|r| r.to_string_lossy().to_string())
        })
        .collect();
    let count = relative.len();
    ListSourceFilesOutput {
        pattern: pattern.to_string(),
        files: relative,
        count,
    }
}

/// Write a single test file for a clause and update the manifest.
pub fn write_test(
    assignment: &AgentAssignment,
    manifest: &Mutex<Manifest>,
    manifest_path: &Path,
    clause_id: &str,
    code: &str,
) -> anyhow::Result<WriteTestOutput> {
    let file_path = write_test_file(assignment, manifest, manifest_path, clause_id, code)?;
    Ok(WriteTestOutput {
        clause_id: clause_id.to_string(),
        file_path: file_path.to_string_lossy().to_string(),
        status: WriteStatus::Written,
    })
}

/// Write multiple test files, returning a per-test outcome for each.
///
/// Failures of individual writes are reported in the result vector; the
/// batch as a whole always succeeds.
pub fn write_tests_batch<I, S1, S2>(
    assignment: &AgentAssignment,
    manifest: &Mutex<Manifest>,
    manifest_path: &Path,
    tests: I,
) -> WriteTestsBatchOutput
where
    I: IntoIterator<Item = (S1, S2)>,
    S1: AsRef<str>,
    S2: AsRef<str>,
{
    let mut results = Vec::new();
    for (clause_id, code) in tests {
        let clause_id = clause_id.as_ref();
        let code = code.as_ref();
        match write_test_file(assignment, manifest, manifest_path, clause_id, code) {
            Ok(file_path) => results.push(WriteTestResult::Ok {
                clause_id: clause_id.to_string(),
                file_path: file_path.to_string_lossy().to_string(),
                status: WriteStatus::Written,
            }),
            Err(e) => results.push(WriteTestResult::Err {
                clause_id: clause_id.to_string(),
                status: ErrorStatus::Error,
                error: e.to_string(),
            }),
        }
    }
    let total = results.len();
    WriteTestsBatchOutput { results, total }
}

/// Compile-check the test files for the given clause ids.
pub fn check_compiles<I, S>(test_dir: &Path, lang: &str, clause_ids: I) -> CheckCompilesOutput
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut results = Vec::new();
    for clause_id in clause_ids {
        let clause_id = clause_id.as_ref();
        if clause_id.is_empty() {
            continue;
        }
        let file_path = derive_test_file_path(test_dir, clause_id, lang);
        if !file_path.exists() {
            results.push(CompileResult::Missing {
                clause_id: clause_id.to_string(),
                error: format!("file not found: {}", file_path.display()),
            });
            continue;
        }
        match check_file_compiles(&file_path, lang) {
            Ok(()) => results.push(CompileResult::Ok {
                clause_id: clause_id.to_string(),
            }),
            Err(e) => results.push(CompileResult::Error {
                clause_id: clause_id.to_string(),
                error: e,
            }),
        }
    }
    CheckCompilesOutput { results }
}

/// Emit a one-line progress update to stderr.
pub fn report_progress(
    assignment_id: &str,
    status: &str,
    message: &str,
    clauses_completed: u64,
    clauses_total: u64,
) {
    eprintln!(
        "  [agent {}] {}: {} ({}/{})",
        assignment_id, status, message, clauses_completed, clauses_total
    );
}

// ── Internal: write_test ────────────────────────────────────────────────

fn write_test_file(
    assignment: &AgentAssignment,
    manifest: &Mutex<Manifest>,
    manifest_path: &Path,
    clause_id: &str,
    code: &str,
) -> anyhow::Result<PathBuf> {
    let test_dir = Path::new(&assignment.test_dir);
    let lang = assignment.target_language.as_str();
    let file_path = derive_test_file_path(test_dir, clause_id, lang);

    if let Some(parent) = file_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let merged = if file_path.exists() {
        let existing = std::fs::read_to_string(&file_path)
            .map_err(|e| anyhow::anyhow!("failed to read {}: {}", file_path.display(), e))?;
        merge_test_block(&existing, code, lang)
    } else {
        format!("{}\n{}\n", default_file_header(lang), code.trim_end())
    };

    std::fs::write(&file_path, merged)
        .map_err(|e| anyhow::anyhow!("failed to write test file {}: {}", file_path.display(), e))?;

    let content_hash = find_content_hash(assignment, clause_id);

    {
        let mut m = manifest.lock().unwrap();
        m.entries.insert(
            clause_id.to_string(),
            ManifestEntry {
                clause_hash: content_hash,
                source_hash: String::new(),
                generated_at: chrono::Utc::now(),
                model: "agent".to_string(),
            },
        );
        // Save manifest to disk immediately (Ctrl-C safe).
        m.save(manifest_path)?;
    }

    Ok(file_path)
}

fn find_content_hash(assignment: &AgentAssignment, clause_id: &str) -> String {
    for group in &assignment.groups {
        for clause in &group.clauses {
            if clause.id == clause_id {
                return clause.content_hash.clone();
            }
            for ow in &clause.otherwise {
                if ow.id == clause_id {
                    return ow.content_hash.clone();
                }
            }
        }
    }
    String::new()
}

// ── Internal: file path derivation ──────────────────────────────────────

/// Derive the test file path from a clause id and target language.
///
/// Per-subsection layout: a clause id like `parser::clause_ir::must_foo`
/// lands in `<test_dir>/src/parser/clause_ir_test.rs` (Rust), with one
/// file shared by all clauses in the same subsection. Other languages
/// follow the same shape minus the leading `src/`.
pub fn derive_test_file_path(test_dir: &Path, clause_id: &str, lang: &str) -> PathBuf {
    let ext = match lang {
        "rust" => "_test.rs",
        "python" => "_test.py",
        "typescript" => ".test.ts",
        "javascript" => ".test.js",
        "go" => "_test.go",
        _ => "_test.rs",
    };

    let segments: Vec<&str> = clause_id.split("::").collect();
    let (dir_segs, file_stem): (&[&str], &str) = match segments.as_slice() {
        [] => (&[], "unknown"),
        [one] => (&[][..], *one),
        rest => {
            let len = rest.len();
            (&rest[..len - 2], rest[len - 2])
        }
    };

    let mut path = test_dir.to_path_buf();
    if lang == "rust" {
        path.push("src");
    }
    for seg in dir_segs {
        path.push(seg);
    }
    path.push(format!("{}{}", file_stem, ext));
    path
}

fn default_file_header(lang: &str) -> &'static str {
    match lang {
        "rust" => "#![allow(dead_code, clippy::all, non_snake_case, unused_imports)]\n",
        _ => "",
    }
}

// ── Internal: test-block extraction & merging ──────────────────────────

fn extract_test_fn_name(code: &str) -> Option<String> {
    let mut saw_test_attr = false;
    for line in code.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("#[test]") {
            saw_test_attr = true;
            continue;
        }
        if saw_test_attr && let Some(rest) = trimmed.strip_prefix("fn ") {
            let end = rest.find(['(', '<']).unwrap_or(rest.len());
            return Some(rest[..end].trim().to_string());
        }
    }
    None
}

fn find_test_block(content: &str, fn_name: &str) -> Option<(usize, usize)> {
    let needle = format!("fn {}(", fn_name);
    let fn_idx = content.find(&needle)?;

    let prefix = &content[..fn_idx];
    let mut start = fn_idx;
    for (line_start, line) in prefix
        .rmatch_indices('\n')
        .map(|(i, _)| i + 1)
        .zip(prefix.lines().rev())
    {
        let trimmed = line.trim_start();
        if trimmed.starts_with("#[") || trimmed.starts_with("///") || trimmed.is_empty() {
            start = line_start;
        } else {
            break;
        }
    }

    let bytes = content.as_bytes();
    let mut depth = 0i32;
    let mut i = fn_idx;
    let mut started = false;
    while i < bytes.len() {
        match bytes[i] {
            b'{' => {
                depth += 1;
                started = true;
            }
            b'}' => {
                depth -= 1;
                if started && depth == 0 {
                    let mut end = i + 1;
                    if end < bytes.len() && bytes[end] == b'\n' {
                        end += 1;
                    }
                    return Some((start, end));
                }
            }
            _ => {}
        }
        i += 1;
    }
    None
}

fn merge_test_block(existing: &str, new_block: &str, lang: &str) -> String {
    if lang != "rust" {
        let mut out = existing.trim_end().to_string();
        out.push_str("\n\n");
        out.push_str(new_block.trim());
        out.push('\n');
        return out;
    }

    let Some(new_fn_name) = extract_test_fn_name(new_block) else {
        let mut out = existing.trim_end().to_string();
        out.push_str("\n\n");
        out.push_str(new_block.trim());
        out.push('\n');
        return out;
    };

    if let Some((start, end)) = find_test_block(existing, &new_fn_name) {
        let mut out = String::with_capacity(existing.len() + new_block.len());
        out.push_str(&existing[..start]);
        out.push_str(new_block.trim());
        out.push('\n');
        out.push_str(&existing[end..]);
        return out;
    }

    let mut out = existing.trim_end().to_string();
    out.push_str("\n\n");
    out.push_str(new_block.trim());
    out.push('\n');
    out
}

// ── Internal: compile checking ──────────────────────────────────────────

fn check_file_compiles(file_path: &Path, lang: &str) -> Result<(), String> {
    use std::process::Command;

    let output = match lang {
        "rust" => Command::new("rustc")
            .args(["--edition", "2021", "--crate-type", "lib", "--out-dir"])
            .arg(std::env::temp_dir())
            .arg(file_path)
            .output(),
        "python" => Command::new("python")
            .args(["-m", "py_compile"])
            .arg(file_path)
            .output(),
        "typescript" => Command::new("npx")
            .args(["tsc", "--noEmit"])
            .arg(file_path)
            .output(),
        "javascript" => Command::new("node")
            .args(["--check"])
            .arg(file_path)
            .output(),
        "go" => Command::new("go").args(["vet"]).arg(file_path).output(),
        _ => return Err(format!("unsupported language for compile check: {}", lang)),
    };

    match output {
        Ok(out) => {
            if out.status.success() {
                Ok(())
            } else {
                let stderr = String::from_utf8_lossy(&out.stderr);
                let stdout = String::from_utf8_lossy(&out.stdout);
                let detail = if stderr.trim().is_empty() {
                    stdout.trim().to_string()
                } else {
                    stderr.trim().to_string()
                };
                Err(detail)
            }
        }
        Err(e) => Err(format!("failed to run compile check: {}", e)),
    }
}

// ── Internal: glob walking ──────────────────────────────────────────────

fn collect_files_matching(root: &Path, pattern: &str, results: &mut Vec<PathBuf>) {
    collect_files_recursive(root, &mut |path| {
        let rel = path.strip_prefix(root).unwrap_or(path);
        let rel_str = rel.to_string_lossy();
        if simple_glob_match(pattern, &rel_str) {
            results.push(path.to_path_buf());
        }
    });
}

fn simple_glob_match(pattern: &str, path: &str) -> bool {
    if let Some(ext_pattern) = pattern.strip_prefix("**/") {
        if ext_pattern.starts_with('*') && let Some(ext) = ext_pattern.strip_prefix('*') {
            return path.ends_with(ext);
        }
        return path.ends_with(ext_pattern);
    }

    if pattern.starts_with('*')
        && let Some(ext) = pattern.strip_prefix('*')
    {
        return path.ends_with(ext) && !path.contains('/');
    }

    if let Some((prefix, suffix)) = pattern.split_once("**") {
        let suffix = suffix.strip_prefix('/').unwrap_or(suffix);
        if let Some(rest) = path.strip_prefix(prefix) {
            if suffix.starts_with('*') && let Some(ext) = suffix.strip_prefix('*') {
                return rest.ends_with(ext);
            }
            return rest.ends_with(suffix);
        }
        return false;
    }

    path == pattern
}

fn collect_files_recursive(dir: &Path, callback: &mut dyn FnMut(&Path)) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

        if name.starts_with('.') {
            continue;
        }
        if name == "target" || name == "node_modules" || name == "__pycache__" {
            continue;
        }

        if path.is_dir() {
            collect_files_recursive(&path, callback);
        } else if path.is_file() {
            callback(&path);
        }
    }
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derive_test_file_path_rust_three_segments() {
        let p = derive_test_file_path(Path::new("/tmp/td"), "parser::clause_ir::must_foo", "rust");
        assert_eq!(p, PathBuf::from("/tmp/td/src/parser/clause_ir_test.rs"));
    }

    #[test]
    fn derive_test_file_path_python_three_segments() {
        let p = derive_test_file_path(Path::new("/tmp/td"), "parser::clause_ir::must_foo", "python");
        assert_eq!(p, PathBuf::from("/tmp/td/parser/clause_ir_test.py"));
    }

    #[test]
    fn derive_test_file_path_two_segments() {
        let p = derive_test_file_path(Path::new("/tmp/td"), "auth::must_login", "rust");
        assert_eq!(p, PathBuf::from("/tmp/td/src/auth_test.rs"));
    }

    #[test]
    fn extract_fn_name_finds_test() {
        let code = "/// doc\n#[test]\nfn test_foo__bar__baz() {\n    assert!(true);\n}\n";
        assert_eq!(extract_test_fn_name(code).as_deref(), Some("test_foo__bar__baz"));
    }

    #[test]
    fn merge_replaces_existing_rust_test() {
        let existing = "#![allow(dead_code)]\n\n#[test]\nfn test_a() {\n    assert!(false);\n}\n";
        let new_block = "#[test]\nfn test_a() {\n    assert!(true);\n}\n";
        let merged = merge_test_block(existing, new_block, "rust");
        assert!(merged.contains("assert!(true)"));
        assert!(!merged.contains("assert!(false)"));
    }

    #[test]
    fn merge_appends_new_rust_test() {
        let existing = "#![allow(dead_code)]\n\n#[test]\nfn test_a() {\n    assert!(true);\n}\n";
        let new_block = "#[test]\nfn test_b() {\n    assert!(true);\n}\n";
        let merged = merge_test_block(existing, new_block, "rust");
        assert!(merged.contains("fn test_a"));
        assert!(merged.contains("fn test_b"));
    }

    #[test]
    fn read_source_blocks_path_traversal() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        // create one file inside root so `read_source` can canonicalise
        std::fs::write(root.join("inside.txt"), "ok").unwrap();
        // and create an outside file at the parent level
        let outside = root.parent().unwrap().join("outside.txt");
        std::fs::write(&outside, "secret").unwrap();
        let result = read_source(root, "../outside.txt");
        assert!(result.is_err(), "expected traversal block, got {:?}", result);
        let _ = std::fs::remove_file(&outside);
    }

    #[test]
    fn read_source_returns_full_file_when_under_limit() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        std::fs::write(root.join("small.rs"), "fn a() {}\nfn b() {}\n").unwrap();
        let out = read_source(root, "small.rs").unwrap();
        assert!(out.content.contains("fn a"));
        assert!(out.content.contains("fn b"));
        assert!(!out.truncated);
        assert_eq!(out.start_line, None);
        assert_eq!(out.end_line, None);
    }

    #[test]
    fn read_source_truncates_at_byte_limit() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        // 100 lines of "X" * 100 = ~10 KB
        let big: String = (0..100)
            .map(|i| format!("line_{}_{}\n", i, "X".repeat(100)))
            .collect();
        std::fs::write(root.join("big.txt"), &big).unwrap();
        let out = read_source_with(root, "big.txt", None, None, 1024).unwrap();
        assert!(out.truncated);
        assert!(out.content.len() <= 1024);
        assert!(out.end_line.unwrap() < 100); // didn't reach end
        assert!(out.total_bytes.unwrap() > 1024);
    }

    #[test]
    fn read_source_with_line_range_reads_requested_window() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let body: String = (1..=20).map(|i| format!("line {}\n", i)).collect();
        std::fs::write(root.join("ranged.txt"), &body).unwrap();
        let out = read_source_with(root, "ranged.txt", Some(5), Some(7), 4096).unwrap();
        assert_eq!(out.content, "line 5\nline 6\nline 7\n");
        assert_eq!(out.start_line, Some(5));
        assert_eq!(out.end_line, Some(7));
        assert!(!out.truncated);
    }

    #[test]
    fn read_source_truncates_an_overlong_first_line() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        std::fs::write(root.join("long_line.txt"), "X".repeat(10_000)).unwrap();
        let out = read_source_with(root, "long_line.txt", None, None, 256).unwrap();
        assert!(out.truncated);
        assert!(out.content.len() <= 256);
    }

    #[test]
    fn read_source_truncation_respects_utf8_boundaries() {
        // Overlong first line with multi-byte characters at the byte cap.
        // Without boundary-aware truncation, slicing `&str` by raw bytes
        // panics when the cut lands mid-char.
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        // "日" is 3 bytes in UTF-8. Repeat to force a large line whose
        // byte-length happens to land mid-character at arbitrary caps.
        let body: String = "日".repeat(200);
        std::fs::write(root.join("unicode.txt"), &body).unwrap();

        // Try several caps where naive byte-slicing would land mid-char.
        for cap in [50usize, 100, 200, 301, 302] {
            let out = read_source_with(root, "unicode.txt", None, None, cap).unwrap();
            assert!(out.truncated, "cap {}: expected truncation", cap);
            // Content is valid UTF-8 (String invariant) and at or under cap.
            assert!(
                out.content.len() <= cap,
                "cap {}: content.len()={}",
                cap,
                out.content.len()
            );
        }
    }

    #[test]
    fn simple_glob_matches_double_star_extension() {
        assert!(simple_glob_match("**/*.rs", "src/foo/bar.rs"));
        assert!(simple_glob_match("**/*.rs", "lib.rs"));
        assert!(!simple_glob_match("**/*.rs", "lib.py"));
    }
}
