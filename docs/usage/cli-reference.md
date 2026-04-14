---
title: CLI reference
description: Every ought subcommand and what it does.
order: 6
---

Every Ought workflow is driven through the `ought` CLI. Run `ought help` or `ought <command> --help` for the full flag listing.

## Global flags

These work on every subcommand:

- `--config <path>` — use a specific `ought.toml` instead of the one discovered from the working directory.
- `--quiet` — suppress all output except errors and the final summary.
- `--json` — emit structured JSON instead of human-readable text.
- `--junit <path>` — write JUnit XML results to the given file.
- `--color <auto|always|never>` — control terminal color.
- `--verbose` — enable debug-level output.

## ought init

Scaffold `ought.toml` and an example spec in the current directory.

```sh
ought init
```

## ought generate

Read your specs and source code and write test files. Tests are annotated with the clause they enforce, so failures map back to specific lines in the spec.

```sh
ought generate                          # regenerate stale clauses across all specs
ought generate specs/auth.ought.md      # limit to one spec (positional path or glob)
ought generate --force                  # regenerate every clause regardless of hash
ought generate --check                  # exit 1 if any generated tests are stale (for CI)
```

## ought run

Execute the generated tests and produce a clause-mapped report.

```sh
ought run                               # run everything
ought run specs/auth.ought.md           # run tests from matching specs
ought run --fail-on-should              # exit 1 on SHOULD failures too (default: MUST only)
```

## ought check

Validate spec file syntax without generating or running tests.

```sh
ought check
```

## ought inspect

Show the generated test code for a specific clause.

```sh
ought inspect auth::login::must_return_jwt
```

## ought diff

Show the diff between the current generated tests and what would be produced by a fresh generate.

```sh
ought diff
```

## ought watch

Watch for file changes and re-run affected specs.

```sh
ought watch
```

## ought view

Launch the visual spec viewer in the browser.

```sh
ought view                              # serves on :3333, opens the browser
ought view --port 8080
ought view --no-open
```

## ought analyze

Spec-level analysis commands.

### ought analyze audit

Detect contradictions, gaps, and coherence issues across specs.

```sh
ought analyze audit
```

### ought analyze survey

Find behaviors in your source code that are not covered by any spec clause. Useful for catching test gaps in legacy code.

```sh
ought analyze survey
ought analyze survey src/auth/
```

## ought debug

Investigate failing clauses with git history.

### ought debug blame

Explain why a clause is failing by correlating with recent commits.

```sh
ought debug blame auth::login::must_return_jwt
```

### ought debug bisect

Binary-search git history to find the commit that broke a clause.

```sh
ought debug bisect auth::login::must_return_jwt
ought debug bisect auth::login::must_return_jwt --range abc123..def456
ought debug bisect auth::login::must_return_jwt --regenerate
```

`--regenerate` regenerates tests at each commit instead of reusing the current manifest — slower but accurate when the spec itself has changed across the range.

## ought mcp

Model Context Protocol server commands.

### ought mcp serve

Start the MCP server.

```sh
ought mcp serve                                    # stdio transport (for local IDE integration)
ought mcp serve --transport sse --port 8765       # SSE transport for remote clients
ought mcp serve --mode generation --assignment path/to/assignment.json
```

### ought mcp install

Register ought with MCP-compatible coding agents.

```sh
ought mcp install
```
