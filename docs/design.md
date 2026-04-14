# Ought — Design Document

## Vision

Ought separates **test intent** from **test implementation**. Today these are fused together in code — the "what should be true" is buried inside assertion calls and fixture setup. Ought pulls intent up into a human-readable spec and delegates the mechanical implementation to an LLM.

The result is a three-way sync between intent, implementation, and verification — with the LLM as the mediator:

```
            Intent (.ought.md)
               ╱         ╲
        survey╱   audit    ╲generate
            ╱               ╲
      Source Code ◄──blame──► Tests
            ╲               ╱
        diagnose╲         ╱grade
                 ╲       ╱
              Results + Reports
```

Every arrow is LLM-powered. Today's test tools only have the bottom triangle (code, tests, results). Ought adds the intent layer and connects everything through it.

The name comes from philosophy. Hume's is-ought gap (1739) observes that you cannot derive an "ought" from an "is." A spec says what the system *ought* to do. The source code says what it *does*. Testing is detecting when they diverge. Ought lives in that gap.

## Philosophical Foundations

The spec language is grounded in deontic logic — the formal logic of obligation, permission, and prohibition. This is not decorative. Deontic concepts map to real, implementable features that make specs more expressive than anything currently available.

| Concept | Origin | Keyword | What It Does |
|---|---|---|---|
| Obligation | Standard deontic logic | **MUST** | Absolute requirement. Test failure = exit 1. |
| Prohibition | Standard deontic logic | **MUST NOT** | Absolute prohibition. Same severity as MUST. |
| Weak obligation | Ross's prima facie duties | **SHOULD** / **SHOULD NOT** | Required unless valid reason exists. Warning, not failure. |
| Permission | Standard deontic logic | **MAY** | Truly optional. Tracked but never a failure. |
| Negative confirmation | Ought extension | **WONT** | Deliberately absent capability. Generates tests confirming absence or graceful rejection. |
| Conditional obligation | Dyadic deontic logic | **GIVEN** | Precondition that scopes nested clauses. O(p \| q) — "p is obligated, given q." |
| Contrary-to-duty | Chisholm's paradox | **OTHERWISE** | Fallback when parent obligation is violated. Models graceful degradation. |
| Invariant | Temporal deontic logic | **MUST ALWAYS** | Must hold across all states, inputs, and time. Generates property-based tests. |
| Deadline | Temporal deontic logic | **MUST BY** | Must be fulfilled within a time bound. Generates timed assertions. |

### Kant's "Ought Implies Can"

If you're obligated to do something, it must be possible. This maps directly to spec satisfiability checking in `ought analyze audit` — detect contradictory MUSTs, deadline conflicts, and invariants that can't simultaneously hold.

## Spec Format: `.ought.md`

Spec files use standard Markdown (CommonMark) with conventions layered on top. This means they render in GitHub, display in any editor, and require no special tooling to read. Files use the `.ought.md` extension.

### Hierarchical Organization

Specs are organized hierarchically — broad strokes at the top, fine-grained details in sub-files and sub-folders. The top-level spec captures *what* the system does at a product level. Detail specs flesh out *how* each component works.

```
ought/
  ought.ought.md                   # broad strokes — the product-level "what"
  engine/
    parser.ought.md                # how spec parsing works
    generator.ought.md             # how LLM generation works
    runner.ought.md                # how test execution works
    reporter.ought.md              # how reporting/TUI works
  analysis/
    analysis.ought.md              # how survey, audit, blame, bisect work
  cli/
    cli.ought.md                   # CLI commands, flags, exit codes
  integration/
    mcp.ought.md                   # MCP server tools and resources
```

The top-level spec links down to details using standard markdown links in `Details:` annotations. Detail specs link laterally to each other via `requires:`. This forms a natural DAG — broad requirements at the root, specifics at the leaves.

Teams own their structure. Some may prefer one spec per module, others per feature area or per service. The format supports any hierarchy depth. What matters is that:

1. A reader can start at the top level and get the full picture in broad strokes
2. They can drill into any area for specifics by following links
3. Cross-cutting concerns link across the hierarchy via `requires:` and inline references

### File Structure

