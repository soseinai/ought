# Ought

context: Ought is a test framework that separates test intent from test implementation. Developers write high-level behavioral specifications in markdown. An LLM generates concrete test code. The tool executes those tests and reports results mapped back to the original spec clauses. The name comes from Hume's is-ought gap — the spec is the "ought" world, the source code is the "is" world, and testing is detecting when they diverge.

## What Ought Does

- **MUST** accept behavioral specifications written in standard markdown files (`.ought.md`)
- **MUST** use an LLM to generate concrete, runnable test code from those specifications
- **MUST** execute generated tests and report pass/fail results mapped back to the original spec clauses
- **MUST** provide a CLI (`ought`) as the primary interface for all operations
- **MUST NOT** require users to write any test code by hand

## Spec Format

The spec format is the heart of ought — the part that becomes the open standard.

- **MUST** use standard markdown (CommonMark) so specs render in GitHub, editors, and browsers with zero tooling
- **MUST** support RFC 2119 keywords (**MUST**, **MUST NOT**, **SHOULD**, **SHOULD NOT**, **MAY**) as deontic operators
- **MUST** support the **WONT** keyword for deliberately absent capabilities
- **MUST** support **GIVEN** blocks for conditional obligations (clauses that only apply when a precondition holds)
- **MUST** support **OTHERWISE** chains for contrary-to-duty fallbacks (graceful degradation when an obligation is violated)
- **MUST** support **MUST ALWAYS** for invariants (properties that must hold across all states and inputs)
- **MUST** support **MUST BY** for deadline obligations (operations that must complete within a time bound)
- **MUST** support cross-file references via standard markdown links, so specs can link to each other and form a hierarchy
- **SHOULD** be parseable by a standalone library with no LLM dependency, so other tools can consume the format

Details: [Spec Format](./engine/parser.ought.md)

## Language Agnostic

- **MUST** be agnostic to the programming language of the project under test
- **MUST** delegate test execution to the project's existing test harness (cargo test, pytest, jest, go test, etc.)
- **MUST** ship with runners for at least Rust and one other mainstream language
- **SHOULD** support custom runners via configuration
- **MUST NOT** require any language-specific SDK or library in the project under test

Details: [Runner](./engine/runner.ought.md)

## LLM Agnostic

- **MUST** be agnostic to which LLM provider generates the test code
- **MUST** support at least Anthropic (Claude) and OpenAI as providers
- **SHOULD** support local models via Ollama
- **MUST** allow the provider and model to be configured in `ought.toml`
- **MUST NOT** depend on any provider-specific features in the core spec format or runner

Details: [Generator](./engine/generator.ought.md)

## Generated Test Management

- **MUST** track generated tests with content hashes so they are only regenerated when the spec or source changes
- **MUST** only regenerate tests when the user explicitly runs `ought generate` (never during `ought run`)
- **MUST** detect and remove orphaned tests when a clause is deleted from a spec

Details: [Generator — Manifest and Hashing](./engine/generator.ought.md#manifest-and-hashing)

## Reporting

- **MUST** map test results back to the original spec clauses (not just test function names)
- **MUST** distinguish failure severity — MUST failures are errors, SHOULD failures are warnings
- **MUST** produce visually attractive terminal output that makes specs and their status easy to scan
- **SHOULD** support LLM-powered failure diagnosis that explains *why* a test failed in terms of the source code
- **SHOULD** support LLM-powered test quality grading that evaluates whether generated tests actually validate their clauses

Details: [Reporter](./engine/reporter.ought.md)

## LLM-Powered Analysis

Beyond generating and running tests, ought uses LLMs to reason about the relationships between specs, source code, and results.

- **MUST** support surveying source code to discover behaviors not covered by any spec (`ought survey`)
- **MUST** support auditing specs for contradictions, gaps, and coherence issues (`ought audit`)
- **MUST** support blaming a failure on a specific source change with a causal narrative (`ought blame`)
- **SHOULD** support bisecting git history to find the exact commit that broke a clause (`ought bisect`)

Details: [Analysis](./analysis/analysis.ought.md)

## Integration

- **MUST** provide an MCP server so AI assistants and IDE extensions can interact with ought programmatically
- **MUST** be easy to integrate into CI pipelines (run without LLM access, gate on staleness separately)
- **SHOULD** provide a GitHub Action for PR-level reporting
- **SHOULD** be installable via cargo, Homebrew, and as a standalone binary

Details: [MCP Server](./integration/mcp.ought.md), [CLI](./cli/cli.ought.md)

## Implementation

- **MUST** be written in Rust
- **MUST** publish the spec parser as a standalone crate (`ought-spec`) with no LLM dependencies
- **SHOULD** use a workspace structure so components can be used independently
