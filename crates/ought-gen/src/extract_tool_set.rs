//! [`ought_agent::ToolSet`] adapter for the extraction agent loop.
//!
//! Wraps the sync primitives in [`crate::extract_tools`] as async tools
//! and records per-assignment usage so the orchestrator can build an
//! accurate [`crate::ExtractReport`] without scraping model output.

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use serde_json::{Value, json};

use ought_agent::{ToolOutcome, ToolSet};
use ought_llm::ToolSpec;

use crate::extract::ExtractAssignment;
use crate::extract_tools::{self, WriteSpecOutput};
use crate::tools::{self, DEFAULT_READ_SOURCE_LIMIT_BYTES};

/// Tracker for what the extraction agent did.
#[derive(Debug, Default, Clone)]
pub struct ExtractUsage {
    /// Target paths that were written (or previewed under dry-run).
    pub written: Vec<String>,
    /// Per-target failures: (target_path, message).
    pub write_errors: Vec<(String, String)>,
}

/// In-process tool set for extraction tasks.
pub struct ExtractToolSet {
    assignment: ExtractAssignment,
    specs: Vec<ToolSpec>,
    usage: Arc<Mutex<ExtractUsage>>,
    read_source_limit_bytes: usize,
}

impl ExtractToolSet {
    pub fn new(assignment: ExtractAssignment) -> Self {
        Self::with_limits(assignment, DEFAULT_READ_SOURCE_LIMIT_BYTES)
    }

    pub fn with_limits(assignment: ExtractAssignment, read_source_limit_bytes: usize) -> Self {
        Self {
            assignment,
            specs: tool_specs(),
            usage: Arc::new(Mutex::new(ExtractUsage::default())),
            read_source_limit_bytes,
        }
    }

    pub fn usage(&self) -> ExtractUsage {
        self.usage.lock().unwrap().clone()
    }
}

#[async_trait]
impl ToolSet for ExtractToolSet {
    fn specs(&self) -> &[ToolSpec] {
        &self.specs
    }

    async fn execute(&self, name: &str, input: Value) -> ToolOutcome {
        match name {
            "get_assignment" => {
                let out = extract_tools::get_assignment(&self.assignment);
                serde_outcome(&out)
            }

            "read_source" => {
                let path = match input.get("path").and_then(|v| v.as_str()) {
                    Some(p) => p.to_string(),
                    None => return ToolOutcome::err("missing required argument: path"),
                };
                let start_line = input
                    .get("start_line")
                    .and_then(|v| v.as_u64())
                    .map(|n| n as usize);
                let end_line = input
                    .get("end_line")
                    .and_then(|v| v.as_u64())
                    .map(|n| n as usize);
                let project_root = std::path::PathBuf::from(&self.assignment.project_root);
                let limit = self.read_source_limit_bytes;
                match tokio::task::spawn_blocking(move || {
                    tools::read_source_with(&project_root, &path, start_line, end_line, limit)
                })
                .await
                {
                    Ok(Ok(out)) => serde_outcome(&out),
                    Ok(Err(e)) => ToolOutcome::err(e.to_string()),
                    Err(e) => ToolOutcome::err(format!("read_source task panicked: {}", e)),
                }
            }

            "list_source_files" => {
                let pattern = input
                    .get("pattern")
                    .and_then(|v| v.as_str())
                    .unwrap_or("**/*.rs")
                    .to_string();
                let project_root = std::path::PathBuf::from(&self.assignment.project_root);
                let out = tokio::task::spawn_blocking(move || {
                    tools::list_source_files(&project_root, &pattern)
                })
                .await;
                match out {
                    Ok(o) => serde_outcome(&o),
                    Err(e) => ToolOutcome::err(format!("list task panicked: {}", e)),
                }
            }

            "validate_spec" => {
                let content = match input.get("content").and_then(|v| v.as_str()) {
                    Some(s) => s.to_string(),
                    None => return ToolOutcome::err("missing required argument: content"),
                };
                let out = tokio::task::spawn_blocking(move || extract_tools::validate_spec(&content))
                    .await;
                match out {
                    Ok(o) => serde_outcome(&o),
                    Err(e) => ToolOutcome::err(format!("validate_spec task panicked: {}", e)),
                }
            }

            "write_spec" => {
                let target_path = match input.get("target_path").and_then(|v| v.as_str()) {
                    Some(s) => s.to_string(),
                    None => return ToolOutcome::err("missing required argument: target_path"),
                };
                let content = match input.get("content").and_then(|v| v.as_str()) {
                    Some(s) => s.to_string(),
                    None => return ToolOutcome::err("missing required argument: content"),
                };
                let assignment = self.assignment.clone();
                let usage = self.usage.clone();
                let tp_clone = target_path.clone();
                let result = tokio::task::spawn_blocking(move || {
                    extract_tools::write_spec(&assignment, &tp_clone, &content)
                })
                .await;
                match result {
                    Ok(Ok(out)) => {
                        match &out {
                            WriteSpecOutput::Written { target_path, .. }
                            | WriteSpecOutput::DryRun { target_path, .. } => {
                                usage.lock().unwrap().written.push(target_path.clone());
                            }
                            WriteSpecOutput::Rejected {
                                target_path,
                                errors,
                            } => {
                                usage
                                    .lock()
                                    .unwrap()
                                    .write_errors
                                    .push((target_path.clone(), errors.join("; ")));
                            }
                            WriteSpecOutput::SkippedExists { .. } => {}
                        }
                        serde_outcome(&out)
                    }
                    Ok(Err(e)) => {
                        let msg = e.to_string();
                        usage
                            .lock()
                            .unwrap()
                            .write_errors
                            .push((target_path, msg.clone()));
                        ToolOutcome::err(msg)
                    }
                    Err(e) => ToolOutcome::err(format!("write_spec task panicked: {}", e)),
                }
            }

            "report_progress" => {
                let status = input
                    .get("status")
                    .and_then(|v| v.as_str())
                    .unwrap_or("in_progress");
                let message = input.get("message").and_then(|v| v.as_str()).unwrap_or("");
                let completed = input
                    .get("groups_completed")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                let total = input
                    .get("groups_total")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                tools::report_progress(&self.assignment.id, status, message, completed, total);
                ToolOutcome::ok(json!({ "acknowledged": true }).to_string())
            }

            other => ToolOutcome::err(format!("unknown tool: {}", other)),
        }
    }
}