```markdown
# Authentication API

context: REST API at `/api/auth`, uses JWT tokens, backed by PostgreSQL
source: src/auth/
schema: db/migrations/
requires: [users](./users.ought.md)

## Login

Handles credential validation and token issuance.

- **MUST** return a valid JWT token when given correct credentials
- **MUST** return 401 with a generic error when credentials are invalid
- **MUST NOT** leak timing differences between valid and invalid usernames
- **MUST BY 200ms** return a response under normal load
  - **OTHERWISE** return a cached session token
  - **OTHERWISE** return 503 with a Retry-After header
- **SHOULD** rate-limit to 5 attempts per minute per IP
- **MAY** support "remember me" extended token expiry
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

### Anatomy

- **H1** (`#`) — spec name. One per file.
- **H2+** (`##`, `###`) — sections. Map to test groups. Nest arbitrarily.
- **Bullet points** (`- **KEYWORD**`) — clauses. The testable units.
- **Bold keywords** (`**MUST**`) — deontic operators. Must be bold to be recognized; bare "must" in prose is ignored.
- **Prose** between clauses — context for humans and the LLM. Not parsed as clauses.
- **Code blocks** after a clause — hints for the LLM (example payloads, schemas, etc).

### Metadata

Appears at the top of the file, below the H1:

| Key | Purpose | Example |
|---|---|---|
| `context:` | Free-text context for the LLM | `context: REST API using JWT` |
| `source:` | Source code paths (hints for LLM context assembly) | `source: src/auth/, src/models/user.rs` |
| `schema:` | Schema/config/migration files | `schema: db/migrations/` |
| `requires:` | Dependencies on other spec files (builds a DAG) | `requires: [auth](./auth.ought.md)` |

### Keywords Reference

#### Standard obligations

| Keyword | Severity | Test failure | Exit code |
|---|---|---|---|
| **MUST** | required | error | 1 |
| **MUST NOT** | required | error | 1 |
| **SHOULD** | recommended | warning | 0 (1 with `--fail-on-should`) |
| **SHOULD NOT** | recommended | warning | 0 |
| **MAY** | optional | info | 0 |
| **WONT** | negative | error if present | 1 |

#### Deontic extensions

**GIVEN** — conditional block:

```markdown
- **GIVEN** the user is an admin:
  - **MUST** return the admin dashboard
  - **MUST NOT** show the onboarding flow
```

Clauses under GIVEN only apply when the precondition holds. GIVEN blocks can nest. GIVEN is not itself a testable clause — it's a grouping construct that adds precondition context. Nested clauses can use any keyword.

**OTHERWISE** — contrary-to-duty fallback:

```markdown
- **MUST** respond within 200ms
  - **OTHERWISE** return a cached response
  - **OTHERWISE** return 504 Gateway Timeout
```

OTHERWISE clauses are nested under an obligation and form an ordered degradation chain. Each level assumes all previous levels also failed. If the primary obligation fails but an OTHERWISE passes, the overall result is a pass (graceful degradation accepted). OTHERWISE cannot appear under MAY, WONT, or GIVEN — only under obligations that can be violated.

**MUST ALWAYS** — invariant:

```markdown
- **MUST ALWAYS** keep database connections below pool maximum
```

An obligation that must hold continuously across all states and inputs. Generates property-based or fuzz-style tests (using `proptest`, `hypothesis`, `fast-check` where available). No time parameter.

**MUST BY** — deadline:

```markdown
- **MUST BY 200ms** return a response
- **MUST BY 5s** complete the batch job
```

An obligation with a time bound. Duration suffixes: `ms`, `s`, `m`. Generates timed assertions measuring wall-clock time, typically at p99. A configurable tolerance multiplier in `ought.toml` accounts for CI environment variability.

### Multi-File Structure and Linking

Specs are meant to be hierarchical. A top-level spec captures broad strokes (the "what"), linking down to detail specs that flesh out specifics (the "how"). Teams own their directory structure — subfolders by component, by feature area, or by service are all valid.

Cross-file references use standard markdown links:

```markdown
requires: [auth](./auth.ought.md), [inventory](./inventory.ought.md)

- **MUST** match behavior defined in [pricing rules](./pricing.ought.md#discount-rules)
```

Top-level specs can annotate links to detail specs:

```markdown
- **MUST** use an LLM to generate concrete test code from specifications

Details: [Generator](./engine/generator.ought.md)
```

`requires:` declares dependencies. Ought builds a DAG and runs specs in dependency order. `Details:` links are informational — they tell humans where to drill in. Anchor links reference specific sections. Circular dependencies are a parse error.

## Architecture

Four distinct phases, each pluggable:

```
┌─────────────────────────────────────────────────────┐
│  .ought.md specs                                    │
└──────────────────────┬──────────────────────────────┘
                       │ parse (pure Rust, no LLM)
                       ▼
┌─────────────────────────────────────────────────────┐
│  Clause IR                                          │
│  keyword, severity, text, condition, otherwise,     │
│  temporal, source location, stable identifier       │
└──────────────────────┬──────────────────────────────┘
                       │ generate (LLM-powered)
                       ▼
┌─────────────────────────────────────────────────────┐
│  Generated test code (ought/ought-gen/)              │
│  Tracked in manifest.toml with content + source     │
│  hashes.                                            │
└──────────────────────┬──────────────────────────────┘
                       │ execute (language runner)
                       ▼
┌─────────────────────────────────────────────────────┐
│  Results mapped back to spec clauses                │
└─────────────────────────────────────────────────────┘
```

### Parser

Pure Rust, zero LLM dependency. Converts `.ought.md` files into a structured clause IR. Published separately as the `ought-spec` crate for ecosystem interop — this is the component that defines the spec format and becomes the open standard.

The parser recognizes CommonMark markdown, extracts metadata, identifies bold keywords, handles GIVEN nesting and OTHERWISE chains, parses MUST BY durations, builds cross-file dependency graphs, and detects circular references.

Each clause gets a stable identifier derived from the section path and clause text (e.g. `auth::login::must_return_jwt`), plus a content hash for change detection.

### Generator

Takes parsed clause IR plus source code context and uses an LLM to produce concrete test implementations. Provider-agnostic via a `Generator` trait. Providers are invoked by exec-ing their CLI tools (`claude`, `chatgpt`, `ollama`) rather than calling APIs directly — this avoids all auth management and lets users use consumer accounts, pro plans, or API keys as they see fit.

- Ships with Claude (execs `claude` CLI) and OpenAI (execs `chatgpt` CLI) providers.
- Ollama support for local models (execs `ollama` CLI).
- Custom providers by specifying an arbitrary executable in `ought.toml`.

**Context assembly** — the generator reads source files from `source:` metadata (or auto-discovers relevant files), schema files, the `context:` block, and code-block hints attached to clauses. Respects a `max_files` limit to stay within LLM context.

**Keyword-specific generation strategies:**

