use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

use chrono::Utc;
use serde_json::Value;

use ought_run::RunnerConfig;
use ought_spec::{ClauseId, OughtMdParser, Parser, SpecGraph};

use crate::{collect_clauses, count_clauses};

/// Handler for MCP tool invocations.
///
/// Each tool maps to an `ought` CLI command and returns structured JSON.
pub struct ToolHandler {
    /// Project root; relative paths in `spec_roots`, runner `test_dir`s, and
    /// internal artifact locations (manifest) are resolved against this.
    project_root: PathBuf,
    /// Spec roots resolved by the caller (may be absolute or
    /// project-root-relative).
    spec_roots: Vec<PathBuf>,
    /// Runner configuration keyed by runner name (e.g. `rust`, `python`).
    runners: HashMap<String, RunnerConfig>,
}

impl ToolHandler {
    pub fn new(
        project_root: PathBuf,
        spec_roots: Vec<PathBuf>,
        runners: HashMap<String, RunnerConfig>,
    ) -> Self {
        Self { project_root, spec_roots, runners }
    }

    /// Load the spec graph from the configured roots.
    fn load_specs(&self) -> anyhow::Result<SpecGraph> {
        SpecGraph::from_roots(&self.spec_roots).map_err(|errors| {
            let msgs: Vec<String> = errors.iter().map(|e| e.to_string()).collect();
            anyhow::anyhow!("spec parse errors:\n{}", msgs.join("\n"))
        })
    }

    fn base(&self) -> &Path {
        &self.project_root
    }

    /// Wrap a tool result with timing metadata.
    fn with_timing(start: Instant, mut value: Value) -> Value {
        if let Some(obj) = value.as_object_mut() {
            obj.insert(
                "duration_ms".to_string(),
                Value::Number(serde_json::Number::from(start.elapsed().as_millis() as u64)),
            );
            obj.insert(
                "timestamp".to_string(),
                Value::String(Utc::now().to_rfc3339()),
            );
        }
        value
    }

    pub fn ought_run(&self, _args: Value) -> anyhow::Result<Value> {
        let start = Instant::now();
        let specs = self.load_specs()?;

        // Collect all generated tests from the manifest output directory
        // and run them with the configured runner.
        let mut all_results = Vec::new();

        for (runner_name, runner_config) in &self.runners {
            let runner = ought_run::runners::from_name(runner_name)?;
            // Collect generated test files (we look in the runner's test_dir)
            let test_dir = self.base().join(&runner_config.test_dir);

            // For now, run with empty tests list (the runner discovers tests in test_dir)
            let result = runner.run(&[], &test_dir)?;
            all_results.push(serde_json::json!({
                "runner": runner_name,
                "passed": result.passed(),
                "failed": result.failed(),
                "errored": result.errored(),
                "total": result.results.len(),
                "duration_ms": result.total_duration.as_millis() as u64,
                "results": result.results.iter().map(|r| {
                    serde_json::json!({
                        "clause_id": r.clause_id.0,
                        "status": format!("{:?}", r.status),
                        "message": r.message,
                        "duration_ms": r.duration.as_millis() as u64,
                    })
                }).collect::<Vec<_>>(),
            }));
        }

        // Also include spec summary
        let total_clauses: usize = specs
            .specs()
            .iter()
            .map(|s| count_clauses(&s.sections))
            .sum();

        let result = serde_json::json!({
            "total_specs": specs.specs().len(),
            "total_clauses": total_clauses,
            "runners": all_results,
        });

        Ok(Self::with_timing(start, result))
    }

    pub fn ought_generate(&self, args: Value) -> anyhow::Result<Value> {
        let start = Instant::now();
        let specs = self.load_specs()?;

        let force = args
            .get("force")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        // Load manifest
        let manifest_path = self.base().join("ought/ought-gen/manifest.toml");
        let manifest = ought_gen::Manifest::load(&manifest_path)?;

        // Count stale clauses (generation is now done via the agent/orchestrator)
        let mut stale_count = 0;
        for spec in specs.specs() {
            for section in &spec.sections {
                let clauses = collect_clauses(section);
                for clause in clauses {
                    if force || manifest.is_stale(&clause.id, &clause.content_hash, "") {
                        stale_count += 1;
                    }
                }
            }
        }

        let result = serde_json::json!({
            "stale_clauses": stale_count,
            "force": force,
            "message": "Use `ought generate` CLI to run agent-based generation",
        });
        Ok(Self::with_timing(start, result))
    }

