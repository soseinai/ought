use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde_json::Value;

use ought_gen::agent::AgentAssignment;
use ought_gen::manifest::{Manifest, ManifestEntry};

/// Handler for generation-mode MCP tool invocations.
///
/// These tools are called by LLM agents to drive the test generation loop:
/// reading assignments, writing tests, checking compilation, etc.
pub struct GenToolHandler {
    assignment: AgentAssignment,
    manifest: Arc<Mutex<Manifest>>,
    manifest_path: PathBuf,
}

impl GenToolHandler {
    pub fn new(
        assignment: AgentAssignment,
        manifest: Arc<Mutex<Manifest>>,
        manifest_path: PathBuf,
    ) -> Self {
        Self {
            assignment,
            manifest,
            manifest_path,
        }
    }

    /// Returns the assignment as JSON so the agent knows what to generate.
    pub fn get_assignment(&self, _args: Value) -> anyhow::Result<Value> {
        let val = serde_json::to_value(&self.assignment)
            .map_err(|e| anyhow::anyhow!("failed to serialize assignment: {}", e))?;
        Ok(val)
    }

    /// Read a source file relative to the project root.
    pub fn read_source(&self, args: Value) -> anyhow::Result<Value> {
        let path_str = args
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("missing required argument: path"))?;

        let project_root = Path::new(&self.assignment.project_root);
        let resolved = project_root.join(path_str);

        // Security: ensure the resolved path is within the project root.
        let canonical_root = project_root.canonicalize().unwrap_or_else(|_| project_root.to_path_buf());
        let canonical_path = resolved.canonicalize()
            .map_err(|e| anyhow::anyhow!("cannot resolve path '{}': {}", path_str, e))?;

        if !canonical_path.starts_with(&canonical_root) {
            anyhow::bail!("path '{}' is outside the project root", path_str);
        }

        let content = std::fs::read_to_string(&canonical_path)
            .map_err(|e| anyhow::anyhow!("failed to read '{}': {}", path_str, e))?;