fn serde_outcome<T: serde::Serialize>(value: &T) -> ToolOutcome {
    match serde_json::to_string(value) {
        Ok(s) => ToolOutcome::ok(s),
        Err(e) => ToolOutcome::err(format!("serialization error: {}", e)),
    }
}

fn tool_specs() -> Vec<ToolSpec> {
    vec![
        ToolSpec {
            name: "get_assignment".into(),
            description:
                "Return the extraction assignment: groups (each = one target .ought.md file), \
                 their source files, and the titles to use."
                    .into(),
            input_schema: json!({ "type": "object", "properties": {} }),
        },
        ToolSpec {
            name: "read_source".into(),
            description:
                "Read a source file relative to the project root. Reads are capped at a few \
                 tens of KB; call again with start_line / end_line if truncated."
                    .into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "path relative to project root" },
                    "start_line": { "type": "integer" },
                    "end_line": { "type": "integer" }
                },
                "required": ["path"]
            }),
        },
        ToolSpec {
            name: "list_source_files".into(),
            description: "List source files matching a glob pattern under the project root.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "pattern": { "type": "string", "description": "glob pattern, e.g. **/*.rs" }
                }
            }),
        },
        ToolSpec {
            name: "validate_spec".into(),
            description:
                "Parse a draft .ought.md spec with the canonical parser. Returns {ok, errors}. \
                 Call this before write_spec so the grammar gate catches mistakes before disk."
                    .into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "content": { "type": "string", "description": "full .ought.md content" }
                },
                "required": ["content"]
            }),
        },
        ToolSpec {
            name: "write_spec".into(),
            description:
                "Write a validated spec to <specs_root>/<target_path>. Re-validates before \
                 writing; refuses malformed content, paths outside specs_root, or existing \
                 files unless the run was invoked with --force. Under --dry-run, prints to \
                 stdout instead of writing."
                    .into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "target_path": {
                        "type": "string",
                        "description": "Path relative to specs_root, ending in .ought.md"
                    },
                    "content": { "type": "string" }
                },
                "required": ["target_path", "content"]
            }),
        },
        ToolSpec {
            name: "report_progress".into(),
            description: "Emit a one-line progress update to the human user.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "status": { "type": "string" },
                    "message": { "type": "string" },
                    "groups_completed": { "type": "integer" },
                    "groups_total": { "type": "integer" }
                }
            }),
        },
    ]
}
