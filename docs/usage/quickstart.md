---
title: Quick start
description: Write your first spec, generate tests, and run them.
order: 3
---

This walkthrough takes a small project from zero to a passing `ought run` in three steps.

## 1. Initialize

From the root of your project:

```sh
ought init
```

This creates an `ought.toml` config file and an `ought/` directory with an example spec. The language runner is picked automatically from the files in your project (`Cargo.toml` → rust, `package.json` → typescript, etc.).

## 2. Write a spec

Replace the example with `ought/auth.ought.md`, describing what your authentication endpoint should do:

```markdown
# User Authentication

context: REST API at /api/auth, uses JWT tokens
source: src/auth/

## Login

- **MUST** return a valid JWT token when given correct credentials
- **MUST** return 401 with a generic error when credentials are invalid
- **SHOULD** rate-limit to 5 attempts per minute per IP
```

The `context:` and `source:` lines tell the LLM what your code looks like and where it lives. Everything else is plain markdown.

## 3. Generate tests

```sh
ought generate
```

The LLM reads your spec and the source files under `src/auth/`, then writes test files into the test directory configured in `ought.toml` (by default `ought/ought-gen/`). Each test is annotated with the clause it enforces.

## 4. Run

```sh
ought run
```

You'll see a report grouped by spec section, with each clause marked as passing, failing, or confirmed absent (for `WONT` clauses).

## What's next

- [Writing specs](/products/ought/docs/writing-specs) — the full spec syntax
- [Deontic keywords](/products/ought/docs/deontic-keywords) — every keyword and what it means
- [CLI reference](/products/ought/docs/cli-reference) — every command
