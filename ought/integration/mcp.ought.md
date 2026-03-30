# MCP Server

context: Ought exposes an MCP (Model Context Protocol) server so that AI assistants and IDE extensions can interact with ought specs, results, and analysis features programmatically. Launched via `ought mcp serve`. Supports stdio and SSE transports.

source: src/mcp/

requires: [cli](../cli/cli.ought.md), [parser](../engine/parser.ought.md), [runner](../engine/runner.ought.md), [analysis](../analysis/analysis.ought.md)

## Server Lifecycle

- **MUST** start the MCP server via `ought mcp serve`
- **MUST** support stdio transport (default, for local IDE integration)
- **MUST** support SSE transport via `--transport sse --port <port>` for remote clients
- **MUST** advertise all available tools and resources on initialization
- **MUST** shut down cleanly on SIGTERM or client disconnect
- **SHOULD** support `ought mcp install` to auto-register with MCP-compatible coding agents (Claude Code, Codex, OpenCode)

## Tools

- **MUST** expose `ought_run` — run specs and return structured results (accepts optional spec path filter)
- **MUST** expose `ought_generate` — regenerate stale or specified clauses
- **MUST** expose `ought_check` — validate spec syntax
- **MUST** expose `ought_inspect` — return generated test code for a clause
- **MUST** expose `ought_status` — return spec coverage summary (clause counts by severity and status)
- **MUST** expose `ought_survey` — analyze source for uncovered behaviors
- **MUST** expose `ought_audit` — cross-spec conflict and gap analysis
- **MUST** expose `ought_blame` — explain why a clause is failing
- **MUST** return structured JSON responses from all tools (not terminal-formatted text)
- **SHOULD** expose `ought_bisect` — find the breaking commit for a clause
- **SHOULD** include execution duration and timestamp in tool responses

## Resources

- **MUST** expose `ought://specs` — list of all spec files with their clause counts
- **MUST** expose `ought://specs/{name}` — parsed clauses for a specific spec file
- **MUST** expose `ought://results/latest` — results from the most recent run
- **MUST** expose `ought://coverage` — clause coverage map (which clauses have tests, pass/fail status)
- **SHOULD** expose `ought://manifest` — current generation manifest (hashes, timestamps, staleness)
- **SHOULD** support resource subscriptions so clients get notified when results change

## Error Handling

- **MUST** return MCP-compliant error responses with error codes and messages
- **MUST NOT** crash the server on a single tool invocation failure
- **MUST ALWAYS** return valid JSON-RPC responses, even for internal errors
- **MUST ALWAYS** remain responsive to new requests while processing long-running tools (survey, audit, bisect)
- **SHOULD** include actionable error details (e.g. "`claude` CLI not found — install it with `brew install claude`" not just "generation failed")
- **GIVEN** a tool invocation exceeds 60 seconds:
  - **SHOULD** send progress notifications to the client
  - **OTHERWISE** the client may assume the request has timed out