        Ok(serde_json::json!({
            "path": path_str,
            "content": content,
        }))
    }

    /// List source files matching a glob pattern within the project.
    pub fn list_source_files(&self, args: Value) -> anyhow::Result<Value> {
        let pattern = args
            .get("pattern")
            .and_then(|v| v.as_str())
            .unwrap_or("**/*.rs");

        let project_root = Path::new(&self.assignment.project_root);

        let mut files = Vec::new();
        collect_files_matching(project_root, pattern, &mut files);

        // Return paths relative to project root.
        let relative_paths: Vec<String> = files
            .iter()
            .filter_map(|p| {
                p.strip_prefix(project_root)
                    .ok()
                    .map(|r| r.to_string_lossy().to_string())
            })
            .collect();

        Ok(serde_json::json!({
            "pattern": pattern,
            "files": relative_paths,
            "count": relative_paths.len(),
        }))
    }

    /// Write a single test file for a clause.
    pub fn write_test(&self, args: Value) -> anyhow::Result<Value> {
        let clause_id = args
            .get("clause_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("missing required argument: clause_id"))?;
        let code = args
            .get("code")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("missing required argument: code"))?;

        let file_path = self.write_test_file(clause_id, code)?;

        Ok(serde_json::json!({
            "clause_id": clause_id,
            "file_path": file_path.to_string_lossy(),
            "status": "written",
        }))
    }

    /// Write multiple test files at once.
    pub fn write_tests_batch(&self, args: Value) -> anyhow::Result<Value> {
        let tests = args
            .get("tests")
            .and_then(|v| v.as_array())
            .ok_or_else(|| anyhow::anyhow!("missing required argument: tests (array)"))?;

        let mut results = Vec::new();

        for test in tests {
            let clause_id = test
                .get("clause_id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("each test must have clause_id"))?;
            let code = test
                .get("code")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("each test must have code"))?;

            match self.write_test_file(clause_id, code) {
                Ok(file_path) => {
                    results.push(serde_json::json!({
                        "clause_id": clause_id,
                        "file_path": file_path.to_string_lossy(),
                        "status": "written",
                    }));
                }
                Err(e) => {
                    results.push(serde_json::json!({
                        "clause_id": clause_id,
                        "status": "error",
                        "error": format!("{}", e),
                    }));
                }
            }
        }

        Ok(serde_json::json!({
            "results": results,
            "total": results.len(),
        }))
    }

    /// Check if written test files compile.
    pub fn check_compiles(&self, args: Value) -> anyhow::Result<Value> {
        let clause_ids = args
            .get("clause_ids")
            .and_then(|v| v.as_array())
            .ok_or_else(|| anyhow::anyhow!("missing required argument: clause_ids (array)"))?;

        let test_dir = Path::new(&self.assignment.test_dir);
        let lang = self.assignment.target_language.as_str();

        let mut results = Vec::new();

        for id_val in clause_ids {
            let clause_id = id_val.as_str().unwrap_or("");
            if clause_id.is_empty() {
                continue;
            }

            let file_path = derive_test_file_path(test_dir, clause_id, lang);
            if !file_path.exists() {
                results.push(serde_json::json!({
                    "clause_id": clause_id,
                    "status": "missing",
                    "error": format!("file not found: {}", file_path.display()),
                }));
                continue;
            }

            let compile_result = check_file_compiles(&file_path, lang);
            match compile_result {
                Ok(()) => {
                    results.push(serde_json::json!({
                        "clause_id": clause_id,
                        "status": "ok",
                    }));
                }
                Err(error_msg) => {
                    results.push(serde_json::json!({
                        "clause_id": clause_id,
                        "status": "error",
                        "error": error_msg,
                    }));
                }
            }
        }

        Ok(serde_json::json!({
            "results": results,
        }))
    }

    /// Report progress to the parent process via stderr.
    pub fn report_progress(&self, args: Value) -> anyhow::Result<Value> {
        let status = args
            .get("status")
            .and_then(|v| v.as_str())
            .unwrap_or("in_progress");
        let message = args
            .get("message")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let clauses_completed = args
            .get("clauses_completed")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let clauses_total = args
            .get("clauses_total")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);

        eprintln!(
            "  [agent {}] {}: {} ({}/{})",
            self.assignment.id, status, message, clauses_completed, clauses_total
        );

        Ok(serde_json::json!({
            "acknowledged": true,
        }))
    }

    // ── Internal helpers ────────────────────────────────────────────────

    /// Write a test into its per-subsection file and update the manifest.
    ///
    /// Under the per-subsection layout, many clauses share one file. This
    /// function merges the incoming `code` (a single `#[test]` block plus
    /// any doc comments) into the file: if a test with the same function
    /// name already exists, it is replaced; otherwise the new block is
    /// appended. If the file doesn't exist yet, a minimal Rust file header
    /// is written first.
    fn write_test_file(&self, clause_id: &str, code: &str) -> anyhow::Result<PathBuf> {
        let test_dir = Path::new(&self.assignment.test_dir);
        let lang = self.assignment.target_language.as_str();
        let file_path = derive_test_file_path(test_dir, clause_id, lang);

        // Ensure parent directory exists.
        if let Some(parent) = file_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let merged = if file_path.exists() {
            let existing = std::fs::read_to_string(&file_path).map_err(|e| {
                anyhow::anyhow!("failed to read {}: {}", file_path.display(), e)
            })?;
            merge_test_block(&existing, code, lang)
        } else {
            format!("{}\n{}\n", default_file_header(lang), code.trim_end())
        };

        std::fs::write(&file_path, merged).map_err(|e| {
            anyhow::anyhow!("failed to write test file {}: {}", file_path.display(), e)
        })?;

        // Find the content_hash from the assignment.
        let content_hash = self.find_content_hash(clause_id);

        // Update manifest.
        {
            let mut manifest = self.manifest.lock().unwrap();
            manifest.entries.insert(
                clause_id.to_string(),
                ManifestEntry {
                    clause_hash: content_hash,
                    source_hash: String::new(),
                    generated_at: chrono::Utc::now(),
                    model: "agent".to_string(),
                },
            );
            // Save manifest to disk immediately (ctrl+c safe).
            manifest.save(&self.manifest_path)?;
        }

        Ok(file_path)
    }

    /// Look up the content hash for a clause ID from the assignment data.
    fn find_content_hash(&self, clause_id: &str) -> String {
        for group in &self.assignment.groups {
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
}

/// Derive the test file path from a clause ID and language.
///
/// Uses the **per-subsection** layout: all clauses in the same subsection
/// share a single `<subsection>_test.rs` file. For a clause ID like
/// `parser::clause_ir::must_generate_foo`, the subsection is `clause_ir`
/// and the file is `<test_dir>/src/parser/clause_ir_test.rs`. For Rust,
/// tests compile as modules of the `ought-dogfood` crate; other languages
/// still use a single per-subsection file under `<test_dir>/<subsystem>/`.
fn derive_test_file_path(test_dir: &Path, clause_id: &str, lang: &str) -> PathBuf {
    let ext = match lang {
        "rust" => "_test.rs",
        "python" => "_test.py",
        "typescript" => ".test.ts",
        "javascript" => ".test.js",
        "go" => "_test.go",
        _ => "_test.rs",
    };

    let segments: Vec<&str> = clause_id.split("::").collect();
    // Need at least <subsystem>::<subsection>::<clause> — 3 segments — to
    // have both a path prefix and a section file stem. Degrade gracefully
    // for shorter IDs.
    let (dir_segs, file_stem): (&[&str], &str) = match segments.as_slice() {
        [] => (&[], "unknown"),
        [one] => (&[][..], *one),
        // 2+ segments: drop the last (clause slug), use second-to-last as stem.
        rest => {
            let len = rest.len();
            (&rest[..len - 2], rest[len - 2])
        }
    };

    let mut path = test_dir.to_path_buf();
    // Rust tests compile under the cargo crate rooted at test_dir; land them
    // in `src/` so the module tree picks them up. Other languages don't
    // have this constraint, so we skip the src/ prefix there.
    if lang == "rust" {
        path.push("src");
    }
    for seg in dir_segs {
        path.push(seg);
    }
    path.push(format!("{}{}", file_stem, ext));
    path
}

/// Minimal file header for a freshly-created per-subsection test file.
///
/// For Rust, we blanket-allow the lint categories the existing generated
/// files use (dead_code, unused_imports, etc.) so a partially-populated
/// file compiles with `warnings = deny`. Non-Rust languages return an
/// empty header.
fn default_file_header(lang: &str) -> &'static str {
    match lang {
        "rust" => "#![allow(dead_code, clippy::all, non_snake_case, unused_imports)]\n",
        _ => "",
    }
}

