# ought-gen

LLM-powered test generation for ought specs.

Turns a set of stale/new clauses into generated test code by spawning agent
processes (Claude, Codex, etc.) that connect back to ought's MCP server and
drive generation through tool calls. Tracks what has been generated via a
content-hashed manifest so re-runs only touch clauses that actually changed.

## Responsibilities

- Decide which clauses need (re-)generation by comparing clause and source
  hashes against the on-disk manifest.
- Partition work into agent assignments and spawn the configured agent CLI
  with per-run MCP configuration.
- Persist generation state to `ought/ought-gen/manifest.toml` so subsequent
  runs are incremental.
- Own the generator sub-configs (`GeneratorConfig`, `ToleranceConfig`).

## Notable public API

- `Orchestrator::new(&GeneratorConfig, verbose)` / `Orchestrator::run(assignments)`
  — spawn and supervise agent processes, collect `AgentReport`s.
- `Manifest` / `ManifestEntry` — persisted hashes, timestamps, and model
  metadata; `Manifest::is_stale(&clause_id, &clause_hash, &source_hash)`.
- `AgentAssignment`, `AssignmentGroup`, `AssignmentClause`, `AgentReport` —
  the serializable payload exchanged with agents.
- `GeneratedTest`, `Language`, `keyword_str(kw)` — shared test-representation
  types used by runners and the MCP generation tools.
- `GeneratorConfig { provider, model, tolerance, parallelism }`,
  `ToleranceConfig { must_by_multiplier }` — `[generator]` sub-config.
