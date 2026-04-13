# ought-mcp

Model Context Protocol (MCP) server for ought.

Exposes ought's functionality — running specs, generating tests, inspecting
clauses, surveying source, auditing specs — as MCP tools and resources so
that AI assistants and IDE extensions can interact with ought programmatically
over stdio or SSE.

The server is a pure protocol layer: it receives project context from its
caller (via `McpServer::new`) rather than loading `ought.toml` itself.
Loading config from disk is the CLI's responsibility.

## Responsibilities

- Speak JSON-RPC 2.0 over stdio, routing `tools/*` and `resources/*` methods
  to `ToolHandler` and `ResourceHandler`.
- Advertise the ought tool set (`ought_run`, `ought_generate`, `ought_check`,
  `ought_inspect`, `ought_status`, `ought_survey`, `ought_audit`,
  `ought_blame`, `ought_bisect`) and resource URIs (`ought://specs`,
  `ought://specs/{name}`, `ought://results/latest`, `ought://coverage`,
  `ought://manifest`).
- Provide a separate "generation-mode" server (`GenMcpServer`) that agents
  connect to during a single `ought generate` run to read source, write
  tests, and check compilation.
- Install ought as an MCP server for Claude Code / compatible agents.

## Notable public API

- `McpServer::new(project_root, spec_roots, runners)` / `serve(transport)`
  — the normal server, driven by pre-resolved config from the caller.
- `McpServer::install()` — register ought in `~/.claude/mcp.json` (and
  equivalents) so agents can find it.
- `ToolHandler`, `ResourceHandler` — the underlying JSON-producing handlers
  used directly in tests.
- `GenMcpServer::from_assignment_path(path)` / `serve_stdio()` — the
  agent-facing server used by the orchestrator during generation.
- `Transport::{Stdio, Sse { port }}` — transport selector.
- `McpConfig { enabled, transport }` — `[mcp]` sub-config.
