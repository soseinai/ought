# Ought; Behavioral specs that test themselves

Ought separates **test intent** from **test implementation**. You write what your system *ought* to do in plain markdown. An LLM generates the tests. You run them.

## The Problem

Today, test intent and test implementation are fused together in code. The assertion `assert_eq!(response.status(), 401)` buries the intent -- "invalid credentials must return 401" -- inside mechanical setup and plumbing. When the test fails, you see a stack trace, not the requirement that was violated. When requirements change, you rewrite test code instead of updating a sentence.

Ought pulls intent up into a human-readable spec and delegates the mechanical work to an LLM.

## What Ought Does

Ought maintains a three-way sync between intent, source code, and tests -- with the LLM as the mediator:

```
            Intent (.ought.md)
               /         \
        survey/   audit    \generate
            /               \
      Source Code <--blame--> Tests
            \               /
        diagnose\         /grade
                 \       /
              Results + Reports
```

Every arrow is LLM-powered. Traditional test tools only have the bottom triangle (code, tests, results). Ought adds the intent layer and connects everything through it.

## Quick Start

### Install

```
cargo install ought
brew install ought
# or download a binary from GitHub releases
```

### Initialize

```
ought init
```

This creates an `ought/` directory with a sample spec and an `ought.toml` config file.

### Write a spec

Edit `ought/myapp.ought.md`:

```markdown
# User Authentication

context: REST API at /api/auth, uses JWT tokens
source: src/auth/

## Login

- **MUST** return a valid JWT token when given correct credentials
- **MUST** return 401 with a generic error when credentials are invalid
- **MUST NOT** leak timing differences between valid and invalid usernames
- **MUST BY 200ms** return a response under normal load
  - **OTHERWISE** return a cached session token
  - **OTHERWISE** return 503 with a Retry-After header
- **SHOULD** rate-limit to 5 attempts per minute per IP
- **WONT** support basic auth (deprecated in v2)

## Token Refresh

- **GIVEN** the refresh token is valid and not expired:
  - **MUST** issue a new access token
  - **SHOULD** rotate the refresh token (one-time use)
- **GIVEN** the refresh token is expired:
  - **MUST** return 401
  - **MUST** include a `WWW-Authenticate` header

## Invariants

- **MUST ALWAYS** return valid JSON from all endpoints
- **MUST ALWAYS** include a `X-Request-Id` header in every response
```

### Generate tests

```
ought generate
```

The LLM reads your spec and source code, then writes concrete test files into `ought/ought-gen/`.

### Run

```
ought run
```

Output:

```
 Authentication API          myapp.ought.md
 ------------------------------------------------
 Login
   ✓ MUST    return valid JWT on correct credentials
   ✗ MUST    return 401 on invalid credentials
   ✓ MUST    NOT leak timing differences
   ✓ MUST BY 200ms return a response              [47ms / 200ms]
     ↳ ~ OTHERWISE return cached session           (not reached)
     ↳ ~ OTHERWISE return 503                      (not reached)
   ✓ SHOULD  rate-limit to 5 attempts/min/ip
   ⊘ WONT   support basic auth                    (confirmed absent)

 Token Refresh
   GIVEN the refresh token is valid and not expired:
     ✓ MUST    issue a new access token
     ✓ SHOULD  rotate the refresh token
   GIVEN the refresh token is expired:
     ✓ MUST    return 401
     ✓ MUST    include WWW-Authenticate header

 Invariants
   ✓ MUST ALWAYS return valid JSON                 (tested 1000 inputs)
   ✓ MUST ALWAYS include X-Request-Id header       (tested 1000 inputs)

 11 passed · 1 failed · 1 confirmed absent
 MUST coverage: 8/9 (89%)
```

## Spec Format

Spec files are standard CommonMark markdown with the `.ought.md` extension. They render in GitHub, display in any editor, and require no special tooling to read. The formal grammar is defined in [docs/grammar.md](docs/grammar.md) — that file is the source of truth for what the parser accepts.

