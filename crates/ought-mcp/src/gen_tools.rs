//! JSON-RPC adapters over `ought_gen::tools` for the generation-mode MCP
//! server.
//!
//! All real semantics — path validation, file layout, manifest updates,
//! compile checks — live in `ought_gen::tools`. This module only converts
//! the JSON `Value` arguments into typed calls and serializes the
//! returned outputs back to `Value`.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use serde_json::Value;

use ought_gen::agent::AgentAssignment;
use ought_gen::manifest::Manifest;
use ought_gen::tools;

/// Handler for generation-mode MCP tool invocations.
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
        let val = serde_json::to_value(tools::get_assignment(&self.assignment))
            .map_err(|e| anyhow::anyhow!("failed to serialize assignment: {}", e))?;
        Ok(val)
    }

    /// Read a source file relative to the project root.
    pub fn read_source(&self, args: Value) -> anyhow::Result<Value> {
        let path = args
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("missing required argument: path"))?;

        let project_root = std::path::Path::new(&self.assignment.project_root);
        let out = tools::read_source(project_root, path)?;
        Ok(serde_json::to_value(out)?)
    }

    /// List source files matching a glob pattern within the project.
    pub fn list_source_files(&self, args: Value) -> anyhow::Result<Value> {
        let pattern = args
            .get("pattern")
            .and_then(|v| v.as_str())
            .unwrap_or("**/*.rs");

        let project_root = std::path::Path::new(&self.assignment.project_root);
        let out = tools::list_source_files(project_root, pattern);
        Ok(serde_json::to_value(out)?)
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

        let out = tools::write_test(
            &self.assignment,
            &self.manifest,
            &self.manifest_path,
            clause_id,
            code,
        )?;
        Ok(serde_json::to_value(out)?)
    }

    /// Write multiple test files at once.
    pub fn write_tests_batch(&self, args: Value) -> anyhow::Result<Value> {
        let tests = args
            .get("tests")
            .and_then(|v| v.as_array())
            .ok_or_else(|| anyhow::anyhow!("missing required argument: tests (array)"))?;

        let mut pairs: Vec<(String, String)> = Vec::with_capacity(tests.len());
        for test in tests {
            let clause_id = test
                .get("clause_id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("each test must have clause_id"))?;
            let code = test
                .get("code")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("each test must have code"))?;
            pairs.push((clause_id.to_string(), code.to_string()));
        }

        let out = tools::write_tests_batch(
            &self.assignment,
            &self.manifest,
            &self.manifest_path,
            pairs,
        );
        Ok(serde_json::to_value(out)?)
    }

    /// Check if written test files compile.
    pub fn check_compiles(&self, args: Value) -> anyhow::Result<Value> {
        let clause_ids = args
            .get("clause_ids")
            .and_then(|v| v.as_array())
            .ok_or_else(|| anyhow::anyhow!("missing required argument: clause_ids (array)"))?;

        let ids: Vec<&str> = clause_ids.iter().filter_map(|v| v.as_str()).collect();
        let test_dir = std::path::Path::new(&self.assignment.test_dir);
        let lang = self.assignment.target_language.as_str();
        let out = tools::check_compiles(test_dir, lang, ids);
        Ok(serde_json::to_value(out)?)
    }

    /// Report progress to the parent process via stderr.
    pub fn report_progress(&self, args: Value) -> anyhow::Result<Value> {
        let status = args
            .get("status")
            .and_then(|v| v.as_str())
            .unwrap_or("in_progress");
        let message = args.get("message").and_then(|v| v.as_str()).unwrap_or("");
        let clauses_completed = args
            .get("clauses_completed")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let clauses_total = args
            .get("clauses_total")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);

        tools::report_progress(
            &self.assignment.id,
            status,
            message,
            clauses_completed,
            clauses_total,
        );

        Ok(serde_json::json!({ "acknowledged": true }))
    }
}