| Keyword | Test pattern |
|---|---|
| MUST / MUST NOT / SHOULD / MAY | Standard assertion tests |
| WONT | Absence tests (capability doesn't exist) or prevention tests (attempt fails gracefully) |
| GIVEN | Shared precondition setup, one test per nested clause |
| OTHERWISE | Failure simulation + fallback verification, one test per chain level, plus an integration test walking the full chain |
| MUST ALWAYS | Property-based / fuzz tests via proptest, hypothesis, fast-check |
| MUST BY | Timed assertions, p99 measurement over multiple iterations |

Tests are self-contained (no cross-test dependencies), include the original clause text as a doc comment, and use the target language's idiomatic patterns.

### Runner

Delegates execution to language-specific test harnesses. Does not implement its own test execution.

Internally there is **one** runner — `CliRunner` — driven entirely by `[runner.<name>]` in `ought.toml`. Built-in **presets** for `rust`, `python`, `typescript`, and `go` ship pre-filled defaults so users only need to pin a `test_dir`; for any other test harness a user writes the command and format themselves.

```toml
# Preset (zero-config):
[runner.python]
test_dir = "ought/ought-gen/"

# Fully custom:
[runner.ruby]
command = "bundle exec rspec --format=junit --out={junit_path}"
test_dir = "spec/ought/"
format = "junit-xml"
file_extensions = ["rb"]
```

Per-test pass/fail is captured via one of four formats:

- `junit-xml` — JUnit XML emitted by the harness (pytest `--junit-xml`, jest-junit, gotestsum, nextest …)
- `tap` — TAP 13 stream
- `cargo-test` — `cargo test`'s default stdout (used by the Rust preset so no third-party reporter is required)
- `ought-json` — native `RunResult` JSON; the escape hatch for custom runners

Test function names are mapped back to clause identifiers via the reversible `__` ↔ `::` convention (e.g. `test_auth__login__must_return_jwt` ↔ `auth::login::must_return_jwt`).

For OTHERWISE chains, the runner executes the parent first, then walks the chain only if the parent fails, stopping at the first passing fallback. For MUST ALWAYS, it captures iteration count. For MUST BY, it captures measured duration.

### Reporter

The visual face of ought. Renders results mapped back to spec clauses.

```
 ought run

 Authentication API          auth.ought.md
 ────────────────────────────────────────────────────
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

Status indicators: `✓` passed, `✗` failed, `!` errored, `⊘` confirmed absent (WONT), `~` skipped, `↳` otherwise fallback.

Color-coding: MUST failures in red, SHOULD in yellow, MAY in dim. Passing clauses dimmed, failures highlighted.

## LLM-Powered Analysis

These features go beyond generation. They reason about the relationships between intent, implementation, and evidence — things that are genuinely impossible without LLMs.

### Failure Narratives — `ought run --diagnose`

When a test fails, don't just show a stack trace. Feed the failure output, source code, and original clause to the LLM:

```
 ✗ MUST return 401 on invalid credentials

 ╭─ Diagnosis ──────────────────────────────────────────────╮
 │ The handler at src/auth/handler.rs:47 catches the        │
 │ InvalidCredentials error but returns 500 instead of 401. │
 │ The error mapping in error.rs:12 was changed in commit   │
 │ a3f9d2e (March 27) — the status code for                 │
 │ AuthError::Invalid was accidentally changed during the   │
 │ error refactor.                                          │
 │                                                          │
 │ Suggested fix: change line 12 of error.rs from           │
 │   AuthError::Invalid => StatusCode(500)                  │
 │ to                                                       │
 │   AuthError::Invalid => StatusCode(401)                  │
 ╰──────────────────────────────────────────────────────────╯
```

Only activated with `--diagnose` (costs API calls).

### Test Quality Grading — `ought run --grade`

A second LLM pass reviews whether the generated test actually validates the clause:

```
 ✓ MUST return 401            grade: C
   Test only checks status code, not response body.
   A 401 from a different middleware would also pass.
```

Grades A through F. Explanations for anything below B. Only activated with `--grade`.

### Survey — `ought analyze survey [path]`

Inverts the flow. Instead of spec-to-tests, goes code-to-gaps:

```
 ought analyze survey src/auth/

 Discovered behaviors not covered by any spec:
   src/auth/handler.rs
     · Token blacklisting on logout (line 89-102)
     · Admin bypass for service accounts (line 134)

   Suggested clauses for auth.ought.md:
     - MUST invalidate token on logout
     - MUST allow service account bypass when role=admin

 Add these to auth.ought.md? [y/n/edit]
```

Never auto-adds clauses without user confirmation.

### Audit — `ought analyze audit`

Cross-spec reasoning about coherence:

```
 ought analyze audit

 Potential conflicts:
   auth.ought.md:14    SHOULD rate-limit to 5 req/min/ip
   perf.ought.md:8     MUST BY 50ms return a response

   Under sustained load, rate-limit middleware adds ~30ms
   of overhead per request. At p99 this could push response
   times past 50ms.

 Gaps:
   auth.ought.md specifies login and refresh but no logout.
   checkout.ought.md assumes auth context exists but doesn't
   specify what happens when the token expires mid-checkout.
```

Also detects: MUST BY deadline conflicts (operation calling sub-operation with a longer deadline), MUST ALWAYS invariant conflicts, contradictory obligations under overlapping GIVEN conditions, and missing OTHERWISE chains on network-dependent operations.

### Blame — `ought debug blame <clause>`

Explains why a clause is failing by correlating with git history:

```
 ought debug blame auth::login::must_return_401

 Timeline:
   ✓ Passing since: 2026-02-15 (42 days)
   ✗ First failure: 2026-03-28 run #347

 What changed:
   commit a3f9d2e (Mar 27, alice)
     "refactor: consolidate error types"

 Story:
   Alice's error refactor merged AuthError and ApiError into a
   single AppError enum. The status code mapping for Invalid moved
   from a match arm in handler.rs to a From impl in error.rs — but
   the From impl defaults to 500 for all auth variants.
```

### Bisect — `ought debug bisect <clause>`

Automated binary search through git history to find the exact breaking commit. Like `git bisect` but targeted at a specific clause. Always restores the working tree to its original state. Supports `--range` to limit search scope and `--continue` to resume after interruption.

## Generated Tests — `ought/ought-gen/`

Generated tests live in `ought/ought-gen/`, tracked by a `manifest.toml`:

```toml
[auth.login.must_return_jwt]
clause_hash = "a1b2c3d4"
source_hash = "e5f6g7h8"
generated_at = "2026-03-29T10:00:00Z"
model = "claude-sonnet-4-6"
```

**Hash-based regeneration.** Two hashes per clause: one from the clause text + context metadata, one from the referenced source files. `ought generate` only regenerates clauses whose hashes have changed. `--force` regenerates everything. `--check` exits 1 if anything is stale (for CI).

**`ought run` never generates.** It only executes existing generated tests. Generation is always an explicit `ought generate` step. Whether teams commit `ought/ought-gen/` to version control or gitignore it is their choice — ought doesn't need to know.

## CLI

```
ought init                        # scaffold ought.toml + example spec
ought run                         # execute tests, report results
ought run --diagnose              # LLM failure narratives
ought run --grade                 # LLM test quality grading
ought generate                    # regenerate stale clauses
ought generate --force            # regenerate everything
ought generate --check            # exit 1 if stale (CI gate)
ought check                       # validate spec syntax only
ought inspect <clause>            # show generated test code
ought diff                        # show pending generation changes
ought analyze survey [path]       # discover uncovered source behaviors
ought analyze audit               # cross-spec coherence analysis
ought debug blame <clause>        # explain a failure with git context
ought debug bisect <clause>       # find the breaking commit
ought watch                       # re-run on file changes
ought mcp serve                   # start MCP server
ought mcp install                 # register with Claude Code, Codex, OpenCode
```

**Exit codes:** 0 = success (or only SHOULD/MAY failures), 1 = MUST-level failure, 2 = usage error. If a MUST fails but an OTHERWISE in its chain passes, exit 0 (graceful degradation accepted).

**Global flags:** `--config`, `--quiet`, `--json`, `--junit <path>`, `--color`, `--verbose`.

## Configuration — `ought.toml`

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
provider = "anthropic"       # or "openai", "ollama"
model = "claude-sonnet-4-6"

[generator.tolerance]
must_by_multiplier = 1.5     # CI timing tolerance for MUST BY

[runner.rust]
test_dir = "ought/ought-gen/"
# Built-in preset expands to: command = "cargo test --no-fail-fast ...",
# format = "cargo-test", file_extensions = ["rs"]. Override any field to
# customize (e.g. to use `cargo nextest run` and format = "junit-xml").

[runner.python]
test_dir = "ought/ought-gen/"
# Preset: pytest --junit-xml={junit_path} -v {test_dir}, format = "junit-xml"

[mcp]
enabled = true
transport = "stdio"
```

## MCP Server

Ought exposes an MCP server so AI assistants and IDE extensions can interact with specs, results, and analysis programmatically.

### Tools

| Tool | Description |
|---|---|
| `ought_run` | Run specs, return structured results |
| `ought_generate` | Regenerate stale or specified clauses |
| `ought_check` | Validate spec syntax |
| `ought_inspect` | Return generated test code for a clause |
| `ought_status` | Spec coverage summary |
| `ought_survey` | Discover uncovered source behaviors |
| `ought_audit` | Cross-spec conflict and gap analysis |
| `ought_blame` | Explain why a clause is failing |
| `ought_bisect` | Find the breaking commit |

### Resources

| URI | Description |
|---|---|
| `ought://specs` | All spec files with clause counts |
| `ought://specs/{name}` | Parsed clauses for a specific spec |
| `ought://results/latest` | Most recent run results |
| `ought://coverage` | Clause coverage map |
| `ought://manifest` | Generation manifest (hashes, staleness) |

Transports: stdio (default, for IDE integration) and SSE (for remote clients).

## Build Tool Integration

Since `ought` is a standalone CLI, integration is minimal:

- **GitHub Actions** — `ought-action` that runs on PR and comments results on the diff.
- **cargo** — `cargo-ought` subcommand.
- **npm** — `npm run ought` via package.json scripts.
- **Makefile / Just** — `just test-ought`.
- **Pre-commit** — `ought check` for spec validation.

CI workflow (no LLM needed):

```yaml
- run: ought run
```

Nightly (with LLM):

```yaml
- run: ought generate --check
```

## Path to Open Standard

The spec format is the standard, not the tool. The strategy:

1. **`ought-spec` crate** — the parser published standalone on crates.io. Any Rust tool can parse `.ought.md` files.
2. **JSON Schema** — a schema describing the clause IR for cross-language interop.
3. **Spec document** — an RFC-style document defining the `.ought.md` format, keywords, and semantics.
4. **The format is just markdown** — no special tooling needed to read or write it. GitHub renders it. Any editor highlights it.

## Implementation

The engine is written in Rust. Key dependencies (anticipated):

- `clap` — CLI argument parsing
- `pulldown-cmark` — CommonMark parsing
- `tokio` — async runtime (process exec, file IO)
- `console` / `indicatif` — terminal styling and progress indicators
- `serde` / `toml` — configuration and manifest

### Crate Structure

```
ought/
  crates/
    ought-spec/        # parser + clause IR (the standard)
    ought-gen/         # generator trait + providers
    ought-run/         # runner trait + language runners
    ought-report/      # reporter + TUI
    ought-analysis/    # survey, audit, blame, bisect
    ought-mcp/         # MCP server
    ought-cli/         # CLI binary, ties everything together
```

The workspace is split so that `ought-spec` has zero dependencies on LLM infrastructure and can be used standalone.
