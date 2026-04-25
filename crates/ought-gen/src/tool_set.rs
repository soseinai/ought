//! [`oharness_tools::ToolSet`] implementation that exposes the
//! `ought_gen::tools` primitives to an in-process agent loop.
//!
//! Wraps the same primitives the MCP server uses, so behavior is
//! identical between the in-process path and the MCP path. Records the
//! results of every `write_test` and `check_compiles` call so the
//! orchestrator can build an accurate per-clause [`AgentReport`] without
//! parsing model output.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use serde_json::{Value, json};

use oharness_tools::{ToolOutcome, ToolSet, ToolSpec};
use oharness_tools::context::ToolContext;

use crate::agent::AgentAssignment;
use crate::manifest::Manifest;
use crate::tools::{self, CompileResult, DEFAULT_READ_SOURCE_LIMIT_BYTES, WriteTestResult};

/// Tracker for what the agent did. Read by the orchestrator after the
/// agent loop terminates to populate the report.
#[derive(Debug, Default, Clone)]
pub struct ToolUsage {
    pub written: Vec<String>,
    pub write_errors: Vec<(String, String)>,
}

/// In-process tool set for generation tasks.
pub struct GenerateToolSet {
    assignment: AgentAssignment,
    manifest: Arc<Mutex<Manifest>>,
    manifest_path: PathBuf,
    specs: Vec<ToolSpec>,
    usage: Arc<Mutex<ToolUsage>>,
    read_source_limit_bytes: usize,
}

impl GenerateToolSet {
    /// Construct with the default `read_source` size cap.
    pub fn new(
        assignment: AgentAssignment,
        manifest: Arc<Mutex<Manifest>>,
        manifest_path: PathBuf,
    ) -> Self {
        Self::with_limits(
            assignment,
            manifest,
            manifest_path,
            DEFAULT_READ_SOURCE_LIMIT_BYTES,
        )
    }

    /// Construct with an explicit `read_source` cap (in bytes).
    pub fn with_limits(
        assignment: AgentAssignment,
        manifest: Arc<Mutex<Manifest>>,
        manifest_path: PathBuf,
        read_source_limit_bytes: usize,
    ) -> Self {
        Self {
            assignment,
            manifest,
            manifest_path,
            specs: tool_specs(),
            usage: Arc::new(Mutex::new(ToolUsage::default())),
            read_source_limit_bytes,
        }
    }

    /// Snapshot of what the agent has done so far.
    pub fn usage(&self) -> ToolUsage {
        self.usage.lock().unwrap().clone()
    }
}

#[async_trait]
impl ToolSet for GenerateToolSet {
    fn specs(&self) -> &[ToolSpec] {
        &self.specs
    }

