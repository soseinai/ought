# ought-server

Web viewer and HTTP server for ought specs.

Builds a local HTTP server backed by the embedded Svelte UI (built into
`dist/` via Vite, embedded with `rust-embed` so the published crate is
self-contained). Serves the parsed spec graph, a search index, and the
"proofs" — the generated test code — alongside each clause.

Like `ought-mcp`, this crate does not load `ought.toml`; the CLI passes in
the resolved project context.

## Responsibilities

- Parse specs from the provided roots and serve them as JSON at
  `/api/specs` along with proof code extracted from runner test dirs.
- Build an in-memory `SearchIndex` over clauses and expose `/api/search`.
- Serve the embedded SPA as a static fallback for client-side routing.
- Optionally open the browser on start.

## Notable public API

- `serve(project_root, spec_roots, runners, port, open_browser)` — start
  the axum server; blocks until shutdown.
- `ProofIndex::build(&runners, &project_root)` — walk each runner's
  `test_dir`, extract per-clause `Proof`s (name, summary, code, language).
- `SearchIndex::build(specs)` / `search(query, limit)` — ranked clause
  search used by the viewer.
- `Proof`, `ProofIndex` — the types surfaced to the UI.