    pub fn ought_check(&self, args: Value) -> anyhow::Result<Value> {
        let start = Instant::now();

        let filter_spec = args
            .get("spec")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let mut results = Vec::new();
        let mut total_errors = 0usize;

        for root in &self.spec_roots {
            let files = collect_ought_files(root);
            for file in files {
                // Apply filter if given
                if let Some(ref filter) = filter_spec {
                    let file_name = file
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("");
                    if !file_name.contains(filter.as_str()) {
                        continue;
                    }
                }

                match OughtMdParser.parse_file(&file) {
                    Ok(spec) => {
                        let clause_count = count_clauses(&spec.sections);
                        results.push(serde_json::json!({
                            "file": file.display().to_string(),
                            "status": "ok",
                            "clauses": clause_count,
                        }));
                    }
                    Err(errors) => {
                        total_errors += errors.len();
                        results.push(serde_json::json!({
                            "file": file.display().to_string(),
                            "status": "error",
                            "errors": errors.iter().map(|e| serde_json::json!({
                                "line": e.line,
                                "message": e.message,
                            })).collect::<Vec<_>>(),
                        }));
                    }
                }
            }
        }

        let result = serde_json::json!({
            "files": results,
            "total_errors": total_errors,
            "valid": total_errors == 0,
        });
        Ok(Self::with_timing(start, result))
    }

    pub fn ought_inspect(&self, args: Value) -> anyhow::Result<Value> {
        let start = Instant::now();
        let clause_id_str = args
            .get("clause_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("missing required argument: clause_id"))?;

        let specs = self.load_specs()?;

        // Find the clause across all specs
        let target_id = ClauseId(clause_id_str.to_string());
        let mut found_clause = None;

        'outer: for spec in specs.specs() {
            for section in &spec.sections {
                for clause in collect_clauses(section) {
                    if clause.id == target_id {
                        found_clause = Some(clause.clone());
                        break 'outer;
                    }
                }
            }
        }

        let clause =
            found_clause.ok_or_else(|| anyhow::anyhow!("clause not found: {}", clause_id_str))?;

        let result = serde_json::json!({
            "clause_id": clause.id.0,
            "keyword": format!("{:?}", clause.keyword),
            "text": clause.text,
            "condition": clause.condition,
            "hints": clause.hints,
            "content_hash": clause.content_hash,
        });
        Ok(Self::with_timing(start, result))
    }

    pub fn ought_status(&self, _args: Value) -> anyhow::Result<Value> {
        let start = Instant::now();
        let specs = self.load_specs()?;

        let manifest_path = self.base().join("ought/ought-gen/manifest.toml");
        let manifest = ought_gen::Manifest::load(&manifest_path)?;

        let mut total_clauses = 0usize;
        let mut generated = 0usize;
        let mut stale = 0usize;
        let mut missing = 0usize;
        let mut by_keyword: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();

        for spec in specs.specs() {
            for section in &spec.sections {
                for clause in collect_clauses(section) {
                    total_clauses += 1;
                    *by_keyword
                        .entry(format!("{:?}", clause.keyword))
                        .or_insert(0) += 1;

                    if manifest.entries.contains_key(&clause.id.0) {
                        if manifest.is_stale(&clause.id, &clause.content_hash, "") {
                            stale += 1;
                        } else {
                            generated += 1;
                        }
                    } else {
                        missing += 1;
                    }
                }
            }
        }

        let result = serde_json::json!({
            "total_specs": specs.specs().len(),
            "total_clauses": total_clauses,
            "generated": generated,
            "stale": stale,
            "missing": missing,
            "by_keyword": by_keyword,
        });
        Ok(Self::with_timing(start, result))
    }