    async fn execute(&self, name: &str, input: Value, _ctx: &ToolContext) -> ToolOutcome {
        match name {
            "get_assignment" => {
                let out = tools::get_assignment(&self.assignment);
                serde_outcome(&out)
            }

            "read_source" => {
                let path = match input.get("path").and_then(|v| v.as_str()) {
                    Some(p) => p.to_string(),
                    None => return err("missing required argument: path"),
                };
                let start_line = input
                    .get("start_line")
                    .and_then(|v| v.as_u64())
                    .map(|n| n as usize);
                let end_line = input
                    .get("end_line")
                    .and_then(|v| v.as_u64())
                    .map(|n| n as usize);
                let project_root = PathBuf::from(&self.assignment.project_root);
                let limit = self.read_source_limit_bytes;
                match tokio::task::spawn_blocking(move || {
                    tools::read_source_with(&project_root, &path, start_line, end_line, limit)
                })
                .await
                {
                    Ok(Ok(out)) => serde_outcome(&out),
                    Ok(Err(e)) => err(e.to_string()),
                    Err(e) => err(format!("read_source task panicked: {}", e)),
                }
            }

            "list_source_files" => {
                let pattern = input
                    .get("pattern")
                    .and_then(|v| v.as_str())
                    .unwrap_or("**/*.rs")
                    .to_string();
                let project_root = PathBuf::from(&self.assignment.project_root);
                let out = tokio::task::spawn_blocking(move || {
                    tools::list_source_files(&project_root, &pattern)
                })
                .await;
                match out {
                    Ok(o) => serde_outcome(&o),
                    Err(e) => err(format!("list task panicked: {}", e)),
                }
            }

            "write_test" => {
                let clause_id = match input.get("clause_id").and_then(|v| v.as_str()) {
                    Some(s) => s.to_string(),
                    None => return err("missing required argument: clause_id"),
                };
                let code = match input.get("code").and_then(|v| v.as_str()) {
                    Some(s) => s.to_string(),
                    None => return err("missing required argument: code"),
                };
                let assignment = self.assignment.clone();
                let manifest = self.manifest.clone();
                let manifest_path = self.manifest_path.clone();
                let usage = self.usage.clone();
                let cid = clause_id.clone();
                let result = tokio::task::spawn_blocking(move || {
                    tools::write_test(&assignment, &manifest, &manifest_path, &cid, &code)
                })
                .await;
                match result {
                    Ok(Ok(out)) => {
                        usage.lock().unwrap().written.push(clause_id);
                        serde_outcome(&out)
                    }
                    Ok(Err(e)) => {
                        let msg = e.to_string();
                        usage
                            .lock()
                            .unwrap()
                            .write_errors
                            .push((clause_id, msg.clone()));
                        err(msg)
                    }
                    Err(e) => err(format!("write_test task panicked: {}", e)),
                }
            }

            "write_tests_batch" => {
                let tests_arr = match input.get("tests").and_then(|v| v.as_array()) {
                    Some(a) => a.clone(),
                    None => return err("missing required argument: tests (array)"),
                };
                let mut pairs: Vec<(String, String)> = Vec::with_capacity(tests_arr.len());
                for t in &tests_arr {
                    let cid = match t.get("clause_id").and_then(|v| v.as_str()) {
                        Some(s) => s.to_string(),
                        None => return err("each test must have clause_id"),
                    };
                    let code = match t.get("code").and_then(|v| v.as_str()) {
                        Some(s) => s.to_string(),
                        None => return err("each test must have code"),
                    };
                    pairs.push((cid, code));
                }
                let assignment = self.assignment.clone();
                let manifest = self.manifest.clone();
                let manifest_path = self.manifest_path.clone();
                let usage = self.usage.clone();
                let result = tokio::task::spawn_blocking(move || {
                    tools::write_tests_batch(&assignment, &manifest, &manifest_path, pairs)
                })
                .await;
                match result {
                    Ok(out) => {
                        let mut u = usage.lock().unwrap();
                        for r in &out.results {
                            match r {
                                WriteTestResult::Ok { clause_id, .. } => {
                                    u.written.push(clause_id.clone())
                                }
                                WriteTestResult::Err {
                                    clause_id, error, ..
                                } => u
                                    .write_errors
                                    .push((clause_id.clone(), error.clone())),
                            }
                        }
                        drop(u);
                        serde_outcome(&out)
                    }
                    Err(e) => err(format!("batch write task panicked: {}", e)),
                }
            }

            "check_compiles" => {
                let ids: Vec<String> = match input.get("clause_ids").and_then(|v| v.as_array()) {
                    Some(arr) => arr
                        .iter()
                        .filter_map(|v| v.as_str().map(str::to_string))
                        .collect(),
                    None => {
                        return err("missing required argument: clause_ids (array)");
                    }
                };
                let test_dir = PathBuf::from(&self.assignment.test_dir);
                let lang = self.assignment.target_language.clone();
                let result = tokio::task::spawn_blocking(move || {
                    tools::check_compiles(&test_dir, &lang, ids.iter().map(|s| s.as_str()))
                })
                .await;
                match result {
                    Ok(out) => {
                        // We don't move compile failures out of `write_errors`
                        // — write_errors is for writes that failed. Compile
                        // results are reflected back to the model so it can
                        // iterate; the orchestrator doesn't need to track
                        // them separately.
                        let _ = out
                            .results
                            .iter()
                            .filter(|r| matches!(r, CompileResult::Error { .. }))
                            .count();
                        serde_outcome(&out)
                    }
                    Err(e) => err(format!("check_compiles task panicked: {}", e)),
                }
            }

            "report_progress" => {
                let status = input
                    .get("status")
                    .and_then(|v| v.as_str())
                    .unwrap_or("in_progress");
                let message = input.get("message").and_then(|v| v.as_str()).unwrap_or("");
                let completed = input
                    .get("clauses_completed")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                let total = input
                    .get("clauses_total")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                tools::report_progress(&self.assignment.id, status, message, completed, total);
                ok(json!({ "acknowledged": true }).to_string())
            }

            other => err(format!("unknown tool: {}", other)),
        }
    }
}

