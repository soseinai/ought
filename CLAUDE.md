# Ought

Behavioral test framework: specs in `.ought.md`, LLM-generated tests.

## Key references

- **Grammar**: `docs/grammar.md` is the source of truth for the `.ought.md` spec grammar. The parser in `crates/ought-spec/src/parser.rs` must conform to this grammar. When changing parsing behavior, update the grammar first.
- **Design**: `docs/design.md` for architecture and philosophy.
- **Specs as requirements**: the specs in `ought/` ARE the project's own requirements. Use `ought check` to validate.

## Build & test

```
cargo build
cargo test
```

## Workspace layout

```
crates/
  ought-spec/        # parser + clause IR (the open standard, zero LLM deps)
  ought-gen/         # generator trait + providers
  ought-run/         # runner trait + language runners
  ought-report/      # reporter + TUI
  ought-analysis/    # survey, audit, blame, bisect
  ought-mcp/         # MCP server
  ought-server/      # viewer web UI (Svelte + shadcn)
  ought-cli/         # CLI binary
```