**Structure:**

- **H1** (`#`) -- spec name, one per file
- **H2+** (`##`, `###`) -- sections, map to test groups
- **Bullet points** (`- **KEYWORD** ...`) -- clauses, the testable units
- **Bold keywords** (`**MUST**`) -- deontic operators (bare "must" in prose is ignored)
- **Prose** between clauses -- context for humans and the LLM, not parsed as clauses
- **Code blocks** after a clause -- hints for the LLM (example payloads, schemas, etc.)

**Metadata** appears below the H1:

```markdown
# My Service

context: REST API using JWT tokens
source: src/auth/, src/models/user.rs
schema: db/migrations/
requires: [users](./users.ought.md)
```

| Key | Purpose |
|---|---|
| `context:` | Free-text context for the LLM |
| `source:` | Source code paths (hints for LLM context assembly) |
| `schema:` | Schema, config, or migration files |
| `requires:` | Dependencies on other spec files (builds a DAG) |

Specs are hierarchical. A top-level spec captures broad product-level requirements, linking down to detail specs that flesh out specifics via `Details:` annotations and `requires:` links.

## Keywords Reference

### Standard obligations

| Keyword | Severity | On failure | Exit code |
|---|---|---|---|
| **MUST** | required | error | 1 |
| **MUST NOT** | required | error | 1 |
| **SHOULD** | recommended | warning | 0 (1 with `--fail-on-should`) |
| **SHOULD NOT** | recommended | warning | 0 |
| **MAY** | optional | info | 0 |
| **WONT** | negative | error if present | 1 |

### Deontic extensions

| Keyword | What it does |
|---|---|
| **GIVEN** | Conditional block. Nested clauses only apply when the precondition holds. Not itself testable. |
| **OTHERWISE** | Contrary-to-duty fallback. Nested under an obligation, forms an ordered degradation chain. If the parent fails but an OTHERWISE passes, the overall result is a pass. |
| **MUST ALWAYS** | Invariant. Must hold across all states, inputs, and time. Generates property-based / fuzz tests. |
| **MUST BY** | Deadline. Must complete within a time bound (e.g., `**MUST BY 200ms**`). Duration suffixes: `ms`, `s`, `m`. |

## CLI Reference

| Command | Description |
|---|---|
| `ought init` | Scaffold `ought.toml` and an example spec |
| `ought generate` | Regenerate tests for stale clauses |
| `ought generate --force` | Regenerate all tests |
| `ought generate --check` | Exit 1 if any clause is stale (CI gate) |
| `ought run` | Execute tests, report results mapped to clauses |
| `ought run --diagnose` | Add LLM-powered failure narratives |
| `ought run --grade` | Add LLM-powered test quality grading |
| `ought check` | Validate spec syntax only (no LLM, no execution) |
| `ought inspect <clause>` | Show generated test code for a clause |
| `ought diff` | Show pending generation changes |
| `ought survey [path]` | Discover source behaviors not covered by any spec |
| `ought audit` | Cross-spec coherence analysis (contradictions, gaps) |
| `ought blame <clause>` | Explain a failure with git history context |
| `ought bisect <clause>` | Find the exact commit that broke a clause |
| `ought watch` | Re-run on file changes |
| `ought mcp serve` | Start the MCP server |
| `ought mcp install` | Register with Claude Code, Codex, OpenCode |

**Exit codes:** 0 = success (or only SHOULD/MAY failures), 1 = MUST-level failure, 2 = usage error.

**Global flags:** `--config`, `--quiet`, `--json`, `--junit <path>`, `--color`, `--verbose`.

## How It Works

The engine has four phases. **Parse** converts `.ought.md` files into a structured clause IR using a pure-Rust parser with zero LLM dependency. **Generate** takes the clause IR plus source code context and uses an LLM to produce concrete, idiomatic test files. **Execute** delegates to the project's existing test harness (cargo test, pytest, jest, go test) and collects per-test results. **Report** maps results back to spec clauses and renders them in the terminal with severity-appropriate formatting.