/// Extract the `#[test]` function name from a test-block string, if any.
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

/// Find the byte range `[start, end)` of an existing `#[test] fn <name>` block
/// in `content`. `start` covers any preceding `///` doc comments and `#[...]`
/// attributes; `end` is one past the closing `}` of the function body.
fn find_test_block(content: &str, fn_name: &str) -> Option<(usize, usize)> {
    let needle = format!("fn {}(", fn_name);
    let fn_idx = content.find(&needle)?;

    // Walk backwards from fn_idx to pick up attributes and doc comments that
    // belong to this test.
    let prefix = &content[..fn_idx];
    let mut start = fn_idx;
    for (line_start, line) in prefix.rmatch_indices('\n').map(|(i, _)| i + 1).zip(prefix.lines().rev()) {
        let trimmed = line.trim_start();
        if trimmed.starts_with("#[") || trimmed.starts_with("///") || trimmed.is_empty() {
            start = line_start;
        } else {
            break;
        }
    }

    // Walk forward from fn_idx counting braces to find the body end.
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
                    // Include the trailing newline if there is one.
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

/// Merge `new_block` (a single `#[test]` function + its doc comments) into
/// `existing` file content. Replaces any prior block with the same test-fn
/// name, otherwise appends to the end of the file.
fn merge_test_block(existing: &str, new_block: &str, lang: &str) -> String {
    if lang != "rust" {
        // Non-Rust languages don't have the same module structure; naive append.
        let mut out = existing.trim_end().to_string();
        out.push_str("\n\n");
        out.push_str(new_block.trim());
        out.push('\n');
        return out;
    }

    let Some(new_fn_name) = extract_test_fn_name(new_block) else {
        // Can't identify the incoming fn — append conservatively.
        let mut out = existing.trim_end().to_string();
        out.push_str("\n\n");
        out.push_str(new_block.trim());
        out.push('\n');
        return out;
    };

    if let Some((start, end)) = find_test_block(existing, &new_fn_name) {
        // Replace in place.
        let mut out = String::with_capacity(existing.len() + new_block.len());
        out.push_str(&existing[..start]);
        out.push_str(new_block.trim());
        out.push('\n');
        out.push_str(&existing[end..]);
        return out;
    }

    // Append to end of file.
    let mut out = existing.trim_end().to_string();
    out.push_str("\n\n");
    out.push_str(new_block.trim());
    out.push('\n');
    out
}

/// Check if a test file compiles for the given language.
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
        "go" => Command::new("go")
            .args(["vet"])
            .arg(file_path)
            .output(),
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

/// Recursively collect files matching a simple glob pattern.
/// Supports patterns like "**/*.rs", "src/**/*.py", "*.go".
fn collect_files_matching(root: &Path, pattern: &str, results: &mut Vec<PathBuf>) {
    // Simple pattern matching: split on "/" and handle ** as recursive.
    collect_files_recursive(root, &mut |path| {
        let rel = path.strip_prefix(root).unwrap_or(path);
        let rel_str = rel.to_string_lossy();
        if simple_glob_match(pattern, &rel_str) {
            results.push(path.to_path_buf());
        }
    });
}

/// Simple glob matching for patterns like "**/*.rs".
fn simple_glob_match(pattern: &str, path: &str) -> bool {
    // Handle the common case: **/*.ext
    if let Some(ext_pattern) = pattern.strip_prefix("**/") {
        if ext_pattern.starts_with('*') {
            // Pattern like **/*.rs
            if let Some(ext) = ext_pattern.strip_prefix('*') {
                return path.ends_with(ext);
            }
        }
        // Pattern like **/foo.rs
        return path.ends_with(ext_pattern);
    }

    // Handle *.ext at root
    if pattern.starts_with('*')
        && let Some(ext) = pattern.strip_prefix('*') {
            return path.ends_with(ext) && !path.contains('/');
        }

    // Exact prefix match with glob
    if let Some((prefix, suffix)) = pattern.split_once("**") {
        let suffix = suffix.strip_prefix('/').unwrap_or(suffix);
        if let Some(rest) = path.strip_prefix(prefix) {
            if suffix.starts_with('*')
                && let Some(ext) = suffix.strip_prefix('*') {
                    return rest.ends_with(ext);
                }
            return rest.ends_with(suffix);
        }
        return false;
    }

    // Literal match
    path == pattern
}

/// Walk a directory tree, calling the callback for each file.
fn collect_files_recursive(dir: &Path, callback: &mut dyn FnMut(&Path)) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");

        // Skip hidden files/directories.
        if name.starts_with('.') {
            continue;
        }
        // Skip common build directories.
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