fn serde_outcome<T: serde::Serialize>(value: &T) -> ToolOutcome {
    match serde_json::to_string(value) {
        Ok(s) => ok(s),
        Err(e) => err(format!("serialization error: {}", e)),
    }
}

/// Shim: `ought_agent::ToolOutcome::ok` → `oharness_tools::ToolOutcome::success_text`.
fn ok(s: impl Into<String>) -> ToolOutcome {
    ToolOutcome::success_text(s)
}

/// Shim: `ought_agent::ToolOutcome::err` → `oharness_tools::ToolOutcome::error(_, true)`.
/// All ought-gen tool failures are recoverable — the model retries.
fn err(s: impl Into<String>) -> ToolOutcome {
    ToolOutcome::error(s, true)
}

/// JSON-Schema-shaped tool definitions sent to the model.
fn tool_specs() -> Vec<ToolSpec> {
    vec![
        ToolSpec {
            name: "get_assignment".into(),
            description:
                "Return the agent's assignment: the clauses to generate tests for, plus \
                 metadata about the project and target language."
                    .into(),
            input_schema: json!({ "type": "object", "properties": {} }),
        },
        ToolSpec {
            name: "read_source".into(),
            description:
                "Read a source file relative to the project root. Use this to understand \
                 the code under test before generating assertions against it. Reads are \
                 capped at a few tens of KB; if `truncated: true` comes back, call again \
                 with `start_line` / `end_line` to read a specific window."
                    .into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "path relative to project root" },
                    "start_line": {
                        "type": "integer",
                        "description": "1-based inclusive start line. Optional."
                    },
                    "end_line": {
                        "type": "integer",
                        "description": "1-based inclusive end line. Optional."
                    }
                },
                "required": ["path"]
            }),
        },
        ToolSpec {
            name: "list_source_files".into(),
            description:
                "List source files matching a glob pattern (e.g. \"**/*.rs\") under the \
                 project root."
                    .into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "pattern": { "type": "string", "description": "glob pattern" }
                }
            }),
        },
        ToolSpec {
            name: "write_test".into(),
            description:
                "Write a single test for a clause. The code should be one #[test] fn (Rust) \
                 plus its preceding doc comment. Multiple clauses in the same subsection \
                 share a file; this primitive merges by function name."
                    .into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "clause_id": { "type": "string" },
                    "code": { "type": "string" }
                },
                "required": ["clause_id", "code"]
            }),
        },
        ToolSpec {
            name: "write_tests_batch".into(),
            description:
                "Write multiple tests in one call. Returns per-test success/failure."
                    .into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "tests": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "clause_id": { "type": "string" },
                                "code": { "type": "string" }
                            },
                            "required": ["clause_id", "code"]
                        }
                    }
                },
                "required": ["tests"]
            }),
        },
        ToolSpec {
            name: "check_compiles".into(),
            description:
                "Compile-check the test files for the given clause ids. Use this after \
                 writing to catch syntax errors before reporting completion."
                    .into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "clause_ids": { "type": "array", "items": { "type": "string" } }
                },
                "required": ["clause_ids"]
            }),
        },
        ToolSpec {
            name: "report_progress".into(),
            description:
                "Emit a progress line to the human user. Optional but encouraged for \
                 long-running batches."
                    .into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "status": { "type": "string" },
                    "message": { "type": "string" },
                    "clauses_completed": { "type": "integer" },
                    "clauses_total": { "type": "integer" }
                }
            }),
        },
    ]
}
