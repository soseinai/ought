use serde_json::Value;

/// Handler for MCP resource requests.
///
/// Resources are read-only views into ought's state.
pub struct ResourceHandler {
    // will hold references to config, specs, manifest, etc.
}

impl ResourceHandler {
    /// `ought://specs` — list all spec files with clause counts.
    pub fn specs_list(&self) -> anyhow::Result<Value> {
        todo!()
    }

    /// `ought://specs/{name}` — parsed clauses for a specific spec.
    pub fn specs_get(&self, _name: &str) -> anyhow::Result<Value> {
        todo!()
    }

    /// `ought://results/latest` — most recent run results.
    pub fn results_latest(&self) -> anyhow::Result<Value> {
        todo!()
    }

    /// `ought://coverage` — clause coverage map.
    pub fn coverage(&self) -> anyhow::Result<Value> {
        todo!()
    }

    /// `ought://manifest` — generation manifest with hashes and staleness.
    pub fn manifest(&self) -> anyhow::Result<Value> {
        todo!()
    }
}
