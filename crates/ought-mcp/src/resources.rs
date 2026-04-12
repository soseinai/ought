use std::path::{Path, PathBuf};

use serde_json::Value;

use ought_spec::SpecGraph;

use crate::{collect_clauses, count_clauses};

/// Handler for MCP resource requests.
///
/// Resources are read-only views into ought's state.
pub struct ResourceHandler {
    /// Project root; relative paths in `spec_roots` and internal artifact
    /// locations (manifest, results) are resolved against this.
    project_root: PathBuf,
    /// Spec roots resolved by the caller (may be absolute or
    /// project-root-relative).
    spec_roots: Vec<PathBuf>,
}

impl ResourceHandler {
    pub fn new(project_root: PathBuf, spec_roots: Vec<PathBuf>) -> Self {
        Self { project_root, spec_roots }
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

    /// `ought://specs` -- list all spec files with clause counts.
    pub fn specs_list(&self) -> anyhow::Result<Value> {
        let specs = self.load_specs()?;

        let list: Vec<Value> = specs
            .specs()
            .iter()
            .map(|spec| {
                let clause_count = count_clauses(&spec.sections);
                serde_json::json!({
                    "name": spec.name,
                    "path": spec.source_path.display().to_string(),
                    "sections": spec.sections.len(),
                    "clauses": clause_count,
                })
            })
            .collect();

        Ok(serde_json::json!({
            "specs": list,
            "total": list.len(),
        }))
    }

    /// `ought://specs/{name}` -- parsed clauses for a specific spec.
    pub fn specs_get(&self, name: &str) -> anyhow::Result<Value> {
        let specs = self.load_specs()?;

        let spec = specs
            .specs()
            .iter()
            .find(|s| s.name == name || s.source_path.display().to_string().contains(name))
            .ok_or_else(|| anyhow::anyhow!("spec not found: {}", name))?;

        let mut clauses_json = Vec::new();
        for section in &spec.sections {
            for clause in collect_clauses(section) {
                clauses_json.push(serde_json::json!({
                    "id": clause.id.0,
                    "keyword": format!("{:?}", clause.keyword),
                    "severity": format!("{:?}", clause.severity),
                    "text": clause.text,
                    "condition": clause.condition,
                    "content_hash": clause.content_hash,
                    "source_location": {
                        "file": clause.source_location.file.display().to_string(),
                        "line": clause.source_location.line,
                    },
                }));
            }
        }

        Ok(serde_json::json!({
            "name": spec.name,
            "path": spec.source_path.display().to_string(),
            "context": spec.metadata.context,
            "sources": spec.metadata.sources,
            "clauses": clauses_json,
        }))
    }

    /// `ought://results/latest` -- most recent run results.
    pub fn results_latest(&self) -> anyhow::Result<Value> {
        // Look for a results file in the project
        let results_path = self.base().join("ought/results/latest.json");
        if results_path.exists() {
            let content = std::fs::read_to_string(&results_path)?;
            let value: Value = serde_json::from_str(&content)?;
            return Ok(value);
        }

        // No results file found -- return empty results
        Ok(serde_json::json!({
            "results": [],
            "message": "No results found. Run `ought run` first.",
        }))
    }

    /// `ought://coverage` -- clause coverage map.
    pub fn coverage(&self) -> anyhow::Result<Value> {
        let specs = self.load_specs()?;

        let manifest_path = self.base().join("ought/ought-gen/manifest.toml");
        let manifest = ought_gen::Manifest::load(&manifest_path)?;

        let mut coverage_entries = Vec::new();
        let mut total = 0usize;
        let mut covered = 0usize;

        for spec in specs.specs() {
            for section in &spec.sections {
                for clause in collect_clauses(section) {
                    total += 1;
                    let has_test = manifest.entries.contains_key(&clause.id.0);
                    let is_stale = if has_test {
                        manifest.is_stale(&clause.id, &clause.content_hash, "")
                    } else {
                        false
                    };

                    if has_test && !is_stale {
                        covered += 1;
                    }

                    coverage_entries.push(serde_json::json!({
                        "clause_id": clause.id.0,
                        "keyword": format!("{:?}", clause.keyword),
                        "has_test": has_test,
                        "is_stale": is_stale,
                        "spec": spec.name,
                    }));
                }
            }
        }

        let coverage_pct = if total > 0 {
            (covered as f64 / total as f64) * 100.0
        } else {
            0.0
        };

        Ok(serde_json::json!({
            "total_clauses": total,
            "covered": covered,
            "coverage_percent": coverage_pct,
            "clauses": coverage_entries,
        }))
    }

    /// `ought://manifest` -- generation manifest with hashes and staleness.
    pub fn manifest(&self) -> anyhow::Result<Value> {
        let manifest_path = self.base().join("ought/ought-gen/manifest.toml");
        let manifest = ought_gen::Manifest::load(&manifest_path)?;

        let entries: Vec<Value> = manifest
            .entries
            .iter()
            .map(|(id, entry)| {
                serde_json::json!({
                    "clause_id": id,
                    "clause_hash": entry.clause_hash,
                    "source_hash": entry.source_hash,
                    "generated_at": entry.generated_at.to_rfc3339(),
                    "model": entry.model,
                })
            })
            .collect();

        Ok(serde_json::json!({
            "entries": entries,
            "total": entries.len(),
        }))
    }
}
