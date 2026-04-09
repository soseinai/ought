# TODO

## Cross-repo/project spec references

Remote spec references via `requires:` — e.g. `requires: [auth](https://github.com/acme/auth-service/ought/auth.ought.md)`.

Clause-level composition: one team's MUST becomes another team's GIVEN:

```
GIVEN auth-service MUST return 401 for invalid tokens
- MUST forward the 401 to the client unchanged
- MUST NOT retry on 401
```

This creates a contract graph across services. Key capabilities:

- **Resolve remote references** — fetch and parse cross-repo spec clauses
- **Version drift detection** — "auth-service changed that MUST in v3.2, your GIVEN is stale"
- **Cross-boundary test generation** — generate integration tests at service boundaries
- **Staleness dashboard** — "12 cross-service references are stale"

Positioning: like protobuf contracts but for *behavior*, not just shape. Each team owns their specs, cross-references create accountability without requiring a monorepo.
