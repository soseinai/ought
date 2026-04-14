---
title: Writing specs
description: The structure of an .ought.md spec file.
order: 4
---

A spec is a markdown file with the extension `.ought.md`. It is plain markdown — it renders correctly in GitHub, displays in any editor, and requires no special tooling to read.

The authoritative grammar lives in `docs/grammar.md`. This page is the practical walkthrough.

## File layout

A spec has three parts:

1. A top-level `# Heading` naming the area being specified
2. A metadata block of key-value lines giving the LLM context
3. One or more `## Section` blocks containing clauses

```markdown
# User Authentication

context: REST API at /api/auth, uses JWT tokens
source: src/auth/

## Login

- **MUST** return a valid JWT token when given correct credentials
- **MUST** return 401 with a generic error when credentials are invalid
```

## Metadata keys

The metadata block sits between the top-level heading and the first `##` section. Keys:

- **context** — a one-line description of the system being tested. The LLM uses this to disambiguate vague clauses.
- **source** — one or more paths or globs pointing at the source code being specified. Used by `ought generate` and `ought debug blame`. Multiple values are comma-separated.
- **schema** — one or more paths or globs to schema or type definitions (OpenAPI, JSON Schema, `.d.ts`, etc.) that constrain the behavior.
- **requires** — other specs this one depends on. Values can be bare paths or markdown links, optionally with an anchor: `[users](./users.ought.md#session)`.

Metadata appears only between the H1 and the first section heading. Metadata-looking lines after the first `##` are treated as prose.

## Clauses

Clauses are list items beginning with a **bold** deontic keyword. The keyword sets the severity:

```markdown
- **MUST** return a valid JWT token when given correct credentials
- **SHOULD** rate-limit to 5 attempts per minute per IP
- **WONT** support basic auth (deprecated in v2)
```

Keywords must be bold. A list item starting with the bare word `MUST` is treated as prose. See [Deontic keywords](/products/ought/docs/deontic-keywords) for the full list.

## Nested clauses and OTHERWISE fallbacks

Clauses nest with two-space indentation. Use `OTHERWISE` for ordered fallback chains under an obligation:

```markdown
- **MUST BY 200ms** return a response under normal load
  - **OTHERWISE** return a cached session token
  - **OTHERWISE** return 503 with a Retry-After header
```

`OTHERWISE` must live under a `MUST`, `MUST NOT`, `SHOULD`, `SHOULD NOT`, `MUST ALWAYS`, or `MUST BY` — it can't stand alone or sit under `MAY`, `WONT`, or `GIVEN`.

## GIVEN blocks

`GIVEN` is a bolded list item that groups nested clauses under a shared precondition. It produces no test of its own.

```markdown
- **GIVEN** the refresh token is valid and not expired
  - **MUST** issue a new access token
  - **SHOULD** rotate the refresh token
```

## PENDING clauses

Prefix any clause-producing keyword with `PENDING` to declare intent without generating a test yet. The runner reports these as `pending`.

```markdown
- **PENDING MUST** support WebAuthn passkeys as an alternative to passwords
```

Delete the `PENDING ` prefix to promote the clause once you're ready to enforce it.

## Hints

A fenced code block placed immediately after a clause is attached to that clause as a generation hint. Use hints to give the LLM concrete examples, expected payloads, or fixture data:

```markdown
- **MUST** return 401 with a generic error when credentials are invalid
  ```json
  { "error": "invalid_credentials" }
  ```
```

## Comments and prose

Anything in the spec that isn't a clause, metadata line, or hint is treated as documentation for human readers. The LLM still reads it for context, but it produces no test.
