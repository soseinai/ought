---
title: Deontic keywords
description: The full set of keywords ought recognizes and what each one means.
order: 5
---

Ought is grounded in deontic logic — the formal logic of obligation, permission, and prohibition. Each keyword has a defined severity and behavior on failure.

## Reference

| Keyword        | Severity    | On failure                |
| -------------- | ----------- | ------------------------- |
| `MUST`         | Required    | Error, exit 1             |
| `MUST NOT`     | Required    | Error, exit 1             |
| `SHOULD`       | Recommended | Warning, exit 0           |
| `SHOULD NOT`   | Recommended | Warning, exit 0           |
| `MAY`          | Optional    | Info, exit 0              |
| `WONT`         | Negative    | Error if behavior present |
| `MUST ALWAYS`  | Invariant   | Fuzz / property tests     |
| `MUST BY`      | Deadline    | Error if over time bound  |
| `OTHERWISE`    | Fallback    | Ordered degradation chain |
| `GIVEN`        | Precondition| Scopes nested clauses     |
| `PENDING`      | Modifier    | Declared but not yet generated |

Keywords must be **bold** (`**MUST**`) to be recognized as clauses. A list item beginning with the bare word `MUST` is treated as prose.

## MUST and MUST NOT

The strongest forms. A failed `MUST` clause causes `ought run` to exit non-zero, blocking CI.

```markdown
- **MUST** return a valid JWT token when given correct credentials
- **MUST NOT** leak timing differences between valid and invalid usernames
```

## SHOULD and SHOULD NOT

Recommended behavior. Failures are reported as warnings but do not break the build unless you pass `ought run --fail-on-should`.

```markdown
- **SHOULD** rate-limit to 5 attempts per minute per IP
- **SHOULD NOT** log full request bodies at info level
```

## MAY

Optional behavior. The presence of a `MAY` clause documents that something is permitted, and `ought` verifies it doesn't break anything if implemented.

```markdown
- **MAY** support "remember me" extended token expiry
```

## WONT

The opposite of `MUST` — a behavior that explicitly should not exist. `ought run` confirms the absence and reports an error if the behavior is detected.

```markdown
- **WONT** support basic auth (deprecated in v2)
```

## MUST ALWAYS

An invariant that must hold across all inputs. Ought generates property-based / fuzz tests rather than example-based tests.

```markdown
- **MUST ALWAYS** return a response with a stable schema regardless of input
```

## MUST BY

A `MUST` with a time bound. The unit follows the number — valid units are `ms`, `s`, and `m`. The clause text follows the bound:

```markdown
- **MUST BY 200ms** return a response under normal load
```

## OTHERWISE

`OTHERWISE` defines an ordered fallback chain under a parent obligation. If the parent fails, the first `OTHERWISE` is attempted, and so on. `OTHERWISE` must be nested (indented) under a `MUST`, `MUST NOT`, `SHOULD`, `SHOULD NOT`, `MUST ALWAYS`, or `MUST BY` — it cannot stand alone or live under `MAY`, `WONT`, or `GIVEN`.

```markdown
- **MUST BY 200ms** return a response under normal load
  - **OTHERWISE** return a cached session token
  - **OTHERWISE** return 503 with a Retry-After header
```

## GIVEN

A grouping construct that scopes its nested clauses under a shared precondition. `GIVEN` itself produces no test — it only contextualizes the clauses below it.

```markdown
- **GIVEN** the refresh token is valid and not expired
  - **MUST** issue a new access token
  - **SHOULD** rotate the refresh token
```

## PENDING

An optional prefix that marks a clause as deferred: the intent is declared but the generator must not emit a test and the runner reports it as `pending` (not pass, fail, or skip). To promote a pending clause, delete the `PENDING ` prefix.

```markdown
- **PENDING MUST** support WebAuthn passkeys as an alternative to passwords
```

`PENDING` may precede any clause-producing keyword (`MUST`, `MUST NOT`, `SHOULD`, `SHOULD NOT`, `MAY`, `WONT`, `MUST ALWAYS`, `MUST BY`, `OTHERWISE`). It cannot stand alone and cannot modify `GIVEN`.

**Propagation.** `PENDING` propagates only down the `OTHERWISE` fallback chain — a deferred obligation defers its fallbacks. It does **not** propagate to other nested clauses; each clause's strength is explicit at its declaration site. An explicit `PENDING OTHERWISE` under a non-pending parent is allowed and useful: the happy path ships while the fallback is still being built.