## LLM Providers

Ought invokes LLM CLIs directly by exec-ing `claude`, `chatgpt`, or `ollama` -- no API keys to manage in ought itself. Use your consumer account, pro plan, or API key as you normally would with the CLI tool.

Configure the provider in `ought.toml`:

```toml
[generator]
provider = "anthropic"       # or "openai", "ollama"
model = "claude-sonnet-4-6"
```

Custom providers are supported by specifying an arbitrary executable.

## Analysis Commands

Beyond test generation and execution, ought uses LLMs to reason about relationships between specs, source code, and results.

**`ought survey [path]`** -- Scans source code and identifies behaviors not covered by any spec. Suggests concrete clauses with appropriate keywords. Never auto-adds clauses without user confirmation.

**`ought audit`** -- Checks specs for contradictions (conflicting MUSTs), deadline conflicts (a MUST BY calling a sub-operation with a longer deadline), MUST ALWAYS invariant conflicts, gaps (e.g., login and refresh specified but no logout), and missing OTHERWISE chains on network-dependent operations.

**`ought blame <clause>`** -- Correlates a failing clause with git history to build a causal narrative: what commit broke it, who authored it, and what the change was trying to do.

**`ought bisect <clause>`** -- Automated binary search through git history to find the exact breaking commit. Like `git bisect` but targeted at a specific clause. Always restores the working tree afterward.

## MCP Server

Ought exposes an MCP (Model Context Protocol) server for AI assistants and IDE extensions. Running `ought mcp serve` starts a stdio-based server that exposes tools (`ought_run`, `ought_generate`, `ought_survey`, `ought_audit`, `ought_blame`, `ought_bisect`) and resources (`ought://specs`, `ought://results/latest`, `ought://coverage`, `ought://manifest`). This lets tools like Claude Code, Codex, and OpenCode interact with your specs and results programmatically. Install with `ought mcp install`.

## Configuration

`ought.toml` in the project root:

```toml
[project]
name = "myapp"
version = "0.1.0"

[specs]
roots = ["ought/"]

[context]
search_paths = ["src/", "lib/"]
exclude = ["vendor/", "generated/"]
max_files = 50

[generator]
provider = "anthropic"
model = "claude-sonnet-4-6"

[generator.tolerance]
must_by_multiplier = 1.5     # CI timing tolerance for MUST BY

[runner.rust]
command = "cargo test"
test_dir = "ought/ought-gen/"

[runner.python]
command = "pytest"
test_dir = "ought/ought-gen/"

[mcp]
enabled = true
transport = "stdio"
```

## Philosophy

The spec language is grounded in deontic logic -- the formal logic of obligation, permission, and prohibition. The keywords are not arbitrary labels. MUST and MUST NOT are obligations. SHOULD is a prima facie duty (Ross). MAY is permission. GIVEN models conditional obligation from dyadic deontic logic. OTHERWISE models contrary-to-duty obligations (Chisholm's paradox). MUST ALWAYS and MUST BY draw from temporal deontic logic.

The name comes from Hume's is-ought gap (1739): you cannot derive an "ought" from an "is." The spec says what the system *ought* to do. The source code says what it *does*. Testing is detecting when they diverge. Ought lives in that gap.

See [docs/design.md](docs/design.md) for the full design document.

## Contributing

Ought is written in Rust and structured as a Cargo workspace:

```
crates/
  ought-spec/        # parser + clause IR (the open standard)
  ought-gen/         # generator trait + providers
  ought-run/         # runner trait + language runners
  ought-report/      # reporter + TUI
  ought-analysis/    # survey, audit, blame, bisect
  ought-mcp/         # MCP server
  ought-cli/         # CLI binary
```

`ought-spec` has zero dependencies on LLM infrastructure and is published separately for ecosystem interop.

## License

MIT
