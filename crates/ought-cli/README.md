# ought-cli

The `ought` command-line binary.

Wires the rest of the workspace together: loads `ought.toml`, resolves the
sub-configs each domain crate needs, and drives commands like `check`,
`generate`, `run`, `watch`, `view`, `survey`, `audit`, `blame`, `bisect`,
and `mcp`.

On-disk configuration is a CLI concern — this crate owns the aggregate
`Config` struct and the TOML loading. Every other crate defines only the
sub-config shape it cares about; the CLI composes them.

## Responsibilities

- Parse CLI flags (clap), locate `ought.toml` (explicit `--config` or
  discovery by walking upward), and dispatch to per-command handlers.
- Pass each subsystem only the pieces it needs — e.g. `ought-mcp` gets
  `(project_root, spec_roots, runners)`, `ought-gen::Orchestrator` gets
  `&GeneratorConfig`.
- Package the results from `ought-run` / `ought-analysis` for reporting
  via `ought-report` (terminal / JSON / JUnit).

## Notable public API

The primary output is the `ought` binary. A small `[lib]` target
(`ought_cli`) exposes just enough for integration tests:

- `ought_cli::config::Config` — aggregate `ought.toml` schema.
- `ought_cli::config::ProjectConfig` — `[project]` sub-config (name,
  version).
- `Config::load(path)` / `Config::discover()` — load from an explicit path
  or walk up from the current directory to find `ought.toml`.