    pub fn ought_survey(&self, args: Value) -> anyhow::Result<Value> {
        let start = Instant::now();
        let specs = self.load_specs()?;

        let paths: Vec<PathBuf> = args
            .get("paths")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(PathBuf::from))
                    .collect()
            })
            .unwrap_or_default();

        let result = ought_analysis::survey::survey(&specs, &paths)?;

        let uncovered: Vec<Value> = result
            .uncovered
            .iter()
            .map(|u| {
                serde_json::json!({
                    "file": u.file.display().to_string(),
                    "line": u.line,
                    "description": u.description,
                    "suggested_clause": u.suggested_clause,
                    "suggested_keyword": format!("{:?}", u.suggested_keyword),
                    "suggested_spec": u.suggested_spec.display().to_string(),
                })
            })
            .collect();

        let result = serde_json::json!({
            "uncovered_count": uncovered.len(),
            "uncovered": uncovered,
        });
        Ok(Self::with_timing(start, result))
    }

    pub fn ought_audit(&self, _args: Value) -> anyhow::Result<Value> {
        let start = Instant::now();
        let specs = self.load_specs()?;

        let result = ought_analysis::audit::audit(&specs)?;

        let findings: Vec<Value> = result
            .findings
            .iter()
            .map(|f| {
                serde_json::json!({
                    "kind": format!("{:?}", f.kind),
                    "description": f.description,
                    "clauses": f.clauses.iter().map(|c| &c.0).collect::<Vec<_>>(),
                    "suggestion": f.suggestion,
                    "confidence": f.confidence,
                })
            })
            .collect();

        let result = serde_json::json!({
            "findings_count": findings.len(),
            "findings": findings,
        });
        Ok(Self::with_timing(start, result))
    }

    pub fn ought_blame(&self, args: Value) -> anyhow::Result<Value> {
        let start = Instant::now();
        let clause_id_str = args
            .get("clause_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("missing required argument: clause_id"))?;

        let specs = self.load_specs()?;

        let clause_id = ClauseId(clause_id_str.to_string());

        // We need a RunResult; for now pass an empty one
        let empty_run = ought_run::RunResult {
            results: vec![],
            total_duration: std::time::Duration::ZERO,
        };

        let result =
            ought_analysis::blame::blame(&clause_id, &specs, &empty_run)?;

        let result = serde_json::json!({
            "clause_id": result.clause_id.0,
            "last_passed": result.last_passed.map(|d| d.to_rfc3339()),
            "first_failed": result.first_failed.map(|d| d.to_rfc3339()),
            "likely_commit": result.likely_commit.as_ref().map(|c| serde_json::json!({
                "hash": c.hash,
                "message": c.message,
                "author": c.author,
                "date": c.date.to_rfc3339(),
            })),
            "narrative": result.narrative,
            "suggested_fix": result.suggested_fix,
        });
        Ok(Self::with_timing(start, result))
    }

    pub fn ought_bisect(&self, args: Value) -> anyhow::Result<Value> {
        let start = Instant::now();
        let clause_id_str = args
            .get("clause_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("missing required argument: clause_id"))?;

        let specs = self.load_specs()?;

        let clause_id = ClauseId(clause_id_str.to_string());
        let range = args
            .get("range")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        // Get the first available runner
        let (runner_name, _) = self
            .runners
            .iter()
            .next()
            .ok_or_else(|| anyhow::anyhow!("no runner configured"))?;
        let runner = ought_run::runners::from_name(runner_name)?;

        let options = ought_analysis::bisect::BisectOptions {
            range,
            regenerate: false,
        };

        let result =
            ought_analysis::bisect::bisect(&clause_id, &specs, runner.as_ref(), &options)?;

        let result = serde_json::json!({
            "clause_id": result.clause_id.0,
            "breaking_commit": {
                "hash": result.breaking_commit.hash,
                "message": result.breaking_commit.message,
                "author": result.breaking_commit.author,
                "date": result.breaking_commit.date.to_rfc3339(),
            },
            "diff_summary": result.diff_summary,
        });
        Ok(Self::with_timing(start, result))
    }
}

/// Recursively walk a directory and collect all files matching `*.ought.md`.
fn collect_ought_files(dir: &std::path::Path) -> Vec<PathBuf> {
    let mut results = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                results.extend(collect_ought_files(&path));
            } else if let Some(name) = path.file_name().and_then(|n| n.to_str())
                && name.ends_with(".ought.md")
            {
                results.push(path);
            }
        }
    }
    results
}
